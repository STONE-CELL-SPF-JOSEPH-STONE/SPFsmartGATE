// SPF Smart Gateway - MCP Server (JSON-RPC 2.0 over stdio)
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// ALL tool calls route through this gateway.
// Exposes: spf_read, spf_write, spf_edit, spf_bash, spf_status,
//          spf_calculate, spf_session, spf_brain_search, spf_brain_store

use crate::calculate::{self, ToolParams};
use crate::config::SpfConfig;
use crate::config_db::SpfConfigDb;
use crate::paths::{spf_root, actual_home};
use crate::projects_db::SpfProjectsDb;
use crate::tmp_db::SpfTmpDb;
use crate::agent_state::AgentStateDb;
use crate::fs::SpfFs;
use crate::gate;
use crate::session::Session;
use crate::storage::SpfStorage;
use crate::web::WebClient;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::process::Command;
use std::path::PathBuf;
use chrono::{DateTime, Local, Utc};
use std::fs::OpenOptions;

const PROTOCOL_VERSION: &str = "2024-11-05";

/// Format Unix timestamp as human-readable ISO8601
fn format_timestamp(ts: u64) -> String {
    if ts == 0 {
        return "Never".to_string();
    }
    DateTime::<Utc>::from_timestamp(ts as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| ts.to_string())
}
const SERVER_NAME: &str = "spf-smart-gate";
const SERVER_VERSION: &str = "2.0.0";

/// Brain binary path
fn brain_path() -> PathBuf {
    actual_home().join("stoneshell-brain/target/release/brain")
}

/// Run brain CLI command with model and storage paths
fn run_brain(args: &[&str]) -> (bool, String) {
    let brain = brain_path();
    if !brain.exists() {
        return (false, format!("Brain not found: {:?}", brain));
    }
    let brain_root = actual_home().join("stoneshell-brain");
    let model_path = brain_root.join("models/all-MiniLM-L6-v2");
    let storage_dir = brain_root.join("storage");
    let model_str = model_path.to_string_lossy().to_string();
    let storage_str = storage_dir.to_string_lossy().to_string();
    let mut full_args: Vec<&str> = vec!["-m", &model_str, "-s", &storage_str];
    full_args.extend_from_slice(args);
    match Command::new(&brain)
        .args(&full_args)
        .current_dir(&brain_root)
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                (true, String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                (false, String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        Err(e) => (false, format!("Failed to run brain: {}", e)),
    }
}

/// RAG Collector script path — checks SPF_RAG_PATH env, then LIVE/BIN convention
fn rag_collector_path() -> PathBuf {
    if let Ok(p) = std::env::var("SPF_RAG_PATH") {
        return PathBuf::from(p);
    }
    let conventional = spf_root().join("LIVE/BIN/rag-collector/server.py");
    if conventional.exists() {
        return conventional;
    }
    // Legacy Android path
    PathBuf::from("/storage/emulated/0/Download/api-workspace/projects/MCP_RAG_COLLECTOR/server.py")
}

/// RAG Collector working directory — derived from script path parent
fn rag_collector_dir() -> PathBuf {
    rag_collector_path().parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf()
}

/// Run RAG Collector command
fn run_rag(args: &[&str]) -> (bool, String) {
    let rag = rag_collector_path();
    if !rag.exists() {
        return (false, format!("RAG Collector not found: {:?}", rag));
    }
    match Command::new("python3")
        .arg("-u")
        .arg(&rag)
        .args(args)
        .current_dir(rag_collector_dir())
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                (true, String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                (false, format!("{}\n{}", stdout, stderr))
            }
        }
        Err(e) => (false, format!("Failed to run RAG Collector: {}", e)),
    }
}

/// Log to stderr (stdout is JSON-RPC)
fn log(msg: &str) {
    eprintln!("[spf-smart-gate] {}", msg);
}

/// Persistent command log → LIVE/SESSION/cmd.log
fn cmd_log(msg: &str) {
    let log_path = spf_root().join("LIVE/SESSION/cmd.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let ts = Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(f, "[{}] {}", ts, msg);
    }
}

/// Summarize tool params for logging (truncate large values)
fn param_summary(name: &str, args: &Value) -> String {
    match name {
        n if n.contains("bash") => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            if cmd.len() > 200 { format!("cmd={}…", &cmd[..200]) } else { format!("cmd={}", cmd) }
        }
        n if n.contains("read") || n.contains("edit") || n.contains("glob") => {
            let path = args.get("file_path")
                .or_else(|| args.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let pattern = args.get("pattern").and_then(|v| v.as_str());
            match pattern {
                Some(pat) => format!("path={} pattern={}", path, pat),
                None => format!("path={}", path),
            }
        }
        n if n.contains("write") => {
            let path = args.get("file_path")
                .or_else(|| args.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let size = args.get("content").and_then(|v| v.as_str()).map(|s| s.len()).unwrap_or(0);
            format!("path={} content_len={}", path, size)
        }
        n if n.contains("grep") => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            format!("pattern={} path={}", pattern, path)
        }
        n if n.contains("web") => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("?");
            let query = args.get("query").and_then(|v| v.as_str());
            match query {
                Some(q) => format!("query={}", q),
                None => format!("url={}", url),
            }
        }
        n if n.contains("brain") || n.contains("rag") => {
            let query = args.get("query")
                .or_else(|| args.get("text"))
                .or_else(|| args.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let truncated = if query.len() > 150 { &query[..150] } else { query };
            format!("q={}", truncated)
        }
        _ => {
            let s = args.to_string();
            if s.len() > 300 { format!("{}…", &s[..300]) } else { s }
        }
    }
}

/// Send JSON-RPC response
fn send_response(id: &Value, result: Value) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    });
    let msg = serde_json::to_string(&response).unwrap();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(msg.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// Send JSON-RPC error response
fn send_error(id: &Value, code: i64, message: &str) {
    let response = json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    });
    let msg = serde_json::to_string(&response).unwrap();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(msg.as_bytes());
    let _ = out.write_all(b"\n");
    let _ = out.flush();
}

/// MCP tool definition helper
fn tool_def(name: &str, description: &str, properties: Value, required: Vec<&str>) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        }
    })
}

