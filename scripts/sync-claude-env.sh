#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT_DIR/.env"
CLAUDE_DIR="$ROOT_DIR/.claude"
CLAUDE_SETTINGS="$CLAUDE_DIR/settings.local.json"

if [[ ! -f "$ENV_FILE" ]]; then
  echo "Missing .env file. Copy .env.example to .env and fill MINIMAX_API_KEY."
  exit 1
fi

set -a
source "$ENV_FILE"
set +a

: "${MINIMAX_API_KEY:?MINIMAX_API_KEY is required}"
: "${MINIMAX_ANTHROPIC_BASE_URL:=https://api.minimax.io/anthropic}"
: "${MINIMAX_MODEL:=MiniMax-M2.7}"

mkdir -p "$CLAUDE_DIR"

cat > "$CLAUDE_SETTINGS" <<JSON
{
  "\$schema": "https://json.schemastore.org/claude-code-settings.json",
  "env": {
    "ANTHROPIC_BASE_URL": "$MINIMAX_ANTHROPIC_BASE_URL",
    "ANTHROPIC_API_KEY": "$MINIMAX_API_KEY",
    "ANTHROPIC_MODEL": "$MINIMAX_MODEL",
    "ANTHROPIC_CUSTOM_MODEL_OPTION": "$MINIMAX_MODEL",
    "ANTHROPIC_CUSTOM_MODEL_OPTION_NAME": "MiniMax M2.7",
    "ANTHROPIC_CUSTOM_MODEL_OPTION_DESCRIPTION": "MiniMax M2.7 via Anthropic-compatible API",
    "API_TIMEOUT_MS": "600000"
  }
}
JSON

chmod 600 "$CLAUDE_SETTINGS"
echo "Generated $CLAUDE_SETTINGS from .env"
