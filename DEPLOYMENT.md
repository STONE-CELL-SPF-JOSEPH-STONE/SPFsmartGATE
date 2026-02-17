# BLOCK 13 — DEPLOYMENT, BUILD, & CONFIGURATION

> **Sources**: `Cargo.toml`, `build.sh`, `spf-deploy.sh`, `config.json`, `.claude.json`, `LIVE/` directory
> **Role**: Complete build pipeline, deployment automation, Claude Code integration, and LIVE directory layout
> **Copyright**: 2026 Joseph Stone — All Rights Reserved

---

## 13.1 BUILD SYSTEM

### 13.1.1 Cargo.toml — Package Definition

```
Package:  spf-smart-gate v2.0.0
Edition:  Rust 2021
Author:   Joseph Stone <joepcstone@gmail.com>
License:  Custom (LICENSE file)
Binary:   src/main.rs → spf-smart-gate
Library:  src/lib.rs → spf_smart_gate
```

**13 Dependencies**:
| Crate | Version | Purpose |
|-------|---------|---------|
| `heed` | 0.20 | LMDB bindings (all 6 databases) |
| `serde` | 1.0 (+derive) | Serialization framework |
| `serde_json` | 1.0 | JSON parsing/generation |
| `clap` | 4.5 (+derive) | CLI argument parsing |
| `thiserror` | 1.0 | Error type derivation |
| `anyhow` | 1.0 | Error propagation |
| `log` | 0.4 | Logging facade |
| `env_logger` | 0.11 | stderr logging backend |
| `chrono` | 0.4 (+serde) | Timestamps and date formatting |
| `reqwest` | 0.12 (blocking, rustls-tls, json) | HTTP client for web tools |
| `html2text` | 0.6 | HTML → plain text conversion |
| `sha2` | 0.10 | SHA-256 checksums for blob storage |
| `hex` | 0.4 | Hex encoding for blob filenames |

**Dev dependency**: `tempfile 3` (tests only)

**Release profile** (maximum optimization):
```toml
[profile.release]
opt-level = 3         # Maximum optimization
lto = "fat"           # Full link-time optimization across all crates
codegen-units = 1     # Single codegen unit (slower compile, faster binary)
panic = "abort"       # No unwinding (smaller binary)
strip = true          # Strip debug symbols
```

**Result**: ~5.0 MB aarch64 binary (from 13 crates + std)

### 13.1.2 `build.sh` — Cross-Platform Compilation Script

**Size**: 4,841 bytes
**Version**: v2.1.0

**Usage**:
```bash
./build.sh                    # Auto-detect current platform
./build.sh --target linux     # x86_64-unknown-linux-gnu
./build.sh --target mac       # aarch64-apple-darwin
./build.sh --target macx86    # x86_64-apple-darwin
./build.sh --target android   # aarch64-linux-android
./build.sh --debug            # Debug build (opt-level 1)
./build.sh --release          # Release build (default)
```

**Platform detection** (`detect_target()`):
| OS | Architecture | Target Triple |
|----|-------------|---------------|
| Linux + Termux | aarch64 | `aarch64-linux-android` |
| Linux (other) | aarch64 | `aarch64-unknown-linux-gnu` |
| Linux | x86_64 | `x86_64-unknown-linux-gnu` |
| macOS | arm64 | `aarch64-apple-darwin` |
| macOS | x86_64 | `x86_64-apple-darwin` |

**Build flow**:
1. Parse args (--debug/--release/--target)
2. Detect target triple (or use override)
3. Check if native build or cross-compile
4. For cross: add rustup target if needed
5. `cargo build [--target $TARGET] [--release]`
6. Locate binary in `target/[release|debug]/` or `target/$TARGET/[release|debug]/`
7. Copy binary to `LIVE/BIN/spf-smart-gate`
8. Report binary size

### 13.1.3 `spf-deploy.sh` — Settings Generator & Deployer

**Size**: 11,024 bytes
**Version**: v2.1.0
**Location**: Also deployed to `LIVE/BIN/spf-deploy.sh`

**Usage**:
```bash
./spf-deploy.sh              # Auto-detect and deploy
./spf-deploy.sh --dry-run    # Preview generated config
./spf-deploy.sh --verify     # Check existing paths
```

**What it generates/updates**:

#### 1. `~/.claude/settings.json` (Smart Merge or Fresh)

