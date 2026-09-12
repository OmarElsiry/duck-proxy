from __future__ import annotations

from typing import Optional


class DuckChatError(Exception):
    pass


class ChallengeError(DuckChatError):
    pass


class RateLimitError(DuckChatError):
    pass


class ConversationLimitError(DuckChatError):
    pass


class APIError(DuckChatError):
    def __init__(
        self,
        message: str,
        status_code: Optional[int] = None,
        body: Optional[str] = None,
    ):
        super().__init__(message)
        self.status_code = status_code
        self.body = body

    def __str__(self) -> str:
        base = super().__str__()
        if self.status_code is not None:
            return f"{base} (status={self.status_code})"
        return base
