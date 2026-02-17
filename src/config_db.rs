// SPF Smart Gateway - Configuration LMDB
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// LMDB-backed configuration storage. Replaces config.json with persistent,
// transactional storage. Supports hot-reload without restart.
//
// Database: SPF_CONFIG
// Storage: ~/SPFsmartGATE/LIVE/CONFIG/CONFIG.DB/

use anyhow::{anyhow, Result};
use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};
use serde::{Deserialize, Serialize};
use std::path::Path;

// Import config types from canonical source (config.rs) - NO DUPLICATES
use crate::config::{
    EnforceMode, TierThreshold, TierConfig, FormulaConfig,
    ToolWeight, ComplexityWeights, SpfConfig,
};

const MAX_DB_SIZE: usize = 10 * 1024 * 1024; // 10MB - config is small

/// LMDB-backed SPF configuration storage
pub struct SpfConfigDb {
    env: Env,
    /// Main config store: namespace:key → JSON value
    config: Database<Str, Str>,
    /// Path rules: "allowed:path" or "blocked:path" → bool
    paths: Database<Str, SerdeBincode<bool>>,
    /// Dangerous patterns: pattern → severity (1-10)
    patterns: Database<Str, SerdeBincode<u8>>,
}

// ============================================================================
// IMPLEMENTATION
// ============================================================================

