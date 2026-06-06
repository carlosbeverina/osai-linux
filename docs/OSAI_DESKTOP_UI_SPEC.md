# OSAI Desktop UI Specification

## Purpose

Defines the **final desktop UI feature specification** for OSAI. This is the contract between the shell integration (see `DESKTOP_SHELL_INTEGRATION_DESIGN.md`) and the implementation. The Dev Panel (`/ui`) is a development tool and is NOT the subject of this spec.

## Goals

- Define the exact features, behaviors, and visual contracts for the OSAI desktop UI.
- Make the spec implementable: each section gives enough detail that a coder can build the feature.
- Cover all major surfaces: assistant panel, command palette, approvals, receipts, status, privacy, settings, sessions, file previews, computer-use, errors, accessibility.
- Set explicit non-goals to prevent scope creep.

## Non-Goals

- The web Dev Panel is not the final UI. It is a dev tool.
- A web-only experience. OSAI desktop UI is a native shell integration.
- Replacing the user's chosen desktop. OSAI integrates into the desktop.
- Brand-new visual design language. OSAI uses the host desktop's theme (Adwaita / Breeze / COSMIC).
- Mobile. OSAI desktop UI targets desktop/laptop only.

## Surfaces

The OSAI desktop UI has the following top-level surfaces:

1. Assistant Panel
2. Command Palette
3. Approvals Modal
4. Receipts Timeline
5. Model / Runtime Status Indicator
6. Privacy Indicator
7. Settings Panel
8. Sessions / History
9. File Previews
10. Computer-Use Controls
11. Error States
12. Accessibility Layer

Each is specified below.

## 1. Assistant Panel

### Layout

- Header: model name, runtime status (icon + color), privacy indicator, settings gear.
- Input: multi-line text input with submit-on-Enter (Shift+Enter for newline), file attach button, screenshot button (for computer-use).
- Messages: conversation list, newest at the bottom, role indicator per message.
- Footer: token usage for current session, link to full receipts view, link to settings.

### Behavior

- On submit: input is sent to `POST /v1/chat` (or `POST /v1/ask` if ask-mode is selected).
- Streaming: the response is streamed token-by-token to the panel. (Future: requires Model Router streaming support; today, the panel polls every 200ms or uses Server-Sent Events.)
- Markdown is rendered. Code blocks have syntax highlighting.
- The user can stop generation with a "stop" button.
- Sessions are auto-saved; switching sessions restores the scroll position and input.

### Keyboard

- `Ctrl+Enter` -- submit
- `Shift+Enter` -- newline
- `Ctrl+K` -- open command palette
- `Ctrl+L` -- clear current session
- `Ctrl+,` -- open settings
- `Esc` -- close the panel (if it is overlay-mode)
- `Up arrow` (in input) -- edit last sent message

## 2. Command Palette

### Invocation

Global hotkey `Super+Space` opens the command palette. This hotkey is configurable in Settings.

### Layout

- Single text input at the top.
- Live results list below, with icons and keyboard shortcuts.
- Results are grouped: commands, sessions, settings, models, recent plans.

### Behavior

- Fuzzy search across all command types.
- `Enter` executes the highlighted command.
- `Tab` accepts the highlighted suggestion into the input.
- `Esc` closes the palette.
- Commands are context-aware: if a chat is active, "ask model" is the default.

### Built-in Commands

- Ask model
- Generate plan
- Validate plan
- Show receipts
- Show settings
- Toggle privacy mode
- Switch model
- New session
- Open last plan
- Run doctor
- Toggle visible computer-use
- Cancel current task

## 3. Approvals Modal

### Trigger

- ToolBroker decision is `Ask` (user must approve per-step).
- ToolBroker decision is `Deny` (modal explains why and offers an override path if the user wants to escalate).

### Layout

