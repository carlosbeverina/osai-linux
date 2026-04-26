"""Pytest configuration and fixtures."""

import os

import pytest


def pytest_configure(config):
    """Set up test environment before any modules are imported.

    This runs BEFORE conftest.py fixtures and BEFORE test modules are imported.
    We set environment variables here to ensure they are in place when
    config.py is imported and the Config singleton is created.
    """
    # Force mock mode for cloud provider
    os.environ["OSAI_MODEL_ROUTER_MOCK_CLOUD"] = "true"
    # Force mock mode for local provider
    os.environ["OSAI_LOCAL_MOCK"] = "true"
    # Ensure no real API key is used - set dummy key
    os.environ["MINIMAX_API_KEY"] = "test-minimax-api-key-not-real"
    # Ensure vLLM mock mode
    os.environ["OSAI_VLLM_API_KEY"] = "test-vllm-api-key-not-real"


@pytest.fixture
def client():
    """Create test client with isolated config.

    Uses the environment set up by pytest_configure.
    """
    from fastapi.testclient import TestClient

    # Import app after env vars are set by pytest_configure
    from osai_model_router.main import app

    return TestClient(app)


@pytest.fixture
def isolated_receipts(tmp_path, monkeypatch):
    """Create isolated receipts directory for a test."""
    receipts_dir = tmp_path / "receipts"
    receipts_dir.mkdir()

    from osai_model_router import config as config_module
    monkeypatch.setattr(config_module.config, "receipts_dir", receipts_dir)
    config_module.config.ensure_receipts_dir()

    return receipts_dir
