# BLOCK 12 — HOOK SYSTEM (Dual-Layer Shell Interceptors)

> **Source**: `hooks/` directory (31 shell scripts) + `scripts/` directory (2 support scripts)
> **Role**: Dual-layer interception of ALL tool calls — native Claude Code tools AND MCP server tools
> **Copyright**: 2026 Joseph Stone — All Rights Reserved

---

## 12.1 HOOK ARCHITECTURE OVERVIEW

SPF implements a **dual-layer hook system** that intercepts tool calls at TWO points:

```
┌─────────────────────────────────────────────────────────┐
│                    CLAUDE CODE CLI                        │
│                                                          │
│  User Prompt ──→ [user-prompt.sh] ──→ AI Processing     │
│                                                          │
│  AI calls Native Tool (Read/Write/Edit/Bash/etc.)       │
│       │                                                  │
│       ▼                                                  │
│  LAYER 1: Native Pre-Hooks ──→ EXIT 1 = BLOCKED         │
│  (pre-read.sh, pre-write.sh, pre-edit.sh, etc.)        │
│       │                                                  │
│       ▼ (All blocked — forces AI to use MCP tools)      │
│                                                          │
│  AI calls MCP Tool (spf_read, spf_write, etc.)          │
│       │                                                  │
│       ▼                                                  │
│  LAYER 2: MCP Pre-Hooks ──→ EXIT 0 = ALLOWED (logged)  │
│  (pre-mcp-read.sh, pre-mcp-write.sh, etc.)             │
│       │                                                  │
│       ▼                                                  │
│  SPF Gate Pipeline (Rust — gate.rs)                      │
│       │                                                  │
│       ▼                                                  │
│  [post-action.sh] ──→ State checkpoint + STATUS.txt     │
│  [post-failure.sh] ──→ Failure logging                  │
│                                                          │
│  Session End ──→ [session-end.sh] ──→ Handoff note      │
│  Stop Event  ──→ [stop-check.sh] ──→ State save         │
└─────────────────────────────────────────────────────────┘
```

**Key Design Principle**: Native tools are BLOCKED at Layer 1 (exit 1), forcing all operations through MCP tools. MCP tools are ALLOWED at Layer 2 (exit 0) with audit logging. The actual security enforcement happens in the Rust gate pipeline — hooks add observability and state management.

---

## 12.2 FILE INVENTORY — 31 HOOK SCRIPTS + 2 SUPPORT SCRIPTS

### Session Lifecycle (4 scripts)
| File | Size | Permission | Purpose |
|------|------|------------|---------|
| `session-start.sh` | 2,614 | `rw-------` | Session init, SPF context injection, LMDB5 boot |
| `session-end.sh` | 2,657 | `rwxr-xr-x` | Session checkpoint, handoff note generation |
| `user-prompt.sh` | 6,706 | `rwxr-xr-x` | Prompt complexity calculation, enforcement injection |
| `stop-check.sh` | 1,609 | `rwxr-xr-x` | Stop event handling, session state save |

### Post-Event (2 scripts)
| File | Size | Permission | Purpose |
|------|------|------------|---------|
| `post-action.sh` | 6,373 | `rwxr-xr-x` | State checkpoint, STATUS.txt update, brain sync |
| `post-failure.sh` | 1,188 | `rwxr-xr-x` | Failure logging to spf.log + failures.log |

### Layer 1: Native Tool Interceptors — ALL BLOCK (9 scripts)
| File | Size | Permission | Blocks |
|------|------|------------|--------|
| `pre-read.sh` | 238 | `rwxr-xr-x` | Native Read → forces `spf_read` |
| `pre-edit.sh` | 238 | `rwxr-xr-x` | Native Edit → forces `spf_edit` |
| `pre-write.sh` | 244 | `rwxr-xr-x` | Native Write → forces `spf_write` |
| `pre-bash.sh` | 238 | `rwxr-xr-x` | Native Bash → forces `spf_bash` |
| `pre-glob.sh` | 219 | `rwxr-xr-x` | Native Glob → forces `spf_glob` |
| `pre-grep.sh` | 219 | `rwxr-xr-x` | Native Grep → forces `spf_grep` |
| `pre-notebookedit.sh` | 269 | `rwxr-xr-x` | Native NotebookEdit → forces `spf_notebook_edit` |
| `pre-webfetch.sh` | 245 | `rwxr-xr-x` | Native WebFetch → forces `spf_web_fetch` |
| `pre-websearch.sh` | 251 | `rwxr-xr-x` | Native WebSearch → forces `spf_web_search` |

