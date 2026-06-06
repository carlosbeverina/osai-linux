# Computer Use Implementation Specification

## Purpose

This is the implementation-level specification for OSAI's computer-use subsystem. It builds on `COMPUTER_USE_STRATEGY.md` and `SECURITY_MODEL.md` and is the contract between the design and the coder. It defines the visible and hidden mode architectures, the isolation tradeoffs, the screenshot pipeline, the OCR/vision pipeline, the browser automation bridge, the artifact store, task cancellation, environment reset, network/file/browser policies, the receipt format, the test plan, and the strict approval and privacy controls.

## Goals

- Visible and hidden computer-use are implemented as separate, isolated subsystems.
- Every computer-use task starts as a Plan DSL plan, is validated, and goes through ToolBroker authorization.
- Sensitive actions always require explicit user approval.
- Hidden mode is auditable, cancellable, and resettable.
- No credential entry without explicit user action.
- No purchases/payments without explicit approval.
- No destructive changes without explicit approval.

## Non-Goals

- Always-on invisible agents. OSAI never operates invisibly without user knowledge.
- Cross-host computer use. Computer use is local-only.
- VNC-like uncontrolled remote access. Computer use is mediated by ToolBroker.
- Browser-based credential management. OSAI cannot autofill credentials.

## Modes

### Visible Computer Use

- OSAI operates in the user's active Wayland session.
- The user sees everything OSAI does.
- Sensitive actions pop the approval modal in the active session.
- Screenshots are stored with privacy controls (see below).

### Hidden Computer Use

- OSAI operates in an isolated environment.
- The user does not see intermediate steps.
- The user reviews final outputs before any external transmission.
- The environment is reset/destroyable at any time.

## Isolation Tradeoffs

The hidden environment can be implemented several ways. Each has tradeoffs:

| Option | Isolation | Cost | Performance | Reset Difficulty | Recommended for v1? |
|---|---|---|---|---|---|
| Nested Wayland compositor (`cage`, `weston --nested`) | High (separate Wayland surface) | Low | Good (GPU passthrough) | Easy (kill + restart) | Yes |
| Virtual display (X11 `Xvfb` or Wayland `wlheadless`) | High (no real display) | Low | Good | Easy | Yes |
| Containerized desktop (`podman` + `distrobox`) | High (separate user namespace) | Medium | Good (GPU passthrough with CDI) | Medium | Yes (for testing) |
| VM (KVM/QEMU) | Highest | High (full OS) | Poor (GPU passthrough is complex) | Easy (snapshot + revert) | No (v1) |
| Separate Linux user/session (logind session separation) | Medium (same kernel) | Low | Best | Hard (manual cleanup) | No (overlaps with nested) |

**Recommendation for v1**: Use a nested Wayland compositor (`cage` or `wlheadless`) with a Wayland portal. Containerized desktop via `podman` is a fallback for environments where `cage` is unavailable.

## Architecture

### Visible Mode

```
User Desktop/Session (active, visible)
  |
  | OSAI operates with user watching
  | User can interrupt/cancel/approve
  |
  v
osai-api
  | Plan DSL, validation, ToolBroker
  v
ComputerUseVisible ToolDriver
  |
  | Screenshot capture (wlr-screencopy or PipeWire)
  | Input simulation (wlr-virtual-pointer, wlr-virtual-keyboard)
  | Window/element identification (AT-SPI / wlr-foreign-toplevel)
  |
  v
Wayland Compositor
  |
  v
Active Session UI
```

### Hidden Mode

```
Isolated Environment
(nested Wayland compositor or container, separate session)
  |
  | OSAI operates without user watching
  | Isolated from active desktop
  |
  v
osai-api (in the same osai instance, or in the container)
  | Plan DSL, validation, ToolBroker
  v
ComputerUseHidden ToolDriver
  |
  | Virtual display (wlheadless or Xvfb)
  | Input simulation (uinput)
  | Browser automation (Playwright)
  |
  v
Hidden Session UI (sandboxed, no access to user data)
  |
  v
Artifact Store
  |
  v
User Review
```

## Computer-Use Broker

A new component, `osai-compute-broker`, orchestrates computer-use tasks. It:

- Accepts a Plan DSL plan with a `ComputerUseVisible` or `ComputerUseHidden` action.
- Validates the plan.
- Calls ToolBroker for authorization.
- Manages the lifecycle: create, run, monitor, cancel, complete, destroy.
- Writes receipts for every step.
- Surfaces approval requests to the UI via DBus.

The broker is a system service (`osai-compute-broker.service`) and a user service (`osai-compute-broker-user.service`). The user service is for visible mode; the system service is for hidden mode (which may need root-level Wayland access).

## Screenshot Pipeline

### Capture

