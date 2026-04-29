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

// ============================================================================
// Authorization Preview (no execution)
// ============================================================================

/// Checks if a path is within any of the allowed roots.
/// Expands ~ and resolves relative paths before comparison.
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

    let canonical_path = match std::fs::canonicalize(&expanded_path) {
        Ok(p) => p,
        Err(_) => return false,
    };

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

        let canonical_root = match std::fs::canonicalize(&expanded_root) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if canonical_path.starts_with(&canonical_root) {
            return true;
        }
    }

    false
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
) -> Result<()> {
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

    if !dry_run {
        println!("Receipt written: {}", receipt_id);
    }

    Ok(())
}

/// Runs an apply operation - validates, authorizes, and executes an OSAI plan.
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

    // Validate plan
    if let Err(e) = plan.validate() {
        write_apply_receipt(
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

    // Authorize each step and collect decisions
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
        decisions.push(StepDecision {
            step_id: step.id.clone(),
            action_name: action_name.clone(),
            allowed: decision.allowed,
            requires_approval: decision.requires_user_approval,
            policy_mode: decision.policy_mode,
            reason: decision.reason,
        });

        if !decision.allowed {
            denied_count += 1;
        }
        if decision.requires_user_approval {
            approval_required_count += 1;
        }
    }

    // Print authorization summary
    if json {
        #[derive(Serialize)]
        struct ApplyAuthorizationSummary {
            plan_id: String,
            plan_path: String,
            policy_path: String,
            dry_run: bool,
            steps: Vec<StepAuthorizationSummary>,
            denied_count: u32,
            approval_required_count: u32,
        }
        #[derive(Serialize)]
        struct StepAuthorizationSummary {
            step_id: String,
            action: String,
            allowed: bool,
            approval: bool,
            mode: String,
        }
        let step_summaries: Vec<StepAuthorizationSummary> = decisions
            .iter()
            .map(|d| StepAuthorizationSummary {
                step_id: d.step_id.clone(),
                action: d.action_name.clone(),
                allowed: d.allowed,
                approval: d.requires_approval,
                mode: format!("{:?}", d.policy_mode),
            })
            .collect();
        let summary = ApplyAuthorizationSummary {
            plan_id: plan.id.to_string(),
            plan_path: plan_path.display().to_string(),
            policy_path: policy_path.display().to_string(),
            dry_run,
            steps: step_summaries,
            denied_count,
            approval_required_count,
        };
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    } else {
        println!("Authorization summary for plan: {}", plan_path.display());
        println!("policy: {}", policy_path.display());
        for d in &decisions {
            println!(
                "step={} action={} allowed={} approval={} mode={:?}",
                d.step_id, d.action_name, d.allowed, d.requires_approval, d.policy_mode
            );
        }
        println!(
            "denied={} approval_required={}",
            denied_count, approval_required_count
        );
    }

    // Dry run: write receipt and exit
    if dry_run {
        write_apply_receipt(
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
        println!("Dry run complete; no steps executed.");
        return Ok(());
    }

    // Build ToolExecutor with optional model router URL
    let mut executor = ToolExecutor::new(store, allowed_root.to_vec());
    if let Some(url) = model_router_url {
        executor = executor
            .with_model_router_url(url)
            .map_err(|e| anyhow::anyhow!("invalid model router URL: {}", e))?;
    }

    // Execute steps
    let mut executed_count = 0u32;
    let mut skipped_count = 0u32;
    let mut failed_count = 0u32;
    let approve_set: std::collections::HashSet<&str> = approve.iter().map(|s| s.as_str()).collect();

    for (i, step) in plan.steps.iter().enumerate() {
        let decision = &decisions[i];

        if !decision.allowed {
            if json {
                println!(
                    "{{\"step\":\"{}\",\"action\":\"{}\",\"status\":\"Denied\"}}",
                    decision.step_id, decision.action_name
                );
            } else {
                println!(
                    "step={} action={} execution=Denied reason=\"Policy denied\"",
                    decision.step_id, decision.action_name
                );
            }
            skipped_count += 1;
            continue;
        }

        if decision.requires_approval {
            let is_approved = approve_all || approve_set.contains(decision.step_id.as_str());
            if !is_approved {
                if json {
                    println!(
                        "{{\"step\":\"{}\",\"action\":\"{}\",\"status\":\"Skipped\",\"reason\":\"requires approval\"}}",
                        decision.step_id, decision.action_name
                    );
                } else {
                    println!(
                        "step={} action={} execution=Skipped reason=\"requires user approval\"",
                        decision.step_id, decision.action_name
                    );
                }
                skipped_count += 1;
                continue;
            }
            // Explicitly approved
            let adjusted_decision = osai_toolbroker::AuthorizationDecision {
                allowed: true,
                requires_user_approval: false,
                reason: format!("Explicitly approved by CLI: {}", decision.reason),
                policy_mode: decision.policy_mode,
                request_id: uuid::Uuid::new_v4(),
            };

            let request = step_to_request(&plan, step);
            let result = executor
                .execute_authorized(&request, &adjusted_decision)
                .with_context(|| format!("execution failed for step: {}", step.id))?;

            let exec_status = match result.status {
                ExecutionStatus::Executed => "Executed",
                ExecutionStatus::Failed => "Failed",
                ExecutionStatus::Skipped => "Skipped",
            };
            if json {
                println!(
                    "{{\"step\":\"{}\",\"action\":\"{}\",\"status\":\"{}\",\"error\":\"{}\"}}",
                    decision.step_id,
                    decision.action_name,
                    exec_status,
                    result.error.unwrap_or_default()
                );
            } else {
                println!(
                    "step={} action={} execution={} error=\"{}\"",
                    decision.step_id,
                    decision.action_name,
                    exec_status,
                    result.error.unwrap_or_default()
                );
            }

            if result.status == ExecutionStatus::Executed {
                executed_count += 1;
                approved_step_ids.push(decision.step_id.clone());
            } else if result.status == ExecutionStatus::Failed {
                failed_count += 1;
            } else {
                skipped_count += 1;
            }
            continue;
        }

        // Execute auto-approved step
        let request = step_to_request(&plan, step);
        let auto_decision = osai_toolbroker::AuthorizationDecision {
            allowed: decision.allowed,
            requires_user_approval: decision.requires_approval,
            reason: decision.reason.clone(),
            policy_mode: decision.policy_mode,
            request_id: uuid::Uuid::new_v4(),
        };
        let result = executor
            .execute_authorized(&request, &auto_decision)
            .with_context(|| format!("execution failed for step: {}", step.id))?;

        let exec_status = match result.status {
            ExecutionStatus::Executed => "Executed",
            ExecutionStatus::Failed => "Failed",
            ExecutionStatus::Skipped => "Skipped",
        };
        if json {
            println!(
                "{{\"step\":\"{}\",\"action\":\"{}\",\"status\":\"{}\",\"error\":\"{}\"}}",
                decision.step_id,
                decision.action_name,
                exec_status,
                result.error.unwrap_or_default()
            );
        } else {
            println!(
                "step={} action={} execution={} error=\"{}\"",
                decision.step_id,
                decision.action_name,
                exec_status,
                result.error.unwrap_or_default()
            );
        }

        if result.status == ExecutionStatus::Executed {
            executed_count += 1;
        } else if result.status == ExecutionStatus::Failed {
            failed_count += 1;
        } else {
            skipped_count += 1;
        }
    }

    // Write receipt
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
    let error: Option<&str> = error_msg.as_deref();
    write_apply_receipt(
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
        error,
    )?;

    // Print summary
    if json {
        #[derive(Serialize)]
        struct ApplySummary {
            status: String,
            executed: u32,
            skipped: u32,
            denied: u32,
            approval_required: u32,
            failed: u32,
            approved_steps: Vec<String>,
            dry_run: bool,
        }
        let summary = ApplySummary {
            status: status.to_string(),
            executed: executed_count,
            skipped: skipped_count,
            denied: denied_count,
            approval_required: approval_required_count,
            failed: failed_count,
            approved_steps: approved_step_ids,
            dry_run,
        };
        println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    } else {
        println!(
            "Apply complete: executed={} skipped={} denied={} approval_required={} failed={}",
            executed_count, skipped_count, denied_count, approval_required_count, failed_count
        );
    }

    if failed_count > 0 {
        Err(anyhow::anyhow!("{} step(s) failed", failed_count))
    } else {
        Ok(())
    }
}
