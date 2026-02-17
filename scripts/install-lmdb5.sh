#!/bin/bash
#===============================================================================
# LMDB5 CLI INSTALLATION SCRIPT
# Transfers Claude CLI and all configs into LMDB 5 virtual filesystem
# Creates boot injection for seamless startup
#
# Usage: bash install-lmdb5.sh
# Location: ~/SPFsmartGATE/scripts/install-lmdb5.sh
#===============================================================================

set -e  # Exit on error

#-------------------------------------------------------------------------------
# CONFIGURATION
#-------------------------------------------------------------------------------

SPF_HOME="$HOME/SPFsmartGATE"
STORAGE="$SPF_HOME/storage"
BLOBS="$STORAGE/blobs"
LMDB5_DIR="$STORAGE/agent_state"
MANIFEST="$STORAGE/lmdb5_manifest.json"
BACKUP_DIR="$SPF_HOME/backup/pre-lmdb5-$(date +%Y%m%d-%H%M%S)"
LOG="$SPF_HOME/state/lmdb5-install.log"

# Source paths
# Auto-detect Claude Code installation
if command -v claude &>/dev/null; then
    CLAUDE_CLI="$(dirname "$(dirname "$(readlink -f "$(which claude)")")")"
elif [ -d "/data/data/com.termux/files/usr/lib/node_modules/@anthropic-ai/claude-code" ]; then
    CLAUDE_CLI="/data/data/com.termux/files/usr/lib/node_modules/@anthropic-ai/claude-code"
else
    error "Claude Code not found. Install: npm install -g @anthropic-ai/claude-code"
fi
SPF_BINARY="$SPF_HOME/target/release/spf-smart-gate"
CLAUDE_JSON="$HOME/.claude.json"
CLAUDE_DIR="$HOME/.claude"

# Virtual paths (LMDB 5)
VHOME="/home/agent"

#-------------------------------------------------------------------------------
# HELPER FUNCTIONS
#-------------------------------------------------------------------------------

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG"
}

error() {
    echo "[ERROR] $1" | tee -a "$LOG"
    exit 1
}

check_exists() {
    if [ ! -e "$1" ]; then
        error "Required file/dir not found: $1"
    fi
}

sha256sum_file() {
    sha256sum "$1" 2>/dev/null | cut -d' ' -f1
}

#-------------------------------------------------------------------------------
# PRE-FLIGHT CHECKS
#-------------------------------------------------------------------------------

preflight() {
    log "=== PRE-FLIGHT CHECKS ==="

    # Check SPF build exists
    check_exists "$SPF_BINARY"
    log "✓ SPF binary found: $SPF_BINARY"

    # Check Claude CLI exists
    check_exists "$CLAUDE_CLI"
    log "✓ Claude CLI found: $CLAUDE_CLI"

    # Check Claude config exists
    check_exists "$CLAUDE_JSON"
    log "✓ Claude config found: $CLAUDE_JSON"

    # Check Claude dir exists
    check_exists "$CLAUDE_DIR"
    log "✓ Claude dir found: $CLAUDE_DIR"

    # Create directories
    mkdir -p "$STORAGE" "$BLOBS" "$LMDB5_DIR" "$BACKUP_DIR"
    mkdir -p "$SPF_HOME/state"
    log "✓ Storage directories ready"

    log "=== PRE-FLIGHT COMPLETE ==="
}

#-------------------------------------------------------------------------------
# BACKUP EXISTING
#-------------------------------------------------------------------------------

backup_existing() {
    log "=== CREATING BACKUP ==="

    # Backup current claude configs
    if [ -f "$CLAUDE_JSON" ]; then
        cp "$CLAUDE_JSON" "$BACKUP_DIR/claude.json"
        log "✓ Backed up: .claude.json"
    fi

    # Backup claude dir (exclude debug/)
    if [ -d "$CLAUDE_DIR" ]; then
        rsync -a --exclude='debug/' "$CLAUDE_DIR/" "$BACKUP_DIR/claude/"
        log "✓ Backed up: .claude/ (excluding debug/)"
    fi

    log "Backup location: $BACKUP_DIR"
    log "=== BACKUP COMPLETE ==="
}

