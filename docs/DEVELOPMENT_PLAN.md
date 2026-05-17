# Development Plan

## Priority Order

1. **Protect current MVP** — Never break what works
2. **Extract osai-agent-core** — Refactor, not feature change
3. **Build osai-api** — API service on top of core
4. **Build local dev UI** — Development panel using API
5. **Add desktop integration** — Shell integration using API
6. **Safe tool expansion** — More tools through proper channels
7. **Computer-use subsystem** — Visible and hidden modes with safety
8. **Voice/multimodal** — Pipeline additions
9. **Packaging/installable OS** — Fedora Atomic / BlueBuild
10. **Hardening and distribution** — Production polish

## Phase 1: osai-agent-core Extraction

### Why This First

osai-agent-core extraction is the foundation for everything else:

- CLI logic currently lives in `osai-agent-cli/src/main.rs`
- osai-api needs to call core logic without shelling out to CLI
- Future desktop UI needs the same core logic
- Desktop integration must call osai-api, not shell out to CLI
- Current CLI behavior must remain unchanged

Extracting core first ensures:
- No behavior change for existing users
- API and UI can use the same functions as CLI
- Easier to test core logic in isolation
- Clear API contract before UI work begins

### What Moves to osai-agent-core

Functions to extract from CLI into `crates/osai-agent-core/src/lib.rs`:

```rust
// chat
pub async fn chat_core_async(
    message: &str,
    model_router_url: &str,
    receipts_dir: Option<&Path>,
    model: &str,
    privacy: &str,
    max_tokens: Option<u32>,
    temperature: f32,
) -> Result<ChatResult>

// ask
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

// apply
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

// runtime status
pub async fn collect_runtime_status_async() -> RuntimeStatus
```

### What Stays in osai-agent-cli

Thin CLI wrapper that calls osai-agent-core:

- Argument parsing (clap)
- Output formatting (human vs JSON)
- Error handling (exit codes)
- Example usage text
- Completion scripts

### Implementation Order

1. Create `crates/osai-agent-core/Cargo.toml` with required deps
2. Create `crates/osai-agent-core/src/lib.rs` with module structure
3. Move `chat_core_async` from CLI to core (with tests)
4. Move `ask_core_async` from CLI to core (with tests)
5. Move `run_apply` from CLI to core (with tests)
6. Move runtime status functions to core
7. Update CLI to use osai-agent-core
8. Verify CLI behavior unchanged
9. Run full test suite

### Acceptance Criteria

- CLI behaves identically before and after refactor
- `cargo test --workspace` passes
- MVP loop works
- `osai-agent-core` has clean public API
- No function signatures change without reason
- Tests cover all public functions

### Warnings

- **Do not add features during extraction** — This is a refactor only
- **Do not change behavior** — Only move code
- **Keep tests passing** — If tests break, the extraction is wrong
- **Do not break CLI compatibility** — Existing commands and flags must work

## Phase 2: osai-api

### Why After osai-agent-core

osai-api must use osai-agent-core, not replicate CLI logic:

- osai-api calls core functions directly
- CLI remains a thin wrapper around core
- UI calls osai-api
- API is the stable interface for future UI work

If we build osai-api before core:
- We replicate CLI logic in API (code duplication)
- Changes to core logic require updating both CLI and API
- The architecture boundary is wrong

### Implementation Status

Phase 2 MVP API is implemented:

1. `crates/osai-api/src/main.rs` provides a loopback-only HTTP server (tokio)
2. Chat, ask, and apply endpoints call osai-agent-core directly
3. Token-based auth accepts `Authorization: Bearer` and `X-OSAI-Token`
4. `/v1/auth/status` reports auth state without revealing token values
5. `/ui` static file serving provides the prototype Dev Panel
6. Plan and receipt endpoints enforce path safety for reads/lists
7. Apply defaults to dry-run and uses ToolBroker/ToolExecutor boundaries through core
8. Full Rust and Model Router test suites pass in Codex with `PYTHONPATH=src` for Python tests

### Key Endpoints

```bash
# Unauthenticated (safe for local introspection)
GET  /health
GET  /v1/status
GET  /v1/capabilities
GET  /v1/runtime/status
GET  /v1/auth/status
GET  /ui

# Token-protected
POST /v1/chat
POST /v1/ask
GET  /v1/plans
GET  /v1/plans/read
POST /v1/plans/validate
POST /v1/plans/authorize
POST /v1/apply
GET  /v1/receipts
GET  /v1/receipts/read
```

### Token Design

- Source 1: `OSAI_API_TOKEN` env var (if set)
- Source 2: `~/.config/osai/api-token` file (auto-generated)
- Header: `Authorization: Bearer <token>` or `X-OSAI-Token: <token>`
- Error: HTTP 401 with `{"ok":false,"error":{"code":"unauthorized","message":"..."}}`
- Token never logged, never in receipts

### Acceptance Criteria

- [x] All proposed endpoints implemented
- [x] Protected endpoints return 401 without valid token
- [x] Unauthenticated endpoints work without token
- [x] Token auth uses constant-time comparison
- [x] Dev Panel UI loads at `/ui`
- [x] MVP loop works through API endpoints without shelling out to CLI
- [x] `cargo test --workspace` passes

## Phase 3: Local Dev UI

### Why After osai-api

UI must call osai-api, not bypass it:

- UI uses osai-api for all operations
- Token stored in memory only (not localStorage/sessionStorage)
- Protected calls include auth headers
- Status/runtime panels work without token
- 401 handling shows helpful error

### Implementation Order

1. Extend existing Dev Panel (`crates/osai-api/static/ui.html`)
2. Add token input panel
3. Add `getHeaders()` helper for auth
4. Update all protected fetch calls with auth headers
5. Add 401 handling with helpful messages
6. Test token auth flow
7. Verify receipts/plans/chat all work with token

### Constraints

- **Development tool only** — Not the final desktop UI
- **In-memory token** — No localStorage/sessionStorage
- **No secrets in storage** — Token cleared on page reload
- **Helpful errors** — 401 should show clear message, no retry loops
- **Status without token** — Runtime status panels work unauthenticated

## Phase 4: Desktop/Session Integration

### Why After API is Stable

Desktop integration requires a stable API:

- Shell integration calls osai-api
- Desktop UI uses osai-api for all operations
- API changes require updating desktop UI
- Stable API first, then desktop integration

### Desktop Option Analysis

| Desktop | Language | Notes |
|---------|----------|-------|
| COSMIC | Rust | Strategic choice, aligns with OSAI's stack |
| KDE Plasma | C++/QML | Mature plasmoid system |
| Cinnamon | C/Vala | Traditional, forkable |
| GNOME | C/JS | Powerful but fragile extensions |
| Xfce | C | Lightweight, less AI-OS oriented |

**Recommendation**: Build core and API first (Phases 1-3). Desktop integration comes after API is proven. Target COSMIC long-term, KDE/QML prototype short-term.

### Constraints

- UI must not bypass ToolBroker/ToolExecutor
- All actions through osai-api/osai-agent-core
- No shell-out to CLI

## Do Not Proceed If Tests Fail

At every phase, the following must pass before continuing:

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cd services/model-router && pytest tests
./scripts/osai-local-check  # if runtime is running
```

If any test fails:
1. Stop
2. Fix the failure
3. Verify all tests pass
4. Then continue to next phase

## Not a Feature Change

Phases 1-3 are **refactors only**. The goal is to move existing working code into a better architecture without changing behavior.

If a feature stops working during Phase 1-3:
- The refactor is wrong
- Fix the refactor
- Do not "fix" the behavior by changing what the feature does

Feature changes come after the architecture is established.