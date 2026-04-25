# OSAI Examples

This directory contains example plans, policies, and agents for manual testing of the OSAI system.

## Manual Testing

Test the examples using the `osai-agent` CLI:

### Plan Validation

Validate the organize-downloads plan:
```bash
cargo run -p osai-agent-cli -- plan validate examples/plans/organize-downloads.yml
```

Print the plan as JSON:
```bash
cargo run -p osai-agent-cli -- plan print examples/plans/organize-downloads.yml --format json
```

### Policy Validation

Validate the default-secure policy:
```bash
cargo run -p osai-agent-cli -- policy validate examples/policies/default-secure.yml
```

### Agent Initialization

Initialize a sample agent directory:
```bash
cargo run -p osai-agent-cli -- init /tmp/osai-sample-agent
```

### Directory Structure

```
examples/
├── plans/
│   ├── organize-downloads.yml    # Valid plan with FilesMove steps
│   └── risky-shell.yml          # Invalid plan (policy violation)
├── policies/
│   └── default-secure.yml       # Secure default policy
├── agents/
│   └── downloads-organizer/
│       ├── manifest.yml
│       ├── agent.md
│       ├── permissions.yml
│       └── README.md
└── README.md
```

## Example Descriptions

### organize-downloads.yml

A valid plan that:
- Lists files in ~/Downloads
- Moves files to appropriate subdirectories (Pictures, Documents)
- Requires approval for moves
- Includes rollback capability

### risky-shell.yml

An **invalid** plan that demonstrates security features:
- Attempts ShellRunSandboxed with `network=true` and `sandbox=false`
- Violates the default-secure policy's shell constraints
- Should be denied by ToolBroker

### default-secure.yml

A secure policy that:
- Allows safe actions (ModelChat, DesktopNotify, FilesList, FilesRead)
- Requires approval for file modifications (FilesWrite, FilesMove, FilesDelete)
- Denies shell execution by default
- Blocks network access without sandbox

## Testing Workflow

1. **Validate a valid plan:**
   ```bash
   cargo run -p osai-agent-cli -- plan validate examples/plans/organize-downloads.yml
   # Output: Plan is valid
   ```

2. **Print plan as JSON:**
   ```bash
   cargo run -p osai-agent-cli -- plan print examples/plans/organize-downloads.yml --format json
   ```

3. **Validate policy:**
   ```bash
   cargo run -p osai-agent-cli -- policy validate examples/policies/default-secure.yml
   # Output: Policy is valid
   ```

4. **Initialize new agent:**
   ```bash
   cargo run -p osai-agent-cli -- init /tmp/my-new-agent
   ```
