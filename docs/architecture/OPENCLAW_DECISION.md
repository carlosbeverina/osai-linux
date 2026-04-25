# OpenClaw Integration Decision

> **Status**: Decided
> **Date**: 2026-04-25
> **Deciders**: OSAI Project

## 1. Decision Summary

- **OSAI will not depend on OpenClaw for the MVP core runtime.**
- **OSAI core runtime remains fully independent** - it does not require OpenClaw to function.
- **OpenClaw may be supported later as an optional bridge/integration.**
- **OpenClaw must never receive direct Linux/system permissions** - all actions must flow through OSAI's permission system.

## 2. Rationale

### 2.1 OSAI Already Has Complete Runtime

OSAI MVP already implements a complete agent runtime:

- **Plan DSL** - Typed, validated plan representation
- **ToolBroker** - Policy-based authorization gate
- **ToolExecutor** - Safe action execution with receipt generation
- **Receipt Logger** - Complete audit trail
- **Model Router** - Local/cloud model routing with mock mode
- **Agent CLI** - Command-line interface for plan execution

This runtime is self-contained and does not need OpenClaw to function.

### 2.2 Avoid Duplication and Responsibility Conflict

OpenClaw brings its own runtime responsibilities:
- Agent session management
- Tool execution
- Model integration
- Permission handling

Integrating OpenClaw as core would either:
1. Duplicate OSAI runtime responsibilities, or
2. Replace OSAI runtime, losing the safety properties we built

OSAI's runtime is designed specifically for Linux system integration with typed tools, receipts, and sandboxing. OpenClaw's goals overlap but differ in scope.

### 2.3 MVP Focus

The MVP should prioritize:
1. Local model provider integration (Gemma via Ollama or direct)
2. Memory Manager design and implementation
3. OSAI Command Bar UI
4. Voice Daemon (push-to-talk)
5. Fedora/Universal Blue packaging and VM image

OpenClaw integration can come after these core user-facing features.

### 2.4 Security Requires OSAI Permission Gate

**Critical**: All agent actions must pass through OSAI's ToolBroker. OpenClaw (or any external system) must never be able to:
- Execute shell commands directly
- Access filesystem without ToolBroker authorization
- Call models outside the Model Router
- Perform actions without receipts

If OpenClaw ever becomes supported, it will be as a client that sends plans to OSAI, not as a controlling runtime.

## 3. Future Integration Model

If OpenClaw support is added in the future, the integration model is:

```
┌─────────────────┐
│  OpenClaw       │
│  Gateway        │
│  (External)     │
└────────┬────────┘
         │ localhost only
         ▼
┌─────────────────┐
│  OSAI OpenClaw  │  (optional, disabled by default)
│  Bridge         │
└────────┬────────┘
         │ Convert to Plan DSL
         ▼
┌─────────────────┐
│  OSAI Plan DSL  │  (validated)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  ToolBroker     │  (authorization required)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  ToolExecutor   │  (safe subset only)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Receipt Logger │  (complete audit trail)
└─────────────────┘
```

**Note**: The OSAI OpenClaw Bridge does not exist yet. This diagram describes the target architecture if integration is pursued.

## 4. Rules for Future OpenClaw Bridge

If an OpenClaw bridge is implemented in the future, it **must** follow these rules:

| Rule | Rationale |
|------|----------|
| localhost only | Prevent remote exploitation |
| optional install | Users who don't want OpenClaw aren't affected |
| disabled by default | Explicit opt-in required |
| no direct shell | All shell via ToolBroker sandboxing |
| no direct filesystem writes | All writes via ToolBroker approval + rollback |
| no direct model calls | All model traffic via Model Router |
| every action converted to OSAI Plan DSL | Consistent validation and authorization |
| every action authorized by ToolBroker | Policy enforcement for all actions |
| every action logged with receipts | Complete audit trail |

**Non-negotiable**: OpenClaw bridge will never be allowed to bypass OSAI's security model.

## 5. Deferred Milestone

**OpenClaw Bridge (M4)** is deferred until after:

| Milestone | Description |
|-----------|-------------|
| M5 | Local Model Provider - Gemma 4 integration (real, not mock) |
| M6 | Memory Manager - Scoped, inspectable agent memory |
| M7 | OSAI Command Bar - Core UI for agent interaction |
| M8 | Voice Daemon - Push-to-talk voice intent |
| M9 | Fedora/Universal Blue Image - Installable base image |
| M10 | VM Test Image - Pre-built testing VM |

Only after these milestones should OpenClaw bridge be considered.

## 6. Open Questions

These remain open until bridge development begins:

1. **Protocol**: What wire format does OpenClaw use? HTTP/WebSocket? gRPC?
2. **Authentication**: How does OSAI verify OpenClaw requests?
3. **Session Management**: Does OpenClaw manage sessions or does OSAI?
4. **Bidirectional**: Can OSAI send plans to OpenClaw, or only receive?
5. **Fallback**: What happens if OpenClaw is unavailable?

## 7. Related Documents

- [OSAI MVP Specification](OSAI_MVP_SPEC.md) - Core runtime architecture
- [Security Model](OSAI_MVP_SPEC.md#5-security-model-v01) - Authorization and receipt requirements
