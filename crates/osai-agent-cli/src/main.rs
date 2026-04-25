//! OSAI Agent CLI - Command-line tool for OSAI Agent App manifests and Plan DSL files.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use osai_plan_dsl::OsaiPlan;
use osai_receipt_logger::ReceiptStore;
use osai_toolbroker::ToolPolicy;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// OSAI Agent CLI - Work with OSAI Agent App manifests and Plan DSL files.
#[derive(Parser)]
#[command(name = "osai-agent")]
#[command(about = "OSAI Agent CLI - Manage agent manifests and plan files", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Work with OSAI Plan DSL files.
    Plan {
        #[command(subcommand)]
        action: PlanCommands,
    },
    /// Work with OSAI Policy files.
    Policy {
        #[command(subcommand)]
        action: PolicyCommands,
    },
    /// Work with OSAI Receipts.
    Receipt {
        #[command(subcommand)]
        action: ReceiptCommands,
    },
    /// Initialize a new OSAI agent directory.
    Init {
        /// Directory to initialize.
        directory: PathBuf,
    },
}

#[derive(Subcommand)]
enum PlanCommands {
    /// Validate a plan file.
    Validate {
        /// Path to plan file (YAML or JSON).
        path: PathBuf,
    },
    /// Print a plan file in specified format.
    Print {
        /// Path to plan file (YAML or JSON).
        path: PathBuf,
        /// Output format.
        #[arg(value_enum, default_value_t = OutputFormat::Yaml)]
        format: OutputFormat,
    },
}

#[derive(Subcommand)]
enum PolicyCommands {
    /// Validate a policy file.
    Validate {
        /// Path to policy file (YAML).
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum ReceiptCommands {
    /// List all receipts in a directory.
    List {
        /// Root directory containing receipts.
        root_dir: PathBuf,
    },
    /// Show a specific receipt by UUID.
    Show {
        /// Root directory containing receipts.
        root_dir: PathBuf,
        /// UUID of the receipt.
        uuid: Uuid,
    },
}

#[derive(clap::ValueEnum, Clone, Default)]
enum OutputFormat {
    #[default]
    Yaml,
    Json,
}

fn read_plan(path: &PathBuf) -> Result<OsaiPlan> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read plan file: {}", path.display()))?;

    // Try YAML first, then JSON
    match OsaiPlan::from_yaml(&content) {
        Ok(plan) => Ok(plan),
        Err(_) => OsaiPlan::from_json(&content).with_context(|| {
            format!(
                "failed to parse plan file as YAML or JSON: {}",
                path.display()
            )
        }),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Plan { action } => match action {
            PlanCommands::Validate { path } => {
                let plan = read_plan(&path)?;
                plan.validate().map_err(|e| anyhow::anyhow!("{}", e))?;
                println!("Plan is valid");
                Ok(())
            }
            PlanCommands::Print { path, format } => {
                let plan = read_plan(&path)?;
                plan.validate().map_err(|e| anyhow::anyhow!("{}", e))?;

                match format {
                    OutputFormat::Yaml => {
                        println!("{}", plan.to_yaml().context("failed to serialize to YAML")?);
                    }
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            plan.to_json_pretty()
                                .context("failed to serialize to JSON")?
                        );
                    }
                }
                Ok(())
            }
        },
        Commands::Policy { action } => match action {
            PolicyCommands::Validate { path } => {
                let content = fs::read_to_string(&path)
                    .with_context(|| format!("failed to read policy file: {}", path.display()))?;
                ToolPolicy::from_yaml(&content).map_err(|e| anyhow::anyhow!("{}", e))?;
                println!("Policy is valid");
                Ok(())
            }
        },
        Commands::Receipt { action } => match action {
            ReceiptCommands::List { root_dir } => {
                let store = ReceiptStore::new(&root_dir);
                let paths = store.list().with_context(|| {
                    format!("failed to list receipts in: {}", root_dir.display())
                })?;
                for path in paths {
                    println!("{}", path.display());
                }
                Ok(())
            }
            ReceiptCommands::Show { root_dir, uuid } => {
                let store = ReceiptStore::new(&root_dir);
                let receipt = store.read(uuid).with_context(|| {
                    format!(
                        "failed to read receipt {} from: {}",
                        uuid,
                        root_dir.display()
                    )
                })?;
                println!(
                    "{}",
                    receipt
                        .to_json_pretty()
                        .context("failed to serialize receipt")?
                );
                Ok(())
            }
        },
        Commands::Init { directory } => {
            init_agent_directory(&directory)?;
            println!("Initialized OSAI agent in: {}", directory.display());
            Ok(())
        }
    }
}

