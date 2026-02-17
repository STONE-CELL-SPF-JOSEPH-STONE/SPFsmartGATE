# BLOCK 11 — MCP SERVER & ALL TOOL HANDLERS

> **Source**: `src/mcp.rs` (3,516 lines)
> **Role**: JSON-RPC 2.0 server over stdio — the single entry point for ALL AI agent tool calls
> **Copyright**: 2026 Joseph Stone — All Rights Reserved

---

## 11.1 MCP SERVER IDENTITY & PROTOCOL

```
SERVER_NAME     = "spf-smart-gate"
SERVER_VERSION  = "2.0.0"
PROTOCOL_VERSION = "2024-11-05"   (MCP JSON-RPC wire version)
```

**Communication**: All traffic flows via JSON-RPC 2.0 over stdin/stdout.
- `stdin` → JSON-RPC requests from Claude Code
- `stdout` → JSON-RPC responses (locked, flushed per message)
- `stderr` → Diagnostic logging via `log()` function

---

## 11.2 HELPER FUNCTIONS (Lines 29-231)

### 11.2.1 `format_timestamp(ts: u64) → String`
Converts Unix epoch to `YYYY-MM-DD HH:MM:SS UTC` via chrono. Returns `"Never"` for ts=0.

### 11.2.2 `brain_path() → PathBuf`
Returns: `$HOME/stoneshell-brain/target/release/brain`

### 11.2.3 `run_brain(args: &[&str]) → (bool, String)`
Executes the Brain binary with injected flags:
- `-m $HOME/stoneshell-brain/models/all-MiniLM-L6-v2` (embedding model)
- `-s $HOME/stoneshell-brain/storage` (vector storage dir)
- Working directory: `$HOME/stoneshell-brain/`
- Returns `(success, stdout_or_stderr)`

### 11.2.4 `rag_collector_path() → PathBuf`
Resolution order:
1. `SPF_RAG_PATH` environment variable
2. `LIVE/BIN/rag-collector/server.py` (conventional)
3. `/storage/emulated/0/Download/api-workspace/projects/MCP_RAG_COLLECTOR/server.py` (legacy Android)

### 11.2.5 `rag_collector_dir() → PathBuf`
Parent directory of `rag_collector_path()` — used as working directory.

### 11.2.6 `run_rag(args: &[&str]) → (bool, String)`
Executes: `python3 -u <rag_path> <args>` with working directory set to `rag_collector_dir()`.
- `-u` flag forces unbuffered Python output
- Returns `(success, stdout_or_stderr_combined)`

### 11.2.7 `log(msg: &str)`
Writes to stderr: `[spf-smart-gate] {msg}`

### 11.2.8 `cmd_log(msg: &str)`
**Persistent audit log** → `LIVE/SESSION/cmd.log`
- Timestamped: `[YYYY-MM-DD HH:MM:SS] {msg}`
- Append-only via `OpenOptions::append(true)`
- Records EVERY tool call and EVERY failure

### 11.2.9 `param_summary(name: &str, args: &Value) → String`
Smart parameter summarizer for logging — truncates large values:
| Tool pattern       | Summary format                    | Truncation |
|-------------------|-----------------------------------|------------|
| `*bash*`          | `cmd={command}`                   | 200 chars  |
| `*read*/*edit*/*glob*` | `path={path} pattern={pattern}` | none       |
| `*write*`         | `path={path} content_len={len}`   | none       |
| `*grep*`          | `pattern={pat} path={path}`       | none       |
| `*web*`           | `query={q}` or `url={url}`        | none       |
| `*brain*/*rag*`   | `q={query/text/path}`             | 150 chars  |
| default           | Full JSON string                  | 300 chars  |

### 11.2.10 `send_response(id: &Value, result: Value)`
Formats and writes JSON-RPC 2.0 success response to stdout (locked, flushed).

### 11.2.11 `send_error(id: &Value, code: i64, message: &str)`
Formats and writes JSON-RPC 2.0 error response to stdout. Code `-32601` for unknown methods.

### 11.2.12 `tool_def(name, description, properties, required) → Value`
Builds MCP tool definition JSON with `inputSchema` wrapper.

