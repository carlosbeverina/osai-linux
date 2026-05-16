# Tooling and Policies

## Plan DSL Actions

The Plan DSL defines a set of action types that ToolBroker evaluates and ToolExecutor executes.

### Current Action Types

| Action | Status | Description |
|--------|--------|-------------|
| `FilesList` | ✅ Safe | List directory contents |
| `FilesMove` | ⚠️ Needs review | Move files (requires approval) |
| `FilesWrite` | ⚠️ Needs review | Write files (requires approval) |
| `FilesDelete` | ⚠️ Needs review | Delete files (requires approval + rollback) |
| `ModelChat` | ✅ Safe | Chat with model |
| `ReceiptCreate` | ✅ Safe | Create receipt |
| `DesktopNotify` | ✅ Safe | Send desktop notification |
| `BrowserOpenUrl` | ⚠️ Needs review | Open URL in browser (requires approval) |
| `ShellRunSandboxed` | ⚠️ Sandboxed | Run single command (requires approval) |
| `Custom` | ⚠️ Per-case | Custom action type (requires security review) |

### Future Action Types (Phase 6+)

| Action | Status | Description |
|--------|--------|-------------|
| `ComputerUseVisible` | 🔜 Planned | Visible computer use |
| `ComputerUseHidden` | 🔜 Planned | Hidden/isolated computer use |
| `VoiceInput` | 🔜 Planned | Voice pipeline |
| `VisionInput` | 🔜 Planned | Screenshot understanding |
| `MemoryRead` | 🔜 Planned | Agent memory read |
| `MemoryWrite` | 🔜 Planned | Agent memory write |
| `SystemInfo` | 🔜 Planned | Hardware/system queries |

## ToolBroker Authorization

ToolBroker evaluates every action against the active policy.

### Policy Structure

```yaml
rules:
  - match:
      action: FilesList
    decision: Allow  # Always allowed
  - match:
      action: ShellRunSandboxed
    decision: Ask   # Always ask
  - match:
      action: FilesWrite
    decision: Deny  # Not yet safe
```

### Decision Types

| Decision | Behavior |
|----------|----------|
| `Allow` | Execute without asking |
| `Ask` | Execute only after user approval |
| `Deny` | Do not execute, log as denied |

### Per-Step Authorization

Each step in a plan is evaluated individually:

```rust
struct AuthorizationDecision {
    step_id: String,
    action: ActionType,
    decision: Decision,
    reason: String,
    requires_approval: bool,
    mode: AuthorizationMode,  // "Allow", "Ask", "Deny"
}
```

## Policy Files

Policies are YAML files that define authorization rules.

### Default Policy

`examples/policies/default-secure.yml`:

```yaml
name: Default Secure Policy
description: Secure-by-default policy for OSAI

rules:
  - match:
      action: FilesList
    decision: Allow

  - match:
      action: ModelChat
    decision: Allow

  - match:
      action: ReceiptCreate
    decision: Allow

  - match:
      action: DesktopNotify
    decision: Allow

  - match:
      action: ShellRunSandboxed
    decision: Ask

  - match:
      action: FilesWrite
    decision: Deny

  - match:
      action: FilesMove
    decision: Deny

  - match:
      action: FilesDelete
    decision: Deny

  - match:
      action: BrowserOpenUrl
    decision: Ask
```

### Policy Evaluation

When applying a plan:

```rust
fn authorize_plan(
    plan: &OsaiPlan,
    policy: &Policy,
    allowed_roots: &[PathBuf],
) -> Result<Vec<AuthorizationDecision>>
```

Each step is evaluated:
1. Action type matched against rules
2. Path constraints checked (for file operations)
3. Decision determined
4. If `Ask`, `requires_approval` is set

## ToolExecutor Execution

ToolExecutor executes authorized actions. It respects approvals and only executes what ToolBroker allowed.

### Execution Flow

```
Plan with approvals
    │
    ▼
ToolBroker.authorize()
    │
    ▼
For each step:
    │ If denied → skip
    │ If requires_approval → wait for approval
    │ If allowed → execute
    │
    ▼
ToolExecutor.execute(step)
    │
    ▼
ReceiptLogger.write()
```

### Execution Modes

| Mode | Behavior |
|------|----------|
| `DryRun` | Validate and log, do not execute |
| `Real` | Execute and log |

### FilesList Execution

```rust
async fn execute_files_list(path: &Path) -> Result<FilesListResult> {
    // Validate path is within allowed roots
    // Read directory
    // Return file list
}
```

### FilesWrite Execution (Requires Review)

FilesWrite is currently denied by default policy. Before enabling real execution:
1. Security review of path traversal risks
2. Implementation of rollback capability
3. Test coverage for edge cases
4. User notification of file write

### ShellRunSandboxed Execution

ShellRunSandboxed executes a single command with fixed arguments:

**Allowed**:
```rust
action:
  type: ShellRunSandboxed
  command: "ls"
  args: ["-la", "~/Downloads"]
```

**Not Allowed** (shell features stripped):
```rust
action:
  type: ShellRunSandboxed
  command: "ls -la ~/Downloads"  // Single string not supported
```

Shell features not supported:
- Pipes (`|`)
- Redirects (`>`, `<`)
- Background (`&`)
- Variable expansion (`$VAR`)
- Glob (`*.txt`)
- Command chaining (`;`, `&&`, `||`)

## Allowed Roots

File operations are scoped to allowed roots:

```yaml
allowed_roots:
  - ~/Downloads
  - ~/Documents
```

Operations outside allowed roots are denied by ToolBroker.

### Default Allowed Roots

- `~/Downloads` — Default write location
- `~/Documents` — Default document location

### Adding Allowed Roots

```bash
cargo run -p osai-agent-cli -- apply plan.yml \
  --allowed-root ~/Projects \
  --allowed-root ~/work
```

## Destructive Tools Requiring Security Review

### FilesWrite

**Risk**: Overwrites existing files, could corrupt data

**Required before enabling**:
- Rollback capability
- Backup before write
- User confirmation dialog
- Path traversal prevention
- Test coverage

### FilesMove

**Risk**: Could move files to wrong location

**Required before enabling**:
- Destination validation
- User confirmation
- Rollback capability
- No overwriting existing files without confirmation

### FilesDelete

**Risk**: Permanent data loss

**Required before enabling**:
- Trash instead of delete (configurable)
- Rollback capability
- User confirmation
- No deleting system files
- Audit trail

### BrowserOpenUrl

**Risk**: Could open malicious websites

**Required before enabling**:
- URL validation against allowlist
- No autofill of credentials
- Browser state isolation
- Network policy enforcement

## Approval Workflow

```
User request → Plan generation → Authorization → Approval → Execution
                    │                  │              │
                    │                  │              └── User must approve "Ask" steps
                    │                  │
                    │                  └── ToolBroker marks "Ask" steps
                    │
                    └── Model generates plan
```

### Manual Approval

```bash
cargo run -p osai-agent-cli -- apply plan.yml \
  --approve step-1 \
  --approve step-2
```

### Approve All

```bash
cargo run -p osai-agent-cli -- apply plan.yml --approve-all
```

## Custom Actions

Custom actions allow extensibility but require security review:

```yaml
steps:
  - id: step-1
    action:
      type: Custom
      name: custom_email_action
      inputs:
        to: user@example.com
        subject: Report
    requires_approval: true
```

Custom actions must:
1. Be defined in ToolExecutor
2. Pass ToolBroker authorization
3. Have security review before use
4. Produce valid receipts