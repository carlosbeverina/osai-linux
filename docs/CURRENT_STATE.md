# Current State

## What Exists Now

This document describes what is currently implemented and validated in the OSAI repository. All items marked ✅ exist. Items marked ❌ do not exist yet.

## Rust Workspace Crates

| Crate | Status | Description |
|-------|--------|-------------|
| `osai-agent-cli` | ✅ | CLI tool with chat, ask, apply, plan, tool, doctor commands |
| `osai-plan-dsl` | ✅ | Plan YAML/JSON validation, Plan type, step types |
| `osai-toolbroker` | ✅ | Authorization layer, policy evaluation, decision logging |
| `osai-tool-executor` | ✅ | Action execution for supported tools |
| `osai-receipt-logger` | ✅ | Receipt storage, list, read, secret redaction |
| `osai-agent-core` | ⚠️ | (Partially extracted — chat.rs, ask.rs, apply.rs, runtime.rs, shared.rs; extraction ongoing) |

## Model Router

| Component | Status | Description |
|-----------|--------|-------------|
| FastAPI service | ✅ | OpenAI-compatible `/v1/chat/completions` endpoint |
| llama.cpp provider | ✅ | Local GGUF model support via llama-server |
| vLLM provider | ✅ | Optional vLLM runtime support |
| MiniMax provider | ✅ | Cloud fallback via MiniMax API |
| Receipt middleware | ✅ | Receipts for Model Router calls |
| Health/models endpoints | ✅ | `GET /health`, `GET /v1/models` |

Location: `services/model-router/`

## Local Runtime

| Component | Status | Description |
|-----------|--------|-------------|
| llama.cpp server | ✅ | CUDA-enabled llama-server |
| Gemma 4 E2B Q8 GGUF | ✅ | Default local model (4.7GB GGUF) |
| Qwen 2.5 0.5B GGUF | ✅ | Smoke-test fallback model |
| Model Router | ✅ | Gateway at `http://127.0.0.1:8088` |
| llama-server | ✅ | Local inference at `http://127.0.0.1:8092/v1` |

Local runtime scripts:
- `./scripts/osai-local-up` — Start all local services
- `./scripts/osai-local-check` — Validate all services
- `./scripts/osai-local-down` — Stop all local services
- `./scripts/osai-llamacpp-up/down/check/env` — llama.cpp standalone scripts
- `./scripts/osai-vllm-up/down/check/env` — vLLM standalone scripts (optional)

## MVP CLI Loop (Validated)

The full MVP loop has been validated end-to-end:

```bash
# 1. Chat — local chat through Model Router
cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI chat OK"

# 2. Ask — generate plan from natural language
cargo run -p osai-agent-cli -- ask --print-plan "Create a safe plan to list my Downloads folder"

# 3. Validate — validate generated plan
cargo run -p osai-agent-cli -- plan validate <generated-plan.yml>

# 4. Apply dry-run — test without execution
cargo run -p osai-agent-cli -- apply <plan> --dry-run \
  --policy examples/policies/default-secure.yml \
  --allowed-root "$HOME/Downloads"

# 5. Apply real — execute with ToolBroker/ToolExecutor
cargo run -p osai-agent-cli -- apply <plan> \
  --policy examples/policies/default-secure.yml \
  --allowed-root "$HOME/Downloads"
```

## Plan DSL

OSAI Plan DSL is a YAML-based plan representation:

```yaml
version: '0.1'
id: <uuid>
title: <string>
description: <string>
actor: user
risk: Low|Medium|High|Critical
approval: Auto|Ask|Never
steps:
  - id: <string>
    action:
      type: FilesList|FilesWrite|FilesMove|ModelChat|...
    description: <string>
    requires_approval: <bool>
    inputs: {}
rollback: <plan>|null
metadata: {}
```

Supported action types:
- `FilesList` — List directory contents
- `FilesMove` — Move files (requires approval, not yet real)
- `FilesWrite` — Write files (requires approval, not yet real)
- `ModelChat` — Chat with model
- `ReceiptCreate` — Create receipt
- `DesktopNotify` — Send desktop notification
- `BrowserOpenUrl` — Open URL in browser (requires approval)
- `ShellRunSandboxed` — Run command in sandbox (requires approval)
- `Custom` — Custom action type

