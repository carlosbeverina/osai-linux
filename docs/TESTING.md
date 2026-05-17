# Testing

## Required Tests Before Every Change

```bash
# Rust workspace
cargo fmt --check
cargo check --workspace
cargo test --workspace

# Model Router
cd services/model-router && pytest tests

# Local runtime (requires services running)
./scripts/osai-local-check
```

## Do Not Proceed If Tests Fail

If any required test fails:
1. Stop
2. Fix the failure
3. Verify all tests pass
4. Then continue

## Rust Workspace Tests

### Format Check
```bash
cargo fmt --check
```
Must pass — ensures consistent code style.

### Type Check
```bash
cargo check --workspace
```
Must pass — catches type errors before they reach tests.

### Unit Tests
```bash
cargo test --workspace
```
Must pass — all 200+ tests across all crates.

Current expected test counts:
- osai-agent-cli: ~62 tests
- osai-agent-core: ~28 tests (runtime status tests)
- osai-api: ~53 tests (auth, requests, API mapping, path safety, UI serving)
- osai-plan-dsl: ~21 tests
- osai-receipt-logger: ~28 tests
- osai-tool-executor: ~20 tests
- osai-toolbroker: ~20 tests

## Model Router Tests

```bash
cd services/model-router
pytest tests
```

Currently ~64 tests covering providers, routing, receipts, normalization.

## Local Runtime Tests

```bash
./scripts/osai-local-up
./scripts/osai-local-check
./scripts/osai-local-down
```

Validates:
- llama.cpp /v1/models responds
- Model Router /health responds
- Model Router returns real (non-mock) responses
- osai-agent-cli tool run succeeds
- Tool receipts generated
- No obvious secrets in receipts

## MVP E2E Tests

The full MVP loop must work:

```bash
# 1. Chat
cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI E2E chat OK"

# 2. Ask / plan generation
cargo run -p osai-agent-cli -- ask --print-plan "Create a safe plan to list my Downloads folder"

# 3. Plan validation
PLAN=$(find /tmp -name "create-a-safe-*.yml" 2>/dev/null | sort | tail -1)
cargo run -p osai-agent-cli -- plan validate "$PLAN"

# 4. Apply dry-run
cargo run -p osai-agent-cli -- apply "$PLAN" --dry-run \
  --policy examples/policies/default-secure.yml \
  --allowed-root "$HOME/Downloads"

# 5. Apply real
cargo run -p osai-agent-cli -- apply "$PLAN" \
  --policy examples/policies/default-secure.yml \
  --allowed-root "$HOME/Downloads"
```

## Receipt Secret Scan

After any change that touches receipts:

```bash
grep -R -i "password\|api_key\|token\|secret\|credential\|Reply with exactly\|Create a safe plan" \
  ~/.local/share/osai/receipts/*/ \
  /tmp/osai-*/receipts/*/ \
  2>/dev/null || echo "No secrets or prompt text found"
```

If secrets or prompt text are found in receipts:
1. Fix the redaction logic
2. Add test for the redaction case
3. Re-run receipt secret scan
4. Do not proceed until clean

## GitHub CI Considerations

GitHub CI runners typically do not have:
- NVIDIA GPU / CUDA toolkit
- llama.cpp server running
- Model Router service
- Local model files (GGUF)

Therefore:
- **GPU/model runtime tests cannot run in CI** — Use mock tests
- **E2E tests require local validation** — Document steps for local validation
- **Unit tests can run in CI** — Rust tests, Python tests

For PRs that touch model/runtime code:
1. Write or update mock-based tests
2. Mark real hardware tests as "requires local validation"
3. Document local validation steps in PR body
4. Run `./scripts/osai-local-check` locally and report results

## Future Test Requirements

### osai-api Tests
- Health endpoint returns 200 without token
- Protected endpoints return 401 without token
- Protected endpoints return 401 with invalid token
- Protected endpoints return 200 with valid token
- Auth status endpoint returns token source without revealing token
- Token accepted via Authorization: Bearer
- Token accepted via X-OSAI-Token
- Token not echoed in error responses

### osai-agent-core Tests
- All CLI functions work via direct core calls
- No behavior change vs current CLI
- chat_core_async produces valid receipts
- ask_core_async produces valid plans
- run_apply produces per-step receipts

### Computer Use Tests (Phase 6)

When computer-use is implemented, required tests include:

**Visible Mode**:
- [ ] No action without ToolBroker authorization
- [ ] Receipts generated for all computer-use actions
- [ ] Screenshots captured and stored with privacy controls
- [ ] User can interrupt/cancel computer-use task
- [ ] Approval required for sensitive categories
- [ ] No credential entry without explicit user action

**Hidden Mode**:
- [ ] Isolated environment does not affect active desktop
- [ ] All actions logged in receipts
- [ ] Task can be cancelled
- [ ] Environment can be reset/destroyed
- [ ] Final outputs returned to user
- [ ] No hidden exfiltration of data
- [ ] Screenshots/artifacts under privacy controls
- [ ] User approval required for sensitive categories
- [ ] No uncontrolled shell/browser/file access

**General Computer Use**:
- [ ] Plan DSL computer-use tasks validated
- [ ] ToolBroker evaluates computer-use capabilities
- [ ] Receipts include computer-use mode (visible/hidden)
- [ ] No external transmission of artifacts without user review
- [ ] Network policies enforced
- [ ] Destructive actions require approval
- [ ] Task can be cancelled mid-execution

## Test File Locations

```
crates/
  osai-agent-cli/src/main.rs      # tests module at bottom
  osai-agent-core/src/lib.rs     # tests module at bottom
  osai-api/src/main.rs           # tests module at bottom
  osai-plan-dsl/src/lib.rs       # tests module at bottom
  osai-receipt-logger/src/lib.rs # tests module at bottom
  osai-tool-executor/src/lib.rs  # tests module at bottom
  osai-toolbroker/src/lib.rs     # tests module at bottom

services/model-router/tests/     # pytest test files
```

## Test Conventions

- Tests must be in the same crate/file as the code they test
- Test functions use `#[test]` or `#[tokio::test]`
- Mock tests are preferred over requiring real GPU/runtime
- Local validation steps documented when real hardware needed
- Receipt secret scan is part of the test convention
- Do not commit test outputs or receipt files