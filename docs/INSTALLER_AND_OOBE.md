# Installer and OOBE Design

## Purpose

Defines the OSAI installer and first-boot (OOBE) experience: install goals, dual-boot with Windows, first boot, user creation, hardware detection, model selection, privacy defaults, API token generation, allowed roots, initial health checks, and failure/recovery flows.

## Goals

- Install OSAI Linux on bare metal alongside an existing Windows install (dual boot).
- Provide a single-user, single-machine install (multi-user/multi-host enterprise install is out of scope for v1).
- Run a guided OOBE that produces a working assistant on first boot.
- Be safe to retry. An interrupted install must not brick the disk.
- Be auditable. Every step of the installer writes a receipt to a persistent log.

## Non-Goals

- Network/Kubernetes/Cloud install. (Future; not in v1.)
- Headless install. (Out of scope; OOBE is interactive.)
- Re-partitioning an existing Linux install. (Out of scope; only dual boot with Windows is supported for v1.)
- Enterprise domain join. (Out of scope.)

## Installer Goals

| Goal | Acceptance |
|---|---|
| User can install OSAI alongside Windows in one guided flow | Dual-boot scenario works end-to-end on a tested Lenovo/HW profile |
| User can install OSAI as the only OS on a disk | Wipe-and-install works |
| Installer never silently modifies Windows partitions | Partition preview shown, explicit confirm required |
| Installer writes an install receipt | Receipt written to ESP or `/var/log/osai-install` after success |
| Installer supports resume after power loss | Boot into installer image and resume; partial state is recoverable |
| Installer pre-reserves space for swap, OS, and `/var` | Default layout: 1 GB EFI, 16 GB OS (in BTRFS subvolume), 8 GB swap, rest to `/var` |
| Installer supports LUKS full-disk encryption | Optional, default ON for laptops, OFF for servers |

## Dual-Boot with Windows

### Detection

The installer detects existing Windows installs by:

- Scanning for NTFS partitions.
- Looking for `\EFI\Microsoft\Boot\bootmgfw.efi`.
- Reading the Windows Boot Manager entry in NVRAM.
- Detecting BitLocker by reading the BitLocker header on NTFS partitions (if present, surface a warning).

### Partition Layout

The installer MUST:

1. Show the user the proposed partition table before committing.
2. Never shrink a Windows partition without explicit user confirmation.
3. Reserve at least 30 GB for the OSAI install (more if the user wants models on disk).
4. Leave Windows Boot Manager as the default boot entry. OSAI adds itself to the UEFI boot order but does not overwrite Windows Boot Manager.
5. Use `os-prober` data to populate the GRUB menu (or systemd-boot if used) so the user can boot Windows or OSAI at power-on.

### Layout

```
Partition 1: EFI System Partition (FAT32, 1 GB, existing or new)
Partition 2: Microsoft Reserved (existing, untouched)
Partition 3: Windows (NTFS, existing, untouched)
Partition 4: OSAI /var (ext4 or BTRFS, OS-controlled)
Partition 5: OSAI / (BTRFS subvolume for rpm-ostree)
Partition 6: swap (8 GB or per-RAM)
```

If full-disk encryption is enabled, OSAI `/` and `/var` are inside a LUKS container with a user-chosen passphrase (and a generated recovery key).

### Boot Loader

- The installer uses `systemd-boot` if the system uses it natively (UEFI).
- GRUB is an alternative if `os-prober` finds Windows.
- OSAI never overwrites the Windows Boot Manager entry.
- OSAI registers itself as a UEFI boot option and as a menu entry in the existing boot manager.

## First Boot Experience

After install + first boot, the user lands on the OOBE. The OOBE is:

- A full-screen TUI (or simple GUI) that runs as the first user.
- A separate systemd target (`osai-oobe.target`) that blocks the user session from starting until complete.
- Idempotent. Closing and reopening OOBE does not corrupt state.