---

## 11.3 TOOL DEFINITIONS — 55 EXPOSED TOOLS (Lines 234-711)

All tools returned by `tool_definitions()` via `tools/list` response.

### Category 1: Core Gate (3 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 1 | `spf_calculate` | **tool**, **params** | Dry-run complexity calculation without execution |
| 2 | `spf_status` | *(none)* | Session metrics, enforcement mode, complexity budget |
| 3 | `spf_session` | *(none)* | Full session state: files, actions, anchor ratio, history |

### Category 2: Gated File Operations (4 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 4 | `spf_read` | **file_path**, limit?, offset? | Read file; tracks for Build Anchor Protocol |
| 5 | `spf_write` | **file_path**, **content** | Write file; validates anchor, allowlist, size |
| 6 | `spf_edit` | **file_path**, **old_string**, **new_string**, replace_all? | Edit file; validates anchor, allowlist, blocked paths |
| 7 | `spf_bash` | **command**, timeout? (default:30) | Execute bash; validates dangerous cmds, /tmp, git force |

### Category 3: Search/Glob (2 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 8 | `spf_glob` | **pattern**, path? | `find` command with pattern, max 100 results |
| 9 | `spf_grep` | **pattern**, path?, glob?, case_insensitive?, context_lines? | `rg` (ripgrep), max 500 lines |

### Category 4: Web Browser (4 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 10 | `spf_web_search` | **query**, count? (default:10) | Brave API or DuckDuckGo HTML fallback |
| 11 | `spf_web_fetch` | **url**, **prompt** | Fetch URL → clean text (50KB truncation) |
| 12 | `spf_web_download` | **url**, **save_path** | Download file to disk, tracks write |
| 13 | `spf_web_api` | **method**, **url**, headers?, body? | HTTP API request (GET/POST/PUT/DELETE/PATCH) |

### Category 5: Notebook (1 tool)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 14 | `spf_notebook_edit` | **notebook_path**, **new_source**, cell_number?, cell_type?, edit_mode? | Jupyter cell edit (replace/insert/delete) |

### Category 6: Brain Integration (8 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 15 | `spf_brain_search` | **query**, collection?, limit? | Vector search via Brain binary |
| 16 | `spf_brain_store` | **text**, title?, collection?, tags? | Store + auto-index document |
| 17 | `spf_brain_context` | **query**, max_tokens? | Formatted context for prompt injection |
| 18 | `spf_brain_index` | **path** | Index file/directory into brain |
| 19 | `spf_brain_list` | *(none)* | List all collections and document counts |
| 20 | `spf_brain_status` | *(none)* | Binary check, collection list, storage size |
| 21 | `spf_brain_recall` | **query**, collection? | Full parent document retrieval |
| 22 | `spf_brain_list_docs` | collection? | List stored documents in collection |
| 23 | `spf_brain_get_doc` | **doc_id**, collection? | Retrieve document by ID |

### Category 7: RAG Collector (12 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 24 | `spf_rag_collect_web` | topic?, auto_index? | Web search + document collection |
| 25 | `spf_rag_collect_file` | **path**, category? | Process local file |
| 26 | `spf_rag_collect_folder` | **path**, extensions? | Process all files in folder |
| 27 | `spf_rag_collect_drop` | *(none)* | Process files in DROP_HERE folder |
| 28 | `spf_rag_index_gathered` | category? | Index GATHERED docs to brain |
| 29 | `spf_rag_dedupe` | **category** | Deduplicate brain collection (via brain binary) |
| 30 | `spf_rag_status` | *(none)* | Collector status and stats |
| 31 | `spf_rag_list_gathered` | category? | List documents in GATHERED folder |
| 32 | `spf_rag_bandwidth_status` | *(none)* | Bandwidth usage stats and limits |
| 33 | `spf_rag_fetch_url` | **url**, auto_index? | Single URL fetch with bandwidth limiting |
| 34 | `spf_rag_collect_rss` | feed_name?, auto_index? | Collect from RSS/Atom feeds |
| 35 | `spf_rag_list_feeds` | *(none)* | Read RSS sources config directly |

