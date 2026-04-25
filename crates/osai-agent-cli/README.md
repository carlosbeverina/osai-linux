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
osai-agent tool run --plan <path> --policy <path> --receipts-dir <path> --allowed-root <path>...
```
Authorizes and executes each step in a plan against a policy, then creates audit receipts.

**This command both authorizes AND executes safe actions.** It connects Plan DSL + ToolBroker + ToolExecutor.

For each step, it:
1. Converts the step into a ToolRequest
2. Calls the ToolBroker to authorize
3. Prints authorization decision
4. Calls ToolExecutor to execute (if allowed and no approval required)
5. Prints execution result
6. Writes a receipt

Output format per step:
```
step=<step_id>
authorization: allowed=<true|false> approval=<true|false> mode=<mode> reason="<reason>"
execution: status=<Executed|Failed|Skipped> action=<action> error="<error_or_empty>"
```

**Execution vs Authorization**: `tool authorize` only checks if actions are permitted by policy. `tool run` actually executes safe actions (FilesList, DesktopNotify, ModelChat) through ToolExecutor after authorization.

Exit code is non-zero if any step is denied or execution fails.

**v0.1 Executable Actions**: Only FilesList, DesktopNotify, and ModelChat are executed (simulated). All other actions (FilesWrite, FilesMove, FilesDelete, ShellRunSandboxed, BrowserOpenUrl, Custom) are refused.

**Path Restrictions**: FilesList is constrained to allowed_root directories.

#### Run example
```bash
osai-agent tool run \
  --plan examples/plans/safe-list.yml \
  --policy examples/policies/default-secure.yml \
  --receipts-dir /tmp/osai-receipts \
  --allowed-root /tmp \
  --allowed-root /home/user
```

Sample output:
```
step=step-1
authorization: allowed=true approval=false mode=Allow reason="Action FilesList allowed by policy"
execution: status=Executed action=FilesList error=""
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

## Exit Codes

- `0` - Success
- `non-zero` - Error (validation failure, file not found, denied authorization, etc.)
