# Release and CI Design

## Purpose

Defines the CI strategy for OSAI: Rust tests, Python Model Router tests, mock vs real GPU validation, image build validation, release artifacts, checksums/signatures, PR requirements, and local validation requirements.

This is a design document. It does not implement the CI pipeline.

## Goals

- Every PR runs a fast, deterministic test suite in CI.
- GPU-dependent tests are not run in CI; they are documented and run on local validation hardware.
- Release artifacts are reproducible, signed, and checksummed.
- A tester can verify a release artifact matches its source commit.
- PR requirements are clear and minimal.

## Non-Goals

- A self-hosted CI runner farm. CI runs on GitHub Actions.
- Cross-platform CI. OSAI Linux is built on Fedora Atomic; other distros are not in v1.
- Continuous deployment. Releases are tagged; users opt in via the update channel.

## CI Strategy

The CI pipeline runs on every PR and on every push to `master`:

### Stage 1: Fast Lint and Type Check (parallel)

- `cargo fmt --check` -- format check.
- `cargo check --workspace` -- type check.
- `cargo clippy --workspace --all-targets -- -D warnings` -- lint.
- `markdownlint docs/**/*.md` -- markdown lint (if configured).
- `cargo deny check` -- dependency license and advisory check.

### Stage 2: Rust Unit and Integration Tests (parallel)

- `cargo test --workspace` -- all Rust tests.

### Stage 3: Python Model Router Tests

- `cd services/model-router && pip install -r requirements.txt && pytest tests`

### Stage 4: Smoke Test (Mock GPU)

- `cargo run -p osai-agent-cli -- chat "Reply with exactly: CI OK"` -- uses mock model.
- `cargo run -p osai-agent-cli -- doctor --mock` -- validates API + Model Router in mock mode.

### Stage 5: Image Build (master only)

- Triggered on push to `master` and on tagged releases.
- Builds the BlueBuild recipe.
- Uploads the resulting image to a registry (e.g., `ghcr.io`).
- Verifies the image boots in a QEMU VM (smoke test).
- Runs `osai-cli doctor` inside the VM.
- Runs the OOBE flow inside the VM with a fake model.

### Stage 6: Release (tagged releases only)

- Triggered on a release tag (`v*.*.*`).
- Builds signed release artifacts (see below).
- Publishes a GitHub Release with checksums and signatures.
- Updates the auto-update channel.

### CI Caching

- Cargo target dir: cached between runs.
- pip cache: cached.
- llama.cpp build: not cached (large; built once per release).

## Mock vs Local GPU Validation

CI cannot run GPU-dependent tests. The strategy is:

| Test type | Where run | Notes |
|---|---|---|
| Rust unit tests | CI | No GPU needed |
| Rust integration tests | CI | Use mock Model Router |
| Python Model Router tests | CI | No GPU needed |
| Mock chat E2E | CI | Mock model returns canned responses |
| Real model E2E | Local validation | Requires GPU; not in CI |
| Real `osai-cli doctor` | Local validation | Requires llama-server |
| OOBE | Local validation | Requires a VM or hardware |
| Computer-use | Local validation | Requires display + permissions |

### Mock Mode

The Model Router and `osai-cli` support a mock mode:

- `OSAI_LOCAL_MOCK=true` -- the Model Router returns canned responses for `chat` and `ask`.
- `osai-cli chat --mock` -- uses mock responses locally.
- `osai-cli doctor --mock` -- checks API + Model Router mock mode.

Mock mode is for development and CI. Production runs use real models.

### Local Validation

Local validation runs on the project's validated hardware (RTX 4060 Laptop 8GB VRAM). It is required for:

- Model changes (especially default model).
- ToolBroker changes.
- ToolExecutor changes.
- osai-agent-core changes.
- osai-api changes.
- Plan DSL changes.
- Receipt format changes.

Local validation procedure:

1. Run `./scripts/osai-local-up` -- starts llama-server + Model Router.
2. Run `cargo run -p osai-agent-cli -- chat "Reply with exactly: OSAI local validation OK"`.
3. Run `cargo run -p osai-agent-cli -- ask --print-plan "Create a safe plan to list my Downloads folder"`.
4. Run `cargo run -p osai-agent-cli -- plan validate <generated-plan.yml>`.
5. Run `cargo run -p osai-agent-cli -- apply <generated-plan.yml> --dry-run --policy examples/policies/default-secure.yml --allowed-root "$HOME/Downloads"`.
6. Run `cargo run -p osai-agent-cli -- apply <generated-plan.yml> --policy examples/policies/default-secure.yml --allowed-root "$HOME/Downloads"`.
7. Run the receipt secret scan.
8. Run `cargo test --workspace` (full suite).
9. Run `cd services/model-router && pytest tests`.

