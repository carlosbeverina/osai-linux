# Hardware Profiles Design

## Purpose

Defines how OSAI Linux detects hardware, classifies it into a profile (laptop, desktop, server, CPU-only), maps the profile to a VRAM tier and a recommended model, sets context length and quantization policy, applies thermal/power policy, and produces a validation matrix.

This is a design document. It does not implement the detection code.

## Goals

- A single `osai-cli doctor --hardware` command produces a hardware profile report.
- Hardware profile is stored in `~/.config/osai/hardware.toml` and used by the model selector, OOBE, and Settings.
- The system can switch profiles (e.g., a laptop docking at a desk may move from "Laptop Battery" to "Laptop Plugged").
- Model selection is policy-driven by the profile, not by hardcoded heuristics.

## Non-Goals

- A general system profiler. OSAI only inspects what affects the assistant.
- Vendor-specific tuning. The profile is generic; vendor extensions live in `~/.config/osai/hardware.d/`.
- Real-time monitoring. Profile is computed on demand; updates are pull-based, not pushed.

## Hardware Detection

The detection runs `osai-cli hardware detect` and writes `~/.config/osai/hardware.toml`. It inspects:

- `lscpu` -- CPU model, cores, threads, AVX/AVX2/AVX-512 support.
- `lspci` -- GPU vendor, model, device ID.
- `nvidia-smi` -- VRAM, driver version, CUDA version, GPU utilization.
- `rocm-smi` -- AMD ROCm info (informational; not required).
- `/proc/meminfo` -- total RAM.
- `/sys/class/dmi/id/` -- chassis type (laptop, desktop, server).
- `lsblk` -- disk type (SSD vs HDD) and free space.
- `/sys/class/power_supply/` -- battery presence, capacity.
- `systemd-detect-virt` -- whether the system is a VM.
- `tpm2_getcap` -- whether TPM is available (for LUKS, attestation).

The detection tool is read-only. It does not install drivers or change settings.

### Output Schema (`hardware.toml`)

```toml
[system]
chassis = "laptop"  # laptop | desktop | server | vm | unknown
hostname = "victus"
virtualized = false
tpm_available = true

[cpu]
model = "AMD Ryzen 7 8845HS"
cores = 8
threads = 16
avx2 = true
avx512 = false
ram_gb = 32

[gpu]
present = true
vendor = "nvidia"  # nvidia | amd | intel | none | unknown
model = "RTX 4060 Laptop"
vram_gb = 8
driver_version = "580.126.09"
cuda_version = "13.0"
rocm_version = ""

[storage]
disks = [
  { name = "nvme0n1", type = "ssd", size_gb = 1024, free_gb = 600 }
]

[power]
battery_present = true
ac_connected = true
thermal_state = "nominal"  # nominal | warm | throttled
```

## Profile Tiers

The profile is derived from the detection results:

| Profile | Chassis | GPU | VRAM | CPU |
|---|---|---|---|---|
| `laptop-battery` | laptop | any | any | any |
| `laptop-plugged` | laptop | any | any | any |
| `desktop` | desktop | any | any | any |
| `server` | server | any | any | any |
| `vm-cpu` | vm | none | 0 | any |
| `vm-gpu` | vm | passthrough | varies | any |

`osai-cli hardware profile` reduces the detection to a single profile string and tier:

```toml
[profile]
name = "laptop-plugged"
vram_tier = "8gb"  # see VRAM Tiers below
recommended_model = "gemma-4-12b-it-qat-q4_0"
recommended_context = 4096
quantization = "qat-q4_0"
thermal_policy = "balanced"
```

## VRAM Tiers

The detected VRAM determines the tier:

