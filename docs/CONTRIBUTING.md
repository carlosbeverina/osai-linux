# Contributing to OSAI

## Development Philosophy

OSAI is built with a "docs first, code second" philosophy for significant changes. Before implementing large features, update the documentation. This ensures:
- Design intent is clear
- Implementation can be reviewed
- Future developers understand the reasoning

## Getting Started

### Prerequisites

```bash
# Rust
rustc --version
cargo --version

# Python (for Model Router)
python3 --version

# NVIDIA CUDA (for GPU inference)
nvcc --version
nvidia-smi
```

### Initial Setup

```bash
cd ~/Projects/osai-linux
source "$HOME/.cargo/env"

# Install dependencies
cargo fetch

# Run tests to verify baseline
cargo test --workspace
```

### Running Tests

```bash
# Rust tests
cargo fmt --check
cargo check --workspace
cargo test --workspace

# Model Router tests
cd services/model-router && pytest tests && cd -

# Local runtime (if available)
./scripts/osai-local-up
./scripts/osai-local-check
./scripts/osai-local-down
```

## Branch Strategy

### Branch Naming

```
feature/<feature-name>      # New features
fix/<bug-description>       # Bug fixes
docs/<topic>                # Documentation
refactor/<focus-area>       # Refactoring
test/<what-is-tested>       # Test additions
```

### Creating a Branch

```bash
git checkout -b feature/your-feature-name
```

## Commit Guidelines

### Commit Message Format

```
<type>: <short description>

<longer description if needed>

# Types: feat, fix, docs, refactor, test, chore
# Examples:
# feat: add osai-agent-core extraction
# fix: correct receipt secret redaction
# docs: add computer use strategy document
```

### What to Commit

- **Do commit**:
  - Code changes
  - Test additions
  - Documentation updates
  - Configuration changes (non-secret)

- **Do not commit**:
  - API keys, tokens, credentials
  - Model files (GGUF)
  - Built runtimes
  - Local receipts
  - `.env` files with secrets
  - Large generated files

## Security Expectations

### Before Adding New Tools

New tools that interact with:
- Filesystem (FilesWrite, FilesMove, FilesDelete)
- Network (BrowserOpenUrl, HTTP requests)
- Credentials (password entry, API key use)
- System (ShellRunSandboxed, DesktopNotify)

Require:
1. Security design document
2. ToolBroker authorization rules
3. ToolExecutor implementation
4. Receipt format
5. Security review from project maintainer

### Before Enabling Destructive Actions

FilesWrite, FilesMove, FilesDelete require:
1. Rollback capability
2. User confirmation workflow
3. Path traversal prevention
4. Test coverage for edge cases
5. Security review

### Receipt Privacy

Receipts must never contain:
- Full prompts
- API keys or tokens
- Passwords
- File contents
- Screenshot pixel data

Always use `redact_secrets()` when writing receipts.

## Documentation Expectations

### When Documentation Is Required

Update docs when:
- Adding new components
- Changing architecture
- Adding new tools
- Changing security model
- Adding new environment variables
- Changing Plan DSL format
- Adding new API endpoints

### Where to Put Documentation

| Type | Location |
|------|----------|
| Agent instructions | `AGENTS.md`, `CODEX.md` |
| Architecture | `docs/ARCHITECTURE.md` |
| Project overview | `docs/PROJECT_OVERVIEW.md` |
| Current state | `docs/CURRENT_STATE.md` |
| Development plan | `docs/DEVELOPMENT_PLAN.md` |
| Security | `docs/SECURITY_MODEL.md` |
| Testing | `docs/TESTING.md` |
| Tooling | `docs/TOOLING_AND_POLICIES.md` |
| Specific topics | `docs/*.md` |

## Runtime and Model Handling

### Local Models

Models are stored in `.local-models/`:
```
.local-models/
├── llamacpp/
│   ├── gemma-4-e2b-it/
│   │   └── gemma-4-E2B-it-Q8_0.gguf
│   └── qwen2.5-0.5b-instruct/
│       └── qwen2.5-0.5b-instruct-q4_k_m.gguf
```

**Never commit model files. Never add them to Git.**

### Local Runtimes

Runtimes are stored in `.local-runtimes/`:
```
.local-runtimes/
├── llama.cpp/
└── vllm/ (if used)
```

**Never commit runtime files. Never add them to Git.**

### Environment Variables

Key environment variables:
```bash
OSAI_LLAMACPP_BASE_URL=http://127.0.0.1:8092/v1
OSAI_LLAMACPP_MODEL=gemma-4-E2B-it-Q8_0.gguf
OSAI_LLAMACPP_API_KEY=osai-local-dev-token
OSAI_MODEL_ROUTER_URL=http://127.0.0.1:8088
OSAI_LOCAL_PROVIDER=llamacpp
OSAI_API_TOKEN=<token>  # For osai-api auth
```

Store tokens in `~/.config/osai/api-token`, not in the repository.

## Code Style

- Run `cargo fmt` before committing
- Follow existing patterns in the codebase
- Add tests for new behavior
- Keep functions small and focused
- Document complex logic with comments (only when the WHY is non-obvious)

## Test Expectations

### Before Submitting PR

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cd services/model-router && pytest tests
```

### MVP Loop Test

After any change that affects chat/ask/apply:
```bash
cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI test OK"
cargo run -p osai-agent-cli -- ask --print-plan "Create a safe plan to list my Downloads folder"
```

### Receipt Secret Scan

After any change that touches receipts:
```bash
grep -R -i "password\|api_key\|token\|secret\|credential" \
  ~/.local/share/osai/receipts/*/ 2>/dev/null || echo "Clean"
```

## No Secrets Policy

Never in the repository:
- API keys
- Tokens
- Passwords
- Private keys
- `.env` files
- Cloud credentials
- Model files
- Built runtime binaries

If you accidentally commit secrets:
1. Remove them immediately
2. Rotate the credentials
3. Do not assume Git history is safe

## Communication

For questions:
- Open a GitHub Discussion
- Open a GitHub Issue for bugs
- Prefix branch names clearly

## Recognition

All contributors are recognized in:
- Git commit history
- PR descriptions
- Documentation updates