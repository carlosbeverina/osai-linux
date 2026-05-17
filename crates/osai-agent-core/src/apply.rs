//! Apply operation - validates, authorizes, and executes an OSAI plan end-to-end.

use crate::shared::{default_apply_receipts_dir, step_to_request};
use anyhow::{Context, Result};
use osai_plan_dsl::OsaiPlan;
use osai_receipt_logger::{Receipt, ReceiptStatus, ReceiptStore};
use osai_tool_executor::{ExecutionStatus, ToolExecutor};
use osai_toolbroker::{ToolBroker, ToolPolicy};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================================
// Public Types
// ============================================================================

/// Result of an apply operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub status: String,
    pub executed: u32,
    pub skipped: u32,
    pub denied: u32,
    pub approval_required: u32,
    pub failed: u32,
    pub approved_steps: Vec<String>,
    pub dry_run: bool,
    pub error: Option<String>,
}

/// Authorization preview for a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPreview {
    pub id: String,
    pub action: String,
    pub allowed: bool,
    pub approval_required: bool,
    pub mode: String,
    pub reason: Option<String>,
}

/// Authorization preview result for a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizePreviewResult {
    pub ok: bool,
    pub plan_id: String,
    pub summary: AuthorizeSummary,
    pub steps: Vec<StepPreview>,
    pub error: Option<String>,
}

/// Summary counts for authorization preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeSummary {
    pub allowed: u32,
    pub denied: u32,
    pub approval_required: u32,
}

/// Authorization summary emitted before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyAuthorizationReport {
    pub plan_id: String,
    pub plan_path: String,
    pub policy_path: String,
    pub dry_run: bool,
    pub steps: Vec<ApplyStepAuthorization>,
    pub denied_count: u32,
    pub approval_required_count: u32,
}

/// Authorization details for one apply step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyStepAuthorization {
    pub step_id: String,
    pub action: String,
    pub allowed: bool,
    pub approval: bool,
    pub mode: String,
}

/// Execution status emitted for one apply step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyStepExecution {
    pub step_id: String,
    pub action: String,
    pub status: String,
    pub error: Option<String>,
    pub denied: bool,
    pub approval_skipped: bool,
    pub approved_by_cli: bool,
}

/// Complete core apply output. Contains data only; callers own presentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCoreOutput {
    pub authorization: ApplyAuthorizationReport,
    pub executions: Vec<ApplyStepExecution>,
    pub result: ApplyResult,
    pub receipt_id: Option<String>,
}

// ============================================================================
// Authorization Preview (no execution)
// ============================================================================

/// Checks if a path is within any of the allowed roots.
/// Expands ~ and resolves relative paths before comparison.
/// Does NOT require the path to exist — uses prefix comparison when canonicalize fails.
fn is_path_in_allowed_roots(path: &Path, allowed_roots: &[PathBuf]) -> bool {
    if allowed_roots.is_empty() {
        return true; // No restriction
    }

    let path_str = path.to_string_lossy();
    let expanded_path: PathBuf = if path_str.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(path_str.trim_start_matches("~/"))
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    // Try canonicalize first (resolves symlinks, normalizes path). If it fails because
    // the path doesn't exist yet, fall back to prefix-based comparison.
    let path_is_dir = std::path::Path::new(&expanded_path).exists();
    let canonical_path = std::fs::canonicalize(&expanded_path).ok();

    for root in allowed_roots {
        let root_str = root.to_string_lossy();
        let expanded_root: PathBuf = if root_str.starts_with("~/") {
            if let Ok(home) = std::env::var("HOME") {
                PathBuf::from(home).join(root_str.trim_start_matches("~/"))
            } else {
                root.clone()
            }
        } else {
            root.clone()
        };

        // Canonicalize the root when possible. If the root does not exist yet,
        // fall back to normalized prefix comparison below so callers can allow
        // conventional paths like ~/Downloads in clean test/dev environments.
        let canonical_root = std::fs::canonicalize(&expanded_root).ok();

        // If we have a canonical path (path exists), do precise comparison when
        // the root also canonicalized.
        if let (Some(cp), Some(root)) = (&canonical_path, &canonical_root) {
            if cp.starts_with(root) {
                return true;
            }
            continue;
        }

        // Path doesn't exist — use prefix-based check after normalizing away ..
        // This allows ~/Downloads to be approved even if the directory hasn't been created yet
        if path_is_dir {
            // Path is marked as a directory but canonicalize failed — strange edge case,
            // conservatively deny
            continue;
        }

        // Normalize the path by resolving .. components and comparing prefixes
        let normalized = normalize_path_for_prefix_check(&expanded_path);

        // Normalize the root for comparison too
        let normalized_root = normalize_path_for_prefix_check(&expanded_root);
        let canonical_root_str = canonical_root
            .as_ref()
            .map(|root| root.to_string_lossy().to_string());

        if normalized.starts_with(&normalized_root)
            || canonical_root_str
                .as_deref()
                .map(|root| normalized.starts_with(root))
                .unwrap_or(false)
        {
            return true;
        }
    }

    false
}

