"""Tests for health endpoint."""

import pytest
from fastapi.testclient import TestClient

from osai_model_router.main import app


@pytest.fixture
def client():
    """Create test client."""
    return TestClient(app)


def test_health_returns_ok(client):
    """Test health endpoint returns ok status."""
    response = client.get("/health")
    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"
    assert data["service"] == "osai-model-router"