### Category 8: RAG Smart Search (4 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 36 | `spf_rag_pending_searches` | collection? | Get SearchSeeker gaps (brain binary) |
| 37 | `spf_rag_fulfill_search` | **seeker_id**, collection? | Mark SearchSeeker fulfilled |
| 38 | `spf_rag_smart_search` | **query**, collection? | Smart search with completeness check (<80% triggers seeker) |
| 39 | `spf_rag_auto_fetch_gaps` | collection?, max_fetches? | Auto-fetch for pending seekers |

### Category 9: Config DB (2 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 40 | `spf_config_paths` | *(none)* | List path rules (allowed/blocked) |
| 41 | `spf_config_stats` | *(none)* | Config entries, path rules, dangerous patterns count |

### Category 10: Projects DB (5 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 42 | `spf_projects_list` | *(none)* | List all project key-value entries |
| 43 | `spf_projects_get` | **key** | Get project entry by key |
| 44 | `spf_projects_set` | **key**, **value** | Set project entry |
| 45 | `spf_projects_delete` | **key** | Delete project entry |
| 46 | `spf_projects_stats` | *(none)* | PROJECTS LMDB entry count |

### Category 11: TMP DB (4 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 47 | `spf_tmp_list` | *(none)* | List projects with trust levels |
| 48 | `spf_tmp_stats` | *(none)* | Projects, access logs, resource counts |
| 49 | `spf_tmp_get` | **path** | Full project details (18 fields) |
| 50 | `spf_tmp_active` | *(none)* | Currently active project |

### Category 12: Agent State (5 tools)
| # | Tool | Params (required bold) | Description |
|---|------|----------------------|-------------|
| 51 | `spf_agent_stats` | *(none)* | Memories, sessions, state keys, tags count |
| 52 | `spf_agent_memory_search` | **query**, limit? | Search memories by content |
| 53 | `spf_agent_memory_by_tag` | **tag** | Get memories by tag |
| 54 | `spf_agent_session_info` | *(none)* | Latest session info (chain, files, C, actions) |
| 55 | `spf_agent_context` | *(none)* | Context summary for session continuity |

---

## 11.4 HIDDEN / BLOCKED TOOL HANDLERS (Not in tool_definitions)

These handlers exist in `handle_tool_call` but are **NOT registered** in `tool_definitions()`. They exist as defense-in-depth to return explicit BLOCKED responses if the AI agent somehow discovers or guesses the tool names.

### HARD BLOCKED — Removed from Registry
| Handler | Response | Reason |
|---------|----------|--------|
| `spf_gate` | `"BLOCKED: spf_gate is not available"` | Was a bypass vector — gate is internal only |
| `spf_config_get` | `"BLOCKED: Config read/write is user-only (use CLI)"` | Security: config modification is human-only |
| `spf_config_set` | `"BLOCKED: Config read/write is user-only (use CLI)"` | Security: config modification is human-only |
| `spf_agent_remember` | `"BLOCKED: Agent state writes are user-only (use CLI)"` | Security: memory writes are human-only |
| `spf_agent_forget` | `"BLOCKED: Agent state writes are user-only (use CLI)"` | Security: memory deletion is human-only |
| `spf_agent_set_state` | `"BLOCKED: Agent state writes are user-only (use CLI)"` | Security: state modification is human-only |

### HARD BLOCKED — FS Tools (gate.rs defense-in-depth)
These have full handlers (routing to LMDB partitions) but are:
1. NOT in `tool_definitions()` (invisible to AI)
2. HARD BLOCKED in `gate.rs` validation (10 spf_fs_* tools)

| Handler | Route Target | Operations |
|---------|-------------|------------|
| `spf_fs_exists` | `route_to_lmdb → /config, /tmp, /projects, /home/agent` | Existence check |
| `spf_fs_stat` | `route_to_lmdb → metadata/type/mount info` | File/dir metadata |
| `spf_fs_ls` | `route_to_lmdb → directory listing` | List directory contents |
| `spf_fs_read` | `route_to_lmdb → file content` | Read file content |
| `spf_fs_write` | `route_to_lmdb → write content` | Write file content |
| `spf_fs_mkdir` | `route_to_lmdb → create directory` | Create directory |
| `spf_fs_rm` | `route_to_lmdb → remove` | Remove file/empty dir |
| `spf_fs_rename` | `route_to_lmdb + device rename` | Rename/move with traversal protection |

