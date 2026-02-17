# SPF Smart GATE v2.0.0 — Developer Bible
## Complete Deployment & Architecture Reference

> **Version**: 2.0.0
> **Author**: Joseph Stone (joepcstone@gmail.com)
> **Copyright**: 2026 Joseph Stone — All Rights Reserved
> **Platform**: Pure Rust, MCP JSON-RPC 2.0 over stdio
> **Binary**: 5.0 MB aarch64 (fat LTO, stripped)
> **Source**: ~7,796 lines across 15 Rust modules + 31 shell hooks
> **Generated**: 2026-02-17 from complete source code analysis

---

# TABLE OF CONTENTS

1. [Architecture & Module System](#block-1)
2. [Configuration & Session System](#block-2)
3. [Security Gate Pipeline](#block-3)
4. [Complexity Formula & Calculation](#block-4)
5. [Web Layer & SSRF Protection](#block-5)
6. [LMDB 1: Virtual Filesystem (SPF_FS)](#block-6)
7. [LMDB 2: Config Database (CONFIG.DB)](#block-7)
8. [LMDB 3: Projects Database (PROJECTS.DB)](#block-8)
9. [LMDB 4: TMP Database (TMP.DB)](#block-9)
10. [LMDB 5: Agent State (LMDB5.DB)](#block-10)
11. [MCP Server & All 55 Tool Handlers](#block-11)
12. [Hook System (Dual-Layer Interceptors)](#block-12)
13. [Deployment, Build & Configuration](#block-13)

---

<a name="block-1"></a>
# BLOCK 1 — ARCHITECTURE & MODULE SYSTEM

> **Sources**: `Cargo.toml` (91 lines), `src/lib.rs` (35 lines), `src/main.rs` (551 lines), `src/paths.rs` (87 lines)

---

## 1.1 PACKAGE DEFINITION (`Cargo.toml`)

```
Package:  spf-smart-gate v2.0.0
Edition:  Rust 2021
Author:   Joseph Stone <joepcstone@gmail.com>
License:  Custom (LICENSE file)
Repo:     https://github.com/STONE-CELL-SPF-JOSEPH-STONE/SPFsmartGATE
Binary:   src/main.rs → spf-smart-gate
Library:  src/lib.rs → spf_smart_gate
```

### 13 Production Dependencies
| Crate | Version | Features | Purpose |
|-------|---------|----------|---------|
| `heed` | 0.20 | — | LMDB bindings (all 6 databases) |
| `serde` | 1.0 | derive | Serialization framework |
| `serde_json` | 1.0 | — | JSON parsing/generation |
| `clap` | 4.5 | derive | CLI argument parsing (12 subcommands) |
| `thiserror` | 1.0 | — | Error type derivation |
| `anyhow` | 1.0 | — | Error propagation with context |
| `log` | 0.4 | — | Logging facade |
| `env_logger` | 0.11 | — | stderr logging backend |
| `chrono` | 0.4 | serde | Timestamps and date formatting |
| `reqwest` | 0.12 | blocking, rustls-tls, json | HTTP client for web tools |
| `html2text` | 0.6 | — | HTML → plain text conversion |
| `sha2` | 0.10 | — | SHA-256 checksums for blob storage |
| `hex` | 0.4 | — | Hex encoding for blob filenames |

**Dev dependency**: `tempfile 3` (tests only)

### Release Profile (Maximum Optimization)
```toml
[profile.release]
opt-level = 3         # Maximum optimization
lto = "fat"           # Full link-time optimization across all crates
codegen-units = 1     # Single codegen unit (slower compile, faster binary)
panic = "abort"       # No unwinding (smaller binary)
strip = true          # Strip debug symbols
```

**Result**: ~5.0 MB aarch64 binary from 13 crates + std

---

## 1.2 MODULE SYSTEM (`src/lib.rs` — 35 lines)

15 public module exports organized into two groups:

### Core Modules (10)
| Module | File | Lines | Purpose |
|--------|------|-------|---------|
| `paths` | paths.rs | 87 | Root discovery, platform detection |
| `calculate` | calculate.rs | 311 | Complexity formula C calculation |
| `config` | config.rs | 227 | SpfConfig struct, path validation |
| `gate` | gate.rs | 273 | 5-stage security pipeline |
| `inspect` | inspect.rs | 144 | Content inspection (credentials, injection) |
| `mcp` | mcp.rs | 3,516 | MCP JSON-RPC server, 55 tool handlers |
| `session` | session.rs | 192 | In-memory session state |
| `storage` | storage.rs | 100 | SESSION.DB LMDB wrapper |
| `validate` | validate.rs | 413 | Write allowlist, bash validation |
| `web` | web.rs | 385 | HTTP client, SSRF protection |

### LMDB Modules (5)
| Module | File | Lines | Database |
|--------|------|-------|----------|
| `fs` | fs.rs | 666 | SPF_FS — Virtual filesystem |
| `config_db` | config_db.rs | 517 | CONFIG.DB — Configuration storage |
| `projects_db` | projects_db.rs | 89 | PROJECTS.DB — Project registry |
| `tmp_db` | tmp_db.rs | 609 | TMP.DB — Trust levels, access logs |
| `agent_state` | agent_state.rs | 683 | LMDB5.DB — Agent memory, sessions, state |

**Total**: ~7,796 lines of Rust

---

## 1.3 CLI ENTRY POINT (`src/main.rs` — 551 lines)

### 12 Subcommands
| Command | Purpose |
|---------|---------|
| `serve` | Run MCP server (stdio JSON-RPC) — primary mode |
| `gate <tool> <params>` | One-shot gate check, returns allow/block |
| `calculate <tool> <params>` | Dry-run complexity calculation |
| `status` | Show gateway status, tiers, formula |
| `session` | Dump full session state as JSON |
| `reset` | Reset session (fresh start) |
| `init-config` | Confirm LMDB config state |
| `refresh-paths [--dry-run]` | Update path rules for current system |
| `fs-import <vpath> <file> [--dry-run]` | Import device file to virtual FS |
| `fs-export <vpath> <file>` | Export virtual FS file to device |
| `config-import <json> [--dry-run]` | Import JSON config to CONFIG.DB |
| `config-export <json>` | Export CONFIG.DB state to JSON |

### Boot Sequence (main.rs)
```
1. env_logger::init() — stderr logging
2. Cli::parse() — clap argument parsing
3. create_dir_all(storage) — ensure SESSION.DB directory
4. SpfConfigDb::open(LIVE/CONFIG/CONFIG.DB) — SINGLE SOURCE OF TRUTH
5. config_db.load_full_config() — assemble SpfConfig from LMDB
6. SpfStorage::open(LIVE/SESSION/SESSION.DB) — session persistence
7. storage.load_session() || Session::new() — restore or create
8. match command → dispatch to handler
```

### FS Import Routing
- `/home/agent/*` → LMDB5.DB (AgentStateDb) via `state:file:{relative}` keys
- Everything else → SPF_FS.DB (SpfFs) via `write(virtual_path, data)`
- Both verify write with read-back

### Config Import/Export
- **Import**: Reads JSON file → sets enforce_mode, tiers, formula, weights, allowed/blocked paths, dangerous patterns, scalar configs
- **Export**: Assembles all config state into pretty-printed JSON

---

## 1.4 PATH RESOLUTION (`src/paths.rs` — 87 lines)

### `spf_root()` — Cached via `OnceLock<PathBuf>`
Resolution order:
1. Walk up from binary location looking for `Cargo.toml`
2. `SPF_ROOT` environment variable
3. `$HOME/SPFsmartGATE`
4. **Panic** (unrecoverable)

### `actual_home()` — Cached via `OnceLock<PathBuf>`
Resolution order:
1. Parent directory of `spf_root()`
2. `$HOME` environment variable
3. **Panic**

### `system_pkg_path()` — Platform-detected
- Android/Termux: `$PREFIX` env or `/data/data/com.termux/files/usr`
- Linux/macOS: `/usr`

**Security note**: Write allowlist paths are computed from `spf_root()` but ENFORCED in `validate.rs`. The allowlist is compiled Rust, not configurable.

---

<a name="block-2"></a>
# BLOCK 2 — CONFIGURATION & SESSION SYSTEM

> **Sources**: `src/config.rs` (227 lines), `src/session.rs` (192 lines), `src/storage.rs` (100 lines)

---

## 2.1 MASTER CONFIGURATION (`src/config.rs`)

### `SpfConfig` — Master Struct (11 fields)
```rust
pub struct SpfConfig {
    pub version: String,                    // "2.0.0"
    pub enforce_mode: EnforceMode,          // Soft | Max
    pub allowed_paths: Vec<String>,         // Paths AI can access
    pub blocked_paths: Vec<String>,         // Paths AI cannot access
    pub require_read_before_edit: bool,     // Build Anchor Protocol
    pub max_write_size: usize,              // 100,000 bytes
    pub tiers: TierConfig,                  // 4 complexity tiers
    pub formula: FormulaConfig,             // C calculation parameters
    pub complexity_weights: ComplexityWeights, // 9 tool weight categories
    pub dangerous_commands: Vec<String>,    // 9 blocked command patterns
    pub git_force_patterns: Vec<String>,    // 3 git force flags
}
```

### `EnforceMode` — Two Modes
| Mode | Behavior |
|------|----------|
| `Soft` | Warnings only — violations are logged but not escalated |
| `Max` | Escalation — violations trigger `MAX TIER:` warnings → force CRITICAL tier |

### `TierConfig` — 4 Complexity Tiers
| Tier | C Range | Analyze% | Build% | Requires Approval |
|------|---------|----------|--------|-------------------|
| SIMPLE | < 500 | 40% | 60% | true |
| LIGHT | < 2,000 | 60% | 40% | true |
| MEDIUM | < 10,000 | 75% | 25% | true |
| CRITICAL | ≥ 10,000 | 95% | 5% | true |

### `FormulaConfig` — Complexity Calculation Parameters
```
w_eff = 40,000.0        (effective working memory in tokens)
e = 2.718281828...       (Euler's number)
basic_power = 1          (basic^1)
deps_power = 7           (deps^7: 2→128, 3→2187, 4→16384)
complex_power = 10       (complex^10: 1→1, 2→1024, 3→59049)
files_multiplier = 10    (files × 10)
```

### `ComplexityWeights` — 9 Tool Categories
| Category | basic | deps | complex | files |
|----------|-------|------|---------|-------|
| edit | 10 | 2 | 1 | 1 |
| write | 20 | 2 | 1 | 1 |
| bash_dangerous | 50 | 5 | 2 | 1 |
| bash_git | 30 | 3 | 1 | 1 |
| bash_piped | 20 | 3 | 1 | 1 |
| bash_simple | 10 | 1 | 0 | 1 |
| read | 5 | 1 | 0 | 1 |
| search | 8 | 2 | 0 | 1 |
| unknown | 20 | 3 | 1 | 1 |

### Default Blocked Paths (16)
```
/tmp, /etc, /usr, /system, {system_pkg_path},
{spf_root}/src/, {spf_root}/LIVE/SPF_FS/blobs/,
{spf_root}/Cargo.toml, {spf_root}/Cargo.lock,
{home}/.claude/,
{spf_root}/LIVE/CONFIG.DB, {spf_root}/LIVE/LMDB5/,
{spf_root}/LIVE/state/, {spf_root}/LIVE/storage/,
{spf_root}/hooks/, {spf_root}/scripts/
```

### Default Dangerous Commands (9 config + 7 hardcoded)
**Config patterns**: `rm -rf /`, `rm -rf ~`, `dd if=`, `> /dev/`, `chmod 777`, `curl | sh`, `wget | sh`, `curl|sh`, `wget|sh`

**Hardcoded (cannot be removed)**: `chmod 0777`, `chmod a+rwx`, `mkfs`, `> /dev/sd`, `curl|bash`, `wget -O-|`, `curl -s|`

### Path Validation Methods
- **`is_path_blocked(path)`**: Canonicalizes path (resolves symlinks), checks if it starts with any blocked prefix. Traversal (`..`) in unresolvable paths = always blocked.
- **`is_path_allowed(path)`**: Same canonicalization. Traversal in unresolvable paths = never allowed.

---

## 2.2 SESSION STATE (`src/session.rs` — 192 lines)

### `Session` Struct (12 fields)
```rust
pub struct Session {
    pub action_count: u64,
    pub files_read: Vec<String>,          // Canonical, deduplicated
    pub files_written: Vec<String>,       // Canonical, deduplicated
    pub last_tool: Option<String>,
    pub last_result: Option<String>,
    pub last_file: Option<String>,
    pub started: DateTime<Utc>,
    pub last_action: Option<DateTime<Utc>>,
    pub complexity_history: Vec<ComplexityEntry>,  // Max 100
    pub manifest: Vec<ManifestEntry>,              // Max 200
    pub failures: Vec<FailureEntry>,               // Max 50
    pub rate_window: Vec<DateTime<Utc>>,           // 60s sliding window
}
```

### Key Methods
| Method | Purpose |
|--------|---------|
| `track_read(path)` | Canonicalize path, deduplicate, add to `files_read`. Traversal → `[TRAVERSAL REJECTED]` flag |
| `track_write(path)` | Canonicalize path, deduplicate, add to `files_written`. Same traversal flag |
| `record_action(tool, result, file)` | Increment counter, update last_*, push timestamp to rate_window, prune expired (>60s) |
| `record_complexity(tool, c, tier)` | Push to complexity_history (max 100, FIFO) |
| `record_manifest(tool, c, action, reason)` | Push to manifest (max 200, FIFO) |
| `record_failure(tool, error)` | Push to failures (max 50, FIFO) |
| `anchor_ratio()` | `reads/writes` or `"N/A (no writes)"` |
| `status_summary()` | `"Actions: N | Reads: N | Writes: N | Last: tool | Anchor: r/w"` |

---

## 2.3 SESSION STORAGE (`src/storage.rs` — 100 lines)

### `SpfStorage` — LMDB Wrapper
- **Path**: `LIVE/SESSION/SESSION.DB`
- **Map size**: 50 MB
- **Max DBs**: 4
- **Database name**: `spf_state`
- **Session key**: `"current_session"` (JSON serialized)

### Operations
| Method | Implementation |
|--------|---------------|
| `open(path)` | create_dir_all → EnvOpenOptions → create_database("spf_state") |
| `save_session(session)` | serde_json::to_string → put("current_session", json) → commit |
| `load_session()` | get("current_session") → serde_json::from_str |
| `put(key, value)` | Generic string key-value store |
| `get(key)` | Generic string lookup |
| `delete(key)` | Generic key deletion |

---

<a name="block-3"></a>
# BLOCK 3 — SECURITY GATE PIPELINE

> **Sources**: `src/gate.rs` (273 lines), `src/validate.rs` (413 lines), `src/inspect.rs` (144 lines)

---

## 3.1 GATE PIPELINE (`src/gate.rs`)

### `GateDecision` — Final Output
```rust
pub struct GateDecision {
    pub allowed: bool,
    pub tool: String,
    pub complexity: ComplexityResult,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub message: String,    // "ALLOWED|BLOCKED | tool | C=N | details"
}
```

### 5-Stage Pipeline

```
Stage 1: RATE LIMITING
  └─ Count recent actions in 60s sliding window
  └─ Write/Edit/Bash/Download/Notebook: 60/min
  └─ Web fetch/search/API: 30/min
  └─ Everything else: 120/min
  └─ Exceeded → BLOCKED (RATE_LIMITED tier)

Stage 2: COMPLEXITY CALCULATION
  └─ calculate::calculate(tool, params, config)
  └─ Returns: C value, tier, analyze/build %, a_optimal tokens

Stage 3: RULE VALIDATION (tool-specific routing)
  └─ Edit/spf_edit → validate_edit (allowlist → anchor → blocked)
  └─ Write/spf_write → validate_write (allowlist → size → blocked → anchor)
  └─ Bash/spf_bash → validate_bash (dangerous → git force → /tmp → write-targets)
  └─ Read/spf_read → validate_read (blocked paths)
  └─ spf_web_download → validate_write (path checks)
  └─ spf_notebook_edit → validate_write (path checks)
  └─ 10 spf_fs_* tools → HARD BLOCKED (user/system-only)
  └─ 41 known safe tools → ValidationResult::ok()
  └─ Unknown tools → DEFAULT DENY

Stage 4: CONTENT INSPECTION (Write/Edit/Notebook only)
  └─ inspect::inspect_content(content, file_path, config)
  └─ 18 credential patterns, path traversal, shell injection, blocked path refs

Stage 5: MAX MODE ESCALATION
  └─ If enforce_mode == Max AND any "MAX TIER:" warnings:
     └─ Force CRITICAL tier (95%/5%, requires_approval)
     └─ Add "ESCALATED TO CRITICAL TIER" warning

Final: allowed = validation.valid && inspection.valid
```

---

## 3.2 VALIDATION RULES (`src/validate.rs`)

### Write Allowlist — COMPILED RUST (lines 60-72)
```rust
fn is_write_allowed(file_path: &str) -> bool {
    let resolved = resolve_path(file_path)?;
    let root = spf_root();
    let allowed = [
        "{root}/LIVE/PROJECTS/PROJECTS/",
        "{root}/LIVE/TMP/TMP/",
    ];
    allowed.iter().any(|a| resolved.starts_with(a))
}
```

**Only TWO directories** on the entire device are writable by AI:
1. `LIVE/PROJECTS/PROJECTS/`
2. `LIVE/TMP/TMP/`

### Path Resolution (`resolve_path`)
1. Try `canonicalize(path)` — resolves symlinks (existing files)
2. If file doesn't exist: canonicalize parent + append filename
3. Reject filenames containing `..`
4. If parent unresolvable + has `..`: return None (blocked)

### `validate_edit(file_path, config, session)`
1. **Write allowlist** — FIRST check, hardcoded, no bypass
2. **Build Anchor Protocol** — canonicalize path, check `files_read` contains it
   - Max mode: `"MAX TIER: BUILD ANCHOR — must read before editing"`
   - Soft mode: warning only
3. **Blocked paths** — `config.is_path_blocked(file_path)`

### `validate_write(file_path, content_len, config, session)`
1. **Write allowlist** — FIRST check
2. **File size** — warn if > `max_write_size` (100KB)
3. **Blocked paths** — reject if blocked
4. **Build Anchor** — for existing files, must read before overwrite

### `validate_bash(command, config)`
1. **Config dangerous patterns** — 9 patterns (both raw and normalized)
2. **Hardcoded dangerous** — 7 additional patterns (cannot be removed)
3. **Git force operations** — `--force`, `--hard`, `-f` detection
4. **`/tmp` access** — BLOCKED unconditionally
5. **Bash write-destination enforcement** — `check_bash_write_targets()`

### Bash Write-Destination Enforcement (lines 259-393)
Parses compound commands (split on `;`, `|`, `&&`, `||`) and checks each:

| Command Type | Detection | Check |
|-------------|-----------|-------|
| Redirect `>`, `>>` | Find operator, extract target path | `is_write_allowed(target)` |
| Here-doc `<<` + `>` | Find combined pattern | `is_write_allowed(target)` |
| `cp`, `mv` | Last non-flag arg = destination | `is_write_allowed(dest)` |
| `tee` | All non-flag args are targets | `is_write_allowed(each)` |
| `mkdir`, `touch`, `rm`, `rmdir` | All non-flag args | `is_write_allowed(each)` |
| `sed -i` | All non-flag args after `-i` | `is_write_allowed(each)` |
| `chmod`, `chown` | Non-flag args after first (mode/owner) | `is_write_allowed(each)` |
| `install` | Last non-flag arg = destination | `is_write_allowed(dest)` |
| `dd` | `of=` parameter | `is_write_allowed(dest)` |
| `python -c`, `perl -c`, etc. | `-c` flag detected | WARNING (can't parse) |

### `validate_read(file_path, config)` — Simplest validator
- Only check: `config.is_path_blocked(file_path)`
- Blocked → error. Otherwise → ok.

---

## 3.3 CONTENT INSPECTION (`src/inspect.rs` — 144 lines)

### File-Type Routing
**Code files** (`.sh`, `.bash`, `.zsh`, `.rs`, `.py`, `.js`, `.ts`, `.toml`, `.json`, `.md`):
- Credentials only + path traversal + blocked path references
- Shell injection patterns SKIPPED (normal in code)

**Non-code files**: Full inspection (all 4 checks)

### 18 Credential Patterns
| Pattern | Description |
|---------|-------------|
| `sk-` | API secret key |
| `AKIA` | AWS access key |
| `ghp_` | GitHub personal access token |
| `gho_` | GitHub OAuth token |
| `ghs_` | GitHub server token |
| `github_pat_` | GitHub PAT |
| `glpat-` | GitLab PAT |
| `xoxb-` | Slack bot token |
| `xoxp-` | Slack user token |
| `-----BEGIN RSA PRIVATE KEY` | RSA private key |
| `-----BEGIN OPENSSH PRIVATE KEY` | SSH private key |
| `-----BEGIN EC PRIVATE KEY` | EC private key |
| `-----BEGIN PRIVATE KEY` | Generic private key |
| `password=` | Hardcoded password |
| `passwd=` | Hardcoded password |
| `secret=` | Hardcoded secret |
| `api_key=` / `apikey=` | Hardcoded API key |
| `access_token=` | Hardcoded access token |

### 4 Shell Injection Patterns
| Pattern | Description |
|---------|-------------|
| `$(` | Command substitution |
| `eval ` | Eval statement |
| `exec ` | Exec statement |
| `` ` `` | Backtick substitution |

### Max Mode vs Soft Mode
- **Max**: Detected issues become `"MAX TIER: ..."` warnings → trigger CRITICAL tier escalation
- **Soft**: Standard warnings only

---

<a name="block-4"></a>
# BLOCK 4 — COMPLEXITY FORMULA & CALCULATION

> **Source**: `src/calculate.rs` (311 lines)

---

## 4.1 MASTER FORMULA

```
C = basic^1 + deps^7 + complex^10 + files×10
a_optimal(C) = W_eff × (1 - 1/ln(C + e))
```

Where:
- `W_eff = 40,000` (effective working memory tokens)
- `e = 2.71828...` (Euler's number)
- `C` = complexity value
- `a_optimal` = recommended analysis token allocation

### Exponent Impact
| Component | Value | Result |
|-----------|-------|--------|
| deps=2 | 2^7 | 128 |
| deps=3 | 3^7 | 2,187 |
| deps=4 | 4^7 | 16,384 |
| deps=5 | 5^7 | 78,125 |
| complex=1 | 1^10 | 1 |
| complex=2 | 2^10 | 1,024 |
| complex=3 | 3^10 | 59,049 |
| complex=4 | 4^10 | 1,048,576 |

---

## 4.2 DYNAMIC COMPLEXITY HELPERS

### `calc_complex_factor(content_len, has_risk, is_architectural)` → 0-4
- content > 200 bytes: +1
- content > 1000 bytes: +1
- content > 5000 bytes: +1
- has_risk: +1
- is_architectural: min 3
- Cap: 4

### `calc_files_factor(path, pattern, cmd)` → u64
- `find`/`xargs`/`-r`: 100 (→ 1000)
- `**` recursive glob: 50 (→ 500)
- `*` simple glob: 20 (→ 200)
- Root/src/lib directory: 20 (→ 200)
- Default single file: 1 (→ 10)

### `is_architectural_file(path)` → bool
Matches: config, main., lib., mod., cargo.toml, package.json, .env, settings, schema, *rc, .yaml, .yml

### `has_risk_indicators(content)` → bool
Matches: delete, drop, remove, truncate, override, force, unsafe, rm, sudo

---

## 4.3 PER-TOOL C CALCULATION

### Edit/Write
- basic = weight.basic + content_len/20 (edit) or /50 (write)
- deps = base + replace_all(+2) + large_diff(+1) + imports(+2)
- complex = `calc_complex_factor(len, risk, arch)`
- files = 1 (or 5 if replace_all)

### Bash (4 sub-categories)
| Category | Detection | Base Values |
|----------|-----------|-------------|
| Dangerous | config.dangerous_commands match | basic=50, deps=5+pipes+chains, complex≥3 |
| Git | git push/reset/rebase/merge | basic=30, deps=3+pipes, complex≥2 |
| Piped | `&&` or `\|` | basic=20, deps=3+pipes+chains, complex=1+pipes |
| Simple | everything else | basic=10, deps=1, complex=0 |

### Brain Tools (9)
| Tool | basic | deps | complex | files |
|------|-------|------|---------|-------|
| brain_search | 10 | limit | 0 | 1 |
| brain_store | 20+len/50 | 2 | 0-1 | 1 |
| brain_index | 50 | 5 | 1 | 10 |
| brain_* (others) | 10 | 1 | 0 | 1 |

### RAG Tools (16)
| Tool | basic | deps | complex | files |
|------|-------|------|---------|-------|
| rag_collect_web | 50 | 10 | 1 | 5 |
| rag_fetch_url | 30 | 5 | 1 | 1 |
| rag_collect_folder | 30 | 5 | 0 | 10 |
| rag_index_gathered | 40 | 5 | 1 | 10 |
| rag_smart_search / auto_fetch | 40 | 8 | 1 | 5 |
| rag_* (status/list) | 8 | 1 | 0 | 1 |

### Status/Calculate
basic=5, deps=0, complex=0, files=1

### Unknown Tools
Uses `complexity_weights.unknown`: basic=20, deps=3, complex=1, files=1

### Saturating Math
All arithmetic uses `saturating_pow`, `saturating_add`, `saturating_mul` — system NEVER panics on overflow.

---

<a name="block-5"></a>
# BLOCK 5 — WEB LAYER & SSRF PROTECTION

> **Source**: `src/web.rs` (385 lines)

---

## 5.1 HTTP CLIENT

```rust
pub struct WebClient {
    client: Client,  // reqwest blocking + rustls-tls
}
```
- **User-Agent**: `SPF-SmartGate/1.0 (AI-Browser)`
- **Timeout**: 30 seconds
- **TLS**: rustls (no OpenSSL dependency)

---

## 5.2 SSRF PROTECTION (`validate_url`)

Called before EVERY external HTTP request.

### Scheme Enforcement
- Only `http://` and `https://` allowed
- All other schemes (ftp, file, data, etc.) → BLOCKED

### Named Host Blocking
- `localhost` → BLOCKED
- `::1` → BLOCKED
- `0.0.0.0` → BLOCKED

### IPv4 Classification
| Range | Classification | Action |
|-------|---------------|--------|
| 127.0.0.0/8 | Loopback | BLOCKED |
| 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16 | Private | BLOCKED |
| 169.254.0.0/16 | Link-local / cloud metadata | BLOCKED |
| 100.100.100.200 | Alibaba cloud metadata | BLOCKED |

### IPv6 Classification
| Pattern | Classification | Action |
|---------|---------------|--------|
| `::1` | Loopback | BLOCKED |
| `::ffff:127.*` | IPv4-mapped loopback | BLOCKED |
| `::ffff:10.*`, etc. | IPv4-mapped private | BLOCKED |
| `::ffff:169.254.*` | IPv4-mapped metadata | BLOCKED |
| `fe80::/10` | Link-local | BLOCKED |
| `fc00::/7` | Unique-local (private) | BLOCKED |

---

## 5.3 SEARCH ENGINE

### Auto-selection: `search(query, count)`
1. Check `BRAVE_API_KEY` env var
2. If set and non-empty → Brave Search API
3. Otherwise → DuckDuckGo HTML fallback

### Brave Search
- Endpoint: `https://api.search.brave.com/res/v1/web/search`
- Auth: `X-Subscription-Token` header
- Returns: title, URL, description per result

### DuckDuckGo Fallback
- Endpoint: `https://html.duckduckgo.com/html/` (POST)
- Parses HTML: `result__a` links + `result__snippet` descriptions
- HTML entity decoding: `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#x27;`, `&#39;`, `&nbsp;`

---

## 5.4 PAGE READER (`read_page`)
1. `validate_url()` — SSRF check
2. HTTP GET with 30s timeout
3. Content-type routing:
   - **JSON**: `serde_json::to_string_pretty()`
   - **HTML**: `html2text::from_read(body, 120)` (120-char width)
   - **Other**: raw text

## 5.5 DOWNLOADER (`download`)
1. `validate_url()` — SSRF check
2. HTTP GET → bytes
3. Auto-create parent directories
4. Write to disk
5. Return (size, content_type)

## 5.6 API CLIENT (`api_request`)
- Methods: GET, POST, PUT, DELETE, PATCH, HEAD
- Custom headers via JSON object
- Body for POST/PUT/PATCH (auto Content-Type: application/json)
- Returns: (status_code, response_headers, response_body)

---

<a name="block-6"></a>
# BLOCK 6 — LMDB 1: VIRTUAL FILESYSTEM (SPF_FS)

> **Source**: `src/fs.rs` (666 lines)

---

## 6.1 ARCHITECTURE

```
Path: LIVE/SPF_FS/
Map Size: 4 GB
Databases: 3
  fs_metadata → SerdeBincode<FileMetadata>
  fs_content  → Bytes (raw file content)
  fs_index    → Str (vector_id → path reverse lookup)
Blob Dir: LIVE/SPF_FS/blobs/
```

### Hybrid Storage
| File Size | Storage | Location |
|-----------|---------|----------|
| ≤ 1 MB | Inline LMDB | `fs_content` database |
| > 1 MB | Disk blob | `blobs/{sha256_hex}` |

---

## 6.2 FILE METADATA

```rust
pub struct FileMetadata {
    pub file_type: FileType,      // File | Directory | Symlink
    pub size: u64,
    pub mode: u32,                // 0o644 (files) or 0o755 (dirs)
    pub created_at: i64,          // Unix timestamp
    pub modified_at: i64,
    pub checksum: Option<String>, // SHA-256 hex
    pub version: u64,             // Incremented on each write
    pub vector_id: Option<String>,// RAG reverse lookup
    pub real_path: Option<String>,// Disk blob path (>1MB files)
}
```

---

## 6.3 AUTO-CREATED DIRECTORIES (40)

On first open, `init_structure()` creates:
```
/, /system, /config, /tools, /tmp, /home, /home/agent,
/home/agent/.claude, /home/agent/.claude/projects,
/home/agent/.claude/file-history, /home/agent/.claude/paste-cache,
/home/agent/.claude/session-env, /home/agent/.claude/todos,
/home/agent/.claude/plans, /home/agent/.claude/tasks,
/home/agent/.claude/shell-snapshots, /home/agent/.claude/statsig,
/home/agent/.claude/telemetry, /home/agent/bin,
/home/agent/bin/claude-code, /home/agent/tmp,
/home/agent/.config, /home/agent/.config/settings,
/home/agent/.local, /home/agent/.local/bin,
/home/agent/.local/share, /home/agent/.local/share/history,
/home/agent/.local/share/data, /home/agent/.local/state,
/home/agent/.local/state/sessions, /home/agent/.cache,
/home/agent/.cache/context, /home/agent/.cache/tmp,
/home/agent/.memory, /home/agent/.memory/facts,
/home/agent/.memory/instructions, /home/agent/.memory/preferences,
/home/agent/.memory/pinned, /home/agent/.ssh,
/home/agent/Documents, /home/agent/Documents/notes,
/home/agent/Documents/templates, /home/agent/Projects,
/home/agent/workspace, /home/agent/workspace/current
```

---

## 6.4 OPERATIONS

| Operation | Method | Notes |
|-----------|--------|-------|
| `exists(path)` | metadata lookup | O(1) |
| `stat(path)` | metadata lookup | Returns FileMetadata |
| `read(path)` | Check real_path → disk blob; else content db | Hybrid routing |
| `write(path, data)` | mkdir_p parent → SHA-256 → hybrid store → update metadata | Auto-version bump |
| `mkdir(path)` | Single level, parent must exist | Error if exists |
| `mkdir_p(path)` | Recursive creation | Like `mkdir -p` |
| `ls(path)` | Prefix scan on metadata, depth filter | Direct children only |
| `rm(path)` | Check dir empty → remove blob → delete metadata+content | Can't remove `/` |
| `rm_rf(path)` | Collect all paths with prefix → delete all | Recursive removal |
| `rename(old, new)` | Copy metadata+content → delete old | Auto mkdir_p for dest parent |
| `index_vector(path, id)` | Update metadata.vector_id + index db | RAG integration |
| `vector_to_path(id)` | Reverse lookup in index db | For RAG retrieval |

### Path Normalization
- Resolves `.` and `..`
- Ensures leading `/`
- Removes trailing `/`
- Example: `/home/../home/user/./test` → `/home/user/test`

---

<a name="block-7"></a>
# BLOCK 7 — LMDB 2: CONFIG DATABASE (CONFIG.DB)

> **Source**: `src/config_db.rs` (517 lines)

---

## 7.1 ARCHITECTURE

```
Path: LIVE/CONFIG/CONFIG.DB
Map Size: 10 MB
Databases: 3
  config   → Str:Str    (namespace:key → JSON value)
  paths    → Str:bool   ("allowed:path" or "blocked:path" → true)
  patterns → Str:u8     (pattern → severity 1-10)
```

---

## 7.2 CORE OPERATIONS

### Config (namespace:key)
- `get(namespace, key)` / `set(namespace, key, value)` — string storage
- `get_typed<T>()` / `set_typed<T>()` — JSON deserialization/serialization

### Path Rules
- `allow_path(path)` → stores `"allowed:{path}" → true`
- `block_path(path)` → stores `"blocked:{path}" → true`
- `remove_path_rule(type, path)` → deletes rule
- `is_path_allowed(path)` → canonicalize + prefix match (traversal = false)
- `is_path_blocked(path)` → canonicalize + prefix match (traversal = true)
- `list_path_rules()` → returns all `(type, path)` pairs

### Dangerous Patterns
- `add_dangerous_pattern(pattern, severity)` → severity capped at 10
- `check_dangerous(command)` → returns max matching severity
- `list_dangerous_patterns()` → all `(pattern, severity)` pairs

---

## 7.3 BOOT INITIALIZATION

### `init_defaults()` — One-time (checks `spf:version` exists)
Sets: version, enforce_mode(Max), require_read_before_edit(true), max_write_size(100000), tiers, formula, weights, allowed paths (dynamically from `actual_home()`), blocked paths (16 entries from `spf_root()`), 9 dangerous patterns

### `sync_tier_approval()` — Every boot
**Source of truth is compiled code, not LMDB.** Reads current tiers from LMDB, compares `requires_approval` against hardcoded policy, updates if different. Also bumps version to `"2.0.0"`.

### `load_full_config()` — Primary config method
1. `init_defaults()` — ensure DB populated
2. `sync_tier_approval()` — compiled code wins
3. Collect all path rules → split allowed/blocked
4. Collect dangerous patterns → extract strings
5. Get scalar values (version, require_read, max_write)
6. Assemble complete `SpfConfig` struct

---

<a name="block-8"></a>
# BLOCK 8 — LMDB 3: PROJECTS DATABASE (PROJECTS.DB)

> **Source**: `src/projects_db.rs` (89 lines)

---

## 8.1 ARCHITECTURE

```
Path: LIVE/PROJECTS/PROJECTS.DB
Map Size: 20 MB
Databases: 1
  projects → Str:Str (key → value)
```

**Simplest LMDB module** — pure key-value store with no initialization data.

## 8.2 OPERATIONS

| Method | Purpose |
|--------|---------|
| `open(path)` | Create dir, open env, create database |
| `init_defaults()` | No-op (starts empty) |
| `get(key)` | Read transaction lookup |
| `set(key, value)` | Write transaction store |
| `delete(key)` | Write transaction delete, returns bool |
| `list_all()` | Full scan, returns Vec<(key, value)> |
| `db_stats()` | Entry count |

---

<a name="block-9"></a>
# BLOCK 9 — LMDB 4: TMP DATABASE (TMP.DB)

> **Source**: `src/tmp_db.rs` (609 lines)

---

## 9.1 ARCHITECTURE

```
Path: LIVE/TMP/TMP.DB
Map Size: 50 MB
Databases: 4
  projects    → SerdeBincode<Project>     (path → Project struct)
  access_log  → SerdeBincode<FileAccess>  (timestamp:path → access record)
  resources   → SerdeBincode<ResourceUsage> (project_path → usage)
  active      → Str                       ("active" → project_path)
```

---

## 9.2 TRUST LEVELS (5-tier)

```rust
pub enum TrustLevel {
    Untrusted,  // No access
    Low,        // Read only
    Medium,     // Read + limited write
    High,       // Read + write
    Full,       // All operations
}
```

---

## 9.3 PROJECT STRUCT (18 fields)

```rust
pub struct Project {
    pub name: String,
    pub path: String,
    pub trust_level: TrustLevel,
    pub active: bool,
    pub created_at: i64,
    pub last_accessed: i64,
    pub read_count: u64,
    pub write_count: u64,
    pub session_write_count: u64,
    pub max_session_writes: u64,          // Default: 1000
    pub max_write_size: u64,              // Default: 10MB
    pub total_bytes_written: u64,
    pub total_bytes_read: u64,
    pub total_complexity: u64,
    pub protected_paths: Vec<String>,
    pub allowed_extensions: Vec<String>,
    pub notes: String,
    pub metadata: serde_json::Value,
}
```

---

## 9.4 VALIDATE_OPERATION PIPELINE

```
1. Check project exists for path
2. Check project is active
3. Trust level check:
   - Untrusted → ALL BLOCKED
   - Low → writes BLOCKED
   - Medium/High/Full → allowed (with limits)
4. Protected paths check
5. Write limits:
   - session_write_count ≥ max_session_writes → BLOCKED
   - content_len > max_write_size → BLOCKED
```

---

## 9.5 ACCESS LOGGING

`log_access(virtual_path, project_path, op, source, size, success, error)` → FileAccess record:
```rust
pub struct FileAccess {
    pub timestamp: i64,
    pub virtual_path: String,
    pub project_path: String,
    pub operation: String,      // "read" | "write"
    pub source: String,         // "device" | "lmdb"
    pub size: u64,
    pub success: bool,
    pub error: Option<String>,
}
```

---

<a name="block-10"></a>
# BLOCK 10 — LMDB 5: AGENT STATE (LMDB5.DB)

> **Source**: `src/agent_state.rs` (683 lines)

---

## 10.1 ARCHITECTURE

```
Path: LIVE/LMDB5/LMDB5.DB
Map Size: 100 MB
Databases: 4
  memory   → SerdeBincode<MemoryEntry>    (id → memory)
  sessions → SerdeBincode<SessionContext>  (session_id → context)
  state    → Str:Str                      (key → value, incl. file:* keys)
  tags     → Str:Str                      (tag → comma-separated memory IDs)
```

---

## 10.2 MEMORY SYSTEM

### Memory Types (6, with expiry)
```rust
pub enum MemoryType {
    Fact,           // No expiry
    Instruction,    // No expiry
    Preference,     // No expiry
    Observation,    // 30 days
    Temporary,      // 7 days
    Pinned,         // No expiry
}
```

### MemoryEntry
```rust
pub struct MemoryEntry {
    pub id: String,              // UUID
    pub memory_type: MemoryType,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub accessed_at: i64,
    pub access_count: u64,
    pub relevance_score: f64,    // Decays over time
    pub expires_at: Option<i64>,
}
```

### Key Operations
- `create_memory(type, content, tags)` → UUID, auto-tags
- `recall(query, limit)` → keyword search + relevance scoring
- `search_memories(query, limit)` → content substring search
- `get_by_tag(tag)` → tag index lookup → memory retrieval
- `expire_memories()` → remove entries past `expires_at`

---

## 10.3 SESSION CONTEXT

```rust
pub struct SessionContext {
    pub session_id: String,
    pub parent_session: Option<String>,   // Chain linking
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub working_directory: String,
    pub project_name: Option<String>,
    pub files_modified: Vec<String>,
    pub total_complexity: u64,
    pub total_actions: u64,
    pub summary: Option<String>,
}
```

### Session Chain
- `start_session()` → links to previous via `parent_session`
- `end_session(id, summary)` → sets `ended_at` + summary
- `get_session_chain(id)` → follows parent links to build history

---

## 10.4 STATE DATABASE (key-value)

General-purpose string key-value store. Special prefix:
- `file:{path}` keys → imported config files (used by `route_agent` for virtual FS reads)
- Other keys → agent state data

### Context Summary Builder
`get_context_summary()` → assembles:
1. Latest session info
2. Recent memories (last 5)
3. Active state keys (non-file)
4. Session history summary

### Preferences
`AgentPreferences` struct stored as `state:preferences` key.

---

<a name="block-11"></a>
# BLOCK 11 — MCP SERVER & ALL 55 TOOL HANDLERS

> **Source**: `src/mcp.rs` (3,516 lines)
> **Full documentation**: See `LIVE/TMP/TMP/BLOCK11_MCP_TOOLS.md` (30,089 bytes)

---

## 11.1 SERVER IDENTITY

```
SERVER_NAME     = "spf-smart-gate"
SERVER_VERSION  = "2.0.0"
PROTOCOL_VERSION = "2024-11-05"
Communication:  JSON-RPC 2.0 over stdin/stdout
```

---

## 11.2 TOOL INVENTORY — 55 EXPOSED + 13 HIDDEN

### 55 Exposed Tools (via `tools/list`)
| Category | Count | Tools |
|----------|-------|-------|
| Core Gate | 3 | calculate, status, session |
| File Ops | 4 | read, write, edit, bash |
| Search | 2 | glob, grep |
| Web | 4 | search, fetch, download, api |
| Notebook | 1 | notebook_edit |
| Brain | 9 | search, store, context, index, list, status, recall, list_docs, get_doc |
| RAG | 16 | collect_web/file/folder/drop, index_gathered, dedupe, status, list_gathered, bandwidth_status, fetch_url, collect_rss, list_feeds, pending_searches, fulfill_search, smart_search, auto_fetch_gaps |
| Config | 2 | paths, stats |
| Projects | 5 | list, get, set, delete, stats |
| TMP | 4 | list, stats, get, active |
| Agent | 5 | stats, memory_search, memory_by_tag, session_info, context |

### 13 Hidden/Blocked Handlers
- `spf_gate` → BLOCKED (bypass vector)
- `spf_config_get`, `spf_config_set` → BLOCKED (user-only)
- `spf_agent_remember`, `spf_agent_forget`, `spf_agent_set_state` → BLOCKED (user-only)
- 8× `spf_fs_*` → BLOCKED (not in registry + gate.rs hard block)
- Unknown → DEFAULT DENY

---

## 11.3 LMDB VIRTUAL FS ROUTING

| Path Prefix | Handler | Backend |
|-------------|---------|---------|
| `/config/*` | `route_config()` | CONFIG.DB (read-only mount) |
| `/tmp/*` | `route_device_dir()` | `LIVE/TMP/TMP/` (device disk) |
| `/projects/*` | `route_device_dir()` | `LIVE/PROJECTS/PROJECTS/` |
| `/home/agent/tmp/*` | Redirect → `/tmp/*` | `LIVE/TMP/TMP/` |
| `/home/agent/*` | `route_agent()` | LMDB5.DB (read-only from AI) |
| Everything else | SpfFs | SPF_FS.DB |

---

## 11.4 EXTERNAL BINARY INTEGRATION

| Binary | Language | Path | Used By |
|--------|----------|------|---------|
| Brain | Rust | `$HOME/stoneshell-brain/target/release/brain` | 9 brain tools + 5 RAG tools |
| RAG Collector | Python | `LIVE/BIN/rag-collector/server.py` | 11 RAG tools |
| find | System | — | spf_glob |
| rg (ripgrep) | System | — | spf_grep |
| timeout | System | — | spf_bash |

---

## 11.5 AUDIT TRAIL

Every tool call generates:
1. `cmd.log` (persistent): `[timestamp] CALL/FAIL {name} | {summary}`
2. `session.manifest` (LMDB): (tool, C, status, notes) — max 200
3. `session.files_read/written`: Deduplicated canonical paths
4. `session.failures`: Error records — max 50
5. TMP_DB `access_log`: For `/tmp/` and `/projects/` operations

---

<a name="block-12"></a>
# BLOCK 12 — HOOK SYSTEM (Dual-Layer Interceptors)

> **Source**: `hooks/` directory (31 shell scripts) + `scripts/` (2 support scripts)
> **Full documentation**: See `LIVE/TMP/TMP/BLOCK12_HOOKS.md` (21,699 bytes)

---

## 12.1 DUAL-LAYER ARCHITECTURE

```
Layer 1 (Native Tools): 9 pre-hooks → EXIT 1 → BLOCKED
  Forces AI to use MCP equivalents (spf_read, spf_write, etc.)

Layer 2 (MCP Tools): 16 pre-hooks → EXIT 0 → ALLOWED + logged
  Audit trail before Rust gate pipeline processes the call

Post-Event: post-action.sh (state checkpoint + brain sync)
            post-failure.sh (failure logging)

Lifecycle: session-start.sh → user-prompt.sh → ... → session-end.sh
```

---

## 12.2 HOOK INVENTORY (31 scripts)

| Category | Count | Scripts |
|----------|-------|---------|
| Session Lifecycle | 4 | session-start, session-end, user-prompt, stop-check |
| Post-Event | 2 | post-action, post-failure |
| Layer 1 (Native Block) | 9 | pre-{read,edit,write,bash,glob,grep,notebookedit,webfetch,websearch} |
| Layer 2 (MCP Track) | 9 | pre-mcp-{read,edit,write,bash,glob,grep,notebookedit,webfetch,websearch} |
| Layer 2 (LMDB Domain) | 7 | pre-{brain,rag,agent,config,projects,tmp,spf-meta} |

---

## 12.3 KEY HOOKS

### `user-prompt.sh` (6,706 bytes) — Prompt Complexity Calculator
- Calculates C from prompt text using regex signal matching
- Categories: Math (100-500), Logic (200-500), Science/Domain (150-400)
- Multi-step multipliers: 1.3-1.8×
- Injects enforcement context for C ≥ 500

### `post-action.sh` (6,373 bytes) — State Checkpoint
- Log rotation at 512 MB (keep last 5,000 lines)
- Updates `state/session.json` with action count
- Generates `state/STATUS.txt` (Memory Triad System 2)
- Brain checkpoint on Write/Edit (async, backgrounded)

### `session-start.sh` (2,614 bytes) — Session Init
- Resets session state
- Injects SPF algorithm as `additionalContext`
- Calls `scripts/boot-lmdb5.sh`

---

<a name="block-13"></a>
# BLOCK 13 — DEPLOYMENT, BUILD & CONFIGURATION

> **Sources**: `Cargo.toml`, `build.sh`, `spf-deploy.sh`, `config.json`, `.claude.json`
> **Full documentation**: See `LIVE/TMP/TMP/BLOCK13_DEPLOYMENT.md` (19,140 bytes)

---

## 13.1 BUILD

```bash
./build.sh --release    # Auto-detect platform, build optimized
```

### Platform Support
| Platform | Target Triple | Status |
|----------|--------------|--------|
| Android/Termux aarch64 | `aarch64-linux-android` | **Primary** |
| Linux x86_64 | `x86_64-unknown-linux-gnu` | Supported |
| Linux aarch64 | `aarch64-unknown-linux-gnu` | Supported |
| macOS ARM | `aarch64-apple-darwin` | Supported |
| macOS Intel | `x86_64-apple-darwin` | Supported |

---

## 13.2 DEPLOYMENT

```bash
./spf-deploy.sh         # Generate settings + register MCP server
```

### What It Configures
1. `~/.claude/settings.json` — Hook registration + permissions (smart merge)
2. `~/.claude.json` — MCP server registration (`spf-smart-gate serve`)

---

## 13.3 LIVE DIRECTORY

```
LIVE/
├── BIN/spf-smart-gate/spf-smart-gate   (5.0 MB binary)
├── CONFIG/CONFIG.DB/                     (LMDB 2 — 10 MB)
├── SESSION/SESSION.DB/                   (Session — 50 MB)
├── SPF_FS/SPF_FS.DB/                    (LMDB 1 — 4 GB)
├── PROJECTS/PROJECTS.DB/ + PROJECTS/    (LMDB 3 — 20 MB + device mount)
├── TMP/TMP.DB/ + TMP/                   (LMDB 4 — 50 MB + device mount)
└── LMDB5/LMDB5.DB/                      (LMDB 5 — 100 MB)
```

**Total LMDB allocation**: ~4.23 GB (dominated by 4GB SPF_FS)

---

## 13.4 BOOT SEQUENCE (Full Chain)

```
1. Claude Code reads ~/.claude.json → finds mcpServers.spf-smart-gate
2. Spawns: spf-smart-gate serve
3. Binary boot: env_logger → CLI → CONFIG.DB → load config → SESSION.DB → mcp::run()
4. MCP boot: log → PROJECTS.DB → TMP.DB → LMDB5.DB → SPF_FS → stdin loop
5. Hook fires: session-start.sh → reset state → inject SPF context → boot LMDB5
6. Ready: 9 native tools BLOCKED, 55 MCP tools ACTIVE, 6 LMDBs OPEN
```

---

## 13.5 COMPLETE SOURCE TREE

```
SPFsmartGATE/                       # Repository root
├── Cargo.toml                      # Package (13 deps)
├── Cargo.lock                      # Lock (59,637 bytes)
├── LICENSE / README.md / HANDOFF.md
├── config.json                     # Active Claude config (16,518 bytes)
├── .claude.json                    # Global config reference
├── build.sh                        # Cross-platform build
├── spf-deploy.sh                   # Settings generator
│
├── src/ (15 modules, ~7,796 lines)
│   ├── lib.rs (35) → main.rs (551) → paths.rs (87)
│   ├── config.rs (227) → session.rs (192) → storage.rs (100)
│   ├── gate.rs (273) → validate.rs (413) → inspect.rs (144)
│   ├── calculate.rs (311) → web.rs (385)
│   ├── fs.rs (666) → config_db.rs (517) → projects_db.rs (89)
│   ├── tmp_db.rs (609) → agent_state.rs (683)
│   └── mcp.rs (3,516)
│
├── hooks/ (31 scripts)
│   ├── session-start.sh, session-end.sh, user-prompt.sh, stop-check.sh
│   ├── post-action.sh, post-failure.sh
│   ├── pre-{read,edit,write,bash,glob,grep,notebookedit,webfetch,websearch}.sh (9 blockers)
│   ├── pre-mcp-{...}.sh (9 trackers)
│   └── pre-{brain,rag,agent,config,projects,tmp,spf-meta}.sh (7 domain trackers)
│
├── scripts/
│   ├── boot-lmdb5.sh              # LMDB5 manifest loader
│   └── install-lmdb5.sh           # CLI containment installer
│
├── LIVE/ (6 LMDBs + binaries)
└── state/ (hook-generated state files)
```

---

# SECURITY ARCHITECTURE SUMMARY

| Layer | Mechanism | Enforcement |
|-------|-----------|-------------|
| **1. Native Hook Intercept** | 9 pre-hooks exit 1 | ALL native tools BLOCKED |
| **2. MCP Hook Audit** | 16 pre-hooks exit 0 | All MCP tools LOGGED |
| **3. Rate Limiting** | 60s sliding window | 30-120 ops/min by category |
| **4. Complexity Calculation** | C formula with ^7/^10 exponents | Tier classification |
| **5. Rule Validation** | Write allowlist (compiled Rust) | Only PROJECTS/TMP writable |
| **6. Build Anchor Protocol** | Must read before edit/write | Prevents blind modifications |
| **7. Bash Write Enforcement** | 13 command patterns parsed | Redirect/cp/mv/tee/sed checked |
| **8. Content Inspection** | 18 credential + 4 injection patterns | Max mode → CRITICAL escalation |
| **9. Path Canonicalization** | Symlink resolution + traversal detection | No `..` bypass |
| **10. SSRF Protection** | IPv4/IPv6 loopback/private/metadata | No internal network access |
| **11. Default Deny** | Unknown tools rejected | Only 55 registered tools allowed |
| **12. User-Only Operations** | 6 handlers HARD BLOCKED | Config/memory writes = human only |

---

*End of SPF Smart GATE v2.0.0 Developer Bible*
*Generated from complete source code analysis — every line of every file read and documented*
*Copyright 2026 Joseph Stone — All Rights Reserved*
