//! OSAI Agent Core - Reusable logic for chat, ask, and apply operations.
//!
//! This crate provides the core functionality used by osai-agent-cli.
//! It is NOT a standalone binary - use osai-agent-cli for the CLI interface.

pub mod apply;
pub mod ask;
pub mod chat;
pub mod shared;

pub use apply::{run_apply, ApplyResult};
pub use ask::{ask_core_async, run_ask, AskResult};
pub use chat::{chat_core_async, run_chat, ChatResult};
pub use shared::{is_loopback_url, step_to_request, ChatRequest, ChatResponse};
