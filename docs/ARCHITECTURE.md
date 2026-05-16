# Architecture

## Current Architecture

The current OSAI MVP is CLI-based. The architecture flows one direction: user → CLI → Model Router → local/cloud model, with ToolBroker/ToolExecutor/Receipt Logger as supporting layers.

```
┌─────────────────────────────────────────────────────────────┐
│                    osai-agent-cli                          │
│  (chat, ask, apply, plan, tool, doctor)                    │
└──────────────────┬────────────────────────────────────────┘
                   │ HTTP / direct function calls
                   ▼
┌─────────────────────────────────────────────────────────────┐
│                   Model Router                               │
│  (FastAPI, OpenAI-compatible, llama.cpp/vLLM/MiniMax)        │
│  Port: 127.0.0.1:8088                                       │
└──────────────────┬────────────────────────────────────────┘
                   │ HTTP
        ┌──────────┴──────────┐
        ▼                     ▼
┌───────────────┐    ┌─────────────────┐
│  llama.cpp    │    │  MiniMax API     │
│  (local)      │    │  (cloud fallback)│
│  :8092        │    │                  │
└───────────────┘    └─────────────────┘

┌──────────────────┐     ┌──────────────────┐
│   ToolBroker     │◄────│  osai-agent-cli   │
│  (authorization) │     │  (apply command)  │
└────────┬─────────┘     └──────────────────┘
         │
         ▼
┌──────────────────┐     ┌──────────────────┐
│  ToolExecutor    │      │  ReceiptLogger   │
│  (execution)     │────►│  (audit trail)   │
└──────────────────┘      └──────────────────┘
```

## Target Architecture

The target architecture separates concerns: UI → osai-api → osai-agent-core → ToolBroker/ToolExecutor/receipts → Model Router → local/cloud models.

```
┌─────────────────────────────────────────────────────────────┐
│           Desktop Shell / UI Layer                          │
│  (COSMIC, KDE, GNOME — or web dev panel during dev)        │
└───────────────────────┬────────────────────────────────────┘
                        │ osai-api HTTP
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                      osai-api                               │
│  (Local REST API, loopback-only, token-auth)                │
│  - chat, ask, plan validate/authorize/apply                │
│  - receipts, runtime status, capabilities                   │
│  - future: sessions, history, settings                      │
│  Port: 127.0.0.1:8090                                      │
└───────────────────────┬────────────────────────────────────┘
                        │ osai-agent-core library
                        ▼
┌─────────────────────────────────────────────────────────────┐
│                    osai-agent-core                          │
│  (Shared Rust library: chat, ask, apply logic)             │
│  - chat_core_async()                                       │
│  - ask_core_async()                                        │
│  - run_apply()                                             │
│  - Runtime status collection                                │
└───────────┬───────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────┐
│           Tool Layer                                        │
│                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌────────────┐ │
│  │   ToolBroker    │◄─│  osai-agent-    │  │  Policy    │ │
│  │  (authorization)│  │  core           │  │  (YAML)    │ │
│  └────────┬────────┘  └─────────────────┘  └────────────┘ │
│           │                                              │
│           ▼                                              │
│  ┌─────────────────┐  ┌─────────────────┐                │
│  │ ToolExecutor    │  │ ReceiptLogger   │                │
│  │ (action exec)    │─►│ (audit trail)   │                │
│  └────────┬────────┘  └─────────────────┘                │
│           │                                              │
│           ▼                                              │
│  ┌─────────────────┐                                     │
│  │  Tool Drivers   │                                     │
│  │  FilesList      │                                     │
│  │  FilesWrite     │ (requires approval)                  │
│  │  ModelChat      │                                     │
│  │  DesktopNotify  │                                     │
│  │  BrowserOpenUrl│                                     │
│  │  ShellRunSandboxed                                    │
│  │  ComputerUseVisible (future)                          │
│  │  ComputerUseHidden  (future)                          │
│  └─────────────────┘                                     │
└───────────┬───────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Model Router                             │
│  (FastAPI, OpenAI-compatible, providers)                   │
│  Port: 127.0.0.1:8088                                       │
└───────────┬─────────────────────────────────────────────────┘
            │
            ▼
┌───────────────────────┐  ┌───────────────────────┐
│      llama.cpp        │  │       MiniMax         │
│  (local, Gemma 4)    │  │  (cloud fallback)    │
│  Port: 127.0.0.1:8092│  │                       │
└───────────────────────┘  └───────────────────────┘
```

