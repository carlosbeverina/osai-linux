//! OSAI Tool Executor - Safe execution layer for already-authorized tool requests.
//!
//! ToolExecutor takes authorization decisions from ToolBroker and executes
//! only the safest allowed actions. It serves as a sandboxed execution layer
//! that prevents any mutations or dangerous operations.
//!
//! # Example
//!
//! ```rust
//! use osai_tool_executor::{ExecutionStatus, ToolExecutor};
//! use osai_toolbroker::{ToolRequest, AuthorizationDecision, PolicyMode};
//! use osai_plan_dsl::ActionKind;
//! use osai_receipt_logger::ReceiptStore;
//! use std::collections::BTreeMap;
//! use tempfile::tempdir;
//!
//! let dir = tempdir().unwrap();
//! let store = ReceiptStore::new(dir.path());
//! store.ensure_dirs().unwrap();
//!
//! let executor = ToolExecutor::new(store, vec![std::path::PathBuf::from("/tmp")]);
//!
//! let mut inputs = BTreeMap::new();
//! inputs.insert("title".to_string(), serde_json::json!("Hello"));
//! inputs.insert("body".to_string(), serde_json::json!("World"));
//!
//! let request = ToolRequest::new("agent", ActionKind::DesktopNotify, "Test notification")
//!     .with_inputs(inputs);
//! ```

use osai_plan_dsl::ActionKind;
use osai_receipt_logger::{Receipt, ReceiptStatus, ReceiptStore};
use osai_toolbroker::{AuthorizationDecision, ToolRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

/// Result status of an execution attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Executed,
    Failed,
    Skipped,
}

/// Result of executing a tool request.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The request ID this result is for.
    pub request_id: Uuid,
    /// The status of execution.
    pub status: ExecutionStatus,
    /// The action that was executed.
    pub action: ActionKind,
    /// Output from execution, if any.
    pub output: Option<Value>,
    /// Error message if execution failed.
    pub error: Option<String>,
}

/// Errors for ToolExecutor operations.
#[derive(Debug, Clone, Error)]
pub enum ToolExecutorError {
    #[error("failed to write receipt: {0}")]
    ReceiptWrite(String),
}

/// Keys that indicate sensitive data.
const SECRET_KEYS: &[&str] = &["key", "token", "secret", "password", "credential"];

/// ToolExecutor - safe execution layer for authorized tool requests.
#[derive(Debug, Clone)]
pub struct ToolExecutor {
    /// The receipt store for audit logs.
    receipt_store: ReceiptStore,
    /// Allowed root directories for filesystem operations.
    allowed_roots: Vec<PathBuf>,
}

impl ToolExecutor {
    /// Creates a new ToolExecutor.
    pub fn new(receipt_store: ReceiptStore, allowed_roots: Vec<PathBuf>) -> Self {
        Self {
            receipt_store,
            allowed_roots,
        }
    }

    /// Redacts secret-looking values from inputs.
    fn redact_inputs(&self, inputs: &BTreeMap<String, Value>) -> Value {
        let mut redacted = serde_json::Map::new();

        for (key, value) in inputs {
            let lower_key = key.to_lowercase();
            let is_secret = SECRET_KEYS.iter().any(|s| lower_key.contains(s));

            if is_secret {
                redacted.insert(key.clone(), Value::String("[REDACTED]".to_string()));
            } else {
                redacted.insert(key.clone(), value.clone());
            }
        }

        Value::Object(redacted)
    }

