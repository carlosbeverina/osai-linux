//! Shared types and utilities for chat, ask, and apply operations.

use osai_plan_dsl::{OsaiPlan, PlanStep};
use osai_toolbroker::ToolRequest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// ============================================================================
// Request/Response Types (shared between chat, ask, apply)
// ============================================================================

/// Chat request sent to Model Router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    pub temperature: f32,
    pub metadata: ChatMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMetadata {
    pub privacy: String,
}

/// Chat response from Model Router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: Option<ChatUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub message: ChatChoiceMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoiceMessage {
    pub role: String,
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

// ============================================================================
// Workspace Root Resolution
// ============================================================================

/// Returns the OSAI workspace root using compile-time CARGO_MANIFEST_DIR.
/// This is stable regardless of where the binary is run from or what cwd is.
/// Can be overridden at runtime via OSAI_REPO_ROOT env var.
pub fn workspace_root() -> PathBuf {
    // Runtime override (for installed/dev scenarios where env var is set explicitly)
    if let Ok(override_path) = std::env::var("OSAI_REPO_ROOT") {
        let p = PathBuf::from(override_path);
        if p.is_absolute() {
            return p;
        }
    }

    // Compile-time fallback: derive from CARGO_MANIFEST_DIR
    // manifest_dir for osai-agent-core is .../osai-linux/crates/osai-agent-core
    // parent() -> .../osai-linux/crates, parent() again -> .../osai-linux (workspace root)
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        // Should never fail in practice — CARGO_MANIFEST_DIR is always set for
        // workspace crates. Fallback to current dir only if compilation environment
        // is unusual.
        .unwrap_or_else(|| PathBuf::from("."))
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Validates that a URL is loopback-only (127.0.0.1 or localhost).
pub fn is_loopback_url(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        if parsed.scheme() != "http" {
            return false;
        }
        match parsed.host_str() {
            Some("localhost") | Some("127.0.0.1") => true,
            _ => false,
        }
    } else {
        false
    }
}

/// Converts a plan step to a ToolRequest for authorization/execution.
pub fn step_to_request(plan: &OsaiPlan, step: &PlanStep) -> ToolRequest {
    let mut request = ToolRequest::new(&plan.actor, step.action.clone(), &step.description)
        .with_plan_id(plan.id)
        .with_step_id(&step.id)
        .with_inputs(step.inputs.clone())
        .with_risk(plan.risk.clone());

    // Set request ID to link receipt to step
    request.id = Uuid::new_v4();

    request
}

/// Sanitizes YAML response by stripping markdown fences.
pub fn sanitize_yaml_response(content: &str) -> String {
    let trimmed = content.trim();
    // Strip markdown fences if present
    if trimmed.starts_with("```yaml") || trimmed.starts_with("```") {
        let without_fence = trimmed
            .trim_start_matches("```yaml")
            .trim_start_matches("```")
            .trim_start_matches('\n');
        // Find closing fence
        if let Some(end) = without_fence.find("```") {
            return without_fence[..end].trim_end().to_string();
        }
        return without_fence.to_string();
    }
    trimmed.to_string()
}

/// Creates a slug from a request string for plan filenames.
pub fn slug_from_request(request: &str) -> String {
    request
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}

/// Default receipts directory for chat.
pub fn default_chat_receipts_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("osai")
        .join("receipts")
        .join("chat")
}

/// Default receipts directory for ask.
pub fn default_ask_receipts_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("osai")
        .join("receipts")
        .join("ask")
}

/// Default receipts directory for apply.
pub fn default_apply_receipts_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("osai")
        .join("receipts")
        .join("apply")
}

/// Default plans directory for ask/generated plans.
/// Uses XDG data dir for persistence: ~/.local/share/osai/plans
pub fn default_ask_plans_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("osai")
        .join("plans")
}

/// Default policy file path (absolute).
/// Resolved at compile time from workspace root, not affected by cwd at runtime.
pub fn default_policy_path() -> PathBuf {
    workspace_root()
        .join("examples")
        .join("policies")
        .join("default-secure.yml")
}

/// Resolve a policy path for API/CLI usage.
///
/// - If `policy_path` is Some and absolute: use it as-is
/// - If `policy_path` is Some and relative: resolve from workspace root
/// - If `policy_path` is None: use default_policy_path()
/// - Runtime OSAI_REPO_ROOT env var overrides workspace root detection
///
/// Returns an absolute PathBuf ready for use.
pub fn resolve_policy_path(policy_path: Option<&str>) -> PathBuf {
    match policy_path {
        Some(p) => {
            let p = PathBuf::from(p);
            if p.is_absolute() {
                p
            } else {
                // Relative paths resolved from workspace root (compile-time stable)
                workspace_root().join(p)
            }
        }
        None => default_policy_path(),
    }
}
