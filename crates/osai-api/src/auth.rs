//! Local token-based authentication for osai-api.
//!
//! Token source priority:
//! 1. OSAI_API_TOKEN env var (if set)
//! 2. ~/.config/osai/api-token file (if exists, or created with secure random)
//!
//! Security properties:
//! - Token is never logged.
//! - Token is never written to receipts.
//! - Token is never stored in browser localStorage/sessionStorage.

use std::path::PathBuf;
use std::sync::OnceLock;

static TOKEN_CACHE: OnceLock<Option<String>> = OnceLock::new();

/// Path to the local API token file.
pub fn token_file_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".config").join("osai").join("api-token"))
        .unwrap_or_else(|| PathBuf::from("/tmp/osai-api-token"))
}

/// Get the API token, trying env var first, then the token file.
/// Returns None if no token is configured (auth disabled).
pub fn get_token() -> Option<String> {
    // Check env var first
    if let Ok(env_token) = std::env::var("OSAI_API_TOKEN") {
        if !env_token.is_empty() {
            return Some(env_token);
        }
    }

    // Check cached token (from file)
    TOKEN_CACHE.get_or_init(|| read_token_file().ok()).clone()
}

fn read_token_file() -> std::io::Result<String> {
    let path = token_file_path();
    std::fs::read_to_string(path)
}

/// Token source for /v1/auth/status response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    /// Token was set via OSAI_API_TOKEN env var.
    Env,
    /// Token was loaded from ~/.config/osai/api-token file.
    File,
    /// No token configured — auth is disabled.
    Disabled,
}

impl TokenSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenSource::Env => "env",
            TokenSource::File => "file",
            TokenSource::Disabled => "disabled",
        }
    }
}

/// Describe where the token comes from.
pub fn token_source() -> TokenSource {
    if std::env::var("OSAI_API_TOKEN")
        .map(|v| !v.is_empty())
        .unwrap_or(false)
    {
        TokenSource::Env
    } else if token_file_path().exists() {
        TokenSource::File
    } else {
        TokenSource::Disabled
    }
}

/// Check if a given token is valid.
pub fn validate_token(provided: &str) -> bool {
    match get_token() {
        Some(ref expected) => constant_time_eq(provided.as_bytes(), expected.as_bytes()),
        None => false,
    }
}

/// Constant-time byte comparison to prevent timing attacks.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Auth status response body.
#[derive(Debug, serde::Serialize)]
pub struct AuthStatusResponse {
    pub ok: bool,
    pub auth_required: bool,
    pub token_source: String,
}

impl AuthStatusResponse {
    pub fn new() -> Self {
        let source = token_source();
        Self {
            ok: true,
            auth_required: source != TokenSource::Disabled,
            token_source: source.as_str().to_string(),
        }
    }
}

impl Default for AuthStatusResponse {
    fn default() -> Self {
        Self::new()
    }
}
