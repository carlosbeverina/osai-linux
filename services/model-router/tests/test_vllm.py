"""Tests for vLLM local provider configuration."""

import pytest
from fastapi.testclient import TestClient

from osai_model_router.config import config


def test_loopback_localhost_is_valid():
    """Test that localhost URL is valid."""
    assert config.is_loopback_url("http://localhost:8080/v1") is True


def test_loopback_127001_is_valid():
    """Test that 127.0.0.1 URL is valid."""
    assert config.is_loopback_url("http://127.0.0.1:8080/v1") is True


def test_external_url_is_invalid():
    """Test that external URLs are invalid."""
    assert config.is_loopback_url("http://example.com:8080/v1") is False
    assert config.is_loopback_url("http://0.0.0.0:8080/v1") is False


def test_https_loopback_rejected_by_validate():
    """Test that https URLs are rejected by validate_vllm_url (not is_loopback_url)."""
    # Temporarily set an https URL
    original = config.osai_vllm_base_url
    config.osai_vllm_base_url = "https://127.0.0.1:8091/v1"
    is_valid, error = config.validate_vllm_url()
    config.osai_vllm_base_url = original
    assert is_valid is False
    assert "http://" in error


def test_external_url_rejected_by_validate():
    """Test that external URLs are rejected by validate_vllm_url."""
    # Temporarily set an external URL
    original = config.osai_vllm_base_url
    config.osai_vllm_base_url = "http://example.com:8091/v1"
    is_valid, error = config.validate_vllm_url()
    config.osai_vllm_base_url = original
    assert is_valid is False
    assert "loopback" in error.lower()


def test_vllm_url_validation_valid():
    """Test vLLM URL validation with valid loopback URL."""
    is_valid, error = config.validate_vllm_url()
    # Default URL is http://127.0.0.1:8091/v1 which is valid
    assert is_valid is True
    assert error == ""


def test_vllm_provider_mock_mode_by_default():
    """Test that OSAI_LOCAL_MOCK is True by default."""
    assert config.osai_local_mock is True


def test_vllm_provider_type_is_vllm():
    """Test that OSAI_LOCAL_PROVIDER is vllm by default."""
    assert config.osai_local_provider == "vllm"


def test_vllm_default_base_url():
    """Test that default vLLM base URL is loopback."""
    assert config.osai_vllm_base_url == "http://127.0.0.1:8091/v1"


def test_vllm_default_model():
    """Test that default vLLM model is set."""
    assert config.osai_vllm_model == "gemma-local"


def test_vllm_default_api_key():
    """Test that vLLM API key can be configured via environment."""
    import os
    # In test environment, API key is overridden by conftest
    # This test verifies the config reads from environment variable
    assert config.osai_vllm_api_key == os.getenv("OSAI_VLLM_API_KEY", "osai-local-dev-token")
