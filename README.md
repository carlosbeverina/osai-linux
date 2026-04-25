# OSAI Linux

OSAI is an AI-native Linux distribution where agents are first-class applications, natural language becomes a programmable interface, and every AI action is mediated through typed tools, explicit permissions, memory scopes, sandboxing, and auditable receipts.

## Initial architecture

- Linux base: Fedora Atomic / Universal Blue / BlueBuild
- Local models: Gemma 4 E2B, Gemma 4 E4B, Gemma 4 26B
- Cloud model: MiniMax-M2.7
- Agent runtime: OpenClaw
- Safety layer: OSAI ToolBroker
- UX: voice, mouse and keyboard
- Core UI: OSAI Command Bar
- Memory: scoped, inspectable and user-controlled
- Auditability: receipts for every AI action

## Rust workspace

Current crates:

- `osai-toolbroker`
- `osai-plan-dsl`
- `osai-receipt-logger`
- `osai-agent-cli`
