"""Router for OSAI Model Router.

Routes model requests to appropriate providers based on model alias.
"""

from .config import config
from .providers import BaseProvider, MiniMaxProvider, VllmProvider
from .schemas import ChatCompletionRequest, ChatCompletionResponse


# Local model aliases (resolved to vLLM models)
LOCAL_MODELS = {
    "osai-local",
    "gemma4:e2b",
    "gemma4:e4b",
    "gemma4:26b",
}

# Cloud model aliases
CLOUD_MODELS = {
    "osai-cloud",
    "MiniMax-M2.7",
    "MiniMax-M2.7-highspeed",
}

# All available models
AVAILABLE_MODELS = LOCAL_MODELS | CLOUD_MODELS | {"osai-auto"}


class ModelRouter:
    """Routes chat completion requests to appropriate providers."""

    def __init__(self):
        """Initialize router with providers."""
        # Validate vLLM URL at startup
        is_valid, error = config.validate_vllm_url()
        if not is_valid:
            raise ValueError(f"Invalid vLLM configuration: {error}")

        self.local_provider = VllmProvider(
            base_url=config.osai_vllm_base_url,
            api_key=config.osai_vllm_api_key,
            default_model=config.osai_vllm_model,
            mock_mode=config.osai_local_mock
        )
        self.cloud_provider = MiniMaxProvider(
            api_key=config.minimax_api_key,
            base_url=config.minimax_openai_base_url,
            default_model=config.minimax_model,
            mock_mode=config.osai_mock_cloud
        )

    def route(
        self,
        request: ChatCompletionRequest
    ) -> tuple[ChatCompletionResponse, str, str, bool]:
        """Route a chat completion request to the appropriate provider.

        Args:
            request: Chat completion request

        Returns:
            Tuple of (response, provider_name, routed_model, reasoning_stripped)

        Raises:
            ValueError: If model is unknown or API key is missing
        """
        model = request.model
        metadata = request.metadata or {}

        # Determine the target model
        target_model, provider = self._get_provider_for_model(model, metadata)

        # Generate response
        response, reasoning_stripped = provider.generate(request, target_model)

        # Provider name is 'VllmProvider' or 'MiniMaxProvider'
        provider_name = provider.__class__.__name__

        return response, provider_name, target_model, reasoning_stripped

    def _get_provider_for_model(
        self,
        model: str,
        metadata: dict
    ) -> tuple[str, BaseProvider]:
        """Get the provider and model for a given model alias.

        Args:
            model: Model alias
            metadata: Request metadata

        Returns:
            Tuple of (routed_model, provider)
        """
        # Local models route to vLLM provider
        if model in LOCAL_MODELS:
            resolved_model = self._resolve_local_model(model)
            return resolved_model, self.local_provider

        # Explicit cloud models route to MiniMax provider
        if model == "osai-cloud":
            return config.minimax_model, self.cloud_provider

        if model == "MiniMax-M2.7":
            return config.minimax_model, self.cloud_provider

        if model == "MiniMax-M2.7-highspeed":
            return config.minimax_fast_model, self.cloud_provider

        # Auto-routing based on metadata
        if model == "osai-auto":
            return self._route_auto(metadata)

        raise ValueError(f"Unknown model: {model}")

    def _resolve_local_model(self, model: str) -> str:
        """Resolve local model alias to actual vLLM model.

        Args:
            model: Local model alias

        Returns:
            Resolved model name for vLLM
        """
        # osai-local resolves to configured vLLM model
        if model == "osai-local":
            return config.osai_vllm_model

        # gemma4:* models pass through as-is
        # (vLLM serves these with the names gemma4:e2b, gemma4:e4b, gemma4:26b)
        return model

    def _route_auto(self, metadata: dict) -> tuple[str, BaseProvider]:
        """Route based on metadata hints.

        Args:
            metadata: Request metadata with routing hints

        Returns:
            Tuple of (routed_model, provider)
        """
        # If local_only is specified, route to local vLLM
        if metadata.get("privacy") == "local_only":
            resolved = self._resolve_local_model("osai-local")
            return resolved, self.local_provider

        # If complexity is high, route to cloud
        if metadata.get("complexity") == "high":
            return config.minimax_model, self.cloud_provider

        # If speed is fast, route to fast cloud model
        if metadata.get("speed") == "fast":
            return config.minimax_fast_model, self.cloud_provider

        # Default to local vLLM
        resolved = self._resolve_local_model("osai-local")
        return resolved, self.local_provider

    def get_available_models(self) -> list[str]:
        """Get list of available model aliases.

        Returns:
            List of model aliases
        """
        return sorted(AVAILABLE_MODELS)