/// Normalizes a path for prefix-based comparison by resolving .. components.
/// Does NOT require the path to exist. Used only when canonicalize fails.
/// Handles paths like ~/Downloads/../other but does NOT allow bypass via ..
fn normalize_path_for_prefix_check(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|p| p.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };

    let mut normalized = PathBuf::new();
    for component in abs.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(s) => {
                normalized.push(s);
            }
            other => normalized.push(other.as_os_str()),
        }
    }

    normalized.to_string_lossy().replace('\\', "/")
}

/// Authorize a plan without executing it. Returns preview of authorization decisions.
pub fn authorize_plan_preview(
    plan_path: &PathBuf,
    policy_path: &PathBuf,
    allowed_roots: &[PathBuf],
    approve: &[String],
    approve_all: bool,
) -> Result<AuthorizePreviewResult> {
    // Read and parse plan
    let plan_content = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan file: {}", plan_path.display()))?;
    let plan = match OsaiPlan::from_yaml(&plan_content) {
        Ok(p) => p,
        Err(_) => OsaiPlan::from_json(&plan_content)
            .with_context(|| format!("failed to parse plan file: {}", plan_path.display()))?,
    };

    // Validate plan
    if let Err(e) = plan.validate() {
        return Err(anyhow::anyhow!("plan validation failed: {}", e));
    }

    // Read and parse policy
    let policy_content = fs::read_to_string(policy_path)
        .with_context(|| format!("failed to read policy file: {}", policy_path.display()))?;
    let policy = ToolPolicy::from_yaml(&policy_content)
        .map_err(|e| anyhow::anyhow!("policy parse failed: {}", e))?;

    // Create store and broker (no receipts needed for preview)
    let receipts_dir = default_apply_receipts_dir();
    let store = ReceiptStore::new(&receipts_dir);
    store.ensure_dirs().ok();
    let broker = ToolBroker::new(policy.clone(), store);

    // Authorize each step and collect decisions
    let mut denied_count = 0u32;
    let mut approval_required_count = 0u32;
    let mut allowed_count = 0u32;
    let mut step_previews = Vec::new();
    let approve_set: std::collections::HashSet<&str> = approve.iter().map(|s| s.as_str()).collect();

    for step in &plan.steps {
        let request = step_to_request(&plan, step);
        let decision = broker
            .authorize(&request)
            .with_context(|| format!("authorization failed for step: {}", step.id))?;

        let action_name = request.action_name();

        // For filesystem actions, also check allowed_roots
        let path_in_allowed = match &step.action {
            osai_plan_dsl::ActionKind::FilesList
            | osai_plan_dsl::ActionKind::FilesRead
            | osai_plan_dsl::ActionKind::FilesWrite
            | osai_plan_dsl::ActionKind::FilesMove
            | osai_plan_dsl::ActionKind::FilesDelete => {
                let path_str = step
                    .inputs
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let path = PathBuf::from(path_str);
                is_path_in_allowed_roots(&path, allowed_roots)
            }
            _ => true,
        };

        // Denied steps cannot be overridden by approve flags
        if !decision.allowed || !path_in_allowed {
            denied_count += 1;
            let reason = if !path_in_allowed {
                format!(
                    "Path {}{} is outside allowed roots",
                    step.inputs
                        .get("path")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    if decision.allowed {
                        format!("; ToolBroker reason: {}", decision.reason)
                    } else {
                        String::new()
                    }
                )
            } else {
                decision.reason
            };
            step_previews.push(StepPreview {
                id: step.id.clone(),
                action: action_name,
                allowed: false,
                approval_required: false,
                mode: format!("{:?}", decision.policy_mode),
                reason: Some(reason),
            });
            continue;
        }

        // Approval-required steps
        if decision.requires_user_approval {
            let is_explicitly_approved = approve_all || approve_set.contains(step.id.as_str());
            approval_required_count += 1;
            step_previews.push(StepPreview {
                id: step.id.clone(),
                action: action_name,
                allowed: true,
                approval_required: true,
                mode: format!("{:?}", decision.policy_mode),
                reason: if is_explicitly_approved {
                    Some(format!("Explicitly approved via CLI: {}", decision.reason))
                } else {
                    Some(decision.reason)
                },
            });
        } else {
            allowed_count += 1;
            step_previews.push(StepPreview {
                id: step.id.clone(),
                action: action_name,
                allowed: true,
                approval_required: false,
                mode: format!("{:?}", decision.policy_mode),
                reason: Some(decision.reason),
            });
        }
    }

    Ok(AuthorizePreviewResult {
        ok: true,
        plan_id: plan.id.to_string(),
        summary: AuthorizeSummary {
            allowed: allowed_count,
            denied: denied_count,
            approval_required: approval_required_count,
        },
        steps: step_previews,
        error: None,
    })
}