**MERGE mode** (existing file found):
- Fixes all hook `command` paths to current `$SPF_ROOT/hooks/`
- Ensures `permissions.deny` contains all 8 required entries
- Adds any missing SPF PreToolUse hooks (new tools since last deploy)
- Ensures all 7 lifecycle hooks exist
- **Preserves custom hooks** not part of SPF

**FRESH mode** (no existing file):
- Generates complete settings with:
  - `permissions.deny`: Read, Write, Edit, Bash, Glob, Grep, WebFetch, WebSearch, NotebookEdit
  - All 9 native PreToolUse blocking hooks
  - All MCP PreToolUse tracking hooks (one per tool, grouped by domain)
  - All 7 lifecycle hooks (SessionStart, PreToolUse, PostToolUse, PostToolUseFailure, UserPromptSubmit, Stop, SessionEnd)

**MCP tool-to-hook mapping** (complete reference from `spf-deploy.sh`):
| Hook Script | MCP Tools Covered |
|-------------|------------------|
| `pre-mcp-read.sh` | spf_read, spf_fs_exists, spf_fs_stat, spf_fs_ls, spf_fs_read |
| `pre-mcp-write.sh` | spf_write, spf_fs_write, spf_fs_mkdir, spf_fs_rm, spf_fs_rename |
| `pre-mcp-edit.sh` | spf_edit |
| `pre-mcp-bash.sh` | spf_bash |
| `pre-mcp-glob.sh` | spf_glob |
| `pre-mcp-grep.sh` | spf_grep |
| `pre-mcp-websearch.sh` | spf_web_search |
| `pre-mcp-webfetch.sh` | spf_web_fetch, spf_web_download, spf_web_api |
| `pre-mcp-notebookedit.sh` | spf_notebook_edit |
| `pre-brain.sh` | All 9 spf_brain_* tools |
| `pre-rag.sh` | All 16 spf_rag_* tools |
| `pre-config.sh` | spf_config_paths, spf_config_stats |
| `pre-projects.sh` | All 5 spf_projects_* tools |
| `pre-tmp.sh` | All 4 spf_tmp_* tools |
| `pre-agent.sh` | All 5 spf_agent_* tools |
| `pre-spf-meta.sh` | spf_calculate, spf_status, spf_session |

MCP tool matchers use the prefix: `mcp__spf-smart-gate__`

#### 2. `~/.claude.json` MCP Server Configuration

Updates the `mcpServers` section:
```json
{
  "spf-smart-gate": {
    "type": "stdio",
    "command": "/path/to/LIVE/BIN/spf-smart-gate/spf-smart-gate",
    "args": ["serve"],
    "env": {}
  }
}
```

---

## 13.2 CLAUDE CODE CONFIGURATION FILES

### 13.2.1 `config.json` (Project-level — 16,518 bytes)

This is the **active runtime configuration** at `SPFsmartGATE/config.json`. Key sections:

**MCP Server Registration**:
```json
"mcpServers": {
  "spf-smart-gate": {
    "type": "stdio",
    "command": ".../LIVE/BIN/spf-smart-gate/spf-smart-gate",
    "args": ["serve"],
    "env": {}
  }
}
```

**Hook Configuration** — 7 event types registered:
| Event | Matcher | Script | Effect |
|-------|---------|--------|--------|
| `SessionStart` | `startup\|resume` | `session-start.sh` | SPF context injection |
| `PreToolUse` | `Read` | `pre-read.sh` | BLOCK native Read |
| `PreToolUse` | `Edit` | `pre-edit.sh` | BLOCK native Edit |
| `PreToolUse` | `Write` | `pre-write.sh` | BLOCK native Write |
| `PreToolUse` | `Bash` | `pre-bash.sh` | BLOCK native Bash |
| `PreToolUse` | `Glob` | `pre-glob.sh` | BLOCK native Glob |
| `PreToolUse` | `Grep` | `pre-grep.sh` | BLOCK native Grep |
| `PreToolUse` | `WebFetch` | `pre-webfetch.sh` | BLOCK native WebFetch |
| `PreToolUse` | `NotebookEdit` | `pre-notebookedit.sh` | BLOCK native NotebookEdit |
| `PreToolUse` | `WebSearch` | `pre-websearch.sh` | BLOCK native WebSearch |
| `PostToolUse` | `.*` (all) | `post-action.sh` | State checkpoint |
| `PostToolUseFailure` | `.*` (all) | `post-failure.sh` | Failure logging |
| `UserPromptSubmit` | *(all)* | `user-prompt.sh` | Complexity calc |
| `Stop` | *(all)* | `stop-check.sh` | State save |
| `SessionEnd` | *(all)* | `session-end.sh` | Handoff note |

