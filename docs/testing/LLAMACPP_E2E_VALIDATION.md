# OSAI llama.cpp End-to-End Validation

This document describes how to validate the OSAI llama.cpp local runtime integration end-to-end.

## Architecture Path

```
osai-agent CLI tool run
  → ToolBroker
  → ToolExecutor ModelChat
  → Model Router (OSAI_LOCAL_PROVIDER=llamacpp, OSAI_LOCAL_MOCK=false)
  → llama.cpp llama-server (CUDA build)
  → Qwen2.5-0.5B GGUF Q4_K_M
  → receipts
```

## Hardware & Runtime

| Component | Details |
|-----------|---------|
| OS | elementary OS 8 (Ubuntu 24.04 base) |
| GPU | NVIDIA RTX 4060 Laptop, 8 GB VRAM |
| NVIDIA driver | 580.126.09 |
| CUDA Toolkit | 13.0 |
| nvcc | V13.0.88 |
| llama.cpp | Repo-local CUDA build from `.local-runtimes/llama.cpp` |
| Model | Qwen2.5-0.5B-Instruct GGUF Q4_K_M |
| Model path | `.local-models/llamacpp/qwen2.5-0.5b-instruct/qwen2.5-0.5b-instruct-q4_k_m.gguf` |
| Ports | llama.cpp: 8092, Model Router: 8088 |

## Quick Start

```bash
# 1. Ensure GGUF model is present at the expected path
ls .local-models/llamacpp/qwen2.5-0.5b-instruct/qwen2.5-0.5b-instruct-q4_k_m.gguf

# 2. Build llama.cpp (if not already built)
#    See README.md "Installing llama.cpp" section

# 3. Start both services (llama.cpp + Model Router)
./scripts/osai-local-up

# 4. Run validation checks
./scripts/osai-local-check

# 5. Stop services (Ctrl+C in the osai-local-up terminal,
#    or use osai-local-down for systemd services)
./scripts/osai-local-down
```

## Prerequisites

### 1. Build llama.cpp

```bash
git clone https://github.com/ggml-org/llama.cpp.git .local-runtimes/llama.cpp
cd .local-runtimes/llama.cpp
cmake -B build -DGGML_CUDA=ON
cmake --build build -j
```

### 2. Install Model Router dependencies

```bash
cd services/model-router
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
cd ../..
```

### 3. Download GGUF model

The GGUF model must already be downloaded. This is intentional — the scripts do not download models.

```bash
# Example: Qwen2.5-0.5B-Instruct Q4_K_M GGUF
# Download from Hugging Face and place at:
# .local-models/llamacpp/qwen2.5-0.5b-instruct/qwen2.5-0.5b-instruct-q4_k_m.gguf
```

## Step-by-Step (Manual)

### Terminal A: Start llama.cpp + Model Router

```bash
cd /home/carlosbeverina/Projects/osai-linux
./scripts/osai-local-up
```

Wait for: `llama-server is ready` and `Model Router is ready`

### Terminal B: Run checks

```bash
cd /home/carlosbeverina/Projects/osai-linux
./scripts/osai-local-check
```

### Terminal C: Inspect receipts (optional)

```bash
# Tool receipts
ls -la /tmp/osai-tool-receipts-llamacpp/
cat /tmp/osai-tool-receipts-llamacpp/*.json

# Model Router receipts
ls -la /tmp/osai-model-router-receipts-llamacpp/
cat /tmp/osai-model-router-receipts-llamacpp/*.json
```

## End-to-End Validation Checks

`osai-local-check` verifies:

1. **llama.cpp `/v1/models`** — llama-server is responding on port 8092
2. **Model Router `/health`** — Model Router is responding on port 8088
3. **POST `/v1/chat/completions`** — Model Router routes to real llama.cpp (not mock)
4. **`cargo run -p osai-agent-cli -- tool run`** — CLI executes plan through Model Router
5. **Tool receipts exist** — receipts created under `/tmp/osai-tool-receipts-llamacpp/`
6. **Model Router receipts exist** — receipts under `/tmp/osai-model-router-receipts-llamacpp/`
7. **No secrets in receipts** — grep for password, api_key, token, secret, credential

## Known Issues

### vLLM Gemma 4 E2B — VRAM OOM

**Issue**: vLLM with Gemma 4 E2B failed due to CUDA out-of-memory on the RTX 4060 (8GB VRAM).

**Workaround**: Use llama.cpp with quantized GGUF models instead. llama.cpp handles GPU memory more efficiently and supports CPU fallback.

## Validation Commands

### Verify CUDA build

```bash
ldd .local-runtimes/llama.cpp/build/bin/llama-server | grep -Ei "cuda|cublas|cudart"
```

Expected: lines containing `cuda`, `cublas`, or `cudart` — confirming the CUDA-enabled build.

### Run E2E validation

```bash
./scripts/osai-local-check
```

All 7 checks must pass.

## Observed Performance

- Direct llama.cpp request: ~328 predicted tokens/s on Qwen2.5-0.5B GGUF Q4_K_M
- llama.cpp CUDA build is validated, but the first test GGUF is small and not the final OSAI model

## Next Steps

1. **Test larger GGUF models** (e.g., Gemma 4 E2B/E4B GGUF)
2. **Test Gemma GGUF** models for comparison
3. **Add model selection policy** for laptop profiles (battery vs. performance)
4. **Benchmark** CPU vs GPU llama.cpp performance on RTX 4060
5. **Validate vLLM** on desktop with dedicated GPU when available