### Default Handler
Any unknown tool name → `"Unknown tool: {name}"` (DEFAULT DENY)

**Total: 55 exposed + 6 explicitly blocked + 8 FS blocked = 69 handler branches** (including the `_` default).

---

## 11.5 LMDB PARTITION ROUTING SYSTEM (Lines 714-1140)

The virtual filesystem routes `spf_fs_*` operations to the correct LMDB database or device directory based on path prefix.

### 11.5.1 `route_to_lmdb(path, op, content, config_db, tmp_db, agent_db) → Option<Value>`

**Mount Point Dispatcher** — decides which handler gets the virtual path:

| Virtual Path Prefix | Handler | Backend |
|--------------------|---------|---------|
| `/config` or `/config/*` | `route_config()` | CONFIG.DB (LMDB 2) |
| `/tmp` or `/tmp/*` | `route_device_dir()` | `LIVE/TMP/TMP/` (device filesystem) |
| `/projects` or `/projects/*` | `route_device_dir()` | `LIVE/PROJECTS/PROJECTS/` (device filesystem) |
| `/home/agent/tmp` or `/home/agent/tmp/*` | **Redirects to `/tmp/*`** → `route_device_dir()` | `LIVE/TMP/TMP/` |
| `/home/agent` or `/home/agent/*` | `route_agent()` | LMDB5.DB (Agent State) |
| Anything else | Returns `None` → falls through to SpfFs (LMDB 1) | SPF_FS.DB |

### 11.5.2 `route_config(path, op, config_db) → Value`

**Read-only mount** at `/config/`. ALL write/mkdir/rm/rename operations BLOCKED.

Virtual files exposed:
| Path | Source | Content |
|------|--------|---------|
| `/config/version` | `config_db.get("spf", "version")` | Version string |
| `/config/mode` | `config_db.get_enforce_mode()` | `Soft` or `Max` |
| `/config/tiers` | `config_db.get_tiers()` | Pretty-printed JSON |
| `/config/formula` | `config_db.get_formula()` | Pretty-printed JSON |
| `/config/weights` | `config_db.get_weights()` | Pretty-printed JSON |
| `/config/paths` | `config_db.list_path_rules()` | `type: path` lines |
| `/config/patterns` | `config_db.list_dangerous_patterns()` | `pattern (severity: N)` lines |

### 11.5.3 `route_device_dir(virtual_path, mount_prefix, device_base, op, content, tmp_db) → Value`

**Device-backed mount** — real filesystem operations on device disk. Used for `/tmp/` and `/projects/`.

**Security**: Path traversal protection — rejects any path containing `..`

**Operations**:
| Op | Implementation | Notes |
|----|---------------|-------|
| `ls` | `std::fs::read_dir()` | Sorted, formatted as `d755/−644 size name` |
| `read` | `std::fs::read_to_string()` | Logs to TMP_DB via `log_access()` |
| `write` | `std::fs::write()` + auto `create_dir_all()` for parents | Logs to TMP_DB via `log_access()` |
| `exists` | `device_path.exists()` | Returns `EXISTS` or `NOT FOUND` |
| `stat` | `std::fs::metadata()` | Type, size, mount info, access level |
| `mkdir` | `std::fs::create_dir_all()` | Creates all parent directories |
| `rm` | `remove_dir()` or `remove_file()` | Dir must be empty |
| `rename` | Deferred to `spf_fs_rename` handler | Not handled at this level |

### 11.5.4 `route_agent(path, op, agent_db) → Value`

**LMDB 5 mount** at `/home/agent/`. ALL write operations BLOCKED (read-only from AI perspective).

**Three data sources merged**:
1. **Skeleton directories** — hardcoded virtual FS layout (14 root entries, 10 `.claude/` entries, etc.)
2. **State DB file: keys** — dynamically imported config files from LMDB5 `state` database
3. **Dedicated databases** — memory, sessions, state (filtered: no `file:` keys)

