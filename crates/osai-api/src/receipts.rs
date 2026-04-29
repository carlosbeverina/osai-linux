//! Receipts endpoints - /v1/receipts

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Receipt summary entry.
#[derive(Debug, Serialize)]
pub struct ReceiptSummary {
    pub path: String,
    pub filename: String,
    pub modified_unix: u64,
    pub size_bytes: u64,
    pub kind: String,
    pub id: String,
    pub status: String,
    pub action: String,
    pub actor: String,
}

/// Receipt list query.
#[derive(Debug, Deserialize)]
pub struct ReceiptsQuery {
    pub limit: Option<usize>,
    pub dir: Option<String>,
    pub kind: Option<String>,
}

/// Receipts list response.
#[derive(Debug, Serialize)]
pub struct ReceiptsListResponse {
    pub ok: bool,
    pub receipts: Vec<ReceiptSummary>,
}

/// Receipt read query.
#[derive(Debug, Deserialize)]
pub struct ReceiptReadQuery {
    pub path: String,
}

/// Receipt read response.
#[derive(Debug, Serialize)]
pub struct ReceiptReadResponse {
    pub ok: bool,
    pub path: String,
    pub receipt: serde_json::Value,
}

/// Default receipt root directory.
pub fn default_receipts_root() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("osai")
        .join("receipts")
}

/// List receipt directories for chat, ask, apply.
pub fn receipt_dirs() -> Vec<PathBuf> {
    let root = default_receipts_root();
    vec![
        root.join("chat"),
        root.join("ask"),
        root.join("apply"),
        root.join("tool"),
        root.join("model-router"),
    ]
}

/// Secrets to redact from receipt data.
static REDACT_KEYS: &[&str] = &[
    "api_key",
    "token",
    "password",
    "secret",
    "credential",
    "authorization",
];

/// Defensively redact obvious secret keys from a JSON value.
pub fn redact_secrets(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (k, v) in map {
                if REDACT_KEYS.iter().any(|s| k.eq_ignore_ascii_case(s)) {
                    result.insert(
                        k.clone(),
                        serde_json::Value::String("[REDACTED]".to_string()),
                    );
                } else {
                    result.insert(k.clone(), redact_secrets(v));
                }
            }
            serde_json::Value::Object(result)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(redact_secrets).collect())
        }
        other => other.clone(),
    }
}

/// Checks if a path is within any of the allowed parent directories.
fn is_path_under_any_dir(path: &Path, allowed_parents: &[PathBuf]) -> bool {
    let canonical_path = match std::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    for parent in allowed_parents {
        let canonical_parent = match std::fs::canonicalize(parent) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if canonical_path.starts_with(&canonical_parent) {
            return true;
        }
    }
    false
}

/// List receipts from multiple directories.
pub fn list_receipts(
    limit: usize,
    kind_filter: Option<&str>,
    dir_override: Option<&PathBuf>,
) -> Result<ReceiptsListResponse> {
    let mut entries: Vec<ReceiptSummary> = vec![];

    let dirs_to_scan: Vec<PathBuf> = if let Some(dir) = dir_override {
        vec![dir.clone()]
    } else {
        receipt_dirs()
    };

    for dir in dirs_to_scan {
        if !dir.exists() {
            continue;
        }

        let kind_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_lowercase();

        // Apply kind filter if specified
        if let Some(k) = kind_filter {
            if k != "all" && kind_name != *k {
                continue;
            }
        }

        if let Ok(dir_entries) = std::fs::read_dir(&dir) {
            for entry in dir_entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
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

                // Try to parse receipt JSON
                let content = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let (id, status, action, actor) =
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(json) => {
                            let id = json
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let status = json
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let action = json
                                .get("action")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let actor = json
                                .get("actor")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            (id, status, action, actor)
                        }
                        Err(_) => (
                            filename.replace(".json", ""),
                            "unknown".to_string(),
                            "unknown".to_string(),
                            "unknown".to_string(),
                        ),
                    };

                entries.push(ReceiptSummary {
                    path: path.display().to_string(),
                    filename,
                    modified_unix,
                    size_bytes,
                    kind: kind_name.clone(),
                    id,
                    status,
                    action,
                    actor,
                });
            }
        }
    }

    // Sort newest first
    entries.sort_by(|a, b| b.modified_unix.cmp(&a.modified_unix));
    entries.truncate(limit);

    Ok(ReceiptsListResponse {
        ok: true,
        receipts: entries,
    })
}

/// Read a single receipt with path safety check.
pub fn read_receipt(path_str: &str) -> Result<ReceiptReadResponse> {
    if path_str.trim().is_empty() {
        return Err(anyhow::anyhow!("path is required"));
    }

    let path = PathBuf::from(path_str);

    if !path.exists() {
        return Err(anyhow::anyhow!("receipt file not found: {}", path_str));
    }

    // Path safety: ensure the path is under known receipt roots
    let allowed_roots = receipt_dirs();
    if !allowed_roots.is_empty() && !is_path_under_any_dir(&path, &allowed_roots) {
        return Err(anyhow::anyhow!(
            "path is outside allowed receipts directory: {}",
            path_str
        ));
    }

    let content = fs::read_to_string(&path)?;
    let receipt_raw: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::Value::Null);

    let receipt = redact_secrets(&receipt_raw);

    Ok(ReceiptReadResponse {
        ok: true,
        path: path.display().to_string(),
        receipt,
    })
}
