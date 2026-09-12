from __future__ import annotations

import base64
import mimetypes
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Union


class ModelType(str, Enum):
    GPT5Luna = "gpt-5.6-luna"
    GPT5Mini = "gpt-5.4-mini"
    GPT5Nano = "gpt-5.4-nano"
    Claude = "claude-haiku-4-5"
    ClaudeHaiku = "claude-haiku-4-5"
    Mistral = "mistral-small-2603"
    MistralSmall = "mistral-small-2603"
    GptOss = "tinfoil/gpt-oss-120b"
    Gemma = "tinfoil/gemma4-31b"
    ImageGeneration = "image-generation"

    def __str__(self) -> str:
        return self.value


gpt5_luna = "gpt-5.6-luna"
luna = "gpt-5.6-luna"
gpt5_mini = "gpt-5.4-mini"
gpt5 = "gpt-5.6-luna"
gpt5_nano = "gpt-5.4-nano"
nano = "gpt-5.4-nano"
claude = "claude-haiku-4-5"
claude_haiku = "claude-haiku-4-5"
mistral = "mistral-small-2603"
mistral_small = "mistral-small-2603"
gpt_oss = "tinfoil/gpt-oss-120b"
gpt_oss_120b = "tinfoil/gpt-oss-120b"
gemma = "tinfoil/gemma4-31b"
image_generation = "image-generation"

MODEL_ALIASES: Dict[str, str] = {
    "luna": gpt5_luna,
    "gpt-5.6-luna": gpt5_luna,
    "gpt-5.6": gpt5_luna,
    "gpt5_luna": gpt5_luna,
    "gpt5": gpt5_luna,
    "gpt-5": gpt5_luna,
    "gpt5-mini": gpt5_mini,
    "gpt-5-mini": gpt5_mini,
    "gpt5_mini": gpt5_mini,
    "gpt5.4": gpt5_mini,
    "gpt-5.4": gpt5_mini,
    "gpt5.4-mini": gpt5_mini,
    "gpt-5.4-mini": gpt5_mini,
    "gpt5.4_mini": gpt5_mini,
    "gpt5_nano": gpt5_nano,
    "gpt5.4-nano": gpt5_nano,
    "gpt-5.4-nano": gpt5_nano,
    "gpt5.4_nano": gpt5_nano,
    "nano": gpt5_nano,
    "claude": claude,
    "claude-haiku": claude_haiku,
    "claude_haiku": claude_haiku,
    "claude-haiku-4-5": claude_haiku,
    "haiku": claude_haiku,
    "mistral": mistral,
    "mistral-small": mistral_small,
    "mistral_small": mistral_small,
    "mistral-small-2603": mistral_small,
    "gpt-oss": gpt_oss,
    "gpt_oss": gpt_oss,
    "gpt-oss-120b": gpt_oss_120b,
    "gpt_oss_120b": gpt_oss_120b,
    "tinfoil/gpt-oss-120b": gpt_oss_120b,
    "gemma": gemma,
    "gemma4": gemma,
    "tinfoil/gemma4-31b": gemma,
    "image-generation": image_generation,
    "image_generation": image_generation,
    "image": image_generation,
    "img": image_generation,
}

_MODEL_CAPABILITIES: Dict[str, Dict[str, object]] = {
    "gpt-5.6-luna": {
        "reasoning": True,
        "vision": True,
        "web_search": True,
        "fast": "none",
        "default": "low",
        "thinking": "low",
    },
    "gpt-5.4-mini": {
        "reasoning": True,
        "vision": True,
        "web_search": True,
        "fast": "none",
        "default": "medium",
        "thinking": "medium",
        "max": "medium",
    },
    "gpt-5.4-nano": {
        "reasoning": True,
        "vision": True,
        "web_search": True,
        "fast": "none",
        "default": "medium",
        "thinking": "medium",
        "max": "medium",
    },
    "claude-haiku-4-5": {
        "reasoning": True,
        "vision": True,
        "web_search": True,
        "fast": "none",
        "default": "low",
        "thinking": "low",
    },
    "mistral-small-2603": {
        "reasoning": False,
        "vision": False,
        "web_search": False,
    },
    "tinfoil/gpt-oss-120b": {
        "reasoning": True,
        "vision": False,
        "web_search": False,
        "fast": "none",
        "default": "low",
        "thinking": "low",
    },
    "tinfoil/gemma4-31b": {
        "reasoning": True,
        "vision": False,
        "web_search": True,
        "fast": "none",
        "default": "low",
        "thinking": "low",
    },
    "image-generation": {
        "reasoning": False,
        "vision": True,
        "web_search": False,
    },
}


