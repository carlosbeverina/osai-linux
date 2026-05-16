# UI and Desktop Strategy

## The Goal: Desktop Integration, Not a Web App

The current `osai-api` Dev Panel (`/ui`) is a **development tool**, not the final product UI. The goal is OSAI as an integrated Linux desktop experience where the assistant is part of the session, not a separate tab.

**Why not a web app?**
- Web apps cannot access native desktop features (notifications, keybindings, file dialogs)
- Web apps run in a sandbox separate from the desktop
- Users would need to open a browser tab to use OSAI — not integrated
- Computer use requires native compositor access (Wayland/X11 screenshot capture, input simulation)
- Voice push-to-talk works better with native audio integration

**Why a desktop integration?**
- Assistant feels like part of the OS
- Global hotkeys to invoke the assistant
- Native notifications for approvals and completions
- Access to desktop state for computer use
- Plasmoid/extension model for KDE/GNOME

## Current Web UI Purpose

The Dev Panel exists for:
1. **Development and debugging** — Test API endpoints during development
2. **Manual testing** — Validate MVP loop without CLI
3. **Demonstration** — Show osai-api capabilities
4. **Fallback UI** — If desktop integration is not ready

The Dev Panel is intentionally limited:
- Token stored in memory only (no localStorage)
- No persistent sessions
- No native notifications
- No desktop integration
- No computer use

## Desktop Integration Options

### COSMIC (System76)

**Recommendation for long-term**

| Aspect | Details |
|--------|---------|
| Language | Rust |
| Platform | Wayland |
| Model | Modular, component-based |
| Status | In active development by System76 |

**Why strategic**:
- Same language as OSAI (Rust)
- Wayland-native (modern, secure)
- Modular architecture (easy to extend)
- System76 is actively developing it
- Aligns with OSAI's design principles

**When to consider**:
- After osai-agent-core and osai-api are stable
- If COSMIC reaches feature parity with GNOME/KDE
- As primary desktop integration target

### KDE Plasma

**Recommendation for prototyping**

| Aspect | Details |
|--------|---------|
| Language | C++ / QML |
| Platform | Qt, Wayland/X11 |
| Model | Plasmoid extensions |
| Status | Stable, mature |

**Why practical**:
- Mature plasmoid system
- QML is easier to prototype with
- Works on many distributions
- Can prototype OSAI integration quickly

**When to consider**:
- Short-term prototype after osai-api is ready
- KDE is already in use (elementary OS can switch to KDE)

### Cinnamon

| Aspect | Details |
|--------|---------|
| Language | C / Vala |
| Platform | X11 |
| Model | Nemo extensions |
| Status | Stable, smaller community |

**When to consider**:
- If GNOME proves too fragile for extensions
- If Cinnamon has better AI extension support in the future

### GNOME

| Aspect | Details |
|--------|---------|
| Language | C / JavaScript |
| Platform | Wayland/X11 |
| Model | GJS extensions |
| Status | Powerful but extensions break with releases |

**Concerns**:
- Extension API changes can break OSAI integration
- JavaScript for extensions (not Rust)
- Less modular than COSMIC or KDE

### Xfce

| Aspect | Details |
|--------|---------|
| Language | C |
| Platform | X11 |
| Model | Desktop plugins |
| Status | Stable but less AI-OS oriented |

**When to consider**:
- If very lightweight OSAI is needed
- Low-resource systems

## Recommended Development Order

1. **Build osai-agent-core** — Core logic extraction (Phase 1)
2. **Build osai-api** — HTTP API on top of core (Phase 2)
3. **Dev Panel UI** — Development control panel using osai-api (Phase 3)
4. **Desktop prototype** — KDE/QML or GNOME prototype with osai-api (Phase 4)
5. **COSMIC integration** — If/when COSMIC is production-ready (Phase 4+)

## Target UI Features

Regardless of desktop platform, the OSAI UI should include:

### Assistant Panel
- Docked or floating assistant panel
- Natural language input
- Markdown response rendering
- Code syntax highlighting
- File and plan attachments

### Command Palette
- Global hotkey invocation (e.g., Super+Space)
- Fuzzy search for commands
- Quick access to plans, receipts, settings
- Keyboard-navigable

### Approvals Modal
- Clear display of action to be performed
- Risk level indicator
- Approve/Deny buttons
- "Approve all" option for plan steps
- Timeout for auto-denial

### Receipts Timeline
- Chronological list of OSAI actions
- Filter by action type
- Filter by status (Executed, Denied, Failed)
- Click to read details
- Search

### Model/Runtime Status
- Model currently in use
- GPU memory usage
- llama.cpp status
- Model Router status
- Network status (local/cloud)

### Privacy Indicator
- Current privacy mode (local_only, cloud_fallback, cloud_only)
- Last cloud usage timestamp
- Toggle for privacy mode

### File Action Previews
- Preview files before write/move
- Confirmation with file content preview
- Destructive action warnings

### Settings Panel
- Model selection
- Privacy mode
- Allowed roots configuration
- API token management
- Receipts retention
- Keyboard shortcuts

### Sessions/History
- Chat history per session
- Session naming
- Delete sessions
- Export session

### Voice Controls (Future)
- Push-to-talk activation
- Voice input → text
- Audio notification for completion

### Computer Use Controls (Future)
- Visible mode toggle
- Hidden mode task list
- Task cancellation
- Artifact viewer

## Architecture Constraint

**Critical**: UI must call osai-api, which calls osai-agent-core, which uses ToolBroker/ToolExecutor. UI must never bypass these layers.

```
UI → osai-api → osai-agent-core → ToolBroker → ToolExecutor → ReceiptLogger
```

What this means:
- No UI shell-out to CLI
- No direct ToolBroker calls from UI
- No bypassing auth/token checks
- No writing receipts directly from UI

## UI Security Considerations

1. **Token storage** — In-memory only, not localStorage/sessionStorage
2. **No secrets in UI state** — Tokens cleared on page reload
3. **401 handling** — Show clear error, no retry loops
4. **Approval confirmation** — User confirms before sensitive actions
5. **Receipt access** — UI reads receipts, does not write (except via osai-api)

## Non-Goals (Current Phase)

- Full desktop integration is Phase 4+
- Voice pipeline is Phase 7
- Computer use UI is Phase 6
- Multiple concurrent agent sessions is future work
- Agent marketplace is future work