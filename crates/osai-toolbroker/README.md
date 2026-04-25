# OSAI ToolBroker

Authorization layer for AI tool execution with auditable receipts.

## What is ToolBroker?

ToolBroker is the gatekeeper that sits between AI agents and tool execution. Every tool request must pass through ToolBroker, which:

1. **Validates the request** against configured policy
2. **Makes an authorization decision** — allow, deny, or require approval
3. **Creates an audit receipt** — immutable record of the decision

## Why Can't Agents Call Tools Directly?

Direct tool execution would be unsafe because:

1. **No policy enforcement** — agents could execute any action without checks
2. **No audit trail** — actions would be invisible to security teams
3. **No approval flow** — destructive actions could execute immediately
4. **No accountability** — impossible to trace what the AI did

ToolBroker ensures every action is:
- **Authorized** — policy controls what actions are permitted
- **Audited** — every decision produces a receipt
- **Accountable** — receipts link actions to actors and timestamps

## Policy Modes

ToolBroker supports three policy modes per action:

| Mode | Meaning |
|------|---------|
| `Allow` | Execute immediately without user approval |
| `Ask` | Execute only after user approves |
| `Deny` | Never allow this action |

The default mode applies to actions without explicit configuration.

## Authorization Flow

```
Agent Request
     │
     ▼
ToolBroker.authorize(request)
     │
     ├──▶ Check policy for action type
     │
     ├──▶ ShellRunSandboxed: check network + sandbox constraints
     │
     ├──▶ Determine decision (allow/ask/deny)
     │
     ├──▶ Create Receipt with redacted inputs
     │
     ├──▶ Write Receipt to ReceiptStore
     │
     ▼
AuthorizationDecision
     │
     ├──▶ allowed: bool
     ├──▶ requires_user_approval: bool
     ├──▶ reason: String
     └──▶ policy_mode: PolicyMode
```

## Receipt Creation

Every authorization decision produces a receipt with:

- **actor** — who made the request
- **action** — what action was requested
- **status** — `Approved` (auto-allowed), `Planned` (needs approval), or `Denied`
- **inputs_redacted** — sanitized inputs with secrets replaced by `[REDACTED]`
- **policy_mode** — which policy mode was applied

Secret redaction keys: `key`, `token`, `secret`, `password`, `credential`

## Secure Default Policy

The secure default policy (`ToolPolicy::default_secure()`):

| Action | Default Mode |
|--------|-------------|
| ModelChat | Allow |
| DesktopNotify | Allow |
| MemoryRead | Allow |
| FilesList | Allow |
| FilesRead | Allow |
| MemoryWrite | Ask |
| FilesWrite | Ask |
| FilesMove | Ask |
| FilesDelete | Ask |
| ShellRunSandboxed | Ask |
| BrowserOpenUrl | Ask |
| Custom | Deny |

Additionally:
- `shell_network_allowed: false` — network access requires explicit enable
- `shell_requires_sandbox: true` — shell commands must be sandboxed

## YAML Policy Example

```yaml
default_mode: Ask
action_modes:
  ModelChat: Allow
  DesktopNotify: Allow
  FilesList: Allow
  FilesRead: Allow
  FilesWrite: Ask
  FilesDelete: Deny
  ShellRunSandboxed: Ask
allowed_roots:
  - /home/user/projects
  - /tmp
shell_network_allowed: false
shell_requires_sandbox: true
```

## Authorization Example

```rust
use osai_toolbroker::{ToolBroker, ToolPolicy, ToolRequest};
use osai_plan_dsl::{ActionKind, RiskLevel};
use osai_receipt_logger::ReceiptStore;

// Create store and broker
let store = ReceiptStore::new("/var/lib/osai/receipts");
store.ensure_dirs().unwrap();

let broker = ToolBroker::new(ToolPolicy::default_secure(), store);

// Authorize a request
let request = ToolRequest::new("osai-agent", ActionKind::ModelChat, "Chat with AI");

let decision = broker.authorize(&request).unwrap();

assert!(decision.allowed);
assert!(!decision.requires_user_approval);

// A receipt was automatically written
```

## Shell Sandbox Constraints

ShellRunSandboxed has additional checks:

1. **Network access denied by default** — unless `shell_network_allowed: true` and `inputs.network: true`
2. **Sandbox required by default** — denied unless `inputs.sandbox: true`

Example allowing network in sandbox:
```yaml
shell_network_allowed: true
shell_requires_sandbox: true
```

```rust
let mut inputs = BTreeMap::new();
inputs.insert("command", json!("curl https://api.example.com"));
inputs.insert("network", json!(true));
inputs.insert("sandbox", json!(true));

let request = ToolRequest::new("agent", ActionKind::ShellRunSandboxed, "API call")
    .with_inputs(inputs);
```

## Flow

```
Natural language
  → Plan DSL
  → ToolBroker.authorize()
  → Receipt written
  → AuthorizationDecision returned
  → (if approved) Tool execution
  → (if executed) Executed receipt written
```
