//! OSAI Receipt Logger - A reliable local receipt system for auditable AI actions.
//!
//! Receipts provide an immutable audit trail for all AI-mediated actions.
//! Every action executed through the OSAI system produces a receipt that captures
//! what was done, by whom, when, and with what outcome.
//!
//! # Example
//!
//! ```rust
//! use osai_receipt_logger::{Receipt, ReceiptStore, ReceiptStatus};
//! use std::path::PathBuf;
//! use tempfile::tempdir;
//!
//! let dir = tempdir().unwrap();
//! let store = ReceiptStore::new(dir.path());
//! store.ensure_dirs().unwrap();
//!
//! let receipt = Receipt::new("osai-agent", "FilesWrite")
//!     .with_risk("Medium")
//!     .with_approval("Ask")
//!     .with_plan_id(uuid::Uuid::new_v4())
//!     .with_status(ReceiptStatus::Executed)
//!     .with_inputs(serde_json::json!({}))
//!     .with_outputs(serde_json::json!({"path": "/tmp/test.txt"}));
//!
//! let path = store.write(&receipt).unwrap();
//! let loaded = store.read(receipt.id).unwrap();
//! assert_eq!(loaded.actor, "osai-agent");
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

/// Status of a receipt/execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    Planned,
    Approved,
    Denied,
    Executed,
    Failed,
    RolledBack,
}

/// A receipt recording an AI-mediated action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    /// Unique identifier for this receipt.
    pub id: Uuid,
    /// Timestamp when the receipt was created.
    pub timestamp: DateTime<Utc>,
    /// The actor (agent or user) who initiated the action.
    pub actor: String,
    /// The action that was performed.
    pub action: String,
    /// The tool that executed the action, if applicable.
    pub tool: Option<String>,
    /// The plan ID this receipt belongs to, if applicable.
    pub plan_id: Option<Uuid>,
    /// Risk level of the action.
    pub risk: String,
    /// Approval mode used.
    pub approval: String,
    /// Current status of the action.
    pub status: ReceiptStatus,
    /// Redacted inputs for the action.
    pub inputs_redacted: serde_json::Value,
    /// Redacted outputs from the action, if available.
    pub outputs_redacted: Option<serde_json::Value>,
    /// Error message if the action failed.
    pub error: Option<String>,
    /// Additional metadata.
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Validation errors for Receipt.
#[derive(Debug, Clone, Error)]
pub enum ReceiptValidationError {
    #[error("actor must not be empty")]
    EmptyActor,
    #[error("action must not be empty")]
    EmptyAction,
    #[error("risk must not be empty")]
    EmptyRisk,
    #[error("approval must not be empty")]
    EmptyApproval,
    #[error("error must be present if status is Failed")]
    MissingErrorOnFailed,
    #[error("outputs_redacted should be present if status is Executed or RolledBack")]
    MissingOutputsOnSuccess,
}

/// Parse errors for Receipt.
#[derive(Debug, Clone, Error)]
pub enum ReceiptParseError {
    #[error("failed to parse JSON: {0}")]
    JsonError(String),
}

/// Serialization errors for Receipt.
#[derive(Debug, Clone, Error)]
pub enum ReceiptSerializeError {
    #[error("failed to serialize to JSON: {0}")]
    JsonError(String),
}

/// Errors for ReceiptStore operations.
#[derive(Debug, Clone, Error)]
pub enum ReceiptStoreError {
    #[error("failed to create directory: {0}")]
    CreateDir(String),
    #[error("failed to read directory: {0}")]
    ReadDir(String),
    #[error("failed to read file: {0}")]
    ReadFile(String),
    #[error("failed to write file: {0}")]
    WriteFile(String),
    #[error("failed to parse receipt: {0}")]
    ParseReceipt(String),
    #[error("receipt validation failed: {0}")]
    Validation(String),
    #[error("receipt file already exists: {0}")]
    FileExists(PathBuf),
    #[error("receipt not found: {0}")]
    NotFound(Uuid),
}