## Component Roles

### osai-agent-cli
The CLI is a wrapper around osai-agent-core. It exists for development and automation. In the future, the CLI will use osai-agent-core directly (no shell-out), and UI will use osai-api.

### osai-agent-core (partially extracted)
The extracted shared library containing core logic: chat, ask, apply, runtime status. Both CLI and osai-api use this library. Extraction is ongoing.

### osai-api (prototype)
Local HTTP API service exposing osai-agent-core functionality over HTTP. Binds to loopback only. Token-auth protected. Dev Panel UI available at `http://127.0.0.1:8090/ui`. This is the interface the UI/desktop will call.

### Model Router
FastAPI service that routes model requests to the right provider (llama.cpp, vLLM, MiniMax). Handles provider-specific quirks, response normalization, and receipts.

### ToolBroker
The authorization layer. Every action is evaluated against the policy before execution. ToolBroker does not execute actions — it decides what is allowed.

### ToolExecutor
The execution layer. Executes actions that ToolBroker has authorized. Respects approvals. Writes receipts.

### ReceiptLogger
The audit layer. Every action produces a receipt. Receipts are stored locally and must not contain secrets or full prompts.

### Plan DSL
A typed YAML/JSON representation of a plan. Validated before execution. Structured enough for ToolBroker to evaluate, human-readable enough for users to audit.

## Boundary Rules

1. **Model proposes, ToolBroker authorizes, ToolExecutor executes, receipts audit**
2. **ToolExecutor must never bypass ToolBroker** — No direct execution without authorization
3. **Model output is untrusted** — Plan DSL validates structure, ToolBroker evaluates policy
4. **UI must use osai-api** — Future UI calls osai-api, not shell out to CLI
5. **osai-api must use osai-agent-core** — Not replicate CLI logic
6. **Local services loopback-only** — No external exposure of Model Router or llama-server
7. **Cloud is explicit fallback** — `privacy: local_only` is default, cloud requires policy

## Computer-Use Target Architecture

### Visible Mode
```
User Desktop/Session (active, visible)
  │
  │ OSAI operates with user watching
  │ User can interrupt/cancel/approve
  │
  ▼
ToolBroker ───► ComputerUseVisible tool
                 │
                 ├── screenshots (user sees)
                 ├── receipts (full audit)
                 └── actions visible in session
```

### Hidden Mode
```
Isolated Environment
(nested Wayland, virtual display, container, VM, separate user/session)
  │
  │ OSAI operates without user watching every step
  │ Isolated from active desktop
  │
  ├── screenshots (isolated session only)
  ├── receipts (full audit)
  ├── artifact store (outputs only)
  └── environment reset/destroy capability
        │
        ▼
   Final outputs / summaries returned to user
        │
        ▼
   User reviews before external transmission
```

### Safety Boundary for Computer Use

- All computer-use tasks start as a Plan DSL plan
- Plan must be validated before execution
- ToolBroker must authorize computer-use capabilities
- User approval required for sensitive categories (credentials, payments, destructive actions)
- Strict network/browser/file policies apply
- No credential entry without explicit user action
- No purchases/payments/account changes without explicit approval
- No destructive file/system changes without explicit approval
- All actions summarized in receipts
- Screenshots/artifacts handled with privacy controls
- Task can be cancelled at any time
- Hidden environment can be reset or destroyed
- Outputs must be reviewed before being sent externally

## Data Flow

### Chat Flow
```
User → osai-agent-cli chat → osai-agent-core chat_core_async
  → Model Router → llama.cpp / MiniMax
  → ReceiptLogger (prompt_length, response_length — no full prompt)
  → User
```

### Ask Flow
```
User → osai-agent-cli ask → osai-agent-core ask_core_async
  → Model Router → llama.cpp / MiniMax
  → Plan DSL parse/validate
  → ReceiptLogger (no full request text)
  → Saved plan file
  → User
```

### Apply Flow
```
User → osai-agent-cli apply → osai-agent-core run_apply
  → Plan DSL validation
  → ToolBroker authorize (per-step decisions)
  → User approval (for Ask/Never approval levels)
  → ToolExecutor execute (authorized steps only)
  → ReceiptLogger (per-step receipts)
  → Summary
  → User
```