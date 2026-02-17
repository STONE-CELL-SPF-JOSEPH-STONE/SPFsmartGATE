#!/bin/bash
# SPF Build Script — Cross-platform compilation
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Detects current OS/arch and compiles the correct target binary.
# Copies the result to LIVE/BIN/ ready for deployment.
#
# Usage:
#   ./build.sh                  # Auto-detect and build for current system
#   ./build.sh --target linux   # Build for Linux x86_64
#   ./build.sh --target mac     # Build for macOS ARM (M-series)
#   ./build.sh --target macx86  # Build for macOS Intel
#   ./build.sh --target android # Build for Android/Termux ARM64
#   ./build.sh --release        # Release build (default)
#   ./build.sh --debug          # Debug build

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$SCRIPT_DIR"

BUILD_MODE="release"
TARGET_OVERRIDE=""

for arg in "$@"; do
    case "$arg" in
        --debug) BUILD_MODE="debug" ;;
        --release) BUILD_MODE="release" ;;
        --target)
            # Next arg is the target — handled below
            ;;
        linux|mac|macx86|android)
            TARGET_OVERRIDE="$arg"
            ;;
    esac
done

# Also handle --target <value> format
while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)
            TARGET_OVERRIDE="${2:-}"
            shift 2
            ;;
        *) shift ;;
    esac
done

# Detect current platform
detect_target() {
    local os=$(uname -s)
    local arch=$(uname -m)

    case "$os" in
        Linux)
            case "$arch" in
                aarch64|arm64)
                    # Check if Termux/Android
                    if [ -d "/data/data/com.termux" ]; then
                        echo "aarch64-linux-android"
                    else
                        echo "aarch64-unknown-linux-gnu"
                    fi
                    ;;
                x86_64)
                    echo "x86_64-unknown-linux-gnu"
                    ;;
                *)
                    echo "UNSUPPORTED: $os $arch"
                    ;;
            esac
            ;;
        Darwin)
            case "$arch" in
                arm64)
                    echo "aarch64-apple-darwin"
                    ;;
                x86_64)
                    echo "x86_64-apple-darwin"
                    ;;
                *)
                    echo "UNSUPPORTED: $os $arch"
                    ;;
            esac
            ;;
        *)
            echo "UNSUPPORTED: $os $arch"
            ;;
    esac
}

# Resolve target triple
resolve_target() {
    local override="$1"
    case "$override" in
        "")       detect_target ;;
        linux)    echo "x86_64-unknown-linux-gnu" ;;
        mac)      echo "aarch64-apple-darwin" ;;
        macx86)   echo "x86_64-apple-darwin" ;;
        android)  echo "aarch64-linux-android" ;;
        *)        echo "$override" ;;  # Allow raw triple
    esac
}

TARGET=$(resolve_target "$TARGET_OVERRIDE")

if [[ "$TARGET" == UNSUPPORTED* ]]; then
    echo "ERROR: $TARGET"
    exit 1
fi

echo "SPF Build v2.1.0"
echo "  SPF_ROOT:  $SPF_ROOT"
echo "  Target:    $TARGET"
echo "  Mode:      $BUILD_MODE"
echo ""

# Detect if this is a native (host) build or cross-compile
HOST_TARGET=$(rustc -vV 2>/dev/null | grep '^host:' | cut -d' ' -f2)
NATIVE_BUILD=false
if [ "$TARGET" = "$HOST_TARGET" ] || [ -z "$HOST_TARGET" ]; then
    NATIVE_BUILD=true
    echo "  Native:    yes (skipping --target flag)"
fi

# Cross-compile: try rustup for target setup (not available in Termux)
if [ "$NATIVE_BUILD" = false ]; then
    if command -v rustup &>/dev/null; then
        if ! rustup target list --installed 2>/dev/null | grep -q "$TARGET"; then
            echo "Adding rustup target: $TARGET"
            rustup target add "$TARGET"
        fi
    else
        echo "  NOTE: rustup not found — assuming target is available via system packages"
    fi
fi

# Build
cd "$SPF_ROOT"

BUILD_FLAGS=""
if [ "$NATIVE_BUILD" = false ]; then
    BUILD_FLAGS="--target $TARGET"
fi
if [ "$BUILD_MODE" = "release" ]; then
    BUILD_FLAGS="$BUILD_FLAGS --release"
fi

echo ""
echo "=== Building ==="
echo "  cargo build $BUILD_FLAGS"
echo ""
cargo build $BUILD_FLAGS

# Locate binary — native builds go to target/release/, cross to target/$TARGET/release/
if [ "$NATIVE_BUILD" = true ]; then
    BIN_DIR="$SPF_ROOT/target/${BUILD_MODE}"
else
    BIN_DIR="$SPF_ROOT/target/$TARGET/${BUILD_MODE}"
fi
BIN_PATH="$BIN_DIR/spf-smart-gate"

if [ ! -f "$BIN_PATH" ]; then
    echo "ERROR: Binary not found at $BIN_PATH"
    exit 1
fi

# Copy to LIVE/BIN
DEST="$SPF_ROOT/LIVE/BIN/spf-smart-gate"
cp "$BIN_PATH" "$DEST"
chmod +x "$DEST"

BIN_SIZE=$(du -h "$DEST" | cut -f1)

echo ""
echo "=== Build Complete ==="
echo "  Binary:  $DEST"
echo "  Size:    $BIN_SIZE"
echo "  Target:  $TARGET"
echo "  Mode:    $BUILD_MODE"
echo ""
echo "Run spf-deploy.sh to update settings.json if needed."
