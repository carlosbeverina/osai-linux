"""Tests for mock mode isolation.

Verifies that mock mode works correctly regardless of API key configuration.
"""

import pytest
from fastapi.testclient import TestClient

from osai_model_router import config
from osai_model_router.main import app


class TestMockIsolation:
    """Test mock mode isolation."""

    def test_cloud_mock_returns_minimax_response(self, client):
        """Cloud mock returns MiniMax mock response."""
        response = client.post("/v1/chat/completions", json={
            "model": "osai-cloud",
            "messages": [{"role": "user", "content": "Hello"}]
        })
        assert response.status_code == 200
        data = response.json()
        assert "MiniMax" in data["choices"][0]["message"]["content"]

    def test_cloud_mock_works_without_real_api_key(self, client, monkeypatch):
        """Cloud mock returns mock response even with empty API key.

        This test verifies that when MINIMAX_API_KEY is empty, the cloud
        mock still works correctly because mock_mode=True.
        """
        # Set empty API key - has_minimax_api_key property will return False automatically
        monkeypatch.setattr(config.config, "minimax_api_key", "")

        response = client.post("/v1/chat/completions", json={
            "model": "osai-cloud",
            "messages": [{"role": "user", "content": "Hello"}]
        })
        assert response.status_code == 200
        data = response.json()
        # Should return mock response, not error
        assert "MiniMax" in data["choices"][0]["message"]["content"]

    def test_minimax_highspeed_mock_returns_200(self, client):
        """MiniMax-M2.7-highspeed mock returns 200."""
        response = client.post("/v1/chat/completions", json={
            "model": "MiniMax-M2.7-highspeed",
            "messages": [{"role": "user", "content": "Hello"}]
        })
        assert response.status_code == 200
        data = response.json()
        assert "MiniMax" in data["choices"][0]["message"]["content"]

    def test_osai_auto_speed_fast_returns_minimax_mock(self, client):
        """osai-auto with speed=fast returns MiniMax highspeed mock."""
        response = client.post("/v1/chat/completions", json={
            "model": "osai-auto",
            "messages": [{"role": "user", "content": "Hello"}],
            "metadata": {"speed": "fast"}
        })
        assert response.status_code == 200
        data = response.json()
        assert "MiniMax" in data["choices"][0]["message"]["content"]

    def test_osai_auto_complexity_high_returns_minimax_mock(self, client):
        """osai-auto with complexity=high returns MiniMax mock."""
        response = client.post("/v1/chat/completions", json={
            "model": "osai-auto",
            "messages": [{"role": "user", "content": "Hello"}],
            "metadata": {"complexity": "high"}
        })
        assert response.status_code == 200
        data = response.json()
        assert "MiniMax" in data["choices"][0]["message"]["content"]

    def test_local_mock_returns_llamacpp_response(self, client):
        """Local mock returns llama.cpp mock response."""
        response = client.post("/v1/chat/completions", json={
            "model": "osai-local",
            "messages": [{"role": "user", "content": "Hello"}]
        })
        assert response.status_code == 200
        data = response.json()
        assert "llama.cpp" in data["choices"][0]["message"]["content"]

    def test_osai_auto_no_metadata_routes_local(self, client):
        """osai-auto with no metadata routes to local mock."""
        response = client.post("/v1/chat/completions", json={
            "model": "osai-auto",
            "messages": [{"role": "user", "content": "Hello"}]
        })
        assert response.status_code == 200
        data = response.json()
        # Should route to local vLLM mock
        assert "vLLM" in data["choices"][0]["message"]["content"] or "gemma-local" in data["choices"][0]["message"]["content"]
