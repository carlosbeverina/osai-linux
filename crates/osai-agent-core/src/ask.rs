//! Ask operation - generates OSAI Plan DSL YAML from natural language.

use crate::shared::{
    default_ask_plans_dir, default_ask_receipts_dir, is_loopback_url, sanitize_yaml_response,
    slug_from_request, ChatMessage, ChatMetadata, ChatRequest, ChatResponse,
};
use anyhow::Result;
use osai_plan_dsl::OsaiPlan;
use osai_receipt_logger::{Receipt, ReceiptStatus, ReceiptStore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Result of an ask operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AskResult {
    pub status: String,
    pub output_path: Option<String>,
    pub validation: String,
    pub error: Option<String>,
}

fn write_ask_receipt(
    receipts_dir_override: Option<&Path>,
    model_router_url: &str,
    model: &str,
    privacy: &str,
    request_length: usize,
    output_path: Option<&Path>,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    let receipts_dir = if let Some(dir) = receipts_dir_override {
        dir.to_path_buf()
    } else {
        default_ask_receipts_dir()
    };
    let store = ReceiptStore::new(&receipts_dir);
    store
        .ensure_dirs()
        .map_err(|e| anyhow::anyhow!("Failed to create receipts directory: {}", e))?;

    // Extract host from model router URL (no secrets)
    let mr_host = url::Url::parse(model_router_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let receipt_status = if status == "Executed" {
        ReceiptStatus::Executed
    } else {
        ReceiptStatus::Failed
    };

    let mut receipt = Receipt::new("osai-agent", "PlanGenerate")
        .with_tool("osai-agent ask")
        .with_risk("Low")
        .with_approval("Auto")
        .with_status(receipt_status)
        .with_inputs(serde_json::json!({
            "model_router_url_host": mr_host,
            "model": model,
            "privacy": privacy,
            "request_length": request_length,
        }))
        .with_outputs(serde_json::json!({
            "validation_status": status.to_lowercase(),
        }));

    if let Some(path) = output_path {
        let outputs = receipt
            .outputs_redacted
            .clone()
            .unwrap_or(serde_json::Value::Null);
        let mut obj = outputs.as_object().cloned().unwrap_or_default();
        obj.insert(
            "output_path".to_string(),
            serde_json::json!(path.display().to_string()),
        );
        receipt.outputs_redacted = Some(serde_json::Value::Object(obj));
    }

    if let Some(err) = error {
        receipt.error = Some(err.to_string());
    }

    store
        .write(&receipt)
        .map_err(|e| anyhow::anyhow!("Failed to write receipt: {}", e))?;

    Ok(())
}

/// Internal async model call for ask operations.
async fn ask_model_call_async(
    model_router_url: &str,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
    system_prompt: &str,
    request: &str,
) -> Result<String> {
    let chat_request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: request.to_string(),
            },
        ],
        max_tokens: max_tokens.or(Some(1200)),
        temperature,
        metadata: ChatMetadata {
            privacy: privacy.to_string(),
        },
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/v1/chat/completions", model_router_url);

    let response = client
        .post(&url)
        .json(&chat_request)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Model router request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Model router returned error {}: {}",
            status,
            body
        ));
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse model router response: {}", e))?;

    let content = chat_response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Model response missing content"))?
        .clone();

    Ok(content)
}