**Permission Configuration**:
```json
"permissions": {
  "allow": ["Read", "Write", "Edit", "Bash", "Glob", "Grep",
            "WebFetch", "WebSearch", "NotebookEdit", "Task"],
  "deny": [],
  "blockedPaths": ["/tmp", "/etc", "/usr", "/system",
                   "/data/data/com.termux/files/usr"],
  "allowedPaths": ["/storage/emulated/0/Download/api-workspace/",
                   "/data/data/com.termux/files/home/"],
  "defaultMode": "default"
}
```

**Project-Level Allowed MCP Tools** (21 tools pre-approved for `$HOME` project):
```
spf_read, spf_write, spf_edit, spf_bash, spf_glob, spf_grep,
spf_status, spf_session, spf_web_search, spf_web_fetch,
spf_web_download, spf_web_api, spf_brain_search, spf_brain_store,
spf_brain_context, spf_brain_index, spf_brain_list, spf_brain_status,
spf_brain_recall, spf_brain_list_docs, spf_brain_get_doc
```

**Telemetry**: Disabled (`"enabled": false`)

### 13.2.2 `.claude.json` (Global — Backup/Reference)

Located at `SPFsmartGATE/.claude.json` — this is a **project-local copy** of the global Claude config.

Key differences from `config.json`:
- **Permissions deny list** is more aggressive — blocks ALL native tools with wildcards:
  ```
  Read(*), Write(*), Edit(*), Bash(*), Glob(*), Grep(*), WebFetch(*),
  WebSearch(*), NotebookEdit(*), Task(*), TaskOutput(*), TaskStop(*),
  AskUserQuestion(*), EnterPlanMode(*), ExitPlanMode(*), TaskCreate(*),
  TaskUpdate(*), TaskList(*), TaskGet(*), Skill(*)
  ```
- **Specific Read exceptions** allowed:
  - `Read(/data/data/com.termux/files/home/SPFsmartGATE/*)`
  - `Read(/data/data/com.termux/files/home/.claude/settings.json)`
  - `Read(/data/data/com.termux/files/home/SPFsmartGATE/hooks/*)`
  - `Read(/data/data/com.termux/files/home/.claude.json)`
- MCP command points to `target/release/spf-smart-gate` (dev path)

---

## 13.3 LIVE DIRECTORY — DEPLOYMENT LAYOUT

```
LIVE/
├── BIN/
│   ├── spf-smart-gate/
│   │   └── spf-smart-gate          # 5,180,560 bytes (5.0 MB) — compiled binary
│   └── spf-deploy.sh               # Deployment script copy
│
├── CONFIG/
│   └── CONFIG.DB/                   # LMDB 2 — SpfConfigDb
│       ├── data.mdb                 # 53,248 bytes — config, paths, patterns
│       └── lock.mdb                 # 8,192 bytes
│
├── SESSION/
│   └── SESSION.DB/                  # LMDB — SpfStorage (session state)
│       ├── data.mdb
│       └── lock.mdb
│   └── cmd.log                      # Persistent audit log (append-only)
│
├── SPF_FS/
│   └── SPF_FS.DB/                   # LMDB 1 — Virtual Filesystem
│       ├── data.mdb                 # fs_metadata, fs_content, fs_index
│       └── lock.mdb
│
├── PROJECTS/
│   └── PROJECTS.DB/                 # LMDB 3 — SpfProjectsDb
│       ├── data.mdb
│       └── lock.mdb
│   └── PROJECTS/                    # Device-backed /projects/ mount
│
├── TMP/
│   └── TMP.DB/                      # LMDB 4 — SpfTmpDb
│       ├── data.mdb
│       └── lock.mdb
│   └── TMP/                         # Device-backed /tmp/ mount (AI writable zone)
│       ├── BLOCK1_ARCHITECTURE.md
│       ├── BLOCK2_CONFIG_SESSION.md
│       ├── ... (all block documentation)
│       └── BLOCK13_DEPLOYMENT.md
│
└── LMDB5/
    └── LMDB5.DB/                    # LMDB 5 — AgentStateDb
        ├── data.mdb                 # memory, sessions, state, tags
        └── lock.mdb
    ├── .claude.json                 # Agent's Claude config view
    ├── .claude/                     # Agent's Claude directory
    ├── CLAUDE.md                    # Agent instructions
    ├── .mcp.json                    # MCP config
    └── stoneshell-brain/            # Brain binary reference
```

