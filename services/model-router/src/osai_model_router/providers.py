"""Model providers for OSAI Model Router."""

from abc import ABC, abstractmethod
from typing import Any

import httpx

from .schemas import ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, Usage


class BaseProvider(ABC):
    """Base class for model providers."""

    @abstractmethod
    def generate(self, request: ChatCompletionRequest, routed_model: str) -> ChatCompletionResponse:
        """Generate a chat completion response."""
        ...


class LocalMockProvider(BaseProvider):
    """Mock provider for local models.

    Returns deterministic content without calling Ollama.
    """

    def generate(self, request: ChatCompletionRequest, routed_model: str) -> ChatCompletionResponse:
        """Generate a mock response for local models."""
        import time

        return ChatCompletionResponse(
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


class MiniMaxProvider(BaseProvider):
    """MiniMax cloud provider.

    Uses OpenAI-compatible API endpoint.
    """

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

    def generate(self, request: ChatCompletionRequest, routed_model: str) -> ChatCompletionResponse:
        """Generate a response via MiniMax API."""
        import time

        if self.mock_mode:
            return self._mock_response(routed_model, request)

        return self._real_request(request, routed_model)

    def _mock_response(self, model: str, request: ChatCompletionRequest) -> ChatCompletionResponse:
        """Return mock response for testing."""
        import time

        return ChatCompletionResponse(
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

    def _real_request(self, request: ChatCompletionRequest, routed_model: str) -> ChatCompletionResponse:
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
        if request.max_tokens is not None:
            payload["max_tokens"] = request.max_tokens

        with httpx.Client(timeout=60.0) as client:
            response = client.post(
                f"{self.base_url}/chat/completions",
                headers=headers,
                json=payload
            )
            response.raise_for_status()
            data = response.json()

            return ChatCompletionResponse(
                id=data["id"],
                created=data["created"],
                model=data["model"],
                choices=[
                    Choice(
                        index=c["index"],
                        message=ChatMessage(
                            role=c["message"]["role"],
                            content=c["message"]["content"]
                        ),
                        finish_reason=c["finish_reason"]
                    )
                    for c in data["choices"]
                ],
                usage=Usage(
                    prompt_tokens=data["usage"]["prompt_tokens"],
                    completion_tokens=data["usage"]["completion_tokens"],
                    total_tokens=data["usage"]["total_tokens"]
                )
            )
