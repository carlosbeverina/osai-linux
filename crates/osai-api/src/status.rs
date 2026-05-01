//! Status endpoint - /v1/status

use serde::Serialize;

/// Status response.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub ok: bool,
    pub service: String,
    pub version: String,
    pub api: ApiInfo,
    pub model_router: ModelRouterInfo,
    pub capabilities: CapabilitiesInfo,
}

/// API server info.
#[derive(Debug, Serialize)]
pub struct ApiInfo {
    pub url: String,
    pub loopback_only: bool,
}

/// Model router info.
#[derive(Debug, Serialize)]
pub struct ModelRouterInfo {
    pub url: String,
    pub reachable: bool,
}

/// All capabilities.
#[derive(Debug, Serialize)]
pub struct CapabilitiesInfo {
    pub chat: bool,
    pub ask: bool,
    pub plan_validate: bool,
    pub plan_authorize: bool,
    pub apply: bool,
    pub plans: bool,
    pub receipts: bool,
    pub runtime_status: bool,
}

impl StatusResponse {
    /// Build a status response with model router reachability check.
    pub async fn new(model_router_url: &str) -> Self {
        let reachable = check_model_router_reachable(model_router_url).await;
        Self {
            ok: true,
            service: "osai-api".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            api: ApiInfo {
                url: "http://127.0.0.1:8090".to_string(),
                loopback_only: true,
            },
            model_router: ModelRouterInfo {
                url: model_router_url.to_string(),
                reachable,
            },
            capabilities: CapabilitiesInfo {
                chat: true,
                ask: true,
                plan_validate: true,
                plan_authorize: true,
                apply: true,
                plans: true,
                receipts: true,
                runtime_status: true,
            },
        }
    }
}

/// Check if the model router is reachable (with short timeout).
async fn check_model_router_reachable(url: &str) -> bool {
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(500))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client.get(&health_url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}
