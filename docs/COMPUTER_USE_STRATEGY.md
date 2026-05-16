# Computer Use Strategy

## Computer Use as a Core OSAI Capability

Computer use is a core planned OSAI capability, not an afterthought. It must be designed safely from the start. The goal is to let OSAI operate computers on behalf of users — both by showing what it's doing (visible mode) and by running tasks in the background (hidden mode).

## Two Modes Defined

### Visible Computer Use

OSAI operates in the same desktop/session the user is actively using. The user can watch actions in real-time, interrupt, and cancel.

**Use cases**:
- Teaching — User learns how to perform a task by watching OSAI do it
- Collaboration — User works alongside OSAI on complex tasks
- Transparency — User sees exactly what OSAI is doing and why
- Approval-on-demand — Sensitive actions require immediate user approval

**Safety**: Visible mode is inherently safer because the user is watching. However, approval must still be required for sensitive categories.

### Hidden/Isolated Computer Use

OSAI operates in a separate isolated environment. The user does not need to watch every intermediate step. OSAI returns final outputs, artifacts, and summaries.

**Use cases**:
- Background task execution — User starts a task and gets notified when complete
- Web research — OSAI browses and returns findings without disturbing user
- Document generation — OSAI processes documents and returns results
- Repetitive UI workflows — OSAI automates repetitive tasks while user works on something else
- App automation — OSAI interacts with applications without user needing to watch

**Implementation options** (mutually exclusive approaches):
1. Nested Wayland compositor (separate seat/weston nested)
2. Virtual display (X11 or Wayland)
3. Containerized desktop (PipeWire + container)
4. VM (full system virtualization)
5. Separate Linux user/session (logind session separation)

**Safety**: Hidden mode is higher risk because actions are not visible. Strong isolation, receipts, approvals, and constraints are essential.

## Why Hidden Mode Is Useful

Users want hidden mode because:
- They can start a long task and check back later
- They can run web research while working
- They don't need to watch repetitive tasks
- Background automation is possible
- Their active desktop is not disturbed

## Why Hidden Mode Is Risky

Hidden mode risks include:
- Invisible credential exposure
- Unintended external communication
- Data exfiltration
- Destructive actions without user seeing
- Account changes without user awareness
- Browser sessions exposing cookies/accounts
- Uncontrolled shell or file access

## Required Safety Design

Every computer-use task must implement these requirements:

### Plan-Based Execution

All computer-use tasks start as Plan DSL:

```yaml
version: '0.1'
id: <uuid>
title: Research competitor pricing
actor: osai-agent
risk: Medium
approval: Ask  # Always ask for hidden mode tasks
steps:
  - id: step-1
    action:
      type: ComputerUseHidden
      task: research_pricing
      parameters:
        urls: ["https://competitor.example.com/pricing"]
        duration_seconds: 300
    requires_approval: true
```

The plan is validated before execution. ToolBroker evaluates the plan against policy.

### User Approval Required

Sensitive categories require explicit user approval:

- Credential entry (passwords, API keys, 2FA)
- Purchases or payments
- Account changes (email, social media, etc.)
- Destructive file operations
- External communications (email, chat, social posting)
- Browser session continuation beyond approved scope

### Strict Policy Enforcement

ToolBroker enforces strict policies on computer-use actions:

- Only approved websites for browser actions
- No arbitrary shell commands
- File access limited to allowed roots
- Network requests limited to whitelisted domains
- No exfiltration of files beyond allowed roots

### No Credential Entry Without User Action

OSAI cannot autofill credentials. User must:
1. Explicitly approve credential entry
2. Manually enter credentials into the isolated environment
3. Or use a system keyring that requires user authentication

### No Purchases/Payments Without Explicit Approval

For any action involving payment:
1. User must explicitly approve the payment action
2. Receipt includes full details of the payment
3. User must confirm purchase amount and recipient
4. No silent continuation after payment

### No Destructive Changes Without Approval

FilesDelete, FilesMove to trash, system configuration changes:
1. User must approve each destructive action
2. Receipt includes what was changed and how
3. Rollback capability must be available if implemented

### Receipt Completeness

Every computer-use action generates a receipt:

```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "ComputerUse",
  "mode": "visible|hidden",
  "status": "Executed",
  "plan_id": "uuid",
  "requested_task": "Research competitor pricing",
  "steps": [
    {
      "id": "step-1",
      "action": "BrowserOpenUrl",
      "url": "https://competitor.example.com/pricing",
      "screenshots_captured": 2,
      "data_extracted": ["pricing_table"],
      "result": "extracted pricing data"
    }
  ],
  "artifacts": [
    {
      "type": "screenshot",
      "path": "/receipts/artifacts/screenshot-001.png",
      "redacted": true
    },
    {
      "type": "text_summary",
      "path": "/receipts/artifacts/summary-001.txt",
      "redacted": false
    }
  ],
  "duration_seconds": 45,
  "outcome": "completed",
  "error": null
}
```

### Screenshot/Artifact Privacy Controls

Screenshots may contain private data:

