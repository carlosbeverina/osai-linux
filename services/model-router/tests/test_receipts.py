"""Tests for receipts functionality."""

import json
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from osai_model_router.config import config
from osai_model_router.main import app


@pytest.fixture
def client_with_receipts(tmp_path, monkeypatch):
    """Create test client with temporary receipts directory."""
    receipts_dir = tmp_path / "receipts"
    receipts_dir.mkdir()
    # Patch config to use temp directory
    monkeypatch.setattr(config, "receipts_dir", receipts_dir)
    config.ensure_receipts_dir()
    # Import router and main to ensure modules are loaded with patched config
    from osai_model_router.router import ModelRouter
    from osai_model_router.main import receipt_writer

    # Recreate receipt_writer with patched config
    from osai_model_router.receipts import ReceiptWriter
    receipt_writer.receipts_dir = receipts_dir
    return TestClient(app), receipts_dir


def test_receipts_are_written(client_with_receipts):
    """Test that receipts are written for chat completions."""
    client, receipts_dir = client_with_receipts

    response = client.post("/v1/chat/completions", json={
        "model": "osai-local",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200

    # Check receipts were written
    receipt_files = list(receipts_dir.glob("*.json"))
    assert len(receipt_files) == 1

    with open(receipt_files[0]) as f:
        receipt = json.load(f)

    assert receipt["service"] == "model-router"
    assert receipt["requested_model"] == "osai-local"
    assert receipt["status"] == "executed"


def test_receipts_do_not_contain_full_prompt_content(client_with_receipts):
    """Test that receipts do not log full message content."""
    client, receipts_dir = client_with_receipts

    response = client.post("/v1/chat/completions", json={
        "model": "osai-local",
        "messages": [
            {"role": "user", "content": "This is a secret message"},
            {"role": "assistant", "content": "I should not log this either"}
        ]
    })
    assert response.status_code == 200

    receipt_files = list(receipts_dir.glob("*.json"))
    with open(receipt_files[0]) as f:
        receipt = json.load(f)

    # Check message content is not in receipt
    receipt_str = json.dumps(receipt)
    assert "secret message" not in receipt_str
    assert "I should not log" not in receipt_str

    # Check input summary is present
    assert "input_summary" in receipt
    assert receipt["input_summary"]["message_count"] == 2
    assert "roles" in receipt["input_summary"]
    assert "user" in receipt["input_summary"]["roles"]
    assert "assistant" in receipt["input_summary"]["roles"]


def test_receipts_contain_routing_metadata(client_with_receipts):
    """Test that receipts contain routing metadata."""
    client, receipts_dir = client_with_receipts

    response = client.post("/v1/chat/completions", json={
        "model": "osai-auto",
        "messages": [{"role": "user", "content": "Hello"}],
        "metadata": {"privacy": "local_only", "complexity": "low"}
    })
    assert response.status_code == 200

    receipt_files = list(receipts_dir.glob("*.json"))
    with open(receipt_files[0]) as f:
        receipt = json.load(f)

    assert receipt["privacy"] == "local_only"
    assert receipt["complexity"] == "low"
    assert receipt["requested_model"] == "osai-auto"
    assert "routed_model" in receipt
    assert "selected_provider" in receipt


def test_receipts_include_local_provider(client_with_receipts):
    """Test that receipts include local_provider and local_mock."""
    client, receipts_dir = client_with_receipts

    response = client.post("/v1/chat/completions", json={
        "model": "osai-local",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200

    receipt_files = list(receipts_dir.glob("*.json"))
    with open(receipt_files[0]) as f:
        receipt = json.load(f)

    # Check local provider info
    assert "local_provider" in receipt
    assert "local_mock" in receipt
    assert "local_base_url_host" in receipt
    assert receipt["local_provider"] == "vllm"
    assert receipt["local_mock"] is True  # Default is mock mode
    assert receipt["local_base_url_host"] == "127.0.0.1"


def test_failed_request_writes_receipt(client_with_receipts):
    """Test that failed requests write receipts with error."""
    client, receipts_dir = client_with_receipts

    response = client.post("/v1/chat/completions", json={
        "model": "unknown-model",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 400

    receipt_files = list(receipts_dir.glob("*.json"))
    assert len(receipt_files) == 1

    with open(receipt_files[0]) as f:
        receipt = json.load(f)

    assert receipt["status"] == "failed"
    assert "error" in receipt
    assert "Unknown model" in receipt["error"]


def test_multiple_requests_write_multiple_receipts(client_with_receipts):
    """Test that multiple requests write multiple receipts."""
    client, receipts_dir = client_with_receipts

    for i in range(3):
        response = client.post("/v1/chat/completions", json={
            "model": "osai-local",
            "messages": [{"role": "user", "content": f"Hello {i}"}]
        })
        assert response.status_code == 200

    receipt_files = list(receipts_dir.glob("*.json"))
    assert len(receipt_files) == 3


def test_minimax_receipt_includes_provider_info(client_with_receipts):
    """Test that minimax receipts also include local_provider info."""
    client, receipts_dir = client_with_receipts

    response = client.post("/v1/chat/completions", json={
        "model": "MiniMax-M2.7",
        "messages": [{"role": "user", "content": "Hello"}]
    })
    assert response.status_code == 200

    receipt_files = list(receipts_dir.glob("*.json"))
    with open(receipt_files[0]) as f:
        receipt = json.load(f)

    # Should have selected_provider as MiniMaxProvider
    assert receipt["selected_provider"] == "MiniMaxProvider"
    # But local_provider info is still present
    assert "local_provider" in receipt
    assert "local_mock" in receipt
    assert "local_base_url_host" in receipt
