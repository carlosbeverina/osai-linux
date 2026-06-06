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
- `gemma4:12b` — Gemma 4 12B QAT Q4_0 GGUF (target default candidate, pending local validation)
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

### Hardware Profiles

See [HARDWARE_PROFILES_DESIGN.md](HARDWARE_PROFILES_DESIGN.md) for hardware detection, VRAM tiers, profile derivation, and the authoritative recommended-model table. The short version:

- CPU-only / <6 GB VRAM: Gemma 4 E2B Q8 GGUF
- 6-8 GB VRAM: Gemma 4 E2B Q8 GGUF (validated default); Gemma 4 12B QAT Q4_0 with reduced context is opt-in only
- 8-16 GB VRAM: Gemma 4 12B QAT Q4_0 GGUF (target default candidate, pending local validation)
- 16-24 GB VRAM: Gemma 4 12B QAT Q4_0 GGUF or Gemma 4 26B A4B QAT Q4_0 GGUF
- 24-80 GB VRAM: Gemma 4 26B A4B QAT Q4_0 GGUF
- 80+ GB VRAM (server): Gemma 4 31B QAT Q4_0 GGUF or vLLM with full precision

Validated default: Gemma 4 E2B Q8 GGUF (RTX 4060 Laptop 8GB VRAM). Gemma 4 12B QAT Q4_0 GGUF is a target default candidate for 8GB+ VRAM, not yet locally validated.

## Gemma 4 12B QAT Q4_0 GGUF Evaluation

Google's Gemma 4 family includes E2B, E4B, 12B, 26B A4B, and 31B. The QAT Q4_0 GGUF format is the recommended local llama.cpp / LM Studio format via the suffix `{model-name}-qat-q4_0-gguf`. Official QAT GGUF checkpoints are available for E2B, E4B, 12B, 26B A4B, and 31B.

### Memory Requirements (per Google's Gemma 4 inference memory table)

The table does not include extra VRAM for runtime overhead or KV cache / context. Larger context windows increase memory usage.

| Model | BF16 | Q8_0 | Q4_0 (QAT) |
|---|---|---|---|
| Gemma 4 E2B | ~5 GB | ~3 GB | ~2 GB |
| Gemma 4 E4B | ~8 GB | ~5 GB | ~3 GB |
| **Gemma 4 12B** | 26.7 GB | 13.4 GB | **6.7 GB** |
| Gemma 4 26B A4B | 57.7 GB | 28.8 GB | 14.4 GB |
| Gemma 4 31B | n/a | n/a | ~16 GB |

Source: <https://ai.google.dev/gemma/docs/core>

### File Convention

- File: `gemma-4-12b-it-qat-q4_0.gguf` (Hugging Face repo `google/gemma-4-12B-it-qat-q4_0-gguf`).
- Approximate file size: 6.98 GB (per Hugging Face model card; the inference memory table lists 6.7 GB for the model weights).

### Hardware Profile Implications

| VRAM Tier | Recommended | Context | Notes |
|---|---|---|---|
| <6 GB / CPU-only | E2B Q8 | 4K | Gemma 4 12B not feasible. |
| 6-8 GB | E2B Q8 (validated default); 12B with reduced context is opt-in only | 4K | 12B Q4_0 with 4K context fits in ~8.2 GB; 8K is ~9.7 GB. |
| 8-12 GB | **Gemma 4 12B QAT Q4_0 (target default candidate)** | 4K-8K | Pending local validation. |
| 12-16 GB | **Gemma 4 12B QAT Q4_0 (target default)** | 8K | Recommended default target. |
| 16-24 GB | 12B Q4_0 or 26B A4B Q4_0 | 8K-16K | Both viable. |
| 24+ GB | 26B A4B Q4_0 | 16K+ | Performance tier. |
| 80+ GB (server) | 31B Q4_0 or vLLM with full precision | 16K-32K | Server tier. |

### Validation Requirements

Gemma 4 12B QAT Q4_0 GGUF is a **target default candidate**, not a validated default. It is treated as such until:

1. A real local validation run completes on the target hardware (see `TESTING.md`).
2. The validation matrix in `HARDWARE_PROFILES_DESIGN.md` is satisfied.
3. A security review is performed (see `SECURITY_REVIEW_CHECKLIST_DESKTOP_AND_DISTRO.md`).
4. The promotion is recorded in the catalog with `status: validated_default`.

Until then, **Gemma 4 E2B Q8 GGUF remains the default**. The system ships with E2B as the safe-mode default. The 12B model is the target default for systems that pass local validation.

### Aliases

| Alias | Model | Status |
|---|---|---|
| `gemma4:e2b` | Gemma 4 E2B Q8 GGUF | validated_default (fallback / safe mode) |
| `gemma4:12b` | Gemma 4 12B QAT Q4_0 GGUF | candidate (pending local validation) |
| `gemma4:26b` | Gemma 4 26B A4B QAT Q4_0 GGUF | experimental |
| `gemma4:31b` | Gemma 4 31B QAT Q4_0 GGUF | experimental |
| `default` | resolved by hardware profile | depends on validation |
| `fallback` | Gemma 4 E2B Q8 GGUF | always available, never demoted |

## Model Management (Future)

Future features needed:
- Download models from HuggingFace
- Verify model checksums
- Select default model per profile
- Switch models without restart
- Automatic model download for new installs