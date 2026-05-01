//! OSAI Runtime Status - Unified observable runtime layer.
//!
//! Used by osai-api and osai-agent-cli to report on the full local runtime stack.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Runtime Status - Top-level response
// ============================================================================

/// Top-level runtime status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub ok: bool,
    pub runtime_mode: RuntimeMode,
    pub overall: OverallHealth,
    pub components: ComponentStatusMap,
    pub systemd: SystemdStatus,
    pub paths: RuntimePaths,
    pub hints: Vec<String>,
}

/// Overall runtime health.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OverallHealth {
    Healthy,
    Degraded,
    Stopped,
    Unknown,
}

/// Runtime mode inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeMode {
    Systemd,
    Manual,
    Partial,
    Stopped,
    Unknown,
}

/// Map of component name -> component status.
pub type ComponentStatusMap = HashMap<String, ComponentStatus>;

// ============================================================================
// Component Status
// ============================================================================

/// Individual component (llama.cpp, model-router, osai-api) status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub url: String,
    pub reachable: bool,
    pub healthy: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Systemd Status
// ============================================================================

/// Systemd user services status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub services: Option<HashMap<String, SystemdServiceStatus>>,
}

/// Individual systemd service status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdServiceStatus {
    pub installed: bool,
    pub enabled: bool,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============================================================================
// Runtime Paths
// ============================================================================

/// Known runtime paths (no secrets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePaths {
    pub repo_root: PathBuf,
    pub plans_dir: PathBuf,
    pub chat_receipts_dir: PathBuf,
    pub ask_receipts_dir: PathBuf,
    pub apply_receipts_dir: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_router_receipts_dir: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_model_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llamacpp_binary_path: Option<PathBuf>,
}

// ============================================================================
// Builder
// ============================================================================

/// Builder for RuntimeStatus.
pub struct RuntimeStatusBuilder {
    components: ComponentStatusMap,
    systemd: SystemdStatus,
    hints: Vec<String>,
}

impl RuntimeStatusBuilder {
    /// Create a new builder with default empty state.
    pub fn new() -> Self {
        Self {
            components: ComponentStatusMap::new(),
            systemd: SystemdStatus {
                available: false,
                services: None,
            },
            hints: Vec::new(),
        }
    }

    /// Add a component status.
    pub fn add_component(mut self, name: &str, status: ComponentStatus) -> Self {
        self.components.insert(name.to_string(), status);
        self
    }

    /// Set systemd status.
    pub fn with_systemd(mut self, status: SystemdStatus) -> Self {
        self.systemd = status;
        self
    }

    /// Add a hint.
    pub fn add_hint(mut self, hint: &str) -> Self {
        self.hints.push(hint.to_string());
        self
    }

    /// Add hints from another list.
    pub fn add_hints(mut self, hints: &[String]) -> Self {
        self.hints.extend(hints.iter().cloned());
        self
    }

    /// Build the final RuntimeStatus, inferring mode and overall health.
    pub fn build(self) -> RuntimeStatus {
        let runtime_mode = infer_runtime_mode(&self.components, &self.systemd);
        let overall = infer_overall_health(&self.components, runtime_mode);

        // Generate hints from missing components
        let hints = if self.hints.is_empty() {
            generate_hints(&self.components, &self.systemd, runtime_mode)
        } else {
            self.hints
        };

        RuntimeStatus {
            ok: true,
            runtime_mode,
            overall,
            components: self.components,
            systemd: self.systemd,
            paths: RuntimePaths {
                repo_root: crate::shared::workspace_root(),
                plans_dir: crate::shared::default_ask_plans_dir(),
                chat_receipts_dir: crate::shared::default_chat_receipts_dir(),
                ask_receipts_dir: crate::shared::default_ask_receipts_dir(),
                apply_receipts_dir: crate::shared::default_apply_receipts_dir(),
                model_router_receipts_dir: None,
                local_model_path: std::env::var("OSAI_LLAMACPP_MODEL_PATH")
                    .ok()
                    .map(PathBuf::from),
                llamacpp_binary_path: std::env::var("OSAI_LLAMACPP_BIN").ok().map(PathBuf::from),
            },
            hints,
        }
    }
}