### LMDB Size Allocations (compiled in Rust):
| Database | Path | Map Size | Purpose |
|----------|------|----------|---------|
| SESSION.DB | `LIVE/SESSION/SESSION.DB` | 50 MB | Session state |
| CONFIG.DB | `LIVE/CONFIG/CONFIG.DB` | 10 MB | Configuration, paths, patterns |
| SPF_FS.DB | `LIVE/SPF_FS/` | 4 GB | Virtual filesystem + blobs |
| PROJECTS.DB | `LIVE/PROJECTS/PROJECTS.DB` | 20 MB | Project key-value store |
| TMP.DB | `LIVE/TMP/TMP.DB` | 50 MB | Trust levels, access logs, resources |
| LMDB5.DB | `LIVE/LMDB5/LMDB5.DB` | 100 MB | Agent memory, sessions, state, tags |

**Total allocated**: ~4.23 GB (dominated by SPF_FS 4GB virtual filesystem)

---

## 13.4 DEPLOYMENT PROCEDURE

### Fresh Installation
```bash
# 1. Clone repository
git clone https://github.com/STONE-CELL-SPF-JOSEPH-STONE/SPFsmartGATE.git
cd SPFsmartGATE

# 2. Build binary
./build.sh --release
# Output: LIVE/BIN/spf-smart-gate/spf-smart-gate (5.0 MB)

# 3. Deploy configuration
./spf-deploy.sh
# Generates: ~/.claude/settings.json (hooks + permissions)
# Updates:   ~/.claude.json (MCP server registration)

# 4. (Optional) Install LMDB5 CLI containment
bash scripts/install-lmdb5.sh
# Copies Claude CLI + configs into LMDB5 virtual filesystem

# 5. Restart Claude Code
# SPF Smart Gate starts automatically via MCP stdio
```

### Update/Redeployment
```bash
# 1. Rebuild
./build.sh --release

# 2. Redeploy (smart merge — preserves custom hooks)
./spf-deploy.sh

# 3. Restart Claude Code
```

### Verification
```bash
# Check deployment paths are correct
./spf-deploy.sh --verify

# Check binary
LIVE/BIN/spf-smart-gate/spf-smart-gate status
```

---

## 13.5 BOOT SEQUENCE (Full Chain)

When Claude Code starts with SPF configured:

```
1. Claude Code reads ~/.claude.json
   → Finds mcpServers.spf-smart-gate (stdio)
   → Spawns: LIVE/BIN/spf-smart-gate/spf-smart-gate serve

2. SPF binary boot (main.rs):
   a. env_logger::init()
   b. CLI parse → Serve subcommand
   c. Open CONFIG.DB → load SpfConfig
   d. Open SESSION.DB → load/create Session
   e. Call mcp::run(config, config_db, session, storage)

3. MCP server boot (mcp.rs run()):
   a. Log server name + version + mode
   b. Open PROJECTS.DB → init_defaults()
   c. Open TMP.DB
   d. Open LMDB5.DB → init_defaults()
   e. Open SPF_FS
   f. Enter stdin JSON-RPC read loop
   g. Respond to "initialize" with capabilities

4. Claude Code hook fires: SessionStart
   → session-start.sh runs:
     a. Reset session.json state
     b. Persist env vars via CLAUDE_ENV_FILE
     c. Inject SPF algorithm as additionalContext
     d. Source boot-lmdb5.sh (manifest loading)

5. Claude Code is now ready with:
   - All native tools BLOCKED (9 pre-hooks exit 1)
   - All MCP tools tracked (16+ pre-hooks exit 0)
   - SPF algorithm loaded in context
   - 6 LMDB databases open and serving
   - Brain + RAG binaries accessible
   - Audit logging active (cmd.log + spf.log)
```

---

## 13.6 FILE MANIFEST — COMPLETE SOURCE TREE