1. **Minimize capture** — Only capture what is needed for the task
2. **User opt-in for transmission** — Cloud use with screenshots requires explicit opt-in
3. **Redaction** — UI elements, sensitive data in screenshots replaced/redacted before storage
4. **User review** — Outputs must be reviewed before external transmission
5. **Retention controls** — User can delete artifacts at any time

### Task Cancellation

Every computer-use task must be cancellable:

1. User invokes cancel (UI button, keyboard shortcut)
2. OSAI stops further actions
3. Current action completes or gracefully terminates
4. Environment is preserved for debugging
5. Receipt marks task as "cancelled"

### Environment Reset/Destroy

Hidden environments must be resettable:

1. User can destroy the isolated environment
2. Browser state, cookies, local storage are cleared
3. Files accessed in isolated environment are preserved for review
4. New hidden environment can be created fresh

### Output Review Before External Transmission

Hidden mode outputs must be reviewed before sending externally:

1. OSAI returns final outputs/artifacts to user
2. User reviews outputs
3. User approves external transmission
4. No automatic exfiltration of results

## Proposed Components

### Computer-Use Broker

Manages the overall computer-use flow:
- Accepts task request
- Creates plan from task description
- Coordinates with ToolBroker for authorization
- Manages execution lifecycle
- Writes receipts

### Desktop Observation Service

Observes the desktop for computer-use:
- Screenshot capture (visible mode)
- Input simulation (keyboard/mouse)
- Window/element identification
- Screen region capture

### Action Executor

Executes approved actions:
- Browser automation
- Application control
- File operations (within allowed roots)
- Shell commands (sandboxed)

### Screenshot Pipeline

Handles screenshot management:
- Capture
- Privacy redaction
- Storage
- Artifact tracking

### OCR/Vision Pipeline

Processes screenshots:
- Text extraction (OCR)
- Screenshot understanding
- Action planning from visual input

### Browser Automation Bridge

Controls browser:
- Opens URLs (via BrowserOpenUrl tool)
- Fills forms (with user approval for credentials)
- Extracts page content
- Manages browser state

### Virtual Display Manager

Manages hidden environment:
- Creates/destroys isolated display
- Manages resolution and geometry
- Coordinates with compositor

### Artifact Store

Manages outputs:
- Stores screenshots and artifacts
- Tracks redacted vs original
- Provides retrieval for user review

## Initial Non-Goals

The following are explicitly not goals for initial computer-use implementation:

1. **Unsafe unrestricted remote desktop control** — No VNC-like uncontrolled access
2. **Bypassing OS permissions** — OSAI operates within OS security boundaries
3. **Automating sensitive websites without approval** — Banking, email, social media require approval
4. **Always-on invisible agent** — OSAI never operates invisibly without user knowledge
5. **Remote access control** — Computer use is local, not remote
6. **Uncontrolled browser sessions** — Browser must follow strict policy

## Computer Use Receipt Fields

When implemented, receipts must include:

| Field | Description |
|-------|-------------|
| `mode` | `visible` or `hidden` |
| `plan_id` | Reference to the plan |
| `requested_task` | What the user asked for |
| `actions_taken[]` | Per-step log of actions |
| `screenshots_captured` | Count (not pixels, unless needed) |
| `artifacts_created[]` | List of artifacts with redaction status |
| `files_touched[]` | Files accessed |
| `urls_opened[]` | URLs visited |
| `credentials_used[]` | Credential types (not values) |
| `duration_seconds` | Task duration |
| `outcome` | `completed`, `cancelled`, `failed` |
| `isolation_environment_id` | For hidden mode, environment identifier |

## Testing Requirements for Computer Use

### Authorization Tests
- [ ] No action without ToolBroker authorization
- [ ] Denied actions not executed
- [ ] Approval required for Ask/Never levels

### Visible Mode Tests
- [ ] User can see what OSAI is doing
- [ ] User can interrupt/cancel task
- [ ] Receipts include all actions
- [ ] Approval required for sensitive actions
- [ ] No credential entry without explicit user action

### Hidden Mode Tests
- [ ] Hidden environment does not affect active desktop
- [ ] All actions logged in receipts
- [ ] Task can be cancelled
- [ ] Environment can be reset/destroyed
- [ ] Final outputs returned to user
- [ ] No hidden exfiltration

### Security Tests
- [ ] No network access beyond policy
- [ ] No file access beyond allowed roots
- [ ] No credential exfiltration
- [ ] Screenshots protected (redacted, not transmitted without consent)
- [ ] Browser sessions isolated
- [ ] Isolated environment cannot access user's active session

## Threat Model Additions

Computer use adds these threats:

1. **Credential exposure via browser** — Browser in isolated session caches credentials
2. **Screenshot containing sensitive data** — Screenshots may show private information
3. **Hidden actions invisible to user** — User doesn't see what OSAI is doing
4. **Data exfiltration via network** — Isolated session sends data externally
5. **Account changes without user seeing** — Email/social media changes happen in hidden mode
6. **File corruption from hidden operations** — Files modified without user awareness

Mitigations:
- Receipts provide audit trail
- Isolated environment limits access
- Network policies restrict egress
- User approval for sensitive categories
- Screenshots under privacy controls