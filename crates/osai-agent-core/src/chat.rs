//! Chat operation - conversational interface with the model router.

use crate::shared::{
    default_chat_receipts_dir, is_loopback_url, ChatMessage, ChatMetadata, ChatRequest,
    ChatResponse,
};
use anyhow::Result;
use osai_receipt_logger::{Receipt, ReceiptStatus, ReceiptStore};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Result of a chat operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResult {
    pub status: String,
    pub content: Option<String>,
    pub response_length: Option<usize>,
    pub error: Option<String>,
}

/// Core chat logic shared between sync and async entry points.
/// Builds the request, calls model router, extracts content, writes receipt.
/// Does NOT print output - caller decides what to do with the result.
pub fn chat_core(
    message: &str,
    model_router_url: &str,
    receipts_dir_override: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
) -> Result<ChatResult> {
    // Validate loopback URL
    if !is_loopback_url(model_router_url) {
        return Err(anyhow::anyhow!(
            "Model router URL must be loopback only (127.0.0.1 or localhost): {}",
            model_router_url
        ));
    }

    // Build request
    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        }],
        max_tokens,
        temperature,
        metadata: ChatMetadata {
            privacy: privacy.to_string(),
        },
    };

    // Call model router
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/v1/chat/completions", model_router_url);

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .map_err(|e| anyhow::anyhow!("Model router request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Model router returned error {}: {}",
            status,
            body
        ));
    }

    let chat_response: ChatResponse = response
        .json()
        .map_err(|e| anyhow::anyhow!("Failed to parse model router response: {}", e))?;

    // Extract assistant content
    let content = chat_response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Model response missing content"))?
        .clone();

    let response_length = content.len();

    // Write receipt
    let receipts_dir = if let Some(dir) = receipts_dir_override {
        dir.to_path_buf()
    } else {
        default_chat_receipts_dir()
    };
    let store = ReceiptStore::new(&receipts_dir);
    store
        .ensure_dirs()
        .map_err(|e| anyhow::anyhow!("Failed to create receipts directory: {}", e))?;

    // Extract host from model router URL for receipt (no secrets)
    let mr_host = url::Url::parse(model_router_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let receipt = Receipt::new("osai-agent", "ModelChat")
        .with_tool("osai-agent chat")
        .with_risk("Low")
        .with_approval("Auto")
        .with_status(ReceiptStatus::Executed)
        .with_inputs(serde_json::json!({
            "model_router_url_host": mr_host,
            "model": model,
            "privacy": privacy,
            "prompt_length": message.len(),
            "temperature": temperature,
            "max_tokens": max_tokens,
        }))
        .with_outputs(serde_json::json!({
            "response_length": response_length,
            "finish_reason": chat_response.choices.first().and_then(|c| c.finish_reason.clone()),
        }));

    store
        .write(&receipt)
        .map_err(|e| anyhow::anyhow!("Failed to write receipt: {}", e))?;

    Ok(ChatResult {
        status: "success".to_string(),
        content: Some(content),
        response_length: Some(response_length),
        error: None,
    })
}

/// Async version of chat_core using reqwest::Client.
/// Use this from async API contexts (osai-api).
pub async fn chat_core_async(
    message: &str,
    model_router_url: &str,
    receipts_dir_override: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
) -> Result<ChatResult> {
    // Validate loopback URL
    if !is_loopback_url(model_router_url) {
        return Err(anyhow::anyhow!(
            "Model router URL must be loopback only (127.0.0.1 or localhost): {}",
            model_router_url
        ));
    }

    // Build request
    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: message.to_string(),
        }],
        max_tokens,
        temperature,
        metadata: ChatMetadata {
            privacy: privacy.to_string(),
        },
    };

    // Call model router asynchronously
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create HTTP client: {}", e))?;

    let url = format!("{}/v1/chat/completions", model_router_url);

    let response = client
        .post(&url)
        .json(&request)
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

    // Extract assistant content
    let content = chat_response
        .choices
        .first()
        .and_then(|c| c.message.content.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Model response missing content"))?
        .clone();

    let response_length = content.len();

    // Write receipt
    let receipts_dir = if let Some(dir) = receipts_dir_override {
        dir.to_path_buf()
    } else {
        default_chat_receipts_dir()
    };
    let store = ReceiptStore::new(&receipts_dir);
    store
        .ensure_dirs()
        .map_err(|e| anyhow::anyhow!("Failed to create receipts directory: {}", e))?;

    // Extract host from model router URL for receipt (no secrets)
    let mr_host = url::Url::parse(model_router_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let receipt = Receipt::new("osai-agent", "ModelChat")
        .with_tool("osai-agent chat")
        .with_risk("Low")
        .with_approval("Auto")
        .with_status(ReceiptStatus::Executed)
        .with_inputs(serde_json::json!({
            "model_router_url_host": mr_host,
            "model": model,
            "privacy": privacy,
            "prompt_length": message.len(),
            "temperature": temperature,
            "max_tokens": max_tokens,
        }))
        .with_outputs(serde_json::json!({
            "response_length": response_length,
            "finish_reason": chat_response.choices.first().and_then(|c| c.finish_reason.clone()),
        }));

    store
        .write(&receipt)
        .map_err(|e| anyhow::anyhow!("Failed to write receipt: {}", e))?;

    Ok(ChatResult {
        status: "success".to_string(),
        content: Some(content),
        response_length: Some(response_length),
        error: None,
    })
}

/// Runs a chat operation - sends a message to the model router and returns the response.
pub fn run_chat(
    message_arg: Option<&str>,
    model_router_url: &str,
    receipts_dir_override: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
    json_output: bool,
    positional_message: &[String],
) -> Result<()> {
    // Validate loopback URL
    if !is_loopback_url(model_router_url) {
        return Err(anyhow::anyhow!(
            "Model router URL must be loopback only (127.0.0.1 or localhost): {}",
            model_router_url
        ));
    }

    // Resolve message: check --message and positional separately
    let has_positional = !positional_message.is_empty();
    let has_message_arg = message_arg.is_some();

    if has_positional && has_message_arg {
        return Err(anyhow::anyhow!(
            "Cannot use both positional message and --message flag. Use one or the other."
        ));
    }

    if !has_positional && !has_message_arg {
        return Err(anyhow::anyhow!(
            "No message provided. Use a positional message or --message flag."
        ));
    }

    let message: String = if has_message_arg {
        message_arg.unwrap().to_string()
    } else {
        positional_message.join(" ")
    };

    let result = chat_core(
        &message,
        model_router_url,
        receipts_dir_override,
        model,
        privacy,
        max_tokens,
        temperature,
    )?;

    // Output
    if json_output {
        println!("{}", serde_json::to_string_pretty(&result).unwrap());
    } else if let Some(content) = result.content {
        println!("{}", content);
    }

    Ok(())
}
