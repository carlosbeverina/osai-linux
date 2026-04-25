# OSAI Agent CLI

Command-line tool for working with OSAI Agent App manifests and Plan DSL files.

## Purpose

OSAI Agent CLI provides commands for:

- Validating and printing OSAI Plan DSL files
- Validating OSAI Policy files
- Listing and viewing receipt logs
- Initializing new OSAI agent directories

## Commands

### Plan Commands

#### Validate a plan
```bash
osai-agent plan validate <path>
```
Reads a YAML or JSON plan file, parses it using osai-plan-dsl, and validates it. Prints "Plan is valid" on success or the validation error on failure.

#### Print a plan
```bash
osai-agent plan print <path> --format json|yaml
```
Reads a plan file, validates it, and prints it in the specified format (JSON pretty or YAML).

### Policy Commands

#### Validate a policy
```bash
osai-agent policy validate <path>
```
Reads a YAML policy file and validates it using osai-toolbroker's ToolPolicy::from_yaml.

### Receipt Commands

#### List receipts
```bash
osai-agent receipt list <root_dir>
```
Lists all receipt JSON file paths in the specified directory, sorted ascending by filename.

#### Show a receipt
```bash
osai-agent receipt show <root_dir> <uuid>
```
Reads and prints a specific receipt by its UUID.

### Init Command

#### Initialize agent directory
```bash
osai-agent init <directory>
```
Creates a new OSAI agent directory with:
- `manifest.yml` - Agent manifest
- `agent.md` - Agent description
- `permissions.yml` - Permission configuration
- `README.md` - Usage documentation

Does not overwrite existing files.

## Example Agent Manifest

```yaml
name: my-agent
version: "0.1"
description: My OSAI agent
entrypoint: agent.md
permissions:
  - FilesList
  - FilesRead
memory:
  type: local
  scope: agent
model_policy: default
```

## Plan Validate Workflow

1. Create a plan file (plan.yml):
```yaml
version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Create project directory
actor: osai-agent
risk: Medium
approval: Ask
steps:
  - id: step-1
    action:
      type: FilesWrite
    description: Create the project directory
    requires_approval: true
    inputs:
      path: /home/user/project
metadata: {}
```

2. Validate the plan:
```bash
osai-agent plan validate plan.yml
# Output: Plan is valid
```

3. Print as JSON:
```bash
osai-agent plan print plan.yml --format json
```

## Exit Codes

- `0` - Success
- `non-zero` - Error (validation failure, file not found, etc.)
