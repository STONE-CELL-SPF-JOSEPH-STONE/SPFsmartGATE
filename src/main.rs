// SPF Smart Gateway - Main Entry Point
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// CLI and MCP stdio server. All tool calls route through this gateway.
// Usage:
//   spf-smart-gate serve                                        # Run MCP server (stdio)
//   spf-smart-gate gate <tool> <params>                         # One-shot gate check
//   spf-smart-gate status                                       # Show gateway status
//   spf-smart-gate session                                      # Show session state
//   spf-smart-gate fs-import <virtual_path> <device_file>       # Import file to LMDB
//   spf-smart-gate fs-export <virtual_path> <device_file>       # Export file from LMDB
//   spf-smart-gate config-import <json_file>                    # Import config to CONFIG.DB
//   spf-smart-gate config-export <json_file>                    # Export config from CONFIG.DB

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use spf_smart_gate::{
    agent_state::AgentStateDb, calculate, config_db::SpfConfigDb, fs::SpfFs,
    gate, mcp, paths, session::Session, storage::SpfStorage,
};
use std::path::PathBuf;

fn default_storage_path() -> PathBuf {
    paths::spf_root().join("LIVE/SESSION/SESSION.DB")
}

#[derive(Parser)]
#[command(name = "spf-smart-gate")]
#[command(author = "Joseph Stone")]
#[command(version = "2.1.0")]
#[command(about = "SPF Smart Gateway - MCP command gateway with LMDB-backed configuration")]
struct Cli {
    /// Session storage directory (LIVE/SESSION/SESSION.DB)
    #[arg(short, long, default_value_os_t = default_storage_path())]
    storage: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run MCP server (stdio JSON-RPC)
    Serve,

    /// One-shot gate check — runs through SPF gate, returns allow/block
    Gate {
        /// Tool name (Read, Write, Edit, Bash, etc.)
        tool: String,

        /// Parameters as JSON string
        params: String,
    },

    /// Calculate complexity without executing
    Calculate {
        /// Tool name
        tool: String,

        /// Parameters as JSON string
        params: String,
    },

    /// Show gateway status
    Status,

    /// Show full session state
    Session,

    /// Reset session (fresh start)
    Reset,

    /// Initialize/verify LMDB config (auto-runs on startup)
    InitConfig,

