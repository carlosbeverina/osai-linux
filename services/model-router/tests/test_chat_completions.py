"""Tests for chat completions endpoint."""

import pytest
from fastapi.testclient import TestClient

from osai_model_router.main import app


@pytest.fixture
def client():
    """Create test client."""
    return TestClient(app)


def test_chat_completions_local_model(client):
    """Test chat completion with local model returns mock response."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-local",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    # osai-local is resolved to the configured local model (llamacpp default)
    assert data["model"] == "gemma-local-gguf"
    assert "choices" in data
    assert len(data["choices"]) == 1
    assert "OSAI llama.cpp local mock response" in data["choices"][0]["message"]["content"]


def test_chat_completions_gemma4_e2b(client):
    """Test chat completion with gemma4:e2b routes to local."""
    response = client.post("/v1/chat/completions", json={
        "model": "gemma4:e2b",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert data["model"] == "gemma4:e2b"
    assert "OSAI llama.cpp local mock response" in data["choices"][0]["message"]["content"]


def test_chat_completions_cloud_model_mock(client):
    """Test chat completion with cloud model returns mock response when mock mode is true."""
    response = client.post("/v1/chat/completions", json={
        "model": "MiniMax-M2.7",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert "MiniMax" in data["choices"][0]["message"]["content"]


def test_chat_completions_stream_returns_error(client):
    """Test that streaming returns 400 error."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-local",
        "messages": [{"role": "user", "content": "Hello"}],
        "stream": True
    })
    assert response.status_code == 400
    assert "not implemented" in response.json()["detail"]


def test_chat_completions_unknown_model(client):
    """Test that unknown model returns 400 error."""
    response = client.post("/v1/chat/completions", json={
        "model": "unknown-model",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 400
    assert "Unknown model" in response.json()["detail"]