The PR body MUST include the local validation results if the change affects any of the listed areas.

## Image Build Validation

When a BlueBuild recipe is built:

1. The image is built in a clean container.
2. The image is booted in a QEMU VM (or `podman machine`).
3. The OOBE flow runs with a fake model (mock mode).
4. `osai-cli doctor` reports OK.
5. The image is tagged with the commit SHA.
6. The image is uploaded to the registry.

Image build is gated on `master` and on release tags.

## Release Artifacts

A release produces:

| Artifact | Description | Signed? | Checksummed? |
|---|---|---|---|
| OSAI image (`.iso` or OCI image) | The installable image | yes (cosign) | yes (SHA256) |
| Source tarball | The full source for the release | yes (GPG) | yes (SHA256) |
| SBOM | Software bill of materials (SPDX format) | yes (cosign) | yes (SHA256) |
| Release notes | Human-readable changes | n/a | n/a |
| Validation report | Local validation results | n/a | yes (SHA256) |

### Checksums and Signatures

- Each artifact has a `SHA256SUMS` file.
- Each artifact is signed with `cosign` (image) or `gpg --detach-sign` (source tarball).
- The signing keys are stored in a secure location (e.g., GitHub secrets or a hardware key).
- Verification procedure is documented in `CONTRIBUTING.md`.

### Reproducibility

The image build is reproducible from the source:

- Same commit + same recipe = same image.
- The build is hermetic (no network during build, except for fetching pinned dependencies).
- The image's SHA256 hash is published with the release.

A tester can:

1. Download the source tarball.
2. Build the image with `just build-image` (or equivalent).
3. Compare the resulting image's SHA256 to the published hash.
4. If they match, the build is verified.

## PR Requirements

Every PR MUST:

- Pass all CI stages (lint, type check, unit tests, mock E2E).
- Be reviewed by at least one maintainer.
- Include a PR body with: summary, files changed, tests run, security impact, runtime impact, follow-ups.
- Not introduce any `unsafe` Rust unless explicitly justified.
- Not add new dependencies without justification.
- Not modify locked files (`Cargo.lock`, `package-lock.json`) without justification.
- Not introduce TODOs without a tracking issue.

PRs that touch certain areas MUST additionally:

- Pass local validation (see above).
- Update relevant docs.
- Add or update tests for the changed behavior.

The "certain areas" are listed in the local validation section above.

## Branch Strategy

- `master` -- always green, always deployable.
- `feature/<name>` -- new features, branched from `master`.
- `fix/<name>` -- bug fixes, branched from `master`.
- `docs/<name>` -- documentation only, branched from `master`.
- `release/<version>` -- release prep, branched from `master`.

PRs target `master`. Releases are cut from `master` to a `release/<version>` branch and tagged.

## Release Cadence

- Minor versions (e.g., `0.2.0`): every 4-8 weeks.
- Patch versions (e.g., `0.2.1`): as needed for security or critical bugs.
- Major versions (e.g., `1.0.0`): when OSAI is ready for production.

## Update Channel Mapping

| Channel | Source | Auto-update? |
|---|---|---|
| `stable` | `v*.*.*` tags | opt-in |
| `beta` | `release/*` branches | opt-in |
| `nightly` | `master` commits | opt-in |

## Open Questions

- Should we use GitHub's `pull_request_target` for CI? (Open: no; use `pull_request` to avoid privilege escalation.)
- Should we run a self-hosted runner for image build? (Open: yes, when volume justifies; defer to v2.)
- Should we publish to Flathub or similar? (Open: yes; in v2; OSAI is a Linux distribution, not a Flatpak, but the runtime components could be Flatpaks.)
- Should we support reproducible builds at the binary level? (Open: yes; Rust supports this; we document the procedure.)

## Acceptance Criteria

This design is accepted when:

- A coder can add a new Rust test and it runs in CI.
- A coder can add a new Python test and it runs in CI.
- A maintainer can cut a release by tagging `master`.
- A tester can verify a release artifact's SHA256 against the source build.
- A reviewer can confirm a PR's local validation results match the change's risk.
- A tester can see CI results on every PR.
