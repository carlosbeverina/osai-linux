# OSAI Agent CLI

Command-line tool for working with OSAI Agent App manifests and Plan DSL files.

## Purpose

OSAI Agent CLI provides commands for:

- Validating and printing OSAI Plan DSL files
- Validating OSAI Policy files
- Authorizing plans against policies with audit receipts
- Listing and viewing receipt logs
- Initializing new OSAI agent directories

## Commands

### Plan Commands

#### Validate a plan
```bash
osai-agent plan validate <path>
```
Reads a YAML or JSON plan file, parses it using osai-plan-dsl, and validates it. Prints "Plan is valid" on success or the validation error on failure.

#### Print a plan
```bash
osai-agent plan print <path> --format json|yaml
```
Reads a plan file, validates it, and prints it in the specified format (JSON pretty or YAML).

### Policy Commands

#### Validate a policy
```bash
osai-agent policy validate <path>
```
Reads a YAML policy file and validates it using osai-toolbroker's ToolPolicy::from_yaml.

### Tool Commands

#### Authorize a plan
```bash
osai-agent tool authorize --plan <path> --policy <path> --receipts-dir <path>
```
Authorizes each step in a plan against a policy and creates audit receipts.

**This command does not execute tools.** It only validates authorization decisions and creates receipts.

For each step, it:
1. Converts the step into a ToolRequest
2. Calls the ToolBroker to authorize
3. Writes a receipt for the decision
4. Prints the decision line

Output format per step:
```
step=<step_id> action=<action> allowed=<true|false> approval=<true|false> mode=<mode> reason="<reason>"
```

Exit code is non-zero if any step is denied.

#### Run a plan (authorize + execute)
```bash
osai-agent tool run --plan <path> --policy <path> --receipts-dir <path> --allowed-root <path>... [--approve <step_id>] [--approve-all] [--model-router-url <url>]
```
Authorizes and executes each step in a plan against a policy, then creates audit receipts.

**This command both authorizes AND executes safe actions.** It connects Plan DSL + ToolBroker + ToolExecutor.

For each step, it:
1. Converts the step into a ToolRequest
2. Calls the ToolBroker to authorize
3. Prints authorization decision
4. If approval is required but not provided: skips execution
5. If approval is required and provided via `--approve` or `--approve-all`: passes to executor with adjusted decision
6. Calls ToolExecutor to execute (if allowed and no approval required, or explicitly approved)
7. Prints execution result
8. Writes a receipt

**Approval Flags:**
- `--approve <step_id>` - Approve a specific step by its ID. Can be passed multiple times.
- `--approve-all` - Approve all steps that require user approval.

Output format per step:
```
step=<step_id>
authorization: allowed=<true|false> approval=<true|false> mode=<mode> reason="<reason>"
[approval: source=cli step=<step_id>]
execution: status=<Executed|Failed|Skipped> action=<action> error="<error_or_empty>"
```

**Important**: Approval does not mean unsafe actions execute. ToolExecutor v0.1 still refuses FilesMove, FilesWrite, FilesDelete, ShellRunSandboxed, BrowserOpenUrl, and Custom actions regardless of approval. Approval only bypasses the "requires user approval" check for actions that would otherwise be allowed by policy.

**Exit behavior:**
- Unapproved skipped (approval required but not provided): exit 0
- Approved but executor refuses (unsupported action): exit non-zero
- Denied by policy: exit non-zero
- Execution failed: exit non-zero

**v0.1 Executable Actions**: Only FilesList, DesktopNotify, and ModelChat are executed (simulated). All other actions (FilesWrite, FilesMove, FilesDelete, ShellRunSandboxed, BrowserOpenUrl, Custom) are refused.

**Path Restrictions**: FilesList is constrained to allowed_root directories.

#### Run examples

Run without approval (steps requiring approval will be skipped):
```bash
osai-agent tool run \
  --plan examples/plans/organize-downloads.yml \
  --policy examples/policies/default-secure.yml \
  --receipts-dir /tmp/osai-receipts \
  --allowed-root ~/
```

Approve specific steps and execute:
```bash
osai-agent tool run \
  --plan examples/plans/organize-downloads.yml \
  --policy examples/policies/default-secure.yml \
  --receipts-dir /tmp/osai-receipts \
  --allowed-root ~/ \
  --approve step-2
```

Approve all approval-required steps:
```bash
osai-agent tool run \
  --plan examples/plans/organize-downloads.yml \
  --policy examples/policies/default-secure.yml \
  --receipts-dir /tmp/osai-receipts \
  --allowed-root ~/ \
  --approve-all
```

Sample output (without approval):
```
step=step-1
authorization: allowed=true approval=false mode=Allow reason="Action FilesList allowed by policy"
execution: status=Executed action=FilesList error=""
step=step-2
authorization: allowed=true approval=true mode=Ask reason="Action FilesMove requires user approval"
execution: status=Skipped action=FilesMove error="Execution skipped: requires user approval"
```

