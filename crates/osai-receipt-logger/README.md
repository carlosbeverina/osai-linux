# OSAI Receipt Logger

A reliable local receipt system for auditable AI actions.

## What are OSAI Receipts?

OSAI receipts are immutable records that capture every AI-mediated action in the system. Each receipt documents what action was performed, by whom, when, with what inputs and outputs, and what the outcome was. Receipts are stored as JSON files and provide a complete audit trail.

## Why Every Action Needs an Auditable Receipt

AI systems can make mistakes, misunderstand intent, or be exploited through prompt injection. Receipts provide:

1. **Accountability** — Every action is traceable to an actor and timestamp
2. **Auditability** — Security teams can review what the AI did
3. **Reproducibility** — Investigators can replay what happened
4. **Rollback support** — Failed actions can be traced and reversed
5. **Compliance** — Immutable records satisfy security and regulatory requirements
6. **Least surprise** — Users can review AI actions before they are executed

Without receipts, AI actions are invisible and unauditable. Receipts transform "AI said it did something" into verifiable proof.

## Receipt Lifecycle

```
┌─────────┐     ┌──────────┐
│ Planned │────▶│ Approved │
└─────────┘     └──────────┘
     │               │
     ▼               ▼
┌──────────┐   ┌──────────┐
│  Denied  │   │ Executed │
└──────────┘   └──────────┘
                     │
                     ▼
              ┌──────────┐
              │ Failed  │──▶ (action error recorded)
              └──────────┘

              ┌──────────┐
              │ RolledBack│
              └──────────┘
```

1. **Planned** — Action is planned but not yet submitted for approval
2. **Approved** — User or policy approved the action
3. **Denied** — User or policy denied the action
4. **Executed** — Action completed successfully
5. **Failed** — Action failed during execution (error field required)
6. **RolledBack** — Action was successfully rolled back

## Receipt Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | UUID | Unique identifier for this receipt |
| `timestamp` | DateTime | When the receipt was created (UTC) |
| `actor` | String | Who initiated the action |
| `action` | String | What action was performed |
| `tool` | Option\<String\> | Which tool executed the action |
| `plan_id` | Option\<UUID\> | Associated plan this belongs to |
| `risk` | String | Risk level (Low, Medium, High, Critical) |
| `approval` | String | Approval mode used (Auto, Ask, AlwaysDeny) |
| `status` | ReceiptStatus | Current state in the lifecycle |
| `inputs_redacted` | JSON Value | Sanitized input parameters |
| `outputs_redacted` | Option\<JSON Value\> | Sanitized output results |
| `error` | Option\<String\> | Error message if failed |
| `metadata` | BTreeMap | Additional contextual data |

## Why Redacted Inputs and Outputs?

**Inputs and outputs must never contain secrets.**

AI systems often handle sensitive data: API keys, passwords, personal information, file contents. Receipts are stored as plain JSON files that may be readable by system administrators, security scanners, and audit tools.

For this reason:
- `inputs_redacted` should contain only **sanitized parameter names and structures**
- `outputs_redacted` should contain only **result summaries, not raw data**
- Replace sensitive values with `***REDACTED***` or counts/sizes

**Correct example:**
```json
{
  "inputs_redacted": {
    "path": "/home/user/documents",
    "content": "***REDACTED***"
  }
}
```

**Never store:**
```json
{
  "inputs_redacted": {
    "api_key": "sk-1234567890abcdef",
    "password": "my-secret-password"
  }
}
```

## ReceiptStore: Writing, Reading, and Listing

### Writing Receipts

```rust
let store = ReceiptStore::new("/var/lib/osai/receipts");
store.ensure_dirs()?;

// Receipt is validated before writing
let path = store.write(&receipt)?;
```

When writing:
1. The receipt is validated (all required fields present, status constraints met)
2. A filename is generated: `<timestamp>-<uuid>.json`
3. The receipt is serialized to JSON
4. The file is written atomically (validation happens first)
5. **Existing receipts are never overwritten** — write fails if file exists

### Reading Receipts

```rust
// Read by UUID (searches across all files in root directory)
let loaded = store.read(receipt.id)?;
```

When reading:
1. All JSON files in the root directory are scanned
2. Each file is parsed and checked for matching UUID
3. First match is returned
4. `NotFound` error if no match exists

### Listing Receipts

```rust
// Returns paths sorted ascending by filename (timestamp order)
let paths = store.list()?;
```

When listing:
1. All `.json` files in the root directory are found
2. Paths are sorted ascending by filename
3. Full paths are returned, not receipt contents

### Storage Format

- **Directory:** configurable root_dir (e.g., `/var/lib/osai/receipts`)
- **Filename:** `<timestamp>-<uuid>.json`
- **Timestamp format:** `YYYYMMDDTHHMMSS.ffffffZ` (UTC, filename-safe)
- **Contents:** UTF-8 JSON with pretty printing

## Valid JSON Example

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2026-04-25T10:30:00.123456Z",
  "actor": "osai-agent",
  "action": "FilesWrite",
  "tool": "ToolBroker",
  "plan_id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
  "risk": "Medium",
  "approval": "Ask",
  "status": "Executed",
  "inputs_redacted": {
    "path": "/home/user/project/README.md",
    "content": "***REDACTED***"
  },
  "outputs_redacted": {
    "bytes_written": 1024
  },
  "error": null,
  "metadata": {
    "model": "gemma-4-e4b",
    "session_id": "abc123"
  }
}
```

This receipt is **valid** because:
- `actor`, `action`, `risk`, `approval` are all non-empty
- `status` is `Executed` and `outputs_redacted` is present
- `status` is not `Failed` so `error` being null is acceptable

## Invalid JSON Example

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": "2026-04-25T10:30:00.123456Z",
  "actor": "osai-agent",
  "action": "FilesWrite",
  "tool": "ToolBroker",
  "plan_id": "7c9e6679-7425-40de-944b-e07fc1f90ae7",
  "risk": "",
  "approval": "Ask",
  "status": "Executed",
  "inputs_redacted": {
    "path": "/home/user/project/README.md"
  },
  "outputs_redacted": null,
  "error": null,
  "metadata": {}
}
```

This receipt **fails validation** because:

1. **`risk` is empty** — validation rule requires `risk` must not be empty
2. **`status` is `Executed` but `outputs_redacted` is null** — validation rule requires `outputs_redacted` should be present when status is `Executed` or `RolledBack`

## Flow

```
Plan DSL
  → validation
  → user approval
  → ToolBroker execution
  → ReceiptLogger.write()
  → Receipt stored as JSON
  → Receipt returned to caller
```
