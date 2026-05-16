# Model Runtime

## Overview

OSAI supports multiple model runtimes:
- **llama.cpp** (default local) — GGUF quantized models, CPU/CUDA
- **vLLM** (optional local) — HuggingFace models, CUDA, higher throughput
- **MiniMax** (cloud fallback) — API-based cloud inference

The Model Router sits in front of all providers, providing a unified OpenAI-compatible API.

## llama.cpp (Default)

llama.cpp is the default local runtime for OSAI.

### Why llama.cpp?

- **GGUF format** — Efficient quantized models, no compilation needed
- **CUDA support** — GPU acceleration on NVIDIA GPUs
- **Laptop-friendly** — Works on RTX 4060 Laptop 8GB VRAM
- **No custom backend** — llama-server is a simple HTTP server
- **Privacy** — All inference on user's machine

### Current Configuration

```
llama-server: http://127.0.0.1:8092/v1
Model: gemma-4-E2B-it-Q8_0.gguf
Context: 4096
Threads: auto (CPU + GPU)
```

### Gemma 4 E2B Q8 GGUF

- **Size**: 4.7 GB GGUF file
- **Quantization**: Q8_0 (8-bit)
- **Context**: 4096 tokens
- **Location**: `.local-models/llamacpp/gemma-4-e2b-it/gemma-4-E2B-it-Q8_0.gguf`
- **Role**: Default local model for OSAI

### Build Requirements

```bash
# CUDA-enabled build
cmake -B build -G Ninja \
  -DGGML_CUDA=ON \
  -DCMAKE_CUDA_ARCHITECTURES=89 \
  -DCMAKE_BUILD_TYPE=Release

cmake --build build -j"$(nproc)"
```

### llama.cpp Scripts

```bash
./scripts/osai-llamacpp-up     # Start llama-server
./scripts/osai-llamacpp-check  # Validate llama-server
./scripts/osai-llamacpp-down   # Stop llama-server
./scripts/osai-llamacpp-env    # Set environment variables
```

## vLLM (Optional)

vLLM is an optional local runtime for higher throughput.

### Why vLLM?

- **PagedAttention** — Better memory management
- **Higher throughput** — Better for batch processing
- **HuggingFace models** — Wide model compatibility

### Why Not vLLM Currently?

- **VRAM requirements** — Gemma 4 E2B via vLLM failed on RTX 4060 Laptop 8GB (CUDA OOM)
- **llama.cpp is sufficient** — For single-user laptop use case
- **Complexity** — vLLM requires CUDA toolkit + specific driver setup

### When to Use vLLM

- Multi-user deployment
- Server with ample VRAM (24GB+)
- Batch processing workloads
- When Gemma 4 26B or larger models needed

### vLLM Scripts

```bash
./scripts/osai-vllm-up     # Start vLLM server
./scripts/osai-vllm-check  # Validate vLLM
./scripts/osai-vllm-down   # Stop vLLM server
./scripts/osai-vllm-env   # Set environment variables
```

## MiniMax (Cloud Fallback)

MiniMax is the approved cloud provider for OSAI.

### Configuration

```bash
MINIMAX_API_KEY=<key>  # In environment or token file
MINIMAX_BASE_URL=https://api.minimax.chat
MODEL=MiniMax-M2.7
```

### When to Use

- Local GPU unavailable
- Very large models (Gemma 4 26B+)
- Batch processing requiring more VRAM than available
- When explicitly requested (`privacy: cloud_fallback` or `cloud_only`)

## Model Router

The Model Router (`services/model-router/`) provides a unified API:

```bash
POST http://127.0.0.1:8088/v1/chat/completions
```

Supported providers:
- `llamacpp` — Local GGUF models
- `vllm` — Local HuggingFace models
- `minimax` — Cloud API

Model aliases:
- `osai-auto` — Select provider based on privacy setting
- `osai-local` — Force local (llama.cpp)
- `osai-cloud` — Force cloud (MiniMax)
- `gemma4:e2b` — Gemma 4 E2B
- `gemma4:e4b` — Gemma 4 E4B
- `gemma4:26b` — Gemma 4 26B
- `MiniMax-M2.7` — MiniMax cloud model

## Local Model Paths (Do Not Commit)

Local model files are stored outside the repository:

```
.local-models/
├── llamacpp/
│   ├── gemma-4-e2b-it/
│   │   └── gemma-4-E2B-it-Q8_0.gguf
│   └── qwen2.5-0.5b-instruct/
│       └── qwen2.5-0.5b-instruct-q4_k_m.gguf
└── (vLLM models if added)
```

**Never commit these files. Never include them in Git.**

## Model Selection Logic

```
Request metadata.privacy:
  ├── "local_only" → llama.cpp (or vLLM if configured)
  ├── "cloud_fallback" → llama.cpp, fallback to MiniMax on error
  └── "cloud_only" → MiniMax

Request model:
  ├── "osai-auto" → Provider determined by privacy
  ├── "gemma4:e2b" → Gemma 4 E2B via local provider
  ├── "MiniMax-M2.7" → MiniMax cloud
  └── other → Provider configured for that alias
```

## Future Model Needs

### Computer Use Models

Computer use will require additional model capabilities:

**Screenshot Understanding**:
- Local vision model (Qwen2.5-VL, LLaVA, etc.)
- Or cloud vision with explicit opt-in
- Low latency required for real-time interaction

**OCR/Text Extraction**:
- Extract text from screenshots
- Local OCR (Tesseract) or model-based
- Privacy-preserving (local preferred)

**Action Planning**:
- Interpret screenshots and plan actions
- May use the same model as chat
- Gemma 4 or specialized model

### Multimodal Considerations

- **Gemma 4 multimodal** — Experimental, not yet integrated
- **Local vision** — Privacy-preserving, no cloud dependency
- **Cloud vision** — Explicit opt-in, higher quality
- **Latency constraints** — Real-time interaction needs fast local inference

### Hardware Profiles

Future hardware profiles will optimize model selection:

| Profile | VRAM | Recommended Models |
|---------|------|-------------------|
| Laptop | 8GB | Gemma 4 E2B Q8 |
| Desktop | 16-24GB | Gemma 4 E4B Q8, Gemma 4 26B Q8 |
| Server | 80GB+ | vLLM + large models |

Current: Laptop profile is the primary target.

## Model Management (Future)

Future features needed:
- Download models from HuggingFace
- Verify model checksums
- Select default model per profile
- Switch models without restart
- Automatic model download for new installs