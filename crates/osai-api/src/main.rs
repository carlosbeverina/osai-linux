//! OSAI local API service - exposes agent capabilities via HTTP.
//!
//! Binds to loopback only (127.0.0.1) by default for security.
//! Does not expose external interfaces.

use anyhow::Result;
use osai_agent_core::{
    apply::run_apply,
    shared::{
        default_apply_receipts_dir, default_ask_receipts_dir, default_chat_receipts_dir,
        is_loopback_url,
    },
};
use osai_plan_dsl::OsaiPlan;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

// Version constant
const VERSION: &str = "0.1.0";

// ============================================================================
// Request/Response Types
// ============================================================================

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    service: String,
    version: String,
}

/// Capabilities response
#[derive(Debug, Serialize)]
struct CapabilitiesResponse {
    chat: bool,
    ask: bool,
    plan_validate: bool,
    apply: bool,
    receipts: bool,
}

/// Chat API request (v1)
#[derive(Debug, Deserialize)]
struct ChatRequestV1 {
    message: String,
    model_router_url: Option<String>,
    model: Option<String>,
    privacy: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    receipts_dir: Option<String>,
}

/// Chat API response
#[derive(Debug, Serialize)]
struct ChatResponseV1 {
    status: String,
    content: Option<String>,
    response_length: Option<usize>,
    error: Option<String>,
}

/// Ask API request (v1)
#[derive(Debug, Deserialize)]
struct AskRequestV1 {
    request: String,
    model_router_url: Option<String>,
    model: Option<String>,
    privacy: Option<String>,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    plans_dir: Option<String>,
    receipts_dir: Option<String>,
}

/// Ask API response
#[derive(Debug, Serialize)]
struct AskResponseV1 {
    status: String,
    output_path: Option<String>,
    validation: String,
    error: Option<String>,
}

/// Plan validate API request
#[derive(Debug, Deserialize)]
struct PlanValidateRequest {
    plan_path: String,
}

/// Plan validate API response
#[derive(Debug, Serialize)]
struct PlanValidateResponse {
    ok: bool,
    valid: bool,
    error: Option<String>,
}

/// Apply API request (v1)
#[derive(Debug, Deserialize)]
struct ApplyRequestV1 {
    plan_path: String,
    policy_path: Option<String>,
    receipts_dir: Option<String>,
    allowed_roots: Option<Vec<String>>,
    model_router_url: Option<String>,
    approve: Option<Vec<String>>,
    approve_all: Option<bool>,
    dry_run: Option<bool>,
}

/// Apply API response
#[derive(Debug, Serialize)]
struct ApplyResponseV1 {
    status: String,
    executed: u32,
    skipped: u32,
    denied: u32,
    approval_required: u32,
    failed: u32,
    approved_steps: Vec<String>,
    dry_run: bool,
    error: Option<String>,
}

// ============================================================================
// HTTP Helpers
// ============================================================================

/// Parse JSON request body (sync - bytes already in memory)
fn parse_body<T: for<'de> Deserialize<'de>>(buf: &[u8]) -> Result<T> {
    serde_json::from_slice(buf).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
}

/// Send JSON response
async fn send_json<S: Serialize>(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    data: &S,
) -> Result<()> {
    let json = serde_json::to_string(data)?;
    let body = format!(
        "HTTP/1.1 {} \r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        match status {
            200 => "200 OK",
            400 => "400 Bad Request",
            404 => "404 Not Found",
            500 => "500 Internal Server Error",
            _ => "200 OK",
        },
        json.len(),
        json
    );
    stream.write_all(body.as_bytes()).await?;
    Ok(())
}

/// Send error response
async fn send_error(stream: &mut tokio::net::TcpStream, status: u16, message: &str) -> Result<()> {
    let body = format!(
        "HTTP/1.1 {} \r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{{\"error\":\"{}\"}}",
        match status {
            400 => "400 Bad Request",
            500 => "500 Internal Server Error",
            _ => "500 Internal Server Error",
        },
        message.len() + 18, // {"error":"..."} around message
        message
    );
    stream.write_all(body.as_bytes()).await?;
    Ok(())
}

// ============================================================================
// Request Handlers
// ============================================================================

/// Handle GET /health
async fn handle_health(stream: &mut tokio::net::TcpStream) -> Result<()> {
    let resp = HealthResponse {
        ok: true,
        service: "osai-api".to_string(),
        version: VERSION.to_string(),
    };
    send_json(stream, 200, &resp).await
}

/// Handle GET /v1/capabilities
async fn handle_capabilities(stream: &mut tokio::net::TcpStream) -> Result<()> {
    let resp = CapabilitiesResponse {
        chat: true,
        ask: true,
        plan_validate: true,
        apply: true,
        receipts: true,
    };
    send_json(stream, 200, &resp).await
}