### OOBE Steps

```
1. Welcome + language selection
2. Hostname
3. User account (created during install; here user sets password)
4. Locale, timezone, keyboard
5. Privacy defaults
6. Allowed roots setup
7. GPU and runtime detection
8. Local model selection
9. Cloud fallback (opt-in)
10. API token generation
11. Health check
12. Done -> user session starts
```

Each step is described below.

## User Creation

- Username: 3-32 chars, lowercase, alphanumeric and dash.
- Password: 12+ chars, no character-class rules, zxcvbn score >= 3.
- The user is added to standard groups: `wheel`, `osai`, `audio`, `video`, `input`.
- The user is granted passwordless sudo for OOBE only; after OOBE, sudo requires password.

## GPU and Runtime Detection

The OOBE runs:

- `lspci` to enumerate GPUs.
- For NVIDIA: check kernel module, driver version, CUDA version, `nvidia-smi` responds.
- For AMD: check `amdgpu` kernel module, ROCm availability (informational).
- For Intel: check `i915` or `xe` kernel module.
- CPU-only fallback: explicitly noted when no GPU is detected.
- VRAM tier is computed (see `HARDWARE_PROFILES_DESIGN.md`).
- llama.cpp availability: verified by running `llama-server --version`.
- The user sees a summary: "Detected: NVIDIA RTX 4060 Laptop, 8 GB VRAM, CUDA 13.0, llama.cpp ready." Errors are surfaced with a clear next step.

## Local Model Selection and Download

The OOBE recommends a model based on hardware tier:

| Hardware | Recommendation |
|---|---|
| <6 GB VRAM or CPU-only | Gemma 4 E2B Q8 GGUF (current validated default; smoke + safe mode) |
| 8 GB VRAM | Gemma 4 E2B Q8 GGUF (default), Gemma 4 12B QAT Q4_0 GGUF (optional, reduced context) |
| 12-16 GB VRAM | Gemma 4 12B QAT Q4_0 GGUF (preferred default candidate), pending local validation |
| 16-24 GB VRAM | Gemma 4 12B QAT Q4_0 GGUF (target default) or Gemma 4 26B A4B Q4_0 GGUF |
| 80 GB+ VRAM (server) | Gemma 4 31B Q4_0 GGUF or vLLM with full precision |

Model selection is policy-driven by `HARDWARE_PROFILES_DESIGN.md`. The user can override.

The model is downloaded with:

- Source: Hugging Face (official `google/gemma-4-*-qat-q4_0-gguf` repos).
- Auth: none for public repos. Optional HF token for private mirrors.
- Verification: SHA256 checksum from the model card against the downloaded file.
- Resume: downloads resume from a partial file.
- Failure: 3 retries with exponential backoff, then a clear error to the user.
- Storage: per-user by default (`~/.local/share/osai/models/`), system-wide if opted in.

**Important**: Gemma 4 12B QAT Q4_0 GGUF is the **preferred local model candidate** for 8 GB+ VRAM systems **if local validation passes** on the target hardware. It is NOT a validated default until a real local validation run completes (see `TESTING.md` and `HARDWARE_PROFILES_DESIGN.md`).

## Privacy Defaults

The OOBE presents the user with three privacy modes (per `PRIVACY_MODEL.md`):

| Mode | Behavior | Default? |
|---|---|---|
| `local_only` | No cloud use. All inference on the device. | Yes (default) |
| `cloud_fallback` | Local preferred; falls back to cloud on error. | No (opt-in) |
| `cloud_only` | All requests go to cloud. | No (opt-in; for low-end hardware) |

If the user opts into cloud fallback or cloud only, the OOBE prompts for MiniMax API key. The key is written to `~/.config/osai/cloud-credentials.toml` with `chmod 0600`. The key is never logged, never written to receipts.

## API Token Generation

