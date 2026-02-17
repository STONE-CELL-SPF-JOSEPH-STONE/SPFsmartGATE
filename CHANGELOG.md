# Changelog

All notable changes to SPFsmartGATE will be documented in this
file. Format follows [Keep a Changelog](https://keepachangelog.com/).

---

## [2.0.0] — 2026-02-16

### Initial Public Release — Android/Termux (aarch64)

**SPFsmartGATE is a compiled Rust security gateway that sits
between AI agents and your system. Every tool call must pass
through the gate.**

### Added

#### Gate Enforcement Pipeline
- 5-stage pipeline: Rate Limit → Calculate → Validate →
  Inspect → Max Mode Escalation
- Per-minute rate limiting by tool category (write: 60/min,
  web: 30/min, read: 120/min)
- Max Mode escalation — violations escalate to CRITICAL tier
  instead of blocking, forcing maximum scrutiny

#### SPF Complexity Formula
- `C = basic^1 + deps^7 + complex^10 + files * 10`
- `a_optimal(C) = W_eff * (1 - 1 / ln(C + e))`
- 4-tier classification: SIMPLE, LIGHT, MEDIUM, CRITICAL
- Per-tool complexity weights for all 44 allowed tools
- Dynamic analyze/build percentage allocation per tier

#### Build Anchor Protocol
- Read-before-write enforcement — agents must read a file
  before editing or overwriting it
- Prevents blind modifications to existing code

#### Security
- Default-deny tool allowlist (44 known-safe, 10 hard-blocked)
- SSRF protection with full IPv4/IPv6 validation
- Content inspection: credential patterns, path traversal,
  shell injection detection
- Blocked path enforcement (configurable via CONFIG.DB)
- Dangerous command detection for bash operations

#### Architecture
- 6-database LMDB architecture (SPF_FS, CONFIG, PROJECTS,
  TMP_DB, AGENT_STATE, SESSION)
- MCP server via stdio JSON-RPC 2.0
- Pre-compiled binary for Android aarch64
- 17 Rust source modules (8,870+ lines)

#### Claude Code Integration
- 31 hook scripts for full Claude Code lifecycle coverage
- SessionStart, PreToolUse (9 matchers), PostToolUse,
  PostToolUseFailure, UserPromptSubmit, Stop, SessionEnd
- Pre-configured config.json with hooks, permissions, and
  21 pre-approved SPF MCP tools
- LMDB5 containment system (optional, via install-lmdb5.sh)

#### Deployment
- `setup.sh` — one-command installation for Android
- `build.sh` — cross-platform build script with auto-detection
- Universal Android support (auto-detects non-Termux environments)
- Pre-configured LIVE/ directory structure

#### Tools (44 allowed via MCP)
- File operations: spf_read, spf_write, spf_edit
- Search: spf_glob, spf_grep
- System: spf_bash
- Web: spf_web_search, spf_web_fetch, spf_web_download,
  spf_web_api
- Brain: spf_brain_search, spf_brain_store, spf_brain_context,
  spf_brain_index, spf_brain_list, spf_brain_status,
  spf_brain_recall, spf_brain_list_docs, spf_brain_get_doc
- RAG: spf_rag_collect_web, spf_rag_collect_file,
  spf_rag_collect_folder, spf_rag_smart_search, and more
- State: spf_calculate, spf_status, spf_session
- Config: spf_config_paths, spf_config_stats
- Projects: spf_projects_list, spf_projects_get,
  spf_projects_set
- Filesystem: spf_fs_ls, spf_fs_read, spf_fs_write,
  spf_fs_exists, spf_fs_stat, spf_fs_mkdir, spf_fs_rm,
  spf_fs_rename
- Notebook: spf_notebook_edit

### Platform

- **Target**: Android aarch64 (Termux and compatible)
- **Binary**: Pre-compiled, 5.0MB release build
- **Dependencies**: Rust (build), Python 3 (hooks), LMDB
  library, Claude Code

---

## Copyright

Copyright 2026 Joseph Stone. All Rights Reserved.

Licensed under [PolyForm Noncommercial 1.0.0](LICENSE.md).
Commercial use requires a [separate license](COMMERCIAL_LICENSE.md).
