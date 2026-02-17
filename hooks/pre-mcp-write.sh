#!/bin/bash
# SPF Pre-MCP-write Hook - Allows MCP write ops with tracking
# Copyright 2026 Joseph Stone - All Rights Reserved

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
LOG_FILE="$STATE_DIR/spf.log"
mkdir -p "$STATE_DIR"

if [ -t 0 ]; then PARAMS="${1:-{}}"; else PARAMS=$(cat); fi
TOOL_NAME=$(echo "$PARAMS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_name', d.get('tool','unknown')))" 2>/dev/null || echo "mcp_write")

echo "[$(date '+%Y-%m-%d %H:%M:%S')] PRE-MCP-WRITE: $TOOL_NAME" >> "$LOG_FILE"
exit 0
