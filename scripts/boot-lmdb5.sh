#!/bin/bash
#===============================================================================
# LMDB5 BOOT INJECTION
# Loads manifest into LMDB 5 on startup
# Called by: session-start.sh or systemd/init
#===============================================================================

SPF_HOME="$HOME/SPFsmartGATE"
MANIFEST="$SPF_HOME/storage/lmdb5_manifest.json"
SPF_BINARY="$SPF_HOME/storage/blobs/$(ls $SPF_HOME/storage/blobs/ | grep -v claude | head -1)"
LOG="$SPF_HOME/state/boot.log"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] BOOT: $1" >> "$LOG"
}

# Check manifest exists
if [ ! -f "$MANIFEST" ]; then
    log "ERROR: Manifest not found: $MANIFEST"
    exit 1
fi

log "Loading LMDB5 manifest..."

# The SPF binary will read the manifest and populate LMDB 5
# This happens automatically when spf-smart-gate starts with 'serve'
# The fs.rs init checks for manifest and imports entries

log "LMDB5 boot injection complete"

# Export virtual paths for Claude CLI
export SPF_AGENT_HOME="/home/agent"
export SPF_CLAUDE_CONFIG="/home/agent/.claude.json"
export SPF_ACTIVE=1

log "Environment exported: SPF_AGENT_HOME=$SPF_AGENT_HOME"
