# Downloads Organizer Agent

A sample OSAI agent that helps organize files in ~/Downloads.

## Files

- `manifest.yml` - Agent manifest with metadata
- `agent.md` - Agent behavior description
- `permissions.yml` - Permission configuration

## Installation (Future)

This agent will be installable via the OSAI Agent marketplace:

```bash
osai-agent install downloads-organizer
```

After installation, the agent will be registered with the OSAI Agent runtime and available for use.

## Manual Setup

For development/testing, initialize with:

```bash
cargo run -p osai-agent-cli -- init /tmp/downloads-organizer
```

Then copy the example files:

```bash
cp -r examples/agents/downloads-organizer/* /tmp/downloads-organizer/
```

## Usage

Once installed, interact with the agent:

```
User: "Organize my downloads folder"
Agent: [scans Downloads, creates plan, requests approval]
User: "Approve"
Agent: [moves files, logs receipt]
```

## Policy

This agent uses the `default-secure` policy which:
- Allows read-only file operations without approval
- Requires approval for file moves
- Denies shell execution and network access
