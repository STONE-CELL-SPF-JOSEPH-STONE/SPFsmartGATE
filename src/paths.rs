// SPF Smart Gateway - Path Resolution
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// Single source of truth for all SPF path resolution.
// Uses walk-up discovery from binary location — never depends on $HOME.
// Cached via OnceLock for zero-overhead repeated access.
//
// SECURITY NOTE: Write allowlist paths are computed here but ENFORCED
// in validate.rs. The allowlist remains compiled Rust, not configurable.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static SPF_ROOT_CACHE: OnceLock<PathBuf> = OnceLock::new();
static ACTUAL_HOME_CACHE: OnceLock<PathBuf> = OnceLock::new();

/// Find SPFsmartGATE root from binary location — never depends on $HOME.
///
/// Resolution order:
///   1. Walk up from binary location looking for Cargo.toml
///   2. SPF_ROOT environment variable
///   3. HOME env + /SPFsmartGATE
///   4. Panic (unrecoverable — cannot operate without known root)
pub fn spf_root() -> &'static Path {
    SPF_ROOT_CACHE.get_or_init(|| {
        // Primary: walk up from binary location
        if let Ok(exe) = std::env::current_exe() {
            if let Ok(canonical) = exe.canonicalize() {
                let mut dir = canonical.parent();
                while let Some(d) = dir {
                    if d.join("Cargo.toml").exists() {
                        return d.to_path_buf();
                    }
                    dir = d.parent();
                }
            }
        }

        // Fallback: SPF_ROOT environment variable
        if let Ok(root) = std::env::var("SPF_ROOT") {
            let p = PathBuf::from(&root);
            if p.exists() {
                return p;
            }
        }

        // Last resort: HOME/SPFsmartGATE
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join("SPFsmartGATE");
        }

        panic!("Cannot determine SPFsmartGATE root: binary walk-up failed, SPF_ROOT not set, HOME not set");
    })
}

/// Actual user home directory — parent of SPFsmartGATE root.
///
/// Resolution order:
///   1. Parent directory of spf_root()
///   2. HOME environment variable
///   3. Panic
pub fn actual_home() -> &'static Path {
    ACTUAL_HOME_CACHE.get_or_init(|| {
        if let Some(parent) = spf_root().parent() {
            return parent.to_path_buf();
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
        panic!("Cannot determine home directory: spf_root has no parent and HOME not set");
    })
}

/// System package manager path — platform-detected at compile time.
/// Android/Termux: PREFIX env or /data/data/com.termux/files/usr
/// Linux/macOS: /usr
pub fn system_pkg_path() -> String {
    if cfg!(target_os = "android") {
        if let Ok(prefix) = std::env::var("PREFIX") {
            return prefix;
        }
        "/data/data/com.termux/files/usr".to_string()
    } else {
        "/usr".to_string()
    }
}
