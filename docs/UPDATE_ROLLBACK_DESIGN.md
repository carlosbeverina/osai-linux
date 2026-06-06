# Update and Rollback Design

## Purpose

Defines how OSAI Linux updates itself and rolls back: OS updates, OSAI component updates, model updates, config migrations, receipt format migrations, rollback behavior, failure recovery, and compatibility guarantees.

## Goals

- Updates are atomic. Either they fully succeed or they leave the system in the prior state.
- The user can roll back to a previous OS deployment via the bootloader.
- The user can roll back the OSAI component without rolling back the OS.
- Model updates do not require an OS update.
- Receipts and settings survive updates and rollbacks.
- Old receipts remain readable after a format migration.

## Non-Goals

- Live patching of the running kernel.
- Hot-swapping a running `osai-api` without restart.
- Auto-rolling-back an update that the user has used for >24 hours.

## OS Updates

OS updates use `rpm-ostree`. They pull a new deployment:

```
rpm-ostree update
systemctl reboot
```

The new deployment is staged. After reboot, the user runs on the new deployment. If something is wrong, the user can select the previous deployment at the GRUB/systemd-boot menu and reboot.

OS updates include:

- New kernel
- New SELinux policy
- New system packages
- New system service unit files for OSAI
- New `osai-cli`, `osai-agent-core`, `osai-api` binaries

OS updates do NOT include:

- New models
- New per-user settings
- New per-user receipts
- New per-user sessions

## OSAI Component Updates

OSAI components are versioned independently of the OS image. The component version is recorded in `/etc/osai/version.toml`:

```toml
osai_cli = "0.2.0"
osai_agent_core = "0.2.0"
osai_api = "0.2.0"
osai_panel = "0.2.0"  # future
```

Component updates are delivered as:

- A new OS image (atomic, with `rpm-ostree update`).
- A signed delta package that updates the binaries in place (lighter weight, no OS reboot).

For v1, we use the OS image path only. Delta packages are a v2 optimization.

## Model Updates

Model updates are independent of the OS image. The user can pull a new model version without rebooting:

```
osai-cli model pull gemma4:12b --version 2025-07-15
```

Model updates:

- Download the new GGUF file.
- Verify the SHA256 against the catalog.
- Replace the old file (after the new file is verified).
- Update the `installed.toml` metadata.
- Trigger a model switch if the user has set "auto-update models" in Settings.

Model updates do NOT change the active model. The user must explicitly switch or set the new version as default.

## Config Migrations

OSAI config has a schema version. When the schema changes:

1. The new version of `osai-cli` reads the old `settings.toml`.
2. If the schema is older than the supported range, the CLI runs a migration.
3. The migrated file is written to a new path (e.g., `settings.toml.v2`) and the old file is moved to `settings.toml.v1.bak`.
4. The CLI prompts the user to confirm the migration if the change is non-trivial.

Migration rules:

- Backward compatible changes: no migration needed.
- Field renames: automatic migration.
- Field splits: automatic migration with derived values.
- Field removals: automatic with a warning.
- Behavior changes: prompt the user.

The schema version is stored in `~/.config/osai/settings.toml.schema_version` and `/etc/osai/settings.toml.schema_version`.

## Receipt Format Migrations

Receipts are JSON files with a `format_version` field. When the format changes:

1. `osai-cli receipts migrate` reads all receipts in the receipt directories.
2. For each receipt with an older format, the CLI runs the migration.
3. The migrated receipt is written to a new file with the new format.
4. The old file is renamed to `<id>.v1.bak`.

Migration rules:

- Old receipts MUST remain readable indefinitely (read-only fallback).
- New fields default to `null` or empty.
- Removed fields are simply absent in migrated receipts.
- The format version is bumped on any change.

## Rollback Behavior

### OS Rollback

```
rpm-ostree rollback
systemctl reboot
```

After reboot, the previous deployment is the default. The user can also select the previous deployment at the bootloader.

State preserved across rollback:

