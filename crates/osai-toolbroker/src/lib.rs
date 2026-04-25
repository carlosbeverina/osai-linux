//! OSAI ToolBroker - Authorization layer for AI tool execution.
//!
//! ToolBroker sits between AI agents and tool execution, enforcing policy
//! and creating audit receipts for every authorization decision.
//!
//! # Example
//!
//! ```rust
//! use osai_toolbroker::{ToolBroker, ToolPolicy, ToolRequest};
//! use osai_plan_dsl::{ActionKind, RiskLevel};
//! use osai_receipt_logger::ReceiptStore;
//! use tempfile::tempdir;
//!
//! let dir = tempdir().unwrap();
//! let store = ReceiptStore::new(dir.path());
//! store.ensure_dirs().unwrap();
//!
//! let broker = ToolBroker::new(ToolPolicy::default_secure(), store);
//!
//! let request = ToolRequest::new("osai-agent", ActionKind::ModelChat, "Chat with AI model");
//! let decision = broker.authorize(&request).unwrap();
//! assert!(decision.allowed);
//! ```

use osai_plan_dsl::{ActionKind, RiskLevel};
use osai_receipt_logger::{Receipt, ReceiptStatus, ReceiptStore};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

/// A request to authorize a tool action.
#[derive(Debug, Clone)]
pub struct ToolRequest {
    /// Unique identifier for this request.
    pub id: Uuid,
    /// The actor (agent or user) making the request.
    pub actor: String,
    /// Associated plan ID, if this is part of a plan.
    pub plan_id: Option<Uuid>,
    /// Associated step ID within the plan.
    pub step_id: Option<String>,
    /// The action being requested.
    pub action: ActionKind,
    /// Human-readable description of the action.
    pub description: String,
    /// Input parameters for the action.
    pub inputs: BTreeMap<String, serde_json::Value>,
    /// Risk level of the action.
    pub risk: RiskLevel,
}

/// The result of an authorization decision.
#[derive(Debug, Clone)]
pub struct AuthorizationDecision {
    /// The request ID this decision is for.
    pub request_id: Uuid,
    /// Whether the action is allowed.
    pub allowed: bool,
    /// Whether user approval is required before execution.
    pub requires_user_approval: bool,
    /// Human-readable reason for the decision.
    pub reason: String,
    /// The policy mode that was applied.
    pub policy_mode: PolicyMode,
}

/// Policy mode for authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyMode {
    Allow,
    Ask,
    Deny,
}

impl PolicyMode {
    /// Returns true if this mode allows execution.
    pub fn is_allow(&self) -> bool {
        matches!(self, PolicyMode::Allow)
    }

    /// Returns true if this mode requires approval.
    pub fn requires_approval(&self) -> bool {
        matches!(self, PolicyMode::Ask)
    }

    /// Returns true if this mode denies execution.
    pub fn is_deny(&self) -> bool {
        matches!(self, PolicyMode::Deny)
    }
}

/// Policy configuration for the ToolBroker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    /// Default mode for actions without explicit configuration.
    pub default_mode: PolicyMode,
    /// Per-action policy modes.
    pub action_modes: BTreeMap<String, PolicyMode>,
    /// Allowed root directories for filesystem actions.
    pub allowed_roots: Vec<String>,
    /// Whether shell network access is allowed.
    pub shell_network_allowed: bool,
    /// Whether shell commands must be sandboxed.
    pub shell_requires_sandbox: bool,
}

/// Parse errors for ToolPolicy.
#[derive(Debug, Clone, Error)]
pub enum ToolPolicyParseError {
    #[error("failed to parse YAML: {0}")]
    YamlError(String),
}

/// Serialization errors for ToolPolicy.
#[derive(Debug, Clone, Error)]
pub enum ToolPolicySerializeError {
    #[error("failed to serialize to YAML: {0}")]
    YamlError(String),
}

