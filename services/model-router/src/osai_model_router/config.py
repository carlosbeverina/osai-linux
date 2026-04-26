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

    # Local provider selection
    # OSAI_LOCAL_PROVIDER: "llamacpp" (default) or "vllm"
    osai_local_provider: str = os.getenv("OSAI_LOCAL_PROVIDER", "llamacpp")

    # OSAI_LOCAL_MOCK: true for mock mode, false for real provider calls
    osai_local_mock: bool = os.getenv("OSAI_LOCAL_MOCK", "true").lower() == "true"

    # Local llama.cpp Configuration
    # OSAI_LLAMACPP_BASE_URL: llama.cpp OpenAI-compatible base URL
    osai_llamacpp_base_url: str = os.getenv(
        "OSAI_LLAMACPP_BASE_URL", "http://127.0.0.1:8092/v1"
    )
    # OSAI_LLAMACPP_MODEL: default model served by llama.cpp
    osai_llamacpp_model: str = os.getenv("OSAI_LLAMACPP_MODEL", "gemma-local-gguf")
    # OSAI_LLAMACPP_API_KEY: API key for llama.cpp
    osai_llamacpp_api_key: str = os.getenv("OSAI_LLAMACPP_API_KEY", "osai-local-dev-token")

    # Local vLLM Configuration
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

    def validate_llamacpp_url(self) -> tuple[bool, str]:
        """Validate llama.cpp base URL is loopback and http.

        Returns:
            Tuple of (is_valid, error_message)
        """
        if not self.osai_llamacpp_base_url:
            return False, "llama.cpp URL cannot be empty"
        parsed = urlparse(self.osai_llamacpp_base_url)
        if parsed.scheme != "http":
            return False, f"llama.cpp URL must use http:// scheme, got {parsed.scheme}://"
        if parsed.hostname not in ("localhost", "127.0.0.1"):
            return False, f"llama.cpp URL must be loopback (localhost or 127.0.0.1), got {parsed.hostname}"
        return True, ""

    def get_local_provider_config(self) -> tuple[str, str, str]:
        """Get the active local provider configuration.

        Returns:
            Tuple of (provider_name, base_url, api_key)
            provider_name is "llamacpp" or "vllm"
        """
        if self.osai_local_provider == "vllm":
            return "vllm", self.osai_vllm_base_url, self.osai_vllm_api_key
        else:
            return "llamacpp", self.osai_llamacpp_base_url, self.osai_llamacpp_api_key

    def get_local_model(self) -> str:
        """Get the default local model based on selected provider.

        Returns:
            Model name for the selected local provider
        """
        if self.osai_local_provider == "vllm":
            return self.osai_vllm_model
        else:
            return self.osai_llamacpp_model


# Global config instance
config = Config()
