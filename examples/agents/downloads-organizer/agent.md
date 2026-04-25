# Downloads Organizer Agent

## Purpose

This agent helps users organize their ~/Downloads folder by automatically categorizing and moving files into appropriate subdirectories like Pictures, Documents, and Videos.

## How It Works

The agent operates in these steps:

1. **Scan Downloads** - Lists all files in ~/Downloads to understand what needs organizing
2. **Categorize** - Identifies file types (images go to Pictures, documents to Documents, etc.)
3. **Plan Creation** - Creates an OSAI Plan with FilesMove steps for each file to be moved
4. **User Approval** - Presents the plan for user approval before any files are moved
5. **Execution** - Moves files only after explicit user approval
6. **Logging** - Creates a receipt for the operation for audit purposes

## Capabilities

- **List files** in ~/Downloads without approval
- **Read file metadata** to determine file types
- **Move files** to subdirectories with explicit approval
- **Create audit receipts** for all operations

## Limitations

- Cannot delete files
- Cannot execute shell commands
- Cannot access network resources
- Cannot operate outside of designated directories

## Example Interaction

User: "Organize my downloads folder"

Agent response:
- Scans ~/Downloads
- Creates plan with FilesMove operations
- Presents plan for approval
- After user approves, moves files to appropriate directories
- Reports completion with receipt ID

## Safety Features

- All file operations require approval before execution
- Rollback plan is included in case of errors
- No destructive operations (delete) are permitted
- All actions are logged as receipts
