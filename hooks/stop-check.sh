#!/bin/bash
# SPF Stop Check Hook
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Fires on Stop event. Ensures session state is saved.
# Checks stop_hook_active to avoid infinite loops.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
LOG_FILE="$STATE_DIR/spf.log"
SESSION_FILE="$STATE_DIR/session.json"
export SPF_STATE_DIR="$STATE_DIR"

timestamp() {
    date '+%Y-%m-%d %H:%M:%S'
}

log() {
    echo "[$(timestamp)] STOP: $1" >> "$LOG_FILE"
}

# Read hook input from stdin
if [ -t 0 ]; then
    INPUT="{}"
else
    INPUT=$(cat)
fi

# Check if stop hook is already active (avoid infinite loop)
STOP_ACTIVE=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('stop_hook_active',False))" 2>/dev/null || echo "False")

if [ "$STOP_ACTIVE" = "True" ]; then
    log "Stop hook already active, allowing stop"
    exit 0
fi

log "Claude stopping — saving session state"

# Update session with stop timestamp
if [ -f "$SESSION_FILE" ]; then
    python3 << 'PYEOF'
import json
import os
from datetime import datetime

state_dir = os.environ.get("SPF_STATE_DIR", os.path.join(os.environ.get("HOME", ""), "SPFsmartGATE", "state"))
session_file = os.path.join(state_dir, "session.json")
try:
    with open(session_file) as f:
        session = json.load(f)
    session["last_stop"] = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
    with open(session_file, 'w') as f:
        json.dump(session, f, indent=2)
except:
    pass
PYEOF
fi

log "Session state saved on stop"

# Exit 0 = allow Claude to stop normally
exit 0
