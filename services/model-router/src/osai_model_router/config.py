"""Configuration for OSAI Model Router.

Loads configuration from environment variables.
Supports optional .env file via python-dotenv.
"""

import os
from pathlib import Path
from urllib.parse import urlparse

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

    # Local vLLM Configuration
    # OSAI_LOCAL_PROVIDER: vllm (only vllm for now)
    osai_local_provider: str = os.getenv("OSAI_LOCAL_PROVIDER", "vllm")
    # OSAI_LOCAL_MOCK: true for mock mode, false for real vLLM calls
    osai_local_mock: bool = os.getenv("OSAI_LOCAL_MOCK", "true").lower() == "true"
    # OSAI_VLLM_BASE_URL: vLLM OpenAI-compatible base URL
    osai_vllm_base_url: str = os.getenv(
        "OSAI_VLLM_BASE_URL", "http://127.0.0.1:8091/v1"
    )
    # OSAI_VLLM_MODEL: default model served by vLLM
    osai_vllm_model: str = os.getenv("OSAI_VLLM_MODEL", "gemma-local")
    # OSAI_VLLM_API_KEY: API key for vLLM
    osai_vllm_api_key: str = os.getenv("OSAI_VLLM_API_KEY", "osai-local-dev-token")

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

    def is_loopback_url(self, url: str) -> bool:
        """Check if URL is loopback-only (localhost or 127.0.0.1).

        Args:
            url: URL to validate

        Returns:
            True if URL is loopback-only
        """
        parsed = urlparse(url)
        host = parsed.hostname
        if host in ("localhost", "127.0.0.1"):
            return True
        return False

    def validate_vllm_url(self) -> tuple[bool, str]:
        """Validate vLLM base URL is loopback and http.

        Returns:
            Tuple of (is_valid, error_message)
        """
        parsed = urlparse(self.osai_vllm_base_url)
        if parsed.scheme != "http":
            return False, f"vLLM URL must use http:// scheme, got {parsed.scheme}://"
        if parsed.hostname not in ("localhost", "127.0.0.1"):
            return False, f"vLLM URL must be loopback (localhost or 127.0.0.1), got {parsed.hostname}"
        return True, ""


# Global config instance
config = Config()
