"""Receipt logging for OSAI Model Router.

Writes JSON receipts for every chat completion request.
Never logs full prompt content.
"""

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

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

    def write_receipt(
        self,
        request_id: str,
        selected_provider: str,
        requested_model: str,
        routed_model: str,
        messages: list[dict[str, str]],
        status: str,
        error: str | None = None,
        metadata: dict[str, Any] | None = None
    ) -> Path:
        """Write a receipt for a chat completion request.

        Args:
            request_id: Unique request identifier
            selected_provider: Provider that handled the request
            requested_model: Model requested by client
            routed_model: Model actually used for routing
            messages: List of message dicts (roles only, content summarized)
            status: "executed" or "failed"
            error: Error message if status is "failed"
            metadata: Request metadata including privacy, complexity, speed hints

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
            }
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