```
SPFsmartGATE/
├── Cargo.toml              # Package definition (13 deps)
├── Cargo.lock              # Dependency lock (59,637 bytes)
├── LICENSE                 # Copyright license
├── README.md               # Repository readme
├── HANDOFF.md              # Session handoff documentation
├── config.json             # Active Claude Code config (16,518 bytes)
├── .claude.json            # Global Claude config reference (16,515 bytes)
│
├── src/                    # Rust source (15 modules)
│   ├── lib.rs              # Module exports (34 lines)
│   ├── main.rs             # CLI entry + 12 subcommands (551 lines)
│   ├── paths.rs            # Root discovery + platform detection (86 lines)
│   ├── config.rs           # SpfConfig + path validation (227 lines)
│   ├── session.rs          # Session state management (192 lines)
│   ├── storage.rs          # LMDB session storage wrapper (100 lines)
│   ├── gate.rs             # 5-stage security pipeline (273 lines)
│   ├── validate.rs         # Write/edit/bash validation (413 lines)
│   ├── inspect.rs          # Content inspection (144 lines)
│   ├── calculate.rs        # Complexity formula (311 lines)
│   ├── web.rs              # HTTP client + SSRF protection (385 lines)
│   ├── fs.rs               # Virtual filesystem LMDB 1 (666 lines)
│   ├── config_db.rs        # Config DB LMDB 2 (517 lines)
│   ├── projects_db.rs      # Projects DB LMDB 3 (89 lines)
│   ├── tmp_db.rs           # TMP DB LMDB 4 (609 lines)
│   ├── agent_state.rs      # Agent State LMDB 5 (683 lines)
│   └── mcp.rs              # MCP server + 55 tools (3,516 lines)
│                           # TOTAL: ~7,796 lines of Rust
│
├── hooks/                  # 31 shell interceptors
│   ├── session-start.sh    # Session init + SPF context injection
│   ├── session-end.sh      # Handoff note generation
│   ├── user-prompt.sh      # Prompt complexity calculator
│   ├── stop-check.sh       # Stop event handler
│   ├── post-action.sh      # State checkpoint + brain sync
│   ├── post-failure.sh     # Failure logger
│   ├── pre-{read,edit,write,bash,glob,grep,notebookedit,webfetch,websearch}.sh
│   │                       # 9 native tool blockers (Layer 1)
│   ├── pre-mcp-{read,edit,write,bash,glob,grep,notebookedit,webfetch,websearch}.sh
│   │                       # 9 MCP tool trackers (Layer 2)
│   └── pre-{brain,rag,agent,config,projects,tmp,spf-meta}.sh
│                           # 7 LMDB domain trackers (Layer 2)
│
├── scripts/                # Support scripts
│   ├── boot-lmdb5.sh       # LMDB5 manifest boot loader
│   └── install-lmdb5.sh    # Full CLI containment installer
│
├── build.sh                # Cross-platform build script
├── spf-deploy.sh           # Settings generator + deployer
│
├── LIVE/                   # Runtime data (6 LMDBs + binaries)
│   ├── BIN/                # Compiled binary + deploy script
│   ├── CONFIG/CONFIG.DB/   # LMDB 2 (10 MB)
│   ├── SESSION/SESSION.DB/ # Session LMDB (50 MB)
│   ├── SPF_FS/             # LMDB 1 (4 GB)
│   ├── PROJECTS/           # LMDB 3 (20 MB) + device mount
│   ├── TMP/                # LMDB 4 (50 MB) + device mount
│   └── LMDB5/              # LMDB 5 (100 MB) + agent home
│
├── state/                  # Hook-generated state files
│   ├── session.json        # Current session state
│   ├── spf.log             # Audit log (512 MB rotation)
│   ├── failures.log        # Failure log
│   ├── STATUS.txt          # Human-readable status
│   ├── handoff.md          # Cross-session handoff note
│   └── boot.log            # LMDB5 boot log
│
├── storage/                # LMDB5 installation artifacts
│   ├── blobs/              # SHA-256 named binary blobs
│   ├── staging/            # Config file staging
│   └── lmdb5_manifest.json # Virtual FS manifest
│
├── backup/                 # Pre-installation backups
├── claude/                 # Claude-specific configs
├── docs/                   # Additional documentation
└── target/                 # Cargo build output
```

---

## 13.7 PLATFORM SUPPORT

| Platform | Architecture | Target Triple | Status |
|----------|-------------|---------------|--------|
| Android/Termux | aarch64 | `aarch64-linux-android` | **Primary** (current deployment) |
| Linux | x86_64 | `x86_64-unknown-linux-gnu` | Supported |
| Linux | aarch64 | `aarch64-unknown-linux-gnu` | Supported |
| macOS | ARM (M-series) | `aarch64-apple-darwin` | Supported |
| macOS | Intel | `x86_64-apple-darwin` | Supported |

**Current deployment**: Android 13 (kernel 5.15.178), Samsung S918W, Termux environment, aarch64 binary at 5,180,560 bytes.
