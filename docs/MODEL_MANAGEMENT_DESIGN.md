# Model Management Design

## Purpose

Defines how OSAI Linux manages local and cloud models: the catalog, download sources, checksum verification, storage, aliases, default selection, switching, cleanup, offline mode, cloud fallback rules, and integration with the Model Router and llama.cpp.

This is a design document. It does not implement model management code.

## Goals

- Local models are first-class artifacts with versions, checksums, and provenance.
- The user can download, verify, switch, and remove models through `osai-cli` and the UI.
- Default model selection is policy-driven and adapts to hardware.
- Cloud fallback is explicit and opt-in.
- Offline mode works: once models are downloaded, the system runs without network.
- Model files are never committed to the repository.

## Non-Goals

- A general-purpose model marketplace. OSAI ships a curated catalog.
- Training or fine-tuning of local models. OSAI runs pre-trained models.
- Quantization tools. OSAI consumes pre-quantized GGUF files from the catalog.
- License compliance tooling. License info is shown in the catalog; the user is responsible for compliance.

## Local Model Catalog

The catalog is a YAML/JSON file in `/usr/share/osai/models/catalog.toml` (system) and `~/.config/osai/models/catalog.toml` (user overrides).

Each entry contains:

```yaml
- id: gemma-4-12b-it-qat-q4_0
  family: gemma-4
  size_label: 12B
  format: gguf
  quantization: qat-q4_0
  source:
    repo: google/gemma-4-12B-it-qat-q4_0-gguf
    file: gemma-4-12b-it-qat-q4_0.gguf
  sha256: <hex>
  size_bytes: 7320000000
  context_length: 256000
  recommended_context: 8192
  hardware_tier:
    min_vram_gb: 8
    recommended_vram_gb: 12
  aliases: [gemma-4-12b, gemma4:12b]
  status: candidate
  role: default_local_target
  notes: |
    Preferred local model candidate for 8GB+ VRAM systems.
    Pending local validation on target hardware.
    Gemma 4 12B QAT Q4_0 GGUF.
```

Fields:

- `id` -- canonical identifier.
- `family` -- the model family (e.g., `gemma-4`).
- `size_label` -- human-friendly size (e.g., `12B`).
- `format` -- `gguf` for local llama.cpp.
- `quantization` -- e.g., `q8_0`, `qat-q4_0`.
- `source.repo` and `source.file` -- Hugging Face path.
- `sha256` -- checksum for verification.
- `size_bytes` -- expected file size.
- `context_length` -- model's max context.
- `recommended_context` -- what to use by default to keep memory safe.
- `hardware_tier` -- VRAM requirements.
- `aliases` -- short names users can type instead of the full id.
- `status` -- `validated_default`, `known_fallback`, `candidate`, `experimental`.
- `role` -- `default_local`, `fallback`, `smoke`, `candidate`, `cloud`, etc.
- `notes` -- human-readable explanation.

## Current and Target Models

| Model | Status | Role | Notes |
|---|---|---|---|
| `gemma-4-e2b-it-q8_0` | `validated_default` | `fallback` / `smoke` | Current validated default. Keep as known-validated fallback. Q8 GGUF, ~4.7 GB. |
| `gemma-4-12b-it-qat-q4_0` | `candidate` | `default_local_target` | Preferred local model candidate for 8GB+ VRAM if local validation passes. ~6.7 GB weights + runtime overhead. |
| `gemma-4-26b-a4b-qat-q4_0` | `experimental` | `performance` | Future performance model. 14.4 GB weights minimum. |
| `gemma-4-31b-qat-q4_0` | `experimental` | `server_performance` | Future server model. |
| `qwen2.5-0.5b-instruct-q4_k_m` | `validated_default` | `smoke` | Pre-staged in base image for OOBE smoke test. |
| `MiniMax-M2.7` | n/a (cloud) | `cloud_fallback` | Cloud fallback only, policy-controlled. |

**Important**: Gemma 4 12B QAT Q4_0 GGUF is NOT promoted to `validated_default` until a real local validation run completes on the target hardware. See `TESTING.md` for validation requirements and `HARDWARE_PROFILES_DESIGN.md` for tier-specific behavior.

