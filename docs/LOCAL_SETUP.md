# Local Setup

## Repository Path

```bash
cd ~/Projects/osai-linux
```

## Prerequisites

### Rust
```bash
# Install via rustup if not present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Verify
rustc --version
cargo --version
```

### Python (for Model Router)
```bash
# Python 3.10+ recommended
python3 --version

# Create virtual environment
cd services/model-router
python3 -m venv .venv
source .venv/bin/activate

# Install dependencies
pip install -r requirements.txt
```

### NVIDIA CUDA (for GPU inference)
```bash
# CUDA 13.0+ required
nvcc --version

# Verify driver
nvidia-smi
```

### Local Models

Download Gemma 4 E2B Q8 GGUF:
```bash
mkdir -p .local-models/llamacpp/gemma-4-e2b-it
# Download from HuggingFace ggml-org/gemma-4-E2B-it-GGUF
# File: gemma-4-E2B-it-Q8_0.gguf
```

Download Qwen 2.5 0.5B (smoke test fallback):
```bash
mkdir -p .local-models/llamacpp/qwen2.5-0.5b-instruct
# Download from HuggingFace Qwen
# File: qwen2.5-0.5b-instruct-q4_k_m.gguf
```

**Do not commit these files.**

### Local Runtime (llama.cpp)

Clone and build llama.cpp:
```bash
git clone https://github.com/ggerganov/llama.cpp.git .local-runtimes/llama.cpp
cd .local-runtimes/llama.cpp

# Build with CUDA
export PATH=/usr/local/cuda-13.0/bin:$PATH
export LD_LIBRARY_PATH=/usr/local/cuda-13.0/lib64:${LD_LIBRARY_PATH:-}
cmake -B build -G Ninja \
  -DGGML_CUDA=ON \
  -DCMAKE_CUDA_ARCHITECTURES=89 \
  -DCMAKE_BUILD_TYPE=Release
cmake --build build -j"$(nproc)"
```

**Do not commit the `.local-runtimes/` directory.**

## Environment Setup

```bash
source scripts/osai-local-env
```

This sets:
```bash
OSAI_LLAMACPP_BASE_URL=http://127.0.0.1:8092/v1
OSAI_LLAMACPP_MODEL=gemma-4-E2B-it-Q8_0.gguf
OSAI_LLAMACPP_API_KEY=osai-local-dev-token
OSAI_MODEL_ROUTER_URL=http://127.0.0.1:8088
OSAI_LOCAL_PROVIDER=llamacpp
OSAI_LOCAL_MOCK=false
OSAI_LOCAL_TOOL_RECEIPTS_DIR=/tmp/osai-tool-receipts-llamacpp
OSAI_LOCAL_MODEL_RECEIPTS_DIR=/tmp/osai-model-router-receipts-llamacpp
```

## Build and Test

```bash
# Format check
cargo fmt --check

# Type check
cargo check --workspace

# Run tests
cargo test --workspace

# Model Router tests
cd services/model-router && pytest tests && cd -
```

## Start Local Runtime

```bash
# Start all local services (llama-server + Model Router)
./scripts/osai-local-up

# Validate everything is working
./scripts/osai-local-check

# Stop services
./scripts/osai-local-down
```

Or manage llama.cpp separately:
```bash
./scripts/osai-llamacpp-up
./scripts/osai-llamacpp-check
./scripts/osai-llamacpp-down
```

## Manual Service Start (Alternative)

```bash
# Start llama-server
.local-runtimes/llama.cpp/build/bin/llama-server \
  -m .local-models/llamacpp/gemma-4-e2b-it/gemma-4-E2B-it-Q8_0.gguf \
  -c 4096 \
  -ngl 99 \
  --host 127.0.0.1 \
  --port 8092 &

# Start Model Router
cd services/model-router
source .venv/bin/activate
python -m uvicorn main:app --host 127.0.0.1 --port 8088 &
```

## MVP Commands

```bash
# Chat
cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI local setup OK"

# Ask
cargo run -p osai-agent-cli -- ask --print-plan "Create a safe plan to list my Downloads folder"

# Doctor (validates setup)
cargo run -p osai-agent-cli -- doctor

# Validate plan
cargo run -p osai-agent-cli -- plan validate examples/plans/model-chat.yml
```

## Troubleshooting

### "llama.cpp not found"
```bash
# Verify build exists
ls -la .local-runtimes/llama.cpp/build/bin/llama-server

# Rebuild if needed
cd .local-runtimes/llama.cpp
cmake --build build -j"$(nproc)"
```

### "CUDA out of memory"
- Normal on RTX 4060 Laptop 8GB with larger models
- Use Gemma 4 E2B Q8 instead of E4B or 26B
- Close other GPU applications

### "Model Router connection failed"
```bash
# Check if Model Router is running
curl http://127.0.0.1:8088/health

# Restart services
./scripts/osai-local-down
./scripts/osai-local-up
```

### "pytest not found"
```bash
cd services/model-router
source .venv/bin/activate
pip install -r requirements.txt
```

### "OSAI_LLAMACPP_MODEL unbound"
```bash
source scripts/osai-local-env
```

## What Not to Commit

These files/directories are local-only and must never be committed:
```
.local-models/           # Model files (GGUF)
.local-runtimes/        # Built runtimes (llama.cpp, vLLM)
/tmp/osai-*/            # Temporary receipts
~/.local/share/osai/   # User receipts
~/.config/osai/         # Config with tokens
```

## Scripts Summary

| Script | Purpose |
|--------|---------|
| `osai-local-env` | Set environment variables |
| `osai-local-up` | Start all local services |
| `osai-local-check` | Validate all services |
| `osai-local-down` | Stop all local services |
| `osai-llamacpp-env` | llama.cpp environment |
| `osai-llamacpp-up` | Start llama.cpp |
| `osai-llamacpp-check` | Check llama.cpp |
| `osai-llamacpp-down` | Stop llama.cpp |
| `osai-vllm-env` | vLLM environment |
| `osai-vllm-up` | Start vLLM |
| `osai-vllm-check` | Check vLLM |
| `osai-vllm-down` | Stop vLLM |
| `osai-install-user-services` | Install systemd user services |