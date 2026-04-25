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

## Model Router Development

The Model Router service is in `services/model-router/`. Use the development scripts:

```bash
# Start Model Router in foreground
./scripts/osai-dev-up

# Check health and functionality
./scripts/osai-dev-check

# Stop the service
./scripts/osai-dev-down
```

### Systemd User Service

```bash
# Install systemd user units
./scripts/osai-install-user-services

# Enable and start at login
systemctl --user enable --now osai-model-router.service

# Check status
systemctl --user status osai-model-router.service

# View logs
journalctl --user -u osai-model-router -f
```

**Mock mode is the default** (`OSAI_MODEL_ROUTER_MOCK_CLOUD=true`) to avoid accidental MiniMax spend during development.

To opt into real MiniMax, set `MINIMAX_API_KEY` in your `.env` and either:
- Pass `OSAI_MODEL_ROUTER_MOCK_CLOUD=false` to `osai-dev-up`
- Or create `~/.config/osai/model-router.env` with `OSAI_MODEL_ROUTER_MOCK_CLOUD=false`
