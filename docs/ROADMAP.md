# Roadmap

Phased implementation plan for OSAI. Each phase has clear acceptance criteria. Do not skip phases.

## Phase 0: Protect MVP and Documentation (CURRENT)

**Goal**: Lock in current working state with complete documentation.

**Status**: ✅ In progress (this documentation pass)

**Acceptance Criteria**:
- [x] All documentation files created
- [x] Test suite passing
- [x] MVP loop documented and validated
- [ ] osai-agent-core extraction planned with API contract

## Phase 1: osai-agent-core Extraction

**Goal**: Extract CLI logic into a reusable Rust library. CLI becomes a thin wrapper.

**Status**: ⚠️ Partially extracted — chat.rs, ask.rs, apply.rs, runtime.rs, shared.rs exist; full extraction and clean public API pending

**Key Activities**:
- Identify all logic in `osai-agent-cli/src/main.rs` that belongs in core
- Create `crates/osai-agent-core/src/lib.rs` with:
  - `chat_core_async()` — chat with Model Router
  - `ask_core_async()` — ask/generate plan
  - `run_apply()` — validate, authorize, apply
  - `collect_runtime_status_async()` — runtime health checks
- Ensure CLI behavior is unchanged after refactor
- All existing tests pass
- No behavior changes

**Acceptance Criteria**:
- [ ] `osai-agent-core` crate created
- [ ] CLI still works identically (`cargo run -p osai-agent-cli -- chat "test"`)
- [ ] MVP loop still works
- [ ] `cargo test --workspace` passes
- [ ] `osai-agent-core` exports clean public API

## Phase 2: osai-api

**Goal**: Local HTTP API service using osai-agent-core. This is the interface future UI will call.

**Status**: ⚠️ Prototype exists — auth guard, endpoints, Dev Panel UI at port 8090; full endpoint implementation and osai-agent-core integration pending

**Key Endpoints (proposed)**:
- `GET /health` — service health
- `GET /v1/status` — model/runtime status
- `GET /v1/runtime/status` — unified runtime health
- `GET /v1/capabilities` — feature flags
- `POST /v1/chat` — chat with model
- `POST /v1/ask` — generate plan from natural language
- `GET /v1/plans` — list plans
- `GET /v1/plans/read` — read plan content
- `POST /v1/plans/validate` — validate plan
- `POST /v1/plans/authorize` — authorize plan with policy
- `POST /v1/apply` — apply plan (dry-run or real)
- `GET /v1/receipts` — list receipts
- `GET /v1/receipts/read` — read receipt content
- `GET /v1/auth/status` — auth status (token source, required)
- `GET /ui` — Dev Panel web UI

**Security Requirements**:
- Loopback-only binding (127.0.0.1:8090)
- Token-based auth on sensitive endpoints (chat, ask, apply, receipts, plans)
- Token from `OSAI_API_TOKEN` env var or `~/.config/osai/api-token` file
- 401 response for missing/invalid token
- Token never logged, never in receipts

**Acceptance Criteria**:
- [ ] All proposed endpoints implemented
- [ ] Token auth on protected endpoints
- [ ] Health/status/capabilities/UI unauthenticated
- [ ] osai-api uses osai-agent-core (not CLI shell-out)
- [ ] `cargo test --workspace` passes
- [ ] Local dev panel UI functional

## Phase 3: Local Dev UI / Control Panel

**Goal**: Web-based development and control panel (not the final product UI).

**Status**: ⚠️ Prototype exists — Dev Panel UI at `http://127.0.0.1:8090/ui`; final desktop UI not built

**Key Features**:
- Status panel (llama.cpp, Model Router, osai-api health)
- Runtime status panel (unified component health, systemd state)
- Chat panel (send messages, see responses)
- Ask panel (generate plans)
- Plan viewer (view/validate/authorize plans)
- Apply panel (dry-run, real apply with approval modal)
- Receipt viewer (list/read receipts)
- Token input (in-memory only, not localStorage)
- 401 handling with helpful error messages

**Constraints**:
- This is a development tool, not the final desktop UI
- Token stored in JavaScript in-memory only
- No localStorage/sessionStorage for sensitive data
- All protected calls include auth headers
- Must work without token for status/runtime/status panels

**Acceptance Criteria**:
- [ ] Dev Panel loads at `http://127.0.0.1:8090/ui`
- [ ] Status/runtime panels work without token
- [ ] Protected endpoints return 401 with helpful message when token missing
- [ ] Chat/ask/plans/receipts/apply work with valid token
- [ ] Token never persisted across page reload

## Phase 4: Desktop/Session Integration

**Goal**: OSAI assistant integrated into a Linux desktop environment. This is the final UI target.

**Status**: ❌ Not started

**Desktop Options**:
1. **COSMIC (System76)** — Rust, Wayland, modular. Aligns with OSAI's Rust stack.
2. **KDE Plasma** — Mature plasmoid system, QML-based UI
3. **Cinnamon** — Traditional, forkable
4. **GNOME** — Powerful but opinionated, extension API changes can break things
5. **Xfce** — Lightweight but less suited for AI-OS modern needs