/// Handle POST /v1/chat
async fn handle_chat(stream: &mut tokio::net::TcpStream, body: &[u8]) -> Result<()> {
    let req: ChatRequestV1 = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &e.to_string()).await?;
            return Ok(());
        }
    };

    if req.message.trim().is_empty() {
        send_error(stream, 400, "message is required").await?;
        return Ok(());
    }

    let model_router_url = req
        .model_router_url
        .unwrap_or_else(|| "http://127.0.0.1:8088".to_string());

    // Validate loopback URL
    if !is_loopback_url(&model_router_url) {
        send_error(
            stream,
            400,
            &format!(
                "model router URL must be loopback only (127.0.0.1 or localhost): {}",
                model_router_url
            ),
        )
        .await?;
        return Ok(());
    }

    let model = req.model.unwrap_or_else(|| "osai-auto".to_string());
    let privacy = req.privacy.unwrap_or_else(|| "local_only".to_string());
    let temperature = req.temperature.unwrap_or(0.2);

    let receipts_path = req
        .receipts_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_chat_receipts_dir);

    // Call chat_core_async from osai-agent-core (respects architecture boundary)
    let result = osai_agent_core::chat_core_async(
        &req.message,
        &model_router_url,
        Some(&receipts_path),
        &model,
        &privacy,
        req.max_tokens,
        temperature,
    )
    .await;

    let resp = match result {
        Ok(r) => ChatResponseV1 {
            status: r.status,
            content: r.content,
            response_length: r.response_length,
            error: r.error,
        },
        Err(e) => ChatResponseV1 {
            status: "error".to_string(),
            content: None,
            response_length: None,
            error: Some(e.to_string()),
        },
    };
    send_json(stream, 200, &resp).await
}

/// Handle POST /v1/ask
async fn handle_ask(stream: &mut tokio::net::TcpStream, body: &[u8]) -> Result<()> {
    let req: AskRequestV1 = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &e.to_string()).await?;
            return Ok(());
        }
    };

    if req.request.trim().is_empty() {
        send_error(stream, 400, "request is required").await?;
        return Ok(());
    }

    let model_router_url = req
        .model_router_url
        .unwrap_or_else(|| "http://127.0.0.1:8088".to_string());

    // Validate loopback URL
    if !is_loopback_url(&model_router_url) {
        send_error(
            stream,
            400,
            &format!(
                "model_router_url must be loopback only (127.0.0.1 or localhost): {}",
                model_router_url
            ),
        )
        .await?;
        return Ok(());
    }

    let model = req.model.unwrap_or_else(|| "osai-auto".to_string());
    let privacy = req.privacy.unwrap_or_else(|| "local_only".to_string());
    let temperature = req.temperature.unwrap_or(0.1);

    let receipts_path = req
        .receipts_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_ask_receipts_dir);

    let plans_path = req
        .plans_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./generated/plans"));

    // Call ask_core_async from osai-agent-core (respects architecture boundary)
    let result = osai_agent_core::ask_core_async(
        &req.request,
        &model_router_url,
        Some(&receipts_path),
        Some(&plans_path),
        &model,
        &privacy,
        req.max_tokens,
        temperature,
    )
    .await;

    let resp = match result {
        Ok(r) => AskResponseV1 {
            status: r.status,
            output_path: r.output_path,
            validation: r.validation,
            error: r.error,
        },
        Err(e) => AskResponseV1 {
            status: "error".to_string(),
            output_path: None,
            validation: "invalid".to_string(),
            error: Some(e.to_string()),
        },
    };
    send_json(stream, 200, &resp).await
}

/// Handle POST /v1/plans/validate
async fn handle_plan_validate(stream: &mut tokio::net::TcpStream, body: &[u8]) -> Result<()> {
    let req: PlanValidateRequest = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &e.to_string()).await?;
            return Ok(());
        }
    };

    let plan_path = PathBuf::from(&req.plan_path);
    if !plan_path.exists() {
        let resp = PlanValidateResponse {
            ok: false,
            valid: false,
            error: Some(format!("plan file not found: {}", req.plan_path)),
        };
        send_json(stream, 200, &resp).await?;
        return Ok(());
    }

    let content = match fs::read_to_string(&plan_path) {
        Ok(c) => c,
        Err(e) => {
            let resp = PlanValidateResponse {
                ok: false,
                valid: false,
                error: Some(format!("failed to read plan file: {}", e)),
            };
            send_json(stream, 200, &resp).await?;
            return Ok(());
        }
    };

    let plan = match OsaiPlan::from_yaml(&content).or_else(|_| OsaiPlan::from_json(&content)) {
        Ok(p) => p,
        Err(e) => {
            let resp = PlanValidateResponse {
                ok: true,
                valid: false,
                error: Some(format!("parse error: {}", e)),
            };
            send_json(stream, 200, &resp).await?;
            return Ok(());
        }
    };

    match plan.validate() {
        Ok(()) => {
            let resp = PlanValidateResponse {
                ok: true,
                valid: true,
                error: None,
            };
            send_json(stream, 200, &resp).await?;
        }
        Err(e) => {
            let resp = PlanValidateResponse {
                ok: true,
                valid: false,
                error: Some(format!("validation error: {}", e)),
            };
            send_json(stream, 200, &resp).await?;
        }
    };
    Ok(())
}