### Layer 2: MCP Tool Interceptors — ALL ALLOW WITH LOGGING (9 scripts)
| File | Size | Permission | Tracks |
|------|------|------------|--------|
| `pre-mcp-read.sh` | 603 | `rwx------` | MCP spf_read operations |
| `pre-mcp-edit.sh` | 603 | `rwx------` | MCP spf_edit operations |
| `pre-mcp-write.sh` | 607 | `rwx------` | MCP spf_write operations |
| `pre-mcp-bash.sh` | 603 | `rwx------` | MCP spf_bash operations |
| `pre-mcp-glob.sh` | 603 | `rwx------` | MCP spf_glob operations |
| `pre-mcp-grep.sh` | 603 | `rwx------` | MCP spf_grep operations |
| `pre-mcp-notebookedit.sh` | 635 | `rwx------` | MCP spf_notebook_edit operations |
| `pre-mcp-webfetch.sh` | 619 | `rwx------` | MCP spf_web_fetch operations |
| `pre-mcp-websearch.sh` | 623 | `rwx------` | MCP spf_web_search operations |

### Layer 2: LMDB Domain Interceptors — ALL ALLOW WITH LOGGING (7 scripts)
| File | Size | Permission | Tracks |
|------|------|------------|--------|
| `pre-brain.sh` | 599 | `rwx------` | Brain vector operations |
| `pre-rag.sh` | 601 | `rwx------` | RAG collector operations |
| `pre-agent.sh` | 605 | `rwx------` | Agent state LMDB operations |
| `pre-config.sh` | 612 | `rwx------` | Config DB operations |
| `pre-projects.sh` | 619 | `rwx------` | Projects DB operations |
| `pre-tmp.sh` | 600 | `rwx------` | TMP DB operations |
| `pre-spf-meta.sh` | 621 | `rwx------` | SPF meta (calculate, status, session) |

### Support Scripts (2 scripts in `scripts/`)
| File | Size | Permission | Purpose |
|------|------|------------|---------|
| `boot-lmdb5.sh` | 1,141 | `rwxr-xr-x` | LMDB5 manifest loading on boot |
| `install-lmdb5.sh` | 15,044 | `rwxr-xr-x` | Full CLI installation into LMDB5 virtual FS |

---

## 12.3 SESSION LIFECYCLE HOOKS (Detailed)

### 12.3.1 `session-start.sh` — Session Initialization

**Fires on**: `SessionStart` event (Claude CLI startup or `/clear`)
**Exit behavior**: Always exits 0

**Execution flow**:
1. Reads hook input from stdin (JSON with `source` field: `"startup"` or `"clear"`)
2. On `startup` or `clear` source: resets `state/session.json` with fresh state:
   ```json
   {
     "action_count": 0,
     "files_read": [],
     "files_written": [],
     "last_tool": null,
     "last_result": null,
     "started": "YYYY-MM-DD HH:MM:SS",
     "source": "startup"
   }
   ```
3. Persists environment variables via `$CLAUDE_ENV_FILE`:
   - `SPF_SESSION_START` (Unix epoch)
   - `SPF_STATE_DIR` (state directory path)
4. **Injects SPF context** as `additionalContext` via JSON output:
   - Complexity tier table (SIMPLE/LIGHT/MEDIUM/CRITICAL with C thresholds)
   - Master formula: `a_optimal(C) = W_eff × (1 - 1/ln(C + e))`
   - Enforcement rules (5 bullet points)
   - Active MCP server list
   - State file locations
5. Calls `scripts/boot-lmdb5.sh` (LMDB5 manifest loading)

