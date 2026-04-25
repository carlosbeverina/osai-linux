"""Configuration for OSAI Model Router.

Loads configuration from environment variables.
Supports optional .env file via python-dotenv.
"""

import os
from pathlib import Path

# Try to load .env file if python-dotenv is available
try:
    from dotenv import load_dotenv
    load_dotenv()
except ImportError:
    pass  # python-dotenv not installed, use env vars directly


class Config:
    """Model Router configuration."""

    # MiniMax API Configuration
    minimax_api_key: str = os.getenv("MINIMAX_API_KEY", "")
    minimax_openai_base_url: str = os.getenv(
        "MINIMAX_OPENAI_BASE_URL", "https://api.minimax.io/v1"
    )
    minimax_model: str = os.getenv("MINIMAX_MODEL", "MiniMax-M2.7")
    minimax_fast_model: str = os.getenv("MINIMAX_FAST_MODEL", "MiniMax-M2.7-highspeed")

    # Mock mode for cloud provider
    osai_mock_cloud: bool = os.getenv("OSAI_MODEL_ROUTER_MOCK_CLOUD", "true").lower() == "true"

    # Receipts directory
    receipts_dir: Path = Path(
        os.getenv("OSAI_RECEIPTS_DIR", "")
    ) or Path.home() / ".local" / "share" / "osai" / "receipts" / "model-router"

    @property
    def has_minimax_api_key(self) -> bool:
        """Check if MiniMax API key is configured."""
        return bool(self.minimax_api_key and self.minimax_api_key.strip())

    def ensure_receipts_dir(self) -> None:
        """Ensure receipts directory exists."""
        self.receipts_dir.mkdir(parents=True, exist_ok=True)


# Global config instance
config = Config()
