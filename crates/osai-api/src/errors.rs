//! Consistent JSON error handling for osai-api.

use serde::Serialize;

/// Standard error response body.
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

/// Standard error response envelope.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub ok: bool,
    pub error: ErrorBody,
}

impl ErrorResponse {
    /// Create a new error response.
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            ok: false,
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
            },
        }
    }

    /// Bad request (400).
    pub fn bad_request(message: &str) -> Self {
        Self::new("bad_request", message)
    }

    /// Not found (404).
    pub fn not_found(message: &str) -> Self {
        Self::new("not_found", message)
    }

    /// Internal server error (500).
    pub fn internal(message: &str) -> Self {
        Self::new("internal_error", message)
    }
}

/// Send a JSON error response over a TCP stream.
pub async fn send_error<S>(stream: &mut S, status: u16, err: &ErrorResponse) -> anyhow::Result<()>
where
    S: tokio::io::AsyncWrite + Unpin,
{
    let json = serde_json::to_string(err)
        .unwrap_or_else(|_| r#"{"ok":false,"error":{"code":"internal_error","message":"failed to serialize error"}}"#.to_string());
    let body = format!(
        "HTTP/1.1 {} \r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        match status {
            400 => "400 Bad Request",
            404 => "404 Not Found",
            405 => "405 Method Not Allowed",
            500 => "500 Internal Server Error",
            _ => "500 Internal Server Error",
        },
        json.len(),
        json
    );
    tokio::io::AsyncWriteExt::write_all(stream, body.as_bytes()).await?;
    Ok(())
}
