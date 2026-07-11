#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/tunnel-common.sh"

WORKSPACE="${1:-${CODING_TOOLS_MCP_WORKSPACE:-$PWD}}"
PORT="${CODING_TOOLS_MCP_PORT:-8765}"
PROFILE="${CODING_TOOLS_MCP_TOOL_PROFILE:-read-only}"
SERVER_BIN="${CODING_TOOLS_MCP_SERVER_BIN:-coding-tools-mcp}"
AUTH_MODE="${CODING_TOOLS_MCP_AUTH_MODE:-bearer}"

resolve_auth_credentials

ensure_tunnel_command cloudflared
start_coding_tools_mcp "$WORKSPACE" "$PORT" "$PROFILE" "$AUTH_MODE" "$TOKEN" "$SERVER_BIN"
print_tunnel_config "cloudflared" "cloudflared-host" "$PORT" "$PROFILE" "$AUTH_MODE" "$TOKEN"
cloudflared tunnel --url "http://127.0.0.1:$PORT"
