# Security Review Checklist for Desktop and Distro

## Purpose

Defines the security review checklist for OSAI Linux: distro-level security, desktop integration, services, model/runtime, computer-use, installer/OOBE, update/rollback, and release. It is a hard blocker before enabling destructive or external actions.

This is a design document. It is the contract between the security review and the implementation.

## Goals

- Every OSAI subsystem has a security review checklist.
- The checklist is enforced before any release that includes the subsystem.
- The checklist produces a go/no-go decision.
- A tester can verify each item.

## Non-Goals

- A formal threat model document. (See `SECURITY_MODEL.md` for the threat model.)
- A penetration test report. (Performed separately by an external party.)
- A CVE feed integration. (Performed by the security team, not the codebase.)

## Distro-Level Security Review

The base image and image build process must pass:

- [ ] The base image is signed with cosign.
- [ ] The build is reproducible (same source = same image SHA256).
- [ ] The base image has no known CVEs at release time (`rpm-ostree upgrade --preview` and `cargo deny`).
- [ ] SELinux is enforcing.
- [ ] All system services run with `ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateTmp=yes`.
- [ ] All system services that need network bind to loopback only.
- [ ] No service runs as root unless it must.
- [ ] The build pipeline does not embed secrets, tokens, or model files.
- [ ] SBOM is generated and signed.
- [ ] Image hash is published with the release.

## Desktop Integration Security Review

The desktop shell integration must pass:

- [ ] The shell component (Plasmoid / COSMIC applet) only calls `osai-api` over loopback with the per-user API token.
- [ ] The shell component does not shell out to `osai-cli` or any other binary.
- [ ] The shell component does not write to `~/.config/osai/` directly; all writes go through `osai-api`.
- [ ] The global hotkey daemon does not intercept input from secure contexts (e.g., password fields).
- [ ] Notifications do not include sensitive content (no prompts, no file paths from allowed_roots, no model outputs).
- [ ] The status indicators do not leak model or task names to other applications.
- [ ] The Wayland portal permissions are documented and re-prompted on use.
- [ ] The shell component is sandboxed (firejail or systemd unit hardening).

## Service Security Review

Each OSAI service must pass:

- [ ] Binds to `127.0.0.1` only.
- [ ] Uses a dedicated systemd user (no root for user services).
- [ ] `NoNewPrivileges=yes`.
- [ ] `ProtectSystem=strict`, `ProtectHome=read-only`, `PrivateTmp=yes`.
- [ ] `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6` (drop AF_NETLINK if not needed).
- [ ] `SystemCallArchitectures=native`.
- [ ] `SystemCallFilter=@system-service` (tighten further where possible).
- [ ] `MemoryDenyWriteExecute=yes` (drop if incompatible with deps).
- [ ] `RestrictNamespaces=yes` for non-container services.
- [ ] `RestrictRealtime=yes`.
- [ ] Resource limits via `MemoryMax`, `CPUQuota`, `IOWeight`.
- [ ] Logs go to journald only; no log files with world-readable permissions.
- [ ] Secrets are loaded via `LoadCredential` or environment variable, not files in `/etc/`.
- [ ] Service refuses to start with misconfiguration (e.g., binding to non-loopback).

## Model and Runtime Security Review

The model runtime (llama.cpp, Model Router) must pass:

- [ ] llama-server binds to `127.0.0.1` only.
- [ ] llama-server is started with `mlock` to prevent model swap.
- [ ] The model file is verified by SHA256 before load.
- [ ] The model file is read-only after download.
- [ ] The model file is stored outside the repository (`~/.local/share/osai/models/` or `/var/cache/osai/models/`).
- [ ] The Model Router binds to `127.0.0.1` only.
- [ ] The Model Router enforces the privacy mode (local_only, cloud_fallback, cloud_only).
- [ ] The Model Router strips request bodies from cloud receipts.
- [ ] The Model Router never logs API keys or tokens.
- [ ] Cloud credentials are stored with `chmod 0600` and never logged.
- [ ] The default model is a known-validated model (e.g., `gemma-4-e2b-it-q8_0`).
- [ ] Promoting a new model to `validated_default` requires passing `TESTING.md` validation on target hardware.

## Computer-Use Security Review

The computer-use subsystem must pass (see also `COMPUTER_USE_IMPLEMENTATION_SPEC.md`):

- [ ] Every computer-use task starts as a Plan DSL plan.
- [ ] Plan DSL validation rejects malformed plans.
- [ ] ToolBroker authorization is required for every action.
- [ ] Visible mode operates in the user's active Wayland session, with explicit user awareness.
- [ ] Hidden mode uses a nested Wayland compositor or virtual display.
- [ ] Hidden mode cannot access the user's active session.
- [ ] No credential entry without explicit user approval.
- [ ] No payments without explicit user approval.
- [ ] No destructive changes without explicit user approval.
- [ ] No external communication without explicit user approval.
- [ ] Screenshots are redacted before storage.
- [ ] No screenshot pixel data is transmitted externally without explicit user review.
- [ ] Network policies are enforced at the firewall level.
- [ ] File policies are enforced at the ToolBroker level.
- [ ] Browser policies are enforced at the Playwright level.
- [ ] Hidden environments are resettable and destroyable.
- [ ] Hidden tasks are cancellable at any time.
- [ ] The artifact store respects retention policies.
- [ ] Receipts do not contain credentials, full prompts, or unredacted screenshot pixels.
- [ ] Sensitive categories (see list below) are ALWAYS approved by the user, regardless of policy.