fn init_agent_directory(directory: &PathBuf) -> Result<()> {
    // Create directory if it doesn't exist
    if !directory.exists() {
        fs::create_dir_all(directory)
            .with_context(|| format!("failed to create directory: {}", directory.display()))?;
    }

    let manifest_path = directory.join("manifest.yml");
    let agent_md_path = directory.join("agent.md");
    let permissions_path = directory.join("permissions.yml");
    let readme_path = directory.join("README.md");

    // manifest.yml
    if !manifest_path.exists() {
        let manifest = r#"name: my-agent
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
"#;
        fs::write(&manifest_path, manifest)
            .with_context(|| format!("failed to write: {}", manifest_path.display()))?;
    }

    // agent.md
    if !agent_md_path.exists() {
        let agent = r#"# My OSAI Agent

## Purpose
Describe what this agent does.

## Capabilities
- File operations (list, read)
- Memory operations
- Model chat

## Usage
Describe how to use this agent.
"#;
        fs::write(&agent_md_path, agent)
            .with_context(|| format!("failed to write: {}", agent_md_path.display()))?;
    }

    // permissions.yml
    if !permissions_path.exists() {
        let permissions = r#"allowed_actions:
  - FilesList
  - FilesRead
  - MemoryRead
  - ModelChat
denied_actions:
  - ShellRunSandboxed
  - FilesDelete
require_approval:
  - FilesWrite
  - FilesMove
"#;
        fs::write(&permissions_path, permissions)
            .with_context(|| format!("failed to write: {}", permissions_path.display()))?;
    }

    // README.md
    if !readme_path.exists() {
        let readme = r#"# My OSAI Agent

This is an OSAI agent.

## Files

- `manifest.yml` - Agent manifest
- `agent.md` - Agent description and capabilities
- `permissions.yml` - Permission configuration

## Usage

```bash
osai-agent plan validate manifest.yml
```
"#;
        fs::write(&readme_path, readme)
            .with_context(|| format!("failed to write: {}", readme_path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_init_creates_files() {
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path().join("my-agent");

        init_agent_directory(&dir).unwrap();

        assert!(dir.exists());
        assert!(dir.join("manifest.yml").exists());
        assert!(dir.join("agent.md").exists());
        assert!(dir.join("permissions.yml").exists());
        assert!(dir.join("README.md").exists());
    }

    #[test]
    fn test_init_does_not_overwrite() {
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path().join("my-agent");

        // Create directory and one file
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manifest.yml"), "existing content").unwrap();

        // Init should not overwrite
        init_agent_directory(&dir).unwrap();

        let content = fs::read_to_string(dir.join("manifest.yml")).unwrap();
        assert_eq!(content, "existing content");
    }

    #[test]
    fn test_init_creates_directory_if_missing() {
        let tempdir = tempfile::tempdir().unwrap();
        let dir = tempdir.path().join("new-agent");

        assert!(!dir.exists());
        init_agent_directory(&dir).unwrap();
        assert!(dir.exists());
    }

    #[test]
    fn test_plan_validate_valid() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Test Plan
actor: test-actor
risk: Low
approval: Auto
steps:
  - id: step-1
    action:
      type: FilesList
    description: List files
    requires_approval: false
    inputs: {}
metadata: {}
"#;
        fs::write(&plan_path, plan).unwrap();

        let result = read_plan(&plan_path);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.title, "Test Plan");
        assert!(parsed.validate().is_ok());
    }

    #[test]
    fn test_plan_validate_invalid() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: ""
actor: test-actor
risk: Low
approval: Auto
steps:
  - id: step-1
    action:
      type: FilesList
    description: List files
    requires_approval: false
    inputs: {}
metadata: {}
"#;
        fs::write(&plan_path, plan).unwrap();

        let result = read_plan(&plan_path);
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert!(parsed.validate().is_err());
    }

    #[test]
    fn test_policy_validate_valid() {
        let tempdir = tempfile::tempdir().unwrap();
        let policy_path = tempdir.path().join("policy.yml");

        let policy = r#"default_mode: Ask
action_modes:
  ModelChat: Allow
  DesktopNotify: Allow
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&policy_path, policy).unwrap();

        let content = fs::read_to_string(&policy_path).unwrap();
        let result = ToolPolicy::from_yaml(&content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_plan_print_yaml() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Test Plan
actor: test-actor
risk: Low
approval: Auto
steps:
  - id: step-1
    action:
      type: FilesList
    description: List files
    requires_approval: false
    inputs: {}
metadata: {}
"#;
        fs::write(&plan_path, plan).unwrap();

        let parsed = read_plan(&plan_path).unwrap();
        parsed.validate().unwrap();

        let yaml = parsed.to_yaml();
        assert!(yaml.is_ok());
        assert!(yaml.unwrap().contains("title: Test Plan"));
    }

    #[test]
    fn test_plan_print_json() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Test Plan
actor: test-actor
risk: Low
approval: Auto
steps:
  - id: step-1
    action:
      type: FilesList
    description: List files
    requires_approval: false
    inputs: {}
metadata: {}
"#;
        fs::write(&plan_path, plan).unwrap();

        let parsed = read_plan(&plan_path).unwrap();
        parsed.validate().unwrap();

        let json = parsed.to_json_pretty();
        assert!(json.is_ok());
        assert!(json.unwrap().contains("\"title\": \"Test Plan\""));
    }
}