impl Default for RuntimeStatusBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Mode / Health Inference
// ============================================================================

/// Infer runtime mode from component and systemd state.
fn infer_runtime_mode(components: &ComponentStatusMap, systemd: &SystemdStatus) -> RuntimeMode {
    if systemd.available
        && systemd
            .services
            .as_ref()
            .map_or(false, |svcs| svcs.values().any(|s| s.active))
    {
        return RuntimeMode::Systemd;
    }

    let healthy_count = components.values().filter(|c| c.healthy).count();
    let total = components.len();

    if total == 0 {
        RuntimeMode::Unknown
    } else if healthy_count == total {
        RuntimeMode::Manual
    } else if healthy_count > 0 {
        RuntimeMode::Partial
    } else {
        RuntimeMode::Stopped
    }
}

/// Infer overall health from components and runtime mode.
fn infer_overall_health(components: &ComponentStatusMap, mode: RuntimeMode) -> OverallHealth {
    if components.is_empty() {
        return OverallHealth::Unknown;
    }

    let healthy_count = components.values().filter(|c| c.healthy).count();
    let total = components.len();

    match mode {
        RuntimeMode::Systemd | RuntimeMode::Manual => {
            if healthy_count == total {
                OverallHealth::Healthy
            } else if healthy_count > 0 {
                OverallHealth::Degraded
            } else {
                OverallHealth::Stopped
            }
        }
        RuntimeMode::Partial => {
            if healthy_count >= total / 2 {
                OverallHealth::Degraded
            } else {
                OverallHealth::Stopped
            }
        }
        RuntimeMode::Stopped => OverallHealth::Stopped,
        RuntimeMode::Unknown => OverallHealth::Unknown,
    }
}

/// Generate hints from missing/unhealthy components.
fn generate_hints(
    components: &ComponentStatusMap,
    systemd: &SystemdStatus,
    mode: RuntimeMode,
) -> Vec<String> {
    let mut hints = Vec::new();

    if !components.contains_key("llamacpp")
        || !components.get("llamacpp").map_or(false, |c| c.healthy)
    {
        hints.push("llama.cpp is not reachable on 127.0.0.1:8092".to_string());
    }

    if !components.contains_key("model_router")
        || !components.get("model_router").map_or(false, |c| c.healthy)
    {
        hints.push("Model Router is not reachable on 127.0.0.1:8088".to_string());
    }

    if !components.contains_key("osai_api")
        || !components.get("osai_api").map_or(false, |c| c.healthy)
    {
        hints.push("osai-api is not reachable on 127.0.0.1:8090".to_string());
    }

    if hints.is_empty() {
        if mode == RuntimeMode::Systemd {
            hints.push("All components healthy via systemd".to_string());
        } else if mode == RuntimeMode::Manual {
            hints.push("All components healthy in manual mode".to_string());
        }
    } else {
        if mode == RuntimeMode::Stopped || mode == RuntimeMode::Unknown {
            hints.push("Start the full stack with ./scripts/osai-local-up".to_string());
        }
        if systemd.available
            && !systemd
                .services
                .as_ref()
                .map_or(false, |svcs| svcs.values().any(|s| s.active))
        {
            hints.push("Install user services with ./scripts/osai-install-user-services --enable-now --enable-osai-api".to_string());
        }
    }

    hints
}

// ============================================================================
// Sync Wrapper for CLI (uses runtime's own thread pool)
// ============================================================================

/// Collect full runtime status synchronously.
/// Use this from CLI or other sync contexts; API should use the async version directly.
pub fn collect_runtime_status_sync() -> RuntimeStatus {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create runtime for runtime status collection");

    rt.block_on(async { collect_runtime_status_async().await })
}