impl SpfConfigDb {
    /// Open or create config LMDB at given path
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAX_DB_SIZE)
                .max_dbs(8)
                .open(path)?
        };

        let mut wtxn = env.write_txn()?;
        let config = env.create_database(&mut wtxn, Some("config"))?;
        let paths = env.create_database(&mut wtxn, Some("paths"))?;
        let patterns = env.create_database(&mut wtxn, Some("patterns"))?;
        wtxn.commit()?;

        log::info!("SPF Config LMDB opened at {:?}", path);
        Ok(Self { env, config, paths, patterns })
    }

    // ========================================================================
    // CORE CONFIG OPERATIONS
    // ========================================================================

    /// Get a config value by namespace and key
    pub fn get(&self, namespace: &str, key: &str) -> Result<Option<String>> {
        let full_key = format!("{}:{}", namespace, key);
        let rtxn = self.env.read_txn()?;
        Ok(self.config.get(&rtxn, &full_key)?.map(|s| s.to_string()))
    }

    /// Set a config value
    pub fn set(&self, namespace: &str, key: &str, value: &str) -> Result<()> {
        let full_key = format!("{}:{}", namespace, key);
        let mut wtxn = self.env.write_txn()?;
        self.config.put(&mut wtxn, &full_key, value)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Get typed config value (deserialize from JSON)
    pub fn get_typed<T: for<'de> Deserialize<'de>>(&self, namespace: &str, key: &str) -> Result<Option<T>> {
        match self.get(namespace, key)? {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    /// Set typed config value (serialize to JSON)
    pub fn set_typed<T: Serialize>(&self, namespace: &str, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.set(namespace, key, &json)
    }

    // ========================================================================
    // PATH RULES
    // ========================================================================

    /// Add an allowed path
    pub fn allow_path(&self, path: &str) -> Result<()> {
        let key = format!("allowed:{}", path);
        let mut wtxn = self.env.write_txn()?;
        self.paths.put(&mut wtxn, &key, &true)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Add a blocked path
    pub fn block_path(&self, path: &str) -> Result<()> {
        let key = format!("blocked:{}", path);
        let mut wtxn = self.env.write_txn()?;
        self.paths.put(&mut wtxn, &key, &true)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Remove a path rule
    pub fn remove_path_rule(&self, rule_type: &str, path: &str) -> Result<bool> {
        let key = format!("{}:{}", rule_type, path);
        let mut wtxn = self.env.write_txn()?;
        let deleted = self.paths.delete(&mut wtxn, &key)?;
        wtxn.commit()?;
        Ok(deleted)
    }

    /// Check if path is allowed (with canonicalization to prevent traversal bypass)
    pub fn is_path_allowed(&self, path: &str) -> Result<bool> {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => {
                if path.contains("..") {
                    return Ok(false); // Traversal in unresolvable path = never allowed
                }
                path.to_string()
            }
        };
        let rtxn = self.env.read_txn()?;
        let iter = self.paths.iter(&rtxn)?;

        for result in iter {
            let (key, _) = result?;
            if key.starts_with("allowed:") {
                let allowed_path = &key[8..]; // Skip "allowed:"
                if canonical.starts_with(allowed_path) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Check if path is blocked (matches any blocked prefix)
    pub fn is_path_blocked(&self, path: &str) -> Result<bool> {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => {
                if path.contains("..") {
                    return Ok(true); // Traversal in unresolvable path = always blocked
                }
                path.to_string()
            }
        };

        let rtxn = self.env.read_txn()?;
        let iter = self.paths.iter(&rtxn)?;

        for result in iter {
            let (key, _) = result?;
            if key.starts_with("blocked:") {
                let blocked_path = &key[8..]; // Skip "blocked:"
                if canonical.starts_with(blocked_path) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// List all path rules
    pub fn list_path_rules(&self) -> Result<Vec<(String, String)>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.paths.iter(&rtxn)?;

        let mut rules = Vec::new();
        for result in iter {
            let (key, _) = result?;
            if let Some((rule_type, path)) = key.split_once(':') {
                rules.push((rule_type.to_string(), path.to_string()));
            }
        }
        Ok(rules)
    }

    // ========================================================================
    // DANGEROUS PATTERNS
    // ========================================================================

    /// Add a dangerous pattern with severity (1-10)
    pub fn add_dangerous_pattern(&self, pattern: &str, severity: u8) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.patterns.put(&mut wtxn, pattern, &severity.min(10))?;
        wtxn.commit()?;
        Ok(())
    }

    /// Check if command matches any dangerous pattern, returns severity
    pub fn check_dangerous(&self, command: &str) -> Result<Option<u8>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.patterns.iter(&rtxn)?;

        let mut max_severity: Option<u8> = None;
        for result in iter {
            let (pattern, severity) = result?;
            if command.contains(pattern) {
                max_severity = Some(max_severity.map_or(severity, |s| s.max(severity)));
            }
        }
        Ok(max_severity)
    }

    /// List all dangerous patterns
    pub fn list_dangerous_patterns(&self) -> Result<Vec<(String, u8)>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.patterns.iter(&rtxn)?;

        let mut patterns = Vec::new();
        for result in iter {
            let (pattern, severity) = result?;
            patterns.push((pattern.to_string(), severity));
        }
        Ok(patterns)
    }

    // ========================================================================
    // TIER CONFIG
    // ========================================================================

    /// Get tier config
    pub fn get_tiers(&self) -> Result<TierConfig> {
        self.get_typed::<TierConfig>("spf", "tiers")?
            .ok_or_else(|| anyhow!("Tier config not found"))
    }

    /// Set tier config
    pub fn set_tiers(&self, tiers: &TierConfig) -> Result<()> {
        self.set_typed("spf", "tiers", tiers)
    }

    /// Get tier for complexity value
    /// CRITICAL requires approval. Lower tiers protected by Build Anchor + path blocking + content inspection.
    pub fn get_tier_for_c(&self, c: u64) -> Result<(&'static str, u8, u8, bool)> {
        let tiers = self.get_tiers()?;

        if c < tiers.simple.max_c {
            Ok(("SIMPLE", tiers.simple.analyze_percent, tiers.simple.build_percent, tiers.simple.requires_approval))
        } else if c < tiers.light.max_c {
            Ok(("LIGHT", tiers.light.analyze_percent, tiers.light.build_percent, tiers.light.requires_approval))
        } else if c < tiers.medium.max_c {
            Ok(("MEDIUM", tiers.medium.analyze_percent, tiers.medium.build_percent, tiers.medium.requires_approval))
        } else {
            Ok(("CRITICAL", tiers.critical.analyze_percent, tiers.critical.build_percent, tiers.critical.requires_approval))
        }
    }

    // ========================================================================
    // FORMULA CONFIG
    // ========================================================================

    /// Get formula config
    pub fn get_formula(&self) -> Result<FormulaConfig> {
        self.get_typed::<FormulaConfig>("spf", "formula")?
            .ok_or_else(|| anyhow!("Formula config not found"))
    }

    /// Set formula config
    pub fn set_formula(&self, formula: &FormulaConfig) -> Result<()> {
        self.set_typed("spf", "formula", formula)
    }

    // ========================================================================
    // COMPLEXITY WEIGHTS
    // ========================================================================

    /// Get complexity weights
    pub fn get_weights(&self) -> Result<ComplexityWeights> {
        self.get_typed::<ComplexityWeights>("spf", "weights")?
            .ok_or_else(|| anyhow!("Complexity weights not found"))
    }

    /// Set complexity weights
    pub fn set_weights(&self, weights: &ComplexityWeights) -> Result<()> {
        self.set_typed("spf", "weights", weights)
    }

    /// Get weight for a specific tool
    pub fn get_tool_weight(&self, tool: &str) -> Result<ToolWeight> {
        let weights = self.get_weights()?;
        Ok(match tool.to_lowercase().as_str() {
            "edit" => weights.edit,
            "write" => weights.write,
            "bash_dangerous" => weights.bash_dangerous,
            "bash_git" => weights.bash_git,
            "bash_piped" => weights.bash_piped,
            "bash_simple" | "bash" => weights.bash_simple,
            "read" => weights.read,
            "search" | "glob" | "grep" => weights.search,
            _ => weights.unknown,
        })
    }

    // ========================================================================
    // ENFORCE MODE
    // ========================================================================

    /// Get enforce mode
    pub fn get_enforce_mode(&self) -> Result<EnforceMode> {
        self.get_typed::<EnforceMode>("spf", "enforce_mode")?
            .ok_or_else(|| anyhow!("Enforce mode not found"))
    }

    /// Set enforce mode
    pub fn set_enforce_mode(&self, mode: &EnforceMode) -> Result<()> {
        self.set_typed("spf", "enforce_mode", mode)
    }

    // ========================================================================
    // MIGRATION
    // ========================================================================

    /// Initialize with defaults (call once on first run)
    pub fn init_defaults(&self) -> Result<()> {
        // Only init if not already initialized
        if self.get("spf", "version")?.is_some() {
            return Ok(());
        }

        self.set("spf", "version", "1.0.0")?;
        self.set_enforce_mode(&EnforceMode::Max)?;
        self.set("spf", "require_read_before_edit", "true")?;
        self.set("spf", "max_write_size", "100000")?;

        // Default tiers — CRITICAL requires approval, lower tiers protected by other layers
        self.set_tiers(&TierConfig {
            simple: TierThreshold { max_c: 500, analyze_percent: 40, build_percent: 60, requires_approval: false },
            light: TierThreshold { max_c: 2000, analyze_percent: 60, build_percent: 40, requires_approval: false },
            medium: TierThreshold { max_c: 10000, analyze_percent: 75, build_percent: 25, requires_approval: false },
            critical: TierThreshold { max_c: u64::MAX, analyze_percent: 95, build_percent: 5, requires_approval: true },
        })?;

        // Default formula
        self.set_formula(&FormulaConfig {
            w_eff: 40000.0,
            e: std::f64::consts::E,
            basic_power: 1,
            deps_power: 7,
            complex_power: 10,
            files_multiplier: 10,
        })?;

        // Default weights
        self.set_weights(&ComplexityWeights {
            edit: ToolWeight { basic: 10, dependencies: 2, complex: 1, files: 1 },
            write: ToolWeight { basic: 20, dependencies: 2, complex: 1, files: 1 },
            bash_dangerous: ToolWeight { basic: 50, dependencies: 5, complex: 2, files: 1 },
            bash_git: ToolWeight { basic: 30, dependencies: 3, complex: 1, files: 1 },
            bash_piped: ToolWeight { basic: 20, dependencies: 3, complex: 1, files: 1 },
            bash_simple: ToolWeight { basic: 10, dependencies: 1, complex: 0, files: 1 },
            read: ToolWeight { basic: 5, dependencies: 1, complex: 0, files: 1 },
            search: ToolWeight { basic: 8, dependencies: 2, complex: 0, files: 1 },
            unknown: ToolWeight { basic: 20, dependencies: 3, complex: 1, files: 1 },
        })?;

        // Default allowed paths — resolved dynamically from paths module
        let home = crate::paths::actual_home().to_string_lossy();
        self.allow_path(&format!("{}/", home))?;

        // Default blocked paths — resolved dynamically from paths module
        let root = crate::paths::spf_root().to_string_lossy();
        self.block_path("/tmp")?;
        self.block_path("/etc")?;
        self.block_path("/usr")?;
        self.block_path("/system")?;
        self.block_path(&crate::paths::system_pkg_path())?;
        self.block_path(&format!("{}/src/", root))?;
        self.block_path(&format!("{}/LIVE/SPF_FS/blobs/", root))?;
        self.block_path(&format!("{}/Cargo.toml", root))?;
        self.block_path(&format!("{}/Cargo.lock", root))?;
        self.block_path(&format!("{}/.claude/", home))?;
        // System config and state — ZERO AI write access
        self.block_path(&format!("{}/LIVE/CONFIG.DB", root))?;
        self.block_path(&format!("{}/LIVE/LMDB5/", root))?;
        self.block_path(&format!("{}/LIVE/state/", root))?;
        self.block_path(&format!("{}/LIVE/storage/", root))?;
        self.block_path(&format!("{}/hooks/", root))?;
        self.block_path(&format!("{}/scripts/", root))?;

        // Default dangerous patterns
        self.add_dangerous_pattern("rm -rf /", 10)?;
        self.add_dangerous_pattern("rm -rf ~", 10)?;
        self.add_dangerous_pattern("dd if=", 9)?;
        self.add_dangerous_pattern("> /dev/", 9)?;
        self.add_dangerous_pattern("chmod 777", 7)?;
        self.add_dangerous_pattern("curl | sh", 8)?;
        self.add_dangerous_pattern("wget | sh", 8)?;
        self.add_dangerous_pattern("curl|sh", 8)?;
        self.add_dangerous_pattern("wget|sh", 8)?;

        log::info!("SPF Config LMDB initialized with defaults");
        Ok(())
    }

    /// Sync tier approval policy on every boot.
    /// Source of truth is THIS code — LMDB stores runtime state, code defines policy.
    /// Change the values here → next boot picks them up. No version tracking needed.
    pub fn sync_tier_approval(&self) -> Result<()> {
        let mut tiers = self.get_tiers()?;
        let mut changed = false;

        // === APPROVAL POLICY (edit here to change) ===
        let policy: [(&str, bool); 4] = [
            ("SIMPLE",   true),
            ("LIGHT",    true),
            ("MEDIUM",   true),
            ("CRITICAL", true),
        ];

        let tier_refs = [
            &mut tiers.simple,
            &mut tiers.light,
            &mut tiers.medium,
            &mut tiers.critical,
        ];

        for (i, (name, required)) in policy.iter().enumerate() {
            if tier_refs[i].requires_approval != *required {
                log::info!("SPF sync: {} requires_approval {} → {}", name, tier_refs[i].requires_approval, required);
                tier_refs[i].requires_approval = *required;
                changed = true;
            }
        }

        if changed {
            self.set_tiers(&tiers)?;
            log::info!("SPF tier approval policy synced");
        }

        // Keep version current
        self.set("spf", "version", "2.0.0")?;

        Ok(())
    }

    /// Get database stats
    pub fn stats(&self) -> Result<(u64, u64, u64)> {
        let rtxn = self.env.read_txn()?;
        let config_stat = self.config.stat(&rtxn)?;
        let paths_stat = self.paths.stat(&rtxn)?;
        let patterns_stat = self.patterns.stat(&rtxn)?;
        Ok((config_stat.entries as u64, paths_stat.entries as u64, patterns_stat.entries as u64))
    }

    // ========================================================================
    // FULL CONFIG ASSEMBLY (for main.rs - single source of truth)
    // ========================================================================

    /// Load full SpfConfig from LMDB. Auto-initializes if empty.
    /// This is the PRIMARY config loading method - replaces JSON file loading.
    pub fn load_full_config(&self) -> Result<SpfConfig> {
        // Ensure defaults exist, then sync approval policy from code
        self.init_defaults()?;
        self.sync_tier_approval()?;

        // Collect path rules
        let path_rules = self.list_path_rules()?;
        let mut allowed_paths = Vec::new();
        let mut blocked_paths = Vec::new();
        for (rule_type, path) in path_rules {
            match rule_type.as_str() {
                "allowed" => allowed_paths.push(path),
                "blocked" => blocked_paths.push(path),
                _ => {}
            }
        }

        // Collect dangerous commands
        let dangerous_commands: Vec<String> = self.list_dangerous_patterns()?
            .into_iter()
            .map(|(pattern, _)| pattern)
            .collect();

        // Get scalar values
        let version = self.get("spf", "version")?.unwrap_or_else(|| "1.0.0".to_string());
        let require_read = self.get("spf", "require_read_before_edit")?
            .map(|s| s == "true").unwrap_or(true);
        let max_write = self.get("spf", "max_write_size")?
            .and_then(|s| s.parse().ok()).unwrap_or(100_000);

        // Assemble config (types are now identical - no conversion needed)
        Ok(SpfConfig {
            version,
            enforce_mode: self.get_enforce_mode()?,
            allowed_paths,
            blocked_paths,
            require_read_before_edit: require_read,
            max_write_size: max_write,
            tiers: self.get_tiers()?,
            formula: self.get_formula()?,
            complexity_weights: self.get_weights()?,
            dangerous_commands,
            git_force_patterns: vec![
                "--force".to_string(),
                "--hard".to_string(),
                "-f".to_string(),
            ],
        })
    }
}
