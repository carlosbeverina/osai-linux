# Desktop Shell Integration Design

## Purpose

Defines how OSAI integrates with the Linux desktop shell: the long-term target (COSMIC), the prototype option (KDE/QML), other desktop considerations, global hotkey architecture, assistant panel integration, approval modal integration, notifications, status indicators, Wayland permissions, and the rule that desktop UI must call `osai-api` only.

This is a design document. It does not implement the desktop shell.

## Goals

- Make OSAI feel like part of the desktop, not a separate application.
- Provide a global hotkey to invoke the assistant from anywhere.
- Show native notifications for approvals, completions, and errors.
- Provide a persistent status indicator (model loaded, cloud on/off, hidden task running).
- Support visible and hidden computer-use from the shell.
- Keep the architecture rule: UI calls `osai-api` only. Never shell out to `osai-cli`.

## Non-Goals

- A web app for end-user interaction. (The Dev Panel at `http://127.0.0.1:8090/ui` is for development only.)
- A replacement for the desktop. OSAI integrates into the user's chosen desktop.
- Cross-desktop compatibility. v1 targets one desktop; v2 may add others.
- Custom window management. OSAI uses the host desktop's compositor.

## Long-Term Target: COSMIC

| Aspect | Notes |
|---|---|
| Language | Rust |
| Compositor | Wayland (Smithay-based) |
| Architecture | Modular; every shell component is replaceable |
| Stability | Under active development by System76; alpha/beta quality |
| Alignment with OSAI | Same language (Rust), Wayland-native, modular design |

COSMIC is the **long-term production target** for OSAI desktop integration because:

- Rust aligns with OSAI's Rust workspace and reduces FFI overhead.
- The modular architecture allows OSAI to ship a panel, a daemon, and a status indicator as first-class shell components.
- The Wayland-native compositor supports screen capture and input simulation that computer-use needs.
- System76 is actively shipping COSMIC for their hardware, so the project has long-term investment.

We do not commit to COSMIC for v1 because it is still maturing. The prototype is KDE/QML.

## Prototype: KDE Plasma / QML

| Aspect | Notes |
|---|---|
| Language | C++ / QML |
| Compositor | KWin (Wayland/X11) |
| Architecture | Plasmoids, KRunner, KNotifications |
| Stability | Production quality |
| Alignment with OSAI | Stable, mature, easy to prototype |

KDE Plasma is the **prototype target**. We will build a Plasmoid, a KRunner plugin, and a service that talks to `osai-api` via DBus. This lets us ship a working desktop integration before COSMIC is ready.

## Other Desktop Considerations

### GNOME

| Aspect | Notes |
|---|---|
| Language | C / JavaScript (GJS) |
| Compositor | Mutter |
| Extension model | Fragile; extension API breaks between major releases |
| Recommendation | Skip for v1; revisit if COSMIC does not mature |

### Cinnamon

| Aspect | Notes |
|---|---|
| Language | C / Vala |
| Compositor | Muffin (fork of Mutter) |
| Stability | Stable, smaller community |
| Recommendation | Possible v2 target; same Wayland story as GNOME |

### Xfce

| Aspect | Notes |
|---|---|
| Language | C |
| Compositor | xfwm4 (X11 legacy; Wayland in development) |
| Recommendation | Skip; not aligned with AI-OS ambition |

## Global Hotkey Architecture

A global hotkey wakes the assistant from any application, even full-screen games.

### Design

- A small **OSAI Hotkey Daemon** (Rust) runs as a systemd user service.
- The daemon listens for the configured hotkey via the desktop's hotkey API:
  - KDE: `KGlobalAccel` / DBus.
  - COSMIC: Smithay input + custom registration.
  - GNOME: `org.gnome.Shell` / DBus (D-Bus activation).
- The default hotkey is configurable in `~/.config/osai/settings.toml` (`assistant_hotkey`).
- Suggested defaults:
  - `Super+Space` (OSAI Command Palette) -- mirrors the macOS spotlight convention.
  - `Super+Shift+Space` (visible computer-use mode toggle).
  - `Ctrl+Alt+R` (cancel current task).
- Hotkey conflicts with the host desktop MUST be detected and surfaced to the user.

### Behavior

- On activation, the hotkey daemon sends a `panel.show()` event to the assistant panel.
- The panel transitions from hidden/minimized to visible and focused.
- If the panel is already visible, focus is moved to the input field.
- If the user is in a fullscreen app, the panel appears as an overlay (not fullscreen).
- If a hidden computer-use task is running, the hotkey may also show its progress indicator (configurable).

## Assistant Panel Integration

The assistant panel is the primary OSAI surface in the shell. It is:

- A dockable widget (right or top edge by default).
- Configurable position and size in `~/.config/osai/settings.toml`.
- Always-on-top by default; user can pin or auto-hide.
- Connects to `osai-api` over loopback HTTP using the per-user API token.
- Renders Markdown, code blocks (with syntax highlighting), and inline file previews.

### Components

- **Header**: Model name, runtime status, privacy indicator, settings gear.
- **Input**: Multi-line text input with command completion, file attach, screenshot button (for computer-use).
- **Messages**: Conversation history with role indicators.
- **Approvals inline**: A pending approval renders as a modal card in the panel.
- **Footer**: Token usage for the current session, link to settings and receipts.

