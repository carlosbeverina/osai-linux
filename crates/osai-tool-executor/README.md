# OSAI Tool Executor

Safe execution layer for already-authorized tool requests.

## What is ToolExecutor?

ToolExecutor is a sandboxed execution layer that takes authorization decisions from ToolBroker and executes only the safest allowed actions. It provides a critical separation between "should this action be allowed?" (authorization) and "how do I safely execute this action?" (execution).

## Why Separate Execution from Authorization?

Authorization (ToolBroker) decides **what is permitted** based on policy. Execution (ToolExecutor) decides **how to safely run** permitted actions. This separation provides:

1. **Defense in depth** — Even if authorization is bypassed, execution restrictions protect the system
2. **Policy enforcement** — Execution layer independently validates paths and actions
3. **Audit completeness** — Every execution attempt is logged regardless of outcome
4. **Safe by default** — Unknown or dangerous actions are refused by default

## What v0.1 Supports

In this initial version, ToolExecutor supports only read-only, safe actions:

| Action | Behavior | Output |
|--------|----------|--------|
| `FilesList` | Validates path is within allowed_roots | Array of entries (empty in v0.1) |
| `DesktopNotify` | Returns simulated notification | `{"simulated": true, "title": "...", "body": "..."}` |
| `ModelChat` | Returns simulated response | `{"simulated": true, "message": "..."}` |

## What v0.1 Refuses

All mutation and potentially dangerous actions are refused:

- `FilesWrite` — Not executable (would modify filesystem)
- `FilesMove` — Not executable (would modify filesystem)
- `FilesDelete` — Not executable (would modify filesystem)
- `ShellRunSandboxed` — Not executable (no shell in v0.1)
- `BrowserOpenUrl` — Not executable (no browser in v0.1)
- `Custom` — Not executable (unknown behavior)

Any denied, approval-required, or unsupported action returns `ExecutionStatus::Skipped` or `ExecutionStatus::Failed`.

## Receipt Creation

Every execution attempt creates a receipt:

| Decision Type | Receipt Status | Notes |
|--------------|----------------|-------|
| Allowed + Approved | `Executed` | output present |
| Denied | `Failed` | error with reason |
| Approval Required | `Planned` | skipped |
| Unsupported Action | `Failed` | error explains why |

Inputs are redacted in receipts. Keys containing `key`, `token`, `secret`, `password`, or `credential` have their values replaced with `[REDACTED]`.

## Why Shell and Filesystem Mutations Are Disabled in v0.1

v0.1 is intentionally limited to demonstrate the architecture without introducing real risks:

1. **No shell execution** — Prevents command injection attacks
2. **No filesystem mutation** — Prevents data corruption or destruction
3. **Path restrictions** — Even `FilesList` is constrained to allowed_roots
4. **Simulated outputs** — Safe demonstration of action types

Future versions will add controlled filesystem operations with proper sandboxing.

## Usage

```rust
use osai_tool_executor::{ExecutionStatus, ToolExecutor};
use osai_toolbroker::{AuthorizationDecision, PolicyMode, ToolRequest};
use osai_plan_dsl::ActionKind;
use osai_receipt_logger::ReceiptStore;
use std::collections::BTreeMap;

// Create executor with allowed paths
let store = ReceiptStore::new("/var/lib/osai/receipts");
store.ensure_dirs().unwrap();

let executor = ToolExecutor::new(
    store,
    vec![std::path::PathBuf::from("/home/user")],
);

// Create authorized request
let request = ToolRequest::new("agent", ActionKind::DesktopNotify, "Test")
    .with_inputs(serde_json::json!({"title": "Hello", "body": "World"}));

// Execute with authorization decision
let decision = AuthorizationDecision {
    request_id: request.id,
    allowed: true,
    requires_user_approval: false,
    reason: "Allowed by policy".to_string(),
    policy_mode: PolicyMode::Allow,
};

let result = executor.execute_authorized(&request, &decision).unwrap();

match result.status {
    ExecutionStatus::Executed => println!("Action executed: {:?}", result.output),
    ExecutionStatus::Skipped => println!("Action skipped: {}", result.error.unwrap()),
    ExecutionStatus::Failed => println!("Action failed: {}", result.error.unwrap()),
}
```

## Flow

```
Plan DSL
  → ToolBroker.authorize() → AuthorizationDecision
  → ToolExecutor.execute_authorized() → ExecutionResult
  → Receipt written to ReceiptStore
```
