# Security Model

## Core Security Principle

OSAI never trusts the model. Every model output is validated. Every action is authorized. Every change is audited.

**Rule 1**: Model output is untrusted — always validated through Plan DSL and ToolBroker.

**Rule 2**: ToolBroker is the authorization layer — ToolExecutor must never bypass it.

**Rule 3**: Every action produces a receipt — receipts must not leak secrets.

**Rule 4**: Shell commands require sandbox — `ShellRunSandboxed` is the only permitted shell action.

**Rule 5**: Destructive actions require explicit approval — FilesWrite, FilesMove, FilesDelete are not auto-approved.

## Separation of Concerns

```
Model Output (untrusted)
    │
    ▼
Plan DSL Validation (structure validation, not security)
    │
    ▼
ToolBroker Authorization (security decisions)
    │
    ▼
ToolExecutor Execution (only authorized actions)
    │
    ▼
ReceiptLogger Audit (receipts for everything)
```

## Model Output Is Untrusted

The model can produce:
- Invalid Plan DSL (syntax errors)
- Plans that request unauthorized actions
- Plans that exceed allowed roots
- Plans with malicious-looking content
- Unexpected action types or parameters

**Countermeasures**:
- Plan DSL validation catches syntax/structure errors
- ToolBroker evaluates policy against the plan
- ToolExecutor only executes approved steps
- Receipts log what was attempted vs what was executed

## ToolBroker Authorization Layer

ToolBroker evaluates every action against the active policy. Decisions:
- **Allow** — Execute without asking
- **Ask** — Execute only after user approval
- **Deny** — Do not execute, log as denied

ToolBroker does not execute actions. It only decides what is allowed.

**Policy example**:
```yaml
rules:
  - match:
      action: ShellRunSandboxed
    decision: Ask
  - match:
      action: FilesWrite
    decision: Deny  # Not yet safe
  - match:
      action: FilesList
      allowed_paths: ["~/Downloads", "~/Documents"]
    decision: Allow
```

## ToolExecutor Must Not Bypass ToolBroker

ToolExecutor receives authorization decisions from ToolBroker and executes only the authorized steps.

**Critical rule**: ToolExecutor must never execute an action that ToolBroker did not authorize. This is enforced by:
- ToolExecutor checking ToolBroker's decision before execution
- Denied decisions causing immediate failure
- Approved steps being the only ones executed

## Allowed Roots

File operations are scoped by allowed roots:

- `~/Downloads` — Default allowed root
- `~/Documents` — Optional allowed root
- No other paths accessible by default

Plans that operate outside allowed roots are rejected by ToolBroker.

## Loopback-Only Local Services

All local services bind to loopback only:

- llama-server: `http://127.0.0.1:8092`
- Model Router: `http://127.0.0.1:8088`
- osai-api: `http://127.0.0.1:8090`

No external exposure. No network attack surface.

## Cloud Fallback Restrictions

Cloud use is explicit and policy-controlled:

- `privacy: local_only` is the default
- Cloud requires explicit privacy setting in request
- Cloud credentials must not be in the repository
- Cloud API keys must be in environment variables or token files
- MiniMax is the only approved cloud provider

## Shell Sandboxing

`ShellRunSandboxed` is the only permitted shell action. It:
- Runs a single command with fixed arguments
- Does not support pipes, redirects, or shell features
- Does not run interactive commands
- Cannot start background processes
- Is always subject to ToolBroker policy evaluation

**Example**:
```
action:
  type: ShellRunSandboxed
command: "ls"
args: ["-la", "~/Downloads"]
```

## File Operation Risks

| Action | Risk | Status |
|--------|------|--------|
| FilesList | Low — read-only | ✅ Safe |
| FilesWrite | High — can overwrite | ⚠️ Requires approval, security review |
| FilesMove | High — can relocate | ⚠️ Requires approval, security review |
| FilesDelete | Critical — destructive | ⚠️ Requires approval, rollback needed |

FilesWrite, FilesMove, FilesDelete require security review before enabling real execution.

## Computer Use Risks

### Visible Computer Use
- OSAI affects the user's active desktop session
- User can see actions in real-time
- Sensitive actions require approval
- Useful for teaching and collaboration

**Risks**:
- Accidental file modification if approval given too quickly
- Screen recording without user awareness
- Credential entry if user approves

### Hidden Computer Use
- OSAI operates in isolated environment
- User does not watch every step
- OSAI returns final outputs/artifacts

**Risks** (higher):
- Invisible credential exposure
- Unintended external communication
- Data exfiltration
- Destructive actions without user seeing
- Account changes without user awareness
- Browser sessions exposing cookies/accounts

**Required safety design**:
- Every computer-use task starts as a Plan DSL plan
- Plan validated before execution
- ToolBroker must authorize computer-use capabilities
- User approval required for sensitive categories (credentials, payments, destructive actions)
- Strict network/browser/file policies
- No credential entry without explicit user action
- No purchases/payments/account changes without explicit approval
- No destructive changes without explicit approval
- All actions summarized in receipts
- Screenshots/artifacts under privacy controls
- Task can be cancelled at any time
- Hidden environment can be reset/destroyed
- Outputs must be reviewed before external transmission

## Receipt Privacy

Receipts must not contain:
- Full prompts (prompt text itself)
- API keys or tokens
- Passwords
- File contents (for security)
- Screenshot pixel data (without privacy controls)

Receipts must contain:
- Action type
- Timestamp
- Status (Executed, Denied, Failed)
- Model used (if applicable)
- Sanitized inputs (secrets replaced with `[REDACTED]`)
- Outcome/error
- Metadata (for auditing)

## Threat Model

### Threats Addressed

1. **Model produces malicious plan** → ToolBroker evaluates policy, denies unauthorized actions
2. **Plan operates outside allowed roots** → ToolBroker rejects plan
3. **Shell command injection** → ShellRunSandboxed prevents shell features
4. **API key leakage in receipts** → Secret redaction before storage
5. **Unauthorized file access** → Allowed roots constrain file ops
6. **Cloud use without policy** → `local_only` default, explicit opt-in for cloud
7. **Credential exposure via computer use** → No credential entry without explicit user action
8. **Hidden exfiltration via computer use** → Network policies, receipt audit, artifact review

### Threats Partially Addressed

1. **User approves dangerous action** → Required for sensitive categories, receipts provide audit trail
2. **Browser automation for credential theft** → Approval required, isolated session, network policies
3. **Screenshot containing sensitive data** → Privacy controls, user opt-in for capture/transmission

### Threats Not Yet Addressed

1. **Full model jailbreaking** → Plan DSL validation + ToolBroker provide some protection, but model may still produce novel attack vectors
2. **Side-channel data in model responses** → Receipts capture what was attempted, not what was implied
3. **Covert channels via model behavior** → Receipts provide audit trail, but detection is difficult

## Security Review Requirements

Before enabling any of the following, a security review is required:

1. FilesWrite real execution
2. FilesMove real execution
3. FilesDelete real execution
4. BrowserOpenUrl real execution
5. ComputerUseVisible implementation
6. ComputerUseHidden implementation
7. Voice pipeline integration
8. Any new tool that interacts with external systems

Security review should cover:
- Authorization boundary (ToolBroker)
- Execution boundary (ToolExecutor)
- Receipt coverage and privacy
- Denial of service risks
- Privilege escalation risks
- Data exfiltration risks
- Credential exposure risks