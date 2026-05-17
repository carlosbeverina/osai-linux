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

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static TOKEN_CACHE: OnceLock<Option<String>> = OnceLock::new();

/// Path to the local API token file.
pub fn token_file_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".config").join("osai").join("api-token"))
        .unwrap_or_else(|| PathBuf::from("/tmp/osai-api-token"))
}

fn env_token_is_set() -> bool {
    std::env::var("OSAI_API_TOKEN")
        .map(|token| !token.trim().is_empty())
        .unwrap_or(false)
}

/// Get the API token, trying env var first, then the token file.
/// Returns None if no token is configured.
pub fn get_token() -> Option<String> {
    // Check env var first.
    if let Ok(env_token) = std::env::var("OSAI_API_TOKEN") {
        let trimmed = env_token.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Check cached token (from file).
    TOKEN_CACHE
        .get_or_init(|| read_token_file().ok().filter(|token| !token.is_empty()))
        .clone()
}

fn read_token_file() -> io::Result<String> {
    let path = token_file_path();
    std::fs::read_to_string(path).map(|s| s.trim().to_string())
}

fn generate_token() -> String {
    // 256 bits of randomness from two UUIDv4 values is sufficient for a local
    // bearer token and avoids adding another dependency.
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn write_token_file(path: &Path, token: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(token.as_bytes())?;
        file.write_all(b"\n")?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(path, format!("{}\n", token))?;
    }

    Ok(())
}

/// Ensure a token file exists, creating it with restrictive permissions where
/// supported. Does not print or return the token value.
pub fn ensure_token_file() -> io::Result<()> {
    ensure_token_file_at(&token_file_path())
}

/// Ensure a token file exists only when no non-empty `OSAI_API_TOKEN` is set.
///
/// This is the startup path for osai-api: explicit environment tokens take
/// precedence and must not cause a local token file to be created.
pub fn ensure_token_file_if_needed() -> io::Result<()> {
    if env_token_is_set() {
        return Ok(());
    }

    ensure_token_file()
}

fn ensure_token_file_at(path: &Path) -> io::Result<()> {
    if path.exists() {
        return Ok(());
    }

    let token = generate_token();
    match write_token_file(path, &token) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e),
    }
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

/// Describe where the token comes from. This does not create a token file;
/// startup is responsible for calling `ensure_token_file_if_needed`.
pub fn token_source() -> TokenSource {
    if env_token_is_set() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_home() -> PathBuf {
        std::env::temp_dir().join(format!("osai-api-home-test-{}", uuid::Uuid::new_v4()))
    }

    fn with_isolated_home<T>(env_token: Option<&str>, test: impl FnOnce(PathBuf) -> T) -> T {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_home = std::env::var_os("HOME");
        let old_token = std::env::var_os("OSAI_API_TOKEN");
        let home = temp_home();
        std::fs::create_dir_all(&home).unwrap();

        std::env::set_var("HOME", &home);
        match env_token {
            Some(token) => std::env::set_var("OSAI_API_TOKEN", token),
            None => std::env::remove_var("OSAI_API_TOKEN"),
        }

        let result = test(home.clone());

        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match old_token {
            Some(value) => std::env::set_var("OSAI_API_TOKEN", value),
            None => std::env::remove_var("OSAI_API_TOKEN"),
        }
        let _ = std::fs::remove_dir_all(home);

        result
    }

    #[test]
    fn test_ensure_token_file_at_creates_restrictive_file() {
        let path =
            std::env::temp_dir().join(format!("osai-api-token-test-{}", uuid::Uuid::new_v4()));

        ensure_token_file_at(&path).unwrap();

        let token = std::fs::read_to_string(&path).unwrap();
        assert!(token.trim().len() >= 64);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_ensure_token_file_if_needed_skips_file_when_env_token_set() {
        with_isolated_home(Some("env-token-value"), |home| {
            ensure_token_file_if_needed().unwrap();

            let token_path = home.join(".config").join("osai").join("api-token");
            assert!(
                !token_path.exists(),
                "token file should not be created when OSAI_API_TOKEN is set"
            );
            assert_eq!(token_source(), TokenSource::Env);
        });
    }

    #[test]
    fn test_ensure_token_file_if_needed_creates_file_when_env_token_unset() {
        with_isolated_home(None, |home| {
            ensure_token_file_if_needed().unwrap();

            let token_path = home.join(".config").join("osai").join("api-token");
            assert!(token_path.exists());
            let token = std::fs::read_to_string(&token_path).unwrap();
            assert!(token.trim().len() >= 64);
            assert_eq!(token_source(), TokenSource::File);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&token_path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600);
            }
        });
    }

    #[test]
    fn test_ensure_token_file_if_needed_creates_file_when_env_token_empty() {
        with_isolated_home(Some("  	  "), |home| {
            ensure_token_file_if_needed().unwrap();

            let token_path = home.join(".config").join("osai").join("api-token");
            assert!(token_path.exists());
            assert_eq!(token_source(), TokenSource::File);
        });
    }

    #[test]
    fn test_auth_status_reports_env_without_returning_token_value() {
        let secret = "super-secret-env-token";
        with_isolated_home(Some(secret), |_| {
            let resp = AuthStatusResponse::new();
            assert!(resp.ok);
            assert!(resp.auth_required);
            assert_eq!(resp.token_source, "env");

            let json = serde_json::to_string(&resp).unwrap();
            assert!(!json.contains(secret));
        });
    }
}
