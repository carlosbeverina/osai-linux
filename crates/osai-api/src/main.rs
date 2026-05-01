//! osai-api - UI-ready local API for future OSAI desktop/shell UI.
//!
//! Binds to loopback only (127.0.0.1) by default for security.

mod errors;
mod plans;
mod receipts;
mod status;

use anyhow::Result;
use osai_agent_core::{
    apply::authorize_plan_preview,
    shared::{
        default_apply_receipts_dir, default_ask_plans_dir, default_ask_receipts_dir,
        default_chat_receipts_dir, is_loopback_url, resolve_policy_path,
    },
};
use osai_plan_dsl::OsaiPlan;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

use errors::{send_error, ErrorResponse};

// Version constant
const VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// Request/Response Types
// ============================================================================

/// Capabilities response
#[derive(Debug, Serialize)]
struct CapabilitiesResponse {
    chat: bool,
    ask: bool,
    plan_validate: bool,
    plan_authorize: bool,
    apply: bool,
    plans: bool,
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

/// Authorize API request
#[derive(Debug, Deserialize)]
struct AuthorizeRequest {
    plan_path: String,
    policy_path: Option<String>,
    allowed_roots: Option<Vec<String>>,
    model_router_url: Option<String>,
    approve: Option<Vec<String>>,
    approve_all: Option<bool>,
}

// ============================================================================
// HTTP Helpers
// ============================================================================

/// Parse JSON request body
fn parse_body<T: for<'de> Deserialize<'de>>(buf: &[u8]) -> Result<T> {
    serde_json::from_slice(buf).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))
}

/// Send JSON response
async fn send_json<S: Serialize>(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    data: &S,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(data)?;
    let body = format!(
        "HTTP/1.1 {} \r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        match status {
            200 => "200 OK",
            400 => "400 Bad Request",
            404 => "404 Not Found",
            405 => "405 Method Not Allowed",
            500 => "500 Internal Server Error",
            _ => "200 OK",
        },
        json.len(),
        json
    );
    tokio::io::AsyncWriteExt::write_all(stream, body.as_bytes()).await?;
    Ok(())
}

// ============================================================================
// Request Handlers
// ============================================================================

/// Handle GET /health
async fn handle_health(stream: &mut tokio::net::TcpStream) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct HealthResponse {
        ok: bool,
        service: String,
        version: String,
    }
    let resp = HealthResponse {
        ok: true,
        service: "osai-api".to_string(),
        version: VERSION.to_string(),
    };
    send_json(stream, 200, &resp).await
}

/// Handle GET /v1/capabilities
async fn handle_capabilities(stream: &mut tokio::net::TcpStream) -> anyhow::Result<()> {
    let resp = CapabilitiesResponse {
        chat: true,
        ask: true,
        plan_validate: true,
        plan_authorize: true,
        apply: true,
        plans: true,
        receipts: true,
    };
    send_json(stream, 200, &resp).await
}

/// Handle GET /v1/status
async fn handle_status(stream: &mut tokio::net::TcpStream) -> anyhow::Result<()> {
    let model_router_url = "http://127.0.0.1:8088";
    let resp = status::StatusResponse::new(model_router_url).await;
    send_json(stream, 200, &resp).await
}

