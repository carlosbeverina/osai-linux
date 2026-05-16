# osai-agent-core Plan

## Why osai-agent-core Is Next

osai-agent-core extraction is the first engineering task after documentation. The goal is to move the core logic from `osai-agent-cli/src/main.rs` into a reusable library that both the CLI and future osai-api can use.

**Current problem**: CLI logic and osai-api logic are in separate places. If we change how chat/ask/apply works, we have to update both places. Or osai-api shells out to the CLI, which is the wrong architecture.

**Solution**: Extract a shared library. Both CLI and osai-api call the same functions.

## Goals

1. **Refactor only** — Preserve existing behavior exactly
2. **Clean public API** — Clear interface for CLI and API to use
3. **Tests pass** — All existing tests must still pass
4. **CLI unchanged** — Users cannot tell the difference

## What Is osai-agent-core?

A Rust library crate (`crates/osai-agent-core/`) containing:

- Chat logic (`chat_core_async`)
- Ask/plan generation logic (`ask_core_async`)
- Apply logic (`run_apply`)
- Runtime status collection (`collect_runtime_status_async`)
- Shared types (ChatResult, AskResult, etc.)

## What Stays in osai-agent-cli

Thin CLI wrapper:

- `clap` argument parsing
- Output formatting (human vs JSON)
- Exit code handling
- Help text
- Examples

## What Moves to osai-agent-core

### From osai-agent-cli/src/main.rs

Functions to extract:

```rust
// chat_core_async — handles Model Router chat request
pub async fn chat_core_async(
    message: &str,
    model_router_url: &str,
    receipts_dir: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
) -> Result<ChatResult>

// ask_core_async — handles ask request, plan generation
pub async fn ask_core_async(
    request: &str,
    model_router_url: &str,
    receipts_dir: Option<&Path>,
    plans_dir: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
) -> Result<AskResult>

// run_apply — handles validate, authorize, apply flow
pub fn run_apply(
    plan_path: &Path,
    policy_path: &Path,
    receipts_dir: Option<&Path>,
    allowed_roots: &[PathBuf],
    approve: &[String],
    approve_all: bool,
    model_router_url: Option<&str>,
    dry_run: bool,
    force_no_approve: bool,
) -> Result<()>
```

### From osai-agent-core/src/runtime.rs (already exists)

```rust
pub async fn collect_runtime_status_async() -> RuntimeStatus
pub fn collect_runtime_status_sync() -> RuntimeStatus  // CLI uses this
```

### Result Types to Export

```rust
pub struct ChatResult {
    pub status: String,
    pub content: Option<String>,
    pub response_length: Option<usize>,
    pub error: Option<String>,
}

pub struct AskResult {
    pub status: String,
    pub output_path: Option<PathBuf>,
    pub validation: String,
    pub error: Option<String>,
}

// ApplyResult via direct execution, no separate struct
```

## Proposed API

```rust
// crates/osai-agent-core/src/lib.rs

pub mod chat {
    pub async fn chat_core_async(...) -> Result<ChatResult>
}

pub mod ask {
    pub async fn ask_core_async(...) -> Result<AskResult>
}

pub mod apply {
    pub fn run_apply(...) -> Result<()>
}

pub mod runtime {
    pub async fn collect_runtime_status_async() -> RuntimeStatus
    pub fn collect_runtime_status_sync() -> RuntimeStatus
}

// Shared
pub use osai_plan_dsl::OsaiPlan;
pub use osai_receipt_logger::Store as ReceiptStore;
pub use osai_toolbroker::ToolBroker;
```

## Module Structure

```
crates/osai-agent-core/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Main entry, exports
│   ├── chat.rs             # chat_core_async
│   ├── ask.rs              # ask_core_async
│   ├── apply.rs            # run_apply
│   └── runtime.rs          # collect_runtime_status_async (already exists)
```

## Implementation Order

### Step 1: Create osai-agent-core

```bash
mkdir -p crates/osai-agent-core/src
touch crates/osai-agent-core/Cargo.toml
touch crates/osai-agent-core/src/lib.rs
```

Cargo.toml dependencies:
- `osai-plan-dsl`
- `osai-receipt-logger`
- `osai-toolbroker`
- `osai-tool-executor`
- `tokio` (workspace)
- `reqwest` (for Model Router calls)
- `serde`, `serde_json`, `serde_yaml`
- `anyhow`

### Step 2: Move chat_core_async

1. Identify `chat_core_async` function in CLI main.rs
2. Copy to `osai-agent-core/src/chat.rs`
3. Update imports
4. Export from `lib.rs`
5. Update CLI to call osai-agent-core::chat::chat_core_async
6. Verify behavior unchanged
7. Run tests

### Step 3: Move ask_core_async

1. Identify `ask_core_async` function in CLI main.rs
2. Copy to `osai-agent-core/src/ask.rs`
3. Update imports
4. Export from `lib.rs`
5. Update CLI to call osai-agent-core::ask::ask_core_async
6. Verify behavior unchanged
7. Run tests

### Step 4: Move run_apply

1. Identify `run_apply` function in CLI main.rs
2. Copy to `osai-agent-core/src/apply.rs`
3. Update imports
4. Export from `lib.rs`
5. Update CLI to call osai-agent-core::apply::run_apply
6. Verify behavior unchanged
7. Run tests

### Step 5: Runtime status (already exists)

The runtime status collection already exists in `osai-agent-core/src/runtime.rs`. It will be exported from `lib.rs`.

### Step 6: Final verification

```bash
cargo test --workspace
cargo run -p osai-agent-cli -- chat "test"
cargo run -p osai-agent-cli -- ask --print-plan "test"
```

## Testing Requirements

- All existing CLI tests must pass
- All osai-agent-core tests must pass
- MVP loop must work unchanged
- No regression in receipts format

## Acceptance Criteria

1. **CLI works identically** — `cargo run -p osai-agent-cli -- chat "test"` produces same output
2. **MVP loop preserved** — chat, ask, plan validate, apply all work
3. **Tests pass** — `cargo test --workspace` passes
4. **Clean API** — osai-agent-core exports clear public functions
5. **No behavior change** — This is a refactor, not a feature change
6. **osai-api can use it** — Future osai-api will call osai-agent-core, not shell out

## What Not to Do

- **Do not add features** — This is refactor only
- **Do not change function signatures** — Unless necessary for the extraction
- **Do not change behavior** — Only move code
- **Do not break CLI** — Users should not notice any difference
- **Do not skip tests** — All tests must pass before declaring success