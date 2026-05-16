# CODEX.md — Instructions for Codex Web (GitHub AI Agent)

## How to Approach This Repository

You are working on OSAI: a local-first AI operating system built on Linux. The goal is an installable Linux-based system with an OSAI desktop/session, secure authorization, controlled execution, receipts/auditability, local model runtime, optional cloud fallback, and computer-use capabilities.

You will be given tasks via GitHub issues or PRs. Your job is to understand the codebase, implement changes safely, and produce small reviewable PRs.

## Current Implemented State

Verify what exists before assuming anything:

- ✅ `osai-agent-cli` — Rust CLI with chat, ask, apply, plan, tool commands
- ✅ `osai-plan-dsl` — Plan YAML/JSON validation and types
- ✅ `osai-toolbroker` — Authorization layer
- ✅ `osai-tool-executor` — Action execution
- ✅ `osai-receipt-logger` — Receipt storage
- ✅ `services/model-router` — FastAPI Model Router (llama.cpp, vLLM, MiniMax)
- ✅ `scripts/osai-local-*` — Local runtime orchestration
- ✅ `osai-agent-core` — **Partially extracted** (chat.rs, ask.rs, apply.rs, runtime.rs, shared.rs; extraction ongoing)
- ✅ `osai-api` — **Prototype exists** (auth guard, endpoints, Dev Panel UI at port 8090)
- ✅ UI — **Dev Panel prototype** at `http://127.0.0.1:8090/ui` (not final desktop UI)

## Development Sequence

Follow this order when given new work:

1. **Preserve current MVP** — Never break chat, ask, plan validate, apply --dry-run, apply
2. **Complete osai-agent-core extraction** — full extraction and clean public API
3. **Complete osai-api** — full endpoint implementation and osai-agent-core integration
4. **Complete local dev UI/control panel** — polish Dev Panel prototype
5. **Add desktop/session integration** — COSMIC, KDE, or GNOME shell integration (final UI target)
6. **Add safe tool expansion** — More tools through ToolBroker/ToolExecutor
7. **Add computer-use subsystem** — Visible and hidden/isolated modes with safety design
8. **Add voice/multimodal** — Voice pipeline, screenshot understanding
9. **Add packaging/installable Linux system** — Fedora Atomic / Universal Blue / BlueBuild

## How to Run Tests

```bash
cd ~/Projects/osai-linux
source "$HOME/.cargo/env"

# Rust workspace
cargo fmt --check
cargo check --workspace
cargo test --workspace

# Model Router
cd services/model-router && pytest tests

# Local runtime (requires local services running)
./scripts/osai-local-up
./scripts/osai-local-check
./scripts/osai-local-down
```

## Running MVP E2E Tests

```bash
# Chat
cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI MVP chat OK"

# Ask / plan generation
cargo run -p osai-agent-cli -- ask --print-plan "Create a safe plan to list my Downloads folder"

# Plan validation
PLAN=$(find /tmp -name "create-a-safe-*.yml" 2>/dev/null | sort | tail -1)
cargo run -p osai-agent-cli -- plan validate "$PLAN"

# Apply dry-run
cargo run -p osai-agent-cli -- apply "$PLAN" --dry-run --policy examples/policies/default-secure.yml --allowed-root "$HOME/Downloads"

# Apply real
cargo run -p osai-agent-cli -- apply "$PLAN" --policy examples/policies/default-secure.yml --allowed-root "$HOME/Downloads"
```

## PR Behavior

- **Small PRs** — One logical change per PR. Easy to review, easy to revert.
- **Tests required** — `cargo test --workspace` and `pytest tests` must pass before marking PR ready.
- **Do not commit generated artifacts** — Never commit `.local-models/`, `.local-runtimes/`, or local receipt files.
- **Do not commit secrets** — Never commit API keys, tokens, or credentials.
- **Mock vs local validation** — If GPU/model tests cannot run in GitHub CI, use mock tests. Clearly document local validation steps in the PR.
- **Backward compatibility** — CLI commands and flags must not be broken.
- **API/Core architecture** — Future UI must call osai-api/osai-agent-core, not shell out to the CLI.
- **Docs update** — Architectural changes require updating docs files first.

## PR Body Format

```markdown
## Summary
1-3 bullets describing what changed and why.

## Files Changed
List of files changed with brief description.

## Tests Run
- cargo fmt --check
- cargo check --workspace
- cargo test --workspace
- pytest tests (services/model-router)

## Security Impact
What security boundaries were affected, if any.

## Runtime Impact
What runtime behavior changed, if any.

## Limitations
What is not yet implemented or tested.

## Follow-ups
What still needs to be done, if anything.
```

## Important Constraints

1. **Never touch `.local-models/` or `.local-runtimes/`** — These are local-only and must not be committed.
2. **Never store secrets in the repo** — API keys, tokens, passwords do not belong in Git.
3. **Model output is untrusted** — Plan DSL validation and ToolBroker authorization are mandatory.
4. **Receipts must not leak secrets** — Full prompts, API keys, tokens redacted.
5. **Destructive tools require security review** — FilesWrite, FilesMove, FilesDelete must not be made live without review.
6. **Loopback-only** — Local services bind to 127.0.0.1 only.
7. **Computer-use changes require extra safety design** — Visible/hidden mode isolation, receipts, approvals, no hidden exfiltration.

## What to Check Before Finishing a Task

- Current behavior preserved (MVP loop still works)
- `cargo test --workspace` passes
- `pytest tests` passes
- No secrets or local artifacts committed
- Receipts do not leak sensitive data
- CLI compatibility preserved
- Docs updated if architecture changed
- Computer-use safety design included if applicable