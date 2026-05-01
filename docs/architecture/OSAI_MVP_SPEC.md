# OSAI MVP Specification v0.1

> **Status**: Active Development
> **Last Updated**: 2026-04-25

## 1. Project Definition

OSAI is an AI-native Linux distribution where:

- **Agents are first-class applications** - Agents are installed, managed, and run like system services with explicit permissions and memory scopes
- **Natural language becomes a programmable interface** - Users interact with agents through typed intents that are parsed, planned, and executed safely
- **Every AI action is mediated through typed tools** - No direct model-to-shell execution; all actions flow through ToolBroker with explicit authorization
- **Every action produces auditable receipts** - Complete audit trail of what was authorized, what executed, and what the model saw

The safety model ensures that even a compromised or misbehaving model cannot directly execute destructive commands without explicit user approval and validation against policy.

## 2. Current Architecture

### 2.1 Execution Flow

```
User
  │
  ├──▶ osai-agent CLI (chat | ask | apply)
  │      │
  │      ├──▶ chat ──▶ Model Router ──▶ Gemma/MiniMax
  │      │                      │
  │      │                      ▼
  │      │                 Receipt Logger
  │      │
  │      ├──▶ ask ──▶ Model Router ──▶ Gemma (plan generation)
  │      │                      │              │
  │      │                      ▼              ▼
  │      │                 Receipt Logger  Plan DSL YAML (validated)
  │      │
  │      └──▶ apply ──▶ Plan DSL (validated)
  │                    │
  │                    ▼
  │              ToolBroker (authorized)
  │                    │
  │                    ├──▶ ToolExecutor ──▶ Receipt Logger
  │                    │       (safe actions only)
  │                    │
  │                    └──▶ Model Router ──▶ Gemma (when needed)
  │
  └──▶ osai-api (future desktop/shell UI)
         │
         ├──▶ /v1/chat ──▶ Model Router ──▶ Gemma/MiniMax
         ├──▶ /v1/ask ──▶ Model Router ──▶ Gemma (plan generation)
         ├──▶ /v1/plans/validate ──▶ Plan DSL validation
         └──▶ /v1/apply ──▶ ToolBroker ──▶ ToolExecutor ──▶ Receipt Logger
```

### 2.2 Future Path to Production

```
OSAI Desktop/Shell UI
        │
        ▼
osai-api (local HTTP service)
        │
        ├──▶ Model Router ──▶ llama.cpp ──▶ Gemma 4
        │
        ├──▶ ToolBroker ──▶ ToolExecutor
        │
        └──▶ Receipt Logger

OpenClaw Bridge          # Protocol gateway for agent communication
Voice Daemon             # Push-to-talk voice intent
OSAI Command Bar          # Core UI for agent interaction
        │
        ▼
Fedora Atomic / Universal Blue / BlueBuild
        │
        ▼
    ISO Image + Installer
```

## 3. Implemented Components

### 3.1 osai-plan-dsl

**Purpose**: Safe, typed intermediate representation between natural language and system actions.

**Current Capabilities**:
- Parse and validate Plan YAML/JSON files
- Define steps with typed actions (FilesList, FilesMove, ShellRunSandboxed, etc.)
- Specify approval requirements per step
- Risk levels (Low, Medium, High, Critical)
- Rollback step definitions

**Current Limitations**:
- Schema is fixed;extensibility not yet implemented
- No sub-plans or reusable plan libraries
- Validation errors could be more descriptive

### 3.2 osai-receipt-logger

**Purpose**: Immutable audit trail for every authorization decision and execution result.

**Current Capabilities**:
- Store receipts as JSON files with UUID naming
- Link receipts to plan ID and step ID
- Record authorization decisions with reason
- Record execution results with error details
- List and retrieve receipts by UUID

**Current Limitations**:
- No receipt compaction or rotation
- No centralized receipt database
- No receipt aggregation for analytics

### 3.3 osai-toolbroker

**Purpose**: Policy-based authorization gate between plans and execution.

**Current Capabilities**:
- Load policy from YAML (default_mode, action_modes, allowed_roots)
- Authorize ToolRequests against policy
- Deny actions that violate shell_network_allowed or shell_requires_sandbox
- Require user approval for configured actions
- Create authorization receipts

**Current Limitations**:
- Policy is static (loaded from file at startup)
- No policy hot-reload
- No policy versioning
- Limited deny reason granularity

### 3.4 osai-tool-executor

**Purpose**: Execute authorized actions safely with receipt generation.

