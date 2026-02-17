#!/bin/bash
# SPF Post-Action Hook
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Runs after tool calls complete.
# Checkpoints state, updates session, can trigger brain sync.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
SESSION_FILE="$STATE_DIR/session.json"
LOG_FILE="$STATE_DIR/spf.log"
FAILURE_LOG="$STATE_DIR/failures.log"

# Ensure state dir exists
mkdir -p "$STATE_DIR"

# Timestamp
timestamp() {
    date '+%Y-%m-%d %H:%M:%S'
}

log() {
    echo "[$(timestamp)] POST: $1" >> "$LOG_FILE"
}

# Log rotation — 0.5GB max, keep last 5000 lines, one backup
rotate_log() {
    local logfile="$1"
    local max_bytes=536870912
    if [ -f "$logfile" ]; then
        local size
        size=$(stat -c%s "$logfile" 2>/dev/null || stat -f%z "$logfile" 2>/dev/null || echo 0)
        if [ "$size" -gt "$max_bytes" ]; then
            tail -5000 "$logfile" > "$logfile.tmp"
            mv "$logfile" "$logfile.1"
            mv "$logfile.tmp" "$logfile"
        fi
    fi
}

# Rotate on every call (cheap stat check)
rotate_log "$LOG_FILE"
rotate_log "$FAILURE_LOG"

# Read params from stdin
if [ -t 0 ]; then
    PARAMS="${1:-{}}"
else
    PARAMS=$(cat)
fi

# Get tool info from Claude Code's JSON structure (falls back to env vars)
TOOL_NAME=$(echo "$PARAMS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_name', d.get('tool','unknown')))" 2>/dev/null || echo "${SPF_TOOL_NAME:-unknown}")
TOOL_RESULT="${SPF_TOOL_RESULT:-success}"
export SPF_TOOL_PARAMS="$PARAMS"

log "Post-action: $TOOL_NAME = $TOOL_RESULT"

# Update session state
if [ -f "$SESSION_FILE" ]; then
    # Increment action counter
    python3 << EOF
import json
import os

session_file = "$SESSION_FILE"

try:
    with open(session_file) as f:
        session = json.load(f)
except:
    session = {"actions": [], "files_read": [], "files_written": [], "action_count": 0}

session["action_count"] = session.get("action_count", 0) + 1
session["last_tool"] = "$TOOL_NAME"
session["last_result"] = "$TOOL_RESULT"

with open(session_file, 'w') as f:
    json.dump(session, f, indent=2)
EOF
else
    # Create initial session
    cat > "$SESSION_FILE" << EOF
{
  "action_count": 1,
  "files_read": [],
  "files_written": [],
  "last_tool": "$TOOL_NAME",
  "last_result": "$TOOL_RESULT",
  "started": "$(timestamp)"
}
EOF
fi

log "Session updated"

# ============================================
# STATUS.txt Update (Memory Triad System 2)
# ============================================
STATUS_FILE="$STATE_DIR/STATUS.txt"

# Extract file path from Claude Code's JSON structure
FILE_PATH=$(echo "$PARAMS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tool_input',{}).get('file_path','unknown') if 'tool_input' in d else d.get('file_path','unknown'))" 2>/dev/null || echo "unknown")

# Update STATUS.txt with current state
python3 << STATUSEOF
import json
from pathlib import Path
from datetime import datetime

session_file = Path("$SESSION_FILE")
status_file = Path("$STATUS_FILE")
tool_name = "$TOOL_NAME"
tool_result = "$TOOL_RESULT"
file_path = "$FILE_PATH"

# Load session data
try:
    with open(session_file) as f:
        session = json.load(f)
except:
    session = {"action_count": 0, "files_read": [], "files_written": []}

# Track files written
if tool_name in ("Write", "Edit") and file_path != "unknown":
    if file_path not in session.get("files_written", []):
        session.setdefault("files_written", []).append(file_path)
        with open(session_file, 'w') as f:
            json.dump(session, f, indent=2)

# Track files read
if tool_name == "Read" and file_path != "unknown":
    if file_path not in session.get("files_read", []):
        session.setdefault("files_read", []).append(file_path)
        with open(session_file, 'w') as f:
            json.dump(session, f, indent=2)

# Build STATUS.txt content
action_count = session.get("action_count", 0)
files_read = session.get("files_read", [])
files_written = session.get("files_written", [])

status_content = f"""# SPF STATUS — Memory Triad System 2
# Auto-updated by post-action.sh
# Last Update: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}

## Current State
- **Session Actions**: {action_count}
- **Last Tool**: {tool_name}
- **Last Result**: {tool_result}
- **Last File**: {file_path}

## Files Read This Session ({len(files_read)})
{chr(10).join(f'- {f}' for f in files_read[-10:]) if files_read else '- (none yet)'}

## Files Modified This Session ({len(files_written)})
{chr(10).join(f'- {f}' for f in files_written[-10:]) if files_written else '- (none yet)'}

## Build Anchor Status
- Files read: {len(files_read)}
- Files written: {len(files_written)}
- Anchor ratio: {f'{len(files_read)}/{len(files_written)}' if files_written else 'N/A (no writes yet)'}

## Notes
- This file is auto-updated after every tool action
- Used for session recovery and Memory Triad verification
- Check spf.log for detailed action history
"""

# Write STATUS.txt
with open(status_file, 'w') as f:
    f.write(status_content)
STATUSEOF

log "STATUS.txt updated"

# Brain sync on significant actions (Write/Edit)
SPF_HOME="$(cd "$SPF_ROOT/.." 2>/dev/null && pwd || echo "$HOME")"
BRAIN_BIN="$SPF_HOME/stoneshell-brain/target/release/brain"
BRAIN_SYNC_ENABLED="${SPF_BRAIN_SYNC:-true}"

if [ "$BRAIN_SYNC_ENABLED" = "true" ] && [ -x "$BRAIN_BIN" ]; then
    if [ "$TOOL_NAME" = "Write" ] || [ "$TOOL_NAME" = "Edit" ]; then
        log "Checkpointing to brain..."

        # FILE_PATH already extracted above for STATUS.txt

        # Build checkpoint message
        CHECKPOINT_MSG="SPF Action Checkpoint
Tool: $TOOL_NAME
Result: $TOOL_RESULT
File: $FILE_PATH
Time: $(timestamp)
Session Actions: $(python3 -c "import json; print(json.load(open('$SESSION_FILE')).get('action_count',0))" 2>/dev/null || echo "?")"

        # Store to brain (async to not block)
        "$BRAIN_BIN" store "$CHECKPOINT_MSG" \
            -t "SPF: $TOOL_NAME on $(basename "$FILE_PATH" 2>/dev/null || echo "file")" \
            -c "spf_audit" \
            --tags "spf,checkpoint,$TOOL_NAME" \
            --index >> "$LOG_FILE" 2>&1 &

        log "Brain checkpoint queued for $FILE_PATH"
    fi
else
    if [ "$BRAIN_SYNC_ENABLED" = "true" ]; then
        log "Brain sync enabled but binary not found at $BRAIN_BIN"
    fi
fi

exit 0
