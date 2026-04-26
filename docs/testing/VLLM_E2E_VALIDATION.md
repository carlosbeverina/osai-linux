# OSAI vLLM End-to-End Validation

This document describes how to validate the OSAI vLLM local runtime integration end-to-end.

## Architecture Path

```
vLLM real
  → Model Router with OSAI_LOCAL_MOCK=false
  → osai-agent CLI tool run
  → examples/plans/model-chat.yml
  → ToolExecutor ModelChat
  → Model Router
  → vLLM
  → receipts
```

## Prerequisites

### 1. Install vLLM (repo-local)

```bash
mkdir -p .local-runtimes/vllm
python3 -m venv .local-runtimes/vllm/.venv
source .local-runtimes/vllm/.venv/bin/activate
python -m pip install --upgrade pip setuptools wheel
python -m pip install vllm
```

### 2. Install Model Router dependencies

```bash
cd services/model-router
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
cd ../..
```

### 3. Download a model for testing

vLLM requires model files to be downloaded. Use Hugging Face:

```bash
# Example: Qwen2.5-0.5B-Instruct (small, fast for testing)
# This downloads ~1GB to ~/.cache/huggingface/
huggingface-cli download Qwen/Qwen2.5-0.5B-Instruct
```

## Step 1: Start vLLM

In **Terminal A**:

```bash
cd /home/carlosbeverina/Projects/osai-linux

# Activate vLLM environment
source .local-runtimes/vllm/.venv/bin/activate

# Set environment
export OSAI_VLLM_MODEL="Qwen/Qwen2.5-0.5B-Instruct"
export OSAI_VLLM_HOST="127.0.0.1"
export OSAI_VLLM_PORT="8091"
export OSAI_VLLM_API_KEY="osai-local-dev-token"
export OSAI_VLLM_EXTRA_ARGS="--dtype auto --gpu-memory-utilization 0.65"

# Start vLLM
vllm serve "$OSAI_VLLM_MODEL" \
    --host "$OSAI_VLLM_HOST" \
    --port "$OSAI_VLLM_PORT" \
    --api-key "$OSAI_VLLM_API_KEY" \
    $OSAI_VLLM_EXTRA_ARGS
```

Wait for: `Uvicorn running on http://127.0.0.1:8091`

## Step 2: Verify vLLM

In **Terminal B**:

```bash
cd /home/carlosbeverina/Projects/osai-linux

# Check vLLM is responding
curl -s -H "Authorization: Bearer osai-local-dev-token" \
    http://127.0.0.1:8091/v1/models | head -c 200

# Or use the check script
./scripts/osai-vllm-check
```

Expected: HTTP 200, JSON with model list

## Step 3: Start Model Router

In **Terminal C**:

```bash
cd /home/carlosbeverina/Projects/osai-linux/services/model-router
source .venv/bin/activate

export OSAI_LOCAL_PROVIDER=vllm
export OSAI_LOCAL_MOCK=false
export OSAI_VLLM_BASE_URL=http://127.0.0.1:8091/v1
export OSAI_VLLM_MODEL=Qwen/Qwen2.5-0.5B-Instruct
export OSAI_VLLM_API_KEY=osai-local-dev-token
export OSAI_MODEL_ROUTER_MOCK_CLOUD=true
export OSAI_RECEIPTS_DIR=/tmp/osai-model-router-receipts-real-vllm

uvicorn osai_model_router.main:app --host 127.0.0.1 --port 8088
```

Wait for: `Uvicorn running on http://127.0.0.1:8088`

## Step 4: Run E2E Validation

In **Terminal D**:

```bash
cd /home/carlosbeverina/Projects/osai-linux

# Run the automated E2E check
./scripts/osai-e2e-vllm-check
```

This script validates:
1. vLLM `/models` endpoint responds
2. Model Router `/health` responds
3. Model Router returns real vLLM response (not mock)
4. CLI `tool run` executes successfully
5. Receipts are created
6. No secrets leak into receipts

## Manual Verification

### Test Model Router directly

```bash
# Test local model routing
curl -s -X POST http://127.0.0.1:8088/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "osai-auto",
    "messages": [{"role": "user", "content": "Hi"}],
    "metadata": {"privacy": "local_only"}
  }'
```

Expected: Real response from vLLM (not "OSAI vLLM local mock response")

### Inspect receipts

```bash
# Model Router receipts
ls -la /tmp/osai-model-router-receipts-real-vllm/
cat /tmp/osai-model-router-receipts-real-vllm/*.json | python -m json.tool

# Tool receipts
ls -la /tmp/osai-tool-receipts-vllm/
cat /tmp/osai-tool-receipts-vllm/*.json | python -m json.tool
```

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `curl: (7) Failed to connect to 127.0.0.1:8091` | vLLM not running | Start vLLM in Terminal A |
| `curl: (7) Failed to connect to 127.0.0.1:8088` | Model Router not running | Start Model Router in Terminal C |
| Mock response returned | `OSAI_LOCAL_MOCK=false` not set | Set env var in Terminal C |
| `{"detail":"Unauthorized"}` from vLLM | Wrong API key | Check `OSAI_VLLM_API_KEY` matches |
| CUDA OOM | GPU memory exhausted | Lower `--gpu-memory-utilization` or use smaller model |
| Model not found | Model not downloaded | Use `huggingface-cli download` to download model |

## Success Criteria

All checks in `osai-e2e-vllm-check` should show `[OK]`:

- vLLM /models returned HTTP 200
- Model Router /health returned HTTP 200
- Model Router returned real response (not mock)
- CLI tool run completed
- Tool receipts exist
- No obvious secrets in receipts