/// Async core ask logic - generates a plan and returns AskResult.
/// Use this from async API contexts (osai-api).
pub async fn ask_core_async(
    request: &str,
    model_router_url: &str,
    receipts_dir_override: Option<&Path>,
    plans_dir_override: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
) -> Result<AskResult> {
    // Validate loopback URL
    if !is_loopback_url(model_router_url) {
        return Err(anyhow::anyhow!(
            "Model router URL must be loopback only (127.0.0.1 or localhost): {}",
            model_router_url
        ));
    }

    let request_length = request.len();

    // Build system prompt for safe plan generation
    let system_prompt = r#"You are an OSAI plan generator. Return ONLY valid OSAI Plan DSL YAML with no markdown fences, no explanation, and no additional text.

## REQUIRED YAML SCHEMA

Your output MUST match this exact top-level structure:
version: "0.1"
id: "<generate a fresh UUID v4>"
title: "<short title>"
description: "<one-sentence description>"
actor: "user"
risk: Low
approval: Auto
steps:
  - id: "step-1"
    action:
      type: FilesList
    description: "<what this step does>"
    requires_approval: false
    inputs:
      path: "~/Downloads"
rollback:
  available: false
metadata: {}

## CRITICAL RULES

1. Use `steps:` — NEVER use `plan:` or any other top-level key for the action list.
2. Use exact action types: FilesList, FilesMove, FilesWrite, ModelChat, ReceiptCreate, DesktopNotify, BrowserOpenUrl, ShellRunSandboxed, Custom. Do NOT invent names like ListDirectory or ReadFile.
3. Each step needs id, action.type, description, requires_approval, inputs.
4. For listing a directory, use:
   action:
     type: FilesList
   inputs:
     path: "~/Downloads"
5. Omit rollback entirely (or use: rollback: ~). Do not add rollback.steps unless rollback is obvious.
6. Use metadata: {}.
7. Generate a fresh UUID v4 for id — do not reuse the example UUID.

## VALID EXAMPLE

For the request "List my Downloads folder", output exactly:

version: "0.1"
id: "00000000-0000-4000-8000-000000000001"
title: "List Downloads folder"
description: "List files in the user's Downloads folder"
actor: "user"
risk: Low
approval: Auto
steps:
  - id: "step-1"
    action:
      type: FilesList
    description: "List files in Downloads"
    requires_approval: false
    inputs:
      path: "~/Downloads"
metadata: {}

## SAFETY

- Do NOT include ShellRunSandboxed unless user explicitly asks to run a command.
- Do NOT include FilesWrite, FilesMove, FilesDelete (refuse destructive actions).
- Prefer ModelChat or ReceiptCreate for information-only tasks.
- risk: Low by default, Medium only with clear justification.
- approval: Auto for read-only, Ask for anything that modifies state."#;

    // Call model asynchronously
    let content = match ask_model_call_async(
        model_router_url,
        model,
        privacy,
        max_tokens,
        temperature,
        system_prompt,
        request,
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            // Write failed receipt
            let _ = write_ask_receipt(
                receipts_dir_override,
                model_router_url,
                model,
                privacy,
                request_length,
                None,
                "Failed",
                Some(&format!("Model router call failed: {}", e)),
            );
            return Err(anyhow::anyhow!("Model router call failed: {}", e));
        }
    };

    // Sanitize YAML response (strip markdown fences if present)
    let yaml_content = sanitize_yaml_response(&content);

    // Try to parse as OSAI Plan
    let mut plan = match OsaiPlan::from_yaml(&yaml_content) {
        Ok(p) => p,
        Err(e) => {
            // Write failed receipt
            let _ = write_ask_receipt(
                receipts_dir_override,
                model_router_url,
                model,
                privacy,
                request_length,
                None,
                "Failed",
                Some(&format!("YAML parse error: {}", e)),
            );
            return Err(anyhow::anyhow!(
                "Model returned invalid YAML: {}\nRaw response: {}",
                e,
                yaml_content.chars().take(200).collect::<String>()
            ));
        }
    };

    // Validate plan
    if let Err(e) = plan.validate() {
        // Write failed receipt
        let _ = write_ask_receipt(
            receipts_dir_override,
            model_router_url,
            model,
            privacy,
            request_length,
            None,
            "Failed",
            Some(&format!("Validation error: {}", e)),
        );
        return Err(anyhow::anyhow!(
            "Generated plan is invalid: {}\nYAML:\n{}",
            e,
            yaml_content
        ));
    }

    // Replace example UUID with a fresh generated UUID
    let example_uuid = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").ok();
    if let Some(expected) = example_uuid {
        if plan.id == expected {
            plan.id = uuid::Uuid::new_v4();
        }
    }

    // Determine output path using persistent XDG location
    let plans_dir = plans_dir_override
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| default_ask_plans_dir());
    let slug = slug_from_request(request);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let filename = format!("{}-{}.yml", slug, timestamp);
    let output_path = plans_dir.join(&filename);

    // Check if path already exists
    if output_path.exists() {
        return Err(anyhow::anyhow!(
            "Generated plan path already exists: {}",
            output_path.display()
        ));
    }

    // Create parent directories if needed
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create plans directory: {}", e))?;
    }

    // Save plan
    let yaml_output = plan
        .to_yaml()
        .map_err(|e| anyhow::anyhow!("Failed to serialize plan to YAML: {}", e))?;
    fs::write(&output_path, &yaml_output)
        .map_err(|e| anyhow::anyhow!("Failed to write plan file: {}", e))?;

    // Canonicalize to get absolute path for the response
    let absolute_output_path = std::fs::canonicalize(&output_path)
        .map_err(|e| anyhow::anyhow!("Failed to resolve absolute path: {}", e))?;

    // Write success receipt
    let _ = write_ask_receipt(
        receipts_dir_override,
        model_router_url,
        model,
        privacy,
        request_length,
        Some(&output_path),
        "Executed",
        None,
    );

    Ok(AskResult {
        status: "success".to_string(),
        output_path: Some(absolute_output_path.display().to_string()),
        validation: "valid".to_string(),
        error: None,
    })
}