/// Errors for ToolBroker operations.
#[derive(Debug, Clone, Error)]
pub enum ToolBrokerError {
    #[error("failed to write receipt: {0}")]
    ReceiptWrite(String),
    #[error("receipt validation failed: {0}")]
    ReceiptValidation(String),
}

/// Keys that indicate sensitive data.
const SECRET_KEYS: &[&str] = &["key", "token", "secret", "password", "credential"];

impl ToolRequest {
    /// Creates a new ToolRequest.
    pub fn new(
        actor: impl Into<String>,
        action: ActionKind,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            actor: actor.into(),
            plan_id: None,
            step_id: None,
            action,
            description: description.into(),
            inputs: BTreeMap::new(),
            risk: RiskLevel::Low,
        }
    }

    /// Sets the plan ID for this request.
    pub fn with_plan_id(mut self, plan_id: Uuid) -> Self {
        self.plan_id = Some(plan_id);
        self
    }

    /// Sets the step ID for this request.
    pub fn with_step_id(mut self, step_id: impl Into<String>) -> Self {
        self.step_id = Some(step_id.into());
        self
    }

    /// Sets the inputs for this request.
    pub fn with_inputs(mut self, inputs: BTreeMap<String, serde_json::Value>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Sets the risk level for this request.
    pub fn with_risk(mut self, risk: RiskLevel) -> Self {
        self.risk = risk;
        self
    }

    /// Returns the action name as a string.
    pub fn action_name(&self) -> String {
        match &self.action {
            ActionKind::FilesList => "FilesList".to_string(),
            ActionKind::FilesRead => "FilesRead".to_string(),
            ActionKind::FilesWrite => "FilesWrite".to_string(),
            ActionKind::FilesMove => "FilesMove".to_string(),
            ActionKind::FilesDelete => "FilesDelete".to_string(),
            ActionKind::BrowserOpenUrl => "BrowserOpenUrl".to_string(),
            ActionKind::DesktopNotify => "DesktopNotify".to_string(),
            ActionKind::ShellRunSandboxed => "ShellRunSandboxed".to_string(),
            ActionKind::ModelChat => "ModelChat".to_string(),
            ActionKind::MemoryRead => "MemoryRead".to_string(),
            ActionKind::MemoryWrite => "MemoryWrite".to_string(),
            ActionKind::ReceiptCreate => "ReceiptCreate".to_string(),
            ActionKind::Custom(name) => format!("Custom({})", name),
        }
    }

    /// Returns true if this action requires explicit approval under any policy.
    pub fn action_requires_approval(&self) -> bool {
        matches!(
            self.action,
            ActionKind::FilesDelete
                | ActionKind::FilesWrite
                | ActionKind::FilesMove
                | ActionKind::ShellRunSandboxed
                | ActionKind::Custom(_)
        )
    }
}

impl ToolPolicy {
    /// Creates a secure default policy.
    ///
    /// This policy:
    /// - Allows read-only, safe actions (ModelChat, DesktopNotify, MemoryRead, FilesList, FilesRead)
    /// - Requires approval for file modifications (FilesWrite, FilesMove, FilesDelete)
    /// - Requires approval for shell commands
    /// - Denies network access without sandbox
    pub fn default_secure() -> Self {
        let mut action_modes = BTreeMap::new();

        // Safe actions - allowed by default
        action_modes.insert("ModelChat".to_string(), PolicyMode::Allow);
        action_modes.insert("DesktopNotify".to_string(), PolicyMode::Allow);
        action_modes.insert("MemoryRead".to_string(), PolicyMode::Allow);
        action_modes.insert("FilesList".to_string(), PolicyMode::Allow);
        action_modes.insert("FilesRead".to_string(), PolicyMode::Allow);
        action_modes.insert("MemoryWrite".to_string(), PolicyMode::Allow);
        action_modes.insert("ReceiptCreate".to_string(), PolicyMode::Allow);

        // Destructive/modifying actions - require approval
        action_modes.insert("FilesWrite".to_string(), PolicyMode::Ask);
        action_modes.insert("FilesMove".to_string(), PolicyMode::Ask);
        action_modes.insert("FilesDelete".to_string(), PolicyMode::Ask);
        action_modes.insert("ShellRunSandboxed".to_string(), PolicyMode::Ask);
        action_modes.insert("BrowserOpenUrl".to_string(), PolicyMode::Ask);

        // Custom actions - deny by default
        action_modes.insert("Custom".to_string(), PolicyMode::Deny);

        Self {
            default_mode: PolicyMode::Ask,
            action_modes,
            allowed_roots: vec![],
            shell_network_allowed: false,
            shell_requires_sandbox: true,
        }
    }