**Special dynamic directories**:
| Path | Data Source | Content |
|------|-------------|---------|
| `/home/agent/memory` | `db.search_memories("", 100)` | Memory entries as `−644 {size} {id}` |
| `/home/agent/sessions` | `db.get_latest_session()` → `get_session_chain()` | Session chain as `−644 {actions} {id}` |
| `/home/agent/state` | `db.list_state_keys()` (excludes `file:*`) | State keys as `−644 0 {key}` |

**Skeleton root (`/home/agent/`)**:
```
.claude.json, .claude/, bin/, tmp/, .config/, .local/, .cache/,
.memory/, .ssh/, Documents/, Projects/, workspace/, preferences, context
```

**Read routing**:
| Path | Source |
|------|--------|
| `/home/agent/preferences` | `db.get_preferences()` → pretty JSON |
| `/home/agent/context` | `db.get_context_summary()` |
| `/home/agent/memory/{id}` | `db.get_memory(id)` → formatted entry |
| `/home/agent/sessions/{id}` | `db.get_session(id)` → formatted session |
| `/home/agent/state/{key}` | `db.get_state(key)` |
| `/home/agent/{anything}` | `db.get_state("file:{relative}")` → **Dynamic file read** |
| Fallback | `"not found: /home/agent/{relative}"` |

### 11.5.5 `scan_state_dir(db, dir_relative) → Vec<String>`
Helper for `route_agent` LS operations. Scans LMDB5 state keys with prefix `file:{dir}/` and returns:
- Immediate child directories as `d755        0 {name}`
- Immediate child files as `-644        0 {name}`
- Uses `BTreeSet` for sorted, deduplicated output

### 11.5.6 `spf_fs_rename` — Special Device Rename Handler (Lines 3303-3350)
The rename handler has **special device-backed directory logic** that runs BEFORE `route_to_lmdb`:
- If `old_path` starts with `/tmp/` or `/projects/`: device filesystem rename
- Path traversal protection: rejects `..` in either path
- Resolves virtual paths to `LIVE/TMP/TMP/` or `LIVE/PROJECTS/PROJECTS/`
- Creates parent directories for new_path automatically
- Falls through to `route_to_lmdb` for non-device paths

---

## 11.6 TOOL HANDLER PATTERNS (Lines 1140-3356)

Every handler follows a consistent security pattern:

```
1. EXTRACT params from args JSON
2. BUILD ToolParams struct
3. GATE CHECK: gate::process(tool_name, &params, config, session)
   → If !allowed: record_manifest(BLOCKED) → save session → return BLOCKED
4. RECORD ACTION: session.record_action(category, action, detail)
5. EXECUTE the actual operation
6. RECORD MANIFEST: session.record_manifest(tool, C, status, notes)
7. SAVE SESSION: storage.save_session(session)
8. RETURN result JSON
```

### Critical Handler Details:

#### `spf_read` (Lines ~1395-1470)
- Reads via `std::fs::read_to_string()`
- Supports `offset` (line-based) and `limit` (line count)
- Tracks read via `session.track_read(file_path)` → **enables Build Anchor Protocol**
- Content truncated at 50,000 characters with warning
- Returns `"[Binary file — {len} bytes]"` if UTF-8 decode fails

#### `spf_write` (Lines ~1470-1530)
- **Auto-creates parent directories**: `create_dir_all(parent)`
- Writes via `std::fs::write()`
- Tracks write via `session.track_write(file_path)`
- Returns byte count on success

#### `spf_edit` (Lines ~1530-1590)
- Reads file → performs string replacement → writes back
- `replace_all=false` (default): `replacen(old, new, 1)` — single replacement
- `replace_all=true`: `replace(old, new)` — all occurrences
- Tracks write via `session.track_write(file_path)`
- Returns `"No match found"` if old_string not present

#### `spf_bash` (Lines ~1590-1604)
- Wraps command in: `timeout --signal=KILL {timeout} bash -c "{command}"`
- Default timeout: 30 seconds
- stderr captured and appended to output
- Tracks action as `("Bash", "executed", Some(command))`
- On failure: records to session failures list

