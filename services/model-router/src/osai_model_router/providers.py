"""Model providers for OSAI Model Router."""

import re
import time
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any

import httpx

from .schemas import ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, Usage


@dataclass
class NormalizedOutput:
    """Result of output normalization."""
    content: str
    reasoning_stripped: bool
    was_empty: bool


def strip_thinking_blocks(content: str) -> NormalizedOutput:
    """Remove <think>...</think> blocks from content.

    Handles both complete and incomplete thinking blocks:
    - Complete blocks: <think> ...text...</think> -> removed
    - Incomplete blocks: <think> partial text (no closing) -> removed from <think> to line end

    Args:
        content: Raw model output

    Returns:
        NormalizedOutput with stripped content and metadata
    """
    reasoning_stripped = False
    original_content = content

    # First, strip complete <think>...</think> blocks
    content = re.sub(
        r'<think>\s*.*?\s*</think>',
        '',
        content,
        flags=re.DOTALL
    )
    if content != original_content:
        reasoning_stripped = True

    # Then strip any remaining incomplete opening block (from <think> to end of line)
    # This handles the case where there's text after the incomplete block
    if '<think>' in content:
        content = re.sub(r'<think>.*?(?:\n|$)', '', content, flags=re.DOTALL)
        reasoning_stripped = True

    # Trim whitespace
    content = content.strip()

    # Check if content is empty after stripping
    was_empty = len(content) == 0

    # If empty after stripping, use fallback
    if was_empty:
        content = "The model response contained only hidden reasoning and no visible answer."

    return NormalizedOutput(
        content=content,
        reasoning_stripped=reasoning_stripped or was_empty,
        was_empty=was_empty
    )


class BaseProvider(ABC):
    """Base class for model providers."""

    @abstractmethod
    def generate(self, request: ChatCompletionRequest, routed_model: str) -> tuple[ChatCompletionResponse, bool]:
        """Generate a chat completion response.

        Returns:
            Tuple of (response, reasoning_stripped)
        """
        ...


class LocalMockProvider(BaseProvider):
    """Mock provider for local models.

    Returns deterministic content without calling Ollama.
    """

    def generate(self, request: ChatCompletionRequest, routed_model: str) -> tuple[ChatCompletionResponse, bool]:
        """Generate a mock response for local models."""
        response = ChatCompletionResponse(
            id=f"osai-local-{int(time.time() * 1000)}",
            created=int(time.time()),
            model=routed_model,
            choices=[
                Choice(
                    index=0,
                    message=ChatMessage(
                        role="assistant",
                        content=f"OSAI local mock response (model: {routed_model})"
                    ),
                    finish_reason="stop"
                )
            ],
            usage=Usage(
                prompt_tokens=len(str(request.messages)),
                completion_tokens=10,
                total_tokens=len(str(request.messages)) + 10
            )
        )
        return response, False


class MiniMaxProvider(BaseProvider):
    """MiniMax cloud provider.

    Uses OpenAI-compatible API endpoint.
    Applies output normalization to strip thinking blocks.
    """

    # Default max_tokens when not specified by user
    DEFAULT_MAX_TOKENS = 1024

    def __init__(
        self,
        api_key: str,
        base_url: str = "https://api.minimax.io/v1",
        default_model: str = "MiniMax-M2.7",
        mock_mode: bool = True
    ):
        """Initialize MiniMax provider."""
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.default_model = default_model
        self.mock_mode = mock_mode

    def generate(self, request: ChatCompletionRequest, routed_model: str) -> tuple[ChatCompletionResponse, bool]:
        """Generate a response via MiniMax API.

        Returns:
            Tuple of (response, reasoning_stripped)
        """
        if self.mock_mode:
            return self._mock_response(routed_model, request)

        return self._real_request(request, routed_model)

    def _mock_response(self, model: str, request: ChatCompletionRequest) -> tuple[ChatCompletionResponse, bool]:
        """Return mock response for testing."""
        response = ChatCompletionResponse(
            id=f"osai-minimax-{int(time.time() * 1000)}",
            created=int(time.time()),
            model=model,
            choices=[
                Choice(
                    index=0,
                    message=ChatMessage(
                        role="assistant",
                        content=f"OSAI MiniMax mock response (model: {model})"
                    ),
                    finish_reason="stop"
                )
            ],
            usage=Usage(
                prompt_tokens=len(str(request.messages)),
                completion_tokens=12,
                total_tokens=len(str(request.messages)) + 12
            )
        )
        return response, False

    def _real_request(self, request: ChatCompletionRequest, routed_model: str) -> tuple[ChatCompletionResponse, bool]:
        """Make real request to MiniMax API (not called in tests)."""
        if not self.api_key:
            raise ValueError("MINIMAX_API_KEY is not configured")

        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }

        payload = {
            "model": routed_model,
            "messages": [{"role": m.role, "content": m.content} for m in request.messages],
        }

        if request.temperature is not None:
            payload["temperature"] = request.temperature

        # Apply default max_tokens if not specified
        max_tokens = request.max_tokens if request.max_tokens is not None else self.DEFAULT_MAX_TOKENS
        payload["max_tokens"] = max_tokens

        with httpx.Client(timeout=60.0) as client:
            response = client.post(
                f"{self.base_url}/chat/completions",
                headers=headers,
                json=payload
            )
            response.raise_for_status()
            data = response.json()

        # Normalize the output to strip thinking blocks
        normalized = strip_thinking_blocks(data["choices"][0]["message"]["content"])
        finish_reason = data["choices"][0]["finish_reason"]

        return ChatCompletionResponse(
            id=data["id"],
            created=data["created"],
            model=data["model"],
            choices=[
                Choice(
                    index=0,
                    message=ChatMessage(
                        role="assistant",
                        content=normalized.content
                    ),
                    finish_reason=finish_reason
                )
            ],
            usage=Usage(
                prompt_tokens=data["usage"]["prompt_tokens"],
                completion_tokens=data["usage"]["completion_tokens"],
                total_tokens=data["usage"]["total_tokens"]
            )
        ), normalized.reasoning_stripped