    /// Parses a ToolPolicy from YAML.
    pub fn from_yaml(input: &str) -> Result<Self, ToolPolicyParseError> {
        serde_yaml::from_str(input).map_err(|e| ToolPolicyParseError::YamlError(e.to_string()))
    }

    /// Serializes this ToolPolicy to YAML.
    pub fn to_yaml(&self) -> Result<String, ToolPolicySerializeError> {
        serde_yaml::to_string(self).map_err(|e| ToolPolicySerializeError::YamlError(e.to_string()))
    }

    /// Gets the policy mode for an action.
    fn get_action_mode(&self, action_name: &str) -> PolicyMode {
        // Check for exact match first
        if let Some(&mode) = self.action_modes.get(action_name) {
            return mode;
        }

        // Check for Custom prefix match
        if action_name.starts_with("Custom(") {
            if let Some(&mode) = self.action_modes.get("Custom") {
                return mode;
            }
        }

        self.default_mode
    }
}

/// The ToolBroker that handles authorization and receipt creation.
#[derive(Debug, Clone)]
pub struct ToolBroker {
    /// The policy to enforce.
    policy: ToolPolicy,
    /// The receipt store for audit logs.
    receipt_store: ReceiptStore,
}

impl ToolBroker {
    /// Creates a new ToolBroker with the given policy and receipt store.
    pub fn new(policy: ToolPolicy, receipt_store: ReceiptStore) -> Self {
        Self {
            policy,
            receipt_store,
        }
    }

    /// Redacts secret-looking values from inputs.
    fn redact_inputs(&self, inputs: &BTreeMap<String, serde_json::Value>) -> serde_json::Value {
        let mut redacted = serde_json::Map::new();

        for (key, value) in inputs {
            let lower_key = key.to_lowercase();
            let is_secret = SECRET_KEYS.iter().any(|s| lower_key.contains(s));

            if is_secret {
                redacted.insert(
                    key.clone(),
                    serde_json::Value::String("[REDACTED]".to_string()),
                );
            } else {
                redacted.insert(key.clone(), value.clone());
            }
        }

        serde_json::Value::Object(redacted)
    }

    /// Creates a receipt for an authorization decision.
    fn create_receipt(&self, request: &ToolRequest, decision: &AuthorizationDecision) -> Receipt {
        let action_name = request.action_name();

        // Determine receipt status based on decision
        let status = if decision.allowed && !decision.requires_user_approval {
            ReceiptStatus::Approved
        } else if decision.allowed {
            ReceiptStatus::Planned
        } else {
            ReceiptStatus::Denied
        };

        // Build redacted inputs
        let inputs_redacted = serde_json::json!({
            "action": action_name,
            "description": request.description.clone(),
            "inputs": self.redact_inputs(&request.inputs),
        });

        let mut receipt = Receipt::new(&request.actor, &action_name)
            .with_tool("ToolBroker")
            .with_risk(format!("{:?}", request.risk))
            .with_approval(format!("{:?}", decision.policy_mode))
            .with_status(status)
            .with_inputs(inputs_redacted);

        // Use the request ID for traceability
        receipt.id = request.id;

        if let Some(plan_id) = request.plan_id {
            receipt = receipt.with_plan_id(plan_id);
        }

        receipt
    }

