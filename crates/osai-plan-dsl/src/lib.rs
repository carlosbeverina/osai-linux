//! OSAI Plan DSL - A safe typed intermediate representation for AI-generated plans.
//!
//! This crate defines the OSAI Plan DSL used to represent AI-generated plans
//! before they are executed through the ToolBroker.
//!
//! # Example
//!
//! ```yaml
//! version: "0.1"
//! id: "550e8400-e29b-41d4-a716-446655440000"
//! title: "Create project directory"
//! actor: "osai-agent"
//! risk: Medium
//! approval: Ask
//! steps:
//!   - id: "step-1"
//!     action: FilesCreate
//!     description: "Create the project directory"
//!     requires_approval: true
//!     inputs:
//!       path: "/home/user/project"
//!       recursive: true
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

/// Risk level classification for plan execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Approval mode for plan execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalMode {
    Auto,
    Ask,
    AlwaysDeny,
}

/// Action kinds that can be executed through ToolBroker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "name")]
pub enum ActionKind {
    FilesList,
    FilesRead,
    FilesWrite,
    FilesMove,
    FilesDelete,
    BrowserOpenUrl,
    DesktopNotify,
    ShellRunSandboxed,
    ModelChat,
    MemoryRead,
    MemoryWrite,
    ReceiptCreate,
    Custom(String),
}

/// A single step in an OSAI plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Unique identifier for this step.
    pub id: String,
    /// The action to be performed.
    pub action: ActionKind,
    /// Human-readable description of this step.
    pub description: String,
    /// Whether this step requires explicit approval before execution.
    pub requires_approval: bool,
    /// Input parameters for this step.
    pub inputs: BTreeMap<String, serde_json::Value>,
}

/// Rollback plan for reverting a failed plan execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPlan {
    /// Whether rollback is available.
    pub available: bool,
    /// Steps to execute for rollback.
    pub steps: Vec<PlanStep>,
}

/// The main OSAI Plan structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsaiPlan {
    /// Plan DSL version.
    pub version: String,
    /// Unique identifier for this plan.
    pub id: Uuid,
    /// Human-readable title for this plan.
    pub title: String,
    /// Optional detailed description.
    pub description: Option<String>,
    /// The actor (agent or user) this plan is for.
    pub actor: String,
    /// Risk level of executing this plan.
    pub risk: RiskLevel,
    /// Approval mode required for execution.
    pub approval: ApprovalMode,
    /// Steps to execute in order.
    pub steps: Vec<PlanStep>,
    /// Optional rollback plan.
    pub rollback: Option<RollbackPlan>,
    /// Additional metadata.
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Validation errors for OsaiPlan.
#[derive(Debug, Clone, Error)]
pub enum PlanValidationError {
    #[error("version must not be empty")]
    EmptyVersion,
    #[error("title must not be empty")]
    EmptyTitle,
    #[error("actor must not be empty")]
    EmptyActor,
    #[error("steps must not be empty")]
    EmptySteps,
    #[error("step id '{0}' must not be empty")]
    EmptyStepId(String),
    #[error("step id '{0}' is duplicated")]
    DuplicateStepId(String),
    #[error("critical risk requires ApprovalMode::Ask or ApprovalMode::AlwaysDeny, got {0:?}")]
    CriticalRiskRequiresApproval(RiskLevel),
    #[error("action {0:?} must require approval")]
    ActionRequiresApproval(ActionKind),
    #[error("ShellRunSandboxed with network=true requires sandbox=true")]
    ShellSandboxNetworkConflict,
}

/// Parse errors for OsaiPlan.
#[derive(Debug, Clone, Error)]
pub enum PlanParseError {
    #[error("failed to parse YAML: {0}")]
    YamlError(String),
    #[error("failed to parse JSON: {0}")]
    JsonError(String),
}

/// Serialization errors for OsaiPlan.
#[derive(Debug, Clone, Error)]
pub enum PlanSerializeError {
    #[error("failed to serialize to YAML: {0}")]
    YamlError(String),
    #[error("failed to serialize to JSON: {0}")]
    JsonError(String),
}