// ============================================================================
// Receipt Writing
// ============================================================================

fn write_apply_receipt(
    receipts_dir: &PathBuf,
    plan: &OsaiPlan,
    plan_path: &PathBuf,
    policy_path: &PathBuf,
    approved_steps: Option<&[String]>,
    dry_run: bool,
    approve: &[String],
    approve_all: bool,
    executed_count: u32,
    skipped_count: u32,
    denied_count: u32,
    approval_required_count: u32,
    failed_count: u32,
    status: &str,
    error: Option<&str>,
) -> Result<uuid::Uuid> {
    let store = ReceiptStore::new(receipts_dir);
    store
        .ensure_dirs()
        .map_err(|e| anyhow::anyhow!("failed to create receipts directory: {}", e))?;

    let receipt_status = if status == "Executed" {
        ReceiptStatus::Executed
    } else {
        ReceiptStatus::Failed
    };

    let mut receipt = Receipt::new("osai-agent", "PlanApply")
        .with_tool("osai-agent apply")
        .with_risk("Low")
        .with_approval("Auto");

    receipt.status = receipt_status;
    receipt.outputs_redacted = Some(serde_json::json!({
        "plan_id": plan.id.to_string(),
        "plan_path": plan_path.display().to_string(),
        "policy_path": policy_path.display().to_string(),
        "dry_run": dry_run,
        "executed_count": executed_count,
        "skipped_count": skipped_count,
        "denied_count": denied_count,
        "approval_required_count": approval_required_count,
        "failed_count": failed_count,
        "approved_steps": approved_steps.map(|a| a.to_vec()).unwrap_or_default(),
        "approve_all": approve_all,
        "approve_flags": approve.to_vec(),
    }));

    if let Some(err) = error {
        receipt.error = Some(err.to_string());
    }

    let receipt_id = receipt.id;
    store
        .write(&receipt)
        .map_err(|e| anyhow::anyhow!("failed to write receipt: {}", e))?;

    Ok(receipt_id)
}