- Visible mode: `wlr-screencopy` (via `wlr-screencopy-unstable-v1` protocol) or PipeWire portal.
- Hidden mode: read the virtual display directly.

### Privacy Redaction

Before storage, screenshots are run through a redaction step:

- PII detection: known patterns for emails, phone numbers, SSN, credit card numbers.
- UI element redaction: optional, configurable in Settings.
- EXIF stripping: remove all EXIF metadata from image files.
- Hash and length are stored in the receipt; pixel data is stored separately and redaction status is recorded.

### Storage

- Per-task directory: `/var/lib/osai/compute/<task_id>/screenshots/`
- Or per-user: `~/.local/share/osai/compute/<task_id>/screenshots/`
- Retention: per Settings (`computer_use.screenshot_retention_days`, default 7).

## OCR/Vision Pipeline

For understanding screenshots, the pipeline:

- Tries local OCR first (Tesseract or a local vision model).
- Falls back to cloud vision only with explicit user opt-in (`privacy: cloud_fallback` or `cloud_only`).
- The OCR text is stored in the receipt (not the screenshot pixels).
- Vision model outputs are stored in the receipt.

For local vision:

- Tesseract is pre-installed in the base image.
- A local vision model (Qwen2.5-VL, LLaVA, or similar) may be added later as a separate, opt-in model.

## Browser Automation Bridge

Browser automation is implemented using Playwright (Python or Node.js).

- The browser runs in a separate user-data-dir per task.
- Cookies, sessions, and local storage are isolated per task.
- The browser is launched with a custom user agent and minimal extensions.
- The browser has no access to the user's active session.

### Browser ToolDriver Actions

- `BrowserOpenUrl` -- opens a URL in a new page.
- `BrowserClick` -- clicks an element by selector.
- `BrowserType` -- types into an input.
- `BrowserExtract` -- extracts text or attribute from the page.
- `BrowserScreenshot` -- takes a screenshot of the current page.
- `BrowserClose` -- closes the browser and saves state.

Each action is a Plan DSL step with a corresponding ToolBroker decision.

## Artifact Store

The artifact store holds all outputs from a computer-use task:

- Screenshots (with redaction status).
- Extracted text (OCR, DOM extraction).
- Generated files (downloads, exports).
- Final summaries.
- Browser state (cookies, history) for the task.

Structure:

```
/var/lib/osai/compute/<task_id>/
  metadata.toml
  plan.yml
  screenshots/
    001.png
    001.txt       # OCR text
    001.redacted  # true/false
  outputs/
    report.pdf
    data.csv
  summaries/
    summary.md
  browser/
    cookies.json
    history.json
  receipts/
    step-001.json
    step-002.json
```

The user can browse, copy, or delete the artifact store via the UI. Deletion is irreversible.

## Task Lifecycle

```
Created -> Authorized -> Running -> (Paused -> Running) -> Completed | Cancelled | Failed
                                                    \-> Destroyed
```

States:

- `Created` -- task is created, not yet authorized.
- `Authorized` -- ToolBroker has authorized the plan.
- `Running` -- the task is executing.
- `Paused` -- the user has paused the task; the environment is preserved.
- `Completed` -- the task finished successfully.
- `Cancelled` -- the user or a timeout cancelled the task.
- `Failed` -- the task errored; the environment is preserved for debugging.
- `Destroyed` -- the environment is wiped; the artifact store may be retained.

## Task Cancellation

Cancellation must be possible at any time:

- User clicks "Cancel" in the UI.
- User presses the cancel hotkey (`Ctrl+Alt+R`).
- The task exceeds the configured duration (default 2 hours).
- The task exceeds the configured budget (e.g., token count).
- The system shuts down.

The cancel procedure:

1. Set the task state to `Cancelled`.
2. Stop the current step (graceful, with a 5-second grace period).
3. If graceful cancel fails, force-stop the environment.
4. Write a receipt recording the cancellation, the step that was running, and the environment state.
5. The artifact store is retained unless the user explicitly destroys the environment.

## Environment Reset/Destroy

The user can reset the environment to a clean state:

- For nested Wayland: kill the compositor, restart from a clean snapshot.
- For containers: `podman rm -f` and re-create from the base image.
- For VMs: revert to the saved snapshot.

The user can also destroy the environment entirely:

- All session state is wiped.
- Cookies, history, downloaded files are deleted.
- The artifact store may be retained (user choice).

## Network Policies

Computer-use tasks have a strict network policy:

- Allowed domains: configurable per task, default empty (no network).
- Denied domains: banking, email providers, social media, payment processors are denied by default.
- Outbound connections are logged in the receipt.
- New domains require user approval.

The policy is enforced at the firewall level (nftables) and at the application level (browser, curl).

## File Policies

Computer-use tasks have a strict file policy:

- Allowed paths: per Task DSL plan, plus `~/Downloads` and `~/Documents` (default).
- Denied paths: `~/.ssh`, `~/.gnupg`, `~/.config/osai/api-token`, `~/.local/share/osai/keyring` are always denied.
- New paths require user approval.

The policy is enforced by the file ToolDrivers (FilesList, FilesWrite, FilesMove, FilesDelete).

## Browser Policies

Browser automation has additional policies:

- No autofill of passwords.
- No autofill of credit card numbers.
- No autofill of 2FA codes.
- No downloads without user approval (per file).
- No popups or redirects without user approval.
- No script injection without user approval.

These are enforced by Playwright options and by the browser ToolDriver's pre-action checks.

## Receipt Format

A computer-use task produces a master receipt plus per-step receipts.

### Master Receipt

```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "ComputerUse",
  "mode": "visible|hidden",
  "status": "Executed|Denied|Failed|Cancelled",
  "plan_id": "uuid",
  "requested_task": "Research competitor pricing",
  "duration_seconds": 45,
  "outcome": "completed|cancelled|failed|denied",
  "isolation_environment_id": "env-uuid",
  "screenshots_captured": 3,
  "screenshots_redacted": 3,
  "artifacts_created": [
    {"type": "screenshot", "path": "...", "redacted": true, "size_bytes": 12345},
    {"type": "text_summary", "path": "...", "redacted": false, "size_bytes": 234}
  ],
  "files_touched": ["~/Downloads/report.pdf"],
  "urls_opened": ["https://competitor.example.com/pricing"],
  "credentials_used": [],  // types, never values
  "approvals": [
    {"step": "step-3", "action": "BrowserOpenUrl", "approved": true, "timestamp": 1234567892, "user": "carlos"}
  ],
  "steps": [
    {"id": "step-1", "action": "FilesList", "result": "Executed"},
    {"id": "step-2", "action": "ModelChat", "result": "Executed"},
    {"id": "step-3", "action": "BrowserOpenUrl", "result": "Executed"}
  ],
  "receipts_dir": "..."
}
```

### Per-Step Receipts

Per-step receipts use the existing receipt format. They are linked to the master receipt via `plan_id` and `step_id`.

## Test Plan

The computer-use subsystem has a comprehensive test plan in `TESTING.md`. Highlights:

### Visible Mode

- No action without ToolBroker authorization.
- Receipts generated for all actions.
- Screenshots captured and stored with privacy controls.
- User can interrupt and cancel.
- Approval required for sensitive actions.
- No credential entry without explicit user action.

### Hidden Mode

- Isolated environment does not affect active desktop.
- All actions logged in receipts.
- Task can be cancelled.
- Environment can be reset or destroyed.
- Final outputs returned to user.
- No hidden exfiltration of data.
- Screenshots/artifacts under privacy controls.

### Security

- No network access beyond policy.
- No file access beyond allowed roots.
- No credential exfiltration.
- Screenshots protected (redacted, not transmitted without consent).
- Browser sessions isolated.
- Isolated environment cannot access user's active session.

### Privacy

- No full prompts stored.
- No API keys or tokens in receipts.
- No screenshot pixels stored without redaction.
- No file contents stored unless explicitly required.

## Strict Approval and Privacy Controls

The following actions ALWAYS require explicit user approval, regardless of the policy or the user setting:

- Any action involving credentials (passwords, API keys, 2FA codes, OAuth tokens).
- Any action involving payments (purchase, transfer, subscription, donation).
- Any action involving account changes (email, social media, banking).
- Any action involving destructive file operations (delete, move to trash, overwrite).
- Any action involving external communication (email, chat, social posting).
- Any action involving sensitive categories as defined in `SECURITY_MODEL.md`.

These controls are enforced by ToolBroker and cannot be bypassed by the user setting `approve_all: true`.

## Open Questions

- Should the broker be a single component or separate per-mode? (Open: single component with mode flag; simpler operationally.)
- Should screenshots be stored in the artifact store or streamed directly to the user? (Open: store for v1; stream in v2.)
- Should we support multiple concurrent hidden tasks? (Open: yes, in v2; one task per user in v1.)
- Should we add a "watch only" mode where the user can observe but not approve? (Open: defer.)
- Should we add a "training mode" where actions are recorded for later review? (Open: defer.)

## Acceptance Criteria

This spec is accepted when:

- A coder can implement the broker, the screenshot pipeline, and the artifact store.
- A tester can run a visible computer-use task end-to-end on the validated hardware.
- A tester can run a hidden computer-use task in a nested Wayland session.
- A tester can cancel a running task at any time.
- A tester can reset and destroy the hidden environment.
- A tester can verify that no credentials, no full prompts, and no unredacted screenshots are in the receipts.
- A reviewer confirms the network, file, and browser policies are enforced.
- A reviewer confirms the strict approval list is enforced.