impl ActionKind {
    /// Returns true if this action is a destructive or high-impact action.
    pub fn is_destructive(&self) -> bool {
        matches!(
            self,
            ActionKind::FilesDelete | ActionKind::FilesWrite | ActionKind::FilesMove
        )
    }

    /// Returns true if this action requires approval.
    pub fn requires_approval(&self) -> bool {
        matches!(
            self,
            ActionKind::FilesDelete
                | ActionKind::FilesWrite
                | ActionKind::FilesMove
                | ActionKind::ShellRunSandboxed
                | ActionKind::Custom(_)
        )
    }
}

impl OsaiPlan {
    /// Creates a new OsaiPlan with a generated UUID.
    pub fn new(title: String, actor: String, risk: RiskLevel, approval: ApprovalMode) -> Self {
        Self {
            version: "0.1".to_string(),
            id: Uuid::new_v4(),
            title,
            description: None,
            actor,
            risk,
            approval,
            steps: Vec::new(),
            rollback: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Validates this plan according to OSAI DSL rules.
    ///
    /// # Errors
    ///
    /// Returns `PlanValidationError` if the plan fails any validation rule.
    pub fn validate(&self) -> Result<(), PlanValidationError> {
        // version must not be empty
        if self.version.is_empty() {
            return Err(PlanValidationError::EmptyVersion);
        }

        // title must not be empty
        if self.title.is_empty() {
            return Err(PlanValidationError::EmptyTitle);
        }

        // actor must not be empty
        if self.actor.is_empty() {
            return Err(PlanValidationError::EmptyActor);
        }

        // steps must not be empty
        if self.steps.is_empty() {
            return Err(PlanValidationError::EmptySteps);
        }

        // Collect step IDs to check for duplicates
        let mut seen_ids = std::collections::HashSet::new();

        for step in &self.steps {
            // step id must not be empty
            if step.id.is_empty() {
                return Err(PlanValidationError::EmptyStepId(step.id.clone()));
            }

            // step id must be unique
            if !seen_ids.insert(&step.id) {
                return Err(PlanValidationError::DuplicateStepId(step.id.clone()));
            }

            // Critical risk must require ApprovalMode::Ask or ApprovalMode::AlwaysDeny
            if self.risk == RiskLevel::Critical {
                if self.approval != ApprovalMode::Ask && self.approval != ApprovalMode::AlwaysDeny {
                    return Err(PlanValidationError::CriticalRiskRequiresApproval(self.risk));
                }
            }

            // FilesDelete, FilesWrite, FilesMove and ShellRunSandboxed must require approval
            if step.action.requires_approval() && !step.requires_approval {
                return Err(PlanValidationError::ActionRequiresApproval(
                    step.action.clone(),
                ));
            }

            // ShellRunSandboxed must be invalid if inputs.network == true unless inputs.sandbox == true
            if let ActionKind::ShellRunSandboxed = &step.action {
                let network = step
                    .inputs
                    .get("network")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let sandbox = step
                    .inputs
                    .get("sandbox")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if network && !sandbox {
                    return Err(PlanValidationError::ShellSandboxNetworkConflict);
                }
            }

            // Custom actions must require approval
            if let ActionKind::Custom(_) = &step.action {
                if !step.requires_approval {
                    return Err(PlanValidationError::ActionRequiresApproval(
                        step.action.clone(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Parses an OsaiPlan from YAML string.
    ///
    /// # Errors
    ///
    /// Returns `PlanParseError` if parsing fails.
    pub fn from_yaml(input: &str) -> Result<Self, PlanParseError> {
        serde_yaml::from_str(input).map_err(|e| PlanParseError::YamlError(e.to_string()))
    }

    /// Serializes this OsaiPlan to a YAML string.
    ///
    /// # Errors
    ///
    /// Returns `PlanSerializeError` if serialization fails.
    pub fn to_yaml(&self) -> Result<String, PlanSerializeError> {
        serde_yaml::to_string(self).map_err(|e| PlanSerializeError::YamlError(e.to_string()))
    }

    /// Parses an OsaiPlan from JSON string.
    ///
    /// # Errors
    ///
    /// Returns `PlanParseError` if parsing fails.
    pub fn from_json(input: &str) -> Result<Self, PlanParseError> {
        serde_json::from_str(input).map_err(|e| PlanParseError::JsonError(e.to_string()))
    }

    /// Serializes this OsaiPlan to a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns `PlanSerializeError` if serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, PlanSerializeError> {
        serde_json::to_string_pretty(self).map_err(|e| PlanSerializeError::JsonError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn create_basic_plan() -> OsaiPlan {
        let mut inputs = BTreeMap::new();
        inputs.insert("path".to_string(), serde_json::json!("/home/user/project"));

        OsaiPlan {
            version: "0.1".to_string(),
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            title: "Create project directory".to_string(),
            description: Some("Creates a new project directory".to_string()),
            actor: "osai-agent".to_string(),
            risk: RiskLevel::Medium,
            approval: ApprovalMode::Ask,
            steps: vec![PlanStep {
                id: "step-1".to_string(),
                action: ActionKind::FilesWrite,
                description: "Create the project directory".to_string(),
                requires_approval: true,
                inputs,
            }],
            rollback: None,
            metadata: BTreeMap::new(),
        }
    }

    // Successful parse/validate tests

    #[test]
    fn test_valid_plan_from_yaml() {
        let yaml = r#"
version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: "Create project directory"
actor: "osai-agent"
risk: Medium
approval: Ask
steps:
  - id: "step-1"
    action:
      type: FilesWrite
    description: "Create the project directory"
    requires_approval: true
    inputs:
      path: "/home/user/project"
metadata: {}
"#;
        let plan = OsaiPlan::from_yaml(yaml).unwrap();
        assert_eq!(plan.version, "0.1");
        assert_eq!(plan.title, "Create project directory");
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn test_valid_plan_from_json() {
        let json = r#"{
  "version": "0.1",
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "Create project directory",
  "actor": "osai-agent",
  "risk": "Medium",
  "approval": "Ask",
  "steps": [
    {
      "id": "step-1",
      "action": {"type": "FilesWrite"},
      "description": "Create the project directory",
      "requires_approval": true,
      "inputs": {"path": "/home/user/project"}
    }
  ],
  "metadata": {}
}"#;
        let plan = OsaiPlan::from_json(json).unwrap();
        assert_eq!(plan.version, "0.1");
        assert_eq!(plan.title, "Create project directory");
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn test_validate_valid_plan() {
        let plan = create_basic_plan();
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_roundtrip_yaml() {
        let plan = create_basic_plan();
        let yaml = plan.to_yaml().unwrap();
        let parsed = OsaiPlan::from_yaml(&yaml).unwrap();
        assert_eq!(parsed.title, plan.title);
        assert_eq!(parsed.version, plan.version);
    }

    #[test]
    fn test_roundtrip_json() {
        let plan = create_basic_plan();
        let json = plan.to_json_pretty().unwrap();
        let parsed = OsaiPlan::from_json(&json).unwrap();
        assert_eq!(parsed.title, plan.title);
        assert_eq!(parsed.version, plan.version);
    }

    // Failed validation tests

    #[test]
    fn test_empty_version() {
        let mut plan = create_basic_plan();
        plan.version = "".to_string();
        let err = plan.validate().unwrap_err();
        assert!(matches!(err, PlanValidationError::EmptyVersion));
    }

    #[test]
    fn test_empty_title() {
        let mut plan = create_basic_plan();
        plan.title = "".to_string();
        let err = plan.validate().unwrap_err();
        assert!(matches!(err, PlanValidationError::EmptyTitle));
    }

    #[test]
    fn test_empty_actor() {
        let mut plan = create_basic_plan();
        plan.actor = "".to_string();
        let err = plan.validate().unwrap_err();
        assert!(matches!(err, PlanValidationError::EmptyActor));
    }

    #[test]
    fn test_empty_steps() {
        let mut plan = create_basic_plan();
        plan.steps = vec![];
        let err = plan.validate().unwrap_err();
        assert!(matches!(err, PlanValidationError::EmptySteps));
    }

    #[test]
    fn test_empty_step_id() {
        let mut plan = create_basic_plan();
        plan.steps[0].id = "".to_string();
        let err = plan.validate().unwrap_err();
        assert!(matches!(err, PlanValidationError::EmptyStepId(_)));
    }

    #[test]
    fn test_duplicate_step_id() {
        let mut plan = create_basic_plan();
        plan.steps.push(PlanStep {
            id: "step-1".to_string(),
            action: ActionKind::FilesRead,
            description: "Another step".to_string(),
            requires_approval: true,
            inputs: BTreeMap::new(),
        });
        let err = plan.validate().unwrap_err();
        assert!(matches!(err, PlanValidationError::DuplicateStepId(_)));
    }

    #[test]
    fn test_critical_risk_requires_approval() {
        let mut plan = create_basic_plan();
        plan.risk = RiskLevel::Critical;
        plan.approval = ApprovalMode::Auto;
        let err = plan.validate().unwrap_err();
        assert!(matches!(
            err,
            PlanValidationError::CriticalRiskRequiresApproval(RiskLevel::Critical)
        ));
    }

    #[test]
    fn test_action_requires_approval() {
        let mut plan = create_basic_plan();
        plan.steps[0].action = ActionKind::FilesDelete;
        plan.steps[0].requires_approval = false;
        let err = plan.validate().unwrap_err();
        assert!(matches!(
            err,
            PlanValidationError::ActionRequiresApproval(_)
        ));
    }

    #[test]
    fn test_shell_sandbox_network_conflict() {
        let mut inputs = BTreeMap::new();
        inputs.insert("network".to_string(), serde_json::json!(true));
        inputs.insert("sandbox".to_string(), serde_json::json!(false));

        let mut plan = create_basic_plan();
        plan.steps[0].action = ActionKind::ShellRunSandboxed;
        plan.steps[0].inputs = inputs;

        let err = plan.validate().unwrap_err();
        assert!(matches!(
            err,
            PlanValidationError::ShellSandboxNetworkConflict
        ));
    }

    #[test]
    fn test_shell_sandbox_with_network_and_sandbox_ok() {
        let mut inputs = BTreeMap::new();
        inputs.insert("network".to_string(), serde_json::json!(true));
        inputs.insert("sandbox".to_string(), serde_json::json!(true));

        let mut plan = create_basic_plan();
        plan.steps[0].action = ActionKind::ShellRunSandboxed;
        plan.steps[0].inputs = inputs;

        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_custom_action_requires_approval() {
        let mut plan = create_basic_plan();
        plan.steps[0].action = ActionKind::Custom("my_custom_action".to_string());
        plan.steps[0].requires_approval = false;
        let err = plan.validate().unwrap_err();
        assert!(matches!(
            err,
            PlanValidationError::ActionRequiresApproval(_)
        ));
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let yaml = "invalid: yaml: content:";
        let err = OsaiPlan::from_yaml(yaml).unwrap_err();
        assert!(matches!(err, PlanParseError::YamlError(_)));
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not valid json";
        let err = OsaiPlan::from_json(json).unwrap_err();
        assert!(matches!(err, PlanParseError::JsonError(_)));
    }

    #[test]
    fn test_action_kind_requires_approval() {
        assert!(ActionKind::FilesDelete.requires_approval());
        assert!(ActionKind::FilesWrite.requires_approval());
        assert!(ActionKind::FilesMove.requires_approval());
        assert!(ActionKind::ShellRunSandboxed.requires_approval());
        assert!(ActionKind::Custom("test".to_string()).requires_approval());
        assert!(!ActionKind::FilesList.requires_approval());
        assert!(!ActionKind::FilesRead.requires_approval());
    }

    #[test]
    fn test_action_kind_is_destructive() {
        assert!(ActionKind::FilesDelete.is_destructive());
        assert!(ActionKind::FilesWrite.is_destructive());
        assert!(ActionKind::FilesMove.is_destructive());
        assert!(!ActionKind::FilesList.is_destructive());
        assert!(!ActionKind::ShellRunSandboxed.is_destructive());
    }

    #[test]
    fn test_new_plan_generates_uuid() {
        let plan = OsaiPlan::new(
            "Test plan".to_string(),
            "test-actor".to_string(),
            RiskLevel::Low,
            ApprovalMode::Auto,
        );
        assert_ne!(plan.id, Uuid::nil());
    }
}