- Source: random 256-bit value, base64url encoded.
- Storage: `~/.config/osai/api-token` with `chmod 0600`.
- Display: shown once to the user with a copy-to-clipboard button.
- Use: `Authorization: Bearer <token>` or `X-OSAI-Token: <token>` on all protected `osai-api` endpoints.
- Rotation: a "rotate token" action in the Settings panel regenerates and redeploys the token.

## Allowed Roots Setup

The OOBE asks the user which directories OSAI may read and write under ToolBroker. Defaults:

- `~/Downloads` -- read/write
- `~/Documents` -- read/write
- `~/Projects` -- read/write (developer default)
- Other directories -- denied by ToolBroker

The user can change allowed roots later in Settings.

## Initial Health Checks

After model download, the OOBE runs:

1. `osai-cli doctor` -- validates `osai-api`, Model Router, and llama-server.
2. A test chat: "Reply with exactly: OSAI OOBE OK" -- the user sees the response in the OOBE window.
3. A test ask: generates a safe plan to list `~/Downloads` -- shown to the user.
4. A receipt check: at least one chat receipt and one ask receipt must exist after the smoke test.
5. Network check: if the user opted into cloud, a test MiniMax ping returns 200.

If any check fails, the OOBE shows a clear failure message and a "retry" / "skip" / "open docs" choice. Failures do not block the user from finishing OOBE; they are recorded as warnings and the user can re-run `osai-cli doctor` later.

## Failure and Recovery Flows

### Installer Failure

| Failure | Recovery |
|---|---|
| Power loss during install | Boot from USB; installer resumes the partial transaction |
| Partition table corruption | Restore the prior partition table from the pre-install backup the installer writes to ESP |
| BitLocker detected on Windows partition | Installer aborts and instructs the user to disable BitLocker before retrying |
| Insufficient disk space | Installer aborts with a clear message and a required-minimum |
| Bootloader write failure | Installer reverts boot entry changes; old bootloader remains |

### OOBE Failure

| Failure | Recovery |
|---|---|
| Model download fails | OOBE offers to retry, use the pre-staged smoke model, or skip |
| GPU detection fails | OOBE falls back to CPU-only mode and shows a warning |
| API token write fails | OOBE shows the token on screen and asks the user to write it manually |
| Health check fails | OOBE shows the failure, opens the doc, and offers to retry after the user fixes the issue |

### Runtime Failure (After OOBE)

| Failure | Recovery |
|---|---|
| `osai-api` crash | systemd restarts the service. Health check fails; CLI shows the failure with a link to logs |
| llama.cpp crash | systemd restarts. Last receipt records the failure. Model may need reload |
| OOBE interrupted | Next boot, OOBE resumes from the last completed step (state in `/var/lib/osai/oobe-state.toml`) |

## Open Questions

- Should the installer support BTRFS subvolumes with `rpm-ostree` for easier rollback of the OS image, or is a single BTRFS subvolume enough for v1?
- Should OOBE be a TUI (text-mode) or a simple GUI? (Open: TUI is more robust; GUI is friendlier. Recommend TUI for v1.)
- Should we add a "developer mode" checkbox in OOBE that grants passwordless sudo and enables dev tooling? (Open: probably yes; default OFF.)
- Should BitLocker recovery be supported, or is "disable BitLocker first" the only path? (Open: defer to "disable first" for v1.)

## Acceptance Criteria

This design is accepted when:

- A tester can install OSAI alongside a real Windows install on the validated hardware, then boot into both OSes from the UEFI menu.
- A tester can wipe-and-install OSAI as the only OS on a clean disk.
- A tester can run OOBE, download a real model, run the smoke chat, and exit into the user session.
- A tester can intentionally fail each OOBE step and the OOBE offers a clear recovery path.
- All install and OOBE steps are recorded in `/var/log/osai-install/` and `/var/lib/osai/oobe-receipts/`.
- Rollback to a fresh install is documented and works (re-run installer).