/// Handle POST /v1/apply
async fn handle_apply(stream: &mut tokio::net::TcpStream, body: &[u8]) -> Result<()> {
    let req: ApplyRequestV1 = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &e.to_string()).await?;
            return Ok(());
        }
    };

    // Validate model_router_url if provided
    if let Some(ref url) = req.model_router_url {
        if !is_loopback_url(url) {
            send_error(
                stream,
                400,
                &format!(
                    "model_router_url must be loopback only (127.0.0.1 or localhost): {}",
                    url
                ),
            )
            .await?;
            return Ok(());
        }
    }

    let receipts_path = req
        .receipts_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_apply_receipts_dir);

    let policy_path = PathBuf::from(
        req.policy_path
            .unwrap_or_else(|| "examples/policies/default-secure.yml".to_string()),
    );
    let plan_path = PathBuf::from(&req.plan_path);

    // dry_run defaults to true for safety
    let dry_run = req.dry_run.unwrap_or(true);

    let allowed_roots: Vec<PathBuf> = req
        .allowed_roots
        .unwrap_or_default()
        .iter()
        .map(PathBuf::from)
        .collect();

    let result = run_apply(
        &plan_path,
        &policy_path,
        Some(&receipts_path),
        &allowed_roots,
        req.approve.as_deref().unwrap_or(&[]),
        req.approve_all.unwrap_or(false),
        req.model_router_url.as_deref(),
        dry_run,
        false,
    );

    let resp = match result {
        Ok(()) => ApplyResponseV1 {
            status: "success".to_string(),
            executed: 0,
            skipped: 0,
            denied: 0,
            approval_required: 0,
            failed: 0,
            approved_steps: vec![],
            dry_run,
            error: None,
        },
        Err(e) => ApplyResponseV1 {
            status: "error".to_string(),
            executed: 0,
            skipped: 0,
            denied: 0,
            approval_required: 0,
            failed: 0,
            approved_steps: vec![],
            dry_run,
            error: Some(e.to_string()),
        },
    };
    send_json(stream, 200, &resp).await
}

// ============================================================================
// Router
// ============================================================================

/// Route request to handler
async fn route(
    stream: &mut tokio::net::TcpStream,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<()> {
    match (method, path) {
        // Health (no version prefix - simple)
        ("GET", "/health") => handle_health(stream).await?,
        // V1 endpoints
        ("GET", "/v1/capabilities") => handle_capabilities(stream).await?,
        ("POST", "/v1/chat") => handle_chat(stream, body).await?,
        ("POST", "/v1/ask") => handle_ask(stream, body).await?,
        ("POST", "/v1/plans/validate") => handle_plan_validate(stream, body).await?,
        ("POST", "/v1/apply") => handle_apply(stream, body).await?,
        // Aliases for backwards compatibility (no version)
        ("POST", "/chat") => handle_chat(stream, body).await?,
        ("POST", "/ask") => handle_ask(stream, body).await?,
        ("POST", "/apply") => handle_apply(stream, body).await?,
        // 404
        _ => {
            let json = serde_json::to_string(&serde_json::json!({"error": "not found"})).unwrap();
            let body = format!(
                "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                json.len(),
                json
            );
            stream.write_all(body.as_bytes()).await?;
        }
    }
    Ok(())
}

/// Parse HTTP request line
fn parse_request_line(line: &str) -> Option<(&str, &str)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        Some((parts[0], parts[1]))
    } else {
        None
    }
}

