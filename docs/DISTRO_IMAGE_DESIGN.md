# Distro Image Design

## Purpose

This document defines the OSAI Linux image: the base platform, layering model, immutable layout, writable user data, rollback assumptions, and which components belong in the base image vs first-run setup.

This is a design document. It does not implement the image, the installer, or the build pipeline.

## Goals

- Ship an installable Linux distribution where OSAI is a first-class system service, not an application.
- Use an image-based, atomic base so OS updates and OSAI updates are transactional and reversible.
- Preserve the existing OSAI architecture boundaries (UI -> osai-api -> osai-agent-core -> ToolBroker/ToolExecutor/ReceiptLogger).
- Keep all local services loopback-only by default.
- Support rollback without data loss.

## Non-Goals (For This Doc)

- Specific BlueBuild recipe syntax. Only the layering model is defined here.
- Installer flow. See `INSTALLER_AND_OOBE.md`.
- Desktop integration details. See `DESKTOP_SHELL_INTEGRATION_DESIGN.md` and `OSAI_DESKTOP_UI_SPEC.md`.
- Model management details. See `MODEL_MANAGEMENT_DESIGN.md`.

## Base Platform Decision

### Candidates Considered

| Option | Strengths | Weaknesses |
|---|---|---|
| Fedora Atomic | Official Fedora immutable base. Strong SELinux and cgroups story. Fedora ecosystem. | Upstream move-fast model. No first-party image customization pipeline. |
| Universal Blue (uBlue) | Re-base on Fedora Atomic. Mature image customization (BlueBuild). Auto-update tooling. Community images. | Depends on uBlue. Some layers are opinionated. |
| BlueBuild (tooling) | Declarative image recipes (`recipe.yml`). Composable. Built for derivative images. Good fit for OSAI. | Tooling-only. Still needs a base (typically uBlue). |

### Decision

OSAI Linux will ship as a **BlueBuild recipe** on top of a **Universal Blue base** (which itself sits on **Fedora Atomic**). This gives:

- An immutable, transactional Fedora base.
- A declarative OSAI image recipe in this repository.
- A path to auto-updates and clean rollbacks.

Fedora Atomic provides:

- `rpm-ostree` transactional updates.
- `bootupd` for firmware/bootloader.
- SELinux enforcing.
- cgroups v2 by default.
- Wayland as the default session.
- PipeWire for audio.

OSAI's value is added as **layers** on top of that base.

## Image Layering Model

OSAI image is built bottom-up as a stack of signed, immutable layers:

```
+-------------------------------------------------------+
| OSAI user layer (osai, osai-cli)                      |   <- OSAI binaries
+-------------------------------------------------------+
| OSAI runtime layer (llama.cpp, smoke model)           |   <- pre-staged smoke model
+-------------------------------------------------------+
| OSAI desktop layer (future)                           |   <- desktop integration
+-------------------------------------------------------+
| Universal Blue base (cosmic / bazzite / gnome / etc.) |   <- chosen uBlue image
+-------------------------------------------------------+
| Fedora Atomic base                                    |   <- rpm-ostree
+-------------------------------------------------------+
| Firmware + bootloader (bootupd)                       |
+-------------------------------------------------------+
```

Each layer is reproducible from a recipe. Rebuilding any layer regenerates the image above it. Layers are signed. The user cannot accidentally edit them.

## Base Image Packages

The base image MUST include:

- Linux kernel (from uBlue base)
- systemd
- SELinux (enforcing)
- cgroups v2 (default)
- Wayland (default session)
- PipeWire (audio)
- NetworkManager (or uBlue default)
- `rpm-ostree`
- `bootupd`
- `podman` (for sandboxing computer-use later)
- `sqlite` (for sessions/receipts)
- A text editor (vim, nano)
- A browser (Firefox, or uBlue default)
- `curl`, `git`, `python3` (for OOBE and helpers)

The base image MUST NOT include:

- Tokens, API keys, or cloud credentials
- `.local-models/` contents
- `.local-runtimes/` contents
- User-specific data

The base image SHOULD include:

- `osai-cli` binary (or thin wrapper)
- `osai-agent-core` (compiled, installed)
- A pre-staged **smoke-test model** (Qwen 2.5 0.5B Q4_K_M, ~0.4 GB) so first boot can run a sanity check before downloading the larger model
- Systemd user services for `osai-api` and `osai-model-router` (see `SYSTEM_SERVICES_DESIGN.md`)

## Immutable OS Layout

The base image is read-only. The user cannot modify `/usr`, `/etc` (read-only overlay), or `/opt` directly.

Mutable paths (user data) live in standard locations:

| Path | Owner | Persistence | Notes |
|---|---|---|---|
| `/etc` | root | Overlay | Configuration overlay; resets on rollback if not pinned. |
| `/var` | root | Persistent | Service state, logs, caches. |
| `/var/lib/osai` | osai | Persistent | Receipts DB, sessions, settings. |
| `/var/cache/osai` | osai | Persistent | Downloaded model cache. |
| `/var/log/osai` | osai | Logrotate | Service logs. |
| `~/.local/share/osai` | user | Persistent | Per-user receipts, sessions. |
| `~/.config/osai` | user | Persistent | Per-user settings, API token. |
| `~/.local/share/osai/models` | user | Persistent | Per-user downloaded models. |
| `~/.local/share/osai/runtimes` | user | Persistent | Per-user downloaded runtimes. |