**Current Capabilities**:
- Execute FilesList (constrained to allowed_root)
- Execute DesktopNotify (mock/simulated)
- Execute ModelChat (local Model Router or simulated)
- Refuse destructive actions (FilesWrite, FilesMove, FilesDelete)
- Refuse unsafe shell commands
- Generate execution receipts

**Current Limitations**:
- Only 3 actions actually execute (FilesList, DesktopNotify, ModelChat)
- No real filesystem mutations
- No shell command execution
- No browser automation
- ModelChat is simulated without Model Router

### 3.5 osai-agent-cli

**Purpose**: Command-line interface for working with OSAI plans, policies, and tools.

**Current Commands**:
- `plan validate` - Validate plan YAML/JSON
- `plan print` - Print plan in JSON or YAML format
- `policy validate` - Validate policy YAML
- `receipt list` - List receipts in directory
- `receipt show` - Show specific receipt
- `init` - Initialize new agent directory
- `tool authorize` - Authorize plan against policy (no execution)
- `tool run` - Authorize and execute plan
- `doctor` - Run diagnostic checks
- `chat` - Conversational chat with local/cloud model via Model Router
- `ask` - Generate valid OSAI Plan DSL YAML from natural language (no execution)
- `apply` - Validate, authorize, and execute a plan end-to-end (with approval flags)

**Current Limitations**:
- No interactive approval workflow
- No plan step debugging
- No diff between plan versions

### 3.6 services/model-router

**Purpose**: Local service that routes model requests to appropriate providers (local Gemma or cloud MiniMax).

**Current Capabilities**:
- OpenAI-compatible `/v1/chat/completions` endpoint
- `/health` health check endpoint
- `/v1/models` listing endpoint
- Auto-routing based on `metadata` hints (privacy, complexity, speed)
- Mock mode for cloud providers (default)
- Thinking block stripping
- Loopback-only binding (127.0.0.1)

**Current Limitations**:
- No real MiniMax integration (mock mode only)
- No Ollama integration
- No model weight management
- No model download/pulling
- No streaming responses
- Single-user only

### 3.7 scripts/osai-dev-*

**Purpose**: Developer convenience scripts for local development.

**Current Scripts**:
- `osai-dev-env` - Source to set environment variables (safe defaults, mock mode)
- `osai-dev-up` - Start Model Router in foreground
- `osai-dev-check` - Check Model Router health and functionality
- `osai-dev-down` - Stop systemd user service
- `osai-install-user-services` - Install systemd user units

### 3.8 systemd/user/osai-model-router.service

**Purpose**: Persistent background service for Model Router.

**Current Configuration**:
- Runs as user systemd service
- EnvironmentFile: `~/.config/osai/model-router.env` (optional)
- Default mock mode: `OSAI_MODEL_ROUTER_MOCK_CLOUD=true`
- Default receipts dir: `~/.local/share/osai/receipts/model-router`
- Binds to 127.0.0.1:8088
- Restart on failure

### 3.9 osai-api

**Purpose**: Local HTTP service exposing OSAI agent capabilities via `osai-agent-core`. Provides a programmatic API for future desktop/shell UI without shelling out to `osai-agent-cli`.

#### OSAI API Dev Panel

A local Dev Panel UI is available at `http://127.0.0.1:8090/ui` for validating the full API-driven flow during development.

**URL**: `http://127.0.0.1:8090/ui`
**Scope**: Temporary API validation prototype, NOT the final OSAI desktop shell.
**Purpose**: Validate API contracts and user interaction flow before choosing/forking a desktop base.

**Dev Panel Capabilities**:
- Status panel (health, Model Router reachability, capabilities)
- Chat panel (POST /v1/chat)
- Ask / Plan generation panel (POST /v1/ask)
- Plans panel (GET /v1/plans, GET /v1/plans/read)
- Plan validation panel (POST /v1/plans/validate)
- Authorization preview panel (POST /v1/plans/authorize)
- Apply panel (POST /v1/apply with dry-run and real-execution modes)
- Receipts panel (GET /v1/receipts, GET /v1/receipts/read)
- Activity log of recent UI actions/errors

**Validated UI Flow**:
1. `GET /v1/status` - Check local stack health
2. `POST /v1/chat` - Test model chat
3. `POST /v1/ask` - Generate plan from natural language
4. `GET /v1/plans` / `GET /v1/plans/read` - Browse and preview plans
5. `POST /v1/plans/validate` - Validate plan before authorization
6. `POST /v1/plans/authorize` - Preview authorization decisions (no execution)
7. `POST /v1/apply` with `dry_run=true` - Simulate execution
8. `POST /v1/apply` with `dry_run=false` - Execute after explicit user approval + authorization in session
9. `GET /v1/receipts` - Audit history after execution