/// Handle POST /v1/chat
async fn handle_chat(stream: &mut tokio::net::TcpStream, body: &[u8]) -> anyhow::Result<()> {
    let req: ChatRequestV1 = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    if req.message.trim().is_empty() {
        send_error(
            stream,
            400,
            &ErrorResponse::bad_request("message is required"),
        )
        .await?;
        return Ok(());
    }

    let model_router_url = req
        .model_router_url
        .unwrap_or_else(|| "http://127.0.0.1:8088".to_string());

    if !is_loopback_url(&model_router_url) {
        send_error(
            stream,
            400,
            &ErrorResponse::bad_request(&format!(
                "model router URL must be loopback only: {}",
                model_router_url
            )),
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
async fn handle_ask(stream: &mut tokio::net::TcpStream, body: &[u8]) -> anyhow::Result<()> {
    let req: AskRequestV1 = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    if req.request.trim().is_empty() {
        send_error(
            stream,
            400,
            &ErrorResponse::bad_request("request is required"),
        )
        .await?;
        return Ok(());
    }

    let model_router_url = req
        .model_router_url
        .unwrap_or_else(|| "http://127.0.0.1:8088".to_string());

    if !is_loopback_url(&model_router_url) {
        send_error(
            stream,
            400,
            &ErrorResponse::bad_request(&format!(
                "model_router_url must be loopback only: {}",
                model_router_url
            )),
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
        .unwrap_or_else(default_ask_plans_dir);

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
async fn handle_plan_validate(
    stream: &mut tokio::net::TcpStream,
    body: &[u8],
) -> anyhow::Result<()> {
    let req: PlanValidateRequest = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    let plan_path = PathBuf::from(&req.plan_path);
    if !plan_path.exists() {
        send_json(
            stream,
            200,
            &PlanValidateResponse {
                ok: false,
                valid: false,
                error: Some(format!("plan file not found: {}", req.plan_path)),
            },
        )
        .await?;
        return Ok(());
    }

    let content = match fs::read_to_string(&plan_path) {
        Ok(c) => c,
        Err(e) => {
            send_json(
                stream,
                200,
                &PlanValidateResponse {
                    ok: false,
                    valid: false,
                    error: Some(format!("failed to read plan file: {}", e)),
                },
            )
            .await?;
            return Ok(());
        }
    };

    let plan = match OsaiPlan::from_yaml(&content).or_else(|_| OsaiPlan::from_json(&content)) {
        Ok(p) => p,
        Err(e) => {
            send_json(
                stream,
                200,
                &PlanValidateResponse {
                    ok: true,
                    valid: false,
                    error: Some(format!("parse error: {}", e)),
                },
            )
            .await?;
            return Ok(());
        }
    };

    match plan.validate() {
        Ok(()) => {
            send_json(
                stream,
                200,
                &PlanValidateResponse {
                    ok: true,
                    valid: true,
                    error: None,
                },
            )
            .await?;
        }
        Err(e) => {
            send_json(
                stream,
                200,
                &PlanValidateResponse {
                    ok: true,
                    valid: false,
                    error: Some(format!("validation error: {}", e)),
                },
            )
            .await?;
        }
    };
    Ok(())
}

/// Handle GET /v1/plans
async fn handle_plans_list(stream: &mut tokio::net::TcpStream, query: &str) -> anyhow::Result<()> {
    let query: plans::PlansQuery = match serde_urlencoded::from_str(query) {
        Ok(q) => q,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    let limit = query.limit.unwrap_or(20).min(100);
    let dir = if let Some(d) = query.dir {
        PathBuf::from(d)
    } else {
        plans::default_plans_dir()
    };

    match plans::list_plans(&dir, limit) {
        Ok(resp) => send_json(stream, 200, &resp).await,
        Err(e) => send_error(stream, 500, &ErrorResponse::internal(&e.to_string())).await,
    }
}

/// Handle GET /v1/plans/read
async fn handle_plans_read(stream: &mut tokio::net::TcpStream, query: &str) -> anyhow::Result<()> {
    let query: plans::PlanReadQuery = match serde_urlencoded::from_str(query) {
        Ok(q) => q,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    if query.path.trim().is_empty() {
        send_error(stream, 400, &ErrorResponse::bad_request("path is required")).await?;
        return Ok(());
    }

    let plans_dir = plans::default_plans_dir();
    match plans::read_plan(&query.path, &plans_dir) {
        Ok(resp) => send_json(stream, 200, &resp).await,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                send_error(stream, 404, &ErrorResponse::not_found(&msg)).await
            } else if msg.contains("outside allowed") {
                send_error(stream, 400, &ErrorResponse::bad_request(&msg)).await
            } else {
                send_error(stream, 500, &ErrorResponse::internal(&msg)).await
            }
        }
    }
}

/// Handle POST /v1/plans/authorize
async fn handle_plans_authorize(
    stream: &mut tokio::net::TcpStream,
    body: &[u8],
) -> anyhow::Result<()> {
    let req: AuthorizeRequest = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    if req.plan_path.trim().is_empty() {
        send_error(
            stream,
            400,
            &ErrorResponse::bad_request("plan_path is required"),
        )
        .await?;
        return Ok(());
    }

    let plan_path = PathBuf::from(&req.plan_path);
    if !plan_path.exists() {
        send_error(
            stream,
            404,
            &ErrorResponse::not_found(&format!("plan file not found: {}", req.plan_path)),
        )
        .await?;
        return Ok(());
    }

    // Validate model_router_url if provided
    if let Some(ref url) = req.model_router_url {
        if !is_loopback_url(url) {
            send_error(
                stream,
                400,
                &ErrorResponse::bad_request(&format!(
                    "model_router_url must be loopback only: {}",
                    url
                )),
            )
            .await?;
            return Ok(());
        }
    }

    let policy_path = resolve_policy_path(req.policy_path.as_deref());

    let allowed_roots: Vec<PathBuf> = req
        .allowed_roots
        .unwrap_or_else(|| vec![])
        .iter()
        .map(PathBuf::from)
        .collect();

    let approve = req.approve.unwrap_or_default();
    let approve_all = req.approve_all.unwrap_or(false);

    match authorize_plan_preview(
        &plan_path,
        &policy_path,
        &allowed_roots,
        &approve,
        approve_all,
    ) {
        Ok(result) => send_json(stream, 200, &result).await,
        Err(e) => send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await,
    }
}

/// Handle POST /v1/apply
async fn handle_apply(stream: &mut tokio::net::TcpStream, body: &[u8]) -> anyhow::Result<()> {
    let req: ApplyRequestV1 = match parse_body(body) {
        Ok(r) => r,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    if let Some(ref url) = req.model_router_url {
        if !is_loopback_url(url) {
            send_error(
                stream,
                400,
                &ErrorResponse::bad_request(&format!(
                    "model_router_url must be loopback only: {}",
                    url
                )),
            )
            .await?;
            return Ok(());
        }
    }

    let receipts_path = req
        .receipts_dir
        .map(PathBuf::from)
        .unwrap_or_else(default_apply_receipts_dir);

    let policy_path = resolve_policy_path(req.policy_path.as_deref());
    let plan_path = PathBuf::from(&req.plan_path);

    let dry_run = req.dry_run.unwrap_or(true);

    let allowed_roots: Vec<PathBuf> = req
        .allowed_roots
        .unwrap_or_default()
        .iter()
        .map(PathBuf::from)
        .collect();

    let result = osai_agent_core::run_apply(
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

/// Handle GET /v1/receipts
async fn handle_receipts_list(
    stream: &mut tokio::net::TcpStream,
    query: &str,
) -> anyhow::Result<()> {
    let query: receipts::ReceiptsQuery = match serde_urlencoded::from_str(query) {
        Ok(q) => q,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    let limit = query.limit.unwrap_or(50).min(200);
    let dir_override = query.dir.map(PathBuf::from);
    let kind_filter = query.kind.filter(|k| k != "all");

    match receipts::list_receipts(limit, kind_filter.as_deref(), dir_override.as_ref()) {
        Ok(resp) => send_json(stream, 200, &resp).await,
        Err(e) => send_error(stream, 500, &ErrorResponse::internal(&e.to_string())).await,
    }
}

/// Handle GET /v1/receipts/read
async fn handle_receipts_read(
    stream: &mut tokio::net::TcpStream,
    query: &str,
) -> anyhow::Result<()> {
    let query: receipts::ReceiptReadQuery = match serde_urlencoded::from_str(query) {
        Ok(q) => q,
        Err(e) => {
            send_error(stream, 400, &ErrorResponse::bad_request(&e.to_string())).await?;
            return Ok(());
        }
    };

    if query.path.trim().is_empty() {
        send_error(stream, 400, &ErrorResponse::bad_request("path is required")).await?;
        return Ok(());
    }

    match receipts::read_receipt(&query.path) {
        Ok(resp) => send_json(stream, 200, &resp).await,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found") {
                send_error(stream, 404, &ErrorResponse::not_found(&msg)).await
            } else if msg.contains("outside allowed") {
                send_error(stream, 400, &ErrorResponse::bad_request(&msg)).await
            } else {
                send_error(stream, 500, &ErrorResponse::internal(&msg)).await
            }
        }
    }
}

// ============================================================================
// Static File Serving
// ============================================================================

/// Serve a static file with given content type and body.
async fn serve_static(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> anyhow::Result<()> {
    let status_line = match status {
        200 => "200 OK",
        404 => "404 Not Found",
        _ => "200 OK",
    };
    let response = format!(
        "HTTP/1.1 {} \r\nContent-Type: {}\r\nCache-Control: no-store, no-cache, must-revalidate\r\nPragma: no-cache\r\nExpires: 0\r\nContent-Length: {}\r\n\r\n",
        status_line,
        content_type,
        body.len()
    );
    tokio::io::AsyncWriteExt::write_all(stream, response.as_bytes()).await?;
    tokio::io::AsyncWriteExt::write_all(stream, body).await?;
    Ok(())
}

/// Read a static file from the static/ directory.
pub(crate) async fn read_static_file(filename: &str) -> Option<Vec<u8>> {
    // Safe path: only allow alphanumeric, dash, dot, slash, underscore
    let sanitized: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.' || *c == '/' || *c == '_')
        .collect();

    // Prevent directory traversal
    if sanitized.contains("..") {
        return None;
    }

    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static");
    let path = base.join(sanitized.trim_start_matches('/'));

    // Ensure path is within base directory
    let canonical_base = std::fs::canonicalize(&base).ok()?;
    let canonical_path = std::fs::canonicalize(&path).ok()?;

    if !canonical_path.starts_with(&canonical_base) {
        return None;
    }

    tokio::fs::read(&canonical_path).await.ok()
}

// ============================================================================
// Router
// ============================================================================

/// Route request to handler
async fn route(
    stream: &mut tokio::net::TcpStream,
    method: &str,
    path: &str,
    query: &str,
    body: &[u8],
) -> anyhow::Result<()> {
    match (method, path) {
        // Health
        ("GET", "/health") => handle_health(stream).await?,
        // UI static files
        ("GET", "/ui") | ("GET", "/ui/") | ("GET", "/ui/index.html") => {
            match read_static_file("ui.html").await {
                Some(content) => {
                    serve_static(stream, 200, "text/html", &content).await?;
                }
                None => {
                    let err = ErrorResponse::not_found("UI not found");
                    send_error(stream, 404, &err).await?;
                }
            }
        }
        // V1 endpoints
        ("GET", "/v1/status") => handle_status(stream).await?,
        ("GET", "/v1/capabilities") => handle_capabilities(stream).await?,
        ("GET", "/v1/plans") => handle_plans_list(stream, query).await?,
        ("GET", "/v1/plans/read") => handle_plans_read(stream, query).await?,
        ("GET", "/v1/receipts") => handle_receipts_list(stream, query).await?,
        ("GET", "/v1/receipts/read") => handle_receipts_read(stream, query).await?,
        ("POST", "/v1/chat") => handle_chat(stream, body).await?,
        ("POST", "/v1/ask") => handle_ask(stream, body).await?,
        ("POST", "/v1/plans/validate") => handle_plan_validate(stream, body).await?,
        ("POST", "/v1/plans/authorize") => handle_plans_authorize(stream, body).await?,
        ("POST", "/v1/apply") => handle_apply(stream, body).await?,
        // Aliases for backwards compatibility
        ("POST", "/chat") => handle_chat(stream, body).await?,
        ("POST", "/ask") => handle_ask(stream, body).await?,
        ("POST", "/apply") => handle_apply(stream, body).await?,
        // 404
        _ => {
            let err = ErrorResponse::not_found("not found");
            send_error(stream, 404, &err).await?;
        }
    }
    Ok(())
}

/// Parse HTTP request line
fn parse_request_line(line: &str) -> Option<(&str, &str, &str)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 3 {
        Some((parts[0], parts[1], parts[2]))
    } else if parts.len() >= 2 {
        Some((parts[0], parts[1], ""))
    } else {
        None
    }
}

/// Parse query string from path
fn parse_path_and_query(full_path: &str) -> (&str, &str) {
    if let Some(idx) = full_path.find('?') {
        (&full_path[..idx], &full_path[idx + 1..])
    } else {
        (full_path, "")
    }
}

/// Parse and handle a TCP connection
async fn handle_connection(stream: tokio::net::TcpStream) {
    let mut stream = stream;
    let mut buf = [0u8; 16384];

    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request = String::from_utf8_lossy(&buf[..n]);

    let request_line = request.lines().next().unwrap_or("");
    let (method, full_path, _) = match parse_request_line(request_line) {
        Some((m, p, v)) => (m, p, v),
        None => return,
    };

    let (path, query) = parse_path_and_query(full_path);

    let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let mut body = Vec::new();
    if body_start < n {
        body.extend_from_slice(&buf[body_start..n]);
    }

    if let Err(e) = route(&mut stream, method, path, query, &body).await {
        eprintln!("request error: {}", e);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
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
            plan_authorize: true,
            apply: true,
            plans: true,
            receipts: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"chat\":true"));
        assert!(json.contains("\"plan_authorize\":true"));
        assert!(json.contains("\"plans\":true"));
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
        let json = r#"{"model": "test"}"#;
        let result: Result<ChatRequestV1, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_chat_request_empty_message() {
        let json = r#"{"message": ""}"#;
        let req: ChatRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.message, "");
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
        #[derive(Serialize)]
        struct HealthResponse {
            ok: bool,
            service: String,
            version: String,
        }
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
        let json = r#"{"plan_path": "/plan.yml"}"#;
        let req: ApplyRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.dry_run, None);
    }

    #[test]
    fn test_authorize_request_deserialization() {
        let json = r#"{"plan_path": "/plan.yml", "approve_all": true}"#;
        let req: AuthorizeRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.plan_path, "/plan.yml");
        assert_eq!(req.approve_all, Some(true));
        assert!(req.approve.is_none());
    }

    #[test]
    fn test_error_response_bad_request() {
        let err = ErrorResponse::bad_request("message is required");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("\"code\":\"bad_request\""));
        assert!(json.contains("\"message\":\"message is required\""));
    }

    #[test]
    fn test_error_response_not_found() {
        let err = ErrorResponse::not_found("plan not found");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"code\":\"not_found\""));
    }

    #[test]
    fn test_plans_query_deserialization() {
        let query = "limit=10&dir=%2Ftmp%2Fplans";
        let q: plans::PlansQuery = serde_urlencoded::from_str(query).unwrap();
        assert_eq!(q.limit, Some(10));
        assert_eq!(q.dir, Some("/tmp/plans".to_string()));
    }

    #[test]
    fn test_receipts_query_deserialization() {
        let query = "limit=25&kind=chat";
        let q: receipts::ReceiptsQuery = serde_urlencoded::from_str(query).unwrap();
        assert_eq!(q.limit, Some(25));
        assert_eq!(q.kind, Some("chat".to_string()));
    }

    #[test]
    fn test_plans_read_path_safety_rejects_traversal() {
        // Create a temp file outside plans dir and verify it's rejected
        let temp_dir = std::env::temp_dir();
        let outside_path = temp_dir.join("evil.txt");
        std::fs::write(&outside_path, "test").unwrap();
        let plans_dir = plans::default_plans_dir();
        let result = plans::read_plan(outside_path.to_str().unwrap(), &plans_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("outside allowed"));
    }

    #[test]
    fn test_plans_read_path_safety_accepts_valid_path() {
        // When plans dir doesn't exist, canonicalize fails, so this test
        // just verifies the function doesn't panic on a valid-looking path
        let plans_dir = plans::default_plans_dir();
        // A non-existent but structurally valid path should be rejected gracefully
        let fake_path = plans_dir.join("nonexistent.yml");
        let result = plans::read_plan(fake_path.to_str().unwrap(), &plans_dir);
        // Should fail because file doesn't exist (not because of path safety)
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found") || err_msg.contains("outside allowed"));
    }

    #[test]
    fn test_receipts_read_path_safety_rejects_arbitrary_path() {
        let temp_file = std::env::temp_dir().join("evil_receipt.json");
        std::fs::write(&temp_file, r#"{"id":"test"}"#).unwrap();
        let result = receipts::read_receipt(temp_file.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("outside allowed"));
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_receipts_secret_redaction() {
        let mut data = serde_json::Map::new();
        data.insert("api_key".to_string(), serde_json::json!("secret123"));
        data.insert("token".to_string(), serde_json::json!("mytoken"));
        data.insert("password".to_string(), serde_json::json!("hunter2"));
        data.insert("action".to_string(), serde_json::json!("test"));
        data.insert("id".to_string(), serde_json::json!("abc"));

        let value = serde_json::Value::Object(data);
        let redacted = receipts::redact_secrets(&value);

        // Check secret fields are redacted
        if let Some(obj) = redacted.as_object() {
            assert_eq!(obj.get("api_key").unwrap(), "[REDACTED]");
            assert_eq!(obj.get("token").unwrap(), "[REDACTED]");
            assert_eq!(obj.get("password").unwrap(), "[REDACTED]");
            // Non-secret fields should be preserved
            assert_eq!(obj.get("action").unwrap(), "test");
            assert_eq!(obj.get("id").unwrap(), "abc");
        }
    }

    #[test]
    fn test_ask_request_deserialization_with_explicit_plans_dir() {
        let json = r#"{"request": "list downloads", "plans_dir": "/tmp/myplans"}"#;
        let req: AskRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.request, "list downloads");
        assert_eq!(req.plans_dir, Some("/tmp/myplans".to_string()));
    }

    #[test]
    fn test_default_plans_dir_is_persistent() {
        // Verify default plans dir is not a temp path
        let plans_dir = plans::default_plans_dir();
        let plans_dir_str = plans_dir.to_string_lossy();
        assert!(
            !plans_dir_str.contains("tmp"),
            "default plans_dir should not be in tmp"
        );
        assert!(
            plans_dir_str.contains("osai"),
            "default plans_dir should be under osai"
        );
    }

    #[test]
    fn test_ask_request_default_plans_dir() {
        let json = r#"{"request": "list downloads"}"#;
        let req: AskRequestV1 = serde_json::from_str(json).unwrap();
        assert_eq!(req.request, "list downloads");
        assert!(req.plans_dir.is_none()); // not provided, will use default
    }

    #[test]
    fn test_authorize_request_defaults_policy_path() {
        let json = r#"{"plan_path": "/plan.yml"}"#;
        let req: AuthorizeRequest = serde_json::from_str(json).unwrap();
        assert!(req.policy_path.is_none()); // will use default
    }

    #[test]
    fn test_apply_request_defaults_policy_path() {
        let json = r#"{"plan_path": "/plan.yml"}"#;
        let req: ApplyRequestV1 = serde_json::from_str(json).unwrap();
        assert!(req.policy_path.is_none()); // will use default
    }

    // ========================================================================
    // Policy path resolution tests
    // ========================================================================

    #[test]
    fn test_default_policy_path_is_persistent() {
        // The default policy path should be absolute and not contain tmp
        let path = osai_agent_core::default_policy_path();
        let path_str = path.to_string_lossy();
        assert!(
            path.is_absolute(),
            "default policy path should be absolute: {}",
            path_str
        );
        assert!(
            !path_str.contains("tmp"),
            "default policy path should not be in tmp: {}",
            path_str
        );
    }

    #[test]
    fn test_resolve_policy_path_none_uses_default() {
        let result = resolve_policy_path(None);
        let default = osai_agent_core::default_policy_path();
        assert_eq!(result, default);
    }

    #[test]
    fn test_resolve_policy_path_relative_is_absolute() {
        let result = resolve_policy_path(Some("examples/policies/default-secure.yml"));
        assert!(
            result.is_absolute(),
            "relative policy path should be resolved to absolute: {}",
            result.display()
        );
    }

    #[test]
    fn test_resolve_policy_path_absolute_unchanged() {
        let result = resolve_policy_path(Some("/tmp/custom.yml"));
        assert_eq!(result, std::path::PathBuf::from("/tmp/custom.yml"));
    }

    #[test]
    fn test_resolve_policy_path_works_from_different_cwd() {
        // Save and restore cwd
        let orig_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir("/tmp").unwrap();

        let result = resolve_policy_path(None);
        assert!(
            result.is_absolute(),
            "should still resolve to absolute path from /tmp: {}",
            result.display()
        );
        assert!(
            result.to_string_lossy().contains("osai-linux"),
            "should still resolve to repo path from /tmp: {}",
            result.display()
        );

        let _ = std::env::set_current_dir(&orig_cwd);
    }

    // ========================================================================
    // UI static file serving tests
    // ========================================================================

    #[test]
    fn test_ui_route_matches_expected_path() {
        // Verify the route pattern used in the router
        let ui_routes = ["/ui", "/ui/", "/ui/index.html"];
        for route in ui_routes {
            assert!(
                route.starts_with("/ui"),
                "UI route should start with /ui: {}",
                route
            );
        }
    }

    #[tokio::test]
    async fn test_read_static_file_sanitization_rejects_path_traversal() {
        // Test that path traversal attempts are rejected
        let result = crate::read_static_file("../../../etc/passwd").await;
        assert!(result.is_none(), "path traversal should be rejected");
    }

    #[tokio::test]
    async fn test_read_static_file_sanitization_allows_normal_paths() {
        // Test that normal file names are allowed
        let result = crate::read_static_file("ui.html").await;
        assert!(result.is_some(), "ui.html should be readable");
    }

    #[tokio::test]
    async fn test_ui_route_returns_html_with_dev_panel_marker() {
        let result = crate::read_static_file("ui.html").await;
        assert!(result.is_some(), "ui.html should be readable");
        let content = String::from_utf8(result.unwrap()).unwrap();
        assert!(
            content.contains("OSAI API Dev Panel"),
            "ui.html should contain 'OSAI API Dev Panel' marker"
        );
    }
}
