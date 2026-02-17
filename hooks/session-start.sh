#!/bin/bash
# SPF Session Start Hook
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Fires on SessionStart. Queries brain for SPF algorithm,
# injects it as additionalContext so every session starts with SPF loaded.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
LOG_FILE="$STATE_DIR/spf.log"
SESSION_FILE="$STATE_DIR/session.json"

mkdir -p "$STATE_DIR"

timestamp() {
    date '+%Y-%m-%d %H:%M:%S'
}

log() {
    echo "[$(timestamp)] SESSION_START: $1" >> "$LOG_FILE"
}

# Read hook input from stdin
if [ -t 0 ]; then
    INPUT="{}"
else
    INPUT=$(cat)
fi

SOURCE=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('source','startup'))" 2>/dev/null || echo "startup")

log "Session starting (source: $SOURCE)"

# Reset session state for fresh starts
if [ "$SOURCE" = "startup" ] || [ "$SOURCE" = "clear" ]; then
    cat > "$SESSION_FILE" << SEOF
{
  "action_count": 0,
  "files_read": [],
  "files_written": [],
  "last_tool": null,
  "last_result": null,
  "started": "$(timestamp)",
  "source": "$SOURCE"
}
SEOF
    log "Session state reset"
fi

# Set env vars if CLAUDE_ENV_FILE is available
if [ -n "$CLAUDE_ENV_FILE" ]; then
    echo "export SPF_SESSION_START=$(date +%s)" >> "$CLAUDE_ENV_FILE"
    echo "export SPF_STATE_DIR=$STATE_DIR" >> "$CLAUDE_ENV_FILE"
    log "Environment variables persisted"
fi

# Build SPF context to inject
SPF_CONTEXT="# SPF — StoneCell Processing Formula (Auto-Injected)

## Complexity Tiers
| Tier | C Value | Analyze | Build |
|------|---------|---------|-------|
| SIMPLE | < 500 | ~40% | ~60% |
| LIGHT | < 2000 | ~60% | ~40% |
| MEDIUM | < 10000 | ~75% | ~25% |
| CRITICAL | > 10000 | ~95% | ~5% |

## Formula: a_optimal(C) = W_eff × (1 - 1/ln(C + e))
- W_eff = 40,000 tokens | e = Euler's number

## Enforcement
1. Calculate C before action
2. Stay within allocation ratio
3. Never exceed allocation without user approval
4. Checkpoint state to brain on completion
5. Never skip complexity calculation on tasks C > 500

## Active MCP Servers: spf-smart-gate, stoneshell-brain, rag-collector
## Hooks: SPF enforcement active on all tool calls
## State: $STATE_DIR/spf.log | STATUS.txt"

# Output JSON with additionalContext
python3 -c "
import json
context = '''$SPF_CONTEXT'''
output = {
    'hookSpecificOutput': {
        'hookEventName': 'SessionStart',
        'additionalContext': context
    }
}
print(json.dumps(output))
"
# LMDB5 Boot Injection
source "$SPF_ROOT/scripts/boot-lmdb5.sh" 2>/dev/null || true

log "SPF context injected into session"
exit 0