| Tier | VRAM (GB) | Notes |
|---|---|---|
| `cpu-only` | 0 | No GPU or VRAM reporting fails |
| `tiny` | <4 | Rare; treat as cpu-only |
| `small` | 4-6 | E2B-class models only |
| `medium` | 6-8 | E2B default; 12B with reduced context |
| `large` | 8-12 | 12B Q4_0 if validated; 26B A4B with reduced context |
| `xlarge` | 12-16 | 12B Q4_0 target; 26B A4B Q4_0 |
| `xxlarge` | 16-24 | 12B Q4_0 or 26B A4B Q4_0 |
| `huge` | 24-80 | 26B A4B or 31B |
| `extreme` | 80+ | 31B, full precision, multi-model serving |

## Recommended Model by Profile

| Profile | Default model | Fallback |
|---|---|---|
| `laptop-battery` (any VRAM) | `gemma-4-e2b-it-q8_0` | `qwen2.5-0.5b-instruct-q4_k_m` |
| `laptop-plugged` (cpu-only) | `gemma-4-e2b-it-q8_0` | `qwen2.5-0.5b-instruct-q4_k_m` |
| `laptop-plugged` (4-6 GB) | `gemma-4-e2b-it-q8_0` | `qwen2.5-0.5b-instruct-q4_k_m` |
| `laptop-plugged` (6-8 GB) | `gemma-4-e2b-it-q8_0` (validated); 12B with reduced context is opt-in only | `qwen2.5-0.5b-instruct-q4_k_m` |
| `laptop-plugged` (8-12 GB) | `gemma-4-12b-it-qat-q4_0` **if local validation passed**; else `gemma-4-e2b-it-q8_0` | `gemma-4-e2b-it-q8_0` |
| `laptop-plugged` (12-16 GB) | `gemma-4-12b-it-qat-q4_0` (target default) | `gemma-4-e2b-it-q8_0` |
| `desktop` (16-24 GB) | `gemma-4-12b-it-qat-q4_0` (recommended) or `gemma-4-26b-a4b-qat-q4_0` | `gemma-4-e2b-it-q8_0` |
| `desktop` (24-80 GB) | `gemma-4-26b-a4b-qat-q4_0` | `gemma-4-12b-it-qat-q4_0` |
| `server` (80+ GB) | `gemma-4-31b-qat-q4_0` or vLLM with full precision | any installed |
| `vm-cpu` (any) | `gemma-4-e2b-it-q8_0` | `qwen2.5-0.5b-instruct-q4_k_m` |
| `vm-gpu` (varies) | follows VRAM tier rules | follows VRAM tier rules |

**Important**: "Validated default" requires a real local validation run. The OSAI team has validated `gemma-4-e2b-it-q8_0` on RTX 4060 Laptop 8GB VRAM. Gemma 4 12B QAT Q4_0 is a **target default candidate** for 8GB+ VRAM systems. The promotion from "candidate" to "validated default" requires the validation procedure in `TESTING.md` to pass on the target hardware. Until then, the system ships with `gemma-4-e2b-it-q8_0` as the default.

## Context Length Policy

The default context length is chosen to keep memory safe:

| Model | Default context | Max safe context | Notes |
|---|---|---|---|
| `gemma-4-e2b-it-q8_0` | 4096 | 131072 | 4K is enough for chat/ask; raise to 32K for long-context tests |
| `gemma-4-12b-it-qat-q4_0` | 4096 | 262144 | 4K is the safe default on 8GB VRAM |
| `gemma-4-26b-a4b-qat-q4_0` | 8192 | 262144 | 8K is the safe default on 16-24GB |
| `gemma-4-31b-qat-q4_0` | 8192 | 262144 | 8K is the safe default on 24-80GB |
| `qwen2.5-0.5b-instruct-q4_k_m` | 2048 | 32768 | Smoke test only |

Users can raise the context in Settings, but the CLI warns when the requested context would exceed available memory. The system records the requested context and observed tokens/sec in the receipt for diagnostics.

## Quantization Policy

OSAI consumes pre-quantized GGUF files from the catalog. We do not run the quantizer at install time.

| Tier | Default quantization | Notes |
|---|---|---|
| `cpu-only` to `medium` | `q8_0` (E2B) or `qat-q4_0` (12B) | Quality over speed |
| `large` to `xxlarge` | `qat-q4_0` (12B) or `qat-q4_0` (26B) | Balanced |
| `huge`+ | `qat-q4_0` (26B, 31B) or full precision via vLLM | Throughput |