Sample output (with --approve step-2):
```
step=step-1
authorization: allowed=true approval=false mode=Allow reason="Action FilesList allowed by policy"
execution: status=Executed action=FilesList error=""
step=step-2
authorization: allowed=true approval=true mode=Ask reason="Action FilesMove requires user approval"
approval: source=cli step=step-2
execution: status=Failed action=FilesMove error="Action is not executable in ToolExecutor v0.1"
```

### Receipt Commands

#### List receipts
```bash
osai-agent receipt list <root_dir>
```
Lists all receipt JSON file paths in the specified directory, sorted ascending by filename.

#### Show a receipt
```bash
osai-agent receipt show <root_dir> <uuid>
```
Reads and prints a specific receipt by its UUID.

### Init Command

#### Initialize agent directory
```bash
osai-agent init <directory>
```
Creates a new OSAI agent directory with:
- `manifest.yml` - Agent manifest
- `agent.md` - Agent description
- `permissions.yml` - Permission configuration
- `README.md` - Usage documentation

Does not overwrite existing files.

## Example Agent Manifest

```yaml
name: my-agent
version: "0.1"
description: My OSAI agent
entrypoint: agent.md
permissions:
  - FilesList
  - FilesRead
memory:
  type: local
  scope: agent
model_policy: default
```

## Plan Validate Workflow

1. Create a plan file (plan.yml):
```yaml
version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Create project directory
actor: osai-agent
risk: Medium
approval: Ask
steps:
  - id: step-1
    action:
      type: FilesWrite
    description: Create the project directory
    requires_approval: true
    inputs:
      path: /home/user/project
metadata: {}
```

2. Validate the plan:
```bash
osai-agent plan validate plan.yml
# Output: Plan is valid
```

3. Print as JSON:
```bash
osai-agent plan print plan.yml --format json
```

## Tool Authorize Workflow

Authorize the organize-downloads plan using the default-secure policy:

```bash
osai-agent tool authorize \
  --plan examples/plans/organize-downloads.yml \
  --policy examples/policies/default-secure.yml \
  --receipts-dir /tmp/osai-receipts
```

Sample output:
```
step=step-1 action=FilesList allowed=true approval=false mode=Allow reason="Action FilesList allowed by policy"
step=step-2 action=FilesMove allowed=true approval=true mode=Ask reason="Action FilesMove requires user approval"
step=step-3 action=FilesMove allowed=true approval=true mode=Ask reason="Action FilesMove requires user approval"
step=step-4 action=ReceiptCreate allowed=true approval=false mode=Allow reason="Action ReceiptCreate allowed by policy"
```

If any step is denied, the exit code will be non-zero.

## Model Router Integration

When executing `ModelChat` actions, ToolExecutor can optionally route requests through a local Model Router service instead of returning simulated responses.

**Important Security Constraints:**
- Model Router URL must use `http://` (not `https://`)
- Model Router URL must be loopback only (`localhost` or `127.0.0.1`)
- External URLs are rejected
- Non-loopback hostnames are rejected
- Empty URLs are rejected

**Without `--model-router-url`**: `ModelChat` returns a simulated response indicating the feature is not yet implemented.

**With `--model-router-url`**: `ModelChat` POSTs to the Model Router's `/v1/chat/completions` endpoint and returns the actual model response.

### Example with Model Router

```bash
cargo run -p osai-agent-cli -- tool run \
  --plan examples/plans/model-chat.yml \
  --policy examples/policies/default-secure.yml \
  --receipts-dir /tmp/osai-receipts \
  --allowed-root "$HOME/Downloads" \
  --model-router-url http://127.0.0.1:8088
```

This executes the `model-chat.yml` plan which contains a single `ModelChat` step. The Model Router (`osai-local`, `gemma4:*`, `MiniMax-*`, `osai-auto`) handles the actual model invocation.

### ModelChat Inputs

The `ModelChat` action accepts these inputs:

| Input | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | No | Model identifier (default: `osai-auto`) |
| `prompt` | string | Yes* | Single prompt string (if no `messages`) |
| `messages` | array | Yes* | Array of message objects with `role` and `content` |
| `metadata` | object | No | Metadata hints for auto-routing (`privacy`, `complexity`, `speed`) |

*Either `prompt` or `messages` must be provided.

### ModelChat Output

When called via Model Router, the output JSON is:
```json
{
  "provider": "model-router",
  "model": "<actual-model-used>",
  "content": "<assistant-response>",
  "simulated": false
}
```

Without Model Router (simulated):
```json
{
  "simulated": true,
  "message": "ModelChat execution is not implemented yet"
}
```

### Example Plan: model-chat.yml

See `examples/plans/model-chat.yml` for a complete example:
```yaml
version: "0.1"
id: "e8b9a2c4-1234-5678-9abc-def012345678"
title: "Model Chat Example"
actor: "user"
risk: Low
approval: Auto
steps:
  - id: "chat-step-1"
    action:
      type: ModelChat
    description: "Chat with local model using osai-auto"
    requires_approval: false
    inputs:
      model: "osai-auto"
      prompt: "Hello, how are you?"
      metadata:
        privacy: "local_only"
        complexity: "low"
```

## Exit Codes

- `0` - Success
- `non-zero` - Error (validation failure, file not found, denied authorization, etc.)
