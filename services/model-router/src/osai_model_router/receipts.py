"""Receipt logging for OSAI Model Router.

Writes JSON receipts for every chat completion request.
Never logs full prompt content.
"""

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from .config import config


class ReceiptWriter:
    """Writes audit receipts for model router requests."""

    def __init__(self, receipts_dir: Path | None = None):
        """Initialize receipt writer."""
        # Store override path if provided, otherwise use None to indicate "use config"
        self._override_dir = receipts_dir

    @property
    def receipts_dir(self) -> Path:
        """Get receipts directory, using config as default."""
        if self._override_dir is not None:
            return self._override_dir
        return config.receipts_dir

    @receipts_dir.setter
    def receipts_dir(self, value: Path) -> None:
        """Set override receipts directory."""
        self._override_dir = value

    def _get_local_provider_info(self) -> dict[str, Any]:
        """Get local provider information for receipt.

        Returns:
            Dict with local_provider, local_mock, and local_base_url_host
        """
        # Get the base URL based on selected local provider
        provider_name, base_url, _ = config.get_local_provider_config()

        # Extract just the host from the base URL
        parsed = urlparse(base_url)
        host_only = parsed.hostname or "unknown"

        return {
            "local_provider": provider_name,
            "local_mock": config.osai_local_mock,
            "local_base_url_host": host_only
        }

    def write_receipt(
        self,
        request_id: str,
        selected_provider: str,
        requested_model: str,
        routed_model: str,
        messages: list[dict[str, str]],
        status: str,
        error: str | None = None,
        metadata: dict[str, Any] | None = None,
        reasoning_stripped: bool = False,
        truncated: bool = False,
        was_empty_after_normalization: bool = False
    ) -> Path:
        """Write a receipt for a chat completion request.

        Args:
            request_id: Unique request identifier
            selected_provider: Provider that handled the request (LlamaCppProvider, VllmProvider, or MiniMaxProvider)
            requested_model: Model requested by client
            routed_model: Model actually used for routing
            messages: List of message dicts (roles only, content summarized)
            status: "executed" or "failed"
            error: Error message if status is "failed"
            metadata: Request metadata including privacy, complexity, speed hints
            reasoning_stripped: Whether thinking blocks were stripped from response
            truncated: Whether response was truncated due to max_tokens
            was_empty_after_normalization: Whether content was empty after normalization (used fallback)

        Returns:
            Path to the written receipt file
        """
        # Extract privacy, complexity, speed from metadata
        privacy = None
        complexity = None
        speed = None
        if metadata:
            privacy = metadata.get("privacy")
            complexity = metadata.get("complexity")
            speed = metadata.get("speed")

        # Get local provider info
        local_info = self._get_local_provider_info()

        receipt = {
            "id": request_id,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "service": "model-router",
            "selected_provider": selected_provider,
            "requested_model": requested_model,
            "routed_model": routed_model,
            "privacy": privacy,
            "complexity": complexity,
            "speed": speed,
            "status": status,
            "prompt_logged": False,
            "input_summary": {
                "message_count": len(messages),
                "roles": list(set(m.get("role", "") for m in messages))
            },
            "reasoning_stripped": reasoning_stripped,
            "truncated": truncated,
            "was_empty_after_normalization": was_empty_after_normalization,
            # Local provider info
            "local_provider": local_info["local_provider"],
            "local_mock": local_info["local_mock"],
            "local_base_url_host": local_info["local_base_url_host"]
        }

        if error:
            receipt["error"] = error

        filename = f"{request_id}.json"
        receipt_path = self.receipts_dir / filename

        # Ensure directory exists
        self.receipts_dir.mkdir(parents=True, exist_ok=True)

        with open(receipt_path, "w") as f:
            json.dump(receipt, f, indent=2)

        return receipt_path
