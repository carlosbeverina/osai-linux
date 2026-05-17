# API Design

## Overview

osai-api is a local HTTP API service that exposes osai-agent-core functionality. It is the interface that future UI (desktop shell integration or web control panel) will call.

**Important**: osai-api must use osai-agent-core, not shell out to the CLI. The architecture boundary is:
```
UI → osai-api → osai-agent-core → ToolBroker/ToolExecutor/receipts
```

## Current Status

osai-api provides the Phase 2 MVP local API with:
- Loopback-only HTTP server at `127.0.0.1:8090`
- Prototype Dev Panel at `/ui`
- Health/status/capabilities/runtime/auth introspection endpoints
- Token-protected chat, ask, plans, apply, and receipts endpoints
- Direct `osai-agent-core` integration for chat, ask, and apply
- Safe plan, receipt, and static-file reads with path traversal protections

## Design Principles

1. **Loopback-only binding** — osai-api binds to `127.0.0.1:8090` only. No external exposure.
2. **Token auth on sensitive endpoints** — chat, ask, apply, receipts, plans require auth
3. **osai-agent-core for logic** — API calls core functions, does not replicate CLI logic
4. **Consistent JSON responses** — All responses use structured JSON
5. **Error responses include code and message** — Easy to debug and handle

## Proposed Endpoints

### Unauthenticated (safe for local introspection)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Service health check |
| GET | `/v1/status` | Model and runtime status |
| GET | `/v1/capabilities` | Feature flags including auth_status |
| GET | `/v1/runtime/status` | Unified component health |
| GET | `/v1/auth/status` | Auth status and token source |
| GET | `/ui` | Dev Panel web UI |
| GET | `/ui/` | Dev Panel (trailing slash) |
| GET | `/ui/index.html` | Dev Panel HTML |

### Token-Protected

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/chat` | Chat with model |
| POST | `/v1/ask` | Generate plan from natural language |
| GET | `/v1/plans` | List plans |
| GET | `/v1/plans/read` | Read plan content |
| POST | `/v1/plans/validate` | Validate plan |
| POST | `/v1/plans/authorize` | Authorize plan with policy |
| POST | `/v1/apply` | Apply plan (dry-run or real) |
| GET | `/v1/receipts` | List receipts |
| GET | `/v1/receipts/read` | Read receipt content |

## Token Authentication

### Token Source (checked in order)

1. `OSAI_API_TOKEN` environment variable (if set and non-empty)
2. `~/.config/osai/api-token` file (auto-generated if missing)

### Token Header

Protected endpoints accept either:
```
Authorization: Bearer <token>
```
or
```
X-OSAI-Token: <token>
```

### 401 Response

```json
{
  "ok": false,
  "error": {
    "code": "unauthorized",
    "message": "missing or invalid local API token"
  }
}
```

Note: The message does not reveal whether the token was wrong or missing. Do not echo the provided token.

### Token Security

- Token is never logged
- Token is never in receipts
- Token comparison uses constant-time equality (prevents timing attacks)
- Token stored in-memory only in UI (not localStorage/sessionStorage)
- Invalid token returns 401, not 403 (prevents enumeration)

## Response Format

### Success

```json
{
  "ok": true,
  "data": { ... }
}
```

### Error

```json
{
  "ok": false,
  "error": {
    "code": "bad_request",
    "message": "human-readable description"
  }
}
```

### Error Codes

| Code | HTTP Status | Description |
|------|------------|-------------|
| `bad_request` | 400 | Invalid request body or parameters |
| `unauthorized` | 401 | Missing or invalid auth token |
| `not_found` | 404 | Resource not found |
| `method_not_allowed` | 405 | HTTP method not supported |
| `internal_error` | 500 | Server-side error |

## Endpoint Details

### GET /health

```json
{
  "ok": true,
  "service": "osai-api",
  "version": "0.1.0"
}
```

### GET /v1/capabilities

```json
{
  "chat": true,
  "ask": true,
  "plan_validate": true,
  "plan_authorize": true,
  "apply": true,
  "plans": true,
  "receipts": true,
  "runtime_status": true,
  "auth_status": true
}
```

### GET /v1/auth/status

```json
{
  "ok": true,
  "auth_required": true,
  "token_source": "file"
}
```

`token_source` values: `env`, `file`, `disabled`

Note: Never returns the actual token value.

### POST /v1/chat

**Request**:
```json
{
  "message": "List my Downloads folder",
  "model": "osai-auto",
  "privacy": "local_only",
  "temperature": 0.2,
  "max_tokens": 512
}
```

**Response**:
```json
{
  "status": "success",
  "content": "Here are the files in your Downloads folder...",
  "response_length": 156,
  "error": null
}
```

### POST /v1/ask

**Request**:
```json
{
  "request": "Create a safe plan to list my Downloads folder",
  "model": "osai-auto",
  "privacy": "local_only"
}
```

**Response**:
```json
{
  "status": "success",
  "output_path": "/path/to/generated-plan.yml",
  "validation": "valid",
  "error": null
}
```

### POST /v1/plans/authorize

**Request**:
```json
{
  "plan_path": "/path/to/plan.yml",
  "policy_path": "examples/policies/default-secure.yml",
  "allowed_roots": ["~/Downloads"],
  "approve_all": false
}
```

**Response**:
```json
{
  "ok": true,
  "steps": [
    {
      "id": "step-1",
      "action": "FilesList",
      "allowed": true,
      "approval_required": false,
      "mode": "Allow",
      "reason": "Allowed by policy"
    }
  ],
  "summary": {
    "allowed": 1,
    "denied": 0,
    "approval_required": 0
  }
}
```

### POST /v1/apply

**Request**:
```json
{
  "plan_path": "/path/to/plan.yml",
  "policy_path": "examples/policies/default-secure.yml",
  "allowed_roots": ["~/Downloads"],
  "approve": ["step-1"],
  "approve_all": false,
  "dry_run": false
}
```

**Response**:
```json
{
  "status": "success",
  "executed": 1,
  "skipped": 0,
  "denied": 0,
  "approval_required": 0,
  "failed": 0,
  "approved_steps": ["step-1"],
  "dry_run": false,
  "error": null
}
```

## Future Endpoints

The following are planned but not yet implemented:

### Sessions/History
```
GET  /v1/sessions         # List chat sessions
GET  /v1/sessions/<id>   # Get session history
DELETE /v1/sessions/<id> # Delete session
```

### Settings
```
GET  /v1/settings        # Get settings
PUT  /v1/settings        # Update settings
```

### Computer Use (Phase 6)
```
POST /v1/computer-use/task     # Create computer-use task
GET  /v1/computer-use/task/<id> # Get task status
DELETE /v1/computer-use/task/<id> # Cancel task
GET  /v1/computer-use/task/<id>/artifacts # Get task artifacts
```

## OpenAPI Documentation

Future osai-api should include OpenAPI documentation at `/docs` or `/openapi.json`.

## Not Shell-Out

osai-api must call osai-agent-core directly:
```rust
// ✅ Correct — calls core directly
let result = osai_agent_core::chat_core_async(...).await;

// ❌ Incorrect — shells out to CLI
let output = std::process::Command::new("cargo")
    .args(["run", "-p", "osai-agent-cli", "--", "chat", &message])
    .output()?;
```

## Security Boundaries

1. **Loopback only** — No external access to osai-api
2. **Token auth** — Sensitive endpoints protected
3. **osai-agent-core** — All business logic in core, not in API
4. **ToolBroker** — API does not bypass authorization
5. **Receipts** — API does not write secrets to receipts
6. **No shell-out** — API uses library calls, not CLI subprocesses