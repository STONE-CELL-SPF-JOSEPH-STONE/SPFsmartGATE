#!/bin/bash
#===============================================================================
# SPFsmartGATE Setup — Android
# Copyright 2026 Joseph Stone. All Rights Reserved.
#
# One-command installation for Android devices.
# Auto-detects environment, builds/verifies binary, configures Claude Code.
#
# Usage: bash setup.sh
#===============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_HOME="$SCRIPT_DIR"
DEFAULT_HOME="/data/data/com.termux/files/home"
DETECTED_HOME="$HOME"

echo ""
echo "==============================================================================="
echo "              SPFsmartGATE Setup — Android"
echo "              Copyright 2026 Joseph Stone"
echo "==============================================================================="
echo ""
echo "  SPF_HOME:  $SPF_HOME"
echo "  HOME:      $DETECTED_HOME"
echo ""

#-------------------------------------------------------------------------------
# STEP 1: CHECK DEPENDENCIES
#-------------------------------------------------------------------------------

echo "=== Step 1: Checking Dependencies ==="

ERRORS=0

# Rust toolchain
if command -v cargo &>/dev/null; then
    echo "  ✓ Rust toolchain: $(cargo --version 2>&1)"
else
    echo "  ✗ Rust toolchain not found"
    echo "    Install: pkg install rust"
    ERRORS=$((ERRORS + 1))
fi

# Python 3 (required by hooks)
if command -v python3 &>/dev/null; then
    echo "  ✓ Python 3: $(python3 --version 2>&1)"
else
    echo "  ✗ Python 3 not found (required by hooks)"
    echo "    Install: pkg install python"
    ERRORS=$((ERRORS + 1))
fi

# LMDB library
if [ -f "/data/data/com.termux/files/usr/lib/liblmdb.so" ]; then
    echo "  ✓ LMDB library found"
elif pkg list-installed 2>/dev/null | grep -q liblmdb; then
    echo "  ✓ LMDB library installed"
else
    echo "  ✗ LMDB library not found"
    echo "    Install: pkg install liblmdb"
    ERRORS=$((ERRORS + 1))
fi

# Claude Code
if command -v claude &>/dev/null; then
    echo "  ✓ Claude Code found"
else
    echo "  ⚠ Claude Code not found (install separately)"
    echo "    Install: npm install -g @anthropic-ai/claude-code"
fi

if [ $ERRORS -gt 0 ]; then
    echo ""
    echo "  ✗ $ERRORS missing dependencies. Install them and rerun setup.sh"
    exit 1
fi

echo ""

#-------------------------------------------------------------------------------
# STEP 2: PATH RESOLUTION
#-------------------------------------------------------------------------------

echo "=== Step 2: Path Resolution ==="

if [ "$DETECTED_HOME" = "$DEFAULT_HOME" ]; then
    echo "  ✓ Standard Termux paths — no adjustment needed"
    NEEDS_PATH_FIX=false
else
    echo "  ⚠ Non-standard HOME detected: $DETECTED_HOME"
    echo "    Will adjust paths from $DEFAULT_HOME → $DETECTED_HOME"
    NEEDS_PATH_FIX=true
fi

echo ""

#-------------------------------------------------------------------------------
# STEP 3: CREATE DIRECTORIES
#-------------------------------------------------------------------------------

echo "=== Step 3: Creating Directories ==="

DIRS=(
    "$SPF_HOME/state"
    "$SPF_HOME/LIVE/BIN/spf-smart-gate"
    "$SPF_HOME/LIVE/CONFIG/CONFIG.DB"
    "$SPF_HOME/LIVE/SESSION/SESSION.DB"
    "$SPF_HOME/LIVE/PROJECTS/PROJECTS.DB"
    "$SPF_HOME/LIVE/TMP/TMP.DB"
    "$SPF_HOME/LIVE/SPF_FS/SPF_FS.DB"
    "$SPF_HOME/LIVE/SPF_FS/blobs"
    "$SPF_HOME/LIVE/LMDB5/LMDB5.DB"
)