/// Collect full runtime status asynchronously.
/// Used by osai-api for live HTTP health checks.
pub async fn collect_runtime_status_async() -> RuntimeStatus {
    // Check all HTTP components in parallel
    let (llamacpp, model_router, osai_api) = tokio::join!(
        check_llamacpp_status("http://127.0.0.1:8092/v1"),
        check_model_router_status("http://127.0.0.1:8088"),
        check_osai_api_status("http://127.0.0.1:8090"),
    );

    // Check systemd availability (blocking, but fast)
    let systemd_available = is_systemd_available();
    let systemd_services = if systemd_available {
        check_systemd_services()
    } else {
        None
    };

    let systemd = SystemdStatus {
        available: systemd_available,
        services: systemd_services,
    };

    RuntimeStatusBuilder::new()
        .add_component("llamacpp", llamacpp)
        .add_component("model_router", model_router)
        .add_component("osai_api", osai_api)
        .with_systemd(systemd)
        .build()
}

// ============================================================================
// HTTP Health Checks (async, used by osai-api)
// ============================================================================

/// Check llama.cpp health (GET /v1/models).
pub async fn check_llamacpp_status(url: &str) -> ComponentStatus {
    let models_url = format!("{}/models", url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2000))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ComponentStatus {
                url: url.to_string(),
                reachable: false,
                healthy: false,
                http_status: None,
                service: None,
                version: None,
                models: None,
                error: Some(format!("client build error: {}", e)),
            };
        }
    };

    match client.get(&models_url).send().await {
        Ok(resp) => {
            let http_status = resp.status().as_u16();
            let is_success = resp.status().is_success();
            if is_success {
                let json = resp.json::<serde_json::Value>().await.ok();
                let models: Option<Vec<String>> = json
                    .as_ref()
                    .and_then(|v| v.get("models").and_then(|m| m.as_array()))
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("name").or_else(|| m.get("model")))
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    });

                ComponentStatus {
                    url: url.to_string(),
                    reachable: true,
                    healthy: true,
                    http_status: Some(http_status),
                    service: Some("llama.cpp".to_string()),
                    version: None,
                    models,
                    error: None,
                }
            } else {
                ComponentStatus {
                    url: url.to_string(),
                    reachable: true,
                    healthy: false,
                    http_status: Some(http_status),
                    service: Some("llama.cpp".to_string()),
                    version: None,
                    models: None,
                    error: Some(format!("HTTP {}", http_status)),
                }
            }
        }
        Err(e) => ComponentStatus {
            url: url.to_string(),
            reachable: false,
            healthy: false,
            http_status: None,
            service: Some("llama.cpp".to_string()),
            version: None,
            models: None,
            error: Some(format!("connection refused: {}", e)),
        },
    }
}

/// Check Model Router health (GET /health).
pub async fn check_model_router_status(url: &str) -> ComponentStatus {
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2000))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ComponentStatus {
                url: url.to_string(),
                reachable: false,
                healthy: false,
                http_status: None,
                service: None,
                version: None,
                models: None,
                error: Some(format!("client build error: {}", e)),
            };
        }
    };

    match client.get(&health_url).send().await {
        Ok(resp) => {
            let http_status = resp.status().as_u16();
            let is_success = resp.status().is_success();
            let json = resp.json::<serde_json::Value>().await.ok();
            let service = json
                .as_ref()
                .and_then(|v| v.get("service").and_then(|s| s.as_str()))
                .map(String::from);

            ComponentStatus {
                url: url.to_string(),
                reachable: true,
                healthy: is_success,
                http_status: Some(http_status),
                service: service.or_else(|| Some("osai-model-router".to_string())),
                version: None,
                models: None,
                error: if is_success {
                    None
                } else {
                    Some(format!("HTTP {}", http_status))
                },
            }
        }
        Err(e) => ComponentStatus {
            url: url.to_string(),
            reachable: false,
            healthy: false,
            http_status: None,
            service: Some("osai-model-router".to_string()),
            version: None,
            models: None,
            error: Some(format!("connection refused: {}", e)),
        },
    }
}

