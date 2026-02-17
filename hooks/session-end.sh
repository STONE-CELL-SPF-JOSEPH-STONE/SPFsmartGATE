#!/bin/bash
# SPF Session End Hook
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Fires on SessionEnd. Checkpoints final session state.
# Writes session summary for next-session handoff.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
LOG_FILE="$STATE_DIR/spf.log"
SESSION_FILE="$STATE_DIR/session.json"
STATUS_FILE="$STATE_DIR/STATUS.txt"
export SPF_STATE_DIR="$STATE_DIR"

timestamp() {
    date '+%Y-%m-%d %H:%M:%S'
}

log() {
    echo "[$(timestamp)] SESSION_END: $1" >> "$LOG_FILE"
}

# Read hook input from stdin
if [ -t 0 ]; then
    INPUT="{}"
else
    INPUT=$(cat)
fi

REASON=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('reason','other'))" 2>/dev/null || echo "other")
export REASON

log "Session ending (reason: $REASON)"

# Build session summary for brain checkpoint
python3 << 'PYEOF'
import json
import os
from datetime import datetime

state_dir = os.environ.get("SPF_STATE_DIR", os.path.join(os.environ.get("HOME", ""), "SPFsmartGATE", "state"))
session_file = os.path.join(state_dir, "session.json")
status_file = os.path.join(state_dir, "STATUS.txt")
log_file = os.path.join(state_dir, "spf.log")

# Load session data
try:
    with open(session_file) as f:
        session = json.load(f)
except:
    session = {"action_count": 0, "files_read": [], "files_written": []}

action_count = session.get("action_count", 0)
files_read = session.get("files_read", [])
files_written = session.get("files_written", [])
started = session.get("started", "unknown")

# Build handoff note
handoff = f"""SESSION HANDOFF NOTE - {datetime.now().strftime('%b %d %Y %H:%M')}

SESSION SUMMARY:
- Started: {started}
- Ended: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}
- Total Actions: {action_count}
- Files Read: {len(files_read)}
- Files Written: {len(files_written)}

FILES MODIFIED:
{chr(10).join(f'- {f}' for f in files_written[-20:]) if files_written else '- (none)'}

FOR NEXT SESSION STARTUP:
- Query brain for this document
- Check ~/SPFsmartGATE/state/STATUS.txt for last known state
- Check ~/SPFsmartGATE/state/spf.log for action history
"""

# Write handoff note to state dir
handoff_file = os.path.join(state_dir, "handoff.md")
with open(handoff_file, 'w') as f:
    f.write(handoff)

# Mark session as ended
session["ended"] = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
session["end_reason"] = os.environ.get("REASON", "other")
with open(session_file, 'w') as f:
    json.dump(session, f, indent=2)

print(f"Session ended: {action_count} actions, {len(files_written)} files modified")
PYEOF

log "Session state checkpointed"
exit 0