/// Runs the core apply operation without printing. This is the reusable business
/// logic used by CLI/API callers; presentation belongs to the caller.
pub fn run_apply_core(
    plan_path: &PathBuf,
    policy_path: &PathBuf,
    receipts_dir_override: Option<&Path>,
    allowed_root: &[PathBuf],
    approve: &[String],
    approve_all: bool,
    model_router_url: Option<&str>,
    dry_run: bool,
) -> Result<ApplyCoreOutput> {
    // Resolve receipts directory
    let receipts_dir = receipts_dir_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| default_apply_receipts_dir());

    // Read and parse plan
    let plan_content = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan file: {}", plan_path.display()))?;
    let plan = match OsaiPlan::from_yaml(&plan_content) {
        Ok(p) => p,
        Err(_) => OsaiPlan::from_json(&plan_content)
            .with_context(|| format!("failed to parse plan file: {}", plan_path.display()))?,
    };

    // Validate plan before any authorization/execution.
    if let Err(e) = plan.validate() {
        let _receipt_id = write_apply_receipt(
            &receipts_dir,
            &plan,
            plan_path,
            policy_path,
            None,
            dry_run,
            approve,
            approve_all,
            0,
            0,
            0,
            0,
            0,
            "Failed",
            Some(&format!("Plan validation failed: {}", e)),
        )?;
        return Err(anyhow::anyhow!("plan validation failed: {}", e));
    }

    // Read and parse policy
    let policy_content = fs::read_to_string(policy_path)
        .with_context(|| format!("failed to read policy file: {}", policy_path.display()))?;
    let policy = ToolPolicy::from_yaml(&policy_content)
        .map_err(|e| anyhow::anyhow!("policy parse failed: {}", e))?;

    // Create store and broker
    let store = ReceiptStore::new(&receipts_dir);
    store.ensure_dirs().with_context(|| {
        format!(
            "failed to create receipts directory: {}",
            receipts_dir.display()
        )
    })?;
    let broker = ToolBroker::new(policy.clone(), store.clone());

    // Authorize each step and collect decisions. ToolBroker remains the
    // authorization boundary; execution below only uses authorized decisions.
    let mut denied_count = 0u32;
    let mut approval_required_count = 0u32;
    let mut approved_step_ids = Vec::new();

    struct StepDecision {
        step_id: String,
        action_name: String,
        allowed: bool,
        requires_approval: bool,
        policy_mode: osai_toolbroker::PolicyMode,
        reason: String,
    }
    let mut decisions = Vec::new();

    for step in &plan.steps {
        let request = step_to_request(&plan, step);
        let decision = broker
            .authorize(&request)
            .with_context(|| format!("authorization failed for step: {}", step.id))?;

        let action_name = request.action_name();
        if !decision.allowed {
            denied_count += 1;
        }
        if decision.requires_user_approval {
            approval_required_count += 1;
        }
        decisions.push(StepDecision {
            step_id: step.id.clone(),
            action_name,
            allowed: decision.allowed,
            requires_approval: decision.requires_user_approval,
            policy_mode: decision.policy_mode,
            reason: decision.reason,
        });
    }

    let authorization = ApplyAuthorizationReport {
        plan_id: plan.id.to_string(),
        plan_path: plan_path.display().to_string(),
        policy_path: policy_path.display().to_string(),
        dry_run,
        steps: decisions
            .iter()
            .map(|d| ApplyStepAuthorization {
                step_id: d.step_id.clone(),
                action: d.action_name.clone(),
                allowed: d.allowed,
                approval: d.requires_approval,
                mode: format!("{:?}", d.policy_mode),
            })
            .collect(),
        denied_count,
        approval_required_count,
    };

    // Dry run: receipt only, no ToolExecutor invocation.
    if dry_run {
        let receipt_id = write_apply_receipt(
            &receipts_dir,
            &plan,
            plan_path,
            policy_path,
            None,
            dry_run,
            approve,
            approve_all,
            0,
            0,
            denied_count,
            approval_required_count,
            0,
            "Executed",
            None,
        )?;
        return Ok(ApplyCoreOutput {
            authorization,
            executions: Vec::new(),
            result: ApplyResult {
                status: "Executed".to_string(),
                executed: 0,
                skipped: 0,
                denied: denied_count,
                approval_required: approval_required_count,
                failed: 0,
                approved_steps: Vec::new(),
                dry_run,
                error: None,
            },
            receipt_id: Some(receipt_id.to_string()),
        });
    }

    // Build ToolExecutor with optional model router URL.
    let mut executor = ToolExecutor::new(store, allowed_root.to_vec());
    if let Some(url) = model_router_url {
        executor = executor
            .with_model_router_url(url)
            .map_err(|e| anyhow::anyhow!("invalid model router URL: {}", e))?;
    }

    let mut executed_count = 0u32;
    let mut skipped_count = 0u32;
    let mut failed_count = 0u32;
    let mut executions = Vec::new();
    let approve_set: std::collections::HashSet<&str> = approve.iter().map(|s| s.as_str()).collect();

    for (i, step) in plan.steps.iter().enumerate() {
        let decision = &decisions[i];

        if !decision.allowed {
            executions.push(ApplyStepExecution {
                step_id: decision.step_id.clone(),
                action: decision.action_name.clone(),
                status: "Denied".to_string(),
                error: Some("Policy denied".to_string()),
                denied: true,
                approval_skipped: false,
                approved_by_cli: false,
            });
            skipped_count += 1;
            continue;
        }

        let mut approved_by_cli = false;
        let auth_decision = if decision.requires_approval {
            let is_approved = approve_all || approve_set.contains(decision.step_id.as_str());
            if !is_approved {
                executions.push(ApplyStepExecution {
                    step_id: decision.step_id.clone(),
                    action: decision.action_name.clone(),
                    status: "Skipped".to_string(),
                    error: Some("requires user approval".to_string()),
                    denied: false,
                    approval_skipped: true,
                    approved_by_cli: false,
                });
                skipped_count += 1;
                continue;
            }
            approved_by_cli = true;
            osai_toolbroker::AuthorizationDecision {
                allowed: true,
                requires_user_approval: false,
                reason: format!("Explicitly approved by CLI: {}", decision.reason),
                policy_mode: decision.policy_mode,
                request_id: uuid::Uuid::new_v4(),
            }
        } else {
            osai_toolbroker::AuthorizationDecision {
                allowed: decision.allowed,
                requires_user_approval: decision.requires_approval,
                reason: decision.reason.clone(),
                policy_mode: decision.policy_mode,
                request_id: uuid::Uuid::new_v4(),
            }
        };

        let request = step_to_request(&plan, step);
        let result = executor
            .execute_authorized(&request, &auth_decision)
            .with_context(|| format!("execution failed for step: {}", step.id))?;

        let exec_status = match result.status {
            ExecutionStatus::Executed => "Executed",
            ExecutionStatus::Failed => "Failed",
            ExecutionStatus::Skipped => "Skipped",
        };
        let error = result.error.unwrap_or_default();
        executions.push(ApplyStepExecution {
            step_id: decision.step_id.clone(),
            action: decision.action_name.clone(),
            status: exec_status.to_string(),
            error: if error.is_empty() { None } else { Some(error) },
            denied: false,
            approval_skipped: false,
            approved_by_cli,
        });

        if result.status == ExecutionStatus::Executed {
            executed_count += 1;
            if approved_by_cli {
                approved_step_ids.push(decision.step_id.clone());
            }
        } else if result.status == ExecutionStatus::Failed {
            failed_count += 1;
        } else {
            skipped_count += 1;
        }
    }

    let status = if failed_count > 0 {
        "Failed"
    } else {
        "Executed"
    };
    let error_msg = if failed_count > 0 {
        Some(format!("{} step(s) failed", failed_count))
    } else {
        None
    };
    let receipt_id = write_apply_receipt(
        &receipts_dir,
        &plan,
        plan_path,
        policy_path,
        Some(&approved_step_ids),
        dry_run,
        approve,
        approve_all,
        executed_count,
        skipped_count,
        denied_count,
        approval_required_count,
        failed_count,
        status,
        error_msg.as_deref(),
    )?;

    let result = ApplyResult {
        status: status.to_string(),
        executed: executed_count,
        skipped: skipped_count,
        denied: denied_count,
        approval_required: approval_required_count,
        failed: failed_count,
        approved_steps: approved_step_ids,
        dry_run,
        error: error_msg.clone(),
    };

    let output = ApplyCoreOutput {
        authorization,
        executions,
        result,
        receipt_id: Some(receipt_id.to_string()),
    };

    Ok(output)
}

