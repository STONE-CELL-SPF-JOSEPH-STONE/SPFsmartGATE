// SPF Smart Gateway - Configuration
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// Loads SPF rules, tiers, formulas, blocked paths. Defaults stored in LMDB.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Master SPF configuration loaded from CONFIG LMDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpfConfig {
    pub version: String,
    pub enforce_mode: EnforceMode,
    pub allowed_paths: Vec<String>,
    pub blocked_paths: Vec<String>,
    pub require_read_before_edit: bool,
    pub max_write_size: usize,
    pub tiers: TierConfig,
    pub formula: FormulaConfig,
    pub complexity_weights: ComplexityWeights,
    pub dangerous_commands: Vec<String>,
    pub git_force_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EnforceMode {
    Soft,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    pub simple: TierThreshold,
    pub light: TierThreshold,
    pub medium: TierThreshold,
    pub critical: TierThreshold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierThreshold {
    pub max_c: u64,
    pub analyze_percent: u8,
    pub build_percent: u8,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaConfig {
    /// W_eff: effective working memory in tokens
    pub w_eff: f64,
    /// Euler's number
    pub e: f64,
    /// C = (basic ^ basic_power) + (deps ^ deps_power) + (complex ^ complex_power) + (files * files_mult)
    pub basic_power: u32,
    pub deps_power: u32,
    pub complex_power: u32,
    pub files_multiplier: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityWeights {
    pub edit: ToolWeight,
    pub write: ToolWeight,
    pub bash_dangerous: ToolWeight,
    pub bash_git: ToolWeight,
    pub bash_piped: ToolWeight,
    pub bash_simple: ToolWeight,
    pub read: ToolWeight,
    pub search: ToolWeight,
    pub unknown: ToolWeight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolWeight {
    pub basic: u64,
    pub dependencies: u64,
    pub complex: u64,
    pub files: u64,
}

impl Default for SpfConfig {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            enforce_mode: EnforceMode::Max,
            allowed_paths: {
                let home = crate::paths::actual_home().to_string_lossy();
                vec![
                    format!("{}/", home),
                ]
            },
            blocked_paths: {
                let root = crate::paths::spf_root().to_string_lossy();
                let home = crate::paths::actual_home().to_string_lossy();
                let mut paths = vec![
                    crate::paths::system_pkg_path(),
                    format!("{}/src/", root),
                    format!("{}/LIVE/SPF_FS/blobs/", root),
                    format!("{}/Cargo.toml", root),
                    format!("{}/Cargo.lock", root),
                    format!("{}/.claude/", home),
                    // System config and state — ZERO AI write access
                    format!("{}/LIVE/CONFIG.DB", root),
                    format!("{}/LIVE/LMDB5/", root),
                    format!("{}/LIVE/state/", root),
                    format!("{}/LIVE/storage/", root),
                    format!("{}/hooks/", root),
                    format!("{}/scripts/", root),
                ];
                if cfg!(target_os = "windows") {
                    paths.extend([
                        r"C:\Windows".to_string(),
                        r"C:\Program Files".to_string(),
                        r"C:\Program Files (x86)".to_string(),
                    ]);
                } else {
                    paths.extend([
                        "/tmp".to_string(),
                        "/etc".to_string(),
                        "/usr".to_string(),
                        "/system".to_string(),
                    ]);
                }
                paths
            },
            require_read_before_edit: true,
            max_write_size: 100_000,
            tiers: TierConfig {
                simple: TierThreshold { max_c: 500, analyze_percent: 40, build_percent: 60, requires_approval: true },
                light: TierThreshold { max_c: 2000, analyze_percent: 60, build_percent: 40, requires_approval: true },
                medium: TierThreshold { max_c: 10000, analyze_percent: 75, build_percent: 25, requires_approval: true },
                critical: TierThreshold { max_c: u64::MAX, analyze_percent: 95, build_percent: 5, requires_approval: true },
            },
            formula: FormulaConfig {
                w_eff: 40000.0,
                e: std::f64::consts::E,
                basic_power: 1,      // ^1 per SPF protocol
                deps_power: 7,       // ^7 per SPF protocol
                complex_power: 10,   // ^10 per SPF protocol
                files_multiplier: 10, // ×10 per SPF protocol
            },
            // Weights scaled for formula: C = basic^1 + deps^7 + complex^10 + files×10
            // deps^7: 2→128, 3→2187, 4→16384, 5→78125
            // complex^10: 1→1, 2→1024
            complexity_weights: ComplexityWeights {
                edit: ToolWeight { basic: 10, dependencies: 2, complex: 1, files: 1 },
                write: ToolWeight { basic: 20, dependencies: 2, complex: 1, files: 1 },
                bash_dangerous: ToolWeight { basic: 50, dependencies: 5, complex: 2, files: 1 },
                bash_git: ToolWeight { basic: 30, dependencies: 3, complex: 1, files: 1 },
                bash_piped: ToolWeight { basic: 20, dependencies: 3, complex: 1, files: 1 },
                bash_simple: ToolWeight { basic: 10, dependencies: 1, complex: 0, files: 1 },
                read: ToolWeight { basic: 5, dependencies: 1, complex: 0, files: 1 },
                search: ToolWeight { basic: 8, dependencies: 2, complex: 0, files: 1 },
                unknown: ToolWeight { basic: 20, dependencies: 3, complex: 1, files: 1 },
            },
            dangerous_commands: vec![
                "rm -rf /".to_string(),
                "rm -rf ~".to_string(),
                "dd if=".to_string(),
                "> /dev/".to_string(),
                "chmod 777".to_string(),
                "curl | sh".to_string(),
                "wget | sh".to_string(),
                "curl|sh".to_string(),
                "wget|sh".to_string(),
            ],
            git_force_patterns: vec![
                "--force".to_string(),
                "--hard".to_string(),
                "-f".to_string(),
            ],
        }
    }
}

impl SpfConfig {
    /// Load config from JSON file, falling back to defaults
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let config: Self = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            log::warn!("Config not found at {:?}, using defaults", path);
            Ok(Self::default())
        }
    }

    /// Save config to JSON file
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Get tier for a given complexity value
    /// CRITICAL tier requires explicit user approval. Lower tiers protected by other layers.
    pub fn get_tier(&self, c: u64) -> (&str, u8, u8, bool) {
        if c < self.tiers.simple.max_c {
            ("SIMPLE", self.tiers.simple.analyze_percent, self.tiers.simple.build_percent, self.tiers.simple.requires_approval)
        } else if c < self.tiers.light.max_c {
            ("LIGHT", self.tiers.light.analyze_percent, self.tiers.light.build_percent, self.tiers.light.requires_approval)
        } else if c < self.tiers.medium.max_c {
            ("MEDIUM", self.tiers.medium.analyze_percent, self.tiers.medium.build_percent, self.tiers.medium.requires_approval)
        } else {
            ("CRITICAL", self.tiers.critical.analyze_percent, self.tiers.critical.build_percent, self.tiers.critical.requires_approval)
        }
    }

    /// Check if a path is blocked (with canonicalization to prevent traversal bypass)
    pub fn is_path_blocked(&self, path: &str) -> bool {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => {
                if path.contains("..") {
                    return true; // Traversal in unresolvable path = always blocked
                }
                path.to_string()
            }
        };
        self.blocked_paths.iter().any(|blocked| canonical.starts_with(blocked))
    }

    /// Check if a path is allowed (with canonicalization to prevent traversal bypass)
    pub fn is_path_allowed(&self, path: &str) -> bool {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => {
                if path.contains("..") {
                    return false; // Traversal in unresolvable path = never allowed
                }
                path.to_string()
            }
        };
        self.allowed_paths.iter().any(|allowed| canonical.starts_with(allowed))
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_boundaries() {
        let config = SpfConfig::default();

        assert_eq!(config.get_tier(0).0, "SIMPLE");
        assert_eq!(config.get_tier(499).0, "SIMPLE");
        assert_eq!(config.get_tier(500).0, "LIGHT");
        assert_eq!(config.get_tier(1999).0, "LIGHT");
        assert_eq!(config.get_tier(2000).0, "MEDIUM");
        assert_eq!(config.get_tier(9999).0, "MEDIUM");
        assert_eq!(config.get_tier(10000).0, "CRITICAL");
        assert_eq!(config.get_tier(u64::MAX - 1).0, "CRITICAL");
    }

    #[test]
    fn default_formula_exponents() {
        let config = SpfConfig::default();
        assert_eq!(config.formula.basic_power, 1);
        assert_eq!(config.formula.deps_power, 7);
        assert_eq!(config.formula.complex_power, 10);
        assert_eq!(config.formula.files_multiplier, 10);
        assert_eq!(config.formula.w_eff, 40000.0);
    }

    #[test]
    fn default_enforce_mode_is_max() {
        let config = SpfConfig::default();
        assert_eq!(config.enforce_mode, EnforceMode::Max);
    }

    #[test]
    fn blocked_paths_include_system_dirs() {
        let config = SpfConfig::default();
        assert!(config.is_path_blocked("/tmp"));
        assert!(config.is_path_blocked("/tmp/evil.sh"));
        assert!(config.is_path_blocked("/etc/passwd"));
        assert!(config.is_path_blocked("/usr/bin/something"));
    }
}