def resolve_model(name: Union["ModelType", str, None]) -> str:
    if name is None:
        return gpt5_mini
    if isinstance(name, ModelType):
        return name.value
    if not isinstance(name, str):
        return str(name)
    key = name.strip()
    return MODEL_ALIASES.get(key.lower(), key)


def list_models() -> List[str]:
    return list(_MODEL_CAPABILITIES.keys())


def model_supports_reasoning(model: Union["ModelType", str]) -> bool:
    return bool(
        _MODEL_CAPABILITIES.get(resolve_model(model), {}).get("reasoning", False)
    )


def model_supports_vision(model: Union["ModelType", str]) -> bool:
    return bool(
        _MODEL_CAPABILITIES.get(resolve_model(model), {}).get("vision", False)
    )


def model_supports_web_search(model: Union["ModelType", str]) -> bool:
    return bool(
        _MODEL_CAPABILITIES.get(resolve_model(model), {}).get("web_search", False)
    )


def vision_capable_default() -> str:
    return "gpt-5.4-mini"


def resolve_effort(
    model: Union["ModelType", str], effort: Optional[str] = None
) -> Optional[str]:
    cap = _MODEL_CAPABILITIES.get(resolve_model(model), {})
    if not cap.get("reasoning"):
        return None
    # Duck Proxy is enforced to operate in adaptive max reasoning mode ("medium" if supported, else "low")
    if effort in ("max", "high", "medium"):
        return str(cap.get("max", cap.get("thinking", "low")))
    return str(cap.get("thinking", "low"))


class Role(str, Enum):
    User = "user"
    Assistant = "assistant"
    System = "system"

    def __str__(self) -> str:
        return self.value


@dataclass
class ImagePart:
    image: str
    mime_type: str = "image/png"

    @classmethod
    def from_bytes(cls, data: bytes, mime_type: str = "image/png") -> "ImagePart":
        b64 = base64.b64encode(data).decode("ascii")
        return cls(image=f"data:{mime_type};base64,{b64}", mime_type=mime_type)

    @classmethod
    def from_path(cls, path: str, mime_type: Optional[str] = None) -> "ImagePart":
        mt = mime_type or mimetypes.guess_type(path)[0] or "image/png"
        with open(path, "rb") as f:
            return cls.from_bytes(f.read(), mime_type=mt)

    def to_part(self) -> dict:
        return {"type": "image", "mimeType": self.mime_type, "image": self.image}


Content = Union[str, List[Union[str, "ImagePart", dict]]]


@dataclass
class Message:
    role: str
    content: Content

    def to_dict(self) -> dict:
        if isinstance(self.content, str):
            return {"role": str(self.role), "content": self.content}
        parts: List[dict] = []
        for p in self.content:
            if isinstance(p, str):
                parts.append({"type": "text", "text": p})
            elif isinstance(p, ImagePart):
                parts.append(p.to_part())
            elif isinstance(p, dict):
                parts.append(p)
            else:
                raise TypeError(f"Unsupported content part: {type(p).__name__}")
        return {"role": str(self.role), "content": parts}


@dataclass
class History:
    model: str = gpt5_mini
    messages: List[Message] = field(default_factory=list)

    def add_user(self, content: Content) -> None:
        self.messages.append(Message(role=Role.User.value, content=content))

    def add_assistant(self, content: str) -> None:
        self.messages.append(Message(role=Role.Assistant.value, content=content))

    def to_messages(self) -> List[dict]:
        return [m.to_dict() for m in self.messages]

    def clear(self) -> None:
        self.messages.clear()