impl Receipt {
    /// Creates a new Receipt with the given actor and action.
    ///
    /// The receipt is initialized with:
    /// - A new UUID
    /// - Current UTC timestamp
    /// - Empty optional fields
    /// - Empty metadata
    pub fn new(actor: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            actor: actor.into(),
            action: action.into(),
            tool: None,
            plan_id: None,
            risk: String::new(),
            approval: String::new(),
            status: ReceiptStatus::Planned,
            inputs_redacted: serde_json::Value::Null,
            outputs_redacted: None,
            error: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Sets the tool for this receipt.
    pub fn with_tool(mut self, tool: impl Into<String>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    /// Sets the plan ID for this receipt.
    pub fn with_plan_id(mut self, plan_id: Uuid) -> Self {
        self.plan_id = Some(plan_id);
        self
    }

    /// Sets the risk level for this receipt.
    pub fn with_risk(mut self, risk: impl Into<String>) -> Self {
        self.risk = risk.into();
        self
    }

    /// Sets the approval mode for this receipt.
    pub fn with_approval(mut self, approval: impl Into<String>) -> Self {
        self.approval = approval.into();
        self
    }

    /// Sets the status for this receipt.
    pub fn with_status(mut self, status: ReceiptStatus) -> Self {
        self.status = status;
        self
    }

    /// Sets the redacted inputs for this receipt.
    pub fn with_inputs(mut self, inputs: serde_json::Value) -> Self {
        self.inputs_redacted = inputs;
        self
    }

    /// Sets the redacted outputs for this receipt.
    pub fn with_outputs(mut self, outputs: serde_json::Value) -> Self {
        self.outputs_redacted = Some(outputs);
        self
    }

    /// Sets the error for this receipt.
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Validates this receipt according to OSAI rules.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptValidationError` if the receipt fails any validation rule.
    pub fn validate(&self) -> Result<(), ReceiptValidationError> {
        // actor must not be empty
        if self.actor.is_empty() {
            return Err(ReceiptValidationError::EmptyActor);
        }

        // action must not be empty
        if self.action.is_empty() {
            return Err(ReceiptValidationError::EmptyAction);
        }

        // risk must not be empty
        if self.risk.is_empty() {
            return Err(ReceiptValidationError::EmptyRisk);
        }

        // approval must not be empty
        if self.approval.is_empty() {
            return Err(ReceiptValidationError::EmptyApproval);
        }

        // error must be present if status is Failed
        if self.status == ReceiptStatus::Failed && self.error.is_none() {
            return Err(ReceiptValidationError::MissingErrorOnFailed);
        }

        // outputs_redacted should be present if status is Executed or RolledBack
        if (self.status == ReceiptStatus::Executed || self.status == ReceiptStatus::RolledBack)
            && self.outputs_redacted.is_none()
        {
            return Err(ReceiptValidationError::MissingOutputsOnSuccess);
        }

        Ok(())
    }

    /// Serializes this Receipt to a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptSerializeError` if serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, ReceiptSerializeError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ReceiptSerializeError::JsonError(e.to_string()))
    }

    /// Parses a Receipt from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptParseError` if parsing fails.
    pub fn from_json(input: &str) -> Result<Self, ReceiptParseError> {
        serde_json::from_str(input).map_err(|e| ReceiptParseError::JsonError(e.to_string()))
    }
}

/// A store for managing receipts on the filesystem.
#[derive(Debug, Clone)]
pub struct ReceiptStore {
    /// Root directory for storing receipts.
    root_dir: PathBuf,
}