#### `spf_glob` (Lines 1606-1667)
- **Path validation**: Canonicalizes search path, rejects `..` traversal
- **Path boundary check**: `is_path_allowed()` AND `!is_path_blocked()`
- Executes: `find {path} -name {pattern}` (safe: no shell interpolation)
- Results capped at **100 lines** (in-process truncation, not piped)
- stderr directed to `/dev/null` via `Stdio::null()`

#### `spf_grep` (Lines 1669-1741)
- **Path validation**: Same as glob — canonicalize + allowed/blocked check
- Executes: `rg` (ripgrep) with flags:
  - `-i` if case_insensitive
  - `-C {n}` for context lines
  - `--glob {filter}` for file type filtering
  - `--` separator prevents pattern-as-flag injection
- Results capped at **500 lines**
- stderr directed to `/dev/null` via `Stdio::null()`

#### `spf_web_fetch` (Lines 1743-1789)
- Calls `WebClient::read_page(url)` → returns (text, raw_len, content_type)
- **Content truncation**: 50,000 characters max
- Output format: `Fetched {url} ({bytes}, {type})\nPrompt: {prompt}\n\n{text}`

#### `spf_web_search` (Lines 1791-1836)
- Calls `WebClient::search(query, count)` → returns (engine_name, results_vec)
- Output: numbered list with title, URL, description per result

#### `spf_web_download` (Lines 1838-1884)
- Calls `WebClient::download(url, save_path)` → returns (size, content_type)
- **Tracks write**: `session.track_write(save_path)` → affects Build Anchor

#### `spf_web_api` (Lines 1886-1934)
- Calls `WebClient::api_request(method, url, headers, body)`
- Returns: HTTP status, response headers, response body (50KB truncation)
- Supports: GET, POST, PUT, DELETE, PATCH, HEAD

#### `spf_notebook_edit` (Lines 1936-2019)
- Reads notebook JSON → parses cells array
- Three modes:
  - **replace**: Overwrites `cells[cell_number].source` and `cell_type`
  - **insert**: Inserts new cell at `cell_number` with metadata and empty outputs
  - **delete**: Removes `cells[cell_number]`
- Writes back with `serde_json::to_string_pretty()`
- Tracks write via `session.track_write()`

#### `spf_brain_*` Tools (Lines 2021-2258)
All 9 brain tools follow identical pattern:
1. Extract params → gate check → record action
2. Call `run_brain(&[subcommand, ...args])`
3. Return success/failure message

**Brain binary subcommands used**:
| MCP Tool | Brain Subcommand | Extra Flags |
|----------|-----------------|-------------|
| `brain_search` | `search {query}` | `--top-k {limit} [--collection {c}]` |
| `brain_store` | `store {text}` | `--title {t} --collection {c} --index [--tags {t}]` |
| `brain_context` | `context {query}` | `--max-tokens {n}` |
| `brain_index` | `index {path}` | *(none)* |
| `brain_list` | `list` | *(none)* |
| `brain_status` | `list` + storage dir scan | Reports binary path, collections, storage MB |
| `brain_recall` | `recall {query}` | `-c {collection}` |
| `brain_list_docs` | `list-docs` | `-c {collection}` |
| `brain_get_doc` | `get-doc {doc_id}` | `-c {collection}` |

#### `spf_rag_*` Tools (Lines 2260-2659)
All 16 RAG tools follow identical pattern:
1. Extract params → gate check → record action
2. Call `run_rag(&[subcommand, ...args])` **OR** `run_brain()` for some operations

