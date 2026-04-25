//! OSAI Agent CLI - Command-line tool for OSAI Agent App manifests and Plan DSL files.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use osai_plan_dsl::{OsaiPlan, PlanStep};
use osai_receipt_logger::ReceiptStore;
use osai_tool_executor::{ExecutionStatus, ToolExecutor};
use osai_toolbroker::{ToolBroker, ToolPolicy, ToolRequest};
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
    /// Work with OSAI Tool authorization.
    Tool {
        #[command(subcommand)]
        action: ToolCommands,
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

#[derive(Subcommand)]
enum ToolCommands {
    /// Authorize a plan against a policy (no execution).
    Authorize {
        /// Path to plan file (YAML or JSON).
        #[arg(long)]
        plan: PathBuf,
        /// Path to policy file (YAML).
        #[arg(long)]
        policy: PathBuf,
        /// Directory for receipts.
        #[arg(long)]
        receipts_dir: PathBuf,
    },
    /// Authorize and execute a plan against a policy.
    Run {
        /// Path to plan file (YAML or JSON).
        #[arg(long)]
        plan: PathBuf,
        /// Path to policy file (YAML).
        #[arg(long)]
        policy: PathBuf,
        /// Directory for receipts.
        #[arg(long)]
        receipts_dir: PathBuf,
        /// Allowed root directories for filesystem operations.
        #[arg(long)]
        allowed_root: Vec<PathBuf>,
        /// Approve a specific step ID for execution.
        #[arg(long)]
        approve: Vec<String>,
        /// Approve all steps that require user approval.
        #[arg(long)]
        approve_all: bool,
        /// Optional URL for the model router (must be loopback only).
        #[arg(long)]
        model_router_url: Option<String>,
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
        Commands::Tool { action } => match action {
            ToolCommands::Authorize {
                plan,
                policy,
                receipts_dir,
            } => {
                authorize_plan(&plan, &policy, &receipts_dir)?;
                Ok(())
            }
            ToolCommands::Run {
                plan,
                policy,
                receipts_dir,
                allowed_root,
                approve,
                approve_all,
                model_router_url,
            } => {
                run_plan(
                    &plan,
                    &policy,
                    &receipts_dir,
                    &allowed_root,
                    &approve,
                    approve_all,
                    model_router_url.as_deref(),
                )?;
                Ok(())
            }
        },
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

fn authorize_plan(
    plan_path: &PathBuf,
    policy_path: &PathBuf,
    receipts_dir: &PathBuf,
) -> Result<()> {
    // Read and parse plan
    let plan_content = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan file: {}", plan_path.display()))?;
    let plan = match OsaiPlan::from_yaml(&plan_content) {
        Ok(p) => p,
        Err(_) => OsaiPlan::from_json(&plan_content)
            .with_context(|| format!("failed to parse plan file: {}", plan_path.display()))?,
    };
    plan.validate()
        .map_err(|e| anyhow::anyhow!("plan validation failed: {}", e))?;

    // Read and parse policy
    let policy_content = fs::read_to_string(policy_path)
        .with_context(|| format!("failed to read policy file: {}", policy_path.display()))?;
    let policy = ToolPolicy::from_yaml(&policy_content)
        .map_err(|e| anyhow::anyhow!("policy parse failed: {}", e))?;

    // Create store and broker
    let store = ReceiptStore::new(receipts_dir);
    store.ensure_dirs().with_context(|| {
        format!(
            "failed to create receipts directory: {}",
            receipts_dir.display()
        )
    })?;
    let broker = ToolBroker::new(policy, store);

    // Authorize each step
    let mut any_denied = false;

    for step in &plan.steps {
        let request = step_to_request(&plan, step);

        let decision = broker
            .authorize(&request)
            .with_context(|| format!("authorization failed for step: {}", step.id))?;

        // Print decision line
        let action_name = request.action_name();
        println!(
            "step={} action={} allowed={} approval={} mode={:?} reason=\"{}\"",
            step.id,
            action_name,
            decision.allowed,
            decision.requires_user_approval,
            decision.policy_mode,
            decision.reason
        );

        if !decision.allowed {
            any_denied = true;
        }
    }

    if any_denied {
        Err(anyhow::anyhow!(
            "authorization failed: one or more steps were denied"
        ))
    } else {
        Ok(())
    }
}

fn run_plan(
    plan_path: &PathBuf,
    policy_path: &PathBuf,
    receipts_dir: &PathBuf,
    allowed_roots: &[PathBuf],
    approve: &[String],
    approve_all: bool,
    model_router_url: Option<&str>,
) -> Result<()> {
    // Read and parse plan
    let plan_content = fs::read_to_string(plan_path)
        .with_context(|| format!("failed to read plan file: {}", plan_path.display()))?;
    let plan = match OsaiPlan::from_yaml(&plan_content) {
        Ok(p) => p,
        Err(_) => OsaiPlan::from_json(&plan_content)
            .with_context(|| format!("failed to parse plan file: {}", plan_path.display()))?,
    };
    plan.validate()
        .map_err(|e| anyhow::anyhow!("plan validation failed: {}", e))?;

    // Read and parse policy
    let policy_content = fs::read_to_string(policy_path)
        .with_context(|| format!("failed to read policy file: {}", policy_path.display()))?;
    let policy = ToolPolicy::from_yaml(&policy_content)
        .map_err(|e| anyhow::anyhow!("policy parse failed: {}", e))?;

    // Create store, broker, and executor
    let store = ReceiptStore::new(receipts_dir);
    store.ensure_dirs().with_context(|| {
        format!(
            "failed to create receipts directory: {}",
            receipts_dir.display()
        )
    })?;
    let broker = ToolBroker::new(policy.clone(), store.clone());

    // Build ToolExecutor with optional model router URL
    let mut executor = ToolExecutor::new(store, allowed_roots.to_vec());
    if let Some(url) = model_router_url {
        executor = executor
            .with_model_router_url(url)
            .map_err(|e| anyhow::anyhow!("invalid model router URL: {}", e))?;
    }

    // Authorize and execute each step
    let mut any_denied = false;
    let mut any_failed = false;
    let approve_set: std::collections::HashSet<&str> = approve.iter().map(|s| s.as_str()).collect();

    for step in &plan.steps {
        let request = step_to_request(&plan, step);

        let decision = broker
            .authorize(&request)
            .with_context(|| format!("authorization failed for step: {}", step.id))?;

        // Print authorization decision
        let action_name = request.action_name();
        println!("step={}", step.id);
        println!(
            "authorization: allowed={} approval={} mode={:?} reason=\"{}\"",
            decision.allowed,
            decision.requires_user_approval,
            decision.policy_mode,
            decision.reason
        );

        if !decision.allowed {
            println!(
                "execution: status={} action={} error=\"Action denied\"",
                "Skipped", action_name
            );
            any_denied = true;
            continue;
        }

        // Check if this step requires approval and if we've approved it
        if decision.requires_user_approval {
            let is_approved = approve_all || approve_set.contains(step.id.as_str());

            if is_approved {
                // Create adjusted decision for explicit CLI approval
                let adjusted_decision = osai_toolbroker::AuthorizationDecision {
                    allowed: true,
                    requires_user_approval: false,
                    reason: format!("Explicitly approved by CLI: {}", decision.reason),
                    policy_mode: decision.policy_mode,
                    request_id: decision.request_id,
                };

                println!("approval: source=cli step={}", step.id);

                // Execute with adjusted decision
                let result = executor
                    .execute_authorized(&request, &adjusted_decision)
                    .with_context(|| format!("execution failed for step: {}", step.id))?;

                let exec_status = match result.status {
                    ExecutionStatus::Executed => "Executed",
                    ExecutionStatus::Failed => "Failed",
                    ExecutionStatus::Skipped => "Skipped",
                };
                println!(
                    "execution: status={} action={} error=\"{}\"",
                    exec_status,
                    action_name,
                    result.error.unwrap_or_default()
                );

                if result.status == ExecutionStatus::Failed {
                    any_failed = true;
                }
            } else {
                println!("execution: status={} action={} error=\"Execution skipped: requires user approval\"", "Skipped", action_name);
            }
            continue;
        }

        // Execute the authorized request
        let result = executor
            .execute_authorized(&request, &decision)
            .with_context(|| format!("execution failed for step: {}", step.id))?;

        // Print execution result
        let exec_status = match result.status {
            ExecutionStatus::Executed => "Executed",
            ExecutionStatus::Failed => "Failed",
            ExecutionStatus::Skipped => "Skipped",
        };
        println!(
            "execution: status={} action={} error=\"{}\"",
            exec_status,
            action_name,
            result.error.unwrap_or_default()
        );

        if result.status == ExecutionStatus::Failed {
            any_failed = true;
        }
    }

    if any_denied {
        Err(anyhow::anyhow!(
            "authorization failed: one or more steps were denied"
        ))
    } else if any_failed {
        Err(anyhow::anyhow!(
            "execution failed: one or more steps failed"
        ))
    } else {
        Ok(())
    }
}

fn step_to_request(plan: &OsaiPlan, step: &PlanStep) -> ToolRequest {
    let mut request = ToolRequest::new(&plan.actor, step.action.clone(), &step.description)
        .with_plan_id(plan.id)
        .with_step_id(&step.id)
        .with_inputs(step.inputs.clone())
        .with_risk(plan.risk);

    // Set request ID to link receipt to step
    request.id = Uuid::new_v4();

    request
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

    #[test]
    fn test_authorize_valid_plan_writes_receipts() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");

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
  - id: step-2
    action:
      type: ModelChat
    description: Chat with model
    requires_approval: false
    inputs: {}
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  FilesList: Allow
  ModelChat: Allow
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        let result = authorize_plan(&plan_path, &policy_path, &receipts_dir);
        assert!(result.is_ok());

        // Check receipts were written
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 2); // One receipt per step
    }

    #[test]
    fn test_authorize_risky_shell_denied() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Risky Plan
actor: test-actor
risk: Critical
approval: Ask
steps:
  - id: step-1
    action:
      type: ShellRunSandboxed
    description: Run shell with network
    requires_approval: true
    inputs:
      command: "curl https://evil.com"
      network: true
      sandbox: false
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  ShellRunSandboxed: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        let result = authorize_plan(&plan_path, &policy_path, &receipts_dir);
        // Should fail because shell_requires_sandbox=true and sandbox=false
        assert!(result.is_err());
    }

    #[test]
    fn test_authorize_creates_receipt_per_step() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");

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
  - id: step-2
    action:
      type: FilesList
    description: List files again
    requires_approval: false
    inputs: {}
metadata: {}
"#;
        let policy = r#"default_mode: Allow
action_modes: {}
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        let result = authorize_plan(&plan_path, &policy_path, &receipts_dir);
        assert!(result.is_ok());

        // Check we have one receipt per step
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_run_safe_plan_executes_files_list() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Safe Plan
actor: test-actor
risk: Low
approval: Auto
steps:
  - id: step-1
    action:
      type: FilesList
    description: List files
    requires_approval: false
    inputs:
      path: /tmp
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  FilesList: Allow
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &[],
            false,
            None,
        );
        assert!(result.is_ok());

        // Check receipts were written (2: one from broker.authorize + one from executor)
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_run_files_move_skips_due_to_approval() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Move Plan
actor: test-actor
risk: Medium
approval: Ask
steps:
  - id: step-1
    action:
      type: FilesMove
    description: Move file
    requires_approval: true
    inputs:
      source: /tmp/a
      destination: /tmp/b
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  FilesMove: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &[],
            false,
            None,
        );
        // Should succeed (not error) but skip execution due to approval requirement
        assert!(result.is_ok());

        // Check receipts were written (skipped, not executed)
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn test_run_risky_shell_denied_exits_nonzero() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Risky Plan
actor: test-actor
risk: Critical
approval: Ask
steps:
  - id: step-1
    action:
      type: ShellRunSandboxed
    description: Run shell with network
    requires_approval: true
    inputs:
      command: "curl https://evil.com"
      network: true
      sandbox: false
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  ShellRunSandboxed: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &[],
            false,
            None,
        );
        // Should fail because shell_requires_sandbox=true and sandbox=false
        assert!(result.is_err());
    }

    #[test]
    fn test_run_writes_receipts() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Test Plan
