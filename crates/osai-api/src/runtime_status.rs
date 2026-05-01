//! Runtime Status endpoint - GET /v1/runtime/status

use osai_agent_core::runtime::collect_runtime_status_async;
use serde::Serialize;

/// Handle GET /v1/runtime/status
pub async fn handle_runtime_status() -> impl Serialize {
    collect_runtime_status_async().await
}