**RAG/Brain subcommand routing**:
| MCP Tool | Executor | Subcommand |
|----------|----------|------------|
| `rag_collect_web` | `run_rag` | `collect [--topic {t}]` |
| `rag_collect_file` | `run_rag` | `collect --path {p}` |
| `rag_collect_folder` | `run_rag` | `collect --path {p}` |
| `rag_collect_drop` | `run_rag` | `drop` |
| `rag_index_gathered` | `run_rag` | `index [--category {c}]` |
| `rag_dedupe` | **`run_brain`** | `dedup -c {category}` |
| `rag_status` | `run_rag` | `status` |
| `rag_list_gathered` | `run_rag` | `list-gathered [--category {c}]` |
| `rag_bandwidth_status` | `run_rag` | `bandwidth` |
| `rag_fetch_url` | `run_rag` | `collect --path {url}` |
| `rag_collect_rss` | `run_rag` | `rss [--feed {name}]` |
| `rag_list_feeds` | **Direct file read** | Reads `sources/rss_sources.json` |
| `rag_pending_searches` | **`run_brain`** | `pending-searches -c {c} -f json` |
| `rag_fulfill_search` | **`run_brain`** | `fulfill-search {id} -c {c}` |
| `rag_smart_search` | **`run_brain`** | `smart-search {q} -c {c} -f json` |
| `rag_auto_fetch_gaps` | **`run_brain`** | `auto-fetch -c {c} --max {n}` |

**Key design note**: `rag_dedupe`, `rag_pending_searches`, `rag_fulfill_search`, `rag_smart_search`, and `rag_auto_fetch_gaps` route through the **Brain binary**, not the RAG Collector. The Brain owns the vector DB; RAG handles document collection.

#### Config DB Tools (Lines 2661-2722)
- `spf_config_get`/`spf_config_set` → **HARD BLOCKED** (user-only)
- `spf_config_paths` → `config_db.list_path_rules()` → `type: path` format
- `spf_config_stats` → `config_db.stats()` → (config_count, paths_count, patterns_count)

#### Projects DB Tools (Lines 2724-2854)
- `spf_projects_list` → `projects_db.list_all()` → `key: value` format
- `spf_projects_get` → `projects_db.get(key)` → single entry
- `spf_projects_set` → `projects_db.set(key, value)` → write
- `spf_projects_delete` → `projects_db.delete(key)` → returns true/false
- `spf_projects_stats` → `projects_db.db_stats()` → entry count

#### TMP DB Tools (Lines 2856-2984)
- `spf_tmp_list` → `tmp_db.list_projects()` → name, path, trust, reads, writes, active
- `spf_tmp_stats` → `tmp_db.db_stats()` → (projects, access_log, resources) counts
- `spf_tmp_get` → `tmp_db.get_project(path)` → **full 18-field display** including:
  trust level, active status, reads/writes, session writes/max, write size limit,
  total complexity, protected paths, timestamps, notes
- `spf_tmp_active` → `tmp_db.get_active()` → active project name, path, trust, counts

#### Agent State Tools (Lines 2986-3152)
- `spf_agent_remember`/`spf_agent_forget`/`spf_agent_set_state` → **HARD BLOCKED** (user-only)
- `spf_agent_stats` → `agent_db.db_stats()` → (memory, sessions, state, tags) counts
- `spf_agent_memory_search` → `agent_db.search_memories(query, limit)` → formatted entries
- `spf_agent_memory_by_tag` → `agent_db.get_by_tag(tag)` → filtered entries
- `spf_agent_session_info` → `agent_db.get_latest_session()` → session_id, parent, times,
  working_dir, project, files_modified count, complexity, actions, summary
- `spf_agent_context` → `agent_db.get_context_summary()` → plain text summary

---

## 11.7 MAIN RUN LOOP — `run()` (Lines 3358-3516)

### 11.7.1 Boot Sequence

```
1. Log server name + version
2. Log enforcement mode (Soft/Max)
3. Accept CONFIG.DB from main.rs (single open, single source of truth)
4. Open PROJECTS.DB at LIVE/PROJECTS/PROJECTS.DB (20MB map)
   → Call init_defaults() on success
5. Open TMP.DB at LIVE/TMP/TMP.DB (50MB map)
6. Open LMDB5.DB at LIVE/LMDB5/LMDB5.DB (100MB map)
   → Call init_defaults() on success
7. Open SPF_FS at LIVE/SPF_FS/ (4GB map)
8. Enter stdin read loop
```

**LMDB open order**: CONFIG (passed in) → PROJECTS → TMP → AGENT_STATE → SPF_FS
All opens are graceful — failure logs a warning and sets `None` (tools return "not initialized").

### 11.7.2 Message Loop

