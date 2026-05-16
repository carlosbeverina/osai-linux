# Privacy Model

## Local-First Privacy

OSAI is designed to keep data on the user's machine by default:

- **Local model inference** — No data sent to external servers for inference
- **Local receipts** — Audit logs stored locally under user control
- **Local plans** — Generated plans stored locally
- **Local-only mode default** — Cloud is explicit opt-in, not default

Privacy is the default, not an option. Cloud use requires explicit user action and policy configuration.

## privacy Modes

### local_only (Default)

```yaml
metadata:
  privacy: local_only
```

When `local_only` is set:
- Model Router routes to local provider only (llama.cpp)
- No network calls to cloud providers
- All data stays on the user's machine
- Receipts stored locally

### cloud_fallback

```yaml
metadata:
  privacy: cloud_fallback
```

When `cloud_fallback` is set:
- Model Router tries local provider first
- Falls back to cloud (MiniMax) if local is unavailable
- Cloud use is logged in receipts
- Explicit opt-in per request

### cloud_only

```yaml
metadata:
  privacy: cloud_only
```

When `cloud_only` is set:
- Model Router uses cloud provider only
- No local inference attempted
- Used for testing or when local hardware unavailable

## What Must Not Be Logged

Receipts and logs must never contain:

- **Full prompts** — The actual text sent to the model
- **API keys or tokens** — Any credential resembling a token
- **Passwords** — User credentials for any service
- **Private file contents** — File data read or written
- **Session tokens** — Browser or application session data
- **SSH keys or certificates** — Authentication credentials
- **Personal identifiable information** — Names, addresses, phone numbers, etc.

## Prompt/Content Minimization

The principle of prompt minimization: store only what is needed for auditing.

Instead of storing full prompt:
```json
{
  "action": "ModelChat",
  "prompt": "List my Downloads folder",
  "prompt_length": 19,
  ...
}
```

Store metadata only:
```json
{
  "action": "ModelChat",
  "prompt_length": 19,
  "model": "gemma-4-E2B-it-Q8_0.gguf",
  ...
}
```

## Secret Redaction

Receipts automatically redact known secret fields:

```rust
fn redact_secrets(value: serde_json::Value) -> serde_json::Value {
    let secret_fields = ["api_key", "token", "password", "secret", "credential"];
    // Replace any key-value pair where key matches secret_fields with [REDACTED]
}
```

Tested via `test_receipts_secret_redaction()` in osai-receipt-logger.

## Future Memory Rules

Agent memory (planned for future) will follow strict rules:

- **User-owned** — Memory data belongs to the user
- **Local by default** — Memory stays on user's machine
- **Explicit access control** — OSAI cannot read memory without permission
- **User-controlled retention** — User can delete memory at any time
- **No hidden memory** — OSAI cannot store hidden context without disclosure

## Computer Use Privacy

Screenshots may contain private data. Hidden sessions may process private documents. Browser sessions may expose cookies or account information.

### Visible Mode Privacy

- User sees what OSAI sees (screenshot preview)
- User can approve or deny each action
- User can cancel at any time
- All actions visible in receipts

### Hidden Mode Privacy (Higher Risk)

**Screenshots**:
- May contain private desktop content
- Stored under privacy controls
- Never transmitted externally without user review

**Browser Sessions**:
- May contain logged-in accounts
- Cookies and sessions isolated
- No credential autofill without user action
- External site interactions require approval

**Document Processing**:
- Private documents may be opened
- Processed in isolated environment
- Final outputs returned, not intermediate steps
- User reviews outputs before external transmission

### Privacy Controls Required

For computer-use implementations:

1. **Minimize capture** — Only capture what is necessary
2. **User opt-in for transmission** — Cloud use with screenshots/desktop state requires explicit opt-in
3. **Retention controls** — User can delete artifacts/ receipts at any time
4. **Hidden session isolation** — No cross-contamination between hidden sessions
5. **Environment reset** — Hidden environment can be reset/destroyed
6. **Output sanitization** — Sensitive data in outputs replaced/redacted before return
7. **No external transmission without review** — User must review outputs before they are sent externally

## Cloud Use Privacy

Cloud use (MiniMax API) requires explicit opt-in:

```bash
curl -X POST http://127.0.0.1:8088/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "MiniMax-M2.7",
    "messages": [...],
    "metadata": {"privacy": "cloud_fallback"}
  }'
```

When cloud_fallback or cloud_only is used:
- Request is sent to MiniMax API
- Data processed by MiniMax cloud service
- Receipt logs cloud usage
- User has chosen to use cloud despite privacy implications

## Privacy vs Security Balance

Privacy and security work together:

- **Security** — Ensures bad actors cannot access data
- **Privacy** — Ensures even authorized access is minimized and auditable

OSAI's approach:
- Security prevents unauthorized access
- Privacy minimizes what is collected
- Receipts provide audit trail of what was accessed
- User retains control over their data

## Privacy Documentation Requirements

When adding new features that handle data:

1. Document what data is collected
2. Document what is stored (and where)
3. Document what is transmitted externally
4. Document how to delete data
5. Document how to opt out
6. Add tests for secret redaction

## User Control

The user ultimately controls their data:

- **Where data is stored** — `~/.local/share/osai/`
- **What is retained** — Receipts can be deleted by user
- **Cloud use** — Requires explicit privacy setting
- **Memory** — Future feature with explicit user control
- **Computer use** — User can inspect receipts and artifacts