    /// Authorizes a tool request and creates an audit receipt.
    pub fn authorize(
        &self,
        request: &ToolRequest,
    ) -> Result<AuthorizationDecision, ToolBrokerError> {
        let action_name = request.action_name();
        let mut mode = self.policy.get_action_mode(&action_name);
        let mut reason = String::new();
        let mut allowed = true;
        let mut requires_user_approval = false;

        // ShellRunSandboxed specific checks
        if let ActionKind::ShellRunSandboxed = &request.action {
            let network = request
                .inputs
                .get("network")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let sandbox = request
                .inputs
                .get("sandbox")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // Check network policy
            if network && !self.policy.shell_network_allowed {
                reason =
                    "ShellRunSandboxed with network=true denied: shell_network_allowed is false"
                        .to_string();
                allowed = false;
                mode = PolicyMode::Deny;
            }
            // Check sandbox policy
            else if !sandbox && self.policy.shell_requires_sandbox {
                reason = "ShellRunSandboxed denied: sandbox required but inputs.sandbox != true"
                    .to_string();
                allowed = false;
                mode = PolicyMode::Deny;
            }
        }

        // Apply policy decision
        if allowed {
            match mode {
                PolicyMode::Allow => {
                    reason = format!("Action {} allowed by policy", action_name);
                    allowed = true;
                    requires_user_approval = false;
                }
                PolicyMode::Ask => {
                    reason = format!("Action {} requires user approval", action_name);
                    allowed = true;
                    requires_user_approval = true;
                }
                PolicyMode::Deny => {
                    if reason.is_empty() {
                        reason = format!("Action {} denied by policy", action_name);
                    }
                    allowed = false;
                    requires_user_approval = false;
                }
            }
        }

        let decision = AuthorizationDecision {
            request_id: request.id,
            allowed,
            requires_user_approval,
            reason: reason.clone(),
            policy_mode: mode,
        };

        // Create and write receipt
        let receipt = self.create_receipt(request, &decision);
        self.receipt_store
            .write(&receipt)
            .map_err(|e| ToolBrokerError::ReceiptWrite(e.to_string()))?;

        Ok(decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_store() -> (tempfile::TempDir, ReceiptStore) {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();
        (tempdir, store)
    }

    fn create_broker_with_store(store: &ReceiptStore) -> ToolBroker {
        ToolBroker::new(ToolPolicy::default_secure(), store.clone())
    }

    // Tests: default policy allows ModelChat

    #[test]
    fn test_default_policy_allows_model_chat() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::ModelChat, "Chat with AI");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(!decision.requires_user_approval);
        assert!(decision.policy_mode.is_allow());
    }

    // Tests: default policy allows DesktopNotify