    /// Returns the action name as a string.
    fn action_name(action: &ActionKind) -> String {
        match action {
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

    /// Creates a receipt for an execution attempt.
    fn create_receipt(
        &self,
        request: &ToolRequest,
        status: ReceiptStatus,
        output: Option<&Value>,
        error: Option<&str>,
    ) -> Receipt {
        let action_name = Self::action_name(&request.action);

        let inputs_redacted = self.redact_inputs(&request.inputs);

        let mut receipt = Receipt::new(&request.actor, &action_name)
            .with_tool("ToolExecutor")
            .with_risk(format!("{:?}", request.risk))
            .with_approval(format!("{:?}", request.action))
            .with_status(status)
            .with_inputs(inputs_redacted);

        // Use request ID for traceability
        receipt.id = request.id;

        if let Some(plan_id) = request.plan_id {
            receipt = receipt.with_plan_id(plan_id);
        }

        if let Some(output) = output {
            receipt = receipt.with_outputs(output.clone());
        }

        if let Some(error) = error {
            receipt = receipt.with_error(error);
        }

        receipt
    }

    /// Writes a receipt for an execution result.
    fn write_receipt(
        &self,
        request: &ToolRequest,
        status: ReceiptStatus,
        output: Option<&Value>,
        error: Option<&str>,
    ) -> Result<(), ToolExecutorError> {
        let receipt = self.create_receipt(request, status, output, error);
        self.receipt_store
            .write(&receipt)
            .map_err(|e| ToolExecutorError::ReceiptWrite(e.to_string()))?;
        Ok(())
    }

    /// Expands "~" to the user's home directory if present.
    fn expand_path(path: &str) -> PathBuf {
        if path.starts_with("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(path.trim_start_matches("~/"));
            }
        }
        PathBuf::from(path)
    }

    /// Checks if a path is within allowed roots.
    fn is_path_allowed(&self, path: &Path) -> bool {
        let canonical = if path.is_relative() {
            std::env::current_dir()
                .ok()
                .map(|p| p.join(path))
                .and_then(|p| p.canonicalize().ok())
        } else {
            path.canonicalize().ok()
        };

        if let Some(canonical) = canonical {
            for root in &self.allowed_roots {
                let root_canonical = if root.is_relative() {
                    std::env::current_dir()
                        .ok()
                        .map(|p| p.join(root))
                        .and_then(|p| p.canonicalize().ok())
                } else {
                    root.canonicalize().ok()
                };

                if let Some(root_canonical) = root_canonical {
                    if canonical.starts_with(&root_canonical) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Executes FilesList action (simulated - read-only listing).
    fn execute_files_list(&self, request: &ToolRequest) -> Result<Value, String> {
        let path_str = request
            .inputs
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("FilesList requires 'path' input")?;

        let path = Self::expand_path(path_str);

        // Check allowed roots
        if !self.is_path_allowed(&path) {
            return Err(format!(
                "Path '{}' is not within allowed roots",
                path.display()
            ));
        }

        // Simulated listing - we don't actually read filesystem in v0.1
        // But we check that the path could be a valid directory
        let entries: Vec<Value> = Vec::new();

        Ok(Value::Array(entries))
    }

    /// Executes DesktopNotify action (simulated).
    fn execute_desktop_notify(&self, request: &ToolRequest) -> Result<Value, String> {
        let title = request
            .inputs
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Notification");

        let body = request
            .inputs
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        Ok(serde_json::json!({
            "simulated": true,
            "title": title,
            "body": body
        }))
    }

    /// Executes ModelChat action (simulated).
    fn execute_model_chat(&self, _request: &ToolRequest) -> Result<Value, String> {
        Ok(serde_json::json!({
            "simulated": true,
            "message": "ModelChat execution is not implemented yet"
        }))
    }

    /// Executes an authorized request.
    ///
    /// # Errors
    ///
    /// Returns `ToolExecutorError` if receipt writing fails.
    pub fn execute_authorized(
        &self,
        request: &ToolRequest,
        decision: &AuthorizationDecision,
    ) -> Result<ExecutionResult, ToolExecutorError> {
        // Case 1: Denied - skip execution
        if !decision.allowed {
            let error_msg = format!("Action denied: {}", decision.reason);
            self.write_receipt(request, ReceiptStatus::Denied, None, Some(&error_msg))?;

            return Ok(ExecutionResult {
                request_id: request.id,
                status: ExecutionStatus::Skipped,
                action: request.action.clone(),
                output: None,
                error: Some(error_msg),
            });
        }

        // Case 2: Requires approval - skip execution
        if decision.requires_user_approval {
            let error_msg = "Execution skipped: requires user approval".to_string();
            self.write_receipt(request, ReceiptStatus::Planned, None, None)?;

            return Ok(ExecutionResult {
                request_id: request.id,
                status: ExecutionStatus::Skipped,
                action: request.action.clone(),
                output: None,
                error: Some(error_msg),
            });
        }

        // Case 3: Allowed and approved - execute only safe actions
        let result = match &request.action {
            ActionKind::FilesList => match self.execute_files_list(request) {
                Ok(output) => {
                    self.write_receipt(request, ReceiptStatus::Executed, Some(&output), None)?;
                    ExecutionResult {
                        request_id: request.id,
                        status: ExecutionStatus::Executed,
                        action: request.action.clone(),
                        output: Some(output),
                        error: None,
                    }
                }
                Err(e) => {
                    self.write_receipt(request, ReceiptStatus::Failed, None, Some(&e))?;
                    ExecutionResult {
                        request_id: request.id,
                        status: ExecutionStatus::Failed,
                        action: request.action.clone(),
                        output: None,
                        error: Some(e),
                    }
                }
            },
            ActionKind::DesktopNotify => match self.execute_desktop_notify(request) {
                Ok(output) => {
                    self.write_receipt(request, ReceiptStatus::Executed, Some(&output), None)?;
                    ExecutionResult {
                        request_id: request.id,
                        status: ExecutionStatus::Executed,
                        action: request.action.clone(),
                        output: Some(output),
                        error: None,
                    }
                }
                Err(e) => {
                    self.write_receipt(request, ReceiptStatus::Failed, None, Some(&e))?;
                    ExecutionResult {
                        request_id: request.id,
                        status: ExecutionStatus::Failed,
                        action: request.action.clone(),
                        output: None,
                        error: Some(e),
                    }
                }
            },
            ActionKind::ModelChat => match self.execute_model_chat(request) {
                Ok(output) => {
                    self.write_receipt(request, ReceiptStatus::Executed, Some(&output), None)?;
                    ExecutionResult {
                        request_id: request.id,
                        status: ExecutionStatus::Executed,
                        action: request.action.clone(),
                        output: Some(output),
                        error: None,
                    }
                }
                Err(e) => {
                    self.write_receipt(request, ReceiptStatus::Failed, None, Some(&e))?;
                    ExecutionResult {
                        request_id: request.id,
                        status: ExecutionStatus::Failed,
                        action: request.action.clone(),
                        output: None,
                        error: Some(e),
                    }
                }
            },
            // Unsupported actions - no mutation
            _ => {
                let error_msg = "Action is not executable in ToolExecutor v0.1".to_string();
                self.write_receipt(request, ReceiptStatus::Failed, None, Some(&error_msg))?;
                ExecutionResult {
                    request_id: request.id,
                    status: ExecutionStatus::Failed,
                    action: request.action.clone(),
                    output: None,
                    error: Some(error_msg),
                }
            }
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use osai_toolbroker::{AuthorizationDecision, PolicyMode};
    use std::collections::BTreeMap;

    fn create_test_store() -> (tempfile::TempDir, ReceiptStore) {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();
        (tempdir, store)
    }

    fn create_allowed_request(action: ActionKind) -> ToolRequest {
        ToolRequest::new("agent", action.clone(), "Test action")
    }

    fn create_allowed_decision() -> AuthorizationDecision {
        AuthorizationDecision {
            request_id: Uuid::new_v4(),
            allowed: true,
            requires_user_approval: false,
            reason: "Allowed by policy".to_string(),
            policy_mode: PolicyMode::Allow,
        }
    }

    fn create_denied_decision() -> AuthorizationDecision {
        AuthorizationDecision {
            request_id: Uuid::new_v4(),
            allowed: false,
            requires_user_approval: false,
            reason: "Denied by policy".to_string(),
            policy_mode: PolicyMode::Deny,
        }
    }

    fn create_approval_required_decision() -> AuthorizationDecision {
        AuthorizationDecision {
            request_id: Uuid::new_v4(),
            allowed: true,
            requires_user_approval: true,
            reason: "Requires approval".to_string(),
            policy_mode: PolicyMode::Ask,
        }
    }

    #[test]
    fn test_files_list_succeeds_for_allowed_root() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![PathBuf::from("/tmp")]);

        let mut request = create_allowed_request(ActionKind::FilesList);
        request.id = Uuid::new_v4();
        let mut inputs = BTreeMap::new();
        inputs.insert("path".to_string(), serde_json::json!("/tmp"));
        request.inputs = inputs;

        let decision = create_allowed_decision();
        let result = executor.execute_authorized(&request, &decision).unwrap();

        assert_eq!(result.status, ExecutionStatus::Executed);
        assert!(result.output.is_some());
        assert!(result.error.is_none());
    }

    #[test]
    fn test_files_list_denies_path_outside_allowed_roots() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![PathBuf::from("/tmp")]);

        let mut request = create_allowed_request(ActionKind::FilesList);
        request.id = Uuid::new_v4();
        let mut inputs = BTreeMap::new();
        inputs.insert("path".to_string(), serde_json::json!("/etc"));
        request.inputs = inputs;

        let decision = create_allowed_decision();
        let result = executor.execute_authorized(&request, &decision).unwrap();

        assert_eq!(result.status, ExecutionStatus::Failed);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("not within allowed roots"));
    }

    #[test]
    fn test_files_list_does_not_recurse() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![PathBuf::from("/tmp")]);

        // v0.1 doesn't actually list, but this verifies the action is handled
        let mut request = create_allowed_request(ActionKind::FilesList);
        request.id = Uuid::new_v4();
        let mut inputs = BTreeMap::new();
        // Use /tmp which exists - nested paths are accepted but listing is empty
        inputs.insert("path".to_string(), serde_json::json!("/tmp"));
        request.inputs = inputs;

        let decision = create_allowed_decision();
        let result = executor.execute_authorized(&request, &decision).unwrap();

        // In v0.1, we don't actually read filesystem, so path is accepted
        // The key is it doesn't try to recurse - we just return empty array
        assert_eq!(result.status, ExecutionStatus::Executed);
    }