/// Check osai-api health (GET /health).
pub async fn check_osai_api_status(url: &str) -> ComponentStatus {
    let health_url = format!("{}/health", url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2000))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ComponentStatus {
                url: url.to_string(),
                reachable: false,
                healthy: false,
                http_status: None,
                service: None,
                version: None,
                models: None,
                error: Some(format!("client build error: {}", e)),
            };
        }
    };

    match client.get(&health_url).send().await {
        Ok(resp) => {
            let http_status = resp.status().as_u16();
            let is_success = resp.status().is_success();
            let json = resp.json::<serde_json::Value>().await.ok();
            let service = json
                .as_ref()
                .and_then(|v| v.get("service").and_then(|s| s.as_str()))
                .map(String::from);
            let version = json
                .as_ref()
                .and_then(|v| v.get("version").and_then(|ver| ver.as_str()))
                .map(String::from);

            ComponentStatus {
                url: url.to_string(),
                reachable: true,
                healthy: is_success,
                http_status: Some(http_status),
                service: service.or_else(|| Some("osai-api".to_string())),
                version,
                models: None,
                error: if is_success {
                    None
                } else {
                    Some(format!("HTTP {}", http_status))
                },
            }
        }
        Err(e) => ComponentStatus {
            url: url.to_string(),
            reachable: false,
            healthy: false,
            http_status: None,
            service: Some("osai-api".to_string()),
            version: None,
            models: None,
            error: Some(format!("connection refused: {}", e)),
        },
    }
}

// ============================================================================
// Systemd Checks (blocking, used by CLI and API)
// ============================================================================

/// Check systemd user services status for OSAI stack.
/// Returns None if systemd is unavailable.
pub fn check_systemd_services() -> Option<HashMap<String, SystemdServiceStatus>> {
    let service_names = [
        "osai-llamacpp.service",
        "osai-model-router.service",
        "osai-api.service",
        "osai-vllm.service",
    ];

    let mut services = HashMap::new();

    for name in &service_names {
        let status = query_systemd_service(name);
        services.insert(name.to_string(), status);
    }

    Some(services)
}

/// Query a single systemd service's status.
fn query_systemd_service(name: &str) -> SystemdServiceStatus {
    use std::process::Command;

    // First check if service is installed (show)
    let show_output = Command::new("systemctl")
        .args(["--user", "show", name])
        .output();

    let (load_state, active_state, sub_state, enabled) = match show_output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut load = None;
            let mut active = None;
            let mut sub = None;
            let mut enabled_val = false;

            for line in stdout.lines() {
                if let Some(val) = line.strip_prefix("LoadState=") {
                    load = Some(val.to_string());
                } else if let Some(val) = line.strip_prefix("ActiveState=") {
                    active = Some(val.to_string());
                } else if let Some(val) = line.strip_prefix("SubState=") {
                    sub = Some(val.to_string());
                } else if let Some(val) = line.strip_prefix("UnitFileState=") {
                    enabled_val = val == "enabled";
                }
            }

            (load, active, sub, enabled_val)
        }
        Err(_) => (None, None, None, false),
    };

    SystemdServiceStatus {
        installed: load_state.is_some(),
        enabled,
        active: active_state.as_deref() == Some("active"),
        load_state,
        sub_state,
        error: None,
    }
}