/// Return all tool definitions
fn tool_definitions() -> Vec<Value> {
    vec![
        // ====== CORE GATE TOOLS ======
        // spf_gate REMOVED — was a bypass vector. Gate is internal only.
        tool_def(
            "spf_calculate",
            "Calculate complexity score for a tool call without executing. Returns C value, tier, and allocation.",
            json!({
                "tool": {"type": "string", "description": "Tool name"},
                "params": {"type": "object", "description": "Tool parameters"}
            }),
            vec!["tool", "params"],
        ),
        tool_def(
            "spf_status",
            "Get current SPF gateway status: session metrics, enforcement mode, complexity budget.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_session",
            "Get full session state: files read/written, action history, anchor ratio, complexity history.",
            json!({}),
            vec![],
        ),

        // ====== GATED FILE OPERATIONS ======
        tool_def(
            "spf_read",
            "Read a file through SPF gateway. Tracks read for Build Anchor Protocol.",
            json!({
                "file_path": {"type": "string", "description": "Absolute path to file"},
                "limit": {"type": "integer", "description": "Max lines to read (optional)"},
                "offset": {"type": "integer", "description": "Line offset to start from (optional)"}
            }),
            vec!["file_path"],
        ),
        tool_def(
            "spf_write",
            "Write a file through SPF gateway. Validates: Build Anchor, blocked paths, file size.",
            json!({
                "file_path": {"type": "string", "description": "Absolute path to file"},
                "content": {"type": "string", "description": "File content to write"}
            }),
            vec!["file_path", "content"],
        ),
        tool_def(
            "spf_edit",
            "Edit a file through SPF gateway. Validates: Build Anchor, blocked paths, change size.",
            json!({
                "file_path": {"type": "string", "description": "Absolute path to file"},
                "old_string": {"type": "string", "description": "Text to replace"},
                "new_string": {"type": "string", "description": "Replacement text"},
                "replace_all": {"type": "boolean", "description": "Replace all occurrences", "default": false}
            }),
            vec!["file_path", "old_string", "new_string"],
        ),
        tool_def(
            "spf_bash",
            "Execute a bash command through SPF gateway. Validates: dangerous commands, /tmp access, git force.",
            json!({
                "command": {"type": "string", "description": "Bash command to execute"},
                "timeout": {"type": "integer", "description": "Timeout in seconds (default: 30)", "default": 30}
            }),
            vec!["command"],
        ),

        // ====== SEARCH/GLOB TOOLS ======
        tool_def(
            "spf_glob",
            "Fast file pattern matching. Supports glob patterns like **/*.rs or src/**/*.ts.",
            json!({
                "pattern": {"type": "string", "description": "Glob pattern to match files"},
                "path": {"type": "string", "description": "Directory to search in (default: current dir)"}
            }),
            vec!["pattern"],
        ),
        tool_def(
            "spf_grep",
            "Search file contents using regex. Built on ripgrep.",
            json!({
                "pattern": {"type": "string", "description": "Regex pattern to search for"},
                "path": {"type": "string", "description": "File or directory to search"},
                "glob": {"type": "string", "description": "Glob filter (e.g. *.rs)"},
                "case_insensitive": {"type": "boolean", "description": "Case insensitive search", "default": true},
                "context_lines": {"type": "integer", "description": "Lines of context around matches", "default": 0}
            }),
            vec!["pattern"],
        ),

        // ====== WEB BROWSER TOOLS ======
        tool_def(
            "spf_web_search",
            "Search the web for information. Uses Brave API if BRAVE_API_KEY set, otherwise DuckDuckGo.",
            json!({
                "query": {"type": "string", "description": "Search query"},
                "count": {"type": "integer", "description": "Max results (default: 10)", "default": 10}
            }),
            vec!["query"],
        ),
        tool_def(
            "spf_web_fetch",
            "Fetch a URL and return clean readable text. HTML is converted to plain text, JSON is pretty-printed.",
            json!({
                "url": {"type": "string", "description": "URL to fetch"},
                "prompt": {"type": "string", "description": "Prompt to run on fetched content"}
            }),
            vec!["url", "prompt"],
        ),
        tool_def(
            "spf_web_download",
            "Download a file from URL and save to disk.",
            json!({
                "url": {"type": "string", "description": "URL to download"},
                "save_path": {"type": "string", "description": "Local path to save file"}
            }),
            vec!["url", "save_path"],
        ),
        tool_def(
            "spf_web_api",
            "Make an API request. Returns status, headers, and response body.",
            json!({
                "method": {"type": "string", "description": "HTTP method (GET, POST, PUT, DELETE, PATCH)"},
                "url": {"type": "string", "description": "API endpoint URL"},
                "headers": {"type": "string", "description": "JSON object of headers (optional)", "default": ""},
                "body": {"type": "string", "description": "Request body JSON (optional)", "default": ""}
            }),
            vec!["method", "url"],
        ),

        // ====== NOTEBOOK TOOL ======
        tool_def(
            "spf_notebook_edit",
            "Edit a Jupyter notebook cell.",
            json!({
                "notebook_path": {"type": "string", "description": "Absolute path to .ipynb file"},
                "cell_number": {"type": "integer", "description": "Cell index (0-based)"},
                "new_source": {"type": "string", "description": "New cell content"},
                "cell_type": {"type": "string", "description": "Cell type: code or markdown"},
                "edit_mode": {"type": "string", "description": "Mode: replace, insert, or delete", "default": "replace"}
            }),
            vec!["notebook_path", "new_source"],
        ),

        // ====== BRAIN PASSTHROUGH ======
        tool_def(
            "spf_brain_search",
            "Search brain through SPF gateway. All brain access is logged and tracked.",
            json!({
                "query": {"type": "string", "description": "Search query"},
                "collection": {"type": "string", "description": "Collection (default: default)", "default": "default"},
                "limit": {"type": "integer", "description": "Max results (default: 5)", "default": 5}
            }),
            vec!["query"],
        ),
        tool_def(
            "spf_brain_store",
            "Store document in brain through SPF gateway.",
            json!({
                "text": {"type": "string", "description": "Text to store"},
                "title": {"type": "string", "description": "Document title", "default": "untitled"},
                "collection": {"type": "string", "description": "Collection", "default": "default"},
                "tags": {"type": "string", "description": "Comma-separated tags", "default": ""}
            }),
            vec!["text"],
        ),

        // ====== ADDITIONAL BRAIN TOOLS ======
        tool_def(
            "spf_brain_context",
            "Get relevant context for a query. Returns formatted context for prompt injection.",
            json!({
                "query": {"type": "string", "description": "Query to get context for"},
                "max_tokens": {"type": "integer", "description": "Max tokens (default: 2000)", "default": 2000}
            }),
            vec!["query"],
        ),
        tool_def(
            "spf_brain_index",
            "Index a file or directory into the brain.",
            json!({
                "path": {"type": "string", "description": "File or directory to index"}
            }),
            vec!["path"],
        ),
        tool_def(
            "spf_brain_list",
            "List all indexed collections and document counts.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_brain_status",
            "Get brain system status.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_brain_recall",
            "Search and return full parent documents. Searches vectors then resolves to complete stored document.",
            json!({
                "query": {"type": "string", "description": "Natural language search query"},
                "collection": {"type": "string", "description": "Collection to search (default: default)", "default": "default"}
            }),
            vec!["query"],
        ),
        tool_def(
            "spf_brain_list_docs",
            "List all stored documents in a collection.",
            json!({
                "collection": {"type": "string", "description": "Collection name (default: default)", "default": "default"}
            }),
            vec![],
        ),
        tool_def(
            "spf_brain_get_doc",
            "Retrieve a specific document by its ID.",
            json!({
                "doc_id": {"type": "string", "description": "Document ID to retrieve"},
                "collection": {"type": "string", "description": "Collection name (default: default)", "default": "default"}
            }),
            vec!["doc_id"],
        ),

        // ====== RAG COLLECTOR TOOLS ======
        tool_def(
            "spf_rag_collect_web",
            "Search web and collect documents. Optional topic filter.",
            json!({
                "topic": {"type": "string", "description": "Topic to search (optional)"},
                "auto_index": {"type": "boolean", "description": "Auto-index collected docs", "default": true}
            }),
            vec![],
        ),
        tool_def(
            "spf_rag_collect_file",
            "Process a local file.",
            json!({
                "path": {"type": "string", "description": "File path"},
                "category": {"type": "string", "description": "Category (default: auto)", "default": "auto"}
            }),
            vec!["path"],
        ),
        tool_def(
            "spf_rag_collect_folder",
            "Process all files in a folder.",
            json!({
                "path": {"type": "string", "description": "Folder path"},
                "extensions": {"type": "array", "items": {"type": "string"}, "description": "File extensions to include"}
            }),
            vec!["path"],
        ),
        tool_def(
            "spf_rag_collect_drop",
            "Process files in DROP_HERE folder.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_rag_index_gathered",
            "Index all documents in GATHERED to brain.",
            json!({
                "category": {"type": "string", "description": "Category to index (optional)"}
            }),
            vec![],
        ),
        tool_def(
            "spf_rag_dedupe",
            "Deduplicate brain collection.",
            json!({
                "category": {"type": "string", "description": "Category to dedupe"}
            }),
            vec!["category"],
        ),
        tool_def(
            "spf_rag_status",
            "Get collector status and stats.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_rag_list_gathered",
            "List documents in GATHERED folder.",
            json!({
                "category": {"type": "string", "description": "Filter by category"}
            }),
            vec![],
        ),
        tool_def(
            "spf_rag_bandwidth_status",
            "Get bandwidth usage stats and limits.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_rag_fetch_url",
            "Fetch a single URL with bandwidth limiting.",
            json!({
                "url": {"type": "string", "description": "URL to fetch"},
                "auto_index": {"type": "boolean", "description": "Auto-index after fetch", "default": true}
            }),
            vec!["url"],
        ),
        tool_def(
            "spf_rag_collect_rss",
            "Collect from RSS/Atom feeds.",
            json!({
                "feed_name": {"type": "string", "description": "Specific feed name (optional)"},
                "auto_index": {"type": "boolean", "description": "Auto-index collected", "default": true}
            }),
            vec![],
        ),
        tool_def(
            "spf_rag_list_feeds",
            "List configured RSS feeds.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_rag_pending_searches",
            "Get pending SearchSeeker vectors from brain (gaps needing fetch).",
            json!({
                "collection": {"type": "string", "description": "Collection to check", "default": "default"}
            }),
            vec![],
        ),
        tool_def(
            "spf_rag_fulfill_search",
            "Mark a SearchSeeker as fulfilled after RAG fetch.",
            json!({
                "seeker_id": {"type": "string", "description": "SearchSeeker ID to fulfill"},
                "collection": {"type": "string", "description": "Collection name", "default": "default"}
            }),
            vec!["seeker_id"],
        ),
        tool_def(
            "spf_rag_smart_search",
            "Run smart search with completeness check - triggers SearchSeeker if <80%.",
            json!({
                "query": {"type": "string", "description": "Search query"},
                "collection": {"type": "string", "description": "Collection to search", "default": "default"}
            }),
            vec!["query"],
        ),
        tool_def(
            "spf_rag_auto_fetch_gaps",
            "Automatically fetch data for all pending SearchSeekers.",
            json!({
                "collection": {"type": "string", "description": "Collection to check", "default": "default"},
                "max_fetches": {"type": "integer", "description": "Max URLs to fetch", "default": 5}
            }),
            vec![],
        ),

        // ====== SPF_CONFIG TOOLS ======
        // NOTE: spf_config_get and spf_config_set removed from MCP - user-only via CLI
        tool_def(
            "spf_config_paths",
            "List all path rules (allowed/blocked) from SPF_CONFIG LMDB.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_config_stats",
            "Get SPF_CONFIG LMDB statistics.",
            json!({}),
            vec![],
        ),

        // ====== PROJECTS_DB TOOLS ======
        tool_def(
            "spf_projects_list",
            "List all entries in the PROJECTS registry.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_projects_get",
            "Get a project entry by key.",
            json!({
                "key": {"type": "string", "description": "Project key to look up"}
            }),
            vec!["key"],
        ),
        tool_def(
            "spf_projects_set",
            "Set a project entry (key-value pair).",
            json!({
                "key": {"type": "string", "description": "Project key"},
                "value": {"type": "string", "description": "Project value (JSON string)"}
            }),
            vec!["key", "value"],
        ),
        tool_def(
            "spf_projects_delete",
            "Delete a project entry by key.",
            json!({
                "key": {"type": "string", "description": "Project key to delete"}
            }),
            vec!["key"],
        ),
        tool_def(
            "spf_projects_stats",
            "Get PROJECTS LMDB statistics.",
            json!({}),
            vec![],
        ),

        // ====== TMP_DB TOOLS ======
        tool_def(
            "spf_tmp_list",
            "List all registered projects with trust levels.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_tmp_stats",
            "Get TMP_DB LMDB statistics (project count, access log count, resource count).",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_tmp_get",
            "Get project info by path.",
            json!({
                "path": {"type": "string", "description": "Project path to look up"}
            }),
            vec!["path"],
        ),
        tool_def(
            "spf_tmp_active",
            "Get the currently active project.",
            json!({}),
            vec![],
        ),

        // ====== AGENT_STATE TOOLS ======
        tool_def(
            "spf_agent_stats",
            "Get AGENT_STATE LMDB statistics (memory count, sessions, state keys, tags).",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_agent_memory_search",
            "Search agent memories by content.",
            json!({
                "query": {"type": "string", "description": "Search query"},
                "limit": {"type": "integer", "description": "Max results (default: 10)"}
            }),
            vec!["query"],
        ),
        tool_def(
            "spf_agent_memory_by_tag",
            "Get agent memories by tag.",
            json!({
                "tag": {"type": "string", "description": "Tag to filter by"}
            }),
            vec!["tag"],
        ),
        tool_def(
            "spf_agent_session_info",
            "Get the most recent session info.",
            json!({}),
            vec![],
        ),
        tool_def(
            "spf_agent_context",
            "Get context summary for session continuity.",
            json!({}),
            vec![],
        ),
        // ====== SPF_FS Tools — REMOVED FROM AI AGENT REGISTRY ======
        // spf_fs_exists, spf_fs_stat, spf_fs_ls, spf_fs_read,
        // spf_fs_write, spf_fs_mkdir, spf_fs_rm, spf_fs_rename
        // These are USER/SYSTEM-ONLY tools. Not exposed to AI agents via MCP.
        // Hard-blocked in gate.rs as additional defense in depth.
    ]
}

// ============================================================================
// LMDB PARTITION ROUTING — virtual filesystem mount points
// ============================================================================

/// Route spf_fs_* calls to the correct LMDB partition based on path prefix.
/// Returns Some(result) if routed, None to fall through to SpfFs (LMDB 1).
fn route_to_lmdb(
    path: &str,
    op: &str,
    content: Option<&str>,
    config_db: &Option<SpfConfigDb>,
    tmp_db: &Option<SpfTmpDb>,
    agent_db: &Option<AgentStateDb>,
) -> Option<Value> {
    let live_base = spf_root().join("LIVE").display().to_string();

    if path == "/config" || path.starts_with("/config/") {
        return Some(route_config(path, op, config_db));
    }
    // /tmp — device-backed directory in LIVE/TMP/TMP/
    if path == "/tmp" || path.starts_with("/tmp/") {
        let device_tmp = format!("{}/TMP/TMP", live_base);
        return Some(route_device_dir(path, "/tmp", &device_tmp, op, content, tmp_db));
    }
    // /projects — device-backed directory in LIVE/PROJECTS/PROJECTS/
    if path == "/projects" || path.starts_with("/projects/") {
        let device_projects = format!("{}/PROJECTS/PROJECTS", live_base);
        return Some(route_device_dir(path, "/projects", &device_projects, op, content, tmp_db));
    }
    // /home/agent/tmp → redirect to /tmp device directory
    if path == "/home/agent/tmp" || path.starts_with("/home/agent/tmp/") {
        let redirected = path.replacen("/home/agent/tmp", "/tmp", 1);
        let device_tmp = format!("{}/TMP/TMP", live_base);
        return Some(route_device_dir(&redirected, "/tmp", &device_tmp, op, content, tmp_db));
    }
    if path == "/home/agent" || path.starts_with("/home/agent/") {
        // Write permission check for /home/agent/* — ALL writes blocked
        if matches!(op, "write" | "mkdir" | "rm" | "rename") {
            return Some(json!({"type": "text", "text": format!("BLOCKED: {} is read-only in /home/agent/", path)}));
        }
        // Read ops route to agent handler
        return Some(route_agent(path, op, agent_db));
    }
    None
}