#-------------------------------------------------------------------------------
# COPY BINARIES TO BLOB STORAGE
#-------------------------------------------------------------------------------

copy_binaries() {
    log "=== COPYING BINARIES ==="

    # SPF Smart Gate binary
    local spf_hash=$(sha256sum_file "$SPF_BINARY")
    local spf_blob="$BLOBS/$spf_hash"
    cp "$SPF_BINARY" "$spf_blob"
    chmod +x "$spf_blob"
    log "✓ SPF binary → blob: $spf_hash"
    echo "$VHOME/bin/spf-smart-gate|$spf_blob|$(stat -c%s "$SPF_BINARY")|binary" >> "$MANIFEST.tmp"

    # Claude CLI (entire directory)
    local claude_blob_dir="$BLOBS/claude-code"
    rm -rf "$claude_blob_dir"
    cp -r "$CLAUDE_CLI" "$claude_blob_dir"
    log "✓ Claude CLI → blob: claude-code/"
    echo "$VHOME/bin/claude-code|$claude_blob_dir|directory|binary" >> "$MANIFEST.tmp"

    log "=== BINARIES COMPLETE ==="
}

#-------------------------------------------------------------------------------
# COPY CONFIG FILES (SMALL - DIRECT)
#-------------------------------------------------------------------------------

copy_configs() {
    log "=== COPYING CONFIG FILES ==="

    local config_staging="$STORAGE/staging/configs"
    mkdir -p "$config_staging"

    # Main claude.json
    cp "$CLAUDE_JSON" "$config_staging/claude.json"
    echo "$VHOME/.claude.json|$config_staging/claude.json|$(stat -c%s "$CLAUDE_JSON")|config" >> "$MANIFEST.tmp"
    log "✓ .claude.json"

    # Small config files (< 1MB, store directly)
    local small_files=(
        "settings.json"
        "config.json"
        ".credentials.json"
        "claude.md"
        "stats-cache.json"
        "settings.local.json"
    )

    for f in "${small_files[@]}"; do
        if [ -f "$CLAUDE_DIR/$f" ]; then
            cp "$CLAUDE_DIR/$f" "$config_staging/$f"
            echo "$VHOME/.claude/$f|$config_staging/$f|$(stat -c%s "$CLAUDE_DIR/$f")|config" >> "$MANIFEST.tmp"
            log "✓ .claude/$f"
        fi
    done

    log "=== CONFIG FILES COMPLETE ==="
}

#-------------------------------------------------------------------------------
# COPY LARGE DIRECTORIES TO BLOB STORAGE
#-------------------------------------------------------------------------------

copy_large_dirs() {
    log "=== COPYING LARGE DIRECTORIES ==="

    local large_dirs=(
        "projects"
        "file-history"
        "paste-cache"
        "session-env"
        "todos"
        "plans"
        "tasks"
        "shell-snapshots"
        "statsig"
        "telemetry"
    )

    for dir in "${large_dirs[@]}"; do
        if [ -d "$CLAUDE_DIR/$dir" ]; then
            local blob_dir="$BLOBS/claude-$dir"
            rm -rf "$blob_dir"
            cp -r "$CLAUDE_DIR/$dir" "$blob_dir"
            local size=$(du -sb "$CLAUDE_DIR/$dir" | cut -f1)
            echo "$VHOME/.claude/$dir|$blob_dir|$size|directory" >> "$MANIFEST.tmp"
            log "✓ .claude/$dir → blob"
        fi
    done

    # history.jsonl (medium file)
    if [ -f "$CLAUDE_DIR/history.jsonl" ]; then
        local hist_hash=$(sha256sum_file "$CLAUDE_DIR/history.jsonl")
        cp "$CLAUDE_DIR/history.jsonl" "$BLOBS/$hist_hash"
        echo "$VHOME/.claude/history.jsonl|$BLOBS/$hist_hash|$(stat -c%s "$CLAUDE_DIR/history.jsonl")|file" >> "$MANIFEST.tmp"
        log "✓ .claude/history.jsonl → blob"
    fi

    log "=== LARGE DIRECTORIES COMPLETE ==="
}