for dir in "${DIRS[@]}"; do
    mkdir -p "$dir"
done
echo "  ✓ All LIVE/ directories ready (9 paths)"

echo ""

#-------------------------------------------------------------------------------
# STEP 4: BUILD OR VERIFY BINARY
#-------------------------------------------------------------------------------

echo "=== Step 4: Binary ==="

BINARY="$SPF_HOME/LIVE/BIN/spf-smart-gate/spf-smart-gate"

if [ -f "$BINARY" ] && [ -x "$BINARY" ]; then
    BIN_SIZE=$(du -h "$BINARY" | cut -f1)
    echo "  ✓ Pre-compiled binary found: $BIN_SIZE"
    echo ""
    read -p "  Rebuild from source? [y/N] " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "  Building from source..."
        bash "$SPF_HOME/build.sh" --target android
    fi
else
    echo "  No pre-compiled binary — building from source..."
    bash "$SPF_HOME/build.sh" --target android
fi

# Verify binary exists after build
if [ ! -f "$BINARY" ]; then
    echo "  ✗ Binary not found after build: $BINARY"
    exit 1
fi

echo "  ✓ Binary ready: $BINARY"
echo ""

#-------------------------------------------------------------------------------
# STEP 5: INITIALIZE DATABASES
#-------------------------------------------------------------------------------

echo "=== Step 5: Initialize Databases ==="

"$BINARY" init-config 2>/dev/null && echo "  ✓ Databases initialized" || echo "  ⚠ init-config returned non-zero (databases may already exist)"

echo ""

#-------------------------------------------------------------------------------
# STEP 6: CONFIGURE CLAUDE CODE
#-------------------------------------------------------------------------------

echo "=== Step 6: Configure Claude Code ==="

CLAUDE_JSON="$DETECTED_HOME/.claude.json"
SHIPPED_CONFIG="$SPF_HOME/config.json"

if [ ! -f "$SHIPPED_CONFIG" ]; then
    echo "  ✗ Shipped config.json missing: $SHIPPED_CONFIG"
    exit 1
fi

# Resolve paths — write to temp file for Python to read
SPF_TMP_CONFIG="/tmp/spf_setup_config.json"
if [ "$NEEDS_PATH_FIX" = true ]; then
    sed "s|$DEFAULT_HOME|$DETECTED_HOME|g" "$SHIPPED_CONFIG" > "$SPF_TMP_CONFIG"
else
    cp "$SHIPPED_CONFIG" "$SPF_TMP_CONFIG"
fi

if [ -f "$CLAUDE_JSON" ]; then
    echo "  Existing ~/.claude.json found"
    cp "$CLAUDE_JSON" "$CLAUDE_JSON.pre-spf-backup"
    echo "  ✓ Backed up to ~/.claude.json.pre-spf-backup"

    # Merge SPF sections into existing config
    python3 - "$CLAUDE_JSON" "$SPF_TMP_CONFIG" << 'PYEOF'
import json, sys, os

claude_path = sys.argv[1]
spf_path = sys.argv[2]
home_key = os.environ.get('HOME', '/data/data/com.termux/files/home')

with open(claude_path, 'r') as f:
    existing = json.load(f)
with open(spf_path, 'r') as f:
    spf_config = json.load(f)

# Merge mcpServers
existing.setdefault('mcpServers', {})
existing['mcpServers'].update(spf_config.get('mcpServers', {}))

# Merge hooks (replace entire event handlers)
existing.setdefault('hooks', {})
for event, handlers in spf_config.get('hooks', {}).items():
    existing['hooks'][event] = handlers

# Merge permissions
existing['permissions'] = spf_config.get('permissions', existing.get('permissions', {}))