impl ReceiptStore {
    /// Creates a new ReceiptStore with the given root directory.
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        Self {
            root_dir: root_dir.into(),
        }
    }

    /// Returns the root directory path.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Ensures the receipt storage directory exists.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError` if directory creation fails.
    pub fn ensure_dirs(&self) -> Result<(), ReceiptStoreError> {
        fs::create_dir_all(&self.root_dir).map_err(|e| {
            ReceiptStoreError::CreateDir(format!("{}: {}", self.root_dir.display(), e))
        })
    }

    /// Generates a filename-safe timestamp string.
    fn timestamp_string(timestamp: &DateTime<Utc>) -> String {
        timestamp.format("%Y%m%dT%H%M%S%.fZ").to_string()
    }

    /// Generates the file path for a receipt.
    fn receipt_path(&self, receipt: &Receipt) -> PathBuf {
        let ts = Self::timestamp_string(&receipt.timestamp);
        self.root_dir.join(format!("{}-{}.json", ts, receipt.id))
    }

    /// Writes a receipt to the store.
    ///
    /// Validates the receipt before writing.
    /// Does not overwrite existing receipt files.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError` if validation fails, file exists, or writing fails.
    pub fn write(&self, receipt: &Receipt) -> Result<PathBuf, ReceiptStoreError> {
        // Validate before saving
        receipt
            .validate()
            .map_err(|e| ReceiptStoreError::Validation(e.to_string()))?;

        let path = self.receipt_path(receipt);

        // Check if file already exists
        if path.exists() {
            return Err(ReceiptStoreError::FileExists(path));
        }

        // Write the receipt
        let json = receipt
            .to_json_pretty()
            .map_err(|e| ReceiptStoreError::WriteFile(e.to_string()))?;

        fs::write(&path, json)
            .map_err(|e| ReceiptStoreError::WriteFile(format!("{}: {}", path.display(), e)))?;

        Ok(path)
    }

    /// Reads a receipt by its UUID.
    ///
    /// Searches through all files in the root directory to find the receipt.
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError` if the receipt is not found or cannot be parsed.
    pub fn read(&self, id: Uuid) -> Result<Receipt, ReceiptStoreError> {
        // Read all files in the directory and search for the UUID
        let entries = fs::read_dir(&self.root_dir).map_err(|e| {
            ReceiptStoreError::ReadDir(format!("{}: {}", self.root_dir.display(), e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(receipt) = Receipt::from_json(&content) {
                        if receipt.id == id {
                            return Ok(receipt);
                        }
                    }
                }
            }
        }

        Err(ReceiptStoreError::NotFound(id))
    }

    /// Lists all receipt file paths, sorted ascending by filename (which includes timestamp).
    ///
    /// # Errors
    ///
    /// Returns `ReceiptStoreError` if the directory cannot be read.
    pub fn list(&self) -> Result<Vec<PathBuf>, ReceiptStoreError> {
        let mut paths = Vec::new();

        let entries = fs::read_dir(&self.root_dir).map_err(|e| {
            ReceiptStoreError::ReadDir(format!("{}: {}", self.root_dir.display(), e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                paths.push(path);
            }
        }

        // Sort ascending by filename
        paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_valid_receipt() -> Receipt {
        Receipt::new("osai-agent", "FilesWrite")
            .with_risk("Medium")
            .with_approval("Ask")
            .with_status(ReceiptStatus::Executed)
            .with_inputs(serde_json::json!({"path": "/tmp/test.txt"}))
            .with_outputs(serde_json::json!({"success": true}))
    }

    // Receipt::new and builder pattern tests

    #[test]
    fn test_receipt_new_generates_uuid() {
        let receipt = Receipt::new("agent", "action");
        assert_ne!(receipt.id, Uuid::nil());
    }

    #[test]
    fn test_receipt_new_has_timestamp() {
        let before = Utc::now();
        let receipt = Receipt::new("agent", "action");
        let after = Utc::now();
        assert!(receipt.timestamp >= before && receipt.timestamp <= after);
    }

    #[test]
    fn test_receipt_builder_chain() {
        let receipt = create_valid_receipt();
        assert_eq!(receipt.actor, "osai-agent");
        assert_eq!(receipt.action, "FilesWrite");
        assert_eq!(receipt.risk, "Medium");
        assert_eq!(receipt.approval, "Ask");
        assert_eq!(receipt.status, ReceiptStatus::Executed);
    }

    #[test]
    fn test_receipt_with_tool() {
        let receipt = Receipt::new("agent", "action").with_tool("ToolBroker");
        assert_eq!(receipt.tool, Some("ToolBroker".to_string()));
    }

    #[test]
    fn test_receipt_with_plan_id() {
        let plan_id = Uuid::new_v4();
        let receipt = Receipt::new("agent", "action").with_plan_id(plan_id);
        assert_eq!(receipt.plan_id, Some(plan_id));
    }

    #[test]
    fn test_receipt_with_error() {
        let receipt = Receipt::new("agent", "action").with_error("Something went wrong");
        assert_eq!(receipt.error, Some("Something went wrong".to_string()));
    }

    // Validation tests

    #[test]
    fn test_validate_valid_receipt() {
        let receipt = create_valid_receipt();
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn test_validate_empty_actor() {
        let receipt = Receipt::new("", "action")
            .with_risk("Low")
            .with_approval("Auto");
        let err = receipt.validate().unwrap_err();
        assert!(matches!(err, ReceiptValidationError::EmptyActor));
    }

    #[test]
    fn test_validate_empty_action() {
        let receipt = Receipt::new("agent", "")
            .with_risk("Low")
            .with_approval("Auto");
        let err = receipt.validate().unwrap_err();
        assert!(matches!(err, ReceiptValidationError::EmptyAction));
    }

    #[test]
    fn test_validate_empty_risk() {
        let receipt = Receipt::new("agent", "action")
            .with_risk("")
            .with_approval("Auto");
        let err = receipt.validate().unwrap_err();
        assert!(matches!(err, ReceiptValidationError::EmptyRisk));
    }

    #[test]
    fn test_validate_empty_approval() {
        let receipt = Receipt::new("agent", "action")
            .with_risk("Low")
            .with_approval("");
        let err = receipt.validate().unwrap_err();
        assert!(matches!(err, ReceiptValidationError::EmptyApproval));
    }

    #[test]
    fn test_validate_failed_without_error() {
        let receipt = Receipt::new("agent", "action")
            .with_risk("High")
            .with_approval("Ask")
            .with_status(ReceiptStatus::Failed);
        let err = receipt.validate().unwrap_err();
        assert!(matches!(err, ReceiptValidationError::MissingErrorOnFailed));
    }

    #[test]
    fn test_validate_failed_with_error() {
        let receipt = Receipt::new("agent", "action")
            .with_risk("High")
            .with_approval("Ask")
            .with_status(ReceiptStatus::Failed)
            .with_error("Network timeout");
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn test_validate_executed_without_outputs() {
        let receipt = Receipt::new("agent", "action")
            .with_risk("Medium")
            .with_approval("Ask")
            .with_status(ReceiptStatus::Executed)
            .with_inputs(serde_json::json!({}));
        let err = receipt.validate().unwrap_err();
        assert!(matches!(
            err,
            ReceiptValidationError::MissingOutputsOnSuccess
        ));
    }

    #[test]
    fn test_validate_rolled_back_without_outputs() {
        let receipt = Receipt::new("agent", "action")
            .with_risk("Medium")
            .with_approval("Ask")
            .with_status(ReceiptStatus::RolledBack)
            .with_inputs(serde_json::json!({}));
        let err = receipt.validate().unwrap_err();
        assert!(matches!(
            err,
            ReceiptValidationError::MissingOutputsOnSuccess
        ));
    }

    #[test]
    fn test_validate_planned_without_outputs() {
        let receipt = Receipt::new("agent", "action")
            .with_risk("Medium")
            .with_approval("Ask")
            .with_status(ReceiptStatus::Planned)
            .with_inputs(serde_json::json!({}));
        assert!(receipt.validate().is_ok());
    }

    // Serialization tests

    #[test]
    fn test_to_json_pretty() {
        let receipt = create_valid_receipt();
        let json = receipt.to_json_pretty().unwrap();
        assert!(json.contains("\"actor\": \"osai-agent\""));
        assert!(json.contains("\"action\": \"FilesWrite\""));
        assert!(json.contains("\"status\": \"Executed\""));
    }

    #[test]
    fn test_from_json() {
        let original = create_valid_receipt();
        let json = original.to_json_pretty().unwrap();
        let parsed = Receipt::from_json(&json).unwrap();
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.actor, original.actor);
        assert_eq!(parsed.action, original.action);
    }

    #[test]
    fn test_roundtrip() {
        let original = create_valid_receipt();
        let json = original.to_json_pretty().unwrap();
        let parsed = Receipt::from_json(&json).unwrap();
        let json2 = parsed.to_json_pretty().unwrap();
        assert_eq!(json, json2);
    }

    #[test]
    fn test_parse_invalid_json() {
        let err = Receipt::from_json("not valid json").unwrap_err();
        assert!(matches!(err, ReceiptParseError::JsonError(_)));
    }

    // ReceiptStore tests with tempfile

    #[test]
    fn test_store_new() {
        let store = ReceiptStore::new("/tmp/receipts");
        assert_eq!(store.root_dir(), Path::new("/tmp/receipts"));
    }

    #[test]
    fn test_store_ensure_dirs() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();
        assert!(tempdir.path().is_dir());
    }

    #[test]
    fn test_store_write_and_read() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();

        let receipt = create_valid_receipt();
        let path = store.write(&receipt).unwrap();

        assert!(path.exists());
        assert_eq!(path.parent(), Some(tempdir.path()));

        let loaded = store.read(receipt.id).unwrap();
        assert_eq!(loaded.id, receipt.id);
        assert_eq!(loaded.actor, receipt.actor);
        assert_eq!(loaded.action, receipt.action);
    }

    #[test]
    fn test_store_write_no_overwrite() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();

        let receipt = create_valid_receipt();
        store.write(&receipt).unwrap();

        let result = store.write(&receipt);
        assert!(matches!(result, Err(ReceiptStoreError::FileExists(_))));
    }

    #[test]
    fn test_store_list() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();

        // Create receipts with different timestamps
        let receipt1 = Receipt::new("agent1", "action1")
            .with_risk("Low")
            .with_approval("Auto")
            .with_status(ReceiptStatus::Executed)
            .with_inputs(serde_json::json!({}))
            .with_outputs(serde_json::json!({}));
        let receipt2 = Receipt::new("agent2", "action2")
            .with_risk("Low")
            .with_approval("Auto")
            .with_status(ReceiptStatus::Executed)
            .with_inputs(serde_json::json!({}))
            .with_outputs(serde_json::json!({}));

        store.write(&receipt1).unwrap();
        store.write(&receipt2).unwrap();

        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 2);
        // Paths should be sorted ascending
        assert!(paths[0] <= paths[1]);
    }

    #[test]
    fn test_store_read_not_found() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();

        let result = store.read(Uuid::new_v4());
        assert!(matches!(result, Err(ReceiptStoreError::NotFound(_))));
    }

    #[test]
    fn test_store_write_validates_before_saving() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();

        let invalid_receipt = Receipt::new("", "action")
            .with_risk("Low")
            .with_approval("Auto")
            .with_status(ReceiptStatus::Planned)
            .with_inputs(serde_json::json!({}));

        let result = store.write(&invalid_receipt);
        assert!(matches!(result, Err(ReceiptStoreError::Validation(_))));
    }

    #[test]
    fn test_receipt_path_format() {
        let tempdir = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(tempdir.path());
        store.ensure_dirs().unwrap();

        let receipt = create_valid_receipt();
        let path = store.receipt_path(&receipt);
        let filename = path.file_name().unwrap().to_str().unwrap();

        // Filename should be <timestamp>-<uuid>.json
        assert!(filename.ends_with(".json"));
        assert!(filename.contains(&receipt.id.to_string()));
    }
}