#-------------------------------------------------------------------------------
# CREATE LMDB5 MANIFEST (JSON)
#-------------------------------------------------------------------------------

create_manifest() {
    log "=== CREATING MANIFEST ==="

    # Convert tmp manifest to JSON
    echo '{' > "$MANIFEST"
    echo '  "version": "1.0",' >> "$MANIFEST"
    echo '  "created": "'$(date -Iseconds)'",' >> "$MANIFEST"
    echo '  "entries": [' >> "$MANIFEST"

    local first=true
    while IFS='|' read -r vpath rpath size ftype; do
        if [ "$first" = true ]; then
            first=false
        else
            echo ',' >> "$MANIFEST"
        fi
        echo -n '    {"virtual": "'$vpath'", "real": "'$rpath'", "size": "'$size'", "type": "'$ftype'"}' >> "$MANIFEST"
    done < "$MANIFEST.tmp"

    echo '' >> "$MANIFEST"
    echo '  ]' >> "$MANIFEST"
    echo '}' >> "$MANIFEST"

    rm -f "$MANIFEST.tmp"
    log "✓ Manifest created: $MANIFEST"
    log "=== MANIFEST COMPLETE ==="
}

#-------------------------------------------------------------------------------
# CREATE BOOT INJECTION SCRIPT
#-------------------------------------------------------------------------------

create_boot_injection() {
    log "=== CREATING BOOT INJECTION ==="

    local boot_script="$SPF_HOME/scripts/boot-lmdb5.sh"
    mkdir -p "$SPF_HOME/scripts"

    cat > "$boot_script" << 'BOOTEOF'
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
BOOTEOF

    chmod +x "$boot_script"
    log "✓ Boot script: $boot_script"

    # Update session-start.sh to call boot injection
    local session_start="$SPF_HOME/hooks/session-start.sh"
    if [ -f "$session_start" ]; then
        if ! grep -q "boot-lmdb5.sh" "$session_start"; then
            echo "" >> "$session_start"
            echo "# LMDB5 Boot Injection" >> "$session_start"
            echo "source $boot_script 2>/dev/null || true" >> "$session_start"
            log "✓ Injected into session-start.sh"
        else
            log "✓ Boot injection already in session-start.sh"
        fi
    fi

    log "=== BOOT INJECTION COMPLETE ==="
}

#-------------------------------------------------------------------------------
# CREATE SYMLINKS FOR BACKWARD COMPATIBILITY
#-------------------------------------------------------------------------------

create_symlinks() {
    log "=== CREATING SYMLINKS ==="

    # Create agent bin directory with symlinks to blob binaries
    local agent_bin="$SPF_HOME/agent-bin"
    mkdir -p "$agent_bin"

    # SPF binary symlink
    local spf_blob=$(ls "$BLOBS" | grep -v claude | head -1)
    if [ -n "$spf_blob" ]; then
        ln -sf "$BLOBS/$spf_blob" "$agent_bin/spf-smart-gate"
        log "✓ Symlink: agent-bin/spf-smart-gate → blob"
    fi

    # Claude CLI symlink
    if [ -d "$BLOBS/claude-code" ]; then
        ln -sf "$BLOBS/claude-code" "$agent_bin/claude-code"
        log "✓ Symlink: agent-bin/claude-code → blob"
    fi

    log "=== SYMLINKS COMPLETE ==="
}

#-------------------------------------------------------------------------------
# VERIFICATION
#-------------------------------------------------------------------------------