### Sensitive Categories (Always Require Approval)

- Credentials (passwords, API keys, 2FA codes, OAuth tokens).
- Payments (purchase, transfer, subscription, donation).
- Account changes (email, social media, banking).
- Destructive file operations (delete, move to trash, overwrite).
- External communication (email, chat, social posting).
- Browser-based credential or payment entry.

## Installer and OOBE Security Review

The installer and OOBE must pass:

- [ ] The installer never silently modifies existing partitions.
- [ ] The installer requires explicit user confirmation before writing the partition table.
- [ ] The installer detects BitLocker and refuses to proceed without disabling it.
- [ ] The installer supports LUKS full-disk encryption (default ON for laptops).
- [ ] The installer never stores the LUKS passphrase in plaintext.
- [ ] The OOBE generates a per-user API token with `chmod 0600`.
- [ ] The OOBE never logs the API token.
- [ ] The OOBE never writes cloud credentials without explicit opt-in.
- [ ] The OOBE validates the model download checksum before use.
- [ ] The OOBE runs in a confined environment (no network for unverified downloads).
- [ ] The OOBE state is stored in `/var/lib/osai/oobe-state.toml` with `chmod 0644`.
- [ ] The installer writes an install receipt to `/var/log/osai-install/`.

## Update and Rollback Security Review

The update and rollback flow must pass:

- [ ] OS updates are signed and verified by rpm-ostree.
- [ ] OSAI component updates are signed.
- [ ] Model updates are checksummed against the catalog.
- [ ] Rollback is state-preserving (no data loss).
- [ ] Old receipts remain readable across updates.
- [ ] Old settings remain readable across schema migrations.
- [ ] Update channels are clearly labeled (stable, beta, nightly).
- [ ] Automatic updates never reboot automatically.
- [ ] The user can opt out of automatic updates.

## Release Security Review

Every release must pass:

- [ ] All CI stages pass.
- [ ] All local validation passes.
- [ ] The release artifact is signed with cosign (image) or GPG (source).
- [ ] SHA256 checksums are published.
- [ ] SBOM is published.
- [ ] No secrets, tokens, or credentials are in the artifact.
- [ ] No local model files are in the artifact.
- [ ] No local runtime files are in the artifact.
- [ ] Release notes describe security-relevant changes.
- [ ] CVE feed is checked; no new high or critical CVEs.

## Explicit Blockers Before Enabling Destructive or External Actions

The following actions MUST NOT be enabled in any release until the corresponding security review item is checked:

- FilesWrite real execution: distro, services, computer-use, release items must pass.
- FilesMove real execution: same.
- FilesDelete real execution: same.
- BrowserOpenUrl real execution: computer-use items must pass.
- ShellRunSandboxed real execution: services, release items must pass.
- ComputerUseVisible: desktop, services, computer-use, release items must pass.
- ComputerUseHidden: all computer-use items must pass.
- Cloud fallback: services, model/runtime, release items must pass.
- Voice input/output: services, release items must pass.
- Auto-update enable: update/rollback, release items must pass.

## Review Sign-Off

A security review produces a sign-off document with:

- Date of review.
- Reviewer name.
- Subsystems reviewed.
- Items checked.
- Items deferred (with reason).
- Items failed (with blocker).
- Sign-off status: APPROVED / APPROVED-WITH-CAVEATS / BLOCKED.

The sign-off is stored in `/var/log/osai-security-reviews/` (system) or referenced in the release notes.

## Continuous Security Practices

Beyond the checklist:

- Every PR runs `cargo deny check` for license and advisory.
- Every PR runs `cargo audit` for known Rust CVEs.
- A weekly scan of the base image for CVEs.
- An external penetration test before each minor release.
- A responsible disclosure policy (see `SECURITY_MODEL.md`).
- A security mailing list for reports.

## Open Questions

- Should we have a formal bug-bounty program? (Open: defer to v2.)
- Should we support a "security review only" build profile? (Open: yes; defer.)
- Should we have a security advisory database for users to subscribe to? (Open: yes; GitHub Security Advisories.)
- Should we have a security key (e.g., YubiKey) requirement for release signing? (Open: yes; required for releases, optional for day-to-day.)

## Acceptance Criteria

This checklist is accepted when:

- A reviewer can use the checklist to perform a security review.
- The checklist produces a sign-off.
- The sign-off is required for release.
- The checklist is updated when new subsystems are added.
- The checklist is enforced in CI where possible (e.g., `cargo deny`, `cargo audit`).