**Safety Gates in Dev Panel**:
- Real execution button is disabled by default
- Requires checkbox "I understand this may execute actions"
- Requires plan to be authorized in the current UI session
- Requires explicit browser confirm() before real execution
- dry_run defaults to true

**Security Properties**:
- No external network assets (CDN, fonts, images)
- No localStorage for API responses or prompts
- Loopback-only binding (127.0.0.1)
- No analytics or telemetry

**Current Limitations**:
- Single-page HTML/CSS/JS only, no framework
- No authentication (future desktop UI handles auth)
- No streaming responses
- Single-user only

**API Endpoints**:

Core (existing):
- `GET /health` - Health check with service/version
- `GET /v1/capabilities` - Returns `{chat, ask, plan_validate, plan_authorize, apply, plans, receipts}: true`
- `POST /v1/chat` - Chat with model router (configurable URL, receipts persisted)
- `POST /v1/ask` - Generate Plan DSL from natural language (receipts/plans persisted)
- `POST /v1/plans/validate` - Validate plan YAML/JSON file
- `POST /v1/apply` - Validate+authorize+execute a plan (dry_run=true by default)
- `POST /chat`, `/ask`, `/apply` - Compatibility aliases to `/v1/*`

New UI-ready endpoints:
- `GET /v1/status` - Local stack/service status with Model Router reachability check
- `GET /v1/plans` - List generated plans (sorted newest first, limit/max 100)
- `GET /v1/plans/read?path=...` - Read one plan for UI preview
- `POST /v1/plans/authorize` - Preview plan authorization without executing
- `GET /v1/receipts` - List recent receipts (chat/ask/apply/tool/model-router)
- `GET /v1/receipts/read?path=...` - Read one receipt with secret redaction

**Request Configuration**:
All endpoints accept optional `model_router_url`, `receipts_dir`, `plans_dir` fields to override defaults.

**Safety Properties**:
- Binds to 127.0.0.1:8090 only (no external interface exposure)
- Validates model_router_url is loopback before any request
- Uses osai-agent-core directly (no CLI subprocess)
- dry_run defaults to true for /v1/apply (execution requires explicit `"dry_run": false`)
- Full prompts not stored in receipts (redacted metadata only)
- Secret redaction on receipt read (api_key, token, password, secret, credential, authorization)
- /v1/plans/authorize does NOT execute ToolExecutor (preview only)
- /v1/plans/read does NOT expose full file contents by default

**Current Limitations**:
- No authentication (future desktop UI handles auth)
- No streaming responses
- No WebSocket support yet
- Single-user only

## 4. Current CLI Commands

### Plan Commands

```bash
osai-agent plan validate <path>
osai-agent plan print <path> --format json|yaml
```

### Policy Commands

```bash
osai-agent policy validate <path>
```

### Receipt Commands

```bash
osai-agent receipt list <root_dir>
osai-agent receipt show <root_dir> <uuid>
```

### Init Command

```bash
osai-agent init <directory>
```

### Tool Commands

```bash
osai-agent tool authorize --plan <path> --policy <path> --receipts-dir <path>
osai-agent tool run --plan <path> --policy <path> --receipts-dir <path> --allowed-root <path>... [--approve <step_id>] [--approve-all] [--model-router-url <url>]
```

### Doctor Command

```bash
osai-agent doctor [--repo-root <path>] [--model-router-url <url>] [--receipts-dir <path>] [--skip-model-router] [--json]
```

## 5. Security Model v0.1

### 5.1 Core Principles

1. **No Direct Model-to-Shell**: Models never execute shell commands directly. All execution flows through ToolBroker authorization.

2. **Plans Must Validate**: Invalid plans cannot be executed. Validation catches schema errors and safety violations before any execution attempt.

3. **ToolBroker Authorizes**: Every action is checked against policy before execution. Policy enforces:
   - `shell_network_allowed`: Whether unsandboxed network access is permitted
   - `shell_requires_sandbox`: Whether shell commands must be sandboxed
   - `allowed_roots`: Which directories FilesList can access

4. **ToolExecutor Executes Only Safe Subset**: Even authorized actions are filtered:
   - FilesWrite, FilesMove, FilesDelete are refused in v0.1
   - ShellRunSandboxed is refused in v0.1
   - BrowserOpenUrl is refused in v0.1

5. **Destructive Actions Require Approval**: Actions in `require_approval` list need explicit `--approve` or `--approve-all`.

