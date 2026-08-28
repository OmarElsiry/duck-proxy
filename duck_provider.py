import time
import base64
import litellm
from litellm import CustomLLM, ModelResponse
from litellm.types.utils import GenericStreamingChunk
from litellm.utils import Choices, Message, Delta, ImageResponse
from duck_ai import DuckChat, resolve_model, image_generation

_USAGE = None  # DuckChat gives no token counts; SSE chunk requires usage field

_SHARED_CHAT = None


def _get_chat_client(model=None) -> DuckChat:
    global _SHARED_CHAT
    if _SHARED_CHAT is None:
        _SHARED_CHAT = DuckChat()
    return _SHARED_CHAT


class DuckProvider(CustomLLM):
    def _content(self, msg):
        c = msg.get("content", "")
        if isinstance(c, list):
            return "".join(p.get("text", "") for p in c if isinstance(p, dict))
        return c or ""

    def _prepare_messages(self, messages):
        out = []
        for m in messages:
            role = m.get("role", "user")
            content = self._content(m)
            out.append({"role": role, "content": content})
        return out

    def completion(self, model, messages, api_base, custom_prompt_dict,
                   model_response, print_verbose, encoding, api_key, logging_obj,
                   optional_params, acompletion=None, litellm_params=None,
                   logger_fn=None, headers={}, timeout=None, client=None, **kwargs):
        m = model.split("/", 1)[-1]
        ws = optional_params.get("web_search", False)
        chat = _get_chat_client()
        formatted_messages = self._prepare_messages(messages)
        out = chat.ask(formatted_messages, model=m, web_search=ws)
        model_response.choices = [Choices(message=Message(content=out, role="assistant"))]
        model_response.model = model
        return model_response

    def streaming(self, model, messages, api_base, custom_prompt_dict,
                  model_response, print_verbose, encoding, api_key, logging_obj,
                  optional_params, acompletion=None, litellm_params=None,
                  logger_fn=None, headers={}, timeout=None, client=None, **kwargs):
        m = model.split("/", 1)[-1]
        ws = optional_params.get("web_search", False)
        chat = _get_chat_client()
        formatted_messages = self._prepare_messages(messages)
        for chunk in chat.stream(formatted_messages, model=m, web_search=ws):
            yield GenericStreamingChunk(text=chunk, is_finished=False,
                                        finish_reason="", usage=_USAGE)
        yield GenericStreamingChunk(text="", is_finished=True,
                                    finish_reason="stop", usage=_USAGE)

    def image_generation(self, model, prompt, api_key, api_base, model_response,
                         optional_params, logging_obj, timeout=None, client=None):
        chat = _get_chat_client()
        data = chat.generate_image(prompt)
        model_response.created = int(time.time())
        model_response.data = [{"b64_json": base64.b64encode(data).decode()}]
        return model_response


litellm.custom_provider_map = [{"provider": "duck", "custom_handler": DuckProvider()}]

