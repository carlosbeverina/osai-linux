# OSAI Documentation

Welcome to the OSAI documentation. This index provides reading order and links for different roles and purposes.

## Quick Links

| Document | Purpose |
|----------|---------|
| [AGENTS.md](../AGENTS.md) | Instructions for AI coding agents |
| [CODEX.md](../CODEX.md) | Specific guidance for Codex Web |
| [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) | What OSAI is and why it exists |
| [CURRENT_STATE.md](CURRENT_STATE.md) | What is currently implemented |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Current and target architecture |

## Reading Order by Role

### New Developer

1. [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) — Understand what OSAI is
2. [CURRENT_STATE.md](CURRENT_STATE.md) — Understand what exists
3. [ARCHITECTURE.md](ARCHITECTURE.md) — Understand the system design
4. [LOCAL_SETUP.md](LOCAL_SETUP.md) — Set up local development
5. [TESTING.md](TESTING.md) — Understand how to test changes
6. [CONTRIBUTING.md](CONTRIBUTING.md) — Understand contribution guidelines

### Codex Web Agent

1. [CODEX.md](../CODEX.md) — Specific guidance for Codex
2. [AGENTS.md](../AGENTS.md) — General agent instructions
3. [CURRENT_STATE.md](CURRENT_STATE.md) — What exists
4. [DEVELOPMENT_PLAN.md](DEVELOPMENT_PLAN.md) — What to implement next
5. [GITHUB_CODEX_WORKFLOW.md](GITHUB_CODEX_WORKFLOW.md) — How to produce PRs
6. [PR_CHECKLIST.md](PR_CHECKLIST.md) — Checklist before submitting

### Security Reviewer

1. [SECURITY_MODEL.md](SECURITY_MODEL.md) — Security boundaries and rules
2. [PRIVACY_MODEL.md](PRIVACY_MODEL.md) — Privacy guarantees
3. [RECEIPTS.md](RECEIPTS.md) — Audit trail structure
4. [COMPUTER_USE_STRATEGY.md](COMPUTER_USE_STRATEGY.md) — Computer use safety design
5. [TOOLING_AND_POLICIES.md](TOOLING_AND_POLICIES.md) — Tool authorization model

### UI Developer

1. [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) — Product vision
2. [UI_DESKTOP_STRATEGY.md](UI_DESKTOP_STRATEGY.md) — Desktop integration strategy
3. [API_DESIGN.md](API_DESIGN.md) — osai-api endpoints
4. [COMPUTER_USE_STRATEGY.md](COMPUTER_USE_STRATEGY.md) — Computer use UI requirements

### Runtime/Model Developer

1. [MODEL_RUNTIME.md](MODEL_RUNTIME.md) — Model runtime architecture
2. [CURRENT_STATE.md](CURRENT_STATE.md) — Current runtime state
3. [LOCAL_SETUP.md](LOCAL_SETUP.md) — Local development setup
4. [ROADMAP.md](ROADMAP.md) — Future runtime features

## Documentation Map

### Foundation
```
AGENTS.md → Project rules for all agents
CODEX.md → Codex-specific guidance
PROJECT_OVERVIEW.md → Product vision and goals
CURRENT_STATE.md → What exists now
ARCHITECTURE.md → System design
```

### Planning
```
ROADMAP.md → Phased implementation plan
DEVELOPMENT_PLAN.md → How to implement in order
OSAI_AGENT_CORE_PLAN.md → osai-agent-core extraction plan
```

### Technical Specifications
```
API_DESIGN.md → osai-api endpoint design
MODEL_RUNTIME.md → llama.cpp/vLLM/MiniMax configuration
LOCAL_SETUP.md → Local development setup
TOOLING_AND_POLICIES.md → ToolBroker and policy system
RECEIPTS.md → Receipt audit trail format
```

### Safety and Security
```
SECURITY_MODEL.md → Security boundaries
PRIVACY_MODEL.md → Privacy guarantees
COMPUTER_USE_STRATEGY.md → Computer use safety design
TESTING.md → Testing requirements
```

### Process
```
GITHUB_CODEX_WORKFLOW.md → How to work with GitHub
CONTRIBUTING.md → Contribution guidelines
PR_CHECKLIST.md → Pre-submission checklist
```

## Key Principles

1. **Local-first** — OSAI defaults to local computation
2. **Security by design** — Model output is untrusted, ToolBroker authorizes
3. **Audit everything** — Every action produces a receipt
4. **Privacy by default** — Secrets never stored in receipts
5. **Small PRs** — One logical change per PR
6. **Docs first** — Large architectural changes documented before code

## Current Phase

OSAI is in Phase 0 (Documentation) followed by Phase 1 (osai-agent-core extraction).

See [ROADMAP.md](ROADMAP.md) for the full phased plan.

## Important Constraints

- **Never touch `.local-models/` or `.local-runtimes/`** — Local-only directories
- **Never commit secrets** — API keys, tokens, credentials not allowed
- **Never bypass ToolBroker** — All actions authorized, not bypassed
- **Never store prompts in receipts** — Only prompt length
- **Do not skip phases** — Follow the implementation order in DEVELOPMENT_PLAN.md

## Finding Things

| What | Where |
|------|-------|
| CLI commands | `crates/osai-agent-cli/src/main.rs` |
| Plan DSL | `crates/osai-plan-dsl/` |
| ToolBroker | `crates/osai-toolbroker/` |
| ToolExecutor | `crates/osai-tool-executor/` |
| ReceiptLogger | `crates/osai-receipt-logger/` |
| Model Router | `services/model-router/` |
| Examples | `examples/` |
| Scripts | `scripts/` |
| Systemd services | `systemd/user/` |

## Getting Help

- Open a GitHub Discussion for design questions
- Open a GitHub Issue for bugs
- Read `CLAUDE.md` for project context