6. **Approval Does Not Bypass Executor Safety**: Approval only bypasses the "requires user approval" check. ToolExecutor still refuses unsupported actions even with approval.

7. **Receipts Are Written**: Every authorization decision and execution result is recorded.

8. **Prompts and Secrets Should Not Be Logged**: Full prompts are not stored in receipts; only message counts and roles.

9. **Model Router Only Binds to 127.0.0.1**: External network exposure is prevented at the socket level.

10. **MiniMax Mock Mode is Default**: Development uses simulated cloud responses to prevent accidental spend.

### 5.2 Authorization Flow

```
Plan Step
    │
    ▼
┌─────────────────┐
│   Validate      │ ─── Invalid? ──▶ REJECT
└─────────────────┘
    │
    ▼
┌─────────────────┐
│  ToolBroker     │
│  authorize()    │ ─── Denied? ──▶ REJECT + receipt
└─────────────────┘
    │
    ▼
┌─────────────────┐
│  requires_      │
│  approval?      │ ─── Yes + not approved? ──▶ SKIP + receipt
└─────────────────┘
    │
    ▼
┌─────────────────┐
│ ToolExecutor    │
│ execute()       │ ─── Unsupported? ──▶ FAIL + receipt
└─────────────────┘
    │
    ▼
 SUCCESS + receipt
```

## 6. Receipts Model

### 6.1 Receipt Types

| Receipt Type | Created By | Contents |
|--------------|------------|---------|
| Authorization | ToolBroker | request_id, action, decision, reason, policy_mode |
| Execution | ToolExecutor | request_id, status, error, duration_ms |
| Model | Model Router | request_id, provider, model, tokens_used, duration_ms |

### 6.2 Redaction Policy

**Stored (for authorization/execution)**:
- Plan ID, step ID
- Action type
- Decision (allowed/denied)
- Policy mode
- Risk level
- Timestamp

**NOT Stored**:
- Full prompts or messages (only counts)
- File paths or contents
- Error details that may contain sensitive data
- API keys or tokens

**ModelChat Special Handling**:
```rust
// Only these fields are stored:
message_count: usize,
roles: Vec<String>,  // ["user", "assistant"]
// NOT: prompt content, messages, responses
```

### 6.3 Receipt Naming

```
{uuid}.json
```

UUIDs are V4 random, providing uniqueness without predictability.

## 7. Model Strategy

### 7.1 Local Model Strategy

**Default Local**: Gemma 4 E2B Q8 GGUF
- Used for: Most agent interactions, validated via llama.cpp CUDA build
- Location: `.local-models/llamacpp/gemma-4-e2b-it/gemma-4-E2B-it-Q8_0.gguf`

**Smoke-test Fallback**: Qwen2.5-0.5B GGUF Q4_K_M
- Used for: Quick validation, CI, resource-constrained environments

**Performance Local**: Gemma 4 26B
- Used for: Complex reasoning tasks
- Activation: Only when plugged in or explicitly requested

**Background Local**: Gemma 4 E2B (same model, lower资源配置)

### 7.2 Cloud Model Strategy

**Default Cloud**: MiniMax-M2.7
- Used for: Tasks requiring higher capability
- Routing: Via Model Router with `osai-cloud` model name

**Fast Cloud**: MiniMax-M2.7-highspeed
- Used for: Low-latency requirements

**Mock Mode (Development)**:
- All cloud calls return simulated responses
- No actual API spend
- Enabled by `OSAI_MODEL_ROUTER_MOCK_CLOUD=true`

### 7.3 Auto-Routing

The `osai-auto` model routes based on metadata hints:

| Metadata | Routing |
|----------|---------|
| `privacy: "local_only"` | Always use local |
| `complexity: "high"` | Use larger local or cloud |
| `speed: "fast"` | Prefer fast models |
| (none) | Default local |

### 7.4 ModelChat Execution Path

```
Plan: ModelChat step
    │
    ▼
ToolExecutor
    │
    ├── No Model Router URL? ──▶ Return simulated response
    │
    └── Model Router URL set ──▶ POST /v1/chat/completions
                                      │
                                      ▼
                               Model Router
                                      │
                    ┌─────────────────┴─────────────────┐
                    │                               │
            osai-auto routing              Direct model (osai-local, etc.)
                    │                               │
                    ▼                               ▼
            Check metadata hints              Route to appropriate provider
                    │                               │
                    └───────────────┬───────────────┘
                                    ▼
                            Gemma or MiniMax
                                    │
                                    ▼
                            Receipt (redacted)
```