    /// Refresh path rules in CONFIG.DB for current system.
    /// Only updates allowed_paths and blocked_paths.
    /// Preserves all other config (tiers, formula, weights, etc.)
    RefreshPaths {
        /// Show what would change without writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Import a device file into LMDB virtual filesystem.
    /// /home/agent/* paths route to LMDB5.DB (AgentStateDb).
    /// All other paths route to SPF_FS.DB.
    FsImport {
        /// Virtual path (e.g. /home/agent/.claude.json)
        virtual_path: String,

        /// Device file to read from
        device_file: PathBuf,

        /// Dry run — show what would happen without writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Export a file from LMDB virtual filesystem to device.
    /// /home/agent/* paths read from LMDB5.DB (AgentStateDb).
    /// All other paths read from SPF_FS.DB.
    FsExport {
        /// Virtual path (e.g. /home/agent/.claude.json)
        virtual_path: String,

        /// Device file to write to
        device_file: PathBuf,
    },

    /// Import config from JSON file into CONFIG.DB
    ConfigImport {
        /// JSON config file to import
        json_file: PathBuf,

        /// Dry run — show what would happen without writing
        #[arg(long)]
        dry_run: bool,
    },

    /// Export CONFIG.DB state to JSON file
    ConfigExport {
        /// Device file to write JSON to
        json_file: PathBuf,
    },
}

fn main() -> Result<()> {
    // Initialize logging (safe if already init)
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).try_init();

    let cli = Cli::parse();

    // Ensure storage directory exists
    std::fs::create_dir_all(&cli.storage)
        .with_context(|| format!("Failed to create storage dir {:?}", cli.storage))?;

    // Open SPF_CONFIG LMDB and load config (SINGLE SOURCE OF TRUTH)
    let config_db_path = paths::spf_root().join("LIVE/CONFIG/CONFIG.DB");
    let config_db = SpfConfigDb::open(&config_db_path)
        .with_context(|| format!("Failed to open SPF_CONFIG LMDB at {:?}", config_db_path))?;

    let config = config_db.load_full_config()
        .with_context(|| "Failed to load config from LMDB")?;

    // Open SPF_STATE storage
    let storage = SpfStorage::open(&cli.storage)
        .with_context(|| format!("Failed to open storage at {:?}", cli.storage))?;

    // Load or create session
    let session = storage.load_session()?.unwrap_or_else(Session::new);

    match &cli.command {
        Commands::Serve => {
            // Run MCP server — blocks forever, consumes session & storage
            mcp::run(config, config_db, session, storage);
            // Unreachable
        }

        Commands::Gate { tool, params } => {
            let params: calculate::ToolParams = serde_json::from_str(params)
                .with_context(|| format!("Invalid params JSON: {}", params))?;

            let decision = gate::process(tool, &params, &config, &session);

            println!("{}", serde_json::to_string_pretty(&decision)?);

            if !decision.allowed {
                std::process::exit(1);
            }

            // Save session after gate call
            storage.save_session(&session)?;
        }

        Commands::Calculate { tool, params } => {
            let params: calculate::ToolParams = serde_json::from_str(params)
                .with_context(|| format!("Invalid params JSON: {}", params))?;

            let result = calculate::calculate(tool, &params, &config);

            println!("{}", serde_json::to_string_pretty(&result)?);

            // Save session after calculate
            storage.save_session(&session)?;
        }

        Commands::Status => {
            println!("SPF Smart Gateway v2.1.0");
            println!("Mode: {:?}", config.enforce_mode);
            println!("Storage: {:?}", cli.storage);
            println!("Config: LMDB (CONFIG/CONFIG.DB)");
            println!();
            println!("Session: {}", session.status_summary());
            println!();
            println!("Tiers:");
            println!("  SIMPLE   < 500    | 40% analyze / 60% build");
            println!("  LIGHT    < 2000   | 60% analyze / 40% build");
            println!("  MEDIUM   < 10000  | 75% analyze / 25% build");
            println!("  CRITICAL > 10000  | 95% analyze / 5% build (requires approval)");
            println!();
            println!("Formula: a_optimal(C) = {} x (1 - 1/ln(C + e))", config.formula.w_eff);
            println!("Complexity: C = basic^1 + deps^7 + complex^10 + files x 10");
        }

        Commands::Session => {
            println!("{}", serde_json::to_string_pretty(&session)?);
        }

        Commands::Reset => {
            let new_session = Session::new();
            storage.save_session(&new_session)?;
            println!("Session reset.");
        }

        Commands::InitConfig => {
            // Config is already initialized via load_full_config() above
            // This command now just confirms the LMDB state
            let (config_count, paths_count, patterns_count) = config_db.stats()?;
            println!("SPF_CONFIG LMDB initialized at {:?}", config_db_path);
            println!("  Config entries: {}", config_count);
            println!("  Path rules: {}", paths_count);
            println!("  Dangerous patterns: {}", patterns_count);
            println!();
            println!("Config is stored in LMDB, not JSON files.");
            println!("Use MCP tools or direct LMDB access to modify.");
        }

        Commands::RefreshPaths { dry_run } => {
            let root = paths::spf_root().to_string_lossy().to_string();
            let home = paths::actual_home().to_string_lossy().to_string();
            let sys_pkg = spf_smart_gate::paths::system_pkg_path();

            // Build new path sets from current system
            let new_allowed: Vec<String> = vec![
                format!("{}/", home),
            ];
            let new_blocked: Vec<String> = vec![
                "/tmp".to_string(),
                "/etc".to_string(),
                "/usr".to_string(),
                "/system".to_string(),
                sys_pkg,
                format!("{}/src/", root),
                format!("{}/LIVE/SPF_FS/blobs/", root),
                format!("{}/Cargo.toml", root),
                format!("{}/Cargo.lock", root),
                format!("{}/.claude/", home),
            ];

            // Show current state
            let current_rules = config_db.list_path_rules()?;
            let cur_allowed: Vec<&str> = current_rules.iter()
                .filter(|(t, _)| t == "allowed").map(|(_, p)| p.as_str()).collect();
            let cur_blocked: Vec<&str> = current_rules.iter()
                .filter(|(t, _)| t == "blocked").map(|(_, p)| p.as_str()).collect();

            println!("=== SPF Refresh Paths ===");
            println!("SPF_ROOT: {}", root);
            println!("HOME:     {}", home);
            println!();
            println!("CURRENT allowed ({}):", cur_allowed.len());
            for p in &cur_allowed { println!("  + {}", p); }
            println!("CURRENT blocked ({}):", cur_blocked.len());
            for p in &cur_blocked { println!("  - {}", p); }
            println!();
            println!("NEW allowed ({}):", new_allowed.len());
            for p in &new_allowed { println!("  + {}", p); }
            println!("NEW blocked ({}):", new_blocked.len());
            for p in &new_blocked { println!("  - {}", p); }

            if *dry_run {
                println!();
                println!("[DRY RUN] No changes written.");
            } else {
                // Remove all existing path rules
                for (rule_type, path) in &current_rules {
                    config_db.remove_path_rule(rule_type, path)?;
                }
                // Write new rules
                for p in &new_allowed {
                    config_db.allow_path(p)?;
                }
                for p in &new_blocked {
                    config_db.block_path(p)?;
                }
                println!();
                println!("Path rules updated. {} allowed, {} blocked.",
                    new_allowed.len(), new_blocked.len());
                println!("All other config preserved (tiers, formula, weights, etc.)");
            }
        }

        // ====================================================================
        // LMDB VIRTUAL FILESYSTEM IMPORT/EXPORT
        // Routes /home/agent/* to LMDB5.DB, everything else to SPF_FS.DB
        // ====================================================================

        Commands::FsImport { virtual_path, device_file, dry_run } => {
            let data = std::fs::read(device_file)
                .with_context(|| format!("Failed to read device file: {:?}", device_file))?;

            println!("fs-import: {:?} -> {}", device_file, virtual_path);
            println!("  Size: {} bytes", data.len());

            if *dry_run {
                println!("  [DRY RUN] No changes made.");
                return Ok(());
            }

            // Route to correct LMDB based on virtual path
            if virtual_path.starts_with("/home/agent/") {
                // LMDB5.DB — Agent config and state files
                let relative = virtual_path.strip_prefix("/home/agent/").unwrap_or(virtual_path);
                let agent_db_path = paths::spf_root().join("LIVE/LMDB5/LMDB5.DB");
                let agent_db = AgentStateDb::open(&agent_db_path)
                    .with_context(|| format!("Failed to open LMDB5 at {:?}", agent_db_path))?;

                let content = String::from_utf8_lossy(&data).to_string();
                let key = format!("file:{}", relative);
                agent_db.set_state(&key, &content)
                    .with_context(|| format!("Failed to store in LMDB5: {}", key))?;

                // Verify
                let stored = agent_db.get_state(&key)?
                    .ok_or_else(|| anyhow::anyhow!("Write succeeded but read-back failed: {}", key))?;

                println!("  Target: LMDB5.DB (AgentState)");
                println!("  Key: {}", key);
                println!("  Stored: {} bytes", stored.len());
                println!("  OK");
            } else {
                // SPF_FS.DB — System virtual filesystem
                let fs_path = paths::spf_root().join("LIVE/SPF_FS");
                let spf_fs = SpfFs::open(&fs_path)
                    .with_context(|| format!("Failed to open SPF_FS at {:?}", fs_path))?;

                spf_fs.write(virtual_path, &data)
                    .with_context(|| format!("Failed to write to virtual path: {}", virtual_path))?;

                // Verify
                let meta = spf_fs.stat(virtual_path)?
                    .ok_or_else(|| anyhow::anyhow!("Write succeeded but stat failed for: {}", virtual_path))?;

                println!("  Target: SPF_FS.DB");
                println!("  Written: {} bytes (version {})", meta.size, meta.version);
                if let Some(ref checksum) = meta.checksum {
                    println!("  Checksum: {}", &checksum[..16]);
                }
                println!("  OK");
            }
        }

        Commands::FsExport { virtual_path, device_file } => {
            // Route to correct LMDB based on virtual path
            let data: Vec<u8> = if virtual_path.starts_with("/home/agent/") {
                // LMDB5.DB — Agent config and state files
                let relative = virtual_path.strip_prefix("/home/agent/").unwrap_or(virtual_path);
                let agent_db_path = paths::spf_root().join("LIVE/LMDB5/LMDB5.DB");
                let agent_db = AgentStateDb::open(&agent_db_path)
                    .with_context(|| format!("Failed to open LMDB5 at {:?}", agent_db_path))?;

                let key = format!("file:{}", relative);
                let content = agent_db.get_state(&key)?
                    .ok_or_else(|| anyhow::anyhow!("Not found in LMDB5: {}", key))?;

                println!("  Source: LMDB5.DB (AgentState)");
                println!("  Key: {}", key);
                content.into_bytes()
            } else {
                // SPF_FS.DB — System virtual filesystem
                let fs_path = paths::spf_root().join("LIVE/SPF_FS");
                let spf_fs = SpfFs::open(&fs_path)
                    .with_context(|| format!("Failed to open SPF_FS at {:?}", fs_path))?;

                println!("  Source: SPF_FS.DB");
                spf_fs.read(virtual_path)
                    .with_context(|| format!("Failed to read virtual path: {}", virtual_path))?
            };

            // Ensure parent directory exists on device
            if let Some(parent) = device_file.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(device_file, &data)
                .with_context(|| format!("Failed to write device file: {:?}", device_file))?;

            println!("fs-export: {} -> {:?}", virtual_path, device_file);
            println!("  Size: {} bytes", data.len());
            println!("  OK");
        }

        // ====================================================================
        // CONFIG.DB IMPORT/EXPORT
        // ====================================================================

        Commands::ConfigImport { json_file, dry_run } => {
            let json_str = std::fs::read_to_string(json_file)
                .with_context(|| format!("Failed to read config file: {:?}", json_file))?;

            let json: serde_json::Value = serde_json::from_str(&json_str)
                .with_context(|| "Invalid JSON in config file")?;

            println!("config-import: {:?}", json_file);

            // Enforce mode
            if let Some(mode) = json.get("enforce_mode").and_then(|v| v.as_str()) {
                println!("  enforce_mode: {}", mode);
                if !dry_run {
                    let mode = serde_json::from_value(json["enforce_mode"].clone())?;
                    config_db.set_enforce_mode(&mode)?;
                }
            }

            // Tiers
            if let Some(tiers_val) = json.get("tiers") {
                println!("  tiers: present");
                if !dry_run {
                    let tiers = serde_json::from_value(tiers_val.clone())?;
                    config_db.set_tiers(&tiers)?;
                }
            }

            // Formula
            if let Some(formula_val) = json.get("formula") {
                println!("  formula: present");
                if !dry_run {
                    let formula = serde_json::from_value(formula_val.clone())?;
                    config_db.set_formula(&formula)?;
                }
            }

            // Weights
            if let Some(weights_val) = json.get("weights") {
                println!("  weights: present");
                if !dry_run {
                    let weights = serde_json::from_value(weights_val.clone())?;
                    config_db.set_weights(&weights)?;
                }
            }

            // Allowed paths
            if let Some(paths) = json.get("allowed_paths").and_then(|v| v.as_array()) {
                println!("  allowed_paths: {} entries", paths.len());
                if !dry_run {
                    for path in paths {
                        if let Some(p) = path.as_str() {
                            config_db.allow_path(p)?;
                        }
                    }
                }
            }

            // Blocked paths
            if let Some(paths) = json.get("blocked_paths").and_then(|v| v.as_array()) {
                println!("  blocked_paths: {} entries", paths.len());
                if !dry_run {
                    for path in paths {
                        if let Some(p) = path.as_str() {
                            config_db.block_path(p)?;
                        }
                    }
                }
            }

            // Dangerous patterns
            if let Some(patterns) = json.get("dangerous_patterns").and_then(|v| v.as_object()) {
                println!("  dangerous_patterns: {} entries", patterns.len());
                if !dry_run {
                    for (pattern, severity) in patterns {
                        let sev = severity.as_u64().unwrap_or(5) as u8;
                        config_db.add_dangerous_pattern(pattern, sev)?;
                    }
                }
            }

            // Scalar config values
            if let Some(obj) = json.get("config").and_then(|v| v.as_object()) {
                println!("  config scalars: {} entries", obj.len());
                if !dry_run {
                    for (key, value) in obj {
                        if let Some(v) = value.as_str() {
                            config_db.set("spf", key, v)?;
                        }
                    }
                }
            }

            if *dry_run {
                println!("  [DRY RUN] No changes made.");
            } else {
                let (config_count, paths_count, patterns_count) = config_db.stats()?;
                println!("  Imported. DB now: {} configs, {} paths, {} patterns", config_count, paths_count, patterns_count);
            }
            println!("  OK");
        }

        Commands::ConfigExport { json_file } => {
            // Collect all config state
            let path_rules = config_db.list_path_rules()?;
            let mut allowed_paths = Vec::new();
            let mut blocked_paths = Vec::new();
            for (rule_type, path) in &path_rules {
                match rule_type.as_str() {
                    "allowed" => allowed_paths.push(path.clone()),
                    "blocked" => blocked_paths.push(path.clone()),
                    _ => {}
                }
            }

            let dangerous_patterns = config_db.list_dangerous_patterns()?;
            let mut patterns_map = serde_json::Map::new();
            for (pattern, severity) in &dangerous_patterns {
                patterns_map.insert(pattern.clone(), serde_json::json!(severity));
            }

            let export = serde_json::json!({
                "version": config.version,
                "enforce_mode": config.enforce_mode,
                "tiers": config.tiers,
                "formula": config.formula,
                "weights": config.complexity_weights,
                "allowed_paths": allowed_paths,
                "blocked_paths": blocked_paths,
                "dangerous_patterns": patterns_map,
                "config": {
                    "require_read_before_edit": config.require_read_before_edit.to_string(),
                    "max_write_size": config.max_write_size.to_string(),
                }
            });

            // Ensure parent directory exists
            if let Some(parent) = json_file.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let json_str = serde_json::to_string_pretty(&export)?;
            std::fs::write(json_file, &json_str)
                .with_context(|| format!("Failed to write config export: {:?}", json_file))?;

            println!("config-export: -> {:?}", json_file);
            println!("  {} configs, {} path rules, {} patterns",
                path_rules.len(), allowed_paths.len() + blocked_paths.len(), dangerous_patterns.len());
            println!("  {} bytes written", json_str.len());
            println!("  OK");
        }
    }

    Ok(())
}