/// Parse and handle a TCP connection
async fn handle_connection(stream: tokio::net::TcpStream) {
    let mut stream = stream;
    let mut buf = [0u8; 16384];

    // Read request
    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse request line
    let request_line = request.lines().next().unwrap_or("");
    let (method, path) = match parse_request_line(request_line) {
        Some((m, p)) => (m, p),
        None => return,
    };

    // Skip headers - find body start
    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let mut body = Vec::new();
    if body_start < n {
        body.extend_from_slice(&buf[body_start..n]);
    }

    // Route request
    if let Err(e) = route(&mut stream, method, path, &body).await {
        eprintln!("request error: {}", e);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let bind_addr = "127.0.0.1:8090";
    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!("OSAI API server listening on {}", bind_addr);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle_connection(stream));
            }
            Err(e) => {
                tracing::error!("accept error: {}", e);
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_response_serialization() {
        let resp = CapabilitiesResponse {
            chat: true,
            ask: true,
            plan_validate: true,
            apply: true,
            receipts: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"chat\":true"));
        assert!(json.contains("\"ask\":true"));
        assert!(json.contains("\"plan_validate\":true"));
        assert!(json.contains("\"apply\":true"));
        assert!(json.contains("\"receipts\":true"));
    }

    #[test]
    fn test_chat_request_deserialization() {
        let json = r#"{"message": "hello", "model": "test", "temperature": 0.5}"#;
        let req: ChatRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "hello");
        assert_eq!(req.model, Some("test".to_string()));
        assert_eq!(req.temperature, Some(0.5));
        assert!(req.model_router_url.is_none());
        assert!(req.receipts_dir.is_none());
    }

    #[test]
    fn test_chat_request_missing_message_field() {
        // message is required, so missing field causes parse error
        // Handler validates this and returns JSON error
        let json = r#"{"model": "test"}"#;
        let result: Result<ChatRequestV1, _> = serde_json::from_str(json);
        assert!(result.is_err()); // message is required
    }

    #[test]
    fn test_chat_request_empty_message() {
        // Empty message string is allowed by deserialization but rejected by handler
        let json = r#"{"message": ""}"#;
        let req: ChatRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "");
        // Handler will reject empty message with 400
    }

    #[test]
    fn test_ask_request_deserialization() {
        let json = r#"{"request": "list downloads", "plans_dir": "/tmp/plans"}"#;
        let req: AskRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.request, "list downloads");
        assert_eq!(req.plans_dir, Some("/tmp/plans".to_string()));
    }

    #[test]
    fn test_plan_validate_request_deserialization() {
        let json = r#"{"plan_path": "/path/to/plan.yml"}"#;
        let req: PlanValidateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.plan_path, "/path/to/plan.yml");
    }

    #[test]
    fn test_plan_validate_response_valid() {
        let resp = PlanValidateResponse {
            ok: true,
            valid: true,
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"valid\":true"));
        assert!(json.contains("\"error\":null"));
    }

    #[test]
    fn test_plan_validate_response_invalid() {
        let resp = PlanValidateResponse {
            ok: true,
            valid: false,
            error: Some("parse error".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"valid\":false"));
        assert!(json.contains("\"error\":\"parse error\""));
    }

    #[test]
    fn test_apply_request_deserialization() {
        let json = r#"{"plan_path": "/plan.yml", "dry_run": false}"#;
        let req: ApplyRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.plan_path, "/plan.yml");
        assert_eq!(req.dry_run, Some(false));
        assert!(req.approve.is_none());
        assert!(req.approve_all.is_none());
    }

    #[test]
    fn test_apply_request_all_fields() {
        let json = r#"{
            "plan_path": "/plan.yml",
            "policy_path": "/policy.yml",
            "receipts_dir": "/receipts",
            "allowed_roots": ["/home"],
            "model_router_url": "http://127.0.0.1:8088",
            "approve": ["step-1"],
            "approve_all": true,
            "dry_run": false
        }"#;
        let req: ApplyRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.plan_path, "/plan.yml");
        assert_eq!(req.policy_path, Some("/policy.yml".to_string()));
        assert_eq!(req.receipts_dir, Some("/receipts".to_string()));
        assert_eq!(req.allowed_roots, Some(vec!["/home".to_string()]));
        assert_eq!(
            req.model_router_url,
            Some("http://127.0.0.1:8088".to_string())
        );
        assert_eq!(req.approve, Some(vec!["step-1".to_string()]));
        assert_eq!(req.approve_all, Some(true));
        assert_eq!(req.dry_run, Some(false));
    }

    #[test]
    fn test_health_response_serialization() {
        let resp = HealthResponse {
            ok: true,
            service: "osai-api".to_string(),
            version: "0.1.0".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"service\":\"osai-api\""));
        assert!(json.contains("\"version\":\"0.1.0\""));
    }

    #[test]
    fn test_is_loopback_url_validation() {
        assert!(is_loopback_url("http://127.0.0.1:8088"));
        assert!(is_loopback_url("http://localhost:8088"));
        assert!(!is_loopback_url("http://0.0.0.0:8088"));
        assert!(!is_loopback_url("http://192.168.1.1:8088"));
        assert!(!is_loopback_url("http://example.com:8088"));
    }

    #[test]
    fn test_apply_request_dry_run_defaults_to_true() {
        // When dry_run is not provided, it should be None (not Some(true))
        // The handler will default it to true
        let json = r#"{"plan_path": "/plan.yml"}"#;
        let req: ApplyRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.dry_run, None); // None means caller didn't specify, handler defaults to true
    }
}