### 7.5 No Direct MiniMax Calls

Agents and tools never call MiniMax directly. All model traffic flows through Model Router, which:
- Enforces mock mode when configured
- Provides audit trail
- Abstracts provider details

## 8. Development Workflow

### 8.1 Starting Local Runtime

```bash
# Start llama.cpp + Model Router + osai-api (foreground supervision)
./scripts/osai-local-up

# Or start individual services:
./scripts/osai-llamacpp-up      # llama.cpp only
./scripts/osai-dev-up           # Model Router only
cargo run -p osai-api           # osai-api only (from repo root)
```

### 8.2 Checking Health

```bash
# Full stack check (llama.cpp, Model Router, osai-api, tool receipts, secrets)
./scripts/osai-local-check

# Individual checks:
./scripts/osai-dev-check        # Model Router only
./scripts/osai-llamacpp-check   # llama.cpp only
```

### 8.3 Stopping Services

```bash
./scripts/osai-dev-down
```

### 8.4 Running Tests

```bash
# Rust tests
cargo test

# Python tests (Model Router)
cd services/model-router && pytest tests/
```

### 8.5 Diagnostic Checks

```bash
# Full diagnostic (skips Model Router)
cargo run -p osai-agent-cli -- doctor --skip-model-router

# With Model Router checks
cargo run -p osai-agent-cli -- doctor

# JSON output for automation
cargo run -p osai-agent-cli -- doctor --json --skip-model-router
```

## 9. MVP Milestones

| Milestone | Status | Description |
|-----------|--------|-------------|
| M1 Core Safety Runtime | **COMPLETED** | Plan DSL, ToolBroker, ToolExecutor, Receipt Logger |
| M2 Model Router Integration | **COMPLETED** | Model Router service with mock mode, CLI integration |
| M3 Service Orchestration | **COMPLETED** | Dev scripts, systemd units, doctor command |
| M4 Local Model Provider | NEXT | Gemma 4 integration (real, not mock) |
| M5 Memory Manager | PLANNED | Scoped, inspectable agent memory |
| M6 OSAI Command Bar UI | PLANNED | Core Tauri/TypeScript UI for agent interaction |
| M7 Voice Daemon | PLANNED | Push-to-talk voice intent capture |
| M8 Fedora/Universal Blue Image | PLANNED | Base OSAI image with atomic updates |
| M9 VM Test Image | PLANNED | Pre-built VM for testing without installation |
| M10 Installer / Dual Boot | PLANNED | User-friendly installation with Windows dual boot |
| M11 OpenClaw Bridge | **DEFERRED** | Optional protocol gateway (see [OpenClaw Decision](OPENCLAW_DECISION.md)) |

**Note**: OpenClaw Bridge is explicitly deferred. See [OpenClaw Integration Decision](OPENCLAW_DECISION.md) for rationale.

## 10. Non-Goals for Now

The following are explicitly **NOT** in scope for MVP v0.1:

- **No kernel modification** - OSAI runs on standard Fedora atomic
- **No direct shell execution** - All execution via ToolBroker authorization
- **No OpenClaw core dependency** - OSAI runtime is independent (see [decision](OPENCLAW_DECISION.md))
- **No filesystem mutation until approval/rollback is stronger** - v0.1 ToolExecutor refuses FilesWrite, FilesMove, FilesDelete
- **No always-listening microphone** - Voice is push-to-talk only
- **No real agent marketplace** - Just manifests and directories
- **No distro installer yet** - Coming in M10

## 11. Open Questions

These items need further design before implementation:

1. **GNOME vs Tauri First UI** - Which platform to prioritize for Command Bar?
2. **Ollama/Gemma Integration** - Direct Ollama API or Model Router as Ollama wrapper?
3. **Local Model Management** - How to download, update, and select local models?
4. **Memory Manager Design** - Scope, persistence, and access control for agent memory?
5. **Rollback for File Mutations** - Atomic transactions or copy-on-write backup?
6. **Policy UI** - How should users view and edit policies visually?
5. **Memory Manager Design** - Scope, persistence, and access control for agent memory?
6. **Rollback for File Mutations** - Atomic transactions or copy-on-write backup?
7. **Policy UI** - How should users view and edit policies visually?

## 12. Related Documentation

- [OSAI Agent CLI](../crates/osai-agent-cli/README.md) - CLI usage and examples
- [Model Router](../services/model-router/README.md) - Model Router API and configuration
- [Plan DSL Schema](../crates/osai-plan-dsl/) - Plan file format reference