- Title: "Approval required"
- Step ID, action type, risk level, description.
- For file actions: a preview of the file (head + tail) and the path.
- For shell actions: the exact command, no placeholders.
- For browser actions: the URL, the destination host, the policy reason.
- Buttons: "Approve" / "Approve all in this plan" / "Deny" / "Edit" (for plans).
- Optional: a comment field that is saved to the receipt.

### Behavior

- Modal blocks the action until user responds.
- Timeout: 60 seconds by default. After timeout, action is denied.
- The user's choice is logged in the receipt with timestamp, decision, and optional comment.
- "Approve all in this plan" is gated behind a confirmation dialog: "Are you sure? This will approve all future steps in this plan."

## 4. Receipts Timeline

### Layout

- Vertical timeline, newest at top.
- Each entry: timestamp, action, status (Executed / Denied / Failed / Approved / Cancelled), short summary.
- Filter chips at top: All / Chat / Ask / Apply / Tool / Model Router / Computer-Use.
- Search bar: filter by plan_id, action, status, date range.
- Click an entry: opens detail view with the full receipt (sanitized, no prompts).

### Behavior

- The timeline is paginated and lazy-loaded.
- The user can export selected receipts as a JSON file.
- The user can delete selected receipts (irreversible; confirmation required).
- The timeline respects the receipts retention policy from Settings (default: 90 days).

## 5. Model / Runtime Status Indicator

### Placement

System tray (StatusNotifierItem).

### States

| State | Icon | Tooltip |
|---|---|---|
| Loading | spinner | "Loading model..." |
| Ready | green dot | "Model: gemma-4-12B-it-QAT-Q4_0 -- Ready" |
| Error | red dot | "Model error -- click for details" |
| Disabled | gray dot | "Model disabled" |

### Behavior

- Click opens the assistant panel.
- Right-click opens a context menu: switch model, run doctor, open receipts.

## 6. Privacy Indicator

### Placement

System tray (StatusNotifierItem).

### States

| Mode | Icon | Tooltip |
|---|---|---|
| `local_only` | shield, no cloud | "Local only -- last cloud use: never" |
| `cloud_fallback` | shield with cloud dot | "Cloud fallback -- last cloud use: 2h ago" |
| `cloud_only` | cloud with arrow | "Cloud only" |

### Behavior

- Click opens the assistant panel on the privacy tab.
- Right-click opens a context menu: change privacy mode, view cloud usage receipts.

## 7. Settings Panel

### Tabs

- General (theme, panel position, hotkeys)
- Models (default model, fallback model, alias mapping)
- Privacy (default mode, cloud credentials, telemetry)
- Allowed Roots (list of directories, add/remove, default write location)
- API Token (show, rotate, copy)
- Computer-Use (visible/hidden mode defaults, cancellation policy, screenshot retention)
- Voice (future: input device, push-to-talk hotkey)
- Updates (channel: stable / beta, automatic updates on/off)
- About (version, build, licenses, links to docs)

### Behavior

- All settings are stored in `~/.config/osai/settings.toml` (per user) and `/etc/osai/settings.toml` (system).
- Changes are written immediately and reloaded by the relevant services.
- The user can revert any setting to the system default.

## 8. Sessions / History

### Layout

- Sidebar with a list of sessions, each with a name, last-modified timestamp, and short preview.
- Main view: the selected session's full message history.
- Top-right: rename, delete, export buttons.

### Behavior

- Sessions are stored in `~/.local/share/osai/sessions/<session_id>/`.
- A session is a folder containing `messages.json`, `metadata.toml`, and a small `index.sqlite`.
- Sessions are searchable by content (full-text).
- "New session" creates an empty session and opens it.
- "Continue in new session" forks the current session.
- "Delete session" is irreversible; confirmation required.

## 9. File Previews

### Layout

- Inline preview rendered in the assistant panel or approval modal.
- For text files: first 200 lines + last 50 lines, with line numbers.
- For images: thumbnail (max 200x200), expandable.
- For PDFs: first page rendered as image, expandable.
- For binary files: file metadata (size, type, hash) only.

