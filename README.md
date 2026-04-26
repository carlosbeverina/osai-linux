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

## Local Model Runtimes

OSAI supports two local model runtimes:

| Runtime | Best For | Port | Model Format |
|---------|----------|------|--------------|
| **llama.cpp** | Laptops, resource-constrained | 8092 | GGUF quantized |
| **vLLM** | Desktops, high throughput | 8091 | FP16/FP8 |

The default provider is **llama.cpp** (set via `OSAI_LOCAL_PROVIDER=llamacpp`).

## Local One-Command Runtime

For development and testing, a single command starts both llama.cpp and Model Router:

```bash
# Start llama.cpp + Model Router together
./scripts/osai-local-up

# Run E2E validation
./scripts/osai-local-check

# Stop (if started with systemd; for foreground, use Ctrl+C)
./scripts/osai-local-down
```

This starts:
- **llama.cpp** on `127.0.0.1:8092` serving a GGUF model
- **Model Router** on `127.0.0.1:8088` routing to real llama.cpp

The default GGUF model is **Gemma 4 E2B Q8 GGUF**, validated at:

```
.local-models/llamacpp/gemma-4-e2b-it/gemma-4-E2B-it-Q8_0.gguf
```

A small Qwen2.5-0.5B GGUF is kept as a smoke-test fallback for quick validation.

**Model files are not committed to git** — download GGUF files separately and place them at the expected paths.

Uses **llama.cpp as the laptop default** (no CUDA required for CPU build).

For full validation documentation, see [docs/testing/LLAMACPP_E2E_VALIDATION.md](docs/testing/LLAMACPP_E2E_VALIDATION.md).

## llama.cpp Local Runtime (Laptop Default)

OSAI uses **llama.cpp** as the default local runtime for laptops and resource-constrained environments. It provides excellent CPU/GPU support with quantized GGUF models.

### Installing llama.cpp

```bash
# Clone llama.cpp
git clone https://github.com/ggml-org/llama.cpp.git .local-runtimes/llama.cpp

# Build with CUDA support
cd .local-runtimes/llama.cpp
cmake -B build -DGGML_CUDA=ON
cmake --build build -j
```

### llama.cpp Scripts

```bash
# Load llama.cpp environment variables
source ./scripts/osai-llamacpp-env

# Start llama-server in foreground (requires llama-server built)
./scripts/osai-llamacpp-up

# Check if llama-server is running and responsive
./scripts/osai-llamacpp-check

# Stop osai-llamacpp systemd service (if active)
./scripts/osai-llamacpp-down
```

### Using Real llama.cpp with Model Router

By default, Model Router uses mock mode (`OSAI_LOCAL_MOCK=true`). To use real llama.cpp:

1. Start llama.cpp: `./scripts/osai-llamacpp-up`
2. Create `~/.config/osai/model-router.env`:
   ```
   OSAI_LOCAL_PROVIDER=llamacpp
   OSAI_LOCAL_MOCK=false
   OSAI_LLAMACPP_BASE_URL=http://127.0.0.1:8092/v1
   OSAI_LLAMACPP_MODEL=gemma-4-E2B-it-Q8_0.gguf
   OSAI_LLAMACPP_API_KEY=osai-local-dev-token
   ```
3. Restart Model Router: `systemctl --user restart osai-model-router.service`

**Note**: llama.cpp must be built manually. These scripts do not build llama.cpp or download models.

## vLLM Local Runtime (Performance Backend)

vLLM provides excellent throughput for GPU-accelerated inference on desktops with dedicated GPUs. Use when llama.cpp is insufficient.

### Installing vLLM

```bash
mkdir -p .local-runtimes/vllm
python3 -m venv .local-runtimes/vllm/.venv
source .local-runtimes/vllm/.venv/bin/activate
python -m pip install --upgrade pip setuptools wheel
python -m pip install vllm
```

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

### Using Real vLLM with Model Router

1. Start vLLM: `./scripts/osai-vllm-up`
2. Create `~/.config/osai/model-router.env`:
   ```
   OSAI_LOCAL_PROVIDER=vllm
   OSAI_LOCAL_MOCK=false
   OSAI_VLLM_BASE_URL=http://127.0.0.1:8091/v1
   OSAI_VLLM_MODEL=gemma-local
   OSAI_VLLM_API_KEY=osai-local-dev-token
   ```
3. Restart Model Router: `systemctl --user restart osai-model-router.service`

### Systemd User Service

```bash
# Install systemd user units (includes model-router, llama.cpp, and vllm services)
./scripts/osai-install-user-services

# Enable and start model-router at login
systemctl --user enable --now osai-model-router.service

# Check llama.cpp service status
systemctl --user status osai-llamacpp.service

# View llama.cpp logs
journalctl --user -u osai-llamacpp -f
```

### End-to-End Validation

After starting a local runtime and Model Router, run the E2E validation:

```bash
# For llama.cpp (default)
./scripts/osai-e2e-vllm-check
```

For full validation instructions, see [docs/testing/VLLM_E2E_VALIDATION.md](docs/testing/VLLM_E2E_VALIDATION.md).

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
