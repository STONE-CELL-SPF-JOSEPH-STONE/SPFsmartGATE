#!/bin/bash
# SPF Post-Failure Hook
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Runs when tool execution fails.
# Logs failures for debugging and brain indexing.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
LOG_FILE="$STATE_DIR/spf.log"
FAILURE_LOG="$STATE_DIR/failures.log"
export SPF_STATE_DIR="$STATE_DIR"

# Ensure state dir exists
mkdir -p "$STATE_DIR"

# Timestamp
timestamp() {
    date '+%Y-%m-%d %H:%M:%S'
}

log() {
    echo "[$(timestamp)] FAILURE: $1" >> "$LOG_FILE"
    echo "[$(timestamp)] $1" >> "$FAILURE_LOG"
}

# Read params from stdin
if [ -t 0 ]; then
    PARAMS="${1:-{}}"
else
    PARAMS=$(cat)
fi

# Extract tool info from Claude Code's JSON structure
TOOL_NAME=$(echo "$PARAMS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_name', d.get('tool','unknown')))" 2>/dev/null || echo "unknown")
ERROR_MSG=$(echo "$PARAMS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error','unknown error')[:200])" 2>/dev/null || echo "unknown error")

log "Tool '$TOOL_NAME' failed: $ERROR_MSG"

# Always exit 0 - we're just logging, not blocking
exit 0
