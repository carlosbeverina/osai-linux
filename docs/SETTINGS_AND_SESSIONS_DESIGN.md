# Settings and Sessions Design

## Purpose

Defines the persistent settings schema for OSAI Linux, the privacy modes, model settings, allowed roots, API token management, receipts retention, sessions/history storage, future memory manager boundaries, and export/delete controls.

## Goals

- All settings are stored in TOML files with a schema version.
- Settings can be edited via the UI, `osai-cli`, or directly (advanced).
- Sessions are stored on disk and survive restarts.
- The user can export or delete all of their OSAI data.
- Future memory features integrate without breaking the existing schema.

## Non-Goals

- A cloud-synced settings store. Settings are local to the host.
- A relational database. SQLite is the v1 store for sessions; TOML for settings.
- Cross-host settings sharing. (Future; v2 enterprise.)

## Settings Schema

Settings live in two locations:

- `/etc/osai/settings.toml` -- system-wide defaults.
- `~/.config/osai/settings.toml` -- per-user overrides.

The CLI merges the two: system defaults are loaded first, then per-user overrides are applied. The user can `osai-cli settings show --effective` to see the merged result.

### Schema

```toml
schema_version = 1

[privacy]
default_mode = "local_only"  # local_only | cloud_fallback | cloud_only
allow_cloud_opt_in = true
redact_cloud_requests = true  # strip request bodies from cloud receipts
last_cloud_use_ts = 0  # unix timestamp, 0 = never

[models]
default_alias = "default"  # resolves via alias map + hardware profile
fallback_alias = "gemma4:e2b"  # always available, even if not installed
auto_switch_on_profile_change = true
context_length = 4096
temperature = 0.2
max_tokens = 512

[allowed_roots]
roots = ["~/Downloads", "~/Documents"]  # read/write by default
default_write = "~/Downloads"
allow_symlink_resolution = false
deny_paths = ["~/.ssh", "~/.gnupg", "~/.config/osai/api-token", "~/.local/share/osai/keyring"]

[api_token]
source = "file"  # file | env | disabled
rotation_required = false
last_rotated_ts = 0

[receipts]
retention_days = 90
auto_vacuum = true
vacuum_interval_hours = 24
export_format = "json"  # json | jsonl

[sessions]
default_session_name = "Default"
max_sessions = 1000
auto_save = true
search_enabled = true
store_full_history = true  # if false, only summaries

[ui]
panel_position = "right"  # left | right | top | bottom | floating
theme = "auto"  # auto | light | dark | high-contrast
reduce_motion = false
default_panel_size = "small"  # small | medium | large
show_token_usage = true
assistant_hotkey = "Super+Space"
cancel_hotkey = "Ctrl+Alt+R"
computer_use_visible_hotkey = "Super+Shift+Space"

[updates]
channel = "stable"  # stable | beta | nightly
auto_check = true
check_interval_hours = 24
auto_download = false
auto_install_os_updates = false

[voice]
enabled = false  # future
input_device = "default"
output_device = "default"
push_to_talk_hotkey = "Ctrl+Space"

[computer_use]
visible_mode_default = false
hidden_mode_allowed = true
screenshot_retention_days = 7
artifact_retention_days = 30
max_session_duration_minutes = 120
cancel_on_user_input = true
```

## Privacy Mode Settings

Three modes, defined in detail in `PRIVACY_MODEL.md`:

- `local_only` (default) -- no cloud. Strongest privacy.
- `cloud_fallback` -- try local first; on error, fall back to cloud. Cloud uses are logged.
- `cloud_only` -- all requests go to cloud. Used for low-end hardware or when local runtime is intentionally disabled.

The `redact_cloud_requests` setting, when true, strips request bodies from receipts written during cloud calls. This further reduces what is stored.

The user can opt out of cloud use entirely by setting `allow_cloud_opt_in = false`. The UI then hides cloud-related settings.

## Model Settings

- `default_alias` -- the alias used when the user says "use the default model". Resolved through the alias map and hardware profile.
- `fallback_alias` -- the model used when the default is unavailable. Should be a known-validated model (e.g., `gemma4:e2b`).
- `auto_switch_on_profile_change` -- when the hardware profile changes (e.g., AC unplugged), the system may switch to a more conservative model.
- `context_length` -- the context window for inference. Capped by available VRAM.
- `temperature`, `max_tokens` -- generation parameters.

The system may also expose per-model overrides:

```toml
[models.overrides."gemma-4-12b-it-qat-q4_0"]
context_length = 4096
temperature = 0.2
```

## Allowed Roots

Allowed roots define where ToolBroker permits file operations. Default: `~/Downloads`, `~/Documents`.

- `roots` -- list of paths the user has explicitly allowed.
- `default_write` -- the default location for write operations.
- `allow_symlink_resolution` -- if true, the system follows symlinks when checking allowed roots. Default false (deny symlinks that escape allowed roots).
- `deny_paths` -- list of paths that are always denied, regardless of allowed roots. Includes SSH keys, GPG keys, API tokens, and the OSAI keyring.

The user can add/remove allowed roots in Settings. Adding a root is reversible. Removing a root does not delete files; it just denies future access.

## API Token Management

The API token is generated at OOBE and stored in `~/.config/osai/api-token` with `chmod 0600`.

