# Receipts

## What Are Receipts?

Receipts are audit records for every OSAI action. They provide a complete audit trail of what was attempted, what was authorized, what was executed, and what the outcome was.

Receipts are generated for:
- Chat requests
- Ask/plan generation requests
- Plan authorization decisions
- Plan apply executions
- Individual tool executions
- Model Router calls

## Receipt Purpose

1. **Audit** — Know what OSAI did, when, and with what result
2. **Debugging** — Reconstruct what happened when something goes wrong
3. **Security** — Verify that unauthorized actions were denied
4. **Privacy** — Prove that secrets were not stored
5. **User review** — Allow user to inspect OSAI's actions

## Current Receipt Use

### Chat Receipts
```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "ModelChat",
  "status": "Executed",
  "model": "gemma-4-E2B-it-Q8_0.gguf",
  "prompt_length": 42,
  "response_length": 128,
  "finish_reason": "stop",
  "receipts_dir": "~/.local/share/osai/receipts/chat"
}
```

Note: `prompt_length` stored, not full prompt. This is intentional.

### Ask/Plan Generation Receipts
```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "Ask",
  "status": "Executed",
  "model": "gemma-4-E2B-it-Q8_0.gguf",
  "output_path": "/path/to/generated-plan.yml",
  "validation": "valid",
  "request_type": "natural_language_plan",
  "receipts_dir": "~/.local/share/osai/receipts/ask"
}
```

### Apply Receipts
```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "PlanApply",
  "status": "Executed",
  "plan_id": "uuid",
  "plan_path": "/path/to/plan.yml",
  "steps": [
    {
      "id": "step-1",
      "action": "FilesList",
      "decision": "Allow",
      "mode": "Allow",
      "status": "Executed"
    }
  ],
  "summary": {
    "allowed": 1,
    "denied": 0,
    "approval_required": 0
  },
  "receipts_dir": "~/.local/share/osai/receipts/apply"
}
```

### Tool Execution Receipts
```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "FilesList",
  "status": "Executed",
  "path": "~/Downloads",
  "result": {
    "files": ["file1.txt", "file2.pdf"],
    "count": 2
  },
  "receipts_dir": "/tmp/osai-tool-receipts"
}
```

### Model Router Receipts
```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "ModelRouter",
  "provider": "llamacpp",
  "model": "gemma-4-E2B-it-Q8_0.gguf",
  "finish_reason": "stop",
  "tokens_used": 170,
  "latency_ms": 2340,
  "receipts_dir": "~/.local/share/osai/receipts/model-router"
}
```

## What Receipts Must Not Contain

Receipts must never contain:

- **Full prompts** — Prompt text itself is not stored (only length)
- **API keys or tokens** — Any credential
- **Passwords** — User credentials
- **File contents** — Data read from files
- **Screenshot pixel data** — Without privacy controls
- **Cookie or session data** — Browser session information
- **SSH keys or certificates** — Authentication credentials

This is enforced by:
- Secret redaction in ReceiptLogger
- `test_receipts_secret_redaction()` test
- Prompt minimization (store length, not content)

## Secret Redaction

The `redact_secrets()` function replaces known secret fields with `[REDACTED]`:

```rust
fn redact_secrets(value: serde_json::Value) -> serde_json::Value {
    let secret_fields = ["api_key", "token", "password", "secret", "credential"];
    // Replace matching key-values with [REDACTED]
}
```

Tested:
```rust
#[test]
fn test_receipts_secret_redaction() {
    let mut data = serde_json::Map::new();
    data.insert("api_key".to_string(), serde_json::json!("secret123"));
    data.insert("token".to_string(), serde_json::json!("mytoken"));
    data.insert("password".to_string(), serde_json::json!("hunter2"));
    data.insert("action".to_string(), serde_json::json!("test"));

    let redacted = receipts::redact_secrets(&serde_json::Value::Object(data));

    assert_eq!(redacted.get("api_key").unwrap(), "[REDACTED]");
    assert_eq!(redacted.get("token").unwrap(), "[REDACTED]");
    assert_eq!(redacted.get("password").unwrap(), "[REDACTED]");
    assert_eq!(redacted.get("action").unwrap(), "test");
}
```

## Receipt Storage

Default locations:
```
~/.local/share/osai/receipts/
├── chat/              # Chat receipts
├── ask/               # Ask receipts
├── apply/             # Apply receipts
├── model-router/      # Model Router receipts
└── tool/              # Tool execution receipts (if configured)
```

Custom locations via environment variables:
- `OSAI_LOCAL_TOOL_RECEIPTS_DIR`
- `OSAI_LOCAL_MODEL_RECEIPTS_DIR`

## Future Receipt Viewer (UI)

A future Receipt Viewer in the desktop UI will:
- List receipts by timestamp
- Filter by action type
- Filter by status
- Search by plan ID
- Read individual receipt details
- Show what was attempted vs executed
- Provide user-friendly error explanations

## Future Computer Use Receipts

When computer-use is implemented, receipts will include:

```json
{
  "id": "uuid",
  "timestamp": 1234567890,
  "action": "ComputerUse",
  "mode": "visible|hidden",
  "status": "Executed",
  "plan_id": "uuid",
  "requested_task": "Summarize my downloads folder",
  "steps": [...],
  "approvals": [
    {"step": "step-3", "action": "BrowserOpenUrl", "approved": true}
  ],
  "actions_taken": [
    {"step": "step-1", "action": "FilesList", "result": "..."},
    {"step": "step-2", "action": "ModelChat", "result": "..."},
    {"step": "step-3", "action": "BrowserOpenUrl", "url": "..."}
  ],
  "artifacts_created": [
    {"type": "screenshot", "path": "...", "redacted": true},
    {"type": "text_summary", "path": "..."}
  ],
  "files_touched": ["~/Downloads/report.pdf"],
  "urls_opened": ["https://..."],
  "screenshots_captured": 3,
  "outcome": "completed",
  "duration_seconds": 45,
  "model_used": "gemma-4-E2B-it-Q8_0.gguf",
  "receipts_dir": "..."
}
```

### Computer Use Receipt Fields

| Field | Description |
|-------|-------------|
| `mode` | `visible` (user watching) or `hidden` (isolated) |
| `requested_task` | What the user asked for |
| `actions_taken` | Detailed per-step log |
| `artifacts_created` | Screenshots, summaries, other outputs |
| `files_touched` | Which files were accessed |
| `urls_opened` | Which URLs were visited |
| `screenshots_captured` | Count (pixels not stored without consent) |
| `redacted` | Whether artifacts were privacy-redacted |
| `duration_seconds` | How long the task took |
| `outcome` | `completed`, `cancelled`, `failed` |

## Secret Leak Testing

After any change that touches receipts, run the secret scan:

```bash
grep -R -i "password\|api_key\|token\|secret\|credential\|Reply with exactly\|Create a safe plan" \
  ~/.local/share/osai/receipts/*/ \
  /tmp/osai-*/receipts/*/ \
  2>/dev/null || echo "No secrets or prompt text found"
```

If secrets are found:
1. Fix the redaction logic
2. Add test for the redaction case
3. Re-run scan
4. Do not proceed until clean

## Receipt Format Evolution

As OSAI evolves, receipt formats may change. Requirements:
- New fields must be backward compatible or versioned
- Old receipts should remain readable
- Version field in receipt header
- No breaking changes without migration path