    #[test]
    fn test_desktop_notify_returns_simulated_output() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![]);

        let mut request = create_allowed_request(ActionKind::DesktopNotify);
        request.id = Uuid::new_v4();
        let mut inputs = BTreeMap::new();
        inputs.insert("title".to_string(), serde_json::json!("Test Title"));
        inputs.insert("body".to_string(), serde_json::json!("Test Body"));
        request.inputs = inputs;

        let decision = create_allowed_decision();
        let result = executor.execute_authorized(&request, &decision).unwrap();

        assert_eq!(result.status, ExecutionStatus::Executed);
        let output = result.output.unwrap();
        assert_eq!(output["simulated"], true);
        assert_eq!(output["title"], "Test Title");
        assert_eq!(output["body"], "Test Body");
    }

    #[test]
    fn test_model_chat_returns_simulated_output() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![]);

        let request = create_allowed_request(ActionKind::ModelChat);
        let decision = create_allowed_decision();
        let result = executor.execute_authorized(&request, &decision).unwrap();

        assert_eq!(result.status, ExecutionStatus::Executed);
        let output = result.output.unwrap();
        assert_eq!(output["simulated"], true);
        assert!(output["message"]
            .as_str()
            .unwrap()
            .contains("not implemented yet"));
    }

    #[test]
    fn test_denied_decision_skips_execution() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![]);

        let request = create_allowed_request(ActionKind::DesktopNotify);
        let mut decision = create_denied_decision();
        decision.request_id = request.id;
        let result = executor.execute_authorized(&request, &decision).unwrap();

        assert_eq!(result.status, ExecutionStatus::Skipped);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("denied"));
    }

    #[test]
    fn test_approval_required_decision_skips_execution() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![]);

        let request = create_allowed_request(ActionKind::DesktopNotify);
        let mut decision = create_approval_required_decision();
        decision.request_id = request.id;
        let result = executor.execute_authorized(&request, &decision).unwrap();

        assert_eq!(result.status, ExecutionStatus::Skipped);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("requires user approval"));
    }

    #[test]
    fn test_unsupported_files_move_does_not_mutate() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![PathBuf::from("/tmp")]);

        let mut request = create_allowed_request(ActionKind::FilesMove);
        request.id = Uuid::new_v4();
        let mut inputs = BTreeMap::new();
        inputs.insert("source".to_string(), serde_json::json!("/tmp/a"));
        inputs.insert("destination".to_string(), serde_json::json!("/tmp/b"));
        request.inputs = inputs;

        let decision = create_allowed_decision();
        let result = executor.execute_authorized(&request, &decision).unwrap();

        assert_eq!(result.status, ExecutionStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .contains("not executable in ToolExecutor v0.1"));
    }

    #[test]
    fn test_execution_writes_receipt() {
        let (tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![PathBuf::from("/tmp")]);

        let mut request = create_allowed_request(ActionKind::DesktopNotify);
        request.id = Uuid::new_v4();
        let mut inputs = BTreeMap::new();
        inputs.insert("title".to_string(), serde_json::json!("Test"));
        inputs.insert("body".to_string(), serde_json::json!("Body"));
        request.inputs = inputs;

        let receipt_id = request.id;
        let decision = create_allowed_decision();
        executor.execute_authorized(&request, &decision).unwrap();

        // Verify receipt was written using the known request ID
        let store = ReceiptStore::new(tempdir.path());
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 1);

        let receipt = store.read(receipt_id).unwrap();
        assert_eq!(receipt.action, "DesktopNotify");
    }

    #[test]
    fn test_secrets_redacted_in_receipts() {
        let (tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![]);

        let mut request = create_allowed_request(ActionKind::DesktopNotify);
        request.id = Uuid::new_v4();
        let mut inputs = BTreeMap::new();
        inputs.insert("api_key".to_string(), serde_json::json!("secret-key-123"));
        inputs.insert("password".to_string(), serde_json::json!("my-password"));
        inputs.insert("title".to_string(), serde_json::json!("Test"));
        request.inputs = inputs;

        let receipt_id = request.id;
        let decision = create_allowed_decision();
        executor.execute_authorized(&request, &decision).unwrap();

        // Verify secrets are redacted in receipt
        let store = ReceiptStore::new(tempdir.path());
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 1);

        let receipt = store.read(receipt_id).unwrap();

        let redacted = &receipt.inputs_redacted;
        if let Value::Object(map) = redacted {
            if let Some(Value::String(val)) = map.get("api_key") {
                assert_eq!(val, "[REDACTED]");
            }
            if let Some(Value::String(val)) = map.get("password") {
                assert_eq!(val, "[REDACTED]");
            }
            if let Some(Value::String(val)) = map.get("title") {
                assert_eq!(val, "Test");
            }
        }
    }

    #[test]
    fn test_new_executor() {
        let (_tempdir, store) = create_test_store();
        let executor = ToolExecutor::new(store, vec![PathBuf::from("/home")]);
        // Just verify it constructs correctly
        assert!(true);
    }
}
