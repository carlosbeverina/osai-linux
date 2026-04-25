"""Router for OSAI Model Router.

Routes model requests to appropriate providers based on model alias.
"""

from .config import config
from .providers import BaseProvider, LocalMockProvider, MiniMaxProvider
from .schemas import ChatCompletionRequest, ChatCompletionResponse


# Local model aliases
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
        self.local_provider = LocalMockProvider()
        self.cloud_provider = MiniMaxProvider(
            api_key=config.minimax_api_key,
            base_url=config.minimax_openai_base_url,
            default_model=config.minimax_model,
            mock_mode=config.osai_mock_cloud
        )

    def route(
        self,
        request: ChatCompletionRequest
    ) -> tuple[ChatCompletionResponse, str, str]:
        """Route a chat completion request to the appropriate provider.

        Args:
            request: Chat completion request

        Returns:
            Tuple of (response, provider_name, routed_model)

        Raises:
            ValueError: If model is unknown or API key is missing
        """
        model = request.model
        metadata = request.metadata or {}

        # Determine the target model
        target_model, provider = self._get_provider_for_model(model, metadata)

        # Generate response
        response = provider.generate(request, target_model)

        return response, provider.__class__.__name__, target_model

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
        # Local models
        if model in LOCAL_MODELS:
            return model, self.local_provider

        # Explicit cloud models
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

    def _route_auto(self, metadata: dict) -> tuple[str, BaseProvider]:
        """Route based on metadata hints.

        Args:
            metadata: Request metadata with routing hints

        Returns:
            Tuple of (routed_model, provider)
        """
        # If local_only is specified, route to local
        if metadata.get("privacy") == "local_only":
            return "gemma4:e4b", self.local_provider

        # If complexity is high, route to cloud
        if metadata.get("complexity") == "high":
            return config.minimax_model, self.cloud_provider

        # If speed is fast, route to fast cloud model
        if metadata.get("speed") == "fast":
            return config.minimax_fast_model, self.cloud_provider

        # Default to local
        return "gemma4:e4b", self.local_provider

    def get_available_models(self) -> list[str]:
        """Get list of available model aliases.

        Returns:
            List of model aliases
        """
        return sorted(AVAILABLE_MODELS)