## Download Sources

- Primary: Hugging Face (`huggingface.co`).
- Optional mirror: a user-configured HTTPS endpoint (for air-gapped or bandwidth-restricted environments).
- Authentication: anonymous for public repos; optional `HF_TOKEN` for gated repos.

`osai-cli model pull <id>` uses the source defined in the catalog. The CLI:

1. Resolves the canonical id (or alias).
2. Looks up the source URL and expected checksum.
3. Downloads to `~/.local/share/osai/models/<id>/<file>` (or `/var/cache/osai/models/<id>/<file>` if system-wide).
4. Verifies the SHA256 against the catalog.
5. Records the install in `~/.local/share/osai/models/installed.toml` (or system equivalent).

## Checksum Verification

After download:

1. The CLI computes SHA256 of the downloaded file.
2. The computed hash is compared to the catalog hash with constant-time comparison.
3. If mismatch: the file is deleted, an error is shown, and the user is asked to retry or report.
4. If the catalog hash is missing or zero, the CLI warns and asks for confirmation before proceeding (only for trusted sources).

## Model Storage

### Per-User (Default)

- Path: `~/.local/share/osai/models/<id>/`
- File: `<source.file>` (e.g., `gemma-4-12b-it-qat-q4_0.gguf`)
- Metadata: `installed.toml` with install date, source, checksum, status

### System-Wide (Opt-in)

- Path: `/var/cache/osai/models/<id>/`
- Same structure as per-user.
- Visible to all users on the host.
- Only `wheel` and `osai` group members can install system-wide.

### Out of Repo

- Model files are NEVER in the repository.
- `.gitignore` must include `.local-models/`, `~/.local/share/osai/models/`, `/var/cache/osai/models/`.
- The `osai-cli` scripts that mention model paths use these external paths, not repo-relative paths.

## Model Aliases

Aliases are short names that resolve to a canonical id. The default alias mapping:

| Alias | Canonical id | Notes |
|---|---|---|
| `default` | depends on hardware tier (see `HARDWARE_PROFILES_DESIGN.md`) | The current default local model |
| `fallback` | `gemma-4-e2b-it-q8_0` | Known-validated fallback |
| `smoke` | `qwen2.5-0.5b-instruct-q4_k_m` | Smoke test model |
| `gemma4:e2b` | `gemma-4-e2b-it-q8_0` | |
| `gemma4:12b` | `gemma-4-12b-it-qat-q4_0` | Pending local validation |
| `gemma4:26b` | `gemma-4-26b-a4b-qat-q4_0` | Experimental |
| `osai-auto` | (resolved by privacy setting) | Model Router alias |
| `osai-local` | (force local) | Model Router alias |
| `osai-cloud` | (force cloud) | Model Router alias |
| `MiniMax-M2.7` | n/a | Cloud model |

Users can add custom aliases in `~/.config/osai/models/aliases.toml`. Custom aliases override built-in aliases.

## Default Model Selection

Default model is determined by:

1. **Hardware tier** (from `HARDWARE_PROFILES_DESIGN.md`):
   - <6 GB VRAM or CPU-only: `gemma-4-e2b-it-q8_0` (validated fallback, no 12B)
   - 6-8 GB VRAM: `gemma-4-e2b-it-q8_0` (validated default; 12B only with reduced context and explicit opt-in)
   - 8-12 GB VRAM: `gemma-4-12b-it-qat-q4_0` **IF** local validation passed; otherwise `gemma-4-e2b-it-q8_0`
   - 12-16 GB VRAM: `gemma-4-12b-it-qat-q4_0` (target default, pending validation)
   - 16-24 GB VRAM: `gemma-4-12b-it-qat-q4_0` (recommended) or `gemma-4-26b-a4b-qat-q4_0`
   - 24 GB+ VRAM: `gemma-4-26b-a4b-qat-q4_0` or `gemma-4-31b-qat-q4_0`
   - 80 GB+ VRAM (server): `gemma-4-31b-qat-q4_0` or full precision via vLLM
