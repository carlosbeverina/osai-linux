# OSAI Linux

OSAI is an AI-native Linux distribution where agents are first-class applications, natural language becomes a programmable interface, and every AI action is mediated through typed tools, explicit permissions, memory scopes, sandboxing, and auditable receipts.

## Architecture

For full system architecture and MVP specification, see:

- [OSAI MVP Specification v0.1](docs/architecture/OSAI_MVP_SPEC.md)

**Note**: OpenClaw is not part of OSAI MVP core. See [OpenClaw Integration Decision](docs/architecture/OPENCLAW_DECISION.md) for rationale.

## Initial architecture

- Linux base: Fedora Atomic / Universal Blue / BlueBuild
- Local models: Gemma 4 E2B, Gemma 4 E4B, Gemma 4 26B
- Cloud model: MiniMax-M2.7
- Agent runtime: OSAI core (independent, not OpenClaw-dependent)
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

## vLLM Local Runtime

OSAI uses **vLLM** as the primary local model runtime (not Ollama). vLLM provides OpenAI-compatible API for seamless routing.

### Installing vLLM

OSAI prefers a repo-local vLLM installation:

```bash
mkdir -p .local-runtimes/vllm
python3 -m venv .local-runtimes/vllm/.venv
source .local-runtimes/vllm/.venv/bin/activate
python -m pip install --upgrade pip setuptools wheel
python -m pip install vllm
```

This installs vLLM into `.local-runtimes/vllm/.venv/bin/vllm`. The scripts will automatically use this path.

### vLLM Scripts

```bash
# Load vLLM environment variables
source ./scripts/osai-vllm-env

# Start vLLM in foreground (requires vllm installed)
./scripts/osai-vllm-up

# Check if vLLM is running and responsive
./scripts/osai-vllm-check

# Stop osai-vllm systemd service (if active)
./scripts/osai-vllm-down
```

### End-to-End Validation

After starting vLLM and Model Router, run the E2E validation:

```bash
# Assumes vLLM is running at http://127.0.0.1:8091
# Assumes Model Router is running at http://127.0.0.1:8088
./scripts/osai-e2e-vllm-check
```

For full validation instructions, see [docs/testing/VLLM_E2E_VALIDATION.md](docs/testing/VLLM_E2E_VALIDATION.md).

### Systemd User Service

```bash
# Install systemd user units (includes both model-router and vllm)
./scripts/osai-install-user-services

# Enable and start model-router at login
systemctl --user enable --now osai-model-router.service

# Check vLLM service status
systemctl --user status osai-vllm.service

# View vLLM logs
journalctl --user -u osai-vllm -f
```

### Using Real vLLM with Model Router

By default, Model Router uses mock mode (`OSAI_LOCAL_MOCK=true`). To use real vLLM:

1. Start vLLM: `./scripts/osai-vllm-up`
2. Create `~/.config/osai/model-router.env`:
   ```
   OSAI_LOCAL_MOCK=false
   OSAI_VLLM_BASE_URL=http://127.0.0.1:8091/v1
   OSAI_VLLM_MODEL=gemma-local
   OSAI_VLLM_API_KEY=osai-local-dev-token
   ```
3. Restart Model Router: `systemctl --user restart osai-model-router.service`

**Note**: vLLM must be installed manually. These scripts do not install vLLM or download models.

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