### Behavior

- Previews are generated on demand via `osai-api`.
- Large files (>10 MB) are previewed as metadata only.
- Previews are redacted for sensitive metadata (e.g., EXIF for images is stripped before display).

## 10. Computer-Use Controls

### Visible Mode

- "Start visible session" button in the assistant panel.
- On activation, a small overlay shows what OSAI is doing (screenshot, click, type).
- A "Pause" / "Cancel" button is always visible.
- Sensitive actions pop the approval modal.

### Hidden Mode

- "Start hidden task" in the assistant panel or CLI.
- A task queue view shows all running and completed hidden tasks.
- Each task has: name, status, elapsed time, screenshots count, output size.
- Click a task to see the artifact store (screenshots, summaries, output files).
- "Cancel" stops the task gracefully.
- "Destroy environment" wipes the hidden session's state and resets to a clean snapshot.

## 11. Error States

| Error | UI Behavior |
|---|---|
| `osai-api` unreachable | Tray icon turns red. Panel shows "API unreachable -- retrying" with a manual retry button. |
| Model failed to load | Panel shows "Model unavailable -- using fallback". Fallback is the smoke model. |
| Approval timeout | Modal closes; receipt records the timeout; the action is denied. |
| Cloud use failed | Notification: "Cloud use failed -- check credentials or network". |
| Computer-use cancelled | Notification: "Task cancelled"; artifact store retains intermediate state. |
| Storage full | Notification: "Out of disk space -- some receipts may be lost". |
| Permission denied (Wayland portal) | Modal: "OSAI needs screen capture permission -- grant now?". |

All errors are logged. Errors do not silently fail. Errors do not crash the UI.

## 12. Accessibility and Keyboard Navigation

### Keyboard

- Every action has a keyboard shortcut or is reachable via Tab + Enter.
- Visible focus ring on all interactive elements.
- `Ctrl+K` opens the command palette from anywhere.
- The command palette itself is fully keyboard navigable.

### Screen Reader

- All interactive elements have ARIA labels.
- Status indicators have `aria-live` regions.
- Approvals modal traps focus until resolved.
- Error notifications are announced.

### Localization

- All user-visible strings are externalized.
- English is the default; the OOBE lets the user choose language at first boot.
- RTL languages are supported where the host desktop supports them.

### Theme

- Light, dark, and high-contrast themes.
- Theme follows the host desktop's setting by default.
- The user can override in Settings.

### Reduced Motion

- Panel transitions are short and non-distracting.
- A "reduce motion" toggle in Settings disables non-essential animations.

## Non-Goals (UI)

- Voice input/output UI is out of scope for v1. (See `ROADMAP.md`.)
- In-panel code execution is out of scope.
- A built-in web browser is out of scope. OSAI uses the host browser.
- A built-in terminal is out of scope. OSAI uses the host terminal.
- A plugin system is out of scope for v1.
- Multiple concurrent agent sessions are out of scope for v1.

## Open Questions

- Should the assistant panel be a pop-out or a docked widget? (Open: both; user choice in Settings.)
- Should the command palette support custom user-defined commands? (Open: defer to v2.)
- Should the receipts timeline show the diff between the original plan and the executed plan? (Open: nice to have; defer if scope grows.)
- Should the UI show a "what's running on my GPU right now" panel for transparency? (Open: nice to have; defer if scope grows.)

## Acceptance Criteria

This spec is accepted when:

- A coder can implement every surface listed in this document against `osai-api`.
- A tester can use every keyboard shortcut and confirm it works.
- A tester can trigger every error state and confirm the UI does not crash.
- A tester with a screen reader can use the assistant panel, command palette, approvals modal, and settings.
- The UI does not shell out to `osai-cli` anywhere.
- The UI uses the host desktop's theme by default and respects the user's choice in Settings.