verify_installation() {
    log "=== VERIFICATION ==="

    local errors=0

    # Check manifest
    if [ -f "$MANIFEST" ]; then
        local count=$(grep -c '"virtual"' "$MANIFEST")
        log "✓ Manifest entries: $count"
    else
        log "✗ Manifest missing"
        ((errors++))
    fi

    # Check blobs
    local blob_count=$(ls -1 "$BLOBS" 2>/dev/null | wc -l)
    log "✓ Blob files: $blob_count"

    # Check boot script
    if [ -x "$SPF_HOME/scripts/boot-lmdb5.sh" ]; then
        log "✓ Boot script executable"
    else
        log "✗ Boot script missing or not executable"
        ((errors++))
    fi

    # Check SPF binary in blob
    if [ -f "$SPF_HOME/agent-bin/spf-smart-gate" ]; then
        log "✓ SPF binary accessible via agent-bin"
    else
        log "✗ SPF binary symlink missing"
        ((errors++))
    fi

    if [ $errors -eq 0 ]; then
        log "=== VERIFICATION PASSED ==="
    else
        log "=== VERIFICATION FAILED: $errors errors ==="
        return 1
    fi
}

#-------------------------------------------------------------------------------
# UPDATE CLAUDE.JSON TO POINT TO NEW PATHS
#-------------------------------------------------------------------------------

update_claude_json() {
    log "=== UPDATING CLAUDE.JSON REFERENCES ==="

    # Update mcpServers command to use agent-bin path
    local new_spf_path="$SPF_HOME/agent-bin/spf-smart-gate"

    # Use jq if available, otherwise sed
    if command -v jq &> /dev/null; then
        jq '.mcpServers["spf-smart-gate"].command = "'$new_spf_path'"' \
            "$CLAUDE_JSON" > "$CLAUDE_JSON.tmp" && mv "$CLAUDE_JSON.tmp" "$CLAUDE_JSON"
        log "✓ Updated mcpServers path via jq"
    else
        sed -i 's|"command": ".*spf-smart-gate.*"|"command": "'$new_spf_path'"|' "$CLAUDE_JSON"
        log "✓ Updated mcpServers path via sed"
    fi

    log "=== CLAUDE.JSON UPDATED ==="
}

#-------------------------------------------------------------------------------
# PRINT SUMMARY
#-------------------------------------------------------------------------------

print_summary() {
    echo ""
    echo "==============================================================================="
    echo "                    LMDB5 INSTALLATION COMPLETE"
    echo "==============================================================================="
    echo ""
    echo "Backup:     $BACKUP_DIR"
    echo "Manifest:   $MANIFEST"
    echo "Blobs:      $BLOBS"
    echo "Boot:       $SPF_HOME/scripts/boot-lmdb5.sh"
    echo "Log:        $LOG"
    echo ""
    echo "Virtual filesystem layout:"
    echo "  /home/agent/"
    echo "  ├── .claude.json"
    echo "  ├── .claude/"
    echo "  │   ├── settings.json"
    echo "  │   ├── config.json"
    echo "  │   ├── projects/"
    echo "  │   └── ..."
    echo "  └── bin/"
    echo "      ├── spf-smart-gate"
    echo "      └── claude-code/"
    echo ""
    echo "Next steps:"
    echo "  1. Restart Claude CLI"
    echo "  2. Test: spf_fs_ls /home/agent/"
    echo "  3. Verify boot injection in session-start.sh"
    echo ""
    echo "==============================================================================="
}

#-------------------------------------------------------------------------------
# MAIN
#-------------------------------------------------------------------------------

main() {
    echo ""
    echo "==============================================================================="
    echo "              LMDB5 CLI INSTALLATION - FULL CONTAINMENT"
    echo "==============================================================================="
    echo ""

    preflight
    backup_existing

    # Initialize manifest
    > "$MANIFEST.tmp"

    copy_binaries
    copy_configs
    copy_large_dirs
    create_manifest
    create_boot_injection
    create_symlinks
    update_claude_json
    verify_installation
    print_summary

    log "=== INSTALLATION COMPLETE ==="
}

# Run main
main "$@"

#===============================================================================
# END OF INSTALLATION SCRIPT
#===============================================================================
