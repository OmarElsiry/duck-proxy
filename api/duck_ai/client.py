from __future__ import annotations

import base64
import json
import logging
import random
import threading
import time
import uuid
from typing import Any, Dict, Iterator, List, Optional, Union

import httpx

from .challenge import make_fe_signals, solve_challenge
from .durable import generate_jwk
from .exceptions import (
    APIError,
    ChallengeError,
    ConversationLimitError,
    DuckChatError,
    RateLimitError,
)
from .models import (
    History,
    ImagePart,
    Message,
    ModelType,
    Role,
    image_generation,
    model_supports_vision,
    model_supports_web_search,
    resolve_effort,
    resolve_model,
    vision_capable_default,
)

log = logging.getLogger("duck_ai")

BASE = "https://duck.ai"
DDG_BASE = "https://duckduckgo.com"
DEFAULT_UA = (
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
    "Chrome/150.0.0.0 Safari/537.36"
)
DEFAULT_FE_VERSION = "serp_20260827_190157_ET-5738d187a3dbca905a80324bd698765a27bf6e44"
TOOL_CHOICE_OFF = {
    "NewsSearch": False,
    "VideosSearch": False,
    "LocalSearch": False,
    "WeatherForecast": False,
}

CHAT_PATH = "/duckchat/v1/chat"
IMAGES_PATH = "/duckchat/v1/chat"


def guard_reasoning_mode(payload: Dict[str, Any]) -> bool:
    """Runtime and test guard ensuring a chat payload is strictly in Duck.ai reasoning mode."""
    model = payload.get("model", "")
    tool_choice = payload.get("metadata", {}).get("toolChoice", {})
    if tool_choice.get("GenerateImage"):
        return True
    from .models import model_supports_reasoning
    if model_supports_reasoning(model):
        effort = payload.get("reasoningEffort")
        if effort not in ("low", "medium"):
            raise ValueError(
                f"ReasoningModeGuardViolation: model '{model}' expected reasoningEffort in ('low', 'medium'), got '{effort}'"
            )
    return True


