# OSAI Model Router

OpenAI-compatible API gateway that routes LLM requests to local or cloud providers based on model alias and metadata hints.

## What is Model Router?

Model Router is an OSAI service that provides a unified OpenAI-compatible API endpoint (`/v1/chat/completions`) and intelligently routes requests to:

- **Local models** (Gemma 4 variants) via mock provider
- **Cloud models** (MiniMax) via MiniMax API

## Why Separate Routing from Agents?

OSAI separates model routing from agent execution for several architectural reasons:

1. **Policy enforcement** - Centralized control over which models can be used and when
2. **Cost control** - Route to cheaper local models when appropriate, cloud for complex tasks
3. **Privacy** - Route sensitive requests to local models only
4. **Performance** - Auto-select fast models for simple tasks, powerful models for complex ones
5. **Audit completeness** - All model requests logged with routing decisions

## Local vs Cloud Routing

### Local Models (osai-local, gemma4:*)

- Processed by `LocalMockProvider` (Ollama integration planned)
- Zero API cost
- Full privacy (no data leaves the machine)
- Lower latency for simple tasks
- Mock responses in v0.1 MVP

### Cloud Models (MiniMax-M2.7, MiniMax-M2.7-highspeed)

- Processed by `MiniMaxProvider` via OpenAI-compatible API
- Requires `MINIMAX_API_KEY`
- Higher capability for complex reasoning
- Network latency involved

### Auto-Routing (osai-auto)

The `osai-auto` model uses metadata hints to route intelligently:

| Metadata | Route |
|----------|-------|
| `privacy: "local_only"` | Local (gemma4:e4b) |
| `complexity: "high"` | Cloud (MiniMax-M2.7) |
| `speed: "fast"` | Fast cloud (MiniMax-M2.7-highspeed) |
| (none) | Local (gemma4:e4b) |

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
OSAI_MODEL_ROUTER_MOCK_CLOUD=true
```

When `true`, cloud model requests return mock responses without calling the MiniMax API.

## Receipts

Every chat completion request writes a JSON receipt to:

- `$OSAI_RECEIPTS_DIR` if set
- Otherwise `~/.local/share/osai/receipts/model-router/`

Receipts include:
- Request ID, timestamp, provider
- Model routing decisions
- Privacy/complexity/speed metadata
- Status (executed/failed)
- **Never** the full prompt content

## Running Locally

### Setup

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
```

### Run

```bash
# With mock cloud (default)
uvicorn osai_model_router.main:app --host 127.0.0.1 --port 8088

# Or run directly
python -m osai_model_router.main
```

### Test

```bash
pytest tests/
```

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

- **Do not store or log full prompts** - Only message count and roles are recorded
- **Do not log API keys** - Keys are never written to logs or receipts
- **Bind to localhost only** - Service listens on 127.0.0.1 only
- **Do not commit secrets** - Use `.env` files excluded from version control

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/v1/models` | List available models |
| POST | `/v1/chat/completions` | OpenAI-compatible chat completions |

## Available Models

- `osai-local` - Local default (gemma4:e4b)
- `osai-cloud` - Cloud default (MiniMax-M2.7)
- `osai-auto` - Auto-route based on metadata
- `gemma4:e2b` - Gemma 4 E2B local
- `gemma4:e4b` - Gemma 4 E4B local
- `gemma4:26b` - Gemma 4 26B local
- `MiniMax-M2.7` - MiniMax standard
- `MiniMax-M2.7-highspeed` - MiniMax fast variant
