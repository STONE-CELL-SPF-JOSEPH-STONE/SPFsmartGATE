# SPFsmartGATE

**AI Security Gateway — Compiled Rust enforcement between AI agents and your system.**

[![License: PolyForm Noncommercial 1.0.0](https://img.shields.io/badge/License-PolyForm%20NC%201.0.0-blue.svg)](LICENSE.md)

---

> **Free for personal use. Commercial use requires a license.**
> See [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md) for details.
> For licensing inquiries, contact: joepcstone@gmail.com

---

## What Is SPFsmartGATE?

SPFsmartGATE is a compiled Rust security gateway that sits
between AI agents and your system. Every tool call from any
AI agent must pass through the gate before touching the
filesystem, running commands, or accessing the web.

It enforces the **StoneCell Processing Formula (SPF)** for
AI self-governance — a mathematical complexity model that
calculates risk, allocates resources, and gates operations
in real time.

The gate is compiled into a native binary. There is no
runtime configuration bypass. The rules are in the code.

## Core Features

### Gate Enforcement Pipeline

Every tool call passes through a 5-stage pipeline:

1. **Rate Limiting** — Per-minute operation caps by tool
   category (write: 60/min, web: 30/min, read: 120/min)
2. **Complexity Calculation** — The SPF formula computes a
   complexity score C for every operation
3. **Validation** — Build Anchor protocol, write allowlist,
   path blocking, dangerous command detection
4. **Content Inspection** — Credential detection, path
   traversal, shell injection scanning
5. **Max Mode Escalation** — Violations escalate to CRITICAL
   tier instead of blocking, forcing maximum scrutiny

### SPF Complexity Formula

```
C = basic^1 + deps^7 + complex^10 + files * 10

a_optimal(C) = W_eff * (1 - 1 / ln(C + e))
```

The formula produces a complexity score that determines:
- **Tier assignment**: SIMPLE, LIGHT, MEDIUM, CRITICAL
- **Resource allocation**: Analyze vs Build percentage
- **Approval requirements**: Per-tier escalation policy

### Build Anchor Protocol

AI agents must read a file before they can edit or overwrite
it. This prevents blind modifications and forces the agent
to understand existing code before changing it.

### Default-Deny Security

Unknown tools are blocked until explicitly added to the gate
allowlist. **55 tools are exposed and allowed** through the
gate. **8 filesystem tools are hard-blocked** at the gate
level (registered but denied). Everything else is denied.

### SSRF Protection

Full IPv4 and IPv6 validation on all web requests. Blocks
loopback, private networks, link-local, cloud metadata
endpoints, and IPv4-mapped IPv6 addresses.

### Content Inspection

Scans all write operations for:
- Credential patterns (API keys, tokens, private keys)
- Path traversal sequences
- Shell injection patterns
- References to blocked paths

## Architecture

SPFsmartGATE uses a 6-database LMDB architecture:

| Database    | Size   | Purpose                          |
|-------------|--------|----------------------------------|
| SESSION     | 50 MB  | Runtime session persistence      |
| CONFIG      | 10 MB  | Configuration and path rules     |
| PROJECTS    | 20 MB  | Project registry                 |
| TMP_DB      | 50 MB  | Metadata tracking and trust      |
| AGENT_STATE | 100 MB | Agent memory and sessions        |
| SPF_FS      | 4 GB   | Virtual filesystem (system-only) |

The MCP (Model Context Protocol) server communicates via
stdio JSON-RPC 2.0, making it compatible with any MCP client.

### Source Modules

```
src/
├── main.rs          — CLI entry point, subcommand dispatch
├── lib.rs           — Library root, module declarations, shared types
├── mcp.rs           — MCP JSON-RPC 2.0 server, 63 tool handlers
├── gate.rs          — 5-stage enforcement pipeline, tool allowlist
├── calculate.rs     — SPF complexity formula, tier assignment
├── validate.rs      — Write allowlist, Build Anchor, path blocking
├── inspect.rs       — Content inspection (creds, traversal, injection)
├── session.rs       — Session LMDB, action logging, metrics
├── storage.rs       — Multi-LMDB environment orchestration (6 databases)
├── web.rs           — HTTP client, SSRF protection, URL validation
├── fs.rs            — Virtual filesystem operations (system-only, blocked)
├── config.rs        — Configuration loading and defaults
├── paths.rs         — Hardcoded path constants, write allowlist
├── config_db.rs     — CONFIG LMDB operations
├── projects_db.rs   — PROJECTS LMDB operations
├── tmp_db.rs        — TMP_DB LMDB + trust management
└── agent_state.rs   — AGENT_STATE LMDB, memory, sessions
```

15 modules + 2 entry points — ~7,800 lines of Rust.

### Tools Overview

**55 tools exposed via MCP** across 12 categories:

| Category         | Count | Key Tools                                          |
|------------------|-------|----------------------------------------------------|
| File Operations  | 3     | spf_read, spf_write, spf_edit                      |
| Search           | 2     | spf_glob, spf_grep                                 |
| System           | 1     | spf_bash                                           |
| Web              | 4     | spf_web_search, spf_web_fetch, spf_web_api, ...    |
| Brain (RAG Core) | 9     | spf_brain_search, spf_brain_store, spf_brain_recall |
| RAG Pipeline     | 16    | spf_rag_collect_web, spf_rag_smart_search, ...     |
| State & Metrics  | 3     | spf_calculate, spf_status, spf_session             |
| Config           | 2     | spf_config_paths, spf_config_stats                 |
| Projects         | 5     | spf_projects_list, spf_projects_get, ...            |
| TMP / Metadata   | 4     | spf_tmp_list, spf_tmp_stats, spf_tmp_get, ...      |
| Agent State      | 5     | spf_agent_memory_search, spf_agent_context, ...    |
| Notebook         | 1     | spf_notebook_edit                                  |

**8 hard-blocked tools** (registered but gate-denied):
spf_fs_ls, spf_fs_read, spf_fs_write, spf_fs_exists,
spf_fs_stat, spf_fs_mkdir, spf_fs_rm, spf_fs_rename

> For the complete tool reference with all parameters, descriptions,
> and LMDB routing, see [MCP_TOOLS.md](SPFsmartGATEdevBIBLE/MCP_TOOLS.md).

### Resilience & Error Handling

SPFsmartGATE does not silently fail:

- **Invalid JSON-RPC** — Malformed requests are rejected at
  the MCP parse layer with proper JSON-RPC error responses
- **Max Mode Escalation** — Repeated violations or suspicious
  patterns escalate the tier to CRITICAL, forcing maximum
  analysis allocation (95% analyze, 5% build) rather than
  simply blocking
- **LMDB Integrity** — Each of the 6 databases operates in
  its own memory-mapped environment. Corruption in one does
  not cascade to others
- **Build Anchor Recovery** — If anchor state is lost, the
  gateway requires a fresh read before allowing any writes,
  preventing stale-state mutations

### Observability

Every operation is tracked via the **SESSION LMDB database**:

- Action counter, read/write counts, last tool used
- Per-operation complexity scores and tier assignments
- Timestamp-indexed session history
- Real-time query via `spf_status` and `spf_session` tools

The **31 hook scripts** provide additional lifecycle logging
for Claude Code integration — session start/end, every tool
call pre/post, prompt submissions, and failure events.

> For hook architecture details, see [HOOKS.md](SPFsmartGATEdevBIBLE/HOOKS.md).

### Key Dependencies

| Crate       | Version | Purpose                              |
|-------------|---------|--------------------------------------|
| heed        | 0.20    | Safe Rust LMDB bindings              |
| serde       | 1.0     | Serialization framework              |
| serde_json  | 1.0     | JSON parsing (MCP protocol)          |
| clap        | 4.5     | CLI argument parsing                 |
| reqwest     | 0.12    | HTTP client (rustls-tls, blocking)   |
| html2text   | 0.6     | HTML → plain text conversion         |
| sha2        | 0.10    | SHA-256 checksums for file integrity |
| chrono      | 0.4     | Timestamps and date handling         |
| thiserror   | 1.0     | Typed error definitions              |
| anyhow      | 1.0     | Flexible error propagation           |

All TLS via **rustls** (pure Rust) — no OpenSSL dependency.

## Installation

### Requirements

- Rust toolchain (rustup.rs)
- LMDB system library

### Build

```bash
cd SPFsmartGATE
cargo build --release
```

The binary is built at `target/release/spf-smart-gate`.

### Initialize Configuration

```bash
./target/release/spf-smart-gate init-config
```

### Start the MCP Server

```bash
./target/release/spf-smart-gate serve
```

### Start Claude/agent  
##Must copy CLI files to flat LMDB5 
##and then export to LMDB5.db
##in new terminal

```bash
HOME=~/SPFsmartGATE/LIVE/LMDB5 claude
```

## CLI Commands

| Command         | Description                        |
|-----------------|------------------------------------|
| `serve`         | Start the MCP server               |
| `gate`          | Process a single tool call         |
| `calculate`     | Calculate complexity for a tool    |
| `status`        | Show SPF gateway status            |
| `session`       | Show current session state         |
| `reset`         | Reset session state                |
| `init-config`   | Initialize configuration database  |
| `refresh-paths` | Refresh blocked/allowed paths      |

## Documentation

| Document | Description |
|----------|-------------|
| [SPFsmartGATEdevBIBLE.md](SPFsmartGATEdevBIBLE/SPFsmartGATEdevBIBLE.md) | Complete technical reference — all 13 blocks covering every feature, security protocol, and implementation detail |
| [MCP_TOOLS.md](SPFsmartGATEdevBIBLE/MCP_TOOLS.md) | Deep-dive on all 55 exposed tools + 13 hidden handlers, LMDB routing, parameter specs |
| [HOOKS.md](SPFsmartGATEdevBIBLE/HOOKS.md) | Full hook system architecture — 31 scripts, dual-layer design, lifecycle coverage |
| [DEPLOYMENT.md](SPFsmartGATEdevBIBLE/DEPLOYMENT.md) | Build system, deployment scripts, config.json structure, LIVE directory layout |
| [SECURITY.md](SECURITY.md) | Security policy and vulnerability reporting |
| [CHANGELOG.md](CHANGELOG.md) | Version history and release notes |
| [NOTICE.md](NOTICE.md) | Attribution and third-party dependency information |

## License

SPFsmartGATE is licensed under the
[PolyForm Noncommercial License 1.0.0](LICENSE.md).

**Personal, educational, and noncommercial use is free.**

**All commercial use requires a separate license.**
See [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md) or
contact joepcstone@gmail.com for licensing inquiries.

## Copyright

Copyright 2026 Joseph Stone. All Rights Reserved.

The StoneCell Processing Formula (SPF) and SPFsmartGATE are
proprietary intellectual property. See [NOTICE.md](NOTICE.md)
for full attribution and third-party dependency information.
