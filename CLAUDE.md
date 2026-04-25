# OSAI Linux - Claude Code Project Context

## Project identity

OSAI is an AI-native Linux distribution where agents are first-class applications, natural language becomes a programmable interface, and every AI action is mediated through typed tools, explicit permissions, memory scopes, sandboxing, and auditable receipts.

## Current phase

We are building OSAI from the ground up as a full Linux distribution project, but not as Linux From Scratch.

The first technical goal is:

1. Implement OSAI Plan DSL.
2. Implement Receipt Logger.
3. Implement ToolBroker.
4. Implement OSAI Agent CLI.
5. Add Model Router.
6. Add OpenClaw Bridge.
7. Add Voice Daemon.
8. Add OSAI Command Bar.
9. Package into a Fedora Atomic / Universal Blue / BlueBuild based image.
10. Build installer flow for dual boot with Windows.

## Target architecture

OSAI Linux
├── Linux Base
│   ├── Fedora Atomic / Universal Blue
│   ├── systemd
│   ├── SELinux
│   ├── cgroups v2
│   ├── Wayland
│   └── PipeWire
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
│   ├── Gemma 4 E2B background
│   ├── Gemma 4 E4B local default
│   ├── Gemma 4 26B local performance
│   └── MiniMax-M2.7 cloud
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
│   ├── Tool APIs
│   ├── MCP adapters
│   ├── D-Bus adapters
│   └── Linux portals
│
├── UX Layer
│   ├── AI Command Bar
│   ├── Voice push-to-talk
│   ├── mouse/keyboard intents
│   ├── Memory Center
│   ├── Agent Center
│   └── Receipt Viewer
│
└── Installer / Distro
    ├── ISO
    ├── dual boot installer
    ├── image updates
    ├── rollback
    └── post-install model setup

## Language choices

Use Rust for critical infrastructure:

- osai-toolbroker
- osai-plan-dsl
- osai-receipt-logger
- osai-agent-cli

Use TypeScript for:

- OpenClaw Bridge
- OSAI Command Bar frontend
- GNOME integration

Use Python only where it accelerates prototyping:

- Model Router MVP
- voice experiments
- model testing scripts

## Security rules

Never allow the model or agents to execute host shell commands directly.

Every action must pass through ToolBroker.

Every action must produce a receipt.

Do not store secrets in the repository.

Do not log full prompts by default.

Do not add API keys, tokens, private files, local model weights, or generated build directories to Git.

Cloud model use must be explicit and policy-controlled.

## Local model policy

Default local model: Gemma 4 E4B.
Background local model: Gemma 4 E2B.
Performance local model: Gemma 4 26B, only when plugged in or explicitly requested.
Cloud model: MiniMax-M2.7.
Fast cloud model: MiniMax-M2.7-highspeed.

## First implementation priority

Implement the Plan DSL before implementing ToolBroker.

The Plan DSL should define a safe, typed intermediate representation between natural language and actual system actions.

Flow:

Natural language
→ interpretation by model
→ OSAI Plan DSL
→ validation
→ simulation
→ user approval
→ execution through ToolBroker
→ receipt

## Development rules

Before changing code:
- Inspect the relevant files.
- Keep changes small.
- Update tests when adding behavior.
- Run cargo fmt.
- Run cargo check before finishing.
- Do not modify unrelated files.
- Do not introduce unsafe Rust unless explicitly justified.
