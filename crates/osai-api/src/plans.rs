//! Plans endpoints - /v1/plans

use anyhow::Result;
use osai_plan_dsl::OsaiPlan;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Plans list query parameters.
#[derive(Debug, Deserialize)]
pub struct PlansQuery {
    pub limit: Option<usize>,
    pub dir: Option<String>,
}

/// Plan summary entry.
#[derive(Debug, Serialize)]
pub struct PlanSummary {
    pub path: String,
    pub filename: String,
    pub modified_unix: u64,
    pub size_bytes: u64,
    pub title: String,
    pub risk: String,
    pub approval: String,
    pub steps: usize,
    pub valid: bool,
    pub error: Option<String>,
}

/// Plans list response.
#[derive(Debug, Serialize)]
pub struct PlansListResponse {
    pub ok: bool,
    pub plans_dir: String,
    pub plans: Vec<PlanSummary>,
}

/// Plan read query parameters.
#[derive(Debug, Deserialize)]
pub struct PlanReadQuery {
    pub path: String,
}

/// Plan read response.
#[derive(Debug, Serialize)]
pub struct PlanReadResponse {
    pub ok: bool,
    pub path: String,
    pub plan: serde_json::Value,
    pub raw: String,
}

/// Default plans directory - uses XDG for persistence.
/// Must match the directory used by ask_core_async so output_path
/// from /v1/ask can be used with /v1/plans/read and other endpoints.
pub fn default_plans_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("osai")
        .join("plans")
}

/// Checks if a path is within a parent directory (for path safety).
/// Returns true if path is a descendant of parent_dir (resolved canonically).
fn is_path_under_dir(path: &Path, parent_dir: &Path) -> bool {
    // If path is absolute, check if it falls within parent_dir after canonicalization
    if path.is_absolute() {
        let canonical_path = match std::fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let canonical_parent = match std::fs::canonicalize(parent_dir) {
            Ok(p) => p,
            Err(_) => return false,
        };
        return canonical_path.starts_with(&canonical_parent);
    }

    // For relative paths, canonicalize both
    let canonical_path = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let canonical_parent = match std::fs::canonicalize(parent_dir) {
        Ok(p) => p,
        Err(_) => return false,
    };
    canonical_path.starts_with(&canonical_parent)
}

/// List plans in a directory.
pub fn list_plans(dir: &Path, limit: usize) -> Result<PlansListResponse> {
    let plans_dir = dir.display().to_string();

    if !dir.exists() {
        return Ok(PlansListResponse {
            ok: true,
            plans_dir,
            plans: vec![],
        });
    }

    let mut entries: Vec<PlanSummary> = vec![];

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only include plan files
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("yml") && ext != Some("yaml") && ext != Some("json") {
            continue;
        }

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let modified_unix = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let size_bytes = metadata.len();

        // Try to parse the plan
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                entries.push(PlanSummary {
                    path: path.display().to_string(),
                    filename,
                    modified_unix,
                    size_bytes,
                    title: String::new(),
                    risk: String::new(),
                    approval: String::new(),
                    steps: 0,
                    valid: false,
                    error: Some("failed to read file".to_string()),
                });
                continue;
            }
        };

        match OsaiPlan::from_yaml(&content).or_else(|_| OsaiPlan::from_json(&content)) {
            Ok(plan) => {
                entries.push(PlanSummary {
                    path: path.display().to_string(),
                    filename,
                    modified_unix,
                    size_bytes,
                    title: plan.title,
                    risk: format!("{:?}", plan.risk),
                    approval: format!("{:?}", plan.approval),
                    steps: plan.steps.len(),
                    valid: true,
                    error: None,
                });
            }
            Err(e) => {
                entries.push(PlanSummary {
                    path: path.display().to_string(),
                    filename,
                    modified_unix,
                    size_bytes,
                    title: String::new(),
                    risk: String::new(),
                    approval: String::new(),
                    steps: 0,
                    valid: false,
                    error: Some(format!("parse error: {}", e)),
                });
            }
        }
    }

    // Sort newest first
    entries.sort_by(|a, b| b.modified_unix.cmp(&a.modified_unix));

    // Apply limit
    entries.truncate(limit);

    Ok(PlansListResponse {
        ok: true,
        plans_dir,
        plans: entries,
    })
}

/// Read and parse a single plan with path safety check.
pub fn read_plan(path_str: &str, plans_dir: &Path) -> Result<PlanReadResponse> {
    if path_str.trim().is_empty() {
        return Err(anyhow::anyhow!("path is required"));
    }

    let path = PathBuf::from(path_str);

    if !path.exists() {
        return Err(anyhow::anyhow!("plan file not found: {}", path_str));
    }

    // Path safety: ensure the path is within the plans directory
    if !is_path_under_dir(&path, plans_dir) {
        return Err(anyhow::anyhow!(
            "path is outside allowed plans directory: {}",
            path_str
        ));
    }

    let content = fs::read_to_string(&path)?;
    let raw = content.clone();

    let plan = OsaiPlan::from_yaml(&content).or_else(|_| OsaiPlan::from_json(&content))?;

    let plan_json = serde_json::to_value(&plan)?;

    Ok(PlanReadResponse {
        ok: true,
        path: path.display().to_string(),
        plan: plan_json,
        raw,
    })
}
