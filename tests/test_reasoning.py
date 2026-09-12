import sys
from pathlib import Path

_repo_root = Path(__file__).resolve().parent.parent
if str(_repo_root) not in sys.path:
    sys.path.insert(0, str(_repo_root))

import pytest
from api.duck_ai import (
    DuckChat,
    guard_reasoning_mode,
    resolve_effort,
    model_supports_reasoning,
    gpt5_luna,
    gpt5_mini,
    claude,
    gemma,
    gpt_oss,
    image_generation,
)


def test_duckchat_default_reasoning_mode():
    """Verify DuckChat defaults to reasoning mode."""
    chat = DuckChat(warm_session=False)
    assert chat.effort == "reasoning"

    payload = chat.build_payload([{"role": "user", "content": "hello"}])
    assert payload["reasoningEffort"] == "low"
    assert guard_reasoning_mode(payload) is True


def test_models_reasoning_effort_enforcement():
    """Verify all reasoning models enforce adaptive max reasoningEffort ('low' or 'medium')."""
    chat = DuckChat(warm_session=False)
    
    # gpt-5.4-mini supports medium as max
    payload_mini = chat.build_payload([{"role": "user", "content": "Explain relativity"}], model=gpt5_mini)
    assert payload_mini["reasoningEffort"] == "medium"
    assert guard_reasoning_mode(payload_mini) is True

    # Other reasoning models enforce low
    for model in [gpt5_luna, claude, gemma, gpt_oss]:
        payload = chat.build_payload(
            [{"role": "user", "content": "Explain relativity"}],
            model=model,
        )
        assert payload["reasoningEffort"] == "low", f"Model {model} must have reasoningEffort='low'"
        assert guard_reasoning_mode(payload) is True


def test_guard_reasoning_mode_rejects_non_reasoning():
    """Test guard raises ValueError if a reasoning model has reasoning disabled."""
    chat = DuckChat(warm_session=False)
    payload = chat.build_payload([{"role": "user", "content": "test"}], model=gpt5_luna)
    assert guard_reasoning_mode(payload) is True

    # Tamper with payload to disable reasoning mode
    payload["reasoningEffort"] = "none"
    with pytest.raises(ValueError, match="ReasoningModeGuardViolation"):
        guard_reasoning_mode(payload)

    payload["reasoningEffort"] = ""
    with pytest.raises(ValueError, match="ReasoningModeGuardViolation"):
        guard_reasoning_mode(payload)


def test_resolve_effort_always_in_reasoning_mode():
    """Verify resolve_effort always returns reasoning mode ('low') for reasoning-capable models."""
    assert resolve_effort(gpt5_luna, None) == "low"
    assert resolve_effort(gpt5_luna, "fast") == "low"
    assert resolve_effort(gpt5_luna, "none") == "low"
    assert resolve_effort(gpt5_luna, "reasoning") == "low"
    assert resolve_effort(gpt5_luna, "thinking") == "low"
    assert resolve_effort(gpt5_luna, "high") == "low"

    assert resolve_effort("mistral-small-2603", None) is None


def test_image_generation_bypasses_reasoning_mode():
    """Image generation payloads do not use reasoningEffort and pass guards."""
    chat = DuckChat(warm_session=False)
    payload = chat.build_payload(
        [{"role": "user", "content": "Cyberpunk duck"}],
        model=image_generation,
    )
    assert payload["reasoningEffort"] == "none"
    assert payload["metadata"]["toolChoice"]["GenerateImage"] is True
    assert guard_reasoning_mode(payload) is True