- `source` -- `file` (default), `env` (if `OSAI_API_TOKEN` is set), or `disabled` (no auth required; only for development).
- `rotation_required` -- if true, the system refuses to start with an old token (used after a security event).
- `last_rotated_ts` -- unix timestamp of last rotation.

Rotation procedure:

1. `osai-cli api-token rotate` (or via Settings).
2. A new 256-bit random token is generated.
3. The new token is written to `~/.config/osai/api-token`.
4. The old token is invalidated.
5. The system restarts `osai-api` to pick up the new token.
6. The UI prompts the user to log in with the new token.

## Receipts Retention

- `retention_days` -- receipts older than this are deleted. Default 90.
- `auto_vacuum` -- if true, the `osai-receipt-vacuum.timer` service prunes old receipts.
- `vacuum_interval_hours` -- how often the vacuum runs. Default 24.
- `export_format` -- format for receipt exports. JSON or JSONL.

The vacuum service:

- Runs as a systemd timer.
- Reads the retention policy from `~/.config/osai/settings.toml`.
- Deletes receipts older than `retention_days`.
- Logs the deletion count to a system receipt.

The user can disable auto-vacuum and run it manually: `osai-cli receipts vacuum`.

## Sessions and History Storage

Sessions are stored in `~/.local/share/osai/sessions/<session_id>/`:

```
<session_id>/
  metadata.toml      # session name, created, last_modified, model, privacy
  messages.json      # full message history
  index.sqlite       # full-text search index
```

### Schema

`metadata.toml`:
```toml
schema_version = 1
id = "01H..."
name = "Default"
created = 1234567890
last_modified = 1234567890
model = "gemma-4-12b-it-qat-q4_0"
privacy = "local_only"
parent_session_id = ""  # if forked
```

`messages.json`:
```jsonl
{"id": "...", "role": "user", "content": "...", "timestamp": 1234567890}
{"id": "...", "role": "assistant", "content": "...", "model": "...", "prompt_length": 12, "response_length": 34, "timestamp": 1234567891}
```

The messages file uses JSONL (one JSON object per line) for append efficiency and streaming reads.

`index.sqlite` contains:

- A FTS5 virtual table over `messages.content` for full-text search.
- An index on `messages.timestamp` for date-range queries.

Settings control:

- `default_session_name` -- name for new sessions.
- `max_sessions` -- soft cap; oldest sessions are auto-deleted when exceeded (configurable to "never delete").
- `auto_save` -- if false, sessions are not persisted.
- `search_enabled` -- if false, the FTS5 index is not built.
- `store_full_history` -- if false, only the latest N messages are stored; older messages are summarized.

## Future Memory Manager Boundaries

OSAI will eventually have a Memory Manager (see `OSAI_AGENT_CORE_PLAN.md` and `PROJECT_OVERVIEW.md`). The Memory Manager will:

- Store agent-scoped facts, user preferences, and conversation summaries.
- Be opt-in per session.
- Be deletable by the user.
- Never be visible to a different user on the same host.

The current Sessions design is the foundation. The Memory Manager will:

- Use the same per-user directory (`~/.local/share/osai/`).
- Use a separate subdirectory (`memory/`) to keep it isolated.
- Be loaded by `osai-agent-core` only when the user opts in.

The boundary between Sessions (conversation history) and Memory (agent knowledge) is:

- Sessions: what was said. Append-only. Searchable.
- Memory: what the agent learned. Editable. Agent-controlled.

## Export and Delete Controls

### Export

- `osai-cli export --all --out /tmp/osai-export` -- exports settings, sessions, and receipts (sanitized) as a tarball.
- `osai-cli export --settings --out settings.toml` -- exports settings only.
- `osai-cli export --sessions --out sessions.tar` -- exports sessions only.
- `osai-cli export --receipts --out receipts.tar` -- exports receipts only.

The exported data is sanitized:

- No full prompts (only length).
- No API keys or tokens.
- No file contents.
- No screenshot pixel data (only metadata).

### Delete

- `osai-cli delete --all` -- deletes all per-user data (settings, sessions, receipts, models). Confirmation required.
- `osai-cli delete --sessions` -- deletes all sessions.
- `osai-cli delete --receipts` -- deletes all receipts.
- `osai-cli delete --settings` -- resets settings to system defaults.

Deleting all data is irreversible. The CLI asks for confirmation by typing the user's API token suffix.

## Open Questions

- Should settings be encrypted at rest? (Open: defer to LUKS full-disk encryption from the installer.)
- Should sessions be encrypted? (Open: not in v1; consider for v2 if sensitive use cases emerge.)
- Should we support multiple users on the same host with isolated settings? (Open: yes; per-user `~/.config/osai/` already provides this.)
- Should the Settings UI support search? (Open: yes, in v2.)

## Acceptance Criteria

This design is accepted when:

- A coder can implement the settings loader and merger.
- A tester can edit `~/.config/osai/settings.toml` and see the change take effect after a service reload.
- A tester can run `osai-cli export --all` and get a sanitized tarball.
- A tester can run `osai-cli delete --all` and confirm all data is gone.
- A tester can rotate the API token and confirm the old token stops working.
- A reviewer confirms the schema is forward-compatible (additive changes are non-breaking).
- A reviewer confirms the boundary between Sessions and Memory is documented.
