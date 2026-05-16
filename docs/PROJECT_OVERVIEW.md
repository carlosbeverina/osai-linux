# Project Overview

## What Is OSAI?

OSAI (Open Source AI) is a local-first AI operating system and assistant layer built on Linux. The goal is an installable Linux-based system where AI agents are first-class applications, natural language becomes a programmable interface, and every AI action is mediated through typed tools, explicit permissions, memory scopes, sandboxing, and auditable receipts.

## Final Target Architecture

```
Linux Base
├── Fedora Atomic / Universal Blue / BlueBuild
├── systemd, SELinux, cgroups v2, Wayland, PipeWire
│
├── OSAI AI Kernel
│   ├── Intent Parser
│   ├── Planner
│   ├── OSAI Plan DSL
│   ├── Model Router
│   ├── Memory Manager
│   ├── Agent Scheduler
│   └── Receipt Logger
│
├── Models
│   ├── Gemma 4 E2B (current validated default)
│   ├── Gemma 4 E4B (future local default, desktop 16-24GB VRAM)
│   ├── Gemma 4 26B (future performance, server/80GB+ VRAM)
│   └── MiniMax-M2.7 (cloud fallback)
│
├── Agent Runtime
│   ├── OpenClaw Gateway
│   ├── OSAI Agent Apps
│   ├── Agent permissions
│   ├── Agent memory scopes
│   └── Agent marketplace
│
├── Tool Layer
│   ├── ToolBroker
│   ├── Tool Drivers
│   ├── MCP adapters, D-Bus adapters, Linux portals
│   └── Computer-use subsystem (visible + hidden modes)
│
└── UX Layer
    ├── AI Command Bar
    ├── Voice push-to-talk
    ├── Mouse/keyboard intents
    ├── Memory Center
    ├── Agent Center
    ├── Receipt Viewer
    └── Desktop shell integration
```

## Local-First Principle

OSAI defaults to local computation. Cloud is an optional fallback, not the primary path.

- **Current validated default local model**: Gemma 4 E2B Q8 GGUF via llama.cpp
- **Future local default**: Gemma 4 E4B Q8 GGUF (desktop with 16-24GB VRAM)
- **Future performance local model**: Gemma 4 26B Q8 GGUF (server/80GB+ VRAM, when plugged in or explicitly requested)
- **Cloud fallback**: MiniMax-M2.7 (explicit, policy-controlled)

Local-first means:
- No cloud dependency for normal operation
- No mandatory internet connection
- All sensitive data stays on the user's machine
- Receipts and audit logs are local by default
- Privacy is the default, not an option

## Why the CLI MVP Exists Now

The current OSAI MVP exists as a CLI because:

1. **Proven the core loop works** — chat, ask, plan, authorize, apply, receipts
2. **Fast iteration** — no UI complexity, pure logic
3. **Testable** — automated tests catch regressions
4. **Extracted core logic** — ToolBroker, ToolExecutor, Receipt Logger are all reusable
5. **Foundation for API** — CLI logic will refactor into osai-agent-core, then osai-api

The CLI is not the final product. It is a working prototype of the core reasoning loop.

## Why Web UI Is Not the Final Product

The current `osai-api` includes a Dev Panel web UI for local development and debugging. This is intentionally limited:

- **Web UI is for development only** — Not the final desktop experience
- **Desktop integration is the goal** — COSMIC, KDE Plasma, or GNOME shell integration
- **Session-level AI assistant** — OSAI should feel like part of the desktop environment, not a separate tab
- **Native notifications, panels, keybindings** — Web apps cannot access these well
- **Computer use requires desktop integration** — Visible and hidden computer use need native compositor access

Future desktop integration options (in priority order):
1. **COSMIC** (System76) — Rust, Wayland, modular, aligns with OSAI's Rust stack
2. **KDE Plasma** — Mature, plasmoids, QML
3. **Cinnamon** — Traditional, forkable
4. **GNOME** — Powerful but opinionated
5. **Xfce** — Lightweight but less AI-OS oriented

Recommendation: Build osai-agent-core and osai-api first. Desktop integration comes after the core logic is stable and the API is proven.

## Computer Use: Core Future Capability

Computer use is a planned core OSAI capability, not an afterthought. It must be designed safely from the start.

### Two Modes

**1. Visible Computer Use**
- OSAI operates in the same desktop/session the user sees
- The user can watch actions, interrupt, and cancel
- Sensitive actions require explicit approval
- Useful for teaching, collaboration, and transparent assistance

**2. Hidden/Isolated Computer Use**
- OSAI operates in a separate isolated environment (nested Wayland compositor, virtual display, containerized desktop, VM, or separate Linux user/session)
- The user does not need to watch every step
- OSAI returns final outputs, artifacts, and summaries
- Must remain auditable, permissioned, cancellable, and constrained

### Safety Requirements for Computer Use

Every computer-use task must:
- Start as a Plan DSL plan, validated before execution
- Go through ToolBroker authorization
- Require user approval for sensitive categories
- Follow strict network/browser/file policies
- Never perform credential entry without explicit user action
- Never perform purchases/payments/account changes without explicit approval
- Never perform destructive changes without explicit approval
- Generate complete receipts with screenshots/artifacts under privacy controls
- Remain cancellable at any time
- Allow the hidden environment to be reset or destroyed
- Sanitize outputs before external transmission

### Computer Use Privacy

Screenshots may contain private data. Hidden sessions may process private documents. Browser sessions may expose cookies or account information.

Required controls:
- Minimize capture and transmission of sensitive data
- Cloud use with screenshots or desktop state requires explicit opt-in
- User must have controls for retention and deletion of artifacts

## Current Implemented State

See [CURRENT_STATE.md](CURRENT_STATE.md) for what is currently implemented.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the phased implementation plan.