# AGENTS.md — Instructions for AI Coding Agents

## Repository Purpose

OSAI (Open Source AI) is a local-first AI operating system and assistant layer built on Linux. The goal is an installable Linux-based system with an OSAI desktop/session, a local assistant, secure authorization, controlled execution, receipts/auditability, local model runtime, optional cloud fallback, and eventually computer-use capabilities.

## Current MVP State

OSAI currently exists as a validated MVP with:

- **Rust workspace** with CLI tools (`osai-agent-cli`), Plan DSL, ToolBroker, ToolExecutor, Receipt Logger
- **Python Model Router** (FastAPI) under `services/model-router`, with OpenAI-compatible endpoints
- **llama.cpp** as the default local runtime
- **Gemma 4 E2B Q8 GGUF** as the default local model
- **Validated MVP loop**: chat, ask, plan validate, apply --dry-run, apply real

## Mandatory Safety Constraints

1. **Never allow the model or agents to execute host shell commands directly.** Every action must pass through ToolBroker.
2. **Every action must produce a receipt.** Receipts must not store full prompts, API keys, tokens, or secrets.
3. **Do not store secrets in the repository.** Do not add API keys, tokens, private files, or credentials to Git.
4. **Do not modify or commit local models or runtimes.** Never touch `.local-models/` or `.local-runtimes/`.
5. **Loopback-only for local services.** Model Router and local backends must bind to 127.0.0.1 only.
6. **Destructive tools must not be made real without security review.** FilesWrite, FilesMove, FilesDelete require approval and security review before live execution.
7. **CLI behavior must remain backward compatible.** Existing commands and flags must not be broken.
8. **Generated docs and code must avoid secrets.** Never echo tokens, passwords, or credentials in output.
9. **Future UI must call osai-api/osai-agent-core, not shell out to the CLI.** This is the architecture rule for UI integration.

## What Already Works

- `cargo run -p osai-agent-cli -- chat "message"` — local chat via Model Router
- `cargo run -p osai-agent-cli -- ask --print-plan "request"` — plan generation from natural language
- `cargo run -p osai-agent-cli -- plan validate <plan>` — Plan DSL validation
- `cargo run -p osai-agent-cli -- apply <plan> --dry-run` — dry-run apply
- `cargo run -p osai-agent-cli -- apply <plan>` — real apply with ToolBroker/ToolExecutor
- `./scripts/osai-local-up` / `./scripts/osai-local-check` / `./scripts/osai-local-down` — local runtime orchestration
- `pytest tests` in `services/model-router` — Model Router tests

## Mandatory Tests Before Every Change

```bash
# Rust workspace
cargo fmt --check
cargo check --workspace
cargo test --workspace

# Model Router
cd services/model-router && pytest tests

# Local runtime (if services are running)
./scripts/osai-local-check
```

## Docs-First Behavior

For large architectural changes, update the documentation files first (in `docs/`) before writing code. This ensures the design intent is clear and reviewable.

## Current/Next Work Context

The following are partially done or not yet implemented:

1. **osai-agent-core** — partially extracted (chat.rs, ask.rs, apply.rs, runtime.rs, shared.rs exist); full extraction and clean public API remain next
2. **osai-api** — prototype exists (auth guard, endpoints, Dev Panel UI at port 8090); full endpoint implementation and core integration remain
3. **Local dev UI/control panel** — Dev Panel prototype exists; completion and polish remain
4. **Desktop/session integration** — future (COSMIC, KDE, or GNOME shell integration — the final UI target)
5. **Computer-use subsystem** — future (visible and hidden/isolated modes with safety design)
6. **Voice/multimodal** — future (voice pipeline, screenshot understanding)
7. **Packaging/installable OS** — future (Fedora Atomic / Universal Blue / BlueBuild based image)

## Security Rules Summary

- Model output is **untrusted** — always validated through Plan DSL and ToolBroker
- ToolBroker is the **authorization layer** — ToolExecutor must not bypass it
- Receipts must **not leak secrets** — prompts, API keys, tokens redacted
- Cloud model use must be **explicit and policy-controlled**
- Shell sandboxing has risks — `ShellRunSandboxed` is the only permitted shell action
- File operations are constrained by **allowed roots**
- Computer-use changes require extra safety review: visible/hidden mode isolation, receipts, approvals, no hidden exfiltration

## What Must Not Be Broken

- The MVP loop (chat, ask, plan validate, apply)
- Existing CLI commands and flags
- Receipt format and privacy properties
- ToolBroker authorization flow
- Test suite pass rate
- Local runtime scripts (`osai-local-up`, `osai-local-check`, `osai-local-down`)

## Local Development Commands

```bash
cd ~/Projects/osai-linux
source "$HOME/.cargo/env"

# Format and type check
cargo fmt --check
cargo check --workspace

# Run tests
cargo test --workspace

# Model Router tests
cd services/model-router && pytest tests

# Local runtime (requires llama.cpp running)
./scripts/osai-local-up
./scripts/osai-local-check
./scripts/osai-local-down
```

## Receipt Locations

- Chat: `~/.local/share/osai/receipts/chat`
- Ask: `~/.local/share/osai/receipts/ask`
- Apply: `~/.local/share/osai/receipts/apply`
- Model Router: `~/.local/share/osai/receipts/model-router`
- Tool receipts: configured via `OSAI_LOCAL_TOOL_RECEIPTS_DIR`

## Important Paths (Do Not Commit)

- `.local-models/` — local model files (GGUF, checkpoints)
- `.local-runtimes/` — llama.cpp, vLLM, other local runtimes
- `~/.config/osai/api-token` — local API token (auto-generated, chmod 0600)
- `~/.local/share/osai/receipts/` — all receipt directories

## Current Repository Structure

```
crates/
  osai-agent-cli/     # CLI tool with chat, ask, apply, plan, tool commands
  osai-agent-core/    # (partially extracted; chat/ask/apply extraction in progress)
  osai-plan-dsl/      # Plan YAML/JSON validation and types
  osai-toolbroker/    # Authorization layer
  osai-tool-executor/ # Action execution
  osai-receipt-logger/ # Receipt storage

services/
  model-router/       # FastAPI OpenAI-compatible gateway

scripts/
  osai-local-*        # Local runtime orchestration
  osai-llamacpp-*    # llama.cpp runtime scripts
  osai-vllm-*         # vLLM runtime scripts (optional)

systemd/user/         # systemd user services for local runtime
```

## Key Files

- `CLAUDE.md` — Project context and development rules
- `docs/ARCHITECTURE.md` — Current and target architecture
- `docs/CURRENT_STATE.md` — Current implemented state
- `docs/ROADMAP.md` — Phased roadmap
- `examples/plans/` — Example OSAI plans
- `examples/policies/` — Example authorization policies