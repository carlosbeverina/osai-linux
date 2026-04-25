"""Pydantic schemas for OSAI Model Router."""

from typing import Any, Literal, Optional

from pydantic import BaseModel, Field


class Message(BaseModel):
    """Chat message schema."""

    role: str
    content: str


class ChatCompletionRequest(BaseModel):
    """OpenAI-compatible chat completion request."""

    model: str
    messages: list[Message]
    temperature: Optional[float] = None
    max_tokens: Optional[int] = None
    stream: Optional[bool] = Field(default=False)
    metadata: Optional[dict[str, Any]] = None


class Usage(BaseModel):
    """Token usage statistics."""

    prompt_tokens: int
    completion_tokens: int
    total_tokens: int


class ChatMessage(BaseModel):
    """Chat message in response."""

    role: str
    content: str


class Choice(BaseModel):
    """Chat completion choice."""

    index: int
    message: ChatMessage
    finish_reason: str


class ChatCompletionResponse(BaseModel):
    """OpenAI-compatible chat completion response."""

    id: str
    object: str = "chat.completion"
    created: int
    model: str
    choices: list[Choice]
    usage: Usage


class ModelInfo(BaseModel):
    """Model information."""

    id: str
    object: str = "model"
    created: int = 0
    owned_by: str = "osai"


class ModelsResponse(BaseModel):
    """List of available models."""

    object: str = "list"
    data: list[ModelInfo]


class HealthResponse(BaseModel):
    """Health check response."""

    status: str
    service: str


class ErrorResponse(BaseModel):
    """Error response."""

    error: str
