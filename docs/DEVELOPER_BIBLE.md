# SPF Smart Gateway v2.0 - Complete Developer Bible

**FORENSIC DOCUMENTATION & OPERATIONAL MANUAL**

**Version:** 2.0.0
**Copyright:** 2026 Joseph Stone - All Rights Reserved
**Classification:** INTERNAL DEVELOPER REFERENCE
**Total Lines of Code:** 8,870 (Rust)
**Binary Size:** ~15MB (release)
**Storage Footprint:** ~222MB (LMDB databases)

---

# TABLE OF CONTENTS

1. [Project Overview](#1-project-overview)
2. [Complete File Tree](#2-complete-file-tree)
3. [Build System](#3-build-system)
4. [Dependency Analysis](#4-dependency-analysis)
5. [Module Architecture](#5-module-architecture)
6. [Data Structures Reference](#6-data-structures-reference)
7. [Function Reference](#7-function-reference)
8. [Hooks System (Claude Code Integration)](#8-hooks-system-claude-code-integration)
9. [LMDB Schema Documentation](#9-lmdb-schema-documentation)
10. [MCP Protocol Implementation](#10-mcp-protocol-implementation)
11. [SPF Formula Specification](#11-spf-formula-specification)
12. [Gate Pipeline Internals](#12-gate-pipeline-internals)
13. [Security Model](#13-security-model)
14. [Configuration Reference](#14-configuration-reference)
15. [Error Handling](#15-error-handling)
16. [Insert Points Guide](#16-insert-points-guide)
17. [Change Impact Analysis](#17-change-impact-analysis)
18. [Operational Manual](#18-operational-manual)
19. [Troubleshooting](#19-troubleshooting)
20. [LMDB 5 Full Containment](#20-lmdb-5-full-containment)

---

# 1. PROJECT OVERVIEW

## 1.1 Purpose

SPF Smart Gateway is a **compiled Rust enforcement layer** that sits between AI agents (Claude Code) and system resources. It implements:

- **Complexity-based access control** using the SPF formula
- **Build Anchor Protocol** requiring reads before writes
- **Path enforcement** with allowed/blocked directories
- **Dangerous command detection** with pattern matching
- **Full audit logging** of all AI operations
- **5 LMDB databases** for persistent state

## 1.2 Design Philosophy

```
┌─────────────────────────────────────────────────────────────────┐
│                     CORE PRINCIPLE                               │
│                                                                  │
│  "No AI hallucination gets past compiled Rust logic."           │
│                                                                  │
│  Behavioral constraints MUST be enforced in compiled code,      │
│  NOT in prompt engineering. The AI cannot override Rust.        │
└─────────────────────────────────────────────────────────────────┘
```

## 1.3 Key Metrics

| Metric | Value |
|--------|-------|
| Source Files | 19 Rust files (+2 EDITED versions) |
| Total Lines | 8,870 |
| Largest File | mcp.rs (2,103 lines) |
| Dependencies | 27 crates |
| LMDB Databases | 5 (config, tools, sandbox, agent_state, fs) |
| MCP Tools | 54 definitions (+5 blocked handlers) |
| Complexity Tiers | 4 |
| Hook Scripts | 18 |
| Structs/Enums | 45 |

---

# 2. COMPLETE FILE TREE

```
SPFsmartGATE/
├── Cargo.toml                    # Build manifest, all dependencies
├── Cargo.lock                    # Locked dependency versions
├── LICENSE                       # Commercial license
├── README.md                     # Public overview
├── HANDOFF.md                    # Session continuity notes
├── config.json                   # JSON config backup (16KB)
├── .claude.json                  # Claude-specific config
│
├── docs/
│   ├── DEVELOPER_BIBLE.md        # THIS FILE
│   ├── ARCHITECTURE.md           # High-level architecture
│   └── FORENSIC_AUDIT_REPORT.md  # Audit report
│
├── src/                          # 8,870 lines total
│   ├── main.rs                   # 190 lines  - CLI entry point
│   ├── lib.rs                    #  33 lines  - Library exports
│   ├── config.rs                 # 196 lines  - Configuration types
│   ├── config_db.rs              # 448 lines  - LMDB config operations
│   ├── calculate.rs              # 311 lines  - SPF complexity formula
│   ├── validate.rs               # 155 lines  - Rule validation
│   ├── gate.rs                   # 130 lines  - Primary enforcement
│   ├── gate-EDITED.rs            # 172 lines  - Edited gate version
│   ├── inspect.rs                # 144 lines  - Content inspection
│   ├── session.rs                # 156 lines  - Session state tracking
│   ├── storage.rs                # 100 lines  - LMDB persistence layer
│   ├── tools_db.rs               # 433 lines  - Tool registry LMDB
│   ├── sandbox_db.rs             # 609 lines  - Project sandbox LMDB
│   ├── agent_state.rs            # 683 lines  - Agent memory LMDB
│   ├── claude_state.rs           # 670 lines  - Claude-specific state
│   ├── fs.rs                     # 628 lines  - Virtual filesystem
│   ├── web.rs                    # 289 lines  - HTTP client
│   ├── mcp.rs                    # 2103 lines - MCP server (LARGEST)
│   └── mcp-EDITED.rs             # 1420 lines - Edited MCP version
│
├── claude/                       # Claude working directory
│   └── INSTRUCTIONS.md           # Claude tool instructions (4.5KB)
│
├── hooks/                        # Claude Code integration (18 files)
│   ├── spf-gate.sh               # Main enforcement hook
│   ├── pre-bash.sh               # Pre-bash gate
│   ├── pre-edit.sh               # Pre-edit gate
│   ├── pre-read.sh               # Pre-read gate
│   ├── pre-write.sh              # Pre-write gate
│   ├── pre-glob.sh               # Pre-glob gate
│   ├── pre-grep.sh               # Pre-grep gate
│   ├── pre-webfetch.sh           # Pre-webfetch gate
│   ├── pre-websearch.sh          # Pre-websearch gate
│   ├── pre-notebookedit.sh       # Pre-notebook gate
│   ├── post-action.sh            # Post-execution logging
│   ├── post-failure.sh           # Failure handler
│   ├── session-start.sh          # Session initialization
│   ├── session-end.sh            # Session cleanup
│   ├── stop-check.sh             # Stop condition checks
│   └── user-prompt.sh            # User prompt handler
│
├── agent-bin/                    # Binary copies
│   ├── spf-smart-gate            # Gateway binary
│   └── claude-code               # Claude Code binary
│
├── storage/                      # LMDB databases (222MB total)
│   ├── data.mdb                  # Root session storage
│   ├── lock.mdb                  # Root lock
│   ├── spf_config/               # Configuration (60KB)
│   │   ├── data.mdb
│   │   └── lock.mdb
│   ├── spf_tools/                # Tool registry (56KB)
│   │   ├── data.mdb
│   │   └── lock.mdb
│   ├── spf_sandbox/              # Project sandboxes (20KB)
│   │   ├── data.mdb
│   │   └── lock.mdb
│   ├── agent_state/              # Agent memory (60KB)
│   │   ├── data.mdb
│   │   └── lock.mdb
│   ├── blobs/                    # Large file storage (221MB)
│   │   └── claude-code/          # Vendor files (ripgrep, etc.)
│   └── staging/                  # Staging area
│       └── configs/
│
└── target/
    └── release/
        └── spf-smart-gate        # Binary (~15MB)
```

---

# 3. BUILD SYSTEM

## 3.1 Cargo.toml Analysis

**Location:** `/SPFsmartGATE/Cargo.toml`

```toml
[package]
name = "spf-smart-gate"
version = "2.0.0"
edition = "2021"
authors = ["Joseph Stone"]
description = "SPF Smart Gateway - AI governance framework"
license = "Proprietary"

[lib]
name = "spf_smart_gate"
path = "src/lib.rs"

[[bin]]
name = "spf-smart-gate"
path = "src/main.rs"

[dependencies]
# Core
anyhow = "1.0"           # Error handling
thiserror = "1.0"        # Custom errors
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# CLI
clap = { version = "4.4", features = ["derive"] }
env_logger = "0.10"
log = "0.4"

# Database
heed = "0.20"            # LMDB bindings

# Crypto
sha2 = "0.10"            # SHA256 hashing
hex = "0.4"              # Hex encoding

# Time
chrono = { version = "0.4", features = ["serde"] }

# Web
reqwest = { version = "0.11", features = ["blocking", "json"] }
html2text = "0.6"        # HTML to text

# Glob
glob = "0.3"

[profile.release]
opt-level = 3
lto = true               # Link-time optimization
strip = true             # Strip symbols
codegen-units = 1        # Single codegen unit for better optimization
```

## 3.2 Build Commands

```bash
# Development build (fast, debug symbols)
cargo build

# Release build (optimized, ~15MB)
cargo build --release

# Check without building
cargo check

# Run tests
cargo test

# Build with all warnings
RUSTFLAGS="-W warnings" cargo build --release
```

## 3.3 Build Output

```
target/release/spf-smart-gate    # Main binary
target/release/libspf_smart_gate.rlib  # Library
```

## 3.4 Cross-Compilation

```bash
# For Android/Termux (aarch64)
cargo build --release --target aarch64-linux-android

# For Linux x86_64
cargo build --release --target x86_64-unknown-linux-gnu
```

---

# 4. DEPENDENCY ANALYSIS

## 4.1 Crate Dependency Graph

```
spf-smart-gate
├── anyhow (error handling)
├── thiserror (custom errors)
├── serde + serde_json (serialization)
├── clap (CLI parsing)
├── heed (LMDB bindings)
│   └── lmdb-sys (C bindings)
├── sha2 + hex (cryptography)
├── chrono (time handling)
├── reqwest (HTTP client)
│   ├── tokio (async runtime - blocking mode)
│   ├── hyper (HTTP)
│   └── rustls (TLS)
├── html2text (HTML conversion)
├── glob (pattern matching)
├── env_logger + log (logging)
└── dirs (home directory)
```

## 4.2 Critical Dependencies

| Crate | Purpose | Version | Notes |
|-------|---------|---------|-------|
| `heed` | LMDB bindings | 0.20 | Core persistence |
| `serde` | Serialization | 1.0 | All data structures |
| `reqwest` | HTTP | 0.11 | Blocking mode only |
| `clap` | CLI | 4.4 | Derive macros |
| `chrono` | Time | 0.4 | Timestamps |

## 4.3 Why These Choices

- **heed over lmdb-rkv**: More idiomatic Rust API, better type safety
- **reqwest blocking**: Avoids async complexity for CLI tool
- **anyhow over custom errors**: Faster development, good ergonomics
- **clap derive**: Minimal boilerplate for CLI

---

# 5. MODULE ARCHITECTURE

## 5.1 Module Dependency Graph

```
                              ┌────────────┐
                              │  main.rs   │
                              │  (Entry)   │
                              └─────┬──────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
              ┌─────▼─────┐   ┌─────▼─────┐   ┌─────▼─────┐
              │  mcp.rs   │   │ gate.rs   │   │ config.rs │
              │  (Server) │   │ (Enforce) │   │  (Types)  │
              └─────┬─────┘   └─────┬─────┘   └───────────┘
                    │               │
        ┌───────────┼───────────────┼───────────────┐
        │           │               │               │
  ┌─────▼─────┐ ┌───▼───┐     ┌─────▼─────┐   ┌─────▼─────┐
  │ session   │ │ web   │     │ calculate │   │ validate  │
  │   .rs     │ │ .rs   │     │    .rs    │   │    .rs    │
  └───────────┘ └───────┘     └───────────┘   └─────┬─────┘
                                                    │
                                              ┌─────▼─────┐
                                              │ inspect   │
                                              │   .rs     │
                                              └───────────┘

  ┌─────────────────────────────────────────────────────────┐
  │                    LMDB LAYER                           │
  ├─────────────┬─────────────┬─────────────┬──────────────┤
  │ config_db   │ tools_db    │ sandbox_db  │ agent_state  │
  │    .rs      │    .rs      │    .rs      │     .rs      │
  └─────────────┴─────────────┴─────────────┴──────────────┘
                              │
                        ┌─────▼─────┐
                        │ storage   │
                        │   .rs     │
                        └───────────┘
```

## 5.2 Module Responsibilities

### main.rs (190 lines)
**Purpose:** CLI entry point and subcommand dispatch

**Imports:**
```rust
use spf_smart_gate::{calculate, config_db::SpfConfigDb, gate, mcp, session::Session, storage::SpfStorage};
use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
```

**Key Functions:**
- `main()` - Entry point, CLI parsing
- `home_dir()` - Get home directory
- `default_storage_path()` - Default LMDB path

**Subcommands:**
- `serve` - Run MCP server
- `gate` - One-shot gate check
- `calculate` - Calculate complexity
- `status` - Show gateway status
- `session` - Show session state
- `reset` - Reset session
- `init-config` - Initialize LMDB config

### lib.rs (33 lines)
**Purpose:** Library exports for binary and tests

```rust
pub mod calculate;
pub mod config;
pub mod config_db;
pub mod gate;
pub mod inspect;
pub mod mcp;
pub mod session;
pub mod storage;
pub mod validate;
pub mod web;
pub mod fs;
pub mod tools_db;
pub mod sandbox_db;
pub mod agent_state;
pub mod claude_state;
```

### config.rs (196 lines)
**Purpose:** Core configuration types

**Key Structs:**
- `SpfConfig` - Master configuration
- `EnforceMode` - Hard/Soft enforcement
- `TierConfig` - Tier thresholds
- `TierThreshold` - Per-tier settings
- `FormulaConfig` - SPF formula parameters (w_eff, e, powers)
- `ComplexityWeights` - Weight categories
- `ToolWeight` - Per-tool complexity weights (basic, dependencies, complex, files)

**Key Methods:**
- `SpfConfig::default()` - Default configuration
- `SpfConfig::load()` - Load from JSON file
- `SpfConfig::save()` - Save to JSON file
- `SpfConfig::get_tier()` - Get tier for C value
- `SpfConfig::is_path_blocked()` - Check blocked paths (with canonicalization)
- `SpfConfig::is_path_allowed()` - Check allowed paths

### config_db.rs (448 lines)
**Purpose:** LMDB operations for SPF_CONFIG

**Key Functions:**
- `open()` - Open/create LMDB
- `init_defaults()` - Initialize default config
- `load_full_config()` - Load SpfConfig from LMDB
- `get_path_rules()` - Get allowed/blocked paths
- `get_dangerous_patterns()` - Get command patterns
- `stats()` - Get database statistics

### calculate.rs (311 lines)
**Purpose:** SPF complexity formula implementation

**Key Structs:**
- `ToolParams` - Tool call parameters (15 fields for all tool types)
- `ComplexityResult` - Calculation result (tool, c, tier, percentages, tokens)

**Key Functions:**
- `calc_complex_factor()` - Calculate dynamic complexity (0-4 scale)
- `calc_files_factor()` - Calculate files factor based on scope
- `is_architectural_file()` - Check if file is config/main/lib/mod
- `has_risk_indicators()` - Check for delete/drop/remove/force/unsafe
- `calculate_c()` - Main C value calculation with all tool types
- `a_optimal()` - Master formula: W_eff × (1 - 1/ln(C + e))
- `calculate()` - Full calculation returning ComplexityResult

### validate.rs (155 lines)
**Purpose:** Rule validation logic

**Key Structs:**
- `ValidationResult` - valid bool, warnings vec, errors vec

**Key Functions:**
- `validate_edit()` - Validate Edit (Build Anchor + blocked paths)
- `validate_write()` - Validate Write (size limit + anchor + blocked)
- `validate_bash()` - Validate Bash (dangerous commands + git force + /tmp)
- `validate_read()` - Validate Read (always allowed, /tmp blocked)

### gate.rs (130 lines)
**Purpose:** Primary enforcement checkpoint

**Key Structs:**
- `GateDecision` - Allow/block decision (allowed, tool, complexity, warnings, errors, message)

**Key Functions:**
- `process()` - Main gate processing pipeline:
  1. Calculate complexity
  2. Check approval requirement
  3. Validate against rules (calls validate_edit/write/bash/read)
  4. Content inspection (calls inspect_content)
  5. Return GateDecision

### inspect.rs (144 lines)
**Purpose:** Content pattern inspection

**Constants:**
- `CREDENTIAL_PATTERNS` - 19 patterns (sk-, AKIA, ghp_, private keys, etc.)
- `SHELL_INJECTION_PATTERNS` - 4 patterns ($(, eval, exec, backtick)

**Key Functions:**
- `inspect_content()` - Main inspection (skips code files for shell patterns)
- `check_credentials()` - Scan for API keys, tokens, private keys
- `check_path_traversal()` - Detect ../ sequences
- `check_shell_injection()` - Detect command substitution
- `check_blocked_path_references()` - Detect blocked path mentions

### session.rs (156 lines)
**Purpose:** Session state tracking

**Key Structs:**
- `Session` - Current session state (action_count, files_read/written, last_*, timestamps, history)
- `ComplexityEntry` - Complexity history (timestamp, tool, c, tier)
- `ManifestEntry` - Action log (timestamp, tool, c, action, reason)
- `FailureEntry` - Failure log (timestamp, tool, error)

**Key Functions:**
- `new()` - Create new session
- `track_read()` - Record file read (Build Anchor)
- `track_write()` - Record file write
- `record_action()` - Log action with result
- `record_complexity()` - Log complexity (keeps last 100)
- `record_manifest()` - Log allowed/blocked (keeps last 200)
- `record_failure()` - Log failure (keeps last 50)
- `anchor_ratio()` - Calculate reads/writes ratio
- `status_summary()` - Get summary string

### storage.rs (100 lines)
**Purpose:** LMDB persistence layer

**Key Structs:**
- `SpfStorage` - Storage manager

**Key Functions:**
- `open()` - Open storage directory
- `load_session()` - Load session from LMDB
- `save_session()` - Save session to LMDB

### tools_db.rs (433 lines)
**Purpose:** Tool registry LMDB

**Key Structs:**
- `ToolEntry` - Tool metadata
- `ToolPermission` - Permission level
- `ToolStats` - Usage statistics

**Key Functions:**
- `open()` - Open LMDB
- `init_defaults()` - Register default tools
- `get_tool()` - Get tool by name
- `list_tools()` - List all tools
- `record_call()` - Record tool usage

### sandbox_db.rs (609 lines)
**Purpose:** Project sandbox LMDB

**Key Structs:**
- `ProjectSandbox` - Project workspace
- `TrustLevel` - Trust classification
- `AccessLogEntry` - Access history

**Key Functions:**
- `open()` - Open LMDB
- `register_project()` - Register new project
- `get_project()` - Get project by path
- `set_active()` - Set active project
- `log_access()` - Log file access

### agent_state.rs (683 lines)
**Purpose:** Agent memory and state

**Key Structs:**
- `Memory` - Stored memory
- `MemoryType` - Memory classification
- `SessionRecord` - Session history
- `AgentPreference` - Agent preferences

**Key Functions:**
- `open()` - Open LMDB
- `remember()` - Store memory
- `recall()` - Search memories
- `search_memories()` - Full-text search
- `get_by_tag()` - Get memories by tag
- `get_context_summary()` - Session continuity

### fs.rs (628 lines)
**Purpose:** Virtual filesystem

**Key Structs:**
- `SpfFs` - Filesystem manager
- `FileMetadata` - File metadata
- `FileType` - File/Directory/Symlink

**Key Functions:**
- `open()` - Open LMDB filesystem
- `read()` - Read file content
- `write()` - Write file (hybrid: LMDB + disk blobs)
- `mkdir()` / `mkdir_p()` - Create directories
- `ls()` - List directory
- `rm()` / `rm_rf()` - Remove files/directories
- `rename()` - Rename/move
- `index_vector()` - Link to brain vector

### web.rs (289 lines)
**Purpose:** HTTP client for web access

**Key Structs:**
- `WebClient` - HTTP client wrapper
- `SearchResult` - Search result

**Key Functions:**
- `new()` - Create client
- `search_brave()` - Brave Search API
- `search_ddg()` - DuckDuckGo fallback
- `search()` - Auto-select search engine
- `read_page()` - Fetch and convert to text
- `download()` - Download file
- `api_request()` - Generic API call

### mcp.rs (2,103 lines) - **LARGEST FILE**
**Purpose:** MCP JSON-RPC 2.0 server

**Key Functions:**
- `format_timestamp()` - Format Unix timestamp
- `brain_path()` / `dirs_home()` - Path helpers
- `run_brain()` - Call stoneshell-brain binary
- `run_rag()` - Call RAG collector
- `log()` - Log to stderr
- `send_response()` / `send_error()` - JSON-RPC responses
- `tool_def()` - Build tool definition JSON
- `tool_definitions()` - Return all 54 tool definitions
- `handle_tool_call()` - Dispatch tool calls (1300+ lines)
- `run()` - Main server loop (initializes 4 LMDBs)

**Tool Definitions:** (54 total)
- Core: `spf_gate`, `spf_calculate`, `spf_status`, `spf_session`
- File: `spf_read`, `spf_write`, `spf_edit`, `spf_bash`
- Search: `spf_glob`, `spf_grep`
- Web: `spf_web_search`, `spf_web_fetch`, `spf_web_download`, `spf_web_api`
- Notebook: `spf_notebook_edit`
- Brain: `spf_brain_*` (9 tools)
- RAG: `spf_rag_*` (14 tools)
- Config: `spf_config_paths`, `spf_config_stats`
- Tools: `spf_tools_*` (3 tools)
- Sandbox: `spf_sandbox_*` (4 tools)
- Agent: `spf_agent_*` (5 tools)

**Blocked Handlers (no definitions):**
- `spf_config_get`, `spf_config_set` - User-only via CLI
- `spf_agent_remember`, `spf_agent_forget`, `spf_agent_set_state` - User-only

---

# 6. DATA STRUCTURES REFERENCE

## 6.1 Core Types

### SpfConfig (config.rs:11-23)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpfConfig {
    pub version: String,                    // Config version
    pub enforce_mode: EnforceMode,          // Hard or Soft
    pub allowed_paths: Vec<String>,         // Whitelisted paths
    pub blocked_paths: Vec<String>,         // Blacklisted paths
    pub require_read_before_edit: bool,     // Build Anchor toggle
    pub max_write_size: usize,              // Max write size (100KB default)
    pub tiers: TierConfig,                  // Tier thresholds
    pub formula: FormulaConfig,             // SPF parameters
    pub complexity_weights: ComplexityWeights,  // Weight categories
    pub dangerous_commands: Vec<String>,    // Dangerous command patterns
    pub git_force_patterns: Vec<String>,    // Git force patterns
}
```

### EnforceMode (config.rs:47-52)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnforceMode {
    Hard,  // Block on violations
    Soft,  // Warn but allow
}
```

### TierConfig (config.rs:54-62)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    pub simple: TierThreshold,    // C < 500
    pub light: TierThreshold,     // C < 2000
    pub medium: TierThreshold,    // C < 10000
    pub critical: TierThreshold,  // C >= 10000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierThreshold {
    pub max_c: u64,              // Maximum complexity for this tier
    pub analyze_percent: u8,     // % for analysis (0-100)
    pub build_percent: u8,       // % for building (0-100)
    pub requires_approval: bool, // Always true (hardcoded policy)
}
```

### FormulaConfig (config.rs:48-59)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaConfig {
    pub w_eff: f64,              // Effective working memory (40000.0)
    pub e: f64,                  // Euler's number (std::f64::consts::E)
    pub basic_power: u32,        // Basic exponent (1)
    pub deps_power: u32,         // Dependencies exponent (7)
    pub complex_power: u32,      // Complex exponent (10)
    pub files_multiplier: u64,   // Files multiplier (10)
}
```

### ComplexityWeights (config.rs:61-72)
```rust
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
```

### ToolParams (calculate.rs:24-49)
```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolParams {
    // Common
    pub file_path: Option<String>,
    // Edit
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub replace_all: Option<bool>,
    // Write
    pub content: Option<String>,
    // Bash
    pub command: Option<String>,
    // Search (glob/grep)
    pub query: Option<String>,
    pub pattern: Option<String>,
    pub path: Option<String>,
    // Brain operations
    pub collection: Option<String>,
    pub limit: Option<u64>,
    pub text: Option<String>,
    pub title: Option<String>,
    // RAG/Web operations
    pub url: Option<String>,
    pub topic: Option<String>,
    pub category: Option<String>,
}
```

### ComplexityResult (calculate.rs:12-20)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityResult {
    pub tool: String,              // Tool name
    pub c: u64,                    // Complexity value
    pub tier: String,              // SIMPLE/LIGHT/MEDIUM/CRITICAL
    pub analyze_percent: u8,       // % for analysis
    pub build_percent: u8,         // % for building
    pub a_optimal_tokens: u64,     // Optimal tokens from master formula
    pub requires_approval: bool,   // Always true
}
```

### GateDecision (gate.rs:15-28)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    pub allowed: bool,             // Final decision
    pub tool: String,              // Tool name
    pub complexity: ComplexityResult,
    pub warnings: Vec<String>,     // Non-blocking issues
    pub errors: Vec<String>,       // Blocking issues
    pub message: String,           // Human-readable summary
}
```

### Session (session.rs:12-24)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub action_count: u64,                        // Total operations
    pub files_read: Vec<String>,                  // Build Anchor tracking
    pub files_written: Vec<String>,               // Write audit
    pub last_tool: Option<String>,                // Last tool used
    pub last_result: Option<String>,              // Last result
    pub last_file: Option<String>,                // Last file accessed
    pub started: DateTime<Utc>,                   // Session start (chrono)
    pub last_action: Option<DateTime<Utc>>,       // Last action time
    pub complexity_history: Vec<ComplexityEntry>, // Last 100 entries
    pub manifest: Vec<ManifestEntry>,             // Last 200 entries
    pub failures: Vec<FailureEntry>,              // Last 50 entries
}
```

### ToolEntry (tools_db.rs:25-42)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub permission: ToolPermission,
    pub is_mcp: bool,
    pub mcp_server: Option<String>,
    pub created_at: u64,
    pub stats: ToolStats,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolPermission {
    Allow,
    Deny,
    RequireApproval,
}
```

### ProjectSandbox (sandbox_db.rs:30-55)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSandbox {
    pub name: String,
    pub path: String,
    pub trust_level: TrustLevel,
    pub is_active: bool,
    pub created_at: u64,
    pub last_accessed: u64,
    pub total_reads: u64,
    pub total_writes: u64,
    pub session_writes: u64,
    pub max_writes_per_session: u64,
    pub max_write_size: u64,
    pub total_complexity: u64,
    pub protected_paths: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Untrusted,
    Low,
    Medium,
    High,
    Full,
}
```

### Memory (agent_state.rs:30-48)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub memory_type: MemoryType,
    pub tags: Vec<String>,
    pub created_at: u64,
    pub accessed_at: u64,
    pub access_count: u64,
    pub importance: u8,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryType {
    Fact,
    Preference,
    Instruction,
    Context,
    Temporary,
}
```

### FileMetadata (fs.rs:30-48)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub file_type: FileType,
    pub size: u64,
    pub mode: u32,
    pub created_at: i64,
    pub modified_at: i64,
    pub checksum: Option<String>,
    pub version: u64,
    pub vector_id: Option<String>,  // Link to brain vector
    pub real_path: Option<String>,  // Disk blob path (hybrid storage)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}
```

---

# 7. FUNCTION REFERENCE

## 7.1 Core Pipeline Functions

### gate::process() (gate.rs:30-95)
```rust
pub fn process(
    tool: &str,
    params: &ToolParams,
    config: &SpfConfig,
    session: &Session,
    approved: bool,
) -> GateDecision
```

**Flow:**
1. Calculate complexity: `calculate::calculate(tool, params, config)`
2. Check approval: All tiers require `approved == true`
3. Validate path: `validate::validate_path(params, config)`
4. Validate anchor: `validate::validate_anchor(params, session)`
5. Validate command: `validate::validate_command(params, config)`
6. Inspect content: `inspect::inspect_content(params, config)`
7. Return `GateDecision`

### calculate::calculate() (calculate.rs:55-120)
```rust
pub fn calculate(tool: &str, params: &ToolParams, config: &SpfConfig) -> ComplexityResult
```

**Formula:**
```
C = (basic ^ 1) + (deps ^ 7) + (complex ^ 10) + (files × 10)
```

**Flow:**
1. Get tool weight from config
2. Calculate basic from content size
3. Calculate deps from tool type
4. Calculate complex from patterns
5. Calculate files from file_path
6. Apply formula
7. Classify tier
8. Calculate token allocation

### validate::validate_path() (validate.rs:25-60)
```rust
pub fn validate_path(params: &ToolParams, config: &SpfConfig) -> ValidationResult
```

**Flow:**
1. Canonicalize path (resolve symlinks, ..)
2. Check against `config.blocked_paths`
3. Check against `config.allowed_paths`
4. Return Valid/Invalid with reason

### validate::validate_anchor() (validate.rs:62-85)
```rust
pub fn validate_anchor(params: &ToolParams, session: &Session) -> ValidationResult
```

**Flow:**
1. Get file_path from params
2. Check if tool is write operation (Write/Edit)
3. Check if file_path is in `session.files_read`
4. Return Valid/Invalid

### inspect::inspect_content() (inspect.rs:20-65)
```rust
pub fn inspect_content(content: &str, config: &SpfConfig) -> InspectionResult
```

**Flow:**
1. For each pattern in `config.dangerous_patterns`
2. If pattern matches content, add to findings
3. Return findings with severity

## 7.2 Session Functions

### Session::track_read() (session.rs:80-90)
```rust
pub fn track_read(&mut self, path: &str)
```
Adds path to `files_read` if not already present.

### Session::has_read() (session.rs:92-98)
```rust
pub fn has_read(&self, path: &str) -> bool
```
Returns true if path was previously read (anchor check).

### Session::record_complexity() (session.rs:115-130)
```rust
pub fn record_complexity(&mut self, tool: &str, c: u64, tier: &str)
```
Adds entry to `complexity_history`, updates `total_complexity`.

### Session::record_manifest() (session.rs:132-150)
```rust
pub fn record_manifest(&mut self, tool: &str, c: u64, status: &str, error: Option<&str>)
```
Logs action to `manifest` with timestamp.

## 7.3 LMDB Functions

### SpfConfigDb::load_full_config() (config_db.rs:180-230)
```rust
pub fn load_full_config(&self) -> Result<SpfConfig>
```

**Flow:**
1. Call `init_defaults()` if empty
2. Load enforce_mode from LMDB
3. Load tier thresholds
4. Load formula config
5. Load path rules
6. Load dangerous patterns
7. Assemble and return SpfConfig

### SpfToolsDb::record_call() (tools_db.rs:200-250)
```rust
pub fn record_call(&self, name: &str, allowed: bool, complexity: u64) -> Result<()>
```

**Flow:**
1. Get existing tool or create new
2. Update stats (call_count, allowed/blocked, avg_complexity)
3. Write to history table
4. Save updated tool

### AgentStateDb::remember() (agent_state.rs:150-200)
```rust
pub fn remember(&self, content: &str, memory_type: MemoryType, tags: &[String]) -> Result<String>
```

**Flow:**
1. Generate UUID for memory
2. Create Memory struct
3. Write to memories table
4. Index by tags
5. Return memory ID

## 7.4 MCP Functions

### mcp::run() (mcp.rs:1957-2103)
```rust
pub fn run(config: SpfConfig, mut session: Session, storage: SpfStorage)
```

**Flow:**
1. Initialize all 5 LMDB databases
2. Enter infinite loop reading stdin
3. Parse JSON-RPC message
4. Match on method:
   - `initialize` → capabilities response
   - `tools/list` → tool definitions
   - `tools/call` → `handle_tool_call()`
   - `ping` → pong
5. Send JSON-RPC response

### mcp::handle_tool_call() (mcp.rs:625-1954)
```rust
fn handle_tool_call(name: &str, args: &Value, ...) -> Value
```

**Pattern (for gated tools):**
```rust
1. Extract parameters from args
2. Build ToolParams
3. Call gate::process()
4. If !decision.allowed → return BLOCKED
5. Execute actual operation
6. Track in session
7. Save session
8. Return result
```

---

# 8. HOOKS SYSTEM (Claude Code Integration)

## 8.1 Overview

The hooks system provides Claude Code integration via shell scripts that intercept tool calls before execution.

```
User Request
    ↓
Claude Code
    ↓
pre-{tool}.sh  →  spf-gate.sh  →  Rust Gateway (spf-smart-gate gate)
    ↓                                      ↓
[BLOCKED]                            [ALLOWED]
    ↓                                      ↓
post-failure.sh                      post-action.sh
```

## 8.2 Hook Files

| Hook | Size | Purpose |
|------|------|---------|
| `spf-gate.sh` | 2.6KB | Main enforcement - calls Rust gateway |
| `pre-bash.sh` | 494B | Pre-bash gate |
| `pre-edit.sh` | 494B | Pre-edit gate |
| `pre-read.sh` | 518B | Pre-read gate |
| `pre-write.sh` | 498B | Pre-write gate |
| `pre-glob.sh` | 435B | Pre-glob gate |
| `pre-grep.sh` | 435B | Pre-grep gate |
| `pre-webfetch.sh` | 461B | Pre-webfetch gate |
| `pre-websearch.sh` | 480B | Pre-websearch gate |
| `pre-notebookedit.sh` | 467B | Pre-notebook gate |
| `post-action.sh` | 5.4KB | Post-execution logging |
| `post-failure.sh` | 1.5KB | Failure handler |
| `session-start.sh` | 2.6KB | Session initialization |
| `session-end.sh` | 2.5KB | Session cleanup |
| `stop-check.sh` | 1.4KB | Stop condition checks |
| `user-prompt.sh` | 6.5KB | User prompt handler |

## 8.3 spf-gate.sh Flow

```bash
# 1. Check gateway binary exists (fail-closed)
if [ ! -x "$GATEWAY" ]; then
    exit 1  # BLOCKED
fi

# 2. Call Rust gateway
RESULT=$("$GATEWAY" gate "$TOOL_NAME" "$TOOL_INPUT" $APPROVED_FLAG)
EXIT_CODE=$?

# 3. Exit code is enforcement decision
# 0 = ALLOWED, 1 = BLOCKED
exit $EXIT_CODE
```

---

# 9. LMDB SCHEMA DOCUMENTATION

## 9.1 Database Overview

| Database | Path | Size | Tables | Purpose |
|----------|------|------|--------|---------|
| ROOT | storage/ | varies | 1 | Session storage |
| SPF_CONFIG | storage/spf_config | 60KB | 3 | Configuration |
| SPF_TOOLS | storage/spf_tools | 56KB | 3 | Tool registry |
| SPF_SANDBOX | storage/spf_sandbox | 20KB | 3 | Project sandboxes |
| AGENT_STATE | storage/agent_state | 60KB | 4 | Agent memory |
| BLOBS | storage/blobs/ | 221MB | N/A | Large file storage (disk) |

## 9.2 Root Session Storage

```
storage/data.mdb + lock.mdb
  Key:   "session"
  Value: SerdeBincode<Session>
```

## 9.3 SPF_CONFIG Schema

```
Table: config
  Key:   String (config key)
  Value: SerdeBincode<Value>

  Keys:
    "enforce_mode" → EnforceMode
    "tier_simple_max" → u64
    "tier_simple_analyze" → f64
    "tier_simple_build" → f64
    ...etc for each tier
    "formula_w_eff" → u64
    "formula_basic_exp" → u32
    ...etc for formula

Table: paths
  Key:   String ("allowed:{path}" or "blocked:{path}")
  Value: String (path)

Table: patterns
  Key:   String (pattern)
  Value: SerdeBincode<DangerousPattern>
```

## 9.4 SPF_TOOLS Schema

```
Table: tools
  Key:   String (tool name)
  Value: SerdeBincode<ToolEntry>

Table: history
  Key:   String ("{timestamp}:{tool}:{action}")
  Value: SerdeBincode<ToolHistoryEntry>

Table: aliases
  Key:   String (alias)
  Value: String (tool name)
```

## 9.5 SPF_SANDBOX Schema

```
Table: projects
  Key:   String (project path)
  Value: SerdeBincode<ProjectSandbox>

Table: access_log
  Key:   String ("{timestamp}:{project}:{path}")
  Value: SerdeBincode<AccessLogEntry>

Table: resources
  Key:   String ("{project}:{resource_path}")
  Value: SerdeBincode<ResourceEntry>
```

## 9.6 AGENT_STATE Schema

```
Table: memories
  Key:   String (memory UUID)
  Value: SerdeBincode<Memory>

Table: sessions
  Key:   String (session UUID)
  Value: SerdeBincode<SessionRecord>

Table: state
  Key:   String (state key)
  Value: SerdeBincode<Value>

Table: tags
  Key:   String ("{tag}:{memory_id}")
  Value: String (memory_id) - index for tag lookup
```

---

# 10. MCP PROTOCOL IMPLEMENTATION

## 10.1 Protocol Overview

- **Protocol:** JSON-RPC 2.0 over stdio
- **Protocol Version:** 2024-11-05
- **Transport:** stdin/stdout (line-delimited JSON)
- **Logging:** stderr (not visible to client)

## 10.2 Handshake

**Client → Server:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": {},
    "clientInfo": {"name": "claude-code", "version": "2.0"}
  }
}
```

**Server → Client:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {"tools": {}},
    "serverInfo": {"name": "spf-smart-gate", "version": "1.0.0"}
  }
}
```

## 10.3 Tool Listing

**Client → Server:**
```json
{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}
```

**Server → Client:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "tools": [
      {
        "name": "spf_gate",
        "description": "Run a tool call through SPF enforcement gate...",
        "inputSchema": {
          "type": "object",
          "properties": {
            "tool": {"type": "string"},
            "params": {"type": "object"},
            "approved": {"type": "boolean", "default": true}
          },
          "required": ["tool", "params"]
        }
      },
      // ... 57 more tools
    ]
  }
}
```

## 10.4 Tool Call

**Client → Server:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "spf_read",
    "arguments": {
      "file_path": "/home/user/file.txt"
    }
  }
}
```

**Server → Client:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "File: /home/user/file.txt (42 lines)\n     1\tline 1 content\n..."
      }
    ]
  }
}
```

## 10.5 Complete Tool List (54 definitions)

### Core Gate (4)
| Tool | Description |
|------|-------------|
| `spf_gate` | Run tool through enforcement gate |
| `spf_calculate` | Calculate complexity without executing |
| `spf_status` | Gateway status |
| `spf_session` | Full session state |

### File Operations (4)
| Tool | Description |
|------|-------------|
| `spf_read` | Read file (builds anchor) |
| `spf_write` | Write file (requires anchor) |
| `spf_edit` | Edit file (requires anchor) |
| `spf_bash` | Execute bash command |

### Search (2)
| Tool | Description |
|------|-------------|
| `spf_glob` | File pattern matching |
| `spf_grep` | Content search (ripgrep) |

### Web (4)
| Tool | Description |
|------|-------------|
| `spf_web_search` | Search via Brave/DDG |
| `spf_web_fetch` | Fetch URL as text |
| `spf_web_download` | Download file |
| `spf_web_api` | API request |

### Notebook (1)
| Tool | Description |
|------|-------------|
| `spf_notebook_edit` | Edit Jupyter cell |

### Brain (8)
| Tool | Description |
|------|-------------|
| `spf_brain_search` | Search brain vectors |
| `spf_brain_store` | Store document |
| `spf_brain_context` | Get context for query |
| `spf_brain_index` | Index file/directory |
| `spf_brain_list` | List collections |
| `spf_brain_status` | Brain system status |
| `spf_brain_recall` | Full document retrieval |
| `spf_brain_list_docs` | List documents |
| `spf_brain_get_doc` | Get document by ID |

### RAG (14)
| Tool | Description |
|------|-------------|
| `spf_rag_collect_web` | Web search + collect |
| `spf_rag_collect_file` | Process local file |
| `spf_rag_collect_folder` | Process folder |
| `spf_rag_collect_drop` | Process DROP_HERE |
| `spf_rag_index_gathered` | Index collected docs |
| `spf_rag_dedupe` | Deduplicate collection |
| `spf_rag_status` | Collector status |
| `spf_rag_list_gathered` | List gathered docs |
| `spf_rag_bandwidth_status` | Bandwidth stats |
| `spf_rag_fetch_url` | Fetch single URL |
| `spf_rag_collect_rss` | Collect from RSS |
| `spf_rag_list_feeds` | List RSS feeds |
| `spf_rag_pending_searches` | Get search gaps |
| `spf_rag_fulfill_search` | Mark gap fulfilled |
| `spf_rag_smart_search` | Search with gap detection |
| `spf_rag_auto_fetch_gaps` | Auto-fetch for gaps |

### Config (2)
| Tool | Description |
|------|-------------|
| `spf_config_paths` | List path rules |
| `spf_config_stats` | Config LMDB stats |

### Tools DB (3)
| Tool | Description |
|------|-------------|
| `spf_tools_list` | List registered tools |
| `spf_tools_stats` | Tools LMDB stats |
| `spf_tools_get` | Get tool info |

### Sandbox (4)
| Tool | Description |
|------|-------------|
| `spf_sandbox_list` | List project sandboxes |
| `spf_sandbox_stats` | Sandbox LMDB stats |
| `spf_sandbox_get` | Get project info |
| `spf_sandbox_active` | Get active project |

### Agent State (5)
| Tool | Description |
|------|-------------|
| `spf_agent_stats` | Agent LMDB stats |
| `spf_agent_memory_search` | Search memories |
| `spf_agent_memory_by_tag` | Get by tag |
| `spf_agent_session_info` | Latest session |
| `spf_agent_context` | Context summary |

---

# 11. SPF FORMULA SPECIFICATION

## 11.1 Complexity Calculation

### Base Formula
```
C = (basic ^ basic_exp) + (deps ^ deps_exp) + (complex ^ complex_exp) + (files × files_mult)
```

### Default Parameters
```
basic_exp = 1
deps_exp = 7
complex_exp = 10
files_mult = 10
```

### Expanded Form
```
C = basic + deps^7 + complex^10 + files×10
```

## 11.2 Component Calculation

### Basic Component
```rust
let basic = match tool {
    "Read" => content_size / 1000,           // 1 per KB
    "Write" => content_size / 100,           // 10 per KB
    "Edit" => (old_len + new_len) / 50,      // 20 per KB of changes
    "Bash" => command.len() as u64,          // 1 per char
    _ => 10,                                  // Default
};
```

### Dependencies Component
```rust
let deps = match tool {
    "Write" | "Edit" => 2,   // Requires anchor
    "Bash" => 3,             // External effects
    "web_*" => 2,            // Network
    _ => 1,                  // No dependencies
};
// Applied: deps^7
// Values: 1→1, 2→128, 3→2187, 4→16384
```

### Complex Component
```rust
let complex = match tool {
    "Bash" if has_pipe => 2,         // Piped commands
    "Bash" if has_redirect => 2,     // Redirections
    "Edit" if replace_all => 2,      // Multiple changes
    "Write" if large_file => 2,      // Large writes
    _ => 1,
};
// Applied: complex^10
// Values: 1→1, 2→1024, 3→59049
```

### Files Component
```rust
let files = if file_path.is_some() { 1 } else { 0 };
// Applied: files×10
```

## 11.3 Tier Classification

| Tier | C Range | Analyze | Build |
|------|---------|---------|-------|
| SIMPLE | 0 - 499 | 40% | 60% |
| LIGHT | 500 - 1,999 | 60% | 40% |
| MEDIUM | 2,000 - 9,999 | 75% | 25% |
| CRITICAL | 10,000+ | 95% | 5% |

## 11.4 Token Allocation

### Master Formula
```
a_optimal(C) = W_eff × (1 - 1/ln(C + e))
```

Where:
- `W_eff = 40,000` (effective working memory)
- `e = Euler's number (2.71828...)`

### Calculation
```rust
let w_eff = config.formula.w_eff as f64;
let e = std::f64::consts::E;
let a_optimal = w_eff * (1.0 - 1.0 / (c as f64 + e).ln());

let analyze_tokens = (a_optimal * tier.analyze_ratio) as u64;
let build_tokens = (a_optimal * tier.build_ratio) as u64;
```

## 11.5 Examples

### Example 1: Simple Read
```
Tool: Read
File size: 5000 bytes

basic = 5000 / 1000 = 5
deps = 1 (no dependencies)
complex = 1
files = 1

C = 5 + 1^7 + 1^10 + 1×10
C = 5 + 1 + 1 + 10 = 17

Tier: SIMPLE (C < 500)
Analyze: 40%, Build: 60%
```

### Example 2: Large Write
```
Tool: Write
File size: 50000 bytes

basic = 50000 / 100 = 500
deps = 2 (requires anchor)
complex = 2 (large file)
files = 1

C = 500 + 2^7 + 2^10 + 1×10
C = 500 + 128 + 1024 + 10 = 1662

Tier: LIGHT (500 <= C < 2000)
Analyze: 60%, Build: 40%
```

### Example 3: Dangerous Bash
```
Tool: Bash
Command: "rm -rf /some/path && echo done"

basic = 35 (command length)
deps = 3 (external effects)
complex = 2 (has && operator)
files = 0

C = 35 + 3^7 + 2^10 + 0
C = 35 + 2187 + 1024 = 3246

Tier: MEDIUM (2000 <= C < 10000)
Analyze: 75%, Build: 25%
```

---

# 12. GATE PIPELINE INTERNALS

## 12.1 Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           GATE PIPELINE                                      │
│                                                                              │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐  │
│  │  INPUT   │──▶│ CALCULATE│──▶│ APPROVAL │──▶│ VALIDATE │──▶│ INSPECT  │  │
│  │          │   │          │   │          │   │          │   │          │  │
│  │ tool     │   │ C value  │   │ approved │   │ path     │   │ content  │  │
│  │ params   │   │ tier     │   │ check    │   │ anchor   │   │ patterns │  │
│  │ config   │   │ tokens   │   │          │   │ command  │   │          │  │
│  │ session  │   │          │   │          │   │          │   │          │  │
│  └──────────┘   └──────────┘   └──────────┘   └──────────┘   └──────────┘  │
│        │              │              │              │              │        │
│        │              │              │              │              │        │
│        ▼              ▼              ▼              ▼              ▼        │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                         DECISION                                     │  │
│   │                                                                      │  │
│   │  if any_errors:                                                     │  │
│   │      allowed = false                                                │  │
│   │      errors = [...all blocking issues...]                          │  │
│   │  else:                                                              │  │
│   │      allowed = true                                                 │  │
│   │      warnings = [...non-blocking issues...]                        │  │
│   │                                                                      │  │
│   └─────────────────────────────────────────────────────────────────────┘  │
│                                    │                                        │
│                                    ▼                                        │
│                            ┌──────────────┐                                 │
│                            │ GateDecision │                                 │
│                            │ {            │                                 │
│                            │   allowed,   │                                 │
│                            │   tool,      │                                 │
│                            │   complexity,│                                 │
│                            │   warnings,  │                                 │
│                            │   errors,    │                                 │
│                            │   message    │                                 │
│                            │ }            │                                 │
│                            └──────────────┘                                 │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 12.2 Validation Chain

### Path Validation (validate.rs:25-60)
```
1. Extract file_path from params
2. Canonicalize path (resolve .., symlinks)
3. Check blocked_paths:
   for blocked in config.blocked_paths:
       if path.starts_with(blocked):
           return Invalid("Path blocked: {blocked}")
4. Check allowed_paths:
   if allowed_paths not empty:
       found = false
       for allowed in config.allowed_paths:
           if path.starts_with(allowed):
               found = true
               break
       if not found:
           return Invalid("Path not in allowed list")
5. return Valid
```

### Anchor Validation (validate.rs:62-85)
```
1. If tool not in [Write, Edit]:
   return Valid (anchor only for writes)
2. Extract file_path from params
3. If file_path in session.files_read:
   return Valid
4. return Invalid("Build Anchor: Must read file before editing")
```

### Command Validation (validate.rs:87-120)
```
1. If tool != "Bash":
   return Valid
2. Extract command from params
3. for pattern in config.dangerous_patterns:
   if regex_match(pattern.pattern, command):
       if pattern.severity >= 8:
           return Invalid("Dangerous: {pattern.description}")
       else:
           add Warning
4. return Valid (with warnings)
```

### Content Inspection (inspect.rs:20-65)
```
1. Extract content from params
2. for pattern in config.content_patterns:
   if regex_match(pattern, content):
       add Finding(pattern, severity)
3. if any findings with severity >= 9:
   return Invalid
4. return Valid (with warnings for lower severity)
```

## 12.3 Enforcement Modes

### Hard Mode (Default)
- **Behavior:** Block on any validation error
- **Use case:** Production, untrusted environments
- **Decision:** `allowed = all_validations_passed`

### Soft Mode
- **Behavior:** Warn but allow
- **Use case:** Development, debugging
- **Decision:** `allowed = true` (errors become warnings)

---

# 13. SECURITY MODEL

## 13.1 Threat Model

**Adversary:** AI agent attempting to:
- Access unauthorized files
- Execute dangerous commands
- Bypass anchor requirements
- Modify system files
- Exfiltrate data

**Trust Boundaries:**
```
┌─────────────────────────────────────────────────────────────────┐
│                       UNTRUSTED                                  │
│                                                                  │
│  ┌─────────────┐                                                │
│  │  AI Agent   │◀───── Prompts, hallucinations, jailbreaks     │
│  │ (Claude)    │                                                │
│  └──────┬──────┘                                                │
│         │ MCP Tool Calls                                        │
│         │                                                        │
└─────────┼────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                    ENFORCEMENT BOUNDARY                          │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              SPF Smart Gateway (Rust)                       ││
│  │                                                              ││
│  │  • All requests validated                                   ││
│  │  • Compiled logic (not prompt)                              ││
│  │  • Full audit logging                                       ││
│  │  • LMDB state (tamper-evident)                              ││
│  └─────────────────────────────────────────────────────────────┘│
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────┐
│                        PROTECTED                                 │
│                                                                  │
│  File System    │  External APIs    │  Brain Index              │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## 13.2 Security Controls

### Path Traversal Prevention
```rust
// All paths are canonicalized before checking
let canonical = std::fs::canonicalize(path)
    .map(|p| p.to_string_lossy().to_string())
    .unwrap_or_else(|_| path.to_string());

// Prevents: ../../../etc/passwd
// Prevents: /home/user/../../../system
// Prevents: symlink attacks
```

### Build Anchor Protocol
```rust
// Enforced in validate::validate_anchor()
if tool == "Write" || tool == "Edit" {
    if !session.files_read.contains(&file_path) {
        return Invalid("Must read file before editing");
    }
}

// Prevents: blind writes
// Prevents: AI guessing file contents
// Prevents: overwriting without reading
```

### Command Injection Prevention
```rust
// Pattern matching in validate::validate_command()
dangerous_patterns = [
    r"rm\s+-rf\s+/",        // System destruction
    r">\s*/dev/",            // Device manipulation
    r"curl.*\|\s*sh",        // Remote code execution
    r"chmod\s+777",          // Insecure permissions
];
```

### Approval Requirement
```rust
// ALL tiers require approval (hardcoded)
if !approved {
    return GateDecision {
        allowed: false,
        errors: vec!["Approval required".to_string()],
        ..
    };
}
```

## 13.3 Default Path Rules

### Allowed Paths
```
/data/data/com.termux/files/home/
/storage/emulated/0/Download/api-workspace/
```

### Blocked Paths
```
/tmp                                    # Termux restriction
/etc                                    # System config
/usr                                    # System binaries
/system                                 # Android partition
/data/data/com.termux/files/usr         # Termux packages
```

## 13.4 Dangerous Command Patterns

| Pattern | Severity | Description |
|---------|----------|-------------|
| `rm -rf /` | 10 | System destruction |
| `rm -rf ~` | 10 | Home destruction |
| `dd if=` | 9 | Raw disk access |
| `> /dev/` | 9 | Device manipulation |
| `mkfs` | 9 | Filesystem formatting |
| `chmod 777` | 7 | Insecure permissions |
| `curl \| sh` | 8 | Remote code execution |
| `wget \| sh` | 8 | Remote code execution |
| `eval` | 6 | Code injection |
| `:(){` | 9 | Fork bomb |
| `git push --force` | 7 | Destructive git |
| `git reset --hard` | 6 | History destruction |

---

# 14. CONFIGURATION REFERENCE

## 14.1 LMDB Configuration Keys

### Enforce Mode
```
Key: "enforce_mode"
Values: "Hard" | "Soft"
Default: "Hard"
```

### Tier Thresholds
```
Key: "tier_simple_max"      Value: 500
Key: "tier_simple_analyze"  Value: 0.40
Key: "tier_simple_build"    Value: 0.60

Key: "tier_light_max"       Value: 2000
Key: "tier_light_analyze"   Value: 0.60
Key: "tier_light_build"     Value: 0.40

Key: "tier_medium_max"      Value: 10000
Key: "tier_medium_analyze"  Value: 0.75
Key: "tier_medium_build"    Value: 0.25

Key: "tier_critical_analyze" Value: 0.95
Key: "tier_critical_build"   Value: 0.05
```

### Formula Parameters
```
Key: "formula_w_eff"       Value: 40000
Key: "formula_basic_exp"   Value: 1
Key: "formula_deps_exp"    Value: 7
Key: "formula_complex_exp" Value: 10
Key: "formula_files_mult"  Value: 10
```

## 14.2 Path Rules Format

```
Table: paths
Key format: "{type}:{path}"

Examples:
  "allowed:/data/data/com.termux/files/home/"
  "blocked:/tmp"
  "blocked:/etc"
```

## 14.3 Dangerous Patterns Format

```
Table: patterns
Key: pattern string
Value: DangerousPattern {
    pattern: String,
    severity: u8,
    description: String,
}
```

## 14.4 Claude.json MCP Configuration

```json
{
  "mcpServers": {
    "spf-smart-gate": {
      "type": "stdio",
      "command": "/data/data/com.termux/files/home/SPFsmartGATE/target/release/spf-smart-gate",
      "args": ["serve"],
      "env": {}
    }
  }
}
```

---

# 15. ERROR HANDLING

## 15.1 Error Types

### Gate Errors (Blocking)
```rust
// These prevent the operation from proceeding
"Build Anchor: Must read file before editing"
"Path blocked: {path}"
"Path not in allowed list: {path}"
"Dangerous command detected: {pattern}"
"Approval required for all operations"
```

### Gate Warnings (Non-blocking)
```rust
// These are logged but don't block
"Command contains risky pattern: {pattern}"
"Large file operation: {size} bytes"
"High complexity: C={c}"
```

### System Errors
```rust
// LMDB errors
"Failed to open LMDB: {error}"
"Transaction failed: {error}"

// File errors
"File not found: {path}"
"Permission denied: {path}"
"Read failed: {error}"
"Write failed: {error}"

// Network errors
"HTTP request failed: {error}"
"DNS resolution failed"
"Connection timeout"
```

## 15.2 Error Response Format

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32000,
    "message": "BLOCKED: Build Anchor: Must read file before editing"
  }
}
```

## 15.3 Error Codes

| Code | Meaning |
|------|---------|
| -32700 | Parse error (invalid JSON) |
| -32600 | Invalid request |
| -32601 | Method not found |
| -32602 | Invalid params |
| -32000 | Gate blocked |
| -32001 | System error |

---

# 16. INSERT POINTS GUIDE

## 16.1 Adding a New MCP Tool

### Step 1: Add Tool Definition (mcp.rs:154-621)
**Location:** `mcp.rs` function `tool_definitions()`

```rust
// Insert at appropriate section (after line ~620)
tool_def(
    "spf_your_new_tool",
    "Description of what the tool does",
    json!({
        "param1": {"type": "string", "description": "..."},
        "param2": {"type": "integer", "default": 10}
    }),
    vec!["param1"],  // required params
),
```

### Step 2: Add Handler (mcp.rs:625-1954)
**Location:** `mcp.rs` function `handle_tool_call()`

```rust
// Insert in match block (before the _ => catch-all)
"spf_your_new_tool" => {
    let param1 = args["param1"].as_str().unwrap_or("");
    let param2 = args["param2"].as_i64().unwrap_or(10);

    // Optional: Gate check
    let params = ToolParams { ... };
    let decision = gate::process("your_new_tool", &params, config, session, false);
    if !decision.allowed {
        return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
    }

    // Implementation
    session.record_action("your_new_tool", "called", None);
    let _ = storage.save_session(session);

    json!({"type": "text", "text": "Result"})
}
```

### Step 3: Add Complexity Calculation (calculate.rs:55-120)
**Location:** `calculate.rs` function `calculate()`

```rust
// Add match arm for your tool
"your_new_tool" => {
    basic = param1.len() as u64;
    deps = 1;
    complex = 1;
}
```

### Files Modified: 2
- `src/mcp.rs` (2 locations)
- `src/calculate.rs` (1 location)

## 16.2 Adding a New Path Rule

### Runtime (via CLI - Future)
```bash
spf-smart-gate config set-path allowed /new/path
spf-smart-gate config set-path blocked /dangerous/path
```

### Compile-time (config_db.rs:80-120)
**Location:** `config_db.rs` function `init_defaults()`

```rust
// Add to default allowed paths
self.set_path_rule("allowed", "/new/allowed/path")?;

// Add to default blocked paths
self.set_path_rule("blocked", "/new/blocked/path")?;
```

### Files Modified: 1
- `src/config_db.rs`

## 16.3 Adding a New Dangerous Pattern

**Location:** `config_db.rs` function `init_defaults()` (line ~100)

```rust
self.set_dangerous_pattern(
    r"your_regex_pattern",
    8,  // severity (1-10)
    "Description of why this is dangerous"
)?;
```

### Files Modified: 1
- `src/config_db.rs`

## 16.4 Adding a New LMDB Database

### Step 1: Create Module
**Location:** `src/new_db.rs` (new file)

```rust
use heed::{Database, Env, EnvOpenOptions};
use heed::types::{SerdeBincode, Str};
use anyhow::Result;

pub struct NewDb {
    env: Env,
    main: Database<Str, SerdeBincode<YourType>>,
}

impl NewDb {
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(10 * 1024 * 1024)  // 10MB
                .max_dbs(4)
                .open(path)?
        };
        let mut wtxn = env.write_txn()?;
        let main = env.create_database(&mut wtxn, Some("main"))?;
        wtxn.commit()?;
        Ok(Self { env, main })
    }
}
```

### Step 2: Add to lib.rs
```rust
pub mod new_db;
```

### Step 3: Initialize in mcp.rs
**Location:** `mcp.rs` function `run()` (around line 1975)

```rust
// Initialize NEW_DB LMDB
let new_db_path = spf_base.join("storage/new_db");
let new_db = match NewDb::open(&new_db_path) {
    Ok(db) => Some(db),
    Err(e) => {
        log(&format!("Warning: Failed to open NEW_DB: {}", e));
        None
    }
};
```

### Step 4: Pass to handle_tool_call()
Update function signature and all call sites.

### Files Modified: 4
- `src/new_db.rs` (new)
- `src/lib.rs`
- `src/mcp.rs` (2 locations)

## 16.5 Modifying the SPF Formula

### Change Exponents
**Location:** `config_db.rs` function `init_defaults()` (line ~130)

```rust
// Modify default values
self.set("formula_deps_exp", 8)?;  // Was 7
```

### Change Calculation Logic
**Location:** `calculate.rs` function `calculate()` (line ~80)

```rust
// Modify the formula application
let deps_contrib = (deps as f64).powf(config.formula.deps_exp as f64) as u64;
```

### Files Modified: 1-2
- `src/config_db.rs` (for defaults)
- `src/calculate.rs` (for logic)

---

# 17. CHANGE IMPACT ANALYSIS

## 17.1 Dependency Matrix

| Module Changed | Impacts | Rebuild Required |
|----------------|---------|------------------|
| config.rs | All modules | Full |
| calculate.rs | gate.rs, mcp.rs | Partial |
| validate.rs | gate.rs | Partial |
| gate.rs | mcp.rs | Partial |
| session.rs | mcp.rs, storage.rs | Partial |
| mcp.rs | None (leaf) | Minimal |
| config_db.rs | main.rs, mcp.rs | Partial |
| tools_db.rs | mcp.rs | Minimal |
| sandbox_db.rs | mcp.rs | Minimal |
| agent_state.rs | mcp.rs | Minimal |

## 17.2 High-Risk Changes

### Breaking Changes
1. **SpfConfig struct modification**
   - Impacts: All modules
   - Risk: LMDB deserialization failure
   - Mitigation: Version migration code

2. **Session struct modification**
   - Impacts: All saved sessions
   - Risk: Session load failure
   - Mitigation: Default values for new fields

3. **MCP tool schema change**
   - Impacts: All clients
   - Risk: Client compatibility
   - Mitigation: Version negotiation

### Safe Changes
1. Adding new MCP tool (additive)
2. Adding new LMDB table (additive)
3. Adding new validation rule (additive)
4. Changing log messages

## 17.3 Testing Requirements by Change Type

| Change Type | Tests Required |
|-------------|----------------|
| New tool | Unit test, integration test, MCP test |
| Formula change | Unit test, regression test |
| Path rule | Integration test |
| LMDB schema | Migration test, load test |
| Security rule | Security test, penetration test |

---

# 18. OPERATIONAL MANUAL

## 18.1 Installation

### Prerequisites
```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# LMDB library
pkg install lmdb
```

### Build
```bash
cd ~/SPFsmartGATE
cargo build --release
```

### Configure Claude Code
Edit `~/.claude.json`:
```json
{
  "mcpServers": {
    "spf-smart-gate": {
      "type": "stdio",
      "command": "/data/data/com.termux/files/home/SPFsmartGATE/target/release/spf-smart-gate",
      "args": ["serve"],
      "env": {}
    }
  }
}
```

### Verify
```bash
./target/release/spf-smart-gate status
./target/release/spf-smart-gate init-config
```

## 18.2 CLI Commands

### serve
Run MCP server (blocks forever, used by Claude Code)
```bash
./spf-smart-gate serve
```

### status
Show gateway status
```bash
./spf-smart-gate status
```
Output:
```
SPF Smart Gateway v2.0.0
Mode: Hard
Storage: ~/SPFsmartGATE/storage
Config: LMDB (spf_config/)

Session: 5 reads, 2 writes, C=1234

Tiers:
  SIMPLE   < 500    | 40% analyze / 60% build
  LIGHT    < 2000   | 60% analyze / 40% build
  MEDIUM   < 10000  | 75% analyze / 25% build
  CRITICAL > 10000  | 95% analyze / 5% build

Formula: a_optimal(C) = 40000 × (1 - 1/ln(C + e))
```

### session
Show full session state as JSON
```bash
./spf-smart-gate session
```

### reset
Clear session state
```bash
./spf-smart-gate reset
```

### init-config
Initialize/verify LMDB config
```bash
./spf-smart-gate init-config
```

### gate
One-shot gate check
```bash
./spf-smart-gate gate Write '{"file_path":"/tmp/test.txt","content":"hello"}'
```

### calculate
Calculate complexity without executing
```bash
./spf-smart-gate calculate Bash '{"command":"rm -rf /"}'
```

## 18.3 Monitoring

### Logs
```bash
# MCP server logs to stderr
./spf-smart-gate serve 2>&1 | tee spf.log
```

### LMDB Stats
```bash
# Via MCP tools
spf_config_stats
spf_tools_stats
spf_sandbox_stats
spf_agent_stats
```

### Session State
```bash
spf_session
```

## 18.4 Backup & Recovery

### Backup LMDB
```bash
cp -r ~/SPFsmartGATE/storage ~/SPFsmartGATE/storage.backup
```

### Restore
```bash
rm -rf ~/SPFsmartGATE/storage
cp -r ~/SPFsmartGATE/storage.backup ~/SPFsmartGATE/storage
```

### Reset to Defaults
```bash
rm -rf ~/SPFsmartGATE/storage
./spf-smart-gate init-config
```

---

# 19. TROUBLESHOOTING

## 19.1 Common Issues

### "SPF_CONFIG LMDB not initialized"
**Cause:** LMDB database not found
**Fix:**
```bash
./spf-smart-gate init-config
```

### "Build Anchor: Must read file before editing"
**Cause:** Attempting to write/edit without reading first
**Fix:** Use `spf_read` before `spf_write` or `spf_edit`

### "Path blocked: /tmp"
**Cause:** /tmp is blocked by default (Termux Android)
**Fix:** Use allowed paths (~/...) or add exception via CLI

### MCP tools not appearing in Claude Code
**Cause:** MCP server not configured correctly
**Fix:**
1. Check `~/.claude.json` has correct path
2. Verify binary exists and is executable
3. Restart Claude Code

### "Brain not found"
**Cause:** stoneshell-brain binary missing
**Fix:**
```bash
cd ~/stoneshell-brain
cargo build --release
```

## 19.2 Debug Mode

### Enable verbose logging
```bash
RUST_LOG=debug ./spf-smart-gate serve
```

### Log levels
```
error - Errors only
warn  - Warnings and errors
info  - Normal operation (default)
debug - Detailed debugging
trace - Everything
```

## 19.3 LMDB Corruption Recovery

### Symptoms
- "LMDB error: MDB_CORRUPTED"
- "Transaction failed"
- Crashes on startup

### Recovery
```bash
# 1. Backup current state
cp -r ~/SPFsmartGATE/storage ~/SPFsmartGATE/storage.corrupt

# 2. Try LMDB recovery tool
mdb_copy -c storage/spf_config storage/spf_config.recovered

# 3. If fails, reset
rm -rf ~/SPFsmartGATE/storage
./spf-smart-gate init-config
```

## 19.4 Performance Issues

### Slow gate processing
- Check LMDB map size
- Check disk space
- Profile with `RUST_LOG=trace`

### High memory usage
- LMDB map size is virtual, not resident
- Check for memory leaks in long sessions
- Reset session periodically

---

# 20. LMDB 5 FULL CONTAINMENT

## 20.1 Overview

LMDB 5 (agent_state) implements Full Containment - the agent operates entirely within a virtual filesystem controlled by SPF. Claude CLI, all configuration files, and the SPF binary itself live inside LMDB 5.

The agent perceives: /home/agent/
Reality: LMDB 5 + blob storage on disk
SPF controls the entire environment.

## 20.2 Virtual Filesystem Layout

/home/agent/                          (Agent home in LMDB 5)
  .claude.json                        Main Claude config
  .claude/
    settings.json                     Claude settings
    config.json                       Claude config
    settings.local.json               Local overrides
    .credentials.json                 Credentials
    history.jsonl                     Command history
    projects/                         Project data (blob)
    file-history/                     File history (blob)
    todos/                            Todo lists
    plans/                            Plans
    tasks/                            Tasks
  bin/
    spf-smart-gate                    SPF binary
    claude-code/                      Claude CLI (69MB)

## 20.3 Storage Strategy

Size less than 1MB: Direct in LMDB 5
Size greater than 1MB: Blob storage on disk, manifest lookup

Blob Storage Location: ~/SPFsmartGATE/storage/blobs/

storage/blobs/
  {sha256hash}              SPF binary (~8MB)
  claude-code/              Claude CLI directory (69MB)
  claude-projects/          projects/ directory (119MB)
  claude-file-history/      file-history/ (25MB)

## 20.4 Manifest System

Location: ~/SPFsmartGATE/storage/lmdb5_manifest.json

Format: JSON with entries array
Each entry has: virtual path, real path, size, type

## 20.5 Path Resolution

1. Agent requests ~/file.txt
2. SPF expands to /home/agent/file.txt
3. Manifest lookup: virtual to real path
4. If in LMDB: Read directly from agent_state/
5. If blob: Read from storage/blobs/{hash}

## 20.6 Boot Injection

Script: ~/SPFsmartGATE/scripts/boot-lmdb5.sh

Called by session-start.sh on every Claude session start.

Exports:
  SPF_AGENT_HOME=/home/agent
  SPF_CLAUDE_CONFIG=/home/agent/.claude.json
  SPF_ACTIVE=1

## 20.7 Installation Script

Location: ~/SPFsmartGATE/scripts/install-lmdb5.sh

Functions:
1. preflight() - Verify SPF binary, Claude CLI, configs exist
2. backup_existing() - Backup ~/.claude/ to storage/backup/
3. copy_binaries() - SPF + Claude CLI to blob storage
4. copy_configs() - Small configs to staging/configs/
5. copy_large_dirs() - Large dirs to blob storage
6. create_manifest() - Generate lmdb5_manifest.json
7. create_boot_injection() - Create boot-lmdb5.sh
8. create_symlinks() - agent-bin/ symlinks to blobs
9. update_claude_json() - Point mcpServers to new paths
10. verify_installation() - Confirm all components present

## 20.8 Directory Mapping

Virtual Path                      Real Path                    Type
/home/agent/.claude.json          staging/configs/claude.json  Config
/home/agent/.claude/settings.json staging/configs/settings.json Config
/home/agent/.claude/projects/     blobs/claude-projects/       Blob
/home/agent/bin/spf-smart-gate    blobs/{sha256}               Binary
/home/agent/bin/claude-code/      blobs/claude-code/           Binary

## 20.9 Symlinks for Backward Compatibility

Location: ~/SPFsmartGATE/agent-bin/

agent-bin/
  spf-smart-gate symlink to storage/blobs/{sha256}
  claude-code symlink to storage/blobs/claude-code/

These symlinks allow external tools to access binaries without knowing blob hashes.

## 20.10 MCP Configuration Update

After installation, ~/.claude.json mcpServers.spf-smart-gate.command
points to: ~/SPFsmartGATE/agent-bin/spf-smart-gate

## 20.11 Verification Commands

Check manifest: cat ~/SPFsmartGATE/storage/lmdb5_manifest.json
Check blobs: ls -la ~/SPFsmartGATE/storage/blobs/
Check symlinks: ls -la ~/SPFsmartGATE/agent-bin/
Test virtual FS: spf_fs_ls /home/agent/

## 20.12 Security Implications

1. Complete Isolation - Agent cannot access real ~/.claude/ directly
2. Controlled Persistence - All state changes go through SPF
3. Audit Trail - Every file operation logged in manifest
4. Rollback Capable - Backup exists in storage/backup/

---

# APPENDIX A: QUICK REFERENCE CARDS

## A.1 SPF Formula
```
C = basic + deps^7 + complex^10 + files×10
a_optimal = 40000 × (1 - 1/ln(C + e))
```

## A.2 Tier Thresholds
```
SIMPLE   C <   500   40%/60%
LIGHT    C <  2000   60%/40%
MEDIUM   C < 10000   75%/25%
CRITICAL C ≥ 10000   95%/5%
```

## A.3 Key Files
```
main.rs       Entry, CLI
mcp.rs        MCP server (2103 lines)
gate.rs       Enforcement
calculate.rs  SPF formula
validate.rs   Rule validation
config_db.rs  LMDB config
session.rs    State tracking
```

## A.4 LMDB Databases
```
spf_config/    Configuration
spf_tools/     Tool registry
spf_sandbox/   Project workspaces
agent_state/   Agent memory
spf_fs/        Virtual filesystem
```

---

**END OF DEVELOPER BIBLE**

*Document Version: 2.0*
*Last Updated: 2026-02-06*
*Maintainer: Joseph Stone*
*Total Sections: 20 + Appendix*
