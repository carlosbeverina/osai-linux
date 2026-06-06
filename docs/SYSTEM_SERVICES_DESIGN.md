# System Services Design

## Purpose

Defines the systemd service layout for OSAI Linux: which services exist, what they do, their dependencies, token/config paths, logs, health checks, restart policies, loopback-only binding, and the service dependency graph.

This is a design document. It does not implement the services or unit files.

## Goals

- All OSAI processes run as systemd services (system or user) with proper restart, logging, and sandboxing.
- All local services bind to loopback only by default.
- The user can introspect any service via `systemctl --user status` or `systemctl status`.
- The service graph is auditable and minimal.
- Health checks are exposed via a single endpoint and via systemd's status reporting.

## Non-Goals

- A process supervisor other than systemd. (systemd is the only supported supervisor.)
- Multi-host orchestration. OSAI is single-host for v1.
- Container-based services. Services run natively (computer-use hidden mode may use containers, but that's documented separately).
- Service mesh, sidecar proxies, etc.

## Service Layout

OSAI uses two systemd scopes:

- **System services** (run as root) -- platform-level.
- **User services** (run as the OSAI user) -- session-level.

The split exists because:

- System services handle platform-level concerns (driver checks, image health, OOBE state).
- User services handle per-session concerns (Model Router, `osai-api`, panel).
- User services can be restarted by the user without affecting other users.

### System Services

| Service | Purpose | Runs as | Binds to |
|---|---|---|---|
| `osai-image-health.service` | One-shot at boot; checks base image, SELinux, cgroups. | root | n/a |
| `osai-oobe.service` | One-shot on first boot; runs OOBE. | root (then user) | n/a |
| `osai-driver-check.service` | One-shot at boot; validates GPU driver, CUDA, etc. | root | n/a |
| `osai-update-check.timer` | Periodic timer; checks for OSAI updates. | root | n/a |
| `osai-update-check.service` | One-shot; performs the actual update check. | root | https://api.github.com |
| `osai-receipt-vacuum.timer` | Periodic timer; vacuums old receipts. | root | n/a |
| `osai-receipt-vacuum.service` | One-shot; prunes old receipts per retention policy. | root | n/a |

### User Services

| Service | Purpose | Runs as | Binds to |
|---|---|---|---|
| `osai-model-router.service` | FastAPI OpenAI-compatible gateway. | osai user | 127.0.0.1:8088 |
| `osai-llama-server.service` | llama.cpp inference server. | osai user | 127.0.0.1:8092 |
| `osai-api.service` | `osai-api` local HTTP API. | osai user | 127.0.0.1:8090 |
| `osai-hotkey.service` | Global hotkey daemon (panel + computer-use). | osai user | session DBus |
| `osai-panel.service` | Assistant panel UI daemon. | osai user | session DBus |
| `osai-compute-visible.service` | Visible computer-use controller (future). | osai user | session DBus |
| `osai-compute-hidden.service` | Hidden computer-use controller (future). | osai user | session DBus |
| `osai-voice.service` | Voice pipeline (future). | osai user | session DBus |

## Service Dependency Graph

```
[boot] osai-image-health.service (one-shot)
   |
   v
[boot] osai-driver-check.service (one-shot)
   |
   v
[user-session-start]
   |
   +-> osai-llama-server.service
   |     (waits for: model file present, GPU ready)
   |
   +-> osai-model-router.service
   |     (waits for: osai-llama-server.service active)
   |
   +-> osai-api.service
   |     (waits for: osai-model-router.service active)
   |
   +-> osai-panel.service
   |     (waits for: osai-api.service active)
   |
   +-> osai-hotkey.service
   |     (waits for: session DBus)
   |
   +-> osai-compute-visible.service (future)
   |     (waits for: osai-api.service, Wayland portal)
   |
   +-> osai-compute-hidden.service (future)
         (waits for: osai-api.service, podman)
```

## osai-api Service

- Unit: `osai-api.service` in `~/.config/systemd/user/`.
- ExecStart: `/usr/bin/osai-api` (with default flags from `/etc/osai/osai-api.env`).
- Restart: `on-failure` with backoff (5s, 10s, 30s, max 5 restarts in 5 minutes, then stop).
- Environment: `OSAI_API_TOKEN` from systemd LoadCredential (the token file is in `~/.config/osai/api-token`).
- BindAddress: `127.0.0.1:8090`.
- Hardening: `NoNewPrivileges=yes`, `ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateTmp=yes`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`, `SystemCallArchitectures=native`, `SystemCallFilter=@system-service`, `ReadWritePaths=/var/lib/osai /var/log/osai`, `MemoryDenyWriteExecute=yes` (drop if incompatible with deps).

## model-router Service

- Unit: `osai-model-router.service`.
- ExecStart: `python3 -m uvicorn main:app --host 127.0.0.1 --port 8088` from `/usr/lib/osai/model-router/`.
- Restart: `on-failure` with backoff.
- Environment: provider URLs (llama-server, MiniMax API key, etc.) from `/etc/osai/model-router.env`.
- BindAddress: `127.0.0.1:8088`.
- Hardening: similar to `osai-api` but allows Python's broader system calls. Consider `RestrictNamespaces=yes`, `RestrictRealtime=yes`.

## llama-server Service

- Unit: `osai-llama-server.service`.
- ExecStart: `/usr/bin/llama-server -m <model> -c <context> -ngl 99 --host 127.0.0.1 --port 8092`.
- The model path is computed at OOBE time and stored in `/etc/osai/llama-server.env`.
- Restart: `on-failure`. If the GPU is missing, the service is permanently failed and the fallback CPU model is started instead.
- BindAddress: `127.0.0.1:8092`.
- Hardening: same as `osai-api` plus `RestrictNamespaces=yes` to prevent container escapes.

## Hotkey and Panel Services

- `osai-hotkey.service` and `osai-panel.service` are user services that live in the user's graphical session.
- They depend on the session's DBus being available.
- They are not privileged.
- Restart: `on-failure`.

## Token and Config Paths

| Path | Owner | Mode | Notes |
|---|---|---|---|
| `/etc/osai/osai-api.env` | root | 0644 | `OSAI_API_BIND=127.0.0.1:8090` |
| `/etc/osai/llama-server.env` | root | 0644 | `OSAI_LLAMACPP_MODEL=...`, `OSAI_LLAMACPP_CONTEXT=...` |
| `/etc/osai/model-router.env` | root | 0640 | `MINIMAX_API_KEY=...` (if opt-in) |
| `~/.config/osai/api-token` | user | 0600 | per-user API token, 256-bit random |
| `~/.config/osai/settings.toml` | user | 0644 | user settings |
| `~/.config/osai/cloud-credentials.toml` | user | 0600 | MiniMax API key (if opt-in) |
| `~/.config/systemd/user/osai-*.service` | user | 0644 | user service unit overrides |
| `/var/lib/osai/oobe-state.toml` | root | 0644 | OOBE progress |
| `/var/lib/osai/receipt-vacuum-state.toml` | root | 0644 | last vacuum timestamp |

## Logs

| Service | Journal |
|---|---|
| System services | system journal (`journalctl -u osai-image-health`) |
| User services | user journal (`journalctl --user -u osai-api`) |

Logs are rotated by systemd-journald. Retention: 30 days default, configurable in `/etc/systemd/journald.conf`.

Errors and panics are also recorded in the receipt store (when applicable). Receipts are the audit trail; logs are the operational trail.

## Health Checks

Each user service exposes a `GET /health` endpoint (where applicable). The `osai-cli doctor` command queries all health endpoints and reports a single status:

- OK: every service responded with 200 and `ok: true`.
- Degraded: at least one non-critical service is down.
- Failed: at least one critical service is down.

### Health Endpoints

| Service | Endpoint | Critical? |
|---|---|---|
| `osai-api` | `GET /health` | yes |
| `osai-model-router` | `GET http://127.0.0.1:8088/health` | yes |
| `osai-llama-server` | `GET http://127.0.0.1:8092/v1/models` (must return at least 1) | yes |
| `osai-panel` | DBus signal `osai.panel.IsReady` | no |
| `osai-hotkey` | DBus signal `osai.hotkey.IsReady` | no |

`osai-cli doctor` runs all checks in parallel with a 5-second timeout each.

## Restart Policies

| Service | Restart | Max retries in 5 min | Backoff |
|---|---|---|---|
| `osai-api` | on-failure | 5 | 5s, 10s, 30s, 60s, 120s |
| `osai-model-router` | on-failure | 5 | same |
| `osai-llama-server` | on-failure | 3 | 30s, 60s, 120s (longer because model reload is slow) |
| `osai-hotkey` | on-failure | 5 | 5s, 10s, 30s |
| `osai-panel` | on-failure | 5 | 5s, 10s, 30s |

If max retries are exceeded, the service is stopped and a notification is shown. The user can manually restart with `systemctl --user restart osai-api`.

## Loopback-Only Binding

Every user service binds to `127.0.0.1`. The systemd unit file MUST use `--host 127.0.0.1` (or equivalent). The unit file MUST NOT bind to `0.0.0.0` or to a public interface.

`osai-api` enforces this: if `OSAI_API_BIND` is set to a non-loopback address, the service refuses to start and logs an error.

## Updates via `rpm-ostree`

System service unit files are part of the immutable base image. Updating a unit file is an image update (see `UPDATE_ROLLBACK_DESIGN.md`).

User service unit overrides are stored in `~/.config/systemd/user/`. They survive updates and rollbacks.

## Open Questions

- Should `osai-model-router` be a system service or a user service? (Open: user service for v1; system service in v2 for multi-user.)
- Should there be a single `osai.target` that pulls in all OSAI services? (Open: yes; reduces user friction.)
- Should OOBE run as a service or a one-shot? (Open: one-shot; idempotent; OOBE state stored in `/var/lib/osai/oobe-state.toml`.)
- Should the panel service be a separate process, or part of `osai-api`? (Open: separate process; isolates UI crashes from API.)

## Acceptance Criteria

This design is accepted when:

- A coder can write the unit files for all listed services.
- A tester can `systemctl --user status osai-api` and see a healthy state.
- A tester can `osai-cli doctor` and see a unified report covering all services.
- A tester can kill any service and watch systemd restart it.
- A tester cannot make any service bind to a non-loopback interface.
- A reviewer confirms no service runs as root unless it must.