    #[test]
    fn test_default_policy_allows_desktop_notify() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::DesktopNotify, "Send notification");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(!decision.requires_user_approval);
    }

    // Tests: FilesDelete requires approval

    #[test]
    fn test_files_delete_requires_approval() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::FilesDelete, "Delete file");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(decision.requires_user_approval);
        assert!(decision.policy_mode.requires_approval());
    }

    // Tests: FilesWrite requires approval

    #[test]
    fn test_files_write_requires_approval() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::FilesWrite, "Write file");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(decision.requires_user_approval);
    }

    // Tests: FilesMove requires approval

    #[test]
    fn test_files_move_requires_approval() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::FilesMove, "Move file");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(decision.requires_user_approval);
    }

    // Tests: ShellRunSandboxed requires approval

    #[test]
    fn test_shell_run_sandboxed_requires_approval() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let mut inputs = BTreeMap::new();
        inputs.insert("command".to_string(), serde_json::json!("ls"));
        inputs.insert("sandbox".to_string(), serde_json::json!(true));

        let request = ToolRequest::new("agent", ActionKind::ShellRunSandboxed, "Run shell")
            .with_inputs(inputs);

        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(decision.requires_user_approval);
    }

    // Tests: ShellRunSandboxed with network denied by default

    #[test]
    fn test_shell_sandbox_network_denied_by_default() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let mut inputs = BTreeMap::new();
        inputs.insert(
            "command".to_string(),
            serde_json::json!("curl https://example.com"),
        );
        inputs.insert("network".to_string(), serde_json::json!(true));
        inputs.insert("sandbox".to_string(), serde_json::json!(false));

        let request = ToolRequest::new(
            "agent",
            ActionKind::ShellRunSandboxed,
            "Run shell with network",
        )
        .with_inputs(inputs);

        let decision = broker.authorize(&request).unwrap();

        assert!(!decision.allowed);
        assert!(decision.policy_mode.is_deny());
        assert!(decision.reason.contains("network=true"));
    }

    // Tests: ShellRunSandboxed without sandbox denied by default

    #[test]
    fn test_shell_sandbox_without_sandbox_denied_by_default() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let mut inputs = BTreeMap::new();
        inputs.insert("command".to_string(), serde_json::json!("ls"));
        inputs.insert("sandbox".to_string(), serde_json::json!(false));

        let request = ToolRequest::new(
            "agent",
            ActionKind::ShellRunSandboxed,
            "Run shell without sandbox",
        )
        .with_inputs(inputs);

        let decision = broker.authorize(&request).unwrap();

        assert!(!decision.allowed);
        assert!(decision.policy_mode.is_deny());
        assert!(decision.reason.contains("sandbox required"));
    }

    // Tests: Custom action requires approval

    #[test]
    fn test_custom_action_requires_approval() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new(
            "agent",
            ActionKind::Custom("my_custom_action".to_string()),
            "Custom action",
        );
        let decision = broker.authorize(&request).unwrap();

        assert!(!decision.allowed);
        assert!(decision.policy_mode.is_deny());
    }

    // Tests: explicit deny policy denies action

    #[test]
    fn test_explicit_deny_policy_denies() {
        let (_tempdir, store) = create_test_store();
        let mut policy = ToolPolicy::default_secure();
        policy
            .action_modes
            .insert("ModelChat".to_string(), PolicyMode::Deny);

        let broker = ToolBroker::new(policy, store);

        let request = ToolRequest::new("agent", ActionKind::ModelChat, "Chat with AI");
        let decision = broker.authorize(&request).unwrap();

        assert!(!decision.allowed);
        assert!(decision.policy_mode.is_deny());
    }

    // Tests: explicit ask policy requires approval

    #[test]
    fn test_explicit_ask_policy_requires_approval() {
        let (_tempdir, store) = create_test_store();
        let mut policy = ToolPolicy::default_secure();
        policy
            .action_modes
            .insert("ModelChat".to_string(), PolicyMode::Ask);

        let broker = ToolBroker::new(policy, store);

        let request = ToolRequest::new("agent", ActionKind::ModelChat, "Chat with AI");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(decision.requires_user_approval);
    }

    // Tests: secret-looking inputs are redacted in receipt

    #[test]
    fn test_secret_inputs_redacted_in_receipt() {
        let (tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let mut inputs = BTreeMap::new();
        inputs.insert("api_key".to_string(), serde_json::json!("secret-key-123"));
        inputs.insert("path".to_string(), serde_json::json!("/tmp/test"));

        let request = ToolRequest::new("agent", ActionKind::FilesWrite, "Write file with secrets")
            .with_inputs(inputs);

        let receipt_id = request.id;
        broker.authorize(&request).unwrap();

        // Find the receipt that was written
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 1);

        // Read the receipt we just created using its known ID
        let receipt = store.read(receipt_id).unwrap();

        let redacted = &receipt.inputs_redacted;
        if let serde_json::Value::Object(map) = redacted {
            // Check that action and description are present
            assert!(map.contains_key("action"));
            assert!(map.contains_key("description"));
            assert!(map.contains_key("inputs"));

            // The inputs should be redacted
            if let Some(serde_json::Value::Object(inputs_map)) = map.get("inputs") {
                // api_key should be redacted
                if let Some(serde_json::Value::String(val)) = inputs_map.get("api_key") {
                    assert_eq!(val, "[REDACTED]");
                } else {
                    panic!("api_key should be present and redacted");
                }
                // path should not be redacted
                if let Some(serde_json::Value::String(val)) = inputs_map.get("path") {
                    assert_eq!(val, "/tmp/test");
                }
            }
        }
    }

    // Tests: authorize writes a receipt file

    #[test]
    fn test_authorize_writes_receipt() {
        let (tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::ModelChat, "Chat with AI");
        broker.authorize(&request).unwrap();

        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 1);
    }

    // Tests: policy roundtrip YAML works

    #[test]
    fn test_policy_roundtrip_yaml() {
        let policy = ToolPolicy::default_secure();
        let yaml = policy.to_yaml().unwrap();
        let parsed = ToolPolicy::from_yaml(&yaml).unwrap();

        assert_eq!(parsed.default_mode, policy.default_mode);
        assert_eq!(parsed.shell_network_allowed, policy.shell_network_allowed);
        assert_eq!(parsed.shell_requires_sandbox, policy.shell_requires_sandbox);
    }

    // Additional tests

    #[test]
    fn test_tool_request_builder() {
        let request = ToolRequest::new("agent", ActionKind::FilesWrite, "Write file")
            .with_plan_id(Uuid::new_v4())
            .with_step_id("step-1")
            .with_risk(RiskLevel::Medium);

        assert_eq!(request.actor, "agent");
        assert!(matches!(request.action, ActionKind::FilesWrite));
        assert_eq!(request.description, "Write file");
        assert!(request.plan_id.is_some());
        assert_eq!(request.step_id, Some("step-1".to_string()));
        assert_eq!(request.risk, RiskLevel::Medium);
    }

    #[test]
    fn test_authorization_decision_fields() {
        let request = ToolRequest::new("agent", ActionKind::ModelChat, "Chat");
        let decision = AuthorizationDecision {
            request_id: request.id,
            allowed: true,
            requires_user_approval: false,
            reason: "Allowed".to_string(),
            policy_mode: PolicyMode::Allow,
        };

        assert_eq!(decision.request_id, request.id);
        assert!(decision.allowed);
        assert!(!decision.requires_user_approval);
    }

    #[test]
    fn test_policy_mode_helpers() {
        assert!(PolicyMode::Allow.is_allow());
        assert!(!PolicyMode::Allow.requires_approval());
        assert!(!PolicyMode::Allow.is_deny());

        assert!(!PolicyMode::Ask.is_allow());
        assert!(PolicyMode::Ask.requires_approval());
        assert!(!PolicyMode::Ask.is_deny());

        assert!(!PolicyMode::Deny.is_allow());
        assert!(!PolicyMode::Deny.requires_approval());
        assert!(PolicyMode::Deny.is_deny());
    }

    #[test]
    fn test_action_name() {
        let request = ToolRequest::new("agent", ActionKind::Custom("test".to_string()), "desc");
        assert_eq!(request.action_name(), "Custom(test)");

        let request = ToolRequest::new("agent", ActionKind::ShellRunSandboxed, "desc");
        assert_eq!(request.action_name(), "ShellRunSandboxed");
    }

    #[test]
    fn test_files_read_allowed_by_default() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::FilesRead, "Read file");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(!decision.requires_user_approval);
    }

    #[test]
    fn test_browser_open_url_requires_approval() {
        let (_tempdir, store) = create_test_store();
        let broker = create_broker_with_store(&store);

        let request = ToolRequest::new("agent", ActionKind::BrowserOpenUrl, "Open URL");
        let decision = broker.authorize(&request).unwrap();

        assert!(decision.allowed);
        assert!(decision.requires_user_approval);
    }
}