If a user wants a different quantization, they can download an alternative GGUF and add it to the catalog via user overrides.

## Thermal and Power Policy

### Laptop Battery

- Default model: smaller, lower-power (E2B Q8).
- Context: conservative (4K).
- llama.cpp: `-ngl 99` (full GPU offload) but `--threads` reduced to share GPU with other apps.
- Notifications: muted to save battery.
- Receipts: summarized, full detail in `/var/log/osai/`.

### Laptop Plugged

- Default model: medium or large (12B if validated).
- Context: standard.
- llama.cpp: full GPU offload, default threads.
- Notifications: standard.

### Desktop / Server

- Default model: largest reasonable.
- Context: standard or extended.
- llama.cpp: full GPU offload, max threads.
- Notifications: standard.

### Thermal Throttling

The detection tool checks `/sys/class/thermal/thermal_zone*/temp` (or `nvidia-smi --query-gpu=clocks.current.sm`). If temperature is above 90C or throttling is reported:

- The system records a `thermal_state: throttled` in `hardware.toml`.
- The model selector downgrades one tier (e.g., from `large` to `medium`) until the temperature drops.
- The user sees a notification: "OSAI downgraded model due to thermal limits".

## Context Length Memory Math

The Google Gemma 4 documentation states the memory table does not include runtime overhead or KV cache. For a model with `W` GB weights, total VRAM use is approximately:

```
total_vram = W + (context_length * hidden_dim * num_layers * 2) / 1e9
```

For Gemma 4 12B at Q4_0:

- W = 6.7 GB
- hidden_dim ~ 3840, num_layers ~ 48
- context_length 4096 -> additional ~1.5 GB for KV cache

So 4096 context fits in ~8.2 GB. 8192 context is ~9.7 GB. 16384 context is ~12.7 GB.

For 8GB VRAM (e.g., RTX 4060 Laptop), the safe default is 4096. For 12GB+ VRAM, 8192 is reasonable. The CLI warns when the user requests a context that would exceed available VRAM.

## Validation Matrix

The following matrix must pass on the validated hardware before a model is promoted to `validated_default`:

| Test | Required for validation |
|---|---|
| `osai-cli chat "Reply with exactly: OK"` returns "OK" | yes |
| `osai-cli ask --print-plan <request>` produces a valid plan | yes |
| `osai-cli plan validate` succeeds | yes |
| `osai-cli apply --dry-run` succeeds | yes |
| `osai-cli apply` real succeeds for `FilesList` | yes |
| Token throughput (tokens/sec) above minimum | yes |
| GPU memory usage below 90% of available | yes |
| No OOM during a 10-minute soak | yes |
| Receipts contain no secrets | yes |
| Restarting `osai-llama-server` recovers within 30s | yes |
| Network disconnected, system continues to work | yes |
| Privacy indicator reflects current mode | yes |
| Switching model does not lose state | yes |

The exact thresholds and reproducibility procedure are in `TESTING.md`.

## Open Questions

- Should the profile be auto-updated (e.g., when AC is unplugged)? (Open: yes; via a small systemd timer or udev rule.)
- Should we expose a `osai-cli hardware export` command for support? (Open: yes; it redacts serial numbers and exports `hardware.toml`.)
- Should the profile support dual-GPU laptops (integrated + discrete)? (Open: yes, in v2; for v1 we use the discrete GPU when present.)

## Acceptance Criteria

This design is accepted when:

- A coder can implement the detection tool that produces a `hardware.toml`.
- A tester can run `osai-cli hardware detect` and `osai-cli hardware profile` and get a sensible profile.
- A tester can verify that the recommended model matches the tier in the table above.
- A tester can unplug AC on a laptop and watch the profile change.
- A reviewer confirms the validation matrix is sufficient to promote a model to `validated_default`.
