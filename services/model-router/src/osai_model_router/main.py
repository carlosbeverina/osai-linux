"""FastAPI main application for OSAI Model Router."""

import time

from fastapi import FastAPI, HTTPException

from .config import config
from .receipts import ReceiptWriter
from .router import ModelRouter
from .schemas import (
    ChatCompletionRequest,
    ChatCompletionResponse,
    HealthResponse,
    ModelInfo,
    ModelsResponse,
)

app = FastAPI(title="OSAI Model Router")
router_instance = ModelRouter()
receipt_writer = ReceiptWriter()


@app.get("/health", response_model=HealthResponse)
async def health() -> HealthResponse:
    """Health check endpoint."""
    return HealthResponse(status="ok", service="osai-model-router")


@app.get("/v1/models", response_model=ModelsResponse)
async def list_models() -> ModelsResponse:
    """List available models."""
    models = [
        ModelInfo(id=model_id)
        for model_id in router_instance.get_available_models()
    ]
    return ModelsResponse(data=models)


@app.post("/v1/chat/completions", response_model=ChatCompletionResponse)
async def chat_completions(request: ChatCompletionRequest) -> ChatCompletionResponse:
    """Handle chat completion request.

    Routes to appropriate provider based on model alias.
    """
    # Check for streaming (not implemented)
    if request.stream:
        raise HTTPException(
            status_code=400,
            detail="streaming is not implemented in Model Router MVP"
        )

    try:
        # Route the request
        response, provider_name, routed_model = router_instance.route(request)

        # Write receipt
        request_id = f"osai-{int(time.time() * 1000)}"
        receipt_writer.write_receipt(
            request_id=request_id,
            selected_provider=provider_name,
            requested_model=request.model,
            routed_model=routed_model,
            messages=[{"role": m.role, "content": ""} for m in request.messages],
            status="executed",
            metadata=request.metadata
        )

        return response

    except ValueError as e:
        # Write failed receipt
        request_id = f"osai-{int(time.time() * 1000)}"
        receipt_writer.write_receipt(
            request_id=request_id,
            selected_provider="unknown",
            requested_model=request.model,
            routed_model="unknown",
            messages=[{"role": m.role, "content": ""} for m in request.messages],
            status="failed",
            error=str(e),
            metadata=request.metadata
        )
        raise HTTPException(status_code=400, detail=str(e))

    except Exception as e:
        # Write failed receipt
        request_id = f"osai-{int(time.time() * 1000)}"
        receipt_writer.write_receipt(
            request_id=request_id,
            selected_provider="error",
            requested_model=request.model,
            routed_model="unknown",
            messages=[{"role": m.role, "content": ""} for m in request.messages],
            status="failed",
            error=str(e),
            metadata=request.metadata
        )
        raise HTTPException(status_code=500, detail=str(e))


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=8088)