/// Check if systemd is available (user instance can be reached).
pub fn is_systemd_available() -> bool {
    use std::process::Command;
    Command::new("systemctl")
        .args(["--user", "show", "--property=GROUP"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ============================================================================
// Pure Utility Functions (for testing)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_status_serialization_shape() {
        let status = RuntimeStatus {
            ok: true,
            runtime_mode: RuntimeMode::Systemd,
            overall: OverallHealth::Healthy,
            components: ComponentStatusMap::new(),
            systemd: SystemdStatus {
                available: true,
                services: None,
            },
            paths: RuntimePaths {
                repo_root: PathBuf::from("/repo"),
                plans_dir: PathBuf::from("/plans"),
                chat_receipts_dir: PathBuf::from("/chat"),
                ask_receipts_dir: PathBuf::from("/ask"),
                apply_receipts_dir: PathBuf::from("/apply"),
                model_router_receipts_dir: None,
                local_model_path: None,
                llamacpp_binary_path: None,
            },
            hints: vec!["All components healthy".to_string()],
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"runtime_mode\":\"systemd\""));
        assert!(json.contains("\"overall\":\"healthy\""));
        assert!(json.contains("\"hints\""));
    }

    #[test]
    fn test_component_status_serialization() {
        let comp = ComponentStatus {
            url: "http://127.0.0.1:8092/v1".to_string(),
            reachable: true,
            healthy: true,
            http_status: Some(200),
            service: Some("llama.cpp".to_string()),
            version: None,
            models: Some(vec!["gemma-4-E2B-it-Q8_0.gguf".to_string()]),
            error: None,
        };
        let json = serde_json::to_string(&comp).unwrap();
        assert!(json.contains("\"reachable\":true"));
        assert!(json.contains("\"healthy\":true"));
        assert!(json.contains("\"http_status\":200"));
        assert!(json.contains("\"models\""));
        assert!(!json.contains("\"error\":null")); // skip_serializing_if
    }

    #[test]
    fn test_runtime_mode_systemd_inference() {
        // All healthy + systemd active -> systemd
        let mut components = ComponentStatusMap::new();
        components.insert(
            "llamacpp".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8092/v1".to_string(),
                reachable: true,
                healthy: true,
                http_status: Some(200),
                service: None,
                version: None,
                models: None,
                error: None,
            },
        );

        let systemd = SystemdStatus {
            available: true,
            services: Some({
                let mut m = HashMap::new();
                m.insert(
                    "osai-llamacpp.service".to_string(),
                    SystemdServiceStatus {
                        installed: true,
                        enabled: true,
                        active: true,
                        load_state: Some("loaded".to_string()),
                        sub_state: Some("running".to_string()),
                        error: None,
                    },
                );
                m
            }),
        };

        let mode = infer_runtime_mode(&components, &systemd);
        assert_eq!(mode, RuntimeMode::Systemd);
    }

    #[test]
    fn test_runtime_mode_manual_inference() {
        // Components healthy but no active systemd -> manual
        let mut components = ComponentStatusMap::new();
        components.insert(
            "llamacpp".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8092/v1".to_string(),
                reachable: true,
                healthy: true,
                http_status: Some(200),
                service: None,
                version: None,
                models: None,
                error: None,
            },
        );
        let systemd = SystemdStatus {
            available: true,
            services: Some({
                let mut m = HashMap::new();
                m.insert(
                    "osai-llamacpp.service".to_string(),
                    SystemdServiceStatus {
                        installed: true,
                        enabled: true,
                        active: false, // not active
                        load_state: Some("loaded".to_string()),
                        sub_state: Some("dead".to_string()),
                        error: None,
                    },
                );
                m
            }),
        };

        let mode = infer_runtime_mode(&components, &systemd);
        // With healthy components but no active systemd -> Manual
        assert_eq!(mode, RuntimeMode::Manual);
    }

    #[test]
    fn test_runtime_mode_partial_inference() {
        // Some healthy -> partial
        let mut components = ComponentStatusMap::new();
        components.insert(
            "llamacpp".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8092/v1".to_string(),
                reachable: true,
                healthy: true,
                http_status: Some(200),
                service: None,
                version: None,
                models: None,
                error: None,
            },
        );
        // model_router not healthy
        components.insert(
            "model_router".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8088".to_string(),
                reachable: false,
                healthy: false,
                http_status: None,
                service: None,
                version: None,
                models: None,
                error: Some("connection refused".to_string()),
            },
        );

        let systemd = SystemdStatus {
            available: false,
            services: None,
        };

        let mode = infer_runtime_mode(&components, &systemd);
        assert_eq!(mode, RuntimeMode::Partial);
    }

    #[test]
    fn test_runtime_mode_all_stopped() {
        let components = ComponentStatusMap::new();
        let systemd = SystemdStatus {
            available: false,
            services: None,
        };

        let mode = infer_runtime_mode(&components, &systemd);
        assert_eq!(mode, RuntimeMode::Unknown);
    }

    #[test]
    fn test_overall_health_inference() {
        // All healthy -> healthy
        let mut components = ComponentStatusMap::new();
        components.insert(
            "test".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8088".to_string(),
                reachable: true,
                healthy: true,
                http_status: Some(200),
                service: None,
                version: None,
                models: None,
                error: None,
            },
        );

        let overall = infer_overall_health(&components, RuntimeMode::Systemd);
        assert_eq!(overall, OverallHealth::Healthy);
    }

    #[test]
    fn test_overall_health_degraded() {
        let mut components = ComponentStatusMap::new();
        components.insert(
            "llamacpp".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8092/v1".to_string(),
                reachable: true,
                healthy: true,
                http_status: Some(200),
                service: None,
                version: None,
                models: None,
                error: None,
            },
        );
        components.insert(
            "model_router".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8088".to_string(),
                reachable: false,
                healthy: false,
                http_status: None,
                service: None,
                version: None,
                models: None,
                error: None,
            },
        );

        let overall = infer_overall_health(&components, RuntimeMode::Partial);
        assert_eq!(overall, OverallHealth::Degraded);
    }

    #[test]
    fn test_systemd_unavailable_does_not_fail() {
        // When systemd is unavailable, is_systemd_available returns false
        // and we should still be able to construct a runtime status
        let components = ComponentStatusMap::new();
        let systemd = SystemdStatus {
            available: false,
            services: None,
        };

        let mode = infer_runtime_mode(&components, &systemd);
        assert_eq!(mode, RuntimeMode::Unknown);

        let overall = infer_overall_health(&components, mode);
        assert_eq!(overall, OverallHealth::Unknown);
    }

    #[test]
    fn test_builder_produces_valid_runtime_status() {
        let status = RuntimeStatusBuilder::new()
            .add_component(
                "llamacpp",
                ComponentStatus {
                    url: "http://127.0.0.1:8092/v1".to_string(),
                    reachable: true,
                    healthy: true,
                    http_status: Some(200),
                    service: Some("llama.cpp".to_string()),
                    version: None,
                    models: None,
                    error: None,
                },
            )
            .with_systemd(SystemdStatus {
                available: true,
                services: Some(HashMap::new()),
            })
            .build();

        assert!(status.ok);
        assert!(status.components.contains_key("llamacpp"));
    }

    #[test]
    fn test_hint_generation_when_llamacpp_down() {
        let mut components = ComponentStatusMap::new();
        components.insert(
            "llamacpp".to_string(),
            ComponentStatus {
                url: "http://127.0.0.1:8092/v1".to_string(),
                reachable: false,
                healthy: false,
                http_status: None,
                service: None,
                version: None,
                models: None,
                error: Some("connection refused".to_string()),
            },
        );

        let hints = generate_hints(
            &components,
            &SystemdStatus {
                available: false,
                services: None,
            },
            RuntimeMode::Stopped,
        );
        assert!(hints.iter().any(|h| h.contains("llama.cpp")));
    }

    #[test]
    fn test_component_status_skips_none_fields() {
        let comp = ComponentStatus {
            url: "http://127.0.0.1:8092/v1".to_string(),
            reachable: true,
            healthy: true,
            http_status: Some(200),
            service: None,
            version: None,
            models: None,
            error: None,
        };
        let json = serde_json::to_string(&comp).unwrap();
        assert!(!json.contains("\"service\":null"));
        assert!(!json.contains("\"version\":null"));
        assert!(!json.contains("\"models\":null"));
        assert!(!json.contains("\"error\":null"));
    }
}
