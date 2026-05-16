# PR Checklist

Use this checklist before marking a PR ready for review.

## Pre-Submission Checks

### Code Quality
- [ ] `cargo fmt --check` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes
- [ ] `pytest tests` passes (services/model-router)

### MVP Loop Preserved
- [ ] `cargo run -p osai-agent-cli -- chat "test"` works
- [ ] `cargo run -p osai-agent-cli -- ask --print-plan "test"` works
- [ ] `cargo run -p osai-agent-cli -- plan validate examples/plans/model-chat.yml` works
- [ ] `cargo run -p osai-agent-cli -- doctor` works

### Security
- [ ] No secrets committed (API keys, tokens, passwords)
- [ ] No local models committed (`.local-models/`)
- [ ] No local runtimes committed (`.local-runtimes/`)
- [ ] Receipts do not leak secrets
- [ ] `grep -R -i "password\|api_key\|token\|secret\|credential" ~/.local/share/osai/receipts/` returns clean
- [ ] ToolBroker boundaries not bypassed
- [ ] ToolExecutor executes only authorized actions

### CLI Compatibility
- [ ] Existing commands still work
- [ ] Existing flags unchanged
- [ ] Output format preserved
- [ ] Exit codes correct

### Architecture
- [ ] UI uses osai-api, not shell-out to CLI
- [ ] osai-api uses osai-agent-core, not replicated CLI logic
- [ ] API endpoints follow documented patterns
- [ ] Token auth implemented for protected endpoints

### Documentation
- [ ] Docs updated if architecture changed
- [ ] New components documented
- [ ] Security model documented if changed
- [ ] API changes documented
- [ ] Breaking changes noted

### Destructive Tools
- [ ] FilesWrite/FilesMove/FilesDelete not enabled without security review
- [ ] BrowserOpenUrl not enabled without security review
- [ ] ShellRunSandboxed follows sandbox constraints
- [ ] Approval required for sensitive categories

### Computer Use (if applicable)
- [ ] All computer-use tasks start as Plan DSL
- [ ] ToolBroker evaluates computer-use authorization
- [ ] User approval required for sensitive categories
- [ ] Receipts include computer-use mode (visible/hidden)
- [ ] Hidden mode isolated from active desktop
- [ ] Task can be cancelled
- [ ] Screenshots/artifacts under privacy controls
- [ ] No hidden exfiltration of data
- [ ] No credential entry without explicit user action
- [ ] No purchases/payments without explicit approval
- [ ] No destructive changes without explicit approval

### Local Runtime (if applicable)
- [ ] `./scripts/osai-local-check` passes
- [ ] llama.cpp /models returns 200
- [ ] Model Router /health returns 200
- [ ] Model Router returns real (non-mock) responses
- [ ] Tool receipts generated

### Receipt Format
- [ ] Receipts have required fields (id, timestamp, action, status)
- [ ] Prompt content not stored (only prompt_length)
- [ ] Secrets redacted with `[REDACTED]`
- [ ] receipts do not contain full prompts or API keys

## PR Body Checklist

- [ ] Summary: 1-3 bullets describing change and why
- [ ] Files Changed: list with brief descriptions
- [ ] Tests Run: cargo fmt, check, test, pytest
- [ ] Security Impact: what boundaries were affected
- [ ] Runtime Impact: what behavior changed
- [ ] Limitations: what is not yet implemented
- [ ] Follow-ups: what still needs to be done

## Review Checklist (for maintainers)

- [ ] Design makes sense
- [ ] Implementation follows architecture
- [ ] Tests are sufficient
- [ ] Documentation is complete
- [ ] No security regressions
- [ ] No secrets in diff
- [ ] Receipts privacy preserved
- [ ] CLI compatibility preserved
- [ ] Computer-use safety design complete (if applicable)
- [ ] No breaking changes without migration path