actor: test-actor
risk: Low
approval: Auto
steps:
  - id: step-1
    action:
      type: DesktopNotify
    description: Send notification
    requires_approval: false
    inputs:
      title: Hello
      body: World
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  DesktopNotify: Allow
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &[],
            false,
            None,
        );
        assert!(result.is_ok());

        // Check receipts were written (2: one from broker.authorize + one from executor)
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_run_with_approve_skips_files_move() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Move Plan
actor: test-actor
risk: Medium
approval: Ask
steps:
  - id: step-1
    action:
      type: FilesMove
    description: Move file
    requires_approval: true
    inputs:
      source: /tmp/a
      destination: /tmp/b
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  FilesMove: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        // Without approval, should skip
        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &[],
            false,
            None,
        );
        assert!(result.is_ok());

        // Check receipts were written (only authorization, no execution)
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn test_run_with_approve_step_executes() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Move Plan
actor: test-actor
risk: Medium
approval: Ask
steps:
  - id: step-1
    action:
      type: FilesMove
    description: Move file
    requires_approval: true
    inputs:
      source: /tmp/a
      destination: /tmp/b
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  FilesMove: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        // With approval, should reach executor but executor refuses FilesMove
        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &["step-1".to_string()],
            false,
            None,
        );
        // Executor refuses FilesMove in v0.1, so this fails
        assert!(result.is_err());

        // Check receipts were written (authorization + failed execution attempt)
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_run_approve_all() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Multi Step Plan
actor: test-actor
risk: Medium
approval: Ask
steps:
  - id: step-1
    action:
      type: FilesMove
    description: Move file 1
    requires_approval: true
    inputs:
      source: /tmp/a
      destination: /tmp/b
  - id: step-2
    action:
      type: FilesMove
    description: Move file 2
    requires_approval: true
    inputs:
      source: /tmp/c
      destination: /tmp/d
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  FilesMove: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        // With approve_all, both steps should reach executor
        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &[],
            true,
            None,
        );
        // Executor refuses FilesMove in v0.1, so this fails
        assert!(result.is_err());

        // Check receipts were written (authorization + failed execution for each step)
        let store = ReceiptStore::new(&receipts_dir);
        let paths = store.list().unwrap();
        assert_eq!(paths.len(), 4); // 2 auth + 2 failed execution
    }

    #[test]
    fn test_run_approved_unsupported_action_fails() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Unsupported Plan