**Output format**: JSON with `hookSpecificOutput.additionalContext` containing the SPF algorithm reference card.

### 12.3.2 `session-end.sh` — Session Checkpoint

**Fires on**: `SessionEnd` event
**Exit behavior**: Always exits 0

**Execution flow**:
1. Reads hook input from stdin (JSON with `reason` field)
2. Runs Python to build session summary:
   - Loads `state/session.json`
   - Generates `state/handoff.md` with:
     - Session times (start/end)
     - Total actions count
     - Files read count / files written count
     - Last 20 files modified
     - Instructions for next session startup
   - Updates `session.json` with `ended` timestamp and `end_reason`
3. Logs to `state/spf.log`

**Handoff note** is designed for cross-session continuity — the next `session-start.sh` can query brain for it.

### 12.3.3 `user-prompt.sh` — Prompt Complexity Calculator

**Fires on**: `UserPromptSubmit` event (every user message)
**Exit behavior**: Always exits 0
**Size**: 6,706 bytes — the most complex hook

**Execution flow**:
1. Reads hook input from stdin (JSON with `prompt` field)
2. Passes to Python via `_SPF_HOOK_INPUT` env var
3. **Python Prompt Complexity Calculator** runs:

   **Base score**: `min(len(prompt) / 10, 200)`

   **Signal Categories (regex-matched, additive)**:
   | Category | Patterns | Score Range |
   |----------|----------|-------------|
   | Math | `∫`, `∑`, `∂`, differential equations, eigenvalues, theorems | 100-500 per match |
   | Logic | Knight/knave, paradoxes, deduction, contradiction | 200-500 per match |
   | Science/Domain | implement, debug, refactor, CRISPR, quantum, algorithm | 150-400 per match |

   **Multi-step multiplier** (cumulative):
   - `and then`, `step N`, `first...then`: ×1.5
   - `compare...contrast`, `analyze...evaluate`: ×1.8
   - Length > 500 chars: ×1.3
   - Length > 1000 chars: ×1.5
   - Question marks > 2: ×(1 + count × 0.1)

   **Final**: `C = max(int((base + math + logic + domain) × multiplier), 50)`

4. **Tier determination**:
   | C Range | Tier | Analyze% | Build% |
   |---------|------|----------|--------|
   | < 500 | SIMPLE | 40 | 60 |
   | 500-1999 | LIGHT | 60 | 40 |
   | 2000-9999 | MEDIUM | 75 | 25 |
   | ≥ 10000 | CRITICAL | 95 | 5 |

5. **Formula application**: `a_optimal = int(40000 × (1 - 1/ln(C + e)))`

6. **Output injection** (3 tiers of enforcement context):
   - **C ≥ 2000**: Full enforcement block with 5-step analysis instructions + optimal token count
   - **C ≥ 500**: Light enforcement with analysis-first reminder
   - **C < 500**: Status line only: `[SPF Status] C={C} {TIER} | Actions: {n} | Reads: {r} | Writes: {w} | Last: {tool}`

   For C ≥ 500: Output as JSON `hookSpecificOutput.additionalContext` (injected into conversation)
   For C < 500: Plain text (shown as system-reminder)

7. Logs to `state/spf.log`: `[PROMPT] C={C} | {tier} | {analyze}%/{build}% | len={len}`

### 12.3.4 `stop-check.sh` — Stop Event Handler

**Fires on**: `Stop` event
**Exit behavior**: Always exits 0 (allows Claude to stop normally)

**Execution flow**:
1. Reads hook input from stdin
2. Checks `stop_hook_active` flag to **prevent infinite loops** (if already true, exits immediately)
3. Updates `session.json` with `last_stop` timestamp
4. Logs to `state/spf.log`

---

## 12.4 POST-EVENT HOOKS (Detailed)

### 12.4.1 `post-action.sh` — Universal Post-Tool Handler

**Fires after**: Every tool call completes
**Exit behavior**: Always exits 0
**Size**: 6,373 bytes — second most complex hook