/// Runs an apply operation and prints the legacy CLI output.
pub fn run_apply(
    plan_path: &PathBuf,
    policy_path: &PathBuf,
    receipts_dir_override: Option<&Path>,
    allowed_root: &[PathBuf],
    approve: &[String],
    approve_all: bool,
    model_router_url: Option<&str>,
    dry_run: bool,
    json: bool,
) -> Result<()> {
    let output = run_apply_core(
        plan_path,
        policy_path,
        receipts_dir_override,
        allowed_root,
        approve,
        approve_all,
        model_router_url,
        dry_run,
    )?;

    print_apply_output(&output, json);

    if output.result.failed > 0 {
        Err(anyhow::anyhow!("{} step(s) failed", output.result.failed))
    } else {
        Ok(())
    }
}

/// Prints apply output in the historical CLI format.
pub fn print_apply_output(output: &ApplyCoreOutput, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&output.authorization).unwrap()
        );
    } else {
        println!(
            "Authorization summary for plan: {}",
            output.authorization.plan_path
        );
        println!("policy: {}", output.authorization.policy_path);
        for d in &output.authorization.steps {
            println!(
                "step={} action={} allowed={} approval={} mode={}",
                d.step_id, d.action, d.allowed, d.approval, d.mode
            );
        }
        println!(
            "denied={} approval_required={}",
            output.authorization.denied_count, output.authorization.approval_required_count
        );
    }

    if output.result.dry_run {
        println!("Dry run complete; no steps executed.");
        return;
    }

    for e in &output.executions {
        if json {
            if e.denied {
                println!(
                    "{{\"step\":\"{}\",\"action\":\"{}\",\"status\":\"Denied\"}}",
                    e.step_id, e.action
                );
            } else if e.approval_skipped {
                println!(
                    "{{\"step\":\"{}\",\"action\":\"{}\",\"status\":\"Skipped\",\"reason\":\"requires approval\"}}",
                    e.step_id, e.action
                );
            } else {
                println!(
                    "{{\"step\":\"{}\",\"action\":\"{}\",\"status\":\"{}\",\"error\":\"{}\"}}",
                    e.step_id,
                    e.action,
                    e.status,
                    e.error.clone().unwrap_or_default()
                );
            }
        } else if e.denied {
            println!(
                "step={} action={} execution=Denied reason=\"Policy denied\"",
                e.step_id, e.action
            );
        } else if e.approval_skipped {
            println!(
                "step={} action={} execution=Skipped reason=\"requires user approval\"",
                e.step_id, e.action
            );
        } else {
            println!(
                "step={} action={} execution={} error=\"{}\"",
                e.step_id,
                e.action,
                e.status,
                e.error.clone().unwrap_or_default()
            );
        }
    }

    if let Some(receipt_id) = &output.receipt_id {
        println!("Receipt written: {}", receipt_id);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&output.result).unwrap());
    } else {
        println!(
            "Apply complete: executed={} skipped={} denied={} approval_required={} failed={}",
            output.result.executed,
            output.result.skipped,
            output.result.denied,
            output.result.approval_required,
            output.result.failed
        );
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn unique_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("osai-core-{}-{}", name, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn with_test_home<T>(test: impl FnOnce(PathBuf) -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_home = std::env::var_os("HOME");
        let home = unique_test_dir("home");
        std::fs::create_dir_all(home.join("Downloads")).unwrap();
        std::env::set_var("HOME", &home);

        let result = test(home.clone());

        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        let _ = std::fs::remove_dir_all(home);
        result
    }

    fn write_failing_browser_plan(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
        let receipts_dir = dir.join("receipts");
        let plan_path = dir.join("failing-browser-plan.yml");
        let policy_path = dir.join("policy.yml");

        std::fs::write(
            &plan_path,
            r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440101"
title: "Open Unsupported Browser"
description: "Exercise a step-level executor failure"
actor: "user"
risk: Low
approval: Auto
steps:
  - id: "step-fails"
    action:
      type: BrowserOpenUrl
    description: "Attempt browser open"
    requires_approval: false
    inputs:
      url: "https://example.com"
metadata: {}"#,
        )
        .unwrap();

        std::fs::write(
            &policy_path,
            r#"version: "0.1"
default_mode: Allow
action_modes:
  BrowserOpenUrl: Allow
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true"#,
        )
        .unwrap();

        (plan_path, policy_path, receipts_dir)
    }

    #[test]
    fn test_run_apply_core_returns_output_for_step_execution_failure() {
        let dir = unique_test_dir("failed-step-output");
        let (plan_path, policy_path, receipts_dir) = write_failing_browser_plan(&dir);

        let output = run_apply_core(
            &plan_path,
            &policy_path,
            Some(receipts_dir.as_path()),
            &[],
            &[],
            false,
            None,
            false,
        )
        .expect("step-level execution failures should be represented in ApplyCoreOutput");

        assert_eq!(output.result.failed, 1);
        assert_eq!(output.result.status, "Failed");
        assert_eq!(output.result.error.as_deref(), Some("1 step(s) failed"));
        assert_eq!(output.executions.len(), 1);
        assert_eq!(output.executions[0].step_id, "step-fails");
        assert_eq!(output.executions[0].status, "Failed");
        assert!(output.executions[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("not executable"));
        assert!(output.receipt_id.is_some());

        let store = ReceiptStore::new(&receipts_dir);
        let receipts = store.list().unwrap();
        assert!(
            receipts.len() >= 2,
            "expected executor and apply receipts, got {}",
            receipts.len()
        );
    }

    #[test]
    fn test_run_apply_still_returns_err_for_step_execution_failure() {
        let dir = unique_test_dir("failed-step-wrapper");
        let (plan_path, policy_path, receipts_dir) = write_failing_browser_plan(&dir);

        let result = run_apply(
            &plan_path,
            &policy_path,
            Some(receipts_dir.as_path()),
            &[],
            &[],
            false,
            None,
            false,
            false,
        );

        let err =
            result.expect_err("legacy apply wrapper should still return Err for failed steps");
        assert!(err.to_string().contains("1 step(s) failed"));

        let store = ReceiptStore::new(&receipts_dir);
        let receipts = store.list().unwrap();
        assert!(
            receipts.len() >= 2,
            "expected executor and apply receipts, got {}",
            receipts.len()
        );
    }

    #[test]
    fn test_is_path_in_allowed_roots_expands_tilde() {
        with_test_home(|home| {
            // ~/Downloads with absolute allowed root
            let path = PathBuf::from("~/Downloads");
            let allowed = vec![home.join("Downloads")];
            assert!(
                is_path_in_allowed_roots(&path, &allowed),
                "~/Downloads should be allowed when $HOME/Downloads is allowed"
            );
        });
    }

    #[test]
    fn test_is_path_in_allowed_roots_tilde_in_allowed_root() {
        // ~/Downloads with tilde in allowed root
        let path = PathBuf::from("~/Downloads");
        let allowed = vec![PathBuf::from("~/Downloads")];
        assert!(
            is_path_in_allowed_roots(&path, &allowed),
            "~/Downloads should be allowed when ~/Downloads is allowed"
        );
    }

    #[test]
    fn test_is_path_in_allowed_roots_denies_etc() {
        with_test_home(|home| {
            let outside = unique_test_dir("outside");
            let allowed = vec![home.join("Downloads")];
            assert!(
                !is_path_in_allowed_roots(&outside, &allowed),
                "outside path should be denied when only $HOME/Downloads is allowed"
            );
            let _ = std::fs::remove_dir_all(outside);
        });
    }

    #[test]
    fn test_is_path_in_allowed_roots_dotdot_does_not_bypass() {
        with_test_home(|home| {
            // ~/Downloads/.. should not bypass to home
            let path = PathBuf::from("~/Downloads/..");
            let allowed = vec![home.join("Downloads")];
            // Path resolves to parent of Downloads which is $HOME, not inside Downloads
            assert!(
                !is_path_in_allowed_roots(&path, &allowed),
                "~/Downloads/.. should not be allowed inside ~/Downloads"
            );
        });
    }

    #[test]
    fn test_normalize_path_handles_parent_dir() {
        // Normalize away .. components
        let path = PathBuf::from("/home/user/Downloads/../Documents");
        let normalized = normalize_path_for_prefix_check(&path);
        assert!(
            normalized.ends_with("Documents"),
            "should resolve to Documents, got: {}",
            normalized
        );
        assert!(
            !normalized.contains(".."),
            "normalized path should not contain ..: {}",
            normalized
        );
    }

    #[test]
    fn test_normalize_path_preserves_absolute() {
        let path = std::env::temp_dir().join("osai-normalize-absolute");
        let normalized = normalize_path_for_prefix_check(&path);
        assert!(
            PathBuf::from(&normalized).is_absolute(),
            "absolute path should stay absolute: {}",
            normalized
        );
    }
}