## Writable User Data

The first user created by the installer (see `INSTALLER_AND_OOBE.md`) becomes the **OSAI user**. Their home directory holds:

- `~/.config/osai/api-token` -- auto-generated, `chmod 0600`
- `~/.config/osai/settings.toml` -- per-user settings
- `~/.local/share/osai/receipts/{chat,ask,apply,model-router}` -- receipts
- `~/.local/share/osai/sessions/` -- chat sessions
- `~/.local/share/osai/models/` -- downloaded models (alternative to `/var/cache/osai`)
- `~/.local/share/osai/runtimes/` -- downloaded runtimes (alternative to `/var/cache/osai`)

The installer MUST explain this layout. The user MUST be able to opt into storing models/runtimes in `/var/cache/osai` (system-wide) or `~/.local/share/osai/...` (per-user). Per-user is the default to keep `/` clean.

## Rollback Assumptions

`rpm-ostree` provides A/B boot slots. An OSAI update either:

- Succeeds atomically and the new deployment becomes the default boot.
- Fails before completion and the old deployment remains default.

Rollback is the user's choice: at boot, the user can select the previous deployment from the bootloader. Once booted into the new deployment, `rpm-ostree rollback` reverts to the previous default.

OSAI state that must survive rollback:

- `/var/lib/osai/**` -- receipts, sessions, settings
- `/var/cache/osai/**` -- model and runtime cache
- User homes -- `~/.local/share/osai/**`, `~/.config/osai/**`
- `/var/log/osai/**` -- logs (kept for audit)

OSAI state that must NOT survive rollback:

- None. Rollback is a state-preserving operation.

OSAI state that must be version-migrated on rollback (forward and back):

- Receipts format versions (see `RECEIPTS.md` and `UPDATE_ROLLBACK_DESIGN.md`)
- Settings schema versions

## SELinux, cgroups, systemd, Wayland, PipeWire Assumptions

### SELinux

- Enforcing in production image.
- OSAI binaries must ship with correct SELinux labels.
- ToolExecutor must not require disabling SELinux for any action.
- Computer-use hidden mode (future) must run under a confined SELinux domain (see `COMPUTER_USE_IMPLEMENTATION_SPEC.md`).

### cgroups v2

- Default. Used for service resource limits.
- `osai-api`, `osai-model-router`, `llama-server` get systemd slice resource limits (CPU, memory, IO).
- Hidden computer-use sessions get a separate confined cgroup.

### systemd

- System services run as `root` for system-level orchestration.
- User services run as the OSAI user.
- See `SYSTEM_SERVICES_DESIGN.md` for the full service graph.

### Wayland

- Default session.
- Computer-use visible mode operates on the active Wayland session.
- Computer-use hidden mode uses a nested Wayland compositor or virtual display.
- See `COMPUTER_USE_IMPLEMENTATION_SPEC.md`.

### PipeWire

- Audio stack for OSAI.
- Voice input (future) uses PipeWire as the source.
- Voice output (future TTS) uses PipeWire sink.

## What Belongs in the Base Image

| Belongs in base | Why |
|---|---|
| `osai-cli` binary | Required for OOBE and basic operation. |
| `osai-agent-core` (compiled) | Required by `osai-api` and `osai-cli`. |
| `osai-api` binary | Required for desktop and remote use. |
| Pre-staged smoke model | Required to validate that the runtime works before downloading the real model. |
| Systemd unit files for OSAI services | Required for autostart. |
| Default config templates | Required for first-run setup. |
| Installer binary | Required to install the image. |
| OOBE assistant | Required for first boot. |
| Receipt secret-scan helper | Required to keep receipts clean. |

## What Belongs in First-Run Setup (OOBE)

| Belongs in OOBE | Why |
|---|---|
| User account creation | Cannot pre-create without knowing the username. |
| Hostname | Cannot pre-set. |
| Locale, timezone, keyboard | Cannot pre-set. |
| Privacy defaults | User choice (see `PRIVACY_MODEL.md`). |
| API token generation | Per-user secret, generated at first run. |
| Default allowed roots | Per-user choice. |
| GPU and runtime detection | Per-hardware. |
| Real model selection and download | Per-hardware VRAM tier. |
| Cloud credentials (if opted in) | Per-user, opt-in only. |
| Initial health check | Must run after OOBE. |

OOBE is documented in detail in `INSTALLER_AND_OOBE.md`.

## Open Questions

- Should the base image include a pre-staged desktop environment, or should desktop be a layer? (Open: depends on uBlue base choice.)
- Should OSAI bundle its own uBlue flavor (e.g. `osai-bazzite`, `osai-cosmic`) or rely on user-installed desktops? (Open: prefer user-installed for v1.)
- How to handle driver signing for NVIDIA proprietary drivers in the immutable base? (Open: uBlue already handles this; OSAI inherits.)
- Should `/var/cache/osai` be encrypted at rest on laptops? (Open: defer to LUKS full-disk encryption from installer.)

## Acceptance Criteria

This design is accepted when:

- A coder can build a minimal BlueBuild recipe that produces a bootable OSAI image with: a working `osai-api` service, a pre-staged smoke model, and a documented OOBE.
- A reviewer can confirm the image layers from this document match the produced image.
- A tester can boot the image, run the OOBE, and produce a working system with at least one real model downloaded.
- Rollback is testable: after an update, the user can boot the previous deployment and OSAI state is preserved.