**Execution flow**:
1. **Log rotation**: Checks `spf.log` and `failures.log` sizes
   - Max size: **512 MB** (536,870,912 bytes)
   - When exceeded: keeps last 5,000 lines, backs up to `.1`
2. Reads params from stdin (JSON with `tool_name`, `tool_input`)
3. **Updates session state** (Python):
   - Increments `action_count`
   - Sets `last_tool` and `last_result`
4. **STATUS.txt generation** (Python — "Memory Triad System 2"):
   - Tracks `files_written` (deduplicating on Write/Edit tool events)
   - Tracks `files_read` (deduplicating on Read tool events)
   - Generates comprehensive `state/STATUS.txt`:
     ```
     # SPF STATUS — Memory Triad System 2
     ## Current State: session actions, last tool, result, file
     ## Files Read This Session (N): last 10 listed
     ## Files Modified This Session (N): last 10 listed
     ## Build Anchor Status: reads/writes ratio
     ## Notes: recovery instructions
     ```
5. **Brain checkpoint** (async — backgrounded with `&`):
   - Only triggers on `Write` or `Edit` tool actions
   - Requires `$SPF_BRAIN_SYNC` = `true` (default) AND brain binary exists
   - Stores checkpoint to brain:
     - Collection: `spf_audit`
     - Tags: `spf,checkpoint,{tool_name}`
     - Content: tool, result, file, timestamp, action count
   - **Non-blocking**: runs in background to avoid tool latency

### 12.4.2 `post-failure.sh` — Failure Logger

**Fires after**: Tool execution fails
**Exit behavior**: Always exits 0 (passive — never blocks)

**Execution flow**:
1. Extracts `tool_name` and `error` (truncated to 200 chars) from stdin JSON
2. Writes to BOTH:
   - `state/spf.log`: `[timestamp] FAILURE: Tool '{name}' failed: {error}`
   - `state/failures.log`: `[timestamp] Tool '{name}' failed: {error}`

---

## 12.5 LAYER 1: NATIVE TOOL INTERCEPTORS (9 scripts)

All 9 native interceptors follow an **identical pattern** — they are deliberately minimal:

```bash
#!/bin/bash
# SPF Pre-{Tool} Hook - BLOCKS Native {Tool}
echo "BLOCKED: Native {Tool} disabled. Use spf_{tool} with approved=true"
exit 1
```

**Exit 1** = Claude Code's hook system **blocks the tool call**. The AI receives the BLOCKED message and is directed to use the MCP equivalent.

| Native Tool | Blocked By | Redirects To |
|-------------|-----------|--------------|
| Read | `pre-read.sh` | `spf_read` |
| Edit | `pre-edit.sh` | `spf_edit` |
| Write | `pre-write.sh` | `spf_write` |
| Bash | `pre-bash.sh` | `spf_bash` |
| Glob | `pre-glob.sh` | `spf_glob` |
| Grep | `pre-grep.sh` | `spf_grep` |
| NotebookEdit | `pre-notebookedit.sh` | `spf_notebook_edit` |
| WebFetch | `pre-webfetch.sh` | `spf_web_fetch` |
| WebSearch | `pre-websearch.sh` | `spf_web_search` |

**Security purpose**: Forces ALL tool operations through the MCP gateway where the Rust gate pipeline enforces complexity limits, path restrictions, Build Anchor Protocol, and content inspection.

---

## 12.6 LAYER 2: MCP TOOL INTERCEPTORS (9 scripts)

All 9 MCP interceptors follow an **identical pattern** — they ALLOW with audit logging:

```bash
#!/bin/bash
# SPF Pre-MCP-{tool} Hook - Allows MCP {tool} ops with tracking
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
LOG_FILE="$STATE_DIR/spf.log"
mkdir -p "$STATE_DIR"

if [ -t 0 ]; then PARAMS="${1:-{}}"; else PARAMS=$(cat); fi
TOOL_NAME=$(echo "$PARAMS" | python3 -c \
  "import sys,json; d=json.load(sys.stdin); print(d.get('tool_name', d.get('tool','unknown')))" \
  2>/dev/null || echo "mcp_{tool}")

echo "[$(date '+%Y-%m-%d %H:%M:%S')] PRE-MCP-{TOOL}: $TOOL_NAME" >> "$LOG_FILE"
exit 0
```

