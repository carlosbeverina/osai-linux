"""Tests for routing logic."""

import pytest
from fastapi.testclient import TestClient

from osai_model_router.main import app


@pytest.fixture
def client():
    """Create test client."""
    return TestClient(app)


def test_osai_auto_local_only(client):
    """Test osai-auto with local_only privacy routes to local provider."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-auto",
        "messages": [{"role": "user", "content": "Hello"}],
        "metadata": {"privacy": "local_only"}
    })
    assert response.status_code == 200
    data = response.json()
    # Should route to local mock (llamacpp by default)
    assert "llama.cpp" in data["choices"][0]["message"]["content"] or "gemma-local-gguf" in data["choices"][0]["message"]["content"]


def test_osai_auto_complexity_high(client):
    """Test osai-auto with high complexity routes to cloud."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-auto",
        "messages": [{"role": "user", "content": "Hello"}],
        "metadata": {"complexity": "high"}
    })
    assert response.status_code == 200
    data = response.json()
    # Should route to MiniMax mock
    assert "MiniMax" in data["choices"][0]["message"]["content"]


def test_osai_auto_speed_fast(client):
    """Test osai-auto with fast speed routes to fast model."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-auto",
        "messages": [{"role": "user", "content": "Hello"}],
        "metadata": {"speed": "fast"}
    })
    assert response.status_code == 200
    data = response.json()
    # Should route to MiniMax-M2.7-highspeed mock
    assert "MiniMax" in data["choices"][0]["message"]["content"]


def test_osai_auto_no_metadata_routes_local(client):
    """Test osai-auto with no metadata routes to local provider by default."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-auto",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    # Should route to local mock (llamacpp by default)
    assert "llama.cpp" in data["choices"][0]["message"]["content"] or "gemma-local-gguf" in data["choices"][0]["message"]["content"]


def test_osai_local_routes_llamacpp(client):
    """Test osai-local routes to llama.cpp local provider by default."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-local",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    # Should route to llama.cpp mock
    assert "llama.cpp" in data["choices"][0]["message"]["content"]


def test_gemma4_e2b_routes_local(client):
    """Test gemma4:e2b routes to local provider."""
    response = client.post("/v1/chat/completions", json={
        "model": "gemma4:e2b",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert "gemma4:e2b" in data["choices"][0]["message"]["content"]


def test_gemma4_e4b_routes_local(client):
    """Test gemma4:e4b routes to local provider."""
    response = client.post("/v1/chat/completions", json={
        "model": "gemma4:e4b",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert "gemma4:e4b" in data["choices"][0]["message"]["content"]


def test_gemma4_26b_routes_local(client):
    """Test gemma4:26b routes to local provider."""
    response = client.post("/v1/chat/completions", json={
        "model": "gemma4:26b",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert "gemma4:26b" in data["choices"][0]["message"]["content"]


def test_osai_cloud_routes_cloud(client):
    """Test osai-cloud routes to MiniMax."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-cloud",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert "MiniMax" in data["choices"][0]["message"]["content"]


def test_minimax_model_routes_cloud(client):
    """Test MiniMax-M2.7 routes to MiniMax."""
    response = client.post("/v1/chat/completions", json={
        "model": "MiniMax-M2.7",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert "MiniMax" in data["choices"][0]["message"]["content"]


def test_minimax_highspeed_routes_cloud(client):
    """Test MiniMax-M2.7-highspeed routes to MiniMax."""
    response = client.post("/v1/chat/completions", json={
        "model": "MiniMax-M2.7-highspeed",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    assert "MiniMax" in data["choices"][0]["message"]["content"]


def test_llamacpp_local_mock_response(client):
    """Test that default local route returns llama.cpp local mock response."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-local",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    # Response should indicate llama.cpp mock
    assert "llama.cpp" in data["choices"][0]["message"]["content"]


def test_osai_auto_local_only_routes_llamacpp(client):
    """Test osai-auto with local_only routes to llama.cpp local provider."""
    response = client.post("/v1/chat/completions", json={
        "model": "osai-auto",
        "messages": [{"role": "user", "content": "Hello"}],
        "metadata": {"privacy": "local_only"}
    })
    assert response.status_code == 200
    data = response.json()
    # Should be llama.cpp mock response
    assert "llama.cpp" in data["choices"][0]["message"]["content"]


def test_explicit_gemma4_e4b_routes_local(client):
    """Test explicit gemma4:e4b routes to local provider."""
    response = client.post("/v1/chat/completions", json={
        "model": "gemma4:e4b",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200
    data = response.json()
    # gemma4:e4b is passed through to local provider
    assert "gemma4:e4b" in data["choices"][0]["message"]["content"]