- `/var/lib/osai/**` -- receipts, sessions, settings
- `/var/cache/osai/**` -- model cache
- `/var/log/osai/**` -- logs
- `~/.local/share/osai/**` -- per-user data
- `~/.config/osai/**` -- per-user settings and tokens

State NOT preserved across rollback:

- None. Rollback is a state-preserving operation.

OSAI services restart on the rolled-back OS. If a service unit file is incompatible with the older OS, the service fails to start and logs a clear error. The user can fall back to the latest OS.

### OSAI Component Rollback

To roll back the OSAI component without rolling back the OS:

1. Use the previous OS image that contains the older OSAI component.
2. Or, install the older component manually (download a specific `.rpm` or binary).

For v1, OSAI component rollback is bound to OS rollback. The component is shipped as part of the OS image.

### Model Rollback

To roll back a model:

```
osai-cli model pull gemma4:12b --version 2025-06-01
osai-cli model use gemma4:12b
```

The model is downloaded (if not cached), verified, and switched to. Receipts record the version change.

## Failure Recovery

| Failure | Recovery |
|---|---|
| OS update fails mid-download | rpm-ostree aborts; old deployment remains default. No reboot required. |
| OS update applies but new image fails to boot | Bootloader offers the previous deployment. The user selects it. |
| Component update fails | Atomic update is reverted. The user continues on the prior version. |
| Model download fails | CLI retries 3 times with exponential backoff. On final failure, the user is shown the error and can retry later. The previous model (if any) is untouched. |
| Model checksum mismatch | Downloaded file is deleted. The user is shown a clear error. The previous model is untouched. |
| Config migration fails | Original config is preserved. The user is shown the error and can fix manually. |
| Receipt migration fails | Original receipt is preserved. The user is shown the error. The CLI can re-run migration per-receipt. |
| Service fails to start after update | systemd logs the failure. `osai-cli doctor` reports the failure. The user can downgrade the OS image to recover. |

## Compatibility Guarantees

| Compatibility | Guarantee |
|---|---|
| Receipts written by version X | Readable by version X, X+1, X+2 (read-only fallback for older). |
| Settings written by version X | Readable by version X, X+1 (with auto-migration if needed). |
| Plan DSL written by version X | Validated against the same schema unless `version` field is bumped. |
| API written by version X | Compatible with clients that target the same major version. Breaking changes bump the major version. |
| Model file written by version X | Compatible with llama.cpp versions that support the same GGUF spec. |
| Service unit files | Backward compatible: an old unit file works on a new OSAI. A new unit file may not work on an old OSAI. |

## Update Channels

- `stable` -- tested, signed, recommended.
- `beta` -- pre-release, signed, for testers.
- `nightly` -- unsigned, latest, for developers.

The default channel is `stable`. The user can switch in Settings.

## Automatic Updates

The user can opt into automatic updates. When enabled:

- OS updates are downloaded in the background. The user is notified to reboot.
- Model updates are downloaded in the background. The user is notified to switch.
- Component updates are downloaded as part of OS updates.

Automatic updates NEVER reboot the system automatically. The user always controls the reboot.

## Open Questions

- Should we ship a "preview channel" between `stable` and `beta`? (Open: defer.)
- Should we support staged rollouts (e.g., 10% of users get the new image first)? (Open: yes, but in v2; for v1 all users on `stable` get the same image.)
- Should we support offline updates (download an `.iso` or `.tar` and apply locally)? (Open: yes; planned for v2.)
- Should we support delta OS updates to reduce download size? (Open: yes; uBlue supports this via OSTree deltas.)

## Acceptance Criteria

This design is accepted when:

- A tester can run `rpm-ostree update` and see a new deployment staged.
- A tester can reboot into the new deployment and verify all OSAI services start.
- A tester can reboot into the previous deployment and verify all OSAI services start.
- A tester can `osai-cli model pull` a new model version and see it download and verify.
- A tester can `osai-cli model use` the new version and see the system switch.
- A tester can manually corrupt a config file and watch the CLI refuse to load it and offer recovery.
- A tester can run `osai-cli receipts migrate` and see all receipts upgraded.
- A reviewer confirms rollback is state-preserving and receipts remain readable.