**Recommended Approach**:
- Build core and API first (Phases 1-3)
- Do not fork a desktop yet
- Prototype with KDE/QML or GNOME extensions once API is proven
- Target COSMIC for long-term production

**Target UI Features**:
- Assistant panel (docked or floating)
- Command palette (global hotkey)
- Approvals modal (for sensitive actions)
- Receipts timeline
- Model/runtime status indicator
- Privacy indicator
- File action previews
- Settings panel
- Sessions/history
- Voice controls (future)
- Computer-use controls (future)

**Constraints**:
- UI must not bypass ToolBroker/ToolExecutor
- All actions through osai-api/osai-agent-core
- No shell-out to CLI

## Phase 5: Safe Tool Expansion

**Goal**: Expand the tool set through ToolBroker/ToolExecutor with proper security review.

**Current Tools** (working, tested):
- FilesList ✅
- ModelChat ✅
- ReceiptCreate ✅
- DesktopNotify ✅
- ShellRunSandboxed ⚠️ (sandboxed, requires approval)

**Tools Needing Security Review** (before making real):
- FilesWrite ⚠️ — requires approval, security review needed
- FilesMove ⚠️ — requires approval, security review needed
- FilesDelete ⚠️ — requires approval, security review needed, rollback needed
- BrowserOpenUrl ⚠️ — requires approval, sandboxing review needed

**Future Tools** (not yet implemented):
- ComputerUseVisible — visible computer use
- ComputerUseHidden — hidden/isolated computer use
- VoiceInput — push-to-talk voice pipeline
- VisionInput — screenshot/OCR pipeline
- MemoryRead/MemoryWrite — agent memory scopes
- SystemInfo — hardware/system queries

## Phase 6: Computer-Use Subsystem

**Goal**: Safe, auditable computer use in two modes: visible and hidden/isolated.

**Status**: ❌ Not started (design documented in `COMPUTER_USE_STRATEGY.md`)

**Visible Mode**:
- OSAI operates in the user's active desktop session
- User can watch, interrupt, cancel
- Sensitive actions require approval
- Receipts generated for all actions
- Screenshots visible to user

**Hidden Mode**:
- Isolated environment (nested Wayland, virtual display, container, VM, separate session)
- User does not need to watch every step
- Final outputs/artifacts/summaries returned
- Must remain auditable, permissioned, cancellable
- Hidden environment can be reset/destroyed

**Safety Requirements**:
- Every computer-use task starts as a Plan DSL plan
- Plan validated before execution
- ToolBroker must authorize computer-use capabilities
- User approval required for sensitive categories
- Strict network/browser/file policies
- No credential entry without explicit user action
- No purchases/payments/account changes without approval
- No destructive changes without approval
- All actions in receipts
- Screenshots/artifacts under privacy controls
- Task cancellable
- Hidden environment resettable

## Phase 7: Voice/Multimodal

**Goal**: Voice input/output, screenshot understanding, multimodal model support.

**Voice Pipeline** (proposed):
```
Microphone → Whisper/faster-whisper/whisper.cpp → text → OSAI ask/generate
```

**Screenshot Understanding** (for computer use):
- Vision model for screenshot analysis
- OCR pipeline for text extraction
- Local model preferred for privacy

**Multimodal Model Options**:
- Gemma 4 multimodal (experimental, future)
- Local vision models (Qwen2.5-VL, custom)
- Cloud vision (explicit opt-in)

## Phase 8: Packaging/Installable Linux System

**Goal**: Installable OSAI Linux image based on Fedora Atomic / Universal Blue / BlueBuild.

**Target Architecture**:
- Fedora Atomic base
- systemd, SELinux, cgroups v2, Wayland, PipeWire
- OSAI layered on top
- Dual-boot installer with Windows

**Package Contents**:
- Linux base
- OSAI AI Kernel (Plan DSL, Model Router, ToolBroker, Receipt Logger)
- Local model runtime (llama.cpp, Gemma 4)
- Desktop shell integration
- Installer

## Phase 9: Hardening, Updates, Distribution

**Goal**: Production hardening, update mechanism, distribution.

**Items**:
- Update mechanism for base OS and OSAI layers
- Rollback capability
- Hardware profile detection (laptop vs desktop vs server)
- Model management (download, update, select)
- Agent marketplace / manifests
- Distribution via Universal Blue / Fedora Atomic

## What Is Current (Phase 0)

- ✅ MVP loop validated: chat, ask, plan validate, apply dry-run, apply real
- ✅ Rust workspace with 5 crates
- ✅ Model Router FastAPI with 3 providers
- ✅ llama.cpp local runtime with CUDA
- ✅ Gemma 4 E2B Q8 GGUF default model (current validated default)
- ⚠️ osai-agent-core partially extracted (chat.rs, ask.rs, apply.rs, runtime.rs, shared.rs)
- ⚠️ osai-api prototype exists (auth guard, endpoints, Dev Panel UI at port 8090)
- ❌ Desktop UI (Phase 4 after osai-api)