2. **User override**: any installed model can be made default.
3. **Cloud fallback**: depends on privacy mode and credentials.

The `default` alias is computed at OOBE and stored in `~/.config/osai/settings.toml`. Users can change it later.

## Switching Models

`osai-cli model use <alias-or-id>`:

1. Validates the model is installed.
2. Stops `osai-llama-server.service` (systemd).
3. Updates `/etc/osai/llama-server.env` to point at the new model.
4. Starts `osai-llama-server.service`.
5. Waits for `/v1/models` to respond with the new model.
6. Records the switch in a receipt.

A model switch is a 5-15 second operation. The user is notified in the tray icon during the switch.

## Cleanup / Removal

`osai-cli model remove <alias-or-id>`:

1. Confirms the user wants to remove (irreversible).
2. Stops `osai-llama-server.service` if it is the active model.
3. Deletes the model file and metadata.
4. Falls back to the next-best installed model.
5. Starts the fallback model.

## Offline Mode

Once models are downloaded, OSAI runs offline. The system MUST function without network access:

- `osai-llama-server.service` binds loopback. No network needed for inference.
- `osai-api` and `osai-model-router` bind loopback. No network needed.
- Cloud fallback is disabled automatically when no network is available; an error is logged in the receipt and a notification is shown.
- The update check is a periodic timer; if no network, it skips silently.

## Cloud Fallback Rules

Cloud use is governed by `PRIVACY_MODEL.md` and the `osai-cli cloud` config:

- `privacy: local_only` (default): no cloud use. Errors are returned, not silently retried.
- `privacy: cloud_fallback`: try local first; on error (model missing, GPU OOM, timeout > 30s), fall back to cloud. Each cloud use is logged in a receipt.
- `privacy: cloud_only`: every request goes to cloud. Used for low-end hardware or when local runtime is intentionally disabled.

Cloud credentials:

- Stored in `~/.config/osai/cloud-credentials.toml` with `chmod 0600`.
- Never logged, never written to receipts.
- Rotation: user can rotate in Settings; old key is overwritten.

## Integration with Model Router and llama.cpp

### llama.cpp

- The Model Router is configured with `OSAI_LLAMACPP_BASE_URL=http://127.0.0.1:8092/v1`.
- `osai-llama-server.service` is the source.
- The CLI's `osai-cli model use` updates the unit's environment file and restarts the service.
- The Model Router picks up the new model on the next request.

### Model Router

- The Model Router is the single point of contact for `osai-api` and `osai-cli`.
- It knows about local providers (llama.cpp, vLLM) and cloud providers (MiniMax).
- The privacy setting and model alias determine the routing.
- The Model Router's health endpoint is at `GET /health` and `GET /v1/models`.

### Settings Sync

- When the user changes the default model in Settings, the same change is written to:
  - `~/.config/osai/settings.toml` (for `osai-cli` reads)
  - The Model Router's runtime config (if configurable)
  - `/etc/osai/llama-server.env` (if the active model is local)
- The system reloads services as needed.

## Open Questions

- Should the catalog support GGUF variants other than Q4_0 / Q8_0 (e.g., Q4_K_M, Q5_K_M)? (Open: yes; allow per-model field; default to the catalog's recommendation.)
- Should the user be able to install multiple versions of the same model? (Open: yes; the canonical id includes a version suffix.)
- Should the CLI support `osai-cli model benchmark` to time inference? (Open: nice to have; defer if scope grows.)
- Should the catalog be signed? (Open: yes; sign with the OSAI release key; verify on load.)

## Acceptance Criteria

This design is accepted when:

- A coder can implement the catalog format, download flow, and verification.
- A tester can `osai-cli model pull gemma4:12b` and see the file downloaded and verified.
- A tester can `osai-cli model use gemma4:12b` and watch the system switch with no error.
- A tester can unplug the network and continue using OSAI with a downloaded model.
- A tester cannot accidentally commit a model file (`.gitignore` blocks it).
- A reviewer confirms the default model selection policy matches the hardware tier.
- A reviewer confirms cloud fallback is opt-in and clearly logged.