## Approval Modal Integration

Sensitive ToolBroker actions require explicit user approval. The approval modal is:

- A native modal dialog (KDE: KNotifications + custom; COSMIC: native modal).
- Triggered by `osai-api` via a DBus signal or HTTP webhook to the shell.
- The modal blocks the action until the user approves or denies.
- For visible computer-use, the modal appears in the active session.
- For hidden computer-use, the modal appears in the user's active session but is clearly labeled "Hidden Task" so the user knows OSAI is asking for approval even if it is running in a separate environment.

### Behavior

- Modal timeout: 60 seconds by default. After timeout, the action is denied and the receipt records "approval timeout: denied".
- Approval is per-step. "Approve all" is allowed but explicitly requires a confirmation.
- The user can attach a comment to an approval/denial for future reference (stored in the receipt).

## Notifications

Native desktop notifications for:

- Approval required (if not in the assistant panel).
- Task complete (visible or hidden).
- Error: model failed to load, network failure, policy denial.
- Health check failure: model runtime, `osai-api`, or Model Router down.
- Update available: new OSAI version, new model version, OS update.

### Design

- Use the host desktop's notification API:
  - KDE: `KNotifications`.
  - COSMIC: cosmic-notifications or notify-rust.
  - GNOME: libnotify.
- Notifications include a click action that opens the relevant UI (panel, settings, receipts viewer).
- Notifications never include sensitive content (no prompts, no file paths from allowed_roots, no model outputs). The notification text is generic; the user clicks through to see details.

## Desktop Status Indicators

Three persistent indicators in the shell:

1. **Model Indicator**: Shows the active model and its status (ready, loading, error).
   - Icon: brain / chip.
   - Tooltip: full model name, quantization, context length.
   - Click: opens the assistant panel.
2. **Privacy Indicator**: Shows the current privacy mode (local / cloud_fallback / cloud_only).
   - Icon: shield with cloud dot or not.
   - Tooltip: explains current mode and last cloud use timestamp.
   - Click: opens privacy settings.
3. **Hidden Task Indicator**: Shows when a hidden computer-use task is running.
   - Icon: a small dot in the panel.
   - Tooltip: task name, elapsed time, cancel button.
   - Click: opens the task detail view.

Indicators are implemented as system tray icons (StatusNotifierItem) so they work across KDE, GNOME, and COSMIC.

## Wayland Permissions

Wayland does not allow applications to read other applications' windows or input events without explicit permission. This affects computer-use.

### Permissions Required

- **Screen capture** (`wlr-screencopy` or `pipewire` portal):
  - Required for visible computer-use.
  - Granted by the user via the XDG Desktop Portal on first use.
  - Stored as a token in the user's secret service.
- **Input simulation** (`wlr-virtual-pointer`, `wlr-virtual-keyboard`):
  - Required for visible computer-use.
  - Granted by the user via a portal that confirms the action.
  - Token stored similarly.
- **Clipboard access**:
  - Required for "paste into another app" workflows.
  - Granted on first use via the clipboard portal.

### Hidden Computer-Use

Hidden computer-use uses a nested Wayland compositor or virtual display (see `COMPUTER_USE_IMPLEMENTATION_SPEC.md`). The hidden compositor does not have access to the user's active session by design.

## Why Desktop UI Must Call `osai-api` Only

This is an architecture rule, not a convenience:

- `osai-api` enforces token auth, loopback binding, and rate limits.
- `osai-api` is the stable interface for future UI work. The CLI may change; the API is the contract.
- The CLI may shell out to OS tools. The API does not. Letting the UI shell out to the CLI would mean double-shell-out, double-process, double-environment, double-errors.
- The CLI is for development, automation, and CI. The UI is for humans. They have different ergonomics and different security models.
- If a UI feature needs a CLI-only feature, the fix is to add the feature to `osai-agent-core` (and expose it via `osai-api`), not to shell out from the UI.

A future architecture diagram:

```
Desktop UI (Plasmoid / COSMIC applet)
    | HTTP loopback, Bearer token
    v
osai-api
    | library call
    v
osai-agent-core
    | library call
    v
ToolBroker / ToolExecutor / ReceiptLogger
```

The UI MUST NOT call `osai-cli` or shell out to anything other than the documented `osai-api` endpoints.

## Open Questions

- Should the assistant panel be one window or multiple (chat / approvals / receipts as tabs)? (Open: tabs for v1; can split later.)
- Should voice input be triggered by a global hotkey (push-to-talk) or a panel button? (Open: hotkey for v1.)
- Should hidden computer-use be initiated from a CLI, the panel, or both? (Open: both; CLI for power users, panel for casual users.)
- How do we handle Wayland compositor differences between KDE, COSMIC, and GNOME? (Open: use the XDG Desktop Portal as a compatibility layer.)

## Acceptance Criteria

This design is accepted when:

- A coder can build a minimal KDE Plasmoid that calls `osai-api` and renders the assistant panel.
- A tester can install the Plasmoid, set a global hotkey, and invoke the assistant from any application.
- A tester can see native notifications when approvals are required.
- A tester can see all three status indicators (model, privacy, hidden task) in the system tray.
- A reviewer confirms the UI does not shell out to `osai-cli` anywhere.