class DuckChat:
    def __init__(
        self,
        model: Union[ModelType, str] = "gpt5_luna",
        *,
        effort: Optional[str] = "reasoning",
        user_agent: Optional[str] = None,
        fe_version: Optional[str] = None,
        client: Optional[httpx.Client] = None,
        timeout: float = 60.0,
        max_retries: int = 3,
        backoff_base: float = 0.6,
        warm_session: bool = True,
        aggressive_warm: bool = False,
        history: bool = False,
    ):
        self.model = resolve_model(model)
        self.effort = effort
        self.user_agent = user_agent or DEFAULT_UA
        self.fe_version = fe_version or DEFAULT_FE_VERSION
        self.timeout = timeout
        self.max_retries = max(1, int(max_retries))
        self.backoff_base = max(0.0, float(backoff_base))
        self.aggressive_warm = aggressive_warm
        self.history_enabled = bool(history)
        self.history = History(model=self.model)
        self._journey_id = uuid.uuid4().hex
        self._jwk: Optional[Dict[str, Any]] = None
        self._jwk_lock = threading.Lock()
        self._owns_client = client is None
        self._client = client or httpx.Client(
            http2=False,
            timeout=httpx.Timeout(timeout, connect=15.0, read=timeout),
            headers={
                "User-Agent": self.user_agent,
                "Accept-Language": "en-US,en;q=0.9",
                "Referer": f"{BASE}/",
                "Origin": BASE,
                "Sec-Fetch-Dest": "empty",
                "Sec-Fetch-Mode": "cors",
                "Sec-Fetch-Site": "same-origin",
                "Sec-Ch-Ua": '"Not;A=Brand";v="8", "Chromium";v="150", "Google Chrome";v="150"',
                "Sec-Ch-Ua-Mobile": "?0",
                "Sec-Ch-Ua-Platform": '"Linux"',
            },
            follow_redirects=True,
        )
        self._warmed = not warm_session
        self._pending_hash: Optional[str] = None
        self._seeded = False
        self._journey_id = uuid.uuid4().hex
        if warm_session:
            try:
                self.warm()
            except Exception as e:
                log.debug("warm-up failed: %s", e)
        if warm_session and aggressive_warm:
            try:
                self.seed_session()
            except Exception as e:
                log.debug("session seed failed: %s", e)

    def __enter__(self) -> "DuckChat":
        return self

    def __exit__(self, *exc) -> None:
        self.close()

    def close(self) -> None:
        if self._owns_client:
            try:
                self._client.close()
            except Exception:
                pass

    def reset(self) -> None:
        self.history.clear()

    def enable_history(self) -> None:
        self.history_enabled = True

    def disable_history(self) -> None:
        self.history_enabled = False
        self.history.clear()

    def warm(self) -> None:
        if self._warmed:
            return
        try:
            self._client.get(f"{BASE}/", timeout=10.0)
            self._client.get(f"{BASE}/duckchat/v1/auth/token", timeout=10.0)
        finally:
            self._warmed = True

    def seed_session(self) -> None:
        self._seeded = True

    def fetch_challenge_header(self) -> str:
        pending = getattr(self, "_pending_hash", None)
        if pending:
            self._pending_hash = None
            return solve_challenge(pending, self.user_agent)

        last_resp = None
        for attempt in range(5):
            r = self._client.get(
                f"{BASE}/duckchat/v1/status",
                headers={
                    "Accept": "*/*",
                    "x-vqd-accept": "1",
                    "x-ddg-journey-id": self._journey_id,
                    "Cache-Control": "no-store",
                    "Pragma": "no-cache",
                    "Referer": f"{BASE}/",
                    "Origin": BASE,
                },
                timeout=self.timeout,
            )
            last_resp = r
            if r.status_code == 200:
                break
            if r.status_code == 429 and attempt < 4:
                time.sleep(4.0)
                continue
            if r.status_code >= 400:
                break

        if last_resp is None:
            raise DuckChatError("failed to contact status endpoint")
        if last_resp.status_code == 429:
            raise RateLimitError(last_resp.text)
        if last_resp.status_code >= 400:
            raise APIError(
                f"status endpoint failed: {last_resp.status_code}",
                last_resp.status_code,
                last_resp.text,
            )
        challenge = last_resp.headers.get("x-vqd-hash-1")
        if not challenge:
            raise DuckChatError("server did not return x-vqd-hash-1 challenge")
        return solve_challenge(challenge, self.user_agent)

    def get_jwk(self) -> Dict[str, Any]:
        if self._jwk is None:
            with self._jwk_lock:
                if self._jwk is None:
                    self._jwk = generate_jwk()
        return self._jwk

    def build_payload(
        self,
        messages: List[Dict[str, Any]],
        *,
        model: Optional[str] = None,
        can_use_tools: bool = True,
        effort: Optional[str] = None,
        web_search: bool = False,
    ) -> Dict[str, Any]:
        m = resolve_model(model or self.model)
        tool_choice: Dict[str, bool] = dict(TOOL_CHOICE_OFF)
        if m == image_generation:
            m = "gpt-5.6-luna"
            tool_choice["GenerateImage"] = True
        elif web_search and model_supports_web_search(m):
            tool_choice["WebSearch"] = True
        effort_val = "none" if m == "gpt-5.6-luna" and tool_choice.get("GenerateImage") else (
            resolve_effort(m, effort if effort is not None else self.effort) or "low"
        )
        payload: Dict[str, Any] = {
            "model": m,
            "metadata": {
                "canUseWebSearch": True,
                "toolChoice": tool_choice,
            },
            "messages": messages,
            "canUseTools": can_use_tools,
            "reasoningEffort": effort_val,
            "canUseApproxLocation": None,
            "canDelegateImageGeneration": None,
            "canUseWebSearch": True,
            "canUploadFiles": None,
            "durableStream": {
                "messageId": str(uuid.uuid4()),
                "conversationId": str(uuid.uuid4()),
                "publicKey": self.get_jwk(),
            },
        }
        guard_reasoning_mode(payload)
        return payload

    @staticmethod
    def endpoint_for(model: str) -> str:
        return CHAT_PATH

    @staticmethod
    def has_image(messages: List[Dict[str, Any]]) -> bool:
        for m in messages:
            c = m.get("content")
            if isinstance(c, list):
                for p in c:
                    if isinstance(p, dict) and p.get("type") == "image":
                        return True
        return False

    def chat_stream(self, payload: Dict[str, Any]):
        hash_header = self.fetch_challenge_header()
        path = self.endpoint_for(payload.get("model", self.model))
        return self._client.stream(
            "POST",
            f"{BASE}{path}",
            content=json.dumps(payload),
            headers={
                "Content-Type": "application/json",
                "Accept": "text/event-stream",
                "x-vqd-hash-1": hash_header,
                "x-fe-signals": make_fe_signals(),
                "x-fe-version": self.fe_version,
                "x-ddg-journey-id": self._journey_id,
                "Referer": f"{BASE}/",
                "Origin": BASE,
            },
        )

    @staticmethod
    def raise_for_status(resp: "httpx.Response") -> None:
        try:
            resp.read()
            body = resp.text
        except Exception:
            body = ""
        if resp.status_code == 418:
            raise ChallengeError(f"server rejected challenge: {body[:200]}")
        if resp.status_code == 429:
            if "ERR_CONVERSATION_LIMIT" in body:
                raise ConversationLimitError(body)
            raise RateLimitError(body)
        raise APIError(
            f"chat failed: HTTP {resp.status_code}", resp.status_code, body
        )

    @staticmethod
    def iter_sse(resp: "httpx.Response") -> Iterator[str]:
        for raw in resp.iter_lines():
            if not raw:
                continue
            if not raw.startswith("data:"):
                continue
            data = raw[5:].lstrip()
            if data:
                yield data

    def attempt_stream(self, payload: Dict[str, Any]) -> Iterator[dict]:
        with self.chat_stream(payload) as resp:
            if resp.status_code != 200:
                self.raise_for_status(resp)
            new_hash = resp.headers.get("x-vqd-hash-1")
            if new_hash:
                self._pending_hash = new_hash
            saw_any = False
            for data in self.iter_sse(resp):
                if data == "[DONE]":
                    return
                if (
                    data.startswith("[CHAT_TITLE")
                    or data.startswith("[LIMIT")
                    or data.startswith("[PING")
                ):
                    continue
                try:
                    obj = json.loads(data)
                except Exception:
                    continue
                if obj.get("action") == "error":
                    msg = (
                        obj.get("type")
                        or obj.get("error")
                        or json.dumps(obj)
                    )
                    if obj.get("status") == 429:
                        if msg == "ERR_CONVERSATION_LIMIT":
                            raise ConversationLimitError(msg)
                        raise RateLimitError(msg)
                    if msg in ("ERR_CHALLENGE", "ERR_INVALID_CHALLENGE"):
                        raise ChallengeError(str(msg))
                    raise APIError(str(msg), obj.get("status"), data)
                saw_any = True
                yield obj
            if not saw_any:
                raise APIError("empty stream from duck.ai", 200, "")

    def stream_with_retry(self, payload: Dict[str, Any]) -> Iterator[dict]:
        last_exc: Optional[BaseException] = None
        for attempt in range(self.max_retries):
            yielded = False
            try:
                for item in self.attempt_stream(payload):
                    yielded = True
                    yield item
                return
            except (ChallengeError, httpx.RemoteProtocolError, httpx.ReadError) as e:
                self._pending_hash = None
                if yielded:
                    raise
                last_exc = e
            except APIError as e:
                if e.status_code is None or e.status_code >= 500:
                    if yielded:
                        raise
                    last_exc = e
                else:
                    raise
            except RateLimitError as e:
                if isinstance(e, ConversationLimitError):
                    raise
                if yielded:
                    raise
                last_exc = e
            except httpx.TimeoutException as e:
                if yielded:
                    raise
                last_exc = e
            if attempt < self.max_retries - 1:
                delay = self.backoff_base * (2**attempt) + random.uniform(0, 0.25)
                log.debug(
                    "duck.ai retry %d/%d after %.2fs",
                    attempt + 2,
                    self.max_retries,
                    delay,
                )
                time.sleep(delay)
        if last_exc is not None:
            raise last_exc
        raise DuckChatError("exhausted retries with no specific error")

    def stream(
        self,
        prompt: Union[str, List[Union[str, ImagePart, dict]]],
        *,
        remember: Optional[bool] = None,
        model: Optional[Union[ModelType, str]] = None,
        effort: Optional[str] = None,
        web_search: bool = False,
    ) -> Iterator[str]:
        if isinstance(prompt, list) and len(prompt) > 0 and isinstance(prompt[0], dict) and "role" in prompt[0]:
            messages = prompt
        elif remember:
            self.history.add_user(prompt)
            messages = self.history.to_messages()
        else:
            messages = [Message(role=Role.User.value, content=prompt).to_dict()]
        is_mm = self.has_image(messages)
        if model is not None:
            use_model = resolve_model(model)
        elif is_mm:
            from .models import model_supports_vision as msv
            use_model = (
                self.model
                if msv(self.model)
                else vision_capable_default()
            )
        else:
            use_model = self.model
        payload = self.build_payload(
            messages,
            model=use_model,
            effort=effort,
            web_search=web_search,
        )
        collected: List[str] = []
        for obj in self.stream_with_retry(payload):
            chunk = obj.get("message") or ""
            if chunk:
                collected.append(chunk)
                yield chunk
        if remember and collected:
            self.history.add_assistant("".join(collected))

    def ask(
        self,
        prompt: Union[str, List[Union[str, ImagePart, dict]]],
        *,
        remember: Optional[bool] = None,
        model: Optional[Union[ModelType, str]] = None,
        effort: Optional[str] = None,
        web_search: bool = False,
    ) -> str:
        return "".join(
            self.stream(
                prompt,
                remember=remember,
                model=model,
                effort=effort,
                web_search=web_search,
            )
        )

    def ask_with_image(
        self,
        prompt: str,
        image: Union[str, bytes, ImagePart],
        *,
        mime_type: str = "image/png",
        remember: Optional[bool] = None,
        model: Optional[Union[ModelType, str]] = None,
        effort: Optional[str] = None,
        web_search: bool = False,
    ) -> str:
        part = self.coerce_image(image, mime_type)
        return self.ask(
            [prompt, part],
            remember=remember,
            model=model,
            effort=effort,
            web_search=web_search,
        )

    @staticmethod
    def coerce_image(image: Union[str, bytes, ImagePart], mime_type: str) -> ImagePart:
        if isinstance(image, ImagePart):
            return image
        if isinstance(image, bytes):
            return ImagePart.from_bytes(image, mime_type=mime_type)
        if isinstance(image, str):
            if image.startswith("data:"):
                return ImagePart(image=image, mime_type=mime_type)
            return ImagePart.from_path(image)
        raise TypeError(f"unsupported image type: {type(image).__name__}")

    def generate_image(self, prompt: str, *, save_to: Optional[str] = None) -> bytes:
        return self.run_image_request(content=prompt, save_to=save_to)

    def edit_image(
        self,
        prompt: str,
        image: Union[str, bytes, ImagePart],
        *,
        mime_type: str = "image/png",
        save_to: Optional[str] = None,
    ) -> bytes:
        part = self.coerce_image(image, mime_type)
        return self.run_image_request(content=[prompt, part], save_to=save_to)

    def run_image_request(
        self,
        *,
        content: Union[str, List[Union[str, ImagePart, dict]]],
        save_to: Optional[str],
    ) -> bytes:
        messages = [Message(role=Role.User.value, content=content).to_dict()]
        payload = self.build_payload(messages, model=image_generation, can_use_tools=True)
        partials: List[str] = []
        final: Optional[str] = None
        for obj in self.stream_with_retry(payload):
            role = obj.get("role") or ""
            result = obj.get("result") or ""
            data_obj = obj.get("data")
            if isinstance(data_obj, dict) and "b64Image" in data_obj:
                final = data_obj["b64Image"]
            elif role == "partial-image" and result:
                partials.append(result)
            elif role in ("generated-image", "image") and result:
                final = result
        b64 = final if final else "".join(partials)
        if not b64:
            raise DuckChatError("image generation returned no data")
        if "," in b64 and b64.startswith("data:"):
            b64 = b64.split(",", 1)[1]
        data = base64.b64decode(b64)
        if save_to:
            with open(save_to, "wb") as f:
                f.write(data)
        return data