/// LMDB 2 — SPF_CONFIG mount at /config/
fn route_config(path: &str, op: &str, config_db: &Option<SpfConfigDb>) -> Value {
    let db = match config_db {
        Some(db) => db,
        None => return json!({"type": "text", "text": "SPF_CONFIG LMDB not initialized"}),
    };

    let relative = path.strip_prefix("/config").unwrap_or("").trim_start_matches('/');

    match op {
        "ls" => {
            if relative.is_empty() {
                json!({"type": "text", "text": "/config:\n-644        0 version\n-644        0 mode\n-644        0 tiers\n-644        0 formula\n-644        0 weights\n-644        0 paths\n-644        0 patterns"})
            } else {
                json!({"type": "text", "text": format!("/config/{}: not a directory", relative)})
            }
        }
        "read" => {
            match relative {
                "version" => match db.get("spf", "version") {
                    Ok(Some(v)) => json!({"type": "text", "text": v}),
                    Ok(None) => json!({"type": "text", "text": "not set"}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                },
                "mode" => match db.get_enforce_mode() {
                    Ok(mode) => json!({"type": "text", "text": format!("{:?}", mode)}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                },
                "tiers" => match db.get_tiers() {
                    Ok(tiers) => json!({"type": "text", "text": serde_json::to_string_pretty(&tiers).unwrap_or_else(|e| format!("error: {}", e))}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                },
                "formula" => match db.get_formula() {
                    Ok(formula) => json!({"type": "text", "text": serde_json::to_string_pretty(&formula).unwrap_or_else(|e| format!("error: {}", e))}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                },
                "weights" => match db.get_weights() {
                    Ok(weights) => json!({"type": "text", "text": serde_json::to_string_pretty(&weights).unwrap_or_else(|e| format!("error: {}", e))}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                },
                "paths" => match db.list_path_rules() {
                    Ok(rules) => {
                        let text = rules.iter()
                            .map(|(t, p)| format!("{}: {}", t, p))
                            .collect::<Vec<_>>()
                            .join("\n");
                        json!({"type": "text", "text": if text.is_empty() { "No path rules".to_string() } else { text }})
                    }
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                },
                "patterns" => match db.list_dangerous_patterns() {
                    Ok(patterns) => {
                        let text = patterns.iter()
                            .map(|(p, s)| format!("{} (severity: {})", p, s))
                            .collect::<Vec<_>>()
                            .join("\n");
                        json!({"type": "text", "text": if text.is_empty() { "No patterns".to_string() } else { text }})
                    }
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                },
                "" => json!({"type": "text", "text": "/config is a directory (use ls)"}),
                _ => json!({"type": "text", "text": format!("not found: /config/{}", relative)}),
            }
        }
        "exists" => {
            let exists = relative.is_empty() || matches!(relative, "version" | "mode" | "tiers" | "formula" | "weights" | "paths" | "patterns");
            json!({"type": "text", "text": format!("/config/{}: {}", relative, if exists { "EXISTS" } else { "NOT FOUND" })})
        }
        "stat" => {
            if relative.is_empty() {
                json!({"type": "text", "text": "Path: /config\nType: Directory\nMount: CONFIG (CONFIG.DB)"})
            } else if matches!(relative, "version" | "mode" | "tiers" | "formula" | "weights" | "paths" | "patterns") {
                json!({"type": "text", "text": format!("Path: /config/{}\nType: File\nMount: CONFIG (CONFIG.DB)\nSource: config_db.{}", relative, relative)})
            } else {
                json!({"type": "text", "text": format!("Not found: /config/{}", relative)})
            }
        }
        "write" | "mkdir" | "rm" | "rename" => {
            json!({"type": "text", "text": "BLOCKED: /config is a read-only mount (use spf_config_* tools)"})
        }
        _ => json!({"type": "text", "text": format!("unsupported operation: {}", op)}),
    }
}

/// Device-backed directory mount: files on device disk, OS provides metadata.
/// Used for /tmp/ and /projects/ — real device filesystem, not LMDB blobs.
fn route_device_dir(
    virtual_path: &str,
    mount_prefix: &str,
    device_base: &str,
    op: &str,
    content: Option<&str>,
    tmp_db: &Option<SpfTmpDb>,
) -> Value {
    let relative = virtual_path.strip_prefix(mount_prefix)
        .unwrap_or("")
        .trim_start_matches('/');

    // Path traversal protection — reject any relative path containing ..
    if relative.contains("..") {
        return json!({"type": "text", "text": format!(
            "BLOCKED: path traversal detected in {}", virtual_path
        )});
    }

    let device_path = if relative.is_empty() {
        std::path::PathBuf::from(device_base)
    } else {
        std::path::PathBuf::from(device_base).join(relative)
    };

    match op {
        "ls" => {
            match std::fs::read_dir(&device_path) {
                Ok(entries) => {
                    let mut items: Vec<String> = Vec::new();
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let meta = entry.metadata().ok();
                        let (prefix, size) = match &meta {
                            Some(m) if m.is_dir() => ("d755", 0u64),
                            Some(m) => ("-644", m.len()),
                            None => ("-???", 0u64),
                        };
                        items.push(format!("{} {:>8} {}", prefix, size, name));
                    }
                    items.sort();
                    if items.is_empty() {
                        json!({"type": "text", "text": format!("{}: empty", virtual_path)})
                    } else {
                        json!({"type": "text", "text": format!("{}:\n{}", virtual_path, items.join("\n"))})
                    }
                }
                Err(_) if !device_path.exists() => {
                    json!({"type": "text", "text": format!("{}: empty", virtual_path)})
                }
                Err(e) => {
                    json!({"type": "text", "text": format!("error listing {}: {}", virtual_path, e)})
                }
            }
        }
        "read" => {
            if relative.is_empty() {
                json!({"type": "text", "text": format!("{} is a directory (use ls)", virtual_path)})
            } else {
                match std::fs::read_to_string(&device_path) {
                    Ok(data) => {
                        // Log read to TMP_DB
                        if let Some(db) = tmp_db {
                            let _ = db.log_access(virtual_path, device_base, "read", "device", data.len() as u64, true, None);
                        }
                        json!({"type": "text", "text": data})
                    }
                    Err(e) => json!({"type": "text", "text": format!("error reading {}: {}", virtual_path, e)}),
                }
            }
        }
        "write" => {
            if let Some(data) = content {
                if let Some(parent) = device_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&device_path, data) {
                    Ok(()) => {
                        // Log write to TMP_DB
                        if let Some(db) = tmp_db {
                            let _ = db.log_access(virtual_path, device_base, "write", "device", data.len() as u64, true, None);
                        }
                        json!({"type": "text", "text": format!("Written: {} ({} bytes)", virtual_path, data.len())})
                    }
                    Err(e) => json!({"type": "text", "text": format!("write failed: {}", e)}),
                }
            } else {
                json!({"type": "text", "text": "write requires content"})
            }
        }
        "exists" => {
            let exists = device_path.exists();
            json!({"type": "text", "text": format!("{}: {}", virtual_path, if exists { "EXISTS" } else { "NOT FOUND" })})
        }
        "stat" => {
            match std::fs::metadata(&device_path) {
                Ok(meta) => {
                    let file_type = if meta.is_dir() { "Directory" } else { "File" };
                    json!({"type": "text", "text": format!(
                        "Path: {}\nType: {}\nSize: {}\nMount: device ({})\nAccess: read-write",
                        virtual_path, file_type, meta.len(), device_base
                    )})
                }
                Err(_) => json!({"type": "text", "text": format!("{}: NOT FOUND", virtual_path)}),
            }
        }
        "mkdir" => {
            match std::fs::create_dir_all(&device_path) {
                Ok(()) => json!({"type": "text", "text": format!("Directory created: {}", virtual_path)}),
                Err(e) => json!({"type": "text", "text": format!("mkdir failed: {}", e)}),
            }
        }
        "rm" => {
            if device_path.is_dir() {
                match std::fs::remove_dir(&device_path) {
                    Ok(()) => json!({"type": "text", "text": format!("Removed: {}", virtual_path)}),
                    Err(e) => json!({"type": "text", "text": format!("rm failed (not empty?): {}", e)}),
                }
            } else if device_path.exists() {
                match std::fs::remove_file(&device_path) {
                    Ok(()) => json!({"type": "text", "text": format!("Removed: {}", virtual_path)}),
                    Err(e) => json!({"type": "text", "text": format!("rm failed: {}", e)}),
                }
            } else {
                json!({"type": "text", "text": format!("{}: NOT FOUND", virtual_path)})
            }
        }
        "rename" => {
            // rename needs new_path — handled at spf_fs_rename level
            json!({"type": "text", "text": "rename: use spf_fs_rename with full paths"})
        }
        _ => json!({"type": "text", "text": format!("unsupported operation: {}", op)}),
    }
}

/// LMDB 5 — AGENT_STATE mount at /home/agent/
// ============================================================================
// ROUTE_AGENT REPLACEMENT — Dynamic reads from LMDB5.DB state db
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// REPLACES: lines 1037-1243 in src/mcp.rs
// INSERT: scan_state_dir helper + replacement route_agent function
//
// What changed:
//   1. READ: state db lookup (file:{path} keys) before "not found" catch-all
//   2. LS: skeleton dirs merged with dynamic file: keys from state db
//   3. EXISTS: state db check for file keys and directory prefixes
//   4. State listing filters out file: keys (those belong to LS, not state/)
//   5. New helper: scan_state_dir() scans state keys for directory children
// ============================================================================

/// Scan state db for file: keys that are immediate children of a directory.
/// Returns formatted ls entries like "d755        0 dirname" or "-644        0 filename".
fn scan_state_dir(db: &AgentStateDb, dir_relative: &str) -> Vec<String> {
    let prefix = if dir_relative.is_empty() {
        "file:".to_string()
    } else {
        format!("file:{}/", dir_relative)
    };

    match db.list_state_keys() {
        Ok(keys) => {
            let mut dirs = std::collections::BTreeSet::new();
            let mut files = std::collections::BTreeSet::new();

            for key in &keys {
                if let Some(rest) = key.strip_prefix(&prefix) {
                    if rest.is_empty() { continue; }
                    match rest.find('/') {
                        Some(pos) => { dirs.insert(rest[..pos].to_string()); }
                        None => { files.insert(rest.to_string()); }
                    }
                }
            }

            let mut entries = Vec::new();
            for d in dirs {
                entries.push(format!("d755        0 {}", d));
            }
            for f in files {
                entries.push(format!("-644        0 {}", f));
            }
            entries
        }
        Err(_) => Vec::new(),
    }
}

/// Route /home/agent/* virtual paths to LMDB5 AgentStateDb.
///
/// Three data sources:
/// 1. Skeleton directories (hardcoded structure — defines virtual FS layout)
/// 2. State db file:{path} keys (imported config files — dynamic READ/LS/EXISTS)
/// 3. Dedicated databases (memory, sessions, state, preferences, context)
fn route_agent(path: &str, op: &str, agent_db: &Option<AgentStateDb>) -> Value {
    let db = match agent_db {
        Some(db) => db,
        None => return json!({"type": "text", "text": "AGENT_STATE LMDB not initialized"}),
    };

    let relative = path.strip_prefix("/home/agent").unwrap_or("").trim_start_matches('/');

    match op {
        "ls" => {
            // Special dynamic directories backed by dedicated LMDB databases
            match relative {
                "memory" => {
                    return match db.search_memories("", 100) {
                        Ok(memories) => {
                            let text = memories.iter()
                                .map(|m| format!("-644 {:>8} {}", m.content.len(), m.id))
                                .collect::<Vec<_>>()
                                .join("\n");
                            json!({"type": "text", "text": if text.is_empty() { "/home/agent/memory: empty".to_string() } else { format!("/home/agent/memory:\n{}", text) }})
                        }
                        Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                    };
                }
                "sessions" => {
                    return match db.get_latest_session() {
                        Ok(Some(latest)) => {
                            match db.get_session_chain(&latest.session_id) {
                                Ok(chain) => {
                                    let text = chain.iter()
                                        .map(|s| format!("-644 {:>8} {}", s.total_actions, s.session_id))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    json!({"type": "text", "text": format!("/home/agent/sessions:\n{}", text)})
                                }
                                Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                            }
                        }
                        Ok(None) => json!({"type": "text", "text": "/home/agent/sessions: empty"}),
                        Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                    };
                }
                "state" => {
                    // Show state keys EXCEPT file: keys (those are served via LS of their dirs)
                    return match db.list_state_keys() {
                        Ok(keys) => {
                            let text = keys.iter()
                                .filter(|k| !k.starts_with("file:"))
                                .map(|k| format!("-644        0 {}", k))
                                .collect::<Vec<_>>()
                                .join("\n");
                            json!({"type": "text", "text": if text.is_empty() { "/home/agent/state: empty".to_string() } else { format!("/home/agent/state:\n{}", text) }})
                        }
                        Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                    };
                }
                _ => {}
            }

            // Skeleton directories — hardcoded virtual FS structure
            let skeleton: Vec<&str> = match relative {
                "" => vec![
                    "-644        0 .claude.json",
                    "d755        0 .claude",
                    "d755        0 bin",
                    "d755        0 tmp",
                    "d755        0 .config",
                    "d755        0 .local",
                    "d755        0 .cache",
                    "d755        0 .memory",
                    "d755        0 .ssh",
                    "d755        0 Documents",
                    "d755        0 Projects",
                    "d755        0 workspace",
                    "-644        0 preferences",
                    "-644        0 context",
                ],
                ".claude" => vec![
                    "d755        0 projects",
                    "d755        0 file-history",
                    "d755        0 paste-cache",
                    "d755        0 session-env",
                    "d755        0 todos",
                    "d755        0 plans",
                    "d755        0 tasks",
                    "d755        0 shell-snapshots",
                    "d755        0 statsig",
                    "d755        0 telemetry",
                ],
                "bin" => vec![
                    "-755        0 spf-smart-gate",
                    "d755        0 claude-code",
                ],
                ".config" => vec!["d755        0 settings"],
                ".local" => vec![
                    "d755        0 bin",
                    "d755        0 share",
                    "d755        0 state",
                ],
                ".local/share" => vec![
                    "d755        0 history",
                    "d755        0 data",
                ],
                ".local/state" => vec!["d755        0 sessions"],
                ".cache" => vec![
                    "d755        0 context",
                    "d755        0 tmp",
                ],
                ".memory" => vec![
                    "d755        0 facts",
                    "d755        0 instructions",
                    "d755        0 preferences",
                    "d755        0 pinned",
                ],
                ".ssh" => vec![],
                "Documents" => vec![
                    "d755        0 notes",
                    "d755        0 templates",
                ],
                "Projects" => vec![],
                "workspace" => vec!["d755        0 current"],
                _ => vec![],
            };

            // Scan state db for imported file: keys in this directory
            let dynamic = scan_state_dir(db, relative);

            // Merge skeleton + dynamic (deduplicate by name)
            let mut seen = std::collections::HashSet::new();
            let mut entries = Vec::new();
            for entry in &skeleton {
                let name = entry.split_whitespace().last().unwrap_or("");
                if seen.insert(name.to_string()) {
                    entries.push(entry.to_string());
                }
            }
            for entry in &dynamic {
                let name = entry.split_whitespace().last().unwrap_or("");
                if seen.insert(name.to_string()) {
                    entries.push(entry.clone());
                }
            }

            // Known skeleton dirs (even when empty) + any dir with dynamic entries
            let is_known_dir = !skeleton.is_empty() || !dynamic.is_empty()
                || matches!(relative, "" | ".ssh" | "Projects");

            if !is_known_dir {
                json!({"type": "text", "text": format!("/home/agent/{}: not a directory", relative)})
            } else {
                let dir = if relative.is_empty() {
                    "/home/agent".to_string()
                } else {
                    format!("/home/agent/{}", relative)
                };
                if entries.is_empty() {
                    json!({"type": "text", "text": format!("{}: empty", dir)})
                } else {
                    json!({"type": "text", "text": format!("{}:\n{}", dir, entries.join("\n"))})
                }
            }
        }
        "read" => {
            if relative.is_empty() {
                return json!({"type": "text", "text": "/home/agent is a directory (use ls)"});
            }

            // Dedicated handlers for special virtual files
            if relative == "preferences" {
                return match db.get_preferences() {
                    Ok(prefs) => json!({"type": "text", "text": serde_json::to_string_pretty(&prefs).unwrap_or_else(|e| format!("error: {}", e))}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                };
            }
            if relative == "context" {
                return match db.get_context_summary() {
                    Ok(summary) => json!({"type": "text", "text": if summary.is_empty() { "No context available".to_string() } else { summary }}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                };
            }
            if let Some(mem_id) = relative.strip_prefix("memory/") {
                return match db.recall(mem_id) {
                    Ok(Some(entry)) => json!({"type": "text", "text": format!(
                        "ID: {}\nType: {:?}\nContent: {}\nTags: {}\nSource: {}\nCreated: {}\nAccessed: {} ({}x)\nRelevance: {:.2}",
                        entry.id, entry.memory_type, entry.content,
                        entry.tags.join(", "), entry.source,
                        format_timestamp(entry.created_at), format_timestamp(entry.last_accessed),
                        entry.access_count, entry.relevance
                    )}),
                    Ok(None) => json!({"type": "text", "text": format!("not found: /home/agent/memory/{}", mem_id)}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                };
            }
            if let Some(session_id) = relative.strip_prefix("sessions/") {
                return match db.get_session(session_id) {
                    Ok(Some(ctx)) => json!({"type": "text", "text": format!(
                        "Session: {}\nParent: {}\nStarted: {}\nEnded: {}\nDir: {}\nActions: {}\nComplexity: {}\nFiles modified: {}\nSummary: {}",
                        ctx.session_id,
                        ctx.parent_session.as_deref().unwrap_or("none"),
                        format_timestamp(ctx.started_at), format_timestamp(ctx.ended_at),
                        ctx.working_dir, ctx.total_actions, ctx.total_complexity,
                        ctx.files_modified.join(", "),
                        if ctx.summary.is_empty() { "none" } else { &ctx.summary }
                    )}),
                    Ok(None) => json!({"type": "text", "text": format!("not found: /home/agent/sessions/{}", session_id)}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                };
            }
            if let Some(key) = relative.strip_prefix("state/") {
                return match db.get_state(key) {
                    Ok(Some(value)) => json!({"type": "text", "text": value}),
                    Ok(None) => json!({"type": "text", "text": format!("not found: /home/agent/state/{}", key)}),
                    Err(e) => json!({"type": "text", "text": format!("error: {}", e)}),
                };
            }

            // Dynamic read from state db — imported config files (file:{path} keys)
            let file_key = format!("file:{}", relative);
            match db.get_state(&file_key) {
                Ok(Some(content)) => json!({"type": "text", "text": content}),
                Ok(None) => json!({"type": "text", "text": format!("not found: /home/agent/{}", relative)}),
                Err(e) => json!({"type": "text", "text": format!("error reading {}: {}", relative, e)}),
            }
        }
        "exists" => {
            // Hardcoded skeleton paths always exist
            let hardcoded = matches!(relative,
                "" | "memory" | "sessions" | "state" | "preferences" | "context"
                | ".claude" | ".claude.json" | "bin" | "tmp" | ".config" | ".local"
                | ".cache" | ".memory" | ".ssh" | "Documents" | "Projects" | "workspace"
            )
                || relative.starts_with("memory/")
                || relative.starts_with("sessions/")
                || relative.starts_with("state/");

            if hardcoded {
                return json!({"type": "text", "text": format!("/home/agent/{}: EXISTS", relative)});
            }

            // Check state db for file: key (imported config file)
            let file_key = format!("file:{}", relative);
            let is_file = db.get_state(&file_key).ok().flatten().is_some();

            // Check if it's a directory containing file: keys
            let is_dir = if !is_file {
                let dir_prefix = format!("file:{}/", relative);
                db.list_state_keys().ok()
                    .map(|keys| keys.iter().any(|k| k.starts_with(&dir_prefix)))
                    .unwrap_or(false)
            } else {
                false
            };

            let exists = is_file || is_dir;
            json!({"type": "text", "text": format!("/home/agent/{}: {}",
                relative, if exists { "EXISTS" } else { "NOT FOUND" })})
        }
        "stat" => {
            if relative.is_empty() {
                json!({"type": "text", "text": "Path: /home/agent\nType: Directory\nMount: AGENT_STATE (LMDB5.DB)"})
            } else {
                json!({"type": "text", "text": format!("Path: /home/agent/{}\nMount: AGENT_STATE (LMDB5.DB)", relative)})
            }
        }
        "write" | "mkdir" | "rm" | "rename" => {
            json!({"type": "text", "text": "BLOCKED: /home/agent is a read-only mount (use spf_agent_* tools)"})
        }
        _ => json!({"type": "text", "text": format!("unsupported operation: {}", op)}),
    }
}

/// Handle a tool call
fn handle_tool_call(
    name: &str,
    args: &Value,
    config: &SpfConfig,
    session: &mut Session,
    storage: &SpfStorage,
    config_db: &Option<SpfConfigDb>,
    projects_db: &Option<SpfProjectsDb>,
    tmp_db: &Option<SpfTmpDb>,
    _fs_db: &Option<SpfFs>,
    agent_db: &Option<AgentStateDb>,
) -> Value {
    match name {
        // ====== spf_gate ======
        // spf_gate REMOVED — was a bypass vector
        "spf_gate" => {
            json!({"type": "text", "text": "BLOCKED: spf_gate removed — gate is internal only"})
        }

        // ====== spf_calculate ======
        "spf_calculate" => {
            let tool = args["tool"].as_str().unwrap_or("unknown");
            let params: ToolParams = serde_json::from_value(
                args.get("params").cloned().unwrap_or(json!({}))
            ).unwrap_or_else(|_| ToolParams {
                ..Default::default()
            });
            let gate_params = ToolParams { command: Some(tool.to_string()), ..Default::default() };
            let decision = gate::process("spf_calculate", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_calculate", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            let result = calculate::calculate(tool, &params, config);
            json!({"type": "text", "text": serde_json::to_string_pretty(&result).unwrap()})
        }

        // ====== spf_status ======
        "spf_status" => {
            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_status", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_status", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            let status = format!(
                "SPF Gateway v{}\nMode: {:?}\nSession: {}\nTiers: SIMPLE(<500) LIGHT(<2000) MEDIUM(<10000) CRITICAL(>10000)\nFormula: a_optimal(C) = {} × (1 - 1/ln(C + e))",
                SERVER_VERSION,
                config.enforce_mode,
                session.status_summary(),
                config.formula.w_eff,
            );
            json!({"type": "text", "text": status})
        }

        // ====== spf_session ======
        "spf_session" => {
            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_session", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_session", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            json!({"type": "text", "text": serde_json::to_string_pretty(session).unwrap()})
        }

        // ====== spf_read ======
        "spf_read" => {
            let file_path = args["file_path"].as_str().unwrap_or("");

            let params = ToolParams {
                file_path: Some(file_path.to_string()),
                ..Default::default()
            };

            let decision = gate::process("Read", &params, config, session);
            if !decision.allowed {
                session.record_manifest("Read", decision.complexity.c, "BLOCKED", decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            // Execute read
            match std::fs::read_to_string(file_path) {
                Ok(content) => {
                    session.track_read(file_path);
                    session.record_action("Read", "success", Some(file_path));
                    let _ = storage.save_session(session);

                    // Apply limit/offset if specified
                    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                    let lines: Vec<&str> = content.lines().collect();
                    let total = lines.len();
                    let start = offset.min(total);
                    let end = if limit > 0 { (start + limit).min(total) } else { total };

                    let numbered: String = lines[start..end]
                        .iter()
                        .enumerate()
                        .map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
                        .collect::<Vec<_>>()
                        .join("\n");

                    json!({"type": "text", "text": format!("File: {} ({} lines)\n{}", file_path, total, numbered)})
                }
                Err(e) => {
                    session.record_action("Read", "failed", Some(file_path));
                    session.record_failure("Read", &e.to_string());
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("Read failed: {}", e)})
                }
            }
        }

        // ====== spf_write ======
        "spf_write" => {
            let file_path = args["file_path"].as_str().unwrap_or("");
            let content = args["content"].as_str().unwrap_or("");

            let params = ToolParams {
                file_path: Some(file_path.to_string()),
                content: Some(content.to_string()),
                ..Default::default()
            };

            let decision = gate::process("Write", &params, config, session);
            if !decision.allowed {
                session.record_manifest("Write", decision.complexity.c, "BLOCKED", decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            // Execute write
            // Ensure parent directory exists
            if let Some(parent) = std::path::Path::new(file_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            match std::fs::write(file_path, content) {
                Ok(()) => {
                    session.track_write(file_path);
                    session.record_action("Write", "success", Some(file_path));
                    session.record_manifest("Write", decision.complexity.c, "ALLOWED", None);
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!(
                        "Written: {} ({} bytes) | C={} {}",
                        file_path, content.len(), decision.complexity.c, decision.complexity.tier
                    )})
                }
                Err(e) => {
                    session.record_action("Write", "failed", Some(file_path));
                    session.record_failure("Write", &e.to_string());
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("Write failed: {}", e)})
                }
            }
        }

        // ====== spf_edit ======
        "spf_edit" => {
            let file_path = args["file_path"].as_str().unwrap_or("");
            let old_string = args["old_string"].as_str().unwrap_or("");
            let new_string = args["new_string"].as_str().unwrap_or("");
            let replace_all = args["replace_all"].as_bool().unwrap_or(false);

            let params = ToolParams {
                file_path: Some(file_path.to_string()),
                old_string: Some(old_string.to_string()),
                new_string: Some(new_string.to_string()),
                replace_all: Some(replace_all),
                ..Default::default()
            };

            let decision = gate::process("Edit", &params, config, session);
            if !decision.allowed {
                session.record_manifest("Edit", decision.complexity.c, "BLOCKED", decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            // Execute edit
            match std::fs::read_to_string(file_path) {
                Ok(content) => {
                    let new_content = if replace_all {
                        content.replace(old_string, new_string)
                    } else {
                        content.replacen(old_string, new_string, 1)
                    };

                    if new_content == content {
                        json!({"type": "text", "text": format!("Edit: old_string not found in {}", file_path)})
                    } else {
                        match std::fs::write(file_path, &new_content) {
                            Ok(()) => {
                                session.track_write(file_path);
                                session.record_action("Edit", "success", Some(file_path));
                                session.record_manifest("Edit", decision.complexity.c, "ALLOWED", None);
                                let _ = storage.save_session(session);
                                json!({"type": "text", "text": format!(
                                    "Edited: {} | C={} {}",
                                    file_path, decision.complexity.c, decision.complexity.tier
                                )})
                            }
                            Err(e) => {
                                session.record_failure("Edit", &e.to_string());
                                let _ = storage.save_session(session);
                                json!({"type": "text", "text": format!("Edit write failed: {}", e)})
                            }
                        }
                    }
                }
                Err(e) => {
                    session.record_failure("Edit", &e.to_string());
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("Edit read failed: {}", e)})
                }
            }
        }

        // ====== spf_bash ======
        "spf_bash" => {
            let command = args["command"].as_str().unwrap_or("");
            let timeout_secs = args["timeout"].as_u64().unwrap_or(30).min(300);

            let params = ToolParams {
                command: Some(command.to_string()),
                ..Default::default()
            };

            let decision = gate::process("Bash", &params, config, session);
            if !decision.allowed {
                session.record_manifest("Bash", decision.complexity.c, "BLOCKED", decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            // Execute bash with timeout enforcement
            let output_result = Command::new("timeout")
                .arg("--signal=KILL")
                .arg(format!("{}s", timeout_secs))
                .arg("bash")
                .arg("-c")
                .arg(command)
                .output()
                .or_else(|_| {
                    // timeout binary not found — fall back to direct execution
                    Command::new("bash")
                        .arg("-c")
                        .arg(command)
                        .output()
                });
            match output_result {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let success = output.status.success();

                    session.record_action("Bash", if success { "success" } else { "failed" }, None);
                    if !success {
                        session.record_failure("Bash", &stderr);
                    }
                    session.record_manifest("Bash", decision.complexity.c, "ALLOWED", None);
                    let _ = storage.save_session(session);

                    let mut result = String::new();
                    if !stdout.is_empty() {
                        result.push_str(&stdout);
                    }
                    if !stderr.is_empty() {
                        result.push_str("\nSTDERR: ");
                        result.push_str(&stderr);
                    }
                    if result.is_empty() {
                        result = format!("Exit code: {}", output.status.code().unwrap_or(-1));
                    }

                    json!({"type": "text", "text": result})
                }
                Err(e) => {
                    session.record_failure("Bash", &e.to_string());
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("Bash failed: {}", e)})
                }
            }
        }

        // ====== spf_glob ======
        "spf_glob" => {
            let pattern = args["pattern"].as_str().unwrap_or("");
            let path = args["path"].as_str().unwrap_or(".");

            let gate_params = ToolParams { command: Some(pattern.to_string()), file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_glob", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_glob", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("Glob", "called", None);

            // Validate search path is within allowed boundaries
            let search_path = match std::fs::canonicalize(path) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => {
                    if path.contains("..") {
                        return json!({"type": "text", "text": "BLOCKED: path traversal detected in search path"});
                    }
                    path.to_string()
                }
            };

            if !config.is_path_allowed(&search_path) || config.is_path_blocked(&search_path) {
                session.record_manifest("spf_glob", decision.complexity.c, "BLOCKED",
                    Some("Search path outside allowed boundaries"));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!(
                    "BLOCKED: glob search path '{}' is outside allowed paths", path
                )});
            }

            // Safe: arguments passed directly, no shell interpolation
            match Command::new("find")
                .arg(path)
                .arg("-name")
                .arg(pattern)
                .stderr(std::process::Stdio::null())
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Limit to first 100 results (replaces piped head -100)
                    let truncated: String = stdout.lines().take(100).collect::<Vec<_>>().join("\n");
                    let _ = storage.save_session(session);
                    if truncated.is_empty() {
                        json!({"type": "text", "text": "No matches found"})
                    } else {
                        json!({"type": "text", "text": truncated})
                    }
                }
                Err(e) => {
                    session.record_failure("Glob", &e.to_string());
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("Glob failed: {}", e)})
                }
            }
        }

        // ====== spf_grep ======
        "spf_grep" => {
            let pattern = args["pattern"].as_str().unwrap_or("");
            let path = args["path"].as_str().unwrap_or(".");
            let glob_filter = args["glob"].as_str().unwrap_or("");
            let case_insensitive = args["case_insensitive"].as_bool().unwrap_or(false);
            let context = args["context_lines"].as_u64().unwrap_or(0);

            let gate_params = ToolParams { command: Some(pattern.to_string()), file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_grep", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_grep", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("Grep", "called", None);

            // Validate search path is within allowed boundaries
            let search_path = match std::fs::canonicalize(path) {
                Ok(p) => p.to_string_lossy().to_string(),
                Err(_) => {
                    if path.contains("..") {
                        return json!({"type": "text", "text": "BLOCKED: path traversal detected in search path"});
                    }
                    path.to_string()
                }
            };

            if !config.is_path_allowed(&search_path) || config.is_path_blocked(&search_path) {
                session.record_manifest("spf_grep", decision.complexity.c, "BLOCKED",
                    Some("Search path outside allowed boundaries"));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!(
                    "BLOCKED: grep search path '{}' is outside allowed paths", path
                )});
            }

            // Safe: arguments passed directly, no shell interpolation
            let mut rg = Command::new("rg");
            if case_insensitive {
                rg.arg("-i");
            }
            if context > 0 {
                rg.arg("-C").arg(context.to_string());
            }
            if !glob_filter.is_empty() {
                rg.arg("--glob").arg(glob_filter);
            }
            // "--" prevents pattern from being interpreted as a flag
            rg.arg("--").arg(pattern).arg(path);
            rg.stderr(std::process::Stdio::null());

            match rg.output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Limit to first 500 lines (replaces piped head -500)
                    let truncated: String = stdout.lines().take(500).collect::<Vec<_>>().join("\n");
                    let _ = storage.save_session(session);
                    if truncated.is_empty() {
                        json!({"type": "text", "text": "No matches found"})
                    } else {
                        json!({"type": "text", "text": truncated})
                    }
                }
                Err(e) => {
                    session.record_failure("Grep", &e.to_string());
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("Grep failed: {}", e)})
                }
            }
        }

        // ====== spf_web_fetch ======
        "spf_web_fetch" => {
            let url = args["url"].as_str().unwrap_or("");
            let prompt = args["prompt"].as_str().unwrap_or("Summarize this content");

            // HARDCODE: Gate check — NO BYPASS
            let params = ToolParams {
                url: Some(url.to_string()),
                query: Some(prompt.to_string()),
                ..Default::default()
            };
            let decision = gate::process("spf_web_fetch", &params, config, session);
            if !decision.allowed {
                session.record_manifest("web_fetch", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            session.record_action("WebFetch", "called", None);
            match WebClient::new() {
                Ok(client) => {
                    match client.read_page(url) {
                        Ok((text, raw_len, content_type)) => {
                            session.record_manifest("web_fetch", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            let truncated = if text.len() > 50000 { &text[..50000] } else { &text };
                            json!({"type": "text", "text": format!(
                                "Fetched {} ({} bytes, {})\nPrompt: {}\n\n{}",
                                url, raw_len, content_type, prompt, truncated
                            )})
                        }
                        Err(e) => {
                            session.record_failure("WebFetch", &e);
                            session.record_manifest("web_fetch", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            json!({"type": "text", "text": format!("WebFetch failed: {}", e)})
                        }
                    }
                }
                Err(e) => {
                    session.record_failure("WebFetch", &e);
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("WebClient init failed: {}", e)})
                }
            }
        }

        // ====== spf_web_search ======
        "spf_web_search" => {
            let query = args["query"].as_str().unwrap_or("");
            let count = args["count"].as_u64().unwrap_or(10) as u32;

            // HARDCODE: Gate check — NO BYPASS
            let params = ToolParams {
                query: Some(query.to_string()),
                ..Default::default()
            };
            let decision = gate::process("spf_web_search", &params, config, session);
            if !decision.allowed {
                session.record_manifest("web_search", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            session.record_action("WebSearch", "called", None);
            match WebClient::new() {
                Ok(client) => {
                    match client.search(query, count) {
                        Ok((engine, results)) => {
                            let mut output = format!("Search '{}' via {} ({} results):\n\n", query, engine, results.len());
                            for (i, r) in results.iter().enumerate() {
                                output.push_str(&format!("{}. {}\n   {}\n   {}\n\n", i + 1, r.title, r.url, r.description));
                            }
                            session.record_manifest("web_search", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            json!({"type": "text", "text": output})
                        }
                        Err(e) => {
                            session.record_failure("WebSearch", &e);
                            session.record_manifest("web_search", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            json!({"type": "text", "text": format!("WebSearch failed: {}", e)})
                        }
                    }
                }
                Err(e) => {
                    session.record_failure("WebSearch", &e);
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("WebClient init failed: {}", e)})
                }
            }
        }

        // ====== spf_web_download ======
        "spf_web_download" => {
            let url = args["url"].as_str().unwrap_or("");
            let save_path = args["save_path"].as_str().unwrap_or("");

            // HARDCODE: Gate check — NO BYPASS
            let params = ToolParams {
                url: Some(url.to_string()),
                file_path: Some(save_path.to_string()),
                ..Default::default()
            };
            let decision = gate::process("spf_web_download", &params, config, session);
            if !decision.allowed {
                session.record_manifest("web_download", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            session.record_action("WebDownload", "called", Some(save_path));
            match WebClient::new() {
                Ok(client) => {
                    match client.download(url, save_path) {
                        Ok((size, content_type)) => {
                            session.track_write(save_path);
                            session.record_manifest("web_download", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            json!({"type": "text", "text": format!(
                                "Downloaded {} → {} ({} bytes, {})",
                                url, save_path, size, content_type
                            )})
                        }
                        Err(e) => {
                            session.record_failure("WebDownload", &e);
                            session.record_manifest("web_download", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            json!({"type": "text", "text": format!("Download failed: {}", e)})
                        }
                    }
                }
                Err(e) => {
                    session.record_failure("WebDownload", &e);
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("WebClient init failed: {}", e)})
                }
            }
        }

        // ====== spf_web_api ======
        "spf_web_api" => {
            let method = args["method"].as_str().unwrap_or("GET");
            let url = args["url"].as_str().unwrap_or("");
            let headers = args["headers"].as_str().unwrap_or("");
            let body = args["body"].as_str().unwrap_or("");

            // HARDCODE: Gate check — NO BYPASS
            let params = ToolParams {
                url: Some(url.to_string()),
                query: Some(method.to_string()),
                ..Default::default()
            };
            let decision = gate::process("spf_web_api", &params, config, session);
            if !decision.allowed {
                session.record_manifest("web_api", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            session.record_action("WebAPI", "called", None);
            match WebClient::new() {
                Ok(client) => {
                    match client.api_request(method, url, headers, body) {
                        Ok((status, resp_headers, resp_body)) => {
                            session.record_manifest("web_api", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            let truncated = if resp_body.len() > 50000 { &resp_body[..50000] } else { &resp_body };
                            json!({"type": "text", "text": format!(
                                "API {} {} → HTTP {}\n\nHeaders:\n{}\n\nBody:\n{}",
                                method, url, status, resp_headers, truncated
                            )})
                        }
                        Err(e) => {
                            session.record_failure("WebAPI", &e);
                            session.record_manifest("web_api", decision.complexity.c, "ALLOWED", None);
                            let _ = storage.save_session(session);
                            json!({"type": "text", "text": format!("API request failed: {}", e)})
                        }
                    }
                }
                Err(e) => {
                    session.record_failure("WebAPI", &e);
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("WebClient init failed: {}", e)})
                }
            }
        }

        // ====== spf_notebook_edit ======
        "spf_notebook_edit" => {
            let notebook_path = args["notebook_path"].as_str().unwrap_or("");
            let new_source = args["new_source"].as_str().unwrap_or("");
            let cell_number = args["cell_number"].as_u64().unwrap_or(0) as usize;
            let cell_type = args["cell_type"].as_str().unwrap_or("code");
            let edit_mode = args["edit_mode"].as_str().unwrap_or("replace");

            // HARDCODE: Gate check — NO BYPASS
            let params = ToolParams {
                file_path: Some(notebook_path.to_string()),
                content: Some(new_source.to_string()),
                ..Default::default()
            };

            let decision = gate::process("spf_notebook_edit", &params, config, session);
            if !decision.allowed {
                session.record_manifest("NotebookEdit", decision.complexity.c, "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": format!("BLOCKED: {}", decision.errors.join(", "))});
            }

            session.record_action("NotebookEdit", "called", Some(notebook_path));

            // Read notebook JSON
            match std::fs::read_to_string(notebook_path) {
                Ok(content) => {
                    match serde_json::from_str::<Value>(&content) {
                        Ok(mut notebook) => {
                            if let Some(cells) = notebook.get_mut("cells").and_then(|c| c.as_array_mut()) {
                                match edit_mode {
                                    "replace" => {
                                        if cell_number < cells.len() {
                                            cells[cell_number]["source"] = json!([new_source]);
                                            cells[cell_number]["cell_type"] = json!(cell_type);
                                        } else {
                                            return json!({"type": "text", "text": format!("Cell {} not found", cell_number)});
                                        }
                                    }
                                    "insert" => {
                                        let new_cell = json!({
                                            "cell_type": cell_type,
                                            "source": [new_source],
                                            "metadata": {},
                                            "outputs": []
                                        });
                                        cells.insert(cell_number, new_cell);
                                    }
                                    "delete" => {
                                        if cell_number < cells.len() {
                                            cells.remove(cell_number);
                                        }
                                    }
                                    _ => return json!({"type": "text", "text": "Invalid edit_mode"})
                                }

                                // Write back
                                match std::fs::write(notebook_path, serde_json::to_string_pretty(&notebook).unwrap()) {
                                    Ok(()) => {
                                        session.track_write(notebook_path);
                                        let _ = storage.save_session(session);
                                        json!({"type": "text", "text": format!("Notebook edited: {} cell {} ({})", notebook_path, cell_number, edit_mode)})
                                    }
                                    Err(e) => {
                                        session.record_failure("NotebookEdit", &e.to_string());
                                        let _ = storage.save_session(session);
                                        json!({"type": "text", "text": format!("Write failed: {}", e)})
                                    }
                                }
                            } else {
                                json!({"type": "text", "text": "Invalid notebook: no cells array"})
                            }
                        }
                        Err(e) => json!({"type": "text", "text": format!("JSON parse error: {}", e)})
                    }
                }
                Err(e) => {
                    session.record_failure("NotebookEdit", &e.to_string());
                    let _ = storage.save_session(session);
                    json!({"type": "text", "text": format!("Read failed: {}", e)})
                }
            }
        }

        // ====== spf_brain_search ======
        "spf_brain_search" => {
            let query = args["query"].as_str().unwrap_or("");
            let collection = args["collection"].as_str().unwrap_or("default");
            let limit = args["limit"].as_u64().unwrap_or(5);

            let gate_params = ToolParams { query: Some(query.to_string()), ..Default::default() };
            let decision = gate::process("spf_brain_search", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_search", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }

            session.record_action("brain_search", "called", None);

            let limit_str = limit.to_string();
            let mut search_args = vec!["search", query, "--top-k", &limit_str];
            if collection != "default" && !collection.is_empty() {
                search_args.push("--collection");
                search_args.push(collection);
            }
            let (success, output) = run_brain(&search_args);
            let _ = storage.save_session(session);

            if success {
                json!({"type": "text", "text": format!("Brain search '{}':\n\n{}", query, output)})
            } else {
                json!({"type": "text", "text": format!("Brain search failed: {}", output)})
            }
        }

        // ====== spf_brain_store ======
        "spf_brain_store" => {
            let text = args["text"].as_str().unwrap_or("");
            let title = args["title"].as_str().unwrap_or("untitled");
            let collection = args["collection"].as_str().unwrap_or("default");
            let tags = args["tags"].as_str().unwrap_or("");

            let gate_params = ToolParams { content: Some(text.to_string()), ..Default::default() };
            let decision = gate::process("spf_brain_store", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_store", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }

            session.record_action("brain_store", "called", None);

            let mut cmd_args = vec!["store", text, "--title", title, "--collection", collection, "--index"];
            if !tags.is_empty() {
                cmd_args.push("--tags");
                cmd_args.push(tags);
            }

            let (success, output) = run_brain(&cmd_args);
            let _ = storage.save_session(session);

            if success {
                json!({"type": "text", "text": format!("Stored to brain:\n{}", output)})
            } else {
                json!({"type": "text", "text": format!("Brain store failed: {}", output)})
            }
        }

        // ====== spf_brain_context ======
        "spf_brain_context" => {
            let query = args["query"].as_str().unwrap_or("");
            let max_tokens = args["max_tokens"].as_u64().unwrap_or(2000);

            let gate_params = ToolParams { query: Some(query.to_string()), ..Default::default() };
            let decision = gate::process("spf_brain_context", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_context", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("brain_context", "called", None);
            let (success, output) = run_brain(&["context", query, "--max-tokens", &max_tokens.to_string()]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("Brain context failed: {}", output)})
            }
        }

        // ====== spf_brain_index ======
        "spf_brain_index" => {
            let path = args["path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_brain_index", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_index", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("brain_index", "called", Some(path));
            let (success, output) = run_brain(&["index", path]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": format!("Indexed: {}\n{}", path, output)})
            } else {
                json!({"type": "text", "text": format!("Brain index failed: {}", output)})
            }
        }

        // ====== spf_brain_list ======
        "spf_brain_list" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_brain_list", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_list", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("brain_list", "called", None);
            let (success, output) = run_brain(&["list"]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("Brain list failed: {}", output)})
            }
        }

        // ====== spf_brain_status ======
        "spf_brain_status" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_brain_status", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_status", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("brain_status", "called", None);
            let brain = brain_path();
            let mut parts = vec![format!("Binary: {:?} ({})", brain, if brain.exists() { "OK" } else { "NOT FOUND" })];
            let (success, output) = run_brain(&["list"]);
            if success {
                parts.push(format!("Collections:\n{}", output));
            }
            let storage_path = actual_home().join("stoneshell-brain/storage");
            if storage_path.exists() {
                if let Ok(entries) = std::fs::read_dir(&storage_path) {
                    let size: u64 = entries.filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum();
                    parts.push(format!("Storage: {:.2} MB", size as f64 / 1024.0 / 1024.0));
                }
            }
            let _ = storage.save_session(session);
            json!({"type": "text", "text": parts.join("\n\n")})
        }

        // ====== spf_brain_recall ======
        "spf_brain_recall" => {
            let query = args["query"].as_str().unwrap_or("");
            let collection = args["collection"].as_str().unwrap_or("default");

            let gate_params = ToolParams { query: Some(query.to_string()), ..Default::default() };
            let decision = gate::process("spf_brain_recall", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_recall", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("brain_recall", "called", None);
            let (success, output) = run_brain(&["recall", query, "-c", collection]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("Brain recall failed: {}", output)})
            }
        }

        // ====== spf_brain_list_docs ======
        "spf_brain_list_docs" => {
            let collection = args["collection"].as_str().unwrap_or("default");

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_brain_list_docs", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_list_docs", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("brain_list_docs", "called", None);
            let (success, output) = run_brain(&["list-docs", "-c", collection]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("Brain list-docs failed: {}", output)})
            }
        }

        // ====== spf_brain_get_doc ======
        "spf_brain_get_doc" => {
            let doc_id = args["doc_id"].as_str().unwrap_or("");
            let collection = args["collection"].as_str().unwrap_or("default");

            let gate_params = ToolParams { command: Some(doc_id.to_string()), ..Default::default() };
            let decision = gate::process("spf_brain_get_doc", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_brain_get_doc", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("brain_get_doc", "called", None);
            let (success, output) = run_brain(&["get-doc", doc_id, "-c", collection]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("Brain get-doc failed: {}", output)})
            }
        }

        // ====== RAG COLLECTOR HANDLERS ======

        // ====== spf_rag_collect_web ======
        "spf_rag_collect_web" => {
            let topic = args["topic"].as_str().unwrap_or("");

            let gate_params = ToolParams { command: Some(topic.to_string()), ..Default::default() };
            let decision = gate::process("spf_rag_collect_web", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_collect_web", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_collect_web", "called", None);
            let mut cmd_args = vec!["collect"];
            if !topic.is_empty() {
                cmd_args.push("--topic");
                cmd_args.push(topic);
            }
            let (success, output) = run_rag(&cmd_args);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG collect-web failed: {}", output)})
            }
        }

        // ====== spf_rag_collect_file ======
        "spf_rag_collect_file" => {
            let path = args["path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_rag_collect_file", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_collect_file", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_collect_file", "called", Some(path));
            let (success, output) = run_rag(&["collect", "--path", path]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG collect-file failed: {}", output)})
            }
        }

        // ====== spf_rag_collect_folder ======
        "spf_rag_collect_folder" => {
            let path = args["path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_rag_collect_folder", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_collect_folder", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_collect_folder", "called", Some(path));
            let (success, output) = run_rag(&["collect", "--path", path]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG collect-folder failed: {}", output)})
            }
        }

        // ====== spf_rag_collect_drop ======
        "spf_rag_collect_drop" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_collect_drop", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_collect_drop", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_collect_drop", "called", None);
            let (success, output) = run_rag(&["drop"]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG collect-drop failed: {}", output)})
            }
        }

        // ====== spf_rag_index_gathered ======
        "spf_rag_index_gathered" => {
            let category = args["category"].as_str().unwrap_or("");

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_index_gathered", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_index_gathered", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_index_gathered", "called", None);
            let mut cmd_args = vec!["index"];
            if !category.is_empty() {
                cmd_args.push("--category");
                cmd_args.push(category);
            }
            let (success, output) = run_rag(&cmd_args);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG index-gathered failed: {}", output)})
            }
        }

        // ====== spf_rag_dedupe ======
        "spf_rag_dedupe" => {
            let category = args["category"].as_str().unwrap_or("");

            let gate_params = ToolParams { command: Some(category.to_string()), ..Default::default() };
            let decision = gate::process("spf_rag_dedupe", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_dedupe", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_dedupe", "called", None);
            // Dedupe goes through brain binary directly
            let (success, output) = run_brain(&["dedup", "-c", category]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG dedupe failed: {}", output)})
            }
        }

        // ====== spf_rag_status ======
        "spf_rag_status" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_status", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_status", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_status", "called", None);
            let (success, output) = run_rag(&["status"]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG status failed: {}", output)})
            }
        }

        // ====== spf_rag_list_gathered ======
        "spf_rag_list_gathered" => {
            let category = args["category"].as_str().unwrap_or("");

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_list_gathered", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_list_gathered", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_list_gathered", "called", None);
            let mut cmd_args = vec!["list-gathered"];
            if !category.is_empty() {
                cmd_args.push("--category");
                cmd_args.push(category);
            }
            let (success, output) = run_rag(&cmd_args);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG list-gathered failed: {}", output)})
            }
        }

        // ====== spf_rag_bandwidth_status ======
        "spf_rag_bandwidth_status" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_bandwidth_status", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_bandwidth_status", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_bandwidth_status", "called", None);
            let (success, output) = run_rag(&["bandwidth"]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG bandwidth-status failed: {}", output)})
            }
        }

        // ====== spf_rag_fetch_url ======
        "spf_rag_fetch_url" => {
            let url = args["url"].as_str().unwrap_or("");

            let gate_params = ToolParams { url: Some(url.to_string()), ..Default::default() };
            let decision = gate::process("spf_rag_fetch_url", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_fetch_url", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_fetch_url", "called", None);
            // Fetch URL through collect with path (URL handling)
            let (success, output) = run_rag(&["collect", "--path", url]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG fetch-url failed: {}", output)})
            }
        }

        // ====== spf_rag_collect_rss ======
        "spf_rag_collect_rss" => {
            let feed_name = args["feed_name"].as_str().unwrap_or("");

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_collect_rss", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_collect_rss", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_collect_rss", "called", None);
            let mut cmd_args = vec!["rss"];
            if !feed_name.is_empty() {
                cmd_args.push("--feed");
                cmd_args.push(feed_name);
            }
            let (success, output) = run_rag(&cmd_args);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG collect-rss failed: {}", output)})
            }
        }

        // ====== spf_rag_list_feeds ======
        "spf_rag_list_feeds" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_list_feeds", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_list_feeds", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_list_feeds", "called", None);
            // Read RSS config directly
            let rss_path = rag_collector_dir().join("sources/rss_sources.json");
            let (success, output) = if rss_path.exists() {
                match std::fs::read_to_string(&rss_path) {
                    Ok(content) => (true, content),
                    Err(e) => (false, format!("Failed to read RSS sources: {}", e)),
                }
            } else {
                (false, "RSS sources file not found".to_string())
            };
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG list-feeds failed: {}", output)})
            }
        }

        // ====== spf_rag_pending_searches ======
        "spf_rag_pending_searches" => {
            let collection = args["collection"].as_str().unwrap_or("default");

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_pending_searches", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_pending_searches", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_pending_searches", "called", None);
            let (success, output) = run_brain(&["pending-searches", "-c", collection, "-f", "json"]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG pending-searches failed: {}", output)})
            }
        }

        // ====== spf_rag_fulfill_search ======
        "spf_rag_fulfill_search" => {
            let seeker_id = args["seeker_id"].as_str().unwrap_or("");
            let collection = args["collection"].as_str().unwrap_or("default");

            let gate_params = ToolParams { command: Some(seeker_id.to_string()), ..Default::default() };
            let decision = gate::process("spf_rag_fulfill_search", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_fulfill_search", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_fulfill_search", "called", None);
            let (success, output) = run_brain(&["fulfill-search", seeker_id, "-c", collection]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG fulfill-search failed: {}", output)})
            }
        }

        // ====== spf_rag_smart_search ======
        "spf_rag_smart_search" => {
            let query = args["query"].as_str().unwrap_or("");
            let collection = args["collection"].as_str().unwrap_or("default");

            let gate_params = ToolParams { query: Some(query.to_string()), ..Default::default() };
            let decision = gate::process("spf_rag_smart_search", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_smart_search", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_smart_search", "called", None);
            let (success, output) = run_brain(&["smart-search", query, "-c", collection, "-f", "json"]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG smart-search failed: {}", output)})
            }
        }

        // ====== spf_rag_auto_fetch_gaps ======
        "spf_rag_auto_fetch_gaps" => {
            let collection = args["collection"].as_str().unwrap_or("default");
            let max_fetches = args["max_fetches"].as_u64().unwrap_or(5);

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_rag_auto_fetch_gaps", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_rag_auto_fetch_gaps", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("rag_auto_fetch_gaps", "called", None);
            // Auto-fetch uses pending-searches then fetches URLs
            let (success, output) = run_brain(&["auto-fetch", "-c", collection, "--max", &max_fetches.to_string()]);
            let _ = storage.save_session(session);
            if success {
                json!({"type": "text", "text": output})
            } else {
                json!({"type": "text", "text": format!("RAG auto-fetch-gaps failed: {}", output)})
            }
        }

        // ====== SPF_CONFIG HANDLERS ======
        // NOTE: spf_config_get and spf_config_set blocked - user-only via CLI
        "spf_config_get" | "spf_config_set" => {
            json!({"type": "text", "text": "BLOCKED: Config read/write is user-only (use CLI)"})
        }

        "spf_config_paths" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_config_paths", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_config_paths", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("config_paths", "list", None);
            let _ = storage.save_session(session);

            match config_db {
                Some(db) => match db.list_path_rules() {
                    Ok(rules) => {
                        let text = rules.iter()
                            .map(|(t, p)| format!("{}: {}", t, p))
                            .collect::<Vec<_>>()
                            .join("\n");
                        json!({"type": "text", "text": if text.is_empty() { "No path rules configured".to_string() } else { text }})
                    }
                    Err(e) => json!({"type": "text", "text": format!("list_path_rules failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "SPF_CONFIG LMDB not initialized"}),
            }
        }

        "spf_config_stats" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_config_stats", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_config_stats", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("config_stats", "get", None);
            let _ = storage.save_session(session);

            match config_db {
                Some(db) => match db.stats() {
                    Ok((config_count, paths_count, patterns_count)) => {
                        json!({"type": "text", "text": format!(
                            "SPF_CONFIG LMDB Stats:\n  Config entries: {}\n  Path rules: {}\n  Dangerous patterns: {}",
                            config_count, paths_count, patterns_count
                        )})
                    }
                    Err(e) => json!({"type": "text", "text": format!("config_stats failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "SPF_CONFIG LMDB not initialized"}),
            }
        }

        // ====== PROJECTS_DB HANDLERS ======
        "spf_projects_list" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_projects_list", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_projects_list", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("projects_list", "list", None);
            let _ = storage.save_session(session);

            match projects_db {
                Some(db) => match db.list_all() {
                    Ok(entries) => {
                        let text = entries.iter()
                            .map(|(k, v)| format!("{}: {}", k, v))
                            .collect::<Vec<_>>()
                            .join("\n");
                        json!({"type": "text", "text": if text.is_empty() { "No projects registered".to_string() } else { text }})
                    }
                    Err(e) => json!({"type": "text", "text": format!("projects_list failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "PROJECTS LMDB not initialized"}),
            }
        }

        "spf_projects_get" => {
            let key = args["key"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(key.to_string()), ..Default::default() };
            let decision = gate::process("spf_projects_get", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_projects_get", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("projects_get", "get", Some(key));
            let _ = storage.save_session(session);

            match projects_db {
                Some(db) => match db.get(key) {
                    Ok(Some(value)) => json!({"type": "text", "text": format!("{}: {}", key, value)}),
                    Ok(None) => json!({"type": "text", "text": format!("Key not found: {}", key)}),
                    Err(e) => json!({"type": "text", "text": format!("projects_get failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "PROJECTS LMDB not initialized"}),
            }
        }

        "spf_projects_set" => {
            let key = args["key"].as_str().unwrap_or("");
            let value = args["value"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(key.to_string()), ..Default::default() };
            let decision = gate::process("spf_projects_set", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_projects_set", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("projects_set", "write", Some(key));
            let _ = storage.save_session(session);

            match projects_db {
                Some(db) => match db.set(key, value) {
                    Ok(()) => json!({"type": "text", "text": format!("Set: {} = {}", key, value)}),
                    Err(e) => json!({"type": "text", "text": format!("projects_set failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "PROJECTS LMDB not initialized"}),
            }
        }

        "spf_projects_delete" => {
            let key = args["key"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(key.to_string()), ..Default::default() };
            let decision = gate::process("spf_projects_delete", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_projects_delete", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("projects_delete", "write", Some(key));
            let _ = storage.save_session(session);

            match projects_db {
                Some(db) => match db.delete(key) {
                    Ok(true) => json!({"type": "text", "text": format!("Deleted: {}", key)}),
                    Ok(false) => json!({"type": "text", "text": format!("Key not found: {}", key)}),
                    Err(e) => json!({"type": "text", "text": format!("projects_delete failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "PROJECTS LMDB not initialized"}),
            }
        }

        "spf_projects_stats" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_projects_stats", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_projects_stats", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("projects_stats", "get", None);
            let _ = storage.save_session(session);

            match projects_db {
                Some(db) => match db.db_stats() {
                    Ok((data_count, _, _)) => {
                        json!({"type": "text", "text": format!(
                            "PROJECTS LMDB Stats:\n  Entries: {}", data_count
                        )})
                    }
                    Err(e) => json!({"type": "text", "text": format!("projects_stats failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "PROJECTS LMDB not initialized"}),
            }
        }

        // ====== TMP_DB HANDLERS ======
        "spf_tmp_list" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_tmp_list", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_tmp_list", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("tmp_list", "list", None);
            let _ = storage.save_session(session);

            match tmp_db {
                Some(db) => match db.list_projects() {
                    Ok(projects) => {
                        let text = projects.iter()
                            .map(|p| format!("{}: {} | trust={:?} | reads={} writes={} | active={}",
                                p.name, p.path, p.trust_level,
                                p.total_reads, p.total_writes, p.is_active))
                            .collect::<Vec<_>>()
                            .join("\n");
                        json!({"type": "text", "text": if text.is_empty() { "No projects registered".to_string() } else { text }})
                    }
                    Err(e) => json!({"type": "text", "text": format!("list_projects failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "TMP_DB LMDB not initialized"}),
            }
        }

        "spf_tmp_stats" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_tmp_stats", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_tmp_stats", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("tmp_stats", "get", None);
            let _ = storage.save_session(session);

            match tmp_db {
                Some(db) => match db.db_stats() {
                    Ok((projects_count, access_count, resources_count)) => {
                        json!({"type": "text", "text": format!(
                            "TMP_DB LMDB Stats:\n  Registered projects: {}\n  Access log entries: {}\n  Resource records: {}",
                            projects_count, access_count, resources_count
                        )})
                    }
                    Err(e) => json!({"type": "text", "text": format!("tmp_stats failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "TMP_DB LMDB not initialized"}),
            }
        }

        "spf_tmp_get" => {
            let path_arg = args["path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path_arg.to_string()), ..Default::default() };
            let decision = gate::process("spf_tmp_get", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_tmp_get", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("tmp_get", "get", Some(path_arg));
            let _ = storage.save_session(session);

            match tmp_db {
                Some(db) => match db.get_project(path_arg) {
                    Ok(Some(proj)) => {
                        json!({"type": "text", "text": format!(
                            "Project: {}\nPath: {}\nTrust: {:?}\nActive: {}\nReads: {} | Writes: {} | Session writes: {}/{}\nMax write size: {} | Total C: {}\nProtected: {:?}\nCreated: {} | Last accessed: {}\nNotes: {}",
                            proj.name, proj.path, proj.trust_level, proj.is_active,
                            proj.total_reads, proj.total_writes, proj.session_writes, proj.max_writes_per_session,
                            proj.max_write_size, proj.total_complexity,
                            proj.protected_paths,
                            format_timestamp(proj.created_at), format_timestamp(proj.last_accessed),
                            if proj.notes.is_empty() { "None" } else { &proj.notes }
                        )})
                    }
                    Ok(None) => json!({"type": "text", "text": format!("Project not found: {}", path_arg)}),
                    Err(e) => json!({"type": "text", "text": format!("get_project failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "TMP_DB LMDB not initialized"}),
            }
        }

        "spf_tmp_active" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_tmp_active", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_tmp_active", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("tmp_active", "get", None);
            let _ = storage.save_session(session);

            match tmp_db {
                Some(db) => match db.get_active() {
                    Ok(Some(path)) => {
                        // Also fetch project details
                        match db.get_project(&path) {
                            Ok(Some(proj)) => {
                                json!({"type": "text", "text": format!(
                                    "Active project: {} ({})\nTrust: {:?} | Reads: {} | Writes: {}",
                                    proj.name, proj.path, proj.trust_level, proj.total_reads, proj.total_writes
                                )})
                            }
                            _ => json!({"type": "text", "text": format!("Active project path: {} (details unavailable)", path)}),
                        }
                    }
                    Ok(None) => json!({"type": "text", "text": "No active project"}),
                    Err(e) => json!({"type": "text", "text": format!("get_active failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "TMP_DB LMDB not initialized"}),
            }
        }

        // ====== AGENT_STATE HANDLERS ======
        // BLOCKED: Write operations are user-only
        "spf_agent_remember" | "spf_agent_forget" | "spf_agent_set_state" => {
            json!({"type": "text", "text": "BLOCKED: Agent state writes are user-only (use CLI)"})
        }

        "spf_agent_stats" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_agent_stats", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_agent_stats", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("agent_stats", "get", None);
            let _ = storage.save_session(session);

            match agent_db {
                Some(db) => match db.db_stats() {
                    Ok((memory_count, sessions_count, state_count, tags_count)) => {
                        json!({"type": "text", "text": format!(
                            "AGENT_STATE LMDB Stats:\n  Memories: {}\n  Sessions: {}\n  State keys: {}\n  Tags: {}",
                            memory_count, sessions_count, state_count, tags_count
                        )})
                    }
                    Err(e) => json!({"type": "text", "text": format!("agent_stats failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "AGENT_STATE LMDB not initialized"}),
            }
        }

        "spf_agent_memory_search" => {
            let query = args["query"].as_str().unwrap_or("");
            let limit = args["limit"].as_u64().unwrap_or(10) as usize;

            let gate_params = ToolParams { query: Some(query.to_string()), ..Default::default() };
            let decision = gate::process("spf_agent_memory_search", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_agent_memory_search", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("agent_memory_search", "search", Some(query));
            let _ = storage.save_session(session);

            match agent_db {
                Some(db) => match db.search_memories(query, limit) {
                    Ok(memories) => {
                        if memories.is_empty() {
                            json!({"type": "text", "text": format!("No memories found for: {}", query)})
                        } else {
                            let text = memories.iter()
                                .map(|m| format!("[{}] {:?} | {}\n  Tags: {:?} | Created: {}",
                                    m.id, m.memory_type, m.content,
                                    m.tags, format_timestamp(m.created_at)))
                                .collect::<Vec<_>>()
                                .join("\n\n");
                            json!({"type": "text", "text": text})
                        }
                    }
                    Err(e) => json!({"type": "text", "text": format!("search_memories failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "AGENT_STATE LMDB not initialized"}),
            }
        }

        "spf_agent_memory_by_tag" => {
            let tag = args["tag"].as_str().unwrap_or("");

            let gate_params = ToolParams { command: Some(tag.to_string()), ..Default::default() };
            let decision = gate::process("spf_agent_memory_by_tag", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_agent_memory_by_tag", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("agent_memory_by_tag", "search", Some(tag));
            let _ = storage.save_session(session);

            match agent_db {
                Some(db) => match db.get_by_tag(tag) {
                    Ok(memories) => {
                        if memories.is_empty() {
                            json!({"type": "text", "text": format!("No memories with tag: {}", tag)})
                        } else {
                            let text = memories.iter()
                                .map(|m| format!("[{}] {:?} | {}",
                                    m.id, m.memory_type, m.content))
                                .collect::<Vec<_>>()
                                .join("\n");
                            json!({"type": "text", "text": text})
                        }
                    }
                    Err(e) => json!({"type": "text", "text": format!("get_by_tag failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "AGENT_STATE LMDB not initialized"}),
            }
        }

        "spf_agent_session_info" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_agent_session_info", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_agent_session_info", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("agent_session_info", "get", None);
            let _ = storage.save_session(session);

            match agent_db {
                Some(db) => match db.get_latest_session() {
                    Ok(Some(sess)) => {
                        json!({"type": "text", "text": format!(
                            "Session: {}\nParent: {}\nStarted: {} | Ended: {}\nWorking dir: {}\nProject: {}\nFiles modified: {}\nComplexity: {} | Actions: {}\nSummary: {}",
                            sess.session_id,
                            sess.parent_session.as_deref().unwrap_or("None"),
                            format_timestamp(sess.started_at),
                            if sess.ended_at == 0 { "Ongoing".to_string() } else { format_timestamp(sess.ended_at) },
                            sess.working_dir,
                            sess.active_project.as_deref().unwrap_or("None"),
                            sess.files_modified.len(),
                            sess.total_complexity, sess.total_actions,
                            if sess.summary.is_empty() { "None" } else { &sess.summary }
                        )})
                    }
                    Ok(None) => json!({"type": "text", "text": "No sessions recorded"}),
                    Err(e) => json!({"type": "text", "text": format!("get_latest_session failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "AGENT_STATE LMDB not initialized"}),
            }
        }

        "spf_agent_context" => {

            let gate_params = ToolParams { ..Default::default() };
            let decision = gate::process("spf_agent_context", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_agent_context", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("agent_context", "get", None);
            let _ = storage.save_session(session);

            match agent_db {
                Some(db) => match db.get_context_summary() {
                    Ok(summary) => {
                        json!({"type": "text", "text": if summary.is_empty() { "No context available".to_string() } else { summary }})
                    }
                    Err(e) => json!({"type": "text", "text": format!("get_context_summary failed: {}", e)}),
                },
                None => json!({"type": "text", "text": "AGENT_STATE LMDB not initialized"}),
            }
        }

        // ====== SPF_FS (LMDB 1) Handlers ======
        "spf_fs_exists" => {
            let path = args["path"].as_str().unwrap_or("/");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_exists", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_exists", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_exists", "check", Some(path));
            let _ = storage.save_session(session);

            if let Some(result) = route_to_lmdb(path, "exists", None, config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: path {} not routable — no LMDB fallback", path)})
        }

        "spf_fs_stat" => {
            let path = args["path"].as_str().unwrap_or("/");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_stat", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_stat", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_stat", "get", Some(path));
            let _ = storage.save_session(session);

            if let Some(result) = route_to_lmdb(path, "stat", None, config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: path {} not routable — no LMDB fallback", path)})
        }

        "spf_fs_ls" => {
            let path = args["path"].as_str().unwrap_or("/");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_ls", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_ls", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_ls", "list", Some(path));
            let _ = storage.save_session(session);

            if let Some(result) = route_to_lmdb(path, "ls", None, config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: path {} not routable — no LMDB fallback", path)})
        }

        "spf_fs_read" => {
            let path = args["path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_read", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_read", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_read", "read", Some(path));
            let _ = storage.save_session(session);

            if let Some(result) = route_to_lmdb(path, "read", None, config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: path {} not routable — no LMDB fallback", path)})
        }

        "spf_fs_write" => {
            let path = args["path"].as_str().unwrap_or("");
            let content = args["content"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path.to_string()), content: Some(content.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_write", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_write", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_write", "write", Some(path));
            let _ = storage.save_session(session);

            if let Some(result) = route_to_lmdb(path, "write", Some(content), config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: path {} not routable — no LMDB fallback", path)})
        }

        "spf_fs_mkdir" => {
            let path = args["path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_mkdir", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_mkdir", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_mkdir", "create", Some(path));
            let _ = storage.save_session(session);

            if let Some(result) = route_to_lmdb(path, "mkdir", None, config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: path {} not routable — no LMDB fallback", path)})
        }

        "spf_fs_rm" => {
            let path = args["path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(path.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_rm", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_rm", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_rm", "remove", Some(path));
            let _ = storage.save_session(session);

            if let Some(result) = route_to_lmdb(path, "rm", None, config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: path {} not routable — no LMDB fallback", path)})
        }

        "spf_fs_rename" => {
            let old_path = args["old_path"].as_str().unwrap_or("");
            let new_path = args["new_path"].as_str().unwrap_or("");

            let gate_params = ToolParams { file_path: Some(old_path.to_string()), ..Default::default() };
            let decision = gate::process("spf_fs_rename", &gate_params, config, session);
            if !decision.allowed {
                session.record_manifest("spf_fs_rename", decision.complexity.c,
                    "BLOCKED",
                    decision.errors.first().map(|s| s.as_str()));
                let _ = storage.save_session(session);
                return json!({"type": "text", "text": decision.message});
            }
            session.record_action("fs_rename", "rename", Some(old_path));
            let _ = storage.save_session(session);

            // Device-backed directory rename (handle before route_to_lmdb)
            let is_device_rename = old_path.starts_with("/tmp/") || old_path.starts_with("/projects/");
            if is_device_rename {
                // Path traversal protection
                if old_path.contains("..") || new_path.contains("..") {
                    return json!({"type": "text", "text": "BLOCKED: path traversal detected in rename paths"});
                }
                let live_base = spf_root().join("LIVE").display().to_string();
                let resolve = |vpath: &str| -> std::path::PathBuf {
                    if vpath.starts_with("/tmp/") {
                        std::path::PathBuf::from(format!("{}/TMP/TMP", live_base))
                            .join(vpath.strip_prefix("/tmp/").unwrap_or(""))
                    } else {
                        std::path::PathBuf::from(format!("{}/PROJECTS/PROJECTS", live_base))
                            .join(vpath.strip_prefix("/projects/").unwrap_or(""))
                    }
                };
                let old_device = resolve(old_path);
                let new_device = resolve(new_path);
                if let Some(parent) = new_device.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                return match std::fs::rename(&old_device, &new_device) {
                    Ok(()) => json!({"type": "text", "text": format!("Renamed: {} -> {}", old_path, new_path)}),
                    Err(e) => json!({"type": "text", "text": format!("rename failed: {}", e)}),
                };
            }
            if let Some(result) = route_to_lmdb(old_path, "rename", None, config_db, tmp_db, agent_db) {
                return result;
            }
            json!({"type": "text", "text": format!("BLOCKED: paths {}, {} not routable — no LMDB fallback", old_path, new_path)})
        }

        _ => {
            json!({"type": "text", "text": format!("Unknown tool: {}", name)})
        }
    }
}

/// Main MCP server loop — runs forever on stdio
pub fn run(config: SpfConfig, config_db: SpfConfigDb, mut session: Session, storage: SpfStorage) {
    log(&format!("Starting {} v{}", SERVER_NAME, SERVER_VERSION));
    log(&format!("Mode: {:?}", config.enforce_mode));

    // LIVE/ base — all LMDBs live here, outside Claude's writable zone
    let live_base = spf_root().join("LIVE");

    // CONFIG LMDB passed from main.rs — single open, single source of truth
    let config_db = Some(config_db);
    log("SPF_CONFIG LMDB active (passed from main)");

    // Initialize PROJECTS LMDB
    let projects_db_path = live_base.join("PROJECTS/PROJECTS.DB");
    log(&format!("PROJECTS path: {:?}", projects_db_path));

    let projects_db = match SpfProjectsDb::open(&projects_db_path) {
        Ok(db) => {
            if let Err(e) = db.init_defaults() {
                log(&format!("Warning: PROJECTS init_defaults failed: {}", e));
            }
            log(&format!("PROJECTS LMDB initialized at {:?}", projects_db_path));
            Some(db)
        }
        Err(e) => {
            log(&format!("Warning: Failed to open PROJECTS LMDB at {:?}: {}", projects_db_path, e));
            None
        }
    };

    // Initialize TMP_DB LMDB (was TMP_DB — tracks /tmp and /projects metadata)
    let tmp_db_path = live_base.join("TMP/TMP.DB");
    log(&format!("TMP_DB path: {:?}", tmp_db_path));

    let tmp_db = match SpfTmpDb::open(&tmp_db_path) {
        Ok(db) => {
            log(&format!("TMP_DB LMDB initialized at {:?}", tmp_db_path));
            Some(db)
        }
        Err(e) => {
            log(&format!("Warning: Failed to open TMP_DB LMDB at {:?}: {}", tmp_db_path, e));
            None
        }
    };

    // Initialize AGENT_STATE LMDB
    let agent_db_path = live_base.join("LMDB5/LMDB5.DB");
    log(&format!("AGENT_STATE path: {:?}", agent_db_path));

    let agent_db = match AgentStateDb::open(&agent_db_path) {
        Ok(db) => {
            if let Err(e) = db.init_defaults() {
                log(&format!("Warning: AGENT_STATE init_defaults failed: {}", e));
            }
            log(&format!("AGENT_STATE LMDB initialized at {:?}", agent_db_path));
            Some(db)
        }
        Err(e) => {
            log(&format!("Warning: Failed to open AGENT_STATE LMDB at {:?}: {}", agent_db_path, e));
            None
        }
    };

    // Initialize SPF_FS LMDB (LMDB 1: Virtual Filesystem)
    let fs_db_storage = live_base.join("SPF_FS");
    log(&format!("SPF_FS path: {:?}", fs_db_storage));

    let fs_db = match SpfFs::open(&fs_db_storage) {
        Ok(db) => {
            log(&format!("SPF_FS LMDB initialized at {:?}/SPF_FS.DB/", fs_db_storage));
            Some(db)
        }
        Err(e) => {
            log(&format!("Warning: Failed to open SPF_FS LMDB: {}", e));
            None
        }
    };

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                log(&format!("stdin read error: {}", e));
                continue;
            }
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                log(&format!("JSON parse error: {}", e));
                continue;
            }
        };

        let method = msg["method"].as_str().unwrap_or("");
        let id = &msg["id"];
        let params = &msg["params"];

        log(&format!("Received: {}", method));

        match method {
            "initialize" => {
                send_response(id, json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "version": SERVER_VERSION,
                    }
                }));
            }

            "notifications/initialized" => {
                // No response needed
            }

            "tools/list" => {
                send_response(id, json!({ "tools": tool_definitions() }));
            }

            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));

                cmd_log(&format!("CALL {} | {}", name, param_summary(name, &args)));

                let result = handle_tool_call(name, &args, &config, &mut session, &storage, &config_db, &projects_db, &tmp_db, &fs_db, &agent_db);

                // Log failures
                let text = result.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if text.starts_with("ERROR") || text.starts_with("BLOCKED") {
                    let snippet: String = text.chars().take(200).collect();
                    cmd_log(&format!("FAIL {} | {}", name, snippet));
                }

                send_response(id, json!({
                    "content": [result]
                }));
            }

            "ping" => {
                send_response(id, json!({}));
            }

            _ => {
                if !id.is_null() {
                    send_error(id, -32601, &format!("Unknown method: {}", method));
                }
            }
        }
    }
}