# Merge project allowedTools
for proj_key, proj_data in spf_config.get('projects', {}).items():
    actual_key = proj_key.replace('/data/data/com.termux/files/home', home_key) if home_key != '/data/data/com.termux/files/home' else proj_key
    existing.setdefault('projects', {})
    existing['projects'].setdefault(actual_key, {})
    existing['projects'][actual_key].setdefault('allowedTools', [])
    for tool in proj_data.get('allowedTools', []):
        if tool not in existing['projects'][actual_key]['allowedTools']:
            existing['projects'][actual_key]['allowedTools'].append(tool)
    existing['projects'][actual_key]['hasTrustDialogAccepted'] = True
    existing['projects'][actual_key]['hasCompletedProjectOnboarding'] = True

with open(claude_path, 'w') as f:
    json.dump(existing, f, indent=2)

print("  ✓ Merged SPF config into existing ~/.claude.json")
PYEOF

else
    echo "  No existing ~/.claude.json — creating new"
    cp "$SPF_TMP_CONFIG" "$CLAUDE_JSON"
    echo "  ✓ Created ~/.claude.json with SPF config"
fi

# Clean up temp file
rm -f "$SPF_TMP_CONFIG"

echo ""

#-------------------------------------------------------------------------------
# STEP 7: SET PERMISSIONS
#-------------------------------------------------------------------------------

echo "=== Step 7: Set Permissions ==="

chmod +x "$SPF_HOME"/hooks/*.sh 2>/dev/null && echo "  ✓ hooks/ — all executable" || echo "  ⚠ No hooks found"
chmod +x "$SPF_HOME"/scripts/*.sh 2>/dev/null && echo "  ✓ scripts/ — all executable" || echo "  ⚠ No scripts found"
chmod +x "$SPF_HOME/build.sh" 2>/dev/null && echo "  ✓ build.sh — executable" || true
chmod +x "$BINARY" 2>/dev/null && echo "  ✓ binary — executable" || true

echo ""

#-------------------------------------------------------------------------------
# STEP 8: VERIFY
#-------------------------------------------------------------------------------

echo "=== Step 8: Verification ==="

PASS=0
FAIL=0

# Binary
if [ -x "$BINARY" ]; then
    echo "  ✓ Binary executable"
    PASS=$((PASS + 1))
else
    echo "  ✗ Binary not executable"
    FAIL=$((FAIL + 1))
fi

# Hooks
HOOK_COUNT=$(ls -1 "$SPF_HOME/hooks/"*.sh 2>/dev/null | wc -l)
echo "  ✓ Hooks: $HOOK_COUNT scripts"
PASS=$((PASS + 1))

# Config
if [ -f "$CLAUDE_JSON" ] && grep -q "spf-smart-gate" "$CLAUDE_JSON" 2>/dev/null; then
    echo "  ✓ Claude Code configured with SPF"
    PASS=$((PASS + 1))
else
    echo "  ✗ Claude Code config missing SPF"
    FAIL=$((FAIL + 1))
fi

# Databases
DB_COUNT=$(find "$SPF_HOME/LIVE" -name "*.DB" -type d 2>/dev/null | wc -l)
echo "  ✓ Database directories: $DB_COUNT"
PASS=$((PASS + 1))

echo ""

if [ $FAIL -eq 0 ]; then
    echo "==============================================================================="
    echo "                    ✓ SETUP COMPLETE — $PASS/$PASS checks passed"
    echo "==============================================================================="
    echo ""
    echo "  Start Claude Code:"
    echo "    $ claude"
    echo ""
    echo "  Verify SPF is active:"
    echo "    > spf_status"
    echo ""
    echo "  For LMDB5 containment (optional):"
    echo "    $ bash scripts/install-lmdb5.sh"
    echo ""
    echo "==============================================================================="
else
    echo "==============================================================================="
    echo "              ⚠ SETUP INCOMPLETE — $FAIL checks failed"
    echo "==============================================================================="
    echo ""
    echo "  Fix the issues above and rerun: bash setup.sh"
    echo ""
    echo "==============================================================================="
    exit 1
fi
