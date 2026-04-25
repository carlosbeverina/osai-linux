# OSAI Plan DSL

A safe typed intermediate representation for AI-generated plans before they are executed.

## What is OSAI Plan DSL?

The OSAI Plan DSL is a structured, validated representation for AI-generated execution plans. It defines what actions an AI agent intends to perform, along with metadata about risk, approval requirements, and rollback capabilities.

## Why a Typed Intermediate Plan?

Directly executing model output is unsafe. Models can generate harmful commands, misunderstand user intent, or produce actions with unintended side effects. The OSAI Plan DSL provides a safety layer:

1. **Structured representation** — Model output is parsed into a strongly-typed structure, not executed as raw commands
2. **Validation before execution** — Every plan is validated against security rules before any action is taken
3. **Human review** — High-risk actions require explicit user approval
4. **Audit trail** — Plans are stored as receipts, enabling full accountability
5. **Rollback capability** — Every plan can define a rollback strategy

**Flow:**
```
Natural language
  → model interpretation
  → OSAI Plan DSL (typed representation)
  → validation
  → simulation (optional)
  → user approval
  → ToolBroker execution
  → Receipt logged
```

## Validation Flow

1. Parse the YAML/JSON input into an `OsaiPlan` struct
2. Run `validate()` which checks:
   - Required fields are non-empty (version, title, actor)
   - Steps array is not empty
   - All step IDs are unique and non-empty
   - Critical risk requires Ask or AlwaysDeny approval
   - Destructive actions require approval
   - ShellRunSandboxed with network=true requires sandbox=true
   - Custom actions require approval
3. If validation fails, return a specific `PlanValidationError`
4. If validation passes, the plan is ready for execution

## Main Data Structures

### OsaiPlan
The root structure containing:
- `version` — DSL version string
- `id` — UUID identifying this plan
- `title` — Human-readable title
- `description` — Optional detailed description
- `actor` — Who/what is executing the plan
- `risk` — RiskLevel (Low, Medium, High, Critical)
- `approval` — ApprovalMode (Auto, Ask, AlwaysDeny)
- `steps` — Vector of PlanStep
- `rollback` — Optional RollbackPlan
- `metadata` — Additional key-value data

### PlanStep
A single action in a plan:
- `id` — Unique step identifier
- `action` — ActionKind enum variant
- `description` — What this step does
- `requires_approval` — Whether to prompt before execution
- `inputs` — Action-specific parameters

### ActionKind
The type of action to perform:
- `FilesList`, `FilesRead`, `FilesWrite`, `FilesMove`, `FilesDelete`
- `BrowserOpenUrl`, `DesktopNotify`
- `ShellRunSandboxed` — sandboxed shell execution
- `ModelChat`, `MemoryRead`, `MemoryWrite`, `ReceiptCreate`
- `Custom(String)` — user-defined action

### RiskLevel
- `Low` — read-only, non-destructive
- `Medium` — modifies state, easily reversible
- `High` — significant side effects
- `Critical` — requires explicit approval

### ApprovalMode
- `Auto` — execute without prompting
- `Ask` — prompt user for approval
- `AlwaysDeny` — never execute automatically

## Valid Example

```yaml
version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: "Create project directory"
description: "Creates a new project directory with standard structure"
actor: "osai-agent"
risk: Medium
approval: Ask
steps:
  - id: "step-1"
    action:
      type: FilesWrite
    description: "Create the project directory"
    requires_approval: true
    inputs:
      path: "/home/user/project"
      recursive: true
  - id: "step-2"
    action:
      type: FilesWrite
    description: "Create README file"
    requires_approval: true
    inputs:
      path: "/home/user/project/README.md"
      content: "# Project\n\nMy new project."
rollback:
  available: true
  steps:
    - id: "rollback-1"
      action:
        type: FilesDelete
      description: "Remove created directory"
      requires_approval: false
      inputs:
        path: "/home/user/project"
        recursive: true
metadata: {}
```

This plan passes validation because:
- version, title, and actor are non-empty
- steps array has unique, non-empty IDs
- FilesWrite actions have `requires_approval: true`
- Rollback plan is valid

## Invalid Example

```yaml
version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: "Delete system files"
actor: "osai-agent"
risk: Critical
approval: Auto
steps:
  - id: "step-1"
    action:
      type: FilesDelete
    description: "Delete tmp files"
    requires_approval: false
    inputs:
      path: "/tmp"
      recursive: true
metadata: {}
```

This plan **fails validation** because:

1. **Critical risk with Auto approval** — Critical risk plans must use `ApprovalMode::Ask` or `ApprovalMode::AlwaysDeny`, not `Auto`
2. **FilesDelete without approval** — The FilesDelete action has `requires_approval: false`, but destructive actions must require approval

## Usage

```rust
use osai_plan_dsl::{OsaiPlan, RiskLevel, ApprovalMode};

// Parse from YAML
let plan = OsaiPlan::from_yaml(yaml_str)?;

// Validate
plan.validate()?;

// Serialize to JSON
let json = plan.to_json_pretty()?;
```