actor: test-actor
risk: Low
approval: Ask
steps:
  - id: step-1
    action:
      type: BrowserOpenUrl
    description: Open URL
    requires_approval: true
    inputs:
      url: https://example.com
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  BrowserOpenUrl: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        // Even with approval, BrowserOpenUrl is not executable in v0.1
        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &["step-1".to_string()],
            false,
            None,
        );
        // Executor refuses BrowserOpenUrl in v0.1, so this fails
        assert!(result.is_err());
    }

    #[test]
    fn test_run_approve_all_does_not_override_denied() {
        let tempdir = tempfile::tempdir().unwrap();
        let plan_path = tempdir.path().join("plan.yml");
        let policy_path = tempdir.path().join("policy.yml");
        let receipts_dir = tempdir.path().join("receipts");
        let allowed_root = vec![std::path::PathBuf::from("/tmp")];

        // ShellRunSandboxed with network=true and sandbox=false should be denied
        let plan = r#"version: "0.1"
id: "550e8400-e29b-41d4-a716-446655440000"
title: Risky Plan
actor: test-actor
risk: Critical
approval: Ask
steps:
  - id: step-1
    action:
      type: ShellRunSandboxed
    description: Run shell with network
    requires_approval: true
    inputs:
      command: "curl https://evil.com"
      network: true
      sandbox: false
metadata: {}
"#;
        let policy = r#"default_mode: Ask
action_modes:
  ShellRunSandboxed: Ask
allowed_roots: []
shell_network_allowed: false
shell_requires_sandbox: true
"#;
        fs::write(&plan_path, plan).unwrap();
        fs::write(&policy_path, policy).unwrap();

        // Even with approve_all, denied actions should still fail
        let result = run_plan(
            &plan_path,
            &policy_path,
            &receipts_dir,
            &allowed_root,
            &[],
            true,
            None,
        );
        // Should fail because shell_requires_sandbox=true and sandbox=false
        assert!(result.is_err());
    }
}
