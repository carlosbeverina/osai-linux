//! OSAI Agent Core - Reusable logic for chat, ask, and apply operations.
//!
//! This crate provides the core functionality used by osai-agent-cli.
//! It is NOT a standalone binary - use osai-agent-cli for the CLI interface.

pub mod apply;
pub mod ask;
pub mod chat;
pub mod runtime;
pub mod shared;

pub use apply::{
    authorize_plan_preview, run_apply, ApplyResult, AuthorizePreviewResult, AuthorizeSummary,
    StepPreview,
};
pub use ask::{ask_core_async, run_ask, AskResult};
pub use chat::{chat_core_async, run_chat, ChatResult};
pub use runtime::{
    check_llamacpp_status, check_model_router_status, check_osai_api_status,
    check_systemd_services, collect_runtime_status_async, collect_runtime_status_sync,
    is_systemd_available, ComponentStatus, OverallHealth, RuntimeMode, RuntimeStatus,
    RuntimeStatusBuilder, SystemdServiceStatus, SystemdStatus,
};
pub use shared::{
    default_apply_receipts_dir, default_ask_plans_dir, default_ask_receipts_dir,
    default_chat_receipts_dir, default_policy_path, is_loopback_url, resolve_policy_path,
    step_to_request, ChatRequest, ChatResponse,
};