Reads line-by-line from stdin (JSON-RPC 2.0):
- Empty lines: skipped
- JSON parse errors: logged, skipped
- Extracts: `method`, `id`, `params`

### 11.7.3 Method Dispatch

| Method | Response |
|--------|----------|
| `initialize` | Protocol version, capabilities (`tools: {}`), server info |
| `notifications/initialized` | No response (notification) |
| `tools/list` | All 55 tool definitions from `tool_definitions()` |
| `tools/call` | Logs to `cmd_log` → `handle_tool_call()` → logs failures → response |
| `ping` | Empty `{}` response |
| Unknown | Error `-32601` if id present (not a notification) |

### 11.7.4 Tool Call Flow in run()

```
1. Extract name + arguments from params
2. cmd_log("CALL {name} | {param_summary}")
3. result = handle_tool_call(name, args, config, session, storage,
                              config_db, projects_db, tmp_db, fs_db, agent_db)
4. Check result text for "ERROR" or "BLOCKED" prefix
   → If failure: cmd_log("FAIL {name} | {first_200_chars}")
5. send_response(id, { "content": [result] })
```

**All 6 LMDB databases** are passed to every `handle_tool_call` invocation as `Option<Db>` references.

---

## 11.8 EXTERNAL BINARY INTEGRATION

### 11.8.1 Brain Binary (Rust)
- **Path**: `$HOME/stoneshell-brain/target/release/brain`
- **Model**: `all-MiniLM-L6-v2` (sentence transformers)
- **Storage**: `$HOME/stoneshell-brain/storage/`
- **Always injected flags**: `-m {model_path} -s {storage_path}`
- **Used by**: 9 brain tools + 5 RAG tools (dedup, pending-searches, fulfill-search, smart-search, auto-fetch)

### 11.8.2 RAG Collector (Python)
- **Interpreter**: `python3 -u` (unbuffered)
- **Path resolution**: `SPF_RAG_PATH` env → `LIVE/BIN/rag-collector/server.py` → legacy Android path
- **Working directory**: Parent of script path
- **Used by**: 11 RAG tools (collect, drop, index, status, list-gathered, bandwidth, fetch, rss)

### 11.8.3 System Binaries
- **`find`**: Used by `spf_glob` — safe argument passing (no shell interpolation)
- **`rg` (ripgrep)**: Used by `spf_grep` — safe argument passing with `--` separator
- **`timeout`**: Used by `spf_bash` — wraps commands with `--signal=KILL {seconds}`
- **`bash`**: Used by `spf_bash` — `bash -c "{command}"`

---

## 11.9 AUDIT TRAIL

Every tool call generates multiple audit records:

1. **cmd.log** (persistent file): `[timestamp] CALL/FAIL {name} | {summary}`
2. **session.manifest** (LMDB): `(tool, C, status, notes)` — max 200 entries
3. **session.actions**: Category-level action tracking
4. **session.files_read / files_written**: Deduplicated canonical path sets
5. **session.failures**: Error records — max 50 entries
6. **TMP_DB access_log**: For `/tmp/` and `/projects/` file operations

---

## 11.10 SECURITY SUMMARY

| Layer | Enforcement |
|-------|-------------|
| **Tool Registry** | 55 tools exposed; 13 handlers exist but are NOT in registry |
| **Gate Pipeline** | Every handler calls `gate::process()` FIRST — no bypass possible |
| **Path Validation** | Glob/grep canonicalize paths + check allowed/blocked boundaries |
| **Shell Safety** | No shell interpolation — all commands use direct arg passing |
| **Content Truncation** | Web fetch: 50KB; Read: 50K chars; Glob: 100 lines; Grep: 500 lines |
| **Write Tracking** | All writes tracked in session for Build Anchor enforcement |
| **Device I/O Logging** | TMP_DB logs all /tmp/ and /projects/ reads and writes |
| **Default Deny** | Unknown tool names → explicit rejection message |
| **User-Only Operations** | Config get/set, agent remember/forget/set_state → HARD BLOCKED from AI |
| **FS Tools** | Removed from registry AND hard-blocked in gate.rs (double defense) |