## Policy System

Policies define what actions are allowed, denied, or require approval:

```yaml
rules:
  - match:
      action: ShellRunSandboxed
    decision: Ask  # Always ask
  - match:
      action: FilesWrite
    decision: Deny  # Not yet safe to execute
```

Default policy: `examples/policies/default-secure.yml`

## Receipts

Receipts are generated for every action (chat, ask, apply, tool execution, Model Router calls):

- Location: `~/.local/share/osai/receipts/{chat,ask,apply,model-router}`
- Format: JSON with metadata, action, outcome, sanitized inputs
- **Never contain**: full prompts, API keys, tokens, passwords, secrets
- **Always redact**: secret field values replaced with `[REDACTED]`

Example receipt structure:
```json
{
  "id": "...",
  "timestamp": 1234567890,
  "action": "ModelChat",
  "status": "Executed",
  "model": "gemma-4-E2B-it-Q8_0.gguf",
  "prompt_length": 42,
  "response_length": 128,
  "finish_reason": "stop",
  "receipts_dir": "..."
}
```

## Known Limitations

The following are **not yet implemented**:

- ⚠️ `osai-agent-core` — Partial extraction done, full extraction ongoing
- ⚠️ `osai-api` — Prototype exists with auth guard, endpoints, Dev Panel UI (not final desktop UI)
- ❌ UI — Dev Panel exists at port 8090, final desktop UI not built
- ❌ Desktop integration — COSMIC/KDE/GNOME shell integration
- ❌ Conversation history — Chat has no memory across sessions
- ❌ Streaming — Model Router does not support streaming responses
- ❌ Voice — No voice pipeline (push-to-talk or continuous)
- ❌ Multimodal/vision — No screenshot understanding or visual input
- ❌ Computer-use — No visible or hidden computer use implementation
- ❌ Packaging/installable OS — No Fedora Atomic / BlueBuild image yet
- ❌ Memory Manager — No agent memory scopes or persistence
- ❌ Model profiles — No hardware profile switching (laptop vs desktop vs server)
- ❌ Multiple concurrent agents — Single agent only
- ❌ Tool receipts in CLI apply — Tool receipts not yet written during apply

## Current Important Commands

```bash
# Rust workspace
cargo fmt --check
cargo check --workspace
cargo test --workspace

# Model Router
cd services/model-router && pytest tests

# Local runtime
./scripts/osai-local-up
./scripts/osai-local-check
./scripts/osai-local-down

# MVP loop
cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI MVP chat OK"
cargo run -p osai-agent-cli -- ask --print-plan "Create a safe plan to list my Downloads folder"
cargo run -p osai-agent-cli -- plan validate examples/plans/model-chat.yml
cargo run -p osai-agent-cli -- doctor
```

## Repository Structure

```
crates/
  osai-agent-cli/     # CLI application
  osai-plan-dsl/      # Plan DSL validation and types
  osai-toolbroker/    # Authorization layer
  osai-tool-executor/ # Action execution
  osai-receipt-logger/ # Receipt storage

services/
  model-router/       # FastAPI OpenAI-compatible gateway

examples/
  plans/              # Example OSAI plans
  policies/           # Example authorization policies

scripts/
  osai-local-*        # Local runtime orchestration
  osai-llamacpp-*    # llama.cpp runtime scripts
  osai-vllm-*        # vLLM runtime scripts

systemd/user/         # systemd user services
```

## Hardware Validated

- **Laptop Victus** with RTX 4060 Laptop GPU (8GB VRAM)
- NVIDIA Driver 580.126.09, CUDA 13.0, nvcc V13.0.88
- OS: elementary OS 8 / Ubuntu 24.04 noble
- Kernel: 6.17.0-22-generic

llama.cpp compiled with CUDA support (`GGML_CUDA=ON`, `CMAKE_CUDA_ARCHITECTURES=89`).