**Exit 0** = Tool call proceeds. The log entry provides an audit trail of every MCP tool invocation BEFORE the Rust gate pipeline processes it.

**Permission note**: All MCP interceptors use `rwx------` (700) — owner-only execution. Native interceptors use `rwxr-xr-x` (755).

---

## 12.7 LAYER 2: LMDB DOMAIN INTERCEPTORS (7 scripts)

These cover the remaining MCP tool domains not handled by the 9 tool-specific interceptors:

| Script | Domain | Tools Covered |
|--------|--------|---------------|
| `pre-brain.sh` | Brain operations | `spf_brain_search`, `spf_brain_store`, `spf_brain_context`, `spf_brain_index`, `spf_brain_list`, `spf_brain_status`, `spf_brain_recall`, `spf_brain_list_docs`, `spf_brain_get_doc` |
| `pre-rag.sh` | RAG collector | All 16 `spf_rag_*` tools |
| `pre-agent.sh` | Agent state | `spf_agent_stats`, `spf_agent_memory_search`, `spf_agent_memory_by_tag`, `spf_agent_session_info`, `spf_agent_context` |
| `pre-config.sh` | Config registry | `spf_config_paths`, `spf_config_stats` |
| `pre-projects.sh` | Projects registry | All 5 `spf_projects_*` tools |
| `pre-tmp.sh` | TMP registry | All 4 `spf_tmp_*` tools |
| `pre-spf-meta.sh` | SPF meta | `spf_calculate`, `spf_status`, `spf_session` |

All follow the same allow-with-logging pattern as the MCP interceptors. Combined with the 9 tool-specific interceptors, this provides **complete coverage of all 55 exposed MCP tools**.

---

## 12.8 SUPPORT SCRIPTS (`scripts/` directory)

### 12.8.1 `boot-lmdb5.sh` — LMDB5 Boot Injection

**Called by**: `session-start.sh` via `source` (silent failure: `2>/dev/null || true`)

**Purpose**: Loads LMDB5 manifest on startup and exports virtual path environment:
- Reads manifest from `storage/lmdb5_manifest.json`
- Finds SPF binary blob in `storage/blobs/` (first non-claude file)
- Exports:
  - `SPF_AGENT_HOME=/home/agent`
  - `SPF_CLAUDE_CONFIG=/home/agent/.claude.json`
  - `SPF_ACTIVE=1`
- Logs to `state/boot.log`

### 12.8.2 `install-lmdb5.sh` — Full CLI Installation Script

**Purpose**: One-time installation that transfers Claude CLI and all configs into LMDB5 virtual filesystem containment.

**Size**: 15,044 bytes — 11 phases:

| Phase | Function | What It Does |
|-------|----------|-------------|
| 1 | `preflight()` | Verifies SPF binary, Claude CLI, .claude.json, .claude/ all exist |
| 2 | `backup_existing()` | Copies `.claude.json` + `.claude/` (minus debug/) to timestamped backup |
| 3 | `copy_binaries()` | SHA-256 hashes SPF binary → blob; copies Claude CLI dir → blob |
| 4 | `copy_configs()` | Copies `.claude.json` + 6 small config files to staging |
| 5 | `copy_large_dirs()` | Copies 10 Claude directories + `history.jsonl` to blobs |
| 6 | `create_manifest()` | Converts pipe-delimited temp manifest to JSON format |
| 7 | `create_boot_injection()` | Generates `scripts/boot-lmdb5.sh` and injects into `session-start.sh` |
| 8 | `create_symlinks()` | Creates `agent-bin/` with symlinks to blob storage |
| 9 | `update_claude_json()` | Updates `.claude.json` mcpServers to use `agent-bin/spf-smart-gate` path |
| 10 | `verify_installation()` | Checks manifest, blobs, boot script, symlinks |
| 11 | `print_summary()` | Displays virtual FS tree and next steps |

