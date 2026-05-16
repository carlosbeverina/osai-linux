# GitHub and Codex Workflow

## How to Work with This Repository

This document describes how Codex Web (or any AI coding agent) should work with the OSAI repository from GitHub.

## Before Starting Any Task

1. **Read the project documentation**:
   - `CLAUDE.md` — Project context and rules
   - `AGENTS.md` — Instructions for AI agents
   - `CODEX.md` — Codex-specific guidance
   - `docs/PROJECT_OVERVIEW.md` — What OSAI is
   - `docs/CURRENT_STATE.md` — What currently exists

2. **Understand the current state**:
   - Run `cargo test --workspace` to verify baseline
   - Verify what exists vs what is planned
   - Do not assume features exist that are in the roadmap

3. **Identify the correct next step**:
   - See `docs/DEVELOPMENT_PLAN.md` for implementation order
   - Follow phases in order
   - Do not skip phases

## PR Size Guidelines

**Small PRs are preferred.** Each PR should be one logical change.

Good PR sizes:
- Single feature or bug fix
- Refactor that preserves behavior
- Documentation update
- Test addition

Bad PR sizes:
- "Refactor everything"
- Large new features that touch many files
- Mixed concerns (new feature + refactor + tests)

If a task is too large, split it into multiple PRs.

## PR Process

1. **Create a branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make changes**:
   - Follow existing code style
   - Add tests for new behavior
   - Update docs for architectural changes
   - Run checks before pushing

3. **Test before push**:
   ```bash
   cargo fmt --check
   cargo check --workspace
   cargo test --workspace
   cd services/model-router && pytest tests
   ```

4. **Push and create PR**:
   ```bash
   git push origin feature/your-feature-name
   ```

5. **PR body format** (see below)

## PR Body Format

```markdown
## Summary
1-3 bullets describing what changed and why.

## Files Changed
- `path/to/file.rs` — Brief description
- `path/to/another.rs` — Brief description

## Tests Run
- `cargo fmt --check` — Passed
- `cargo check --workspace` — Passed
- `cargo test --workspace` — Passed (N tests)
- `pytest tests` (model-router) — Passed (M tests)
- `./scripts/osai-local-check` — Passed (if runtime running)

## Security Impact
What security boundaries were affected, if any.

## Runtime Impact
What runtime behavior changed, if any.

## Limitations
What is not yet implemented or tested.

## Follow-ups
What still needs to be done, if anything.
```

## What to Check Before Finishing

- [ ] Current behavior preserved (MVP loop works)
- [ ] `cargo test --workspace` passes
- [ ] `pytest tests` passes
- [ ] No secrets or local artifacts committed
- [ ] Receipts do not leak sensitive data
- [ ] CLI compatibility preserved
- [ ] Docs updated if architecture changed
- [ ] Computer-use safety design included if applicable

## Mock vs Local Validation

GitHub CI cannot run GPU/model tests. When your changes touch model/runtime code:

1. **Write mock-based tests** that don't require GPU:
   ```rust
   #[test]
   fn test_model_router_without_real_model() {
       // Test parsing, validation, error handling
       // Not actual model inference
   }
   ```

2. **Document local validation steps**:
   ```markdown
   ## Local Validation Required

   This change affects Model Router provider selection.
   Please run locally:

   ```bash
   ./scripts/osai-local-up
   ./scripts/osai-local-check
   cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI local validation OK"
   ```

   Expected: All checks pass, chat responds correctly.
   ```

## Handling Architecture Changes

If your PR changes the architecture (new components, changed boundaries):

1. **Update docs first**:
   - Update relevant doc files in `docs/`
   - Document the new architecture
   - Update diagrams if needed

2. **Make the code change**:
   - Follow the architecture described in docs
   - Ensure the boundary rules are maintained

3. **Add tests**:
   - Test the new interface
   - Test the boundary enforcement

4. **Verify no regression**:
   - Run full test suite
   - Verify MVP loop still works

## What Not to Change

Without explicit approval from the project owner, do not:

1. **Change the security model** — ToolBroker/ToolExecutor boundaries
2. **Remove receipt privacy** — Secrets must not be stored
3. **Bypass the CLI/API boundary** — UI must use osai-api, not shell out
4. **Commit secrets** — API keys, tokens, credentials
5. **Commit local models/runtimes** — `.local-models/`, `.local-runtimes/`
6. **Change the Plan DSL format** — Without versioning and migration
7. **Enable destructive tools** — FilesWrite, FilesDelete without security review

## Working with Code Owners

For significant changes, consider:
1. Opening a draft PR early for discussion
2. Asking clarifying questions about architecture
3. Documenting your understanding before implementing

## Branch Naming

Use descriptive branch names:
- `feature/osai-agent-core-extraction`
- `fix/receipt-secret-redaction`
- `docs/add-computer-use-design`
- `refactor/extract-chat-core`

Avoid:
- `work`
- `temp`
- `fixes`
- ` changes`

## Commit Messages

Follow conventional commits:
- `feat: add osai-agent-core extraction`
- `fix: correct receipt secret redaction for api_key field`
- `docs: add computer use strategy document`
- `refactor: move chat_core_async to osai-agent-core`

Format:
```
<type>: <short description>

<longer description if needed>
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`

## Handling Unsure Situations

If you're unsure about:
- Whether something is in scope — Ask before implementing
- How to implement something — Propose in PR description
- Whether a test is sufficient — Add more tests
- Whether docs need updating — Update them

When in doubt, ask. It's better to clarify before completing the work.

## Do Not Commit Checklist

Before any commit, verify:
- [ ] No API keys or tokens
- [ ] No model files (`.local-models/`)
- [ ] No runtime files (`.local-runtimes/`)
- [ ] No local receipt files
- [ ] No generated tokens in output
- [ ] No passwords or credentials
- [ ] No `.env` files with secrets
- [ ] No large log files