/// Runs an ask operation - generates a safe OSAI Plan DSL YAML from a natural language request.
pub fn run_ask(
    message_arg: Option<&str>,
    model_router_url: &str,
    receipts_dir_override: Option<&Path>,
    plans_dir_override: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
    json_output: bool,
    print_plan: bool,
    output_override: Option<&Path>,
    positional_request: &[String],
) -> Result<()> {
    // Validate loopback URL
    if !is_loopback_url(model_router_url) {
        return Err(anyhow::anyhow!(
            "Model router URL must be loopback only (127.0.0.1 or localhost): {}",
            model_router_url
        ));
    }

    // Resolve request: check --message and positional separately
    let has_positional = !positional_request.is_empty();
    let has_message_arg = message_arg.is_some();

    if has_positional && has_message_arg {
        return Err(anyhow::anyhow!(
            "Cannot use both positional request and --message flag. Use one or the other."
        ));
    }

    if !has_positional && !has_message_arg {
        return Err(anyhow::anyhow!(
            "No request provided. Use a positional request or --message flag."
        ));
    }

    let request: String = if has_message_arg {
        message_arg.unwrap().to_string()
    } else {
        positional_request.join(" ")
    };

    let request_length = request.len();

    // Build system prompt for safe plan generation
    let system_prompt = r#"You are an OSAI plan generator. Return ONLY valid OSAI Plan DSL YAML with no markdown fences, no explanation, and no additional text.

## REQUIRED YAML SCHEMA

Your output MUST match this exact top-level structure:
version: "0.1"
id: "<generate a fresh UUID v4>"
title: "<short title>"
description: "<one-sentence description>"
actor: "user"
risk: Low
approval: Auto
steps:
  - id: "step-1"
    action:
      type: FilesList
    description: "<what this step does>"
    requires_approval: false
    inputs:
      path: "~/Downloads"
rollback:
  available: false
metadata: {}

## CRITICAL RULES

1. Use `steps:` — NEVER use `plan:` or any other top-level key for the action list.
2. Use exact action types: FilesList, FilesMove, FilesWrite, ModelChat, ReceiptCreate, DesktopNotify, BrowserOpenUrl, ShellRunSandboxed, Custom. Do NOT invent names like ListDirectory or ReadFile.
3. Each step needs id, action.type, description, requires_approval, inputs.
4. For listing a directory, use:
   action:
     type: FilesList
   inputs:
     path: "~/Downloads"
5. Omit rollback entirely (or use: rollback: ~). Do not add rollback.steps unless rollback is obvious.
6. Use metadata: {}.
7. Generate a fresh UUID v4 for id — do not reuse the example UUID.

## VALID EXAMPLE

For the request "List my Downloads folder", output exactly:

version: "0.1"
id: "00000000-0000-4000-8000-000000000001"
title: "List Downloads folder"
description: "List files in the user's Downloads folder"
actor: "user"
risk: Low
approval: Auto
steps:
  - id: "step-1"
    action:
      type: FilesList
    description: "List files in Downloads"
    requires_approval: false
    inputs:
      path: "~/Downloads"
metadata: {}

## SAFETY

- Do NOT include ShellRunSandboxed unless user explicitly asks to run a command.
- Do NOT include FilesWrite, FilesMove, FilesDelete (refuse destructive actions).
- Prefer ModelChat or ReceiptCreate for information-only tasks.
- risk: Low by default, Medium only with clear justification.
- approval: Auto for read-only, Ask for anything that modifies state."#;

    let _full_content = format!("{}\n\nUser request: {}", system_prompt, request);

    // Build request
    let chat_request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: request.clone(),
            },
        ],
        max_tokens: max_tokens.or(Some(1200)),
        temperature,
        metadata: ChatMetadata {
            privacy: privacy.to_string(),
        },
    };

    // Call model router
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/v1/chat/completions", model_router_url);

    let response = client
        .post(&url)
        .json(&chat_request)
        .send()
        .map_err(|e| anyhow::anyhow!("Model router request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        // Write failed receipt
        write_ask_receipt(
            receipts_dir_override,
            model_router_url,
            model,
            privacy,
            request_length,
            None,
            "Failed",
            Some(&format!("Model router returned error {}: {}", status, body)),
        )?;
        return Err(anyhow::anyhow!(
            "Model router returned error {}: {}",
            status,
            body
        ));
    }

    let chat_response: ChatResponse = response
        .json()
        .map_err(|e| anyhow::anyhow!("Failed to parse model router response: {}", e))?;

    // Extract content
    let content = chat_response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Model response missing content"))?
        .clone();

    // Sanitize YAML response (strip markdown fences if present)
    let yaml_content = sanitize_yaml_response(&content);

    // Try to parse as OSAI Plan
    let mut plan = match OsaiPlan::from_yaml(&yaml_content) {
        Ok(p) => p,
        Err(e) => {
            // Write failed receipt
            write_ask_receipt(
                receipts_dir_override,
                model_router_url,
                model,
                privacy,
                request_length,
                None,
                "Failed",
                Some(&format!("YAML parse error: {}", e)),
            )?;
            return Err(anyhow::anyhow!(
                "Model returned invalid YAML: {}\nRaw response: {}",
                e,
                yaml_content.chars().take(200).collect::<String>()
            ));
        }
    };

    // Validate plan
    if let Err(e) = plan.validate() {
        // Write failed receipt
        write_ask_receipt(
            receipts_dir_override,
            model_router_url,
            model,
            privacy,
            request_length,
            None,
            "Failed",
            Some(&format!("Validation error: {}", e)),
        )?;
        return Err(anyhow::anyhow!(
            "Generated plan is invalid: {}\nYAML:\n{}",
            e,
            yaml_content
        ));
    }

    // Replace example UUID with a fresh generated UUID
    let example_uuid = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").ok();
    if let Some(expected) = example_uuid {
        if plan.id == expected {
            plan.id = uuid::Uuid::new_v4();
        }
    }

    // Determine output path using persistent XDG location
    let output_path = if let Some(path) = output_override {
        if path.exists() {
            return Err(anyhow::anyhow!(
                "Output path already exists: {}",
                path.display()
            ));
        }
        path.to_path_buf()
    } else {
        let plans_dir = plans_dir_override
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| default_ask_plans_dir());
        let slug = slug_from_request(&request);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("{}-{}.yml", slug, timestamp);
        let full_path = plans_dir.join(&filename);
        if full_path.exists() {
            return Err(anyhow::anyhow!(
                "Generated plan path already exists: {}",
                full_path.display()
            ));
        }
        full_path
    };

    // Create parent directories if needed
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("Failed to create plans directory: {}", e))?;
    }

    // Save plan
    let yaml_output = plan
        .to_yaml()
        .map_err(|e| anyhow::anyhow!("Failed to serialize plan to YAML: {}", e))?;
    fs::write(&output_path, &yaml_output)
        .map_err(|e| anyhow::anyhow!("Failed to write plan file: {}", e))?;

    // Write success receipt
    write_ask_receipt(
        receipts_dir_override,
        model_router_url,
        model,
        privacy,
        request_length,
        Some(&output_path),
        "Executed",
        None,
    )?;

    // Print output
    if json_output {
        #[derive(Serialize)]
        struct AskResult {
            status: String,
            output_path: String,
            validation: String,
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&AskResult {
                status: "success".to_string(),
                output_path: output_path.display().to_string(),
                validation: "valid".to_string(),
            })
            .unwrap()
        );
    } else {
        println!("Generated valid plan: {}", output_path.display());
    }

    // Print plan if requested
    if print_plan {
        println!();
        println!("{}", yaml_output);
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ask_plans_dir_is_persistent() {
        let plans_dir = default_ask_plans_dir();
        let plans_dir_str = plans_dir.to_string_lossy();
        // Should not be a temp dir
        assert!(
            !plans_dir_str.contains("tmp"),
            "default plans dir should not be in tmp"
        );
        // Should be under osai
        assert!(
            plans_dir_str.contains("osai"),
            "default plans dir should be under osai"
        );
    }

    #[test]
    fn test_ask_result_serialization() {
        let result = AskResult {
            status: "success".to_string(),
            output_path: Some("/home/user/.local/share/osai/plans/test-123.yml".to_string()),
            validation: "valid".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains(".local/share/osai/plans"));
    }

    #[test]
    fn test_ask_result_with_error() {
        let result = AskResult {
            status: "error".to_string(),
            output_path: None,
            validation: "invalid".to_string(),
            error: Some("model failed".to_string()),
        };
        assert_eq!(result.status, "error");
        assert!(result.output_path.is_none());
        assert!(result.error.is_some());
    }
}