**Config files captured** (small, inline):
- `.claude.json`, `settings.json`, `config.json`, `.credentials.json`, `claude.md`, `stats-cache.json`, `settings.local.json`

**Directories captured** (large, blob storage):
- `projects/`, `file-history/`, `paste-cache/`, `session-env/`, `todos/`, `plans/`, `tasks/`, `shell-snapshots/`, `statsig/`, `telemetry/`

**Manifest format**:
```json
{
  "version": "1.0",
  "created": "ISO-8601",
  "entries": [
    {"virtual": "/home/agent/.claude.json", "real": "/path/to/staging/claude.json", "size": "1234", "type": "config"},
    {"virtual": "/home/agent/bin/spf-smart-gate", "real": "/path/to/blob/sha256hash", "size": "5000000", "type": "binary"}
  ]
}
```

---

## 12.9 STATE FILES GENERATED BY HOOKS

The hook system maintains the following state files in `SPFsmartGATE/state/`:

| File | Written By | Content |
|------|-----------|---------|
| `session.json` | session-start, post-action, stop-check, session-end | Session state: action_count, files_read/written, last_tool, timestamps |
| `spf.log` | ALL hooks | Append-only audit log, all events timestamped, 512MB rotation |
| `failures.log` | post-failure | Dedicated failure log, 512MB rotation |
| `STATUS.txt` | post-action | Human-readable session summary (Memory Triad System 2) |
| `handoff.md` | session-end | Cross-session continuity note for brain storage |
| `boot.log` | boot-lmdb5 | LMDB5 boot injection log |

---

## 12.10 HOOK SECURITY MODEL

### Permission Separation
- **Native interceptors** (Layer 1): `755` — world-readable/executable (they only print a message and exit 1)
- **MCP interceptors** (Layer 2): `700` — owner-only (they access state files and log to disk)
- **Session lifecycle** hooks: Mixed (`session-start.sh` is `600`, others are `755`)

### No-Bypass Design
1. Native tools **cannot bypass** Layer 1 — hook system is configured in Claude Code's `settings.json`
2. MCP tools **cannot bypass** Layer 2 — hooks run before the MCP call reaches the Rust binary
3. Even if both layers are bypassed, the Rust gate pipeline (`gate.rs`) provides the **third and final enforcement layer**

### Anti-Loop Protection
- `stop-check.sh` checks `stop_hook_active` flag to prevent recursive stop events
- `post-action.sh` brain sync runs in background (`&`) to prevent blocking
- `session-start.sh` boot injection uses `|| true` to silently handle missing scripts

### Log Integrity
- All hooks append to `state/spf.log` with timestamps — never overwrite
- `post-action.sh` performs log rotation at 512MB with backup retention
- Brain checkpoints create immutable vector store entries in `spf_audit` collection

---

## 12.11 HOOK-TO-RUST INTERACTION SUMMARY

| Event | Hook Layer | Rust Layer | Combined Effect |
|-------|-----------|------------|-----------------|
| Session Start | SPF algorithm injected as context | Config loaded, session initialized | AI starts with SPF knowledge + clean state |
| User Prompt | Complexity calculated, tier enforcement injected | — | AI receives complexity guidance before responding |
| Native Read | **BLOCKED** (exit 1) | — | AI forced to use `spf_read` |
| MCP spf_read | Logged (exit 0) | Gate pipeline → validate_read → track_read | Read completes with full enforcement |
| Native Write | **BLOCKED** (exit 1) | — | AI forced to use `spf_write` |
| MCP spf_write | Logged (exit 0) | Gate pipeline → validate_write → anchor check → allowlist | Write completes with full enforcement |
| Post-Action | STATUS.txt updated, brain checkpoint (Write/Edit) | Session saved in LMDB | Dual-state tracking (file + LMDB) |
| Failure | Logged to failures.log | Failure recorded in session | Dual-failure tracking |
| Stop | Session state saved to JSON | — | State preserved for recovery |
| Session End | Handoff note generated | — | Cross-session continuity prepared |
