# OSAI Model Router

OpenAI-compatible API gateway that routes LLM requests to local or cloud providers based on model alias and metadata hints.

## What is Model Router?

Model Router is an OSAI service that provides a unified OpenAI-compatible API endpoint (`/v1/chat/completions`) and intelligently routes requests to:

- **Local models** (Gemma 4 variants) via vLLM
- **Cloud models** (MiniMax) via MiniMax API

## Architecture Decision: vLLM over Ollama

OSAI uses **vLLM** as the primary local model backend, not Ollama. Reasons:

1. **OpenAI compatibility** - vLLM provides OpenAI-compatible API out of the box
2. **Performance** - vLLM is optimized for high-throughput serving
3. **Simplicity** - Single API format for both local and cloud (OpenAI-compatible)
4. **Future-proof** - Better suited for production deployment

Note: llama.cpp may be added later as a fallback for resource-constrained environments.

## Why Separate Routing from Agents?

OSAI separates model routing from agent execution for several architectural reasons:

1. **Policy enforcement** - Centralized control over which models can be used and when
2. **Cost control** - Route to cheaper local models when appropriate, cloud for complex tasks
3. **Privacy** - Route sensitive requests to local models only
4. **Performance** - Auto-select fast models for simple tasks, powerful models for complex ones
5. **Audit completeness** - All model requests logged with routing decisions

## Local vs Cloud Routing

### Local Models (osai-local, gemma4:*)

- Processed by `VllmProvider` via vLLM OpenAI-compatible API
- Zero API cost
- Full privacy (no data leaves the machine)
- Lower latency for simple tasks
- **Mock mode** (default): Returns simulated responses without calling vLLM
- **Real mode**: Calls vLLM server at `OSAI_VLLM_BASE_URL`

### Cloud Models (MiniMax-M2.7, MiniMax-M2.7-highspeed)

- Processed by `MiniMaxProvider` via OpenAI-compatible API
- Requires `MINIMAX_API_KEY`
- Higher capability for complex reasoning
- Network latency involved
- **Mock mode** (default): Returns simulated responses without calling MiniMax

### Auto-Routing (osai-auto)

The `osai-auto` model uses metadata hints to route intelligently:

| Metadata | Route |
|----------|-------|
| `privacy: "local_only"` | Local vLLM |
| `complexity: "high"` | Cloud (MiniMax-M2.7) |
| `speed: "fast"` | Fast cloud (MiniMax-M2.7-highspeed) |
| (none) | Local vLLM |

## Local vLLM Configuration

Configure via environment variables or `.env` file:

```bash
# Provider type (currently only vllm is supported)
OSAI_LOCAL_PROVIDER=vllm

# Mock local responses (true for testing/development)
OSAI_LOCAL_MOCK=true

# vLLM OpenAI-compatible base URL (must be loopback only)
OSAI_VLLM_BASE_URL=http://127.0.0.1:8091/v1

# Default model served by vLLM
OSAI_VLLM_MODEL=gemma-local

# API key for vLLM
OSAI_VLLM_API_KEY=osai-local-dev-token
```

**Security**: vLLM base URL must be loopback-only (localhost or 127.0.0.1). External URLs are rejected.

## Running vLLM Server

OSAI uses repo-local vLLM installation by default. To install vLLM:

```bash
mkdir -p .local-runtimes/vllm
python3 -m venv .local-runtimes/vllm/.venv
source .local-runtimes/vllm/.venv/bin/activate
python -m pip install --upgrade pip setuptools wheel
python -m pip install vllm
```

Start vLLM using the convenience script:

```bash
./scripts/osai-vllm-up
```

Or manually:

```bash
# Using repo-local vLLM
.local-runtimes/vllm/.venv/bin/vllm serve <model_name> \
    --host 127.0.0.1 \
    --port 8091 \
    --api-key osai-local-dev-token
```

Then disable mock mode:

```bash
OSAI_LOCAL_MOCK=false python -m osai_model_router.main
```

## Using Real vLLM Instead of Mock Mode

To use real vLLM instead of mock local responses:

```bash
# Disable mock mode
export OSAI_LOCAL_MOCK=false

# Configure vLLM connection
export OSAI_VLLM_BASE_URL=http://127.0.0.1:8091/v1
export OSAI_VLLM_MODEL=<your-local-model>
export OSAI_VLLM_API_KEY=osai-local-dev-token
```

Or via `~/.config/osai/model-router.env`:

```
OSAI_LOCAL_MOCK=false
OSAI_VLLM_BASE_URL=http://127.0.0.1:8091/v1
OSAI_VLLM_MODEL=gemma-local
OSAI_VLLM_API_KEY=osai-local-dev-token
```

Then restart the Model Router service or restart your development server.

**Note**: No model is downloaded by these scripts. You must install vLLM and download models separately.

## MiniMax Configuration

Configure via environment variables or `.env` file:

```bash
MINIMAX_API_KEY=your-api-key-here
MINIMAX_OPENAI_BASE_URL=https://api.minimax.io/v1
MINIMAX_MODEL=MiniMax-M2.7
MINIMAX_FAST_MODEL=MiniMax-M2.7-highspeed
```

**Security**: Never commit API keys. Use environment variables or a private `.env` file.

## Mock Mode

For testing and development without API calls:

```bash
# Mock local vLLM responses (default: true)
OSAI_LOCAL_MOCK=true

# Mock cloud MiniMax responses (default: true)
OSAI_MODEL_ROUTER_MOCK_CLOUD=true
```

When `true`, requests return mock responses without calling the actual provider.

## Receipts

Every chat completion request writes a JSON receipt to:

- `$OSAI_RECEIPTS_DIR` if set
- Otherwise `~/.local/share/osai/receipts/model-router/`

Receipts include:

- Request ID, timestamp, service name
- Selected provider (`VllmProvider` or `MiniMaxProvider`)
- Requested model and routed model
- Privacy, complexity, speed metadata hints
- Status (executed/failed)
- Input summary (message count, roles) - **no prompt content**
- Reasoning stripped flag
- Truncated flag
- Local provider info (provider type, mock mode, base URL host)

Receipts **never** contain:
- Full prompts or message content
- API keys
- Raw model responses

## Receipt Fields

| Field | Description |
|-------|-------------|
| `selected_provider` | `VllmProvider` or `MiniMaxProvider` |
| `local_provider` | `vllm` (for VllmProvider routes) |
| `local_mock` | `true` if local vLLM is mocked |
| `local_base_url_host` | Host of vLLM URL (e.g., `127.0.0.1`) |
| `routed_model` | Actual model used (may differ from requested) |
| `input_summary` | Message count and roles only |

## Running Locally

### Setup

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

### Run

```bash
# With mock providers (default)
uvicorn osai_model_router.main:app --host 127.0.0.1 --port 8088

# Or run directly
python -m osai_model_router.main
```

### Development Scripts

For convenience, use the OSAI development scripts:

```bash
# Start Model Router in foreground
./scripts/osai-dev-up

# Check health and functionality
./scripts/osai-dev-check

# Stop the service
./scripts/osai-dev-down
```

### Systemd User Service

For a persistent background service:

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

### Test

```bash
pytest tests/
```

**Note**: Tests do not call real vLLM or MiniMax endpoints. All external calls are mocked.

## API Examples

### Health Check

```bash
curl http://127.0.0.1:8088/health
```

### List Models

```bash
curl http://127.0.0.1:8088/v1/models
```

### Chat Completion (OpenAI-compatible)

```bash
curl http://127.0.0.1:8088/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "osai-local",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

### Auto-Routing with Metadata

```bash
curl http://127.0.0.1:8088/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "osai-auto",
    "messages": [{"role": "user", "content": "Hello"}],
    "metadata": {"privacy": "local_only"}
  }'
```

## Security Rules

- **Do not store or log full prompts** - Only message count and roles are recorded in receipts
- **Do not store raw model outputs** - Only metadata about the response is stored
- **Do not log API keys** - Keys are never written to logs or receipts
- **Bind to localhost only** - Service listens on 127.0.0.1 only
- **vLLM URL must be loopback** - External vLLM URLs are rejected
- **Do not commit secrets** - Use `.env` files excluded from version control
- **Strip thinking blocks** - Hidden reasoning is removed before returning responses

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/v1/models` | List available models |
| POST | `/v1/chat/completions` | OpenAI-compatible chat completions |

## Available Models

- `osai-local` - Local default (resolves to `OSAI_VLLM_MODEL`)
- `osai-cloud` - Cloud default (MiniMax-M2.7)
- `osai-auto` - Auto-route based on metadata
- `gemma4:e2b` - Gemma 4 E2B local (direct pass-through to vLLM)
- `gemma4:e4b` - Gemma 4 E4B local (direct pass-through to vLLM)
- `gemma4:26b` - Gemma 4 26B local (direct pass-through to vLLM)
- `MiniMax-M2.7` - MiniMax standard
- `MiniMax-M2.7-highspeed` - MiniMax fast variant
