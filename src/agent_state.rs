// SPF Smart Gateway - Agent State LMDB
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// LMDB-backed persistent state for Agent's virtual home. Stores preferences,
// memory, working context, and session continuity data across sessions.
//
// Database: AGENT_STATE
// Storage: ~/SPFsmartGATE/LIVE/LMDB5/LMDB5.DB/

use anyhow::{anyhow, Result};
use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Atomic counter for unique memory IDs within same timestamp
static MEMORY_COUNTER: AtomicU64 = AtomicU64::new(0);

const MAX_DB_SIZE: usize = 100 * 1024 * 1024; // 100MB - Agent state can grow

/// Memory entry type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryType {
    /// User preference
    Preference,
    /// Fact about the user/project
    Fact,
    /// Instruction from user
    Instruction,
    /// Context from previous sessions
    Context,
    /// Working state (temporary, session-bound)
    Working,
    /// Pinned (never auto-expire)
    Pinned,
}

/// Memory entry stored in Agent's memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique ID
    pub id: String,
    /// Memory content
    pub content: String,
    /// Memory type
    pub memory_type: MemoryType,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Source (session ID or "user" if explicit)
    pub source: String,
    /// Created timestamp
    pub created_at: u64,
    /// Last accessed timestamp
    pub last_accessed: u64,
    /// Access count
    pub access_count: u64,
    /// Relevance score (0.0 - 1.0)
    pub relevance: f64,
    /// Expiry timestamp (0 = never)
    pub expires_at: u64,
}

/// Session context for continuity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    /// Session ID
    pub session_id: String,
    /// Parent session ID (if resumed)
    pub parent_session: Option<String>,
    /// Session start time
    pub started_at: u64,
    /// Session end time (0 if ongoing)
    pub ended_at: u64,
    /// Working directory at start
    pub working_dir: String,
    /// Active project at start
    pub active_project: Option<String>,
    /// Summary of what was accomplished
    pub summary: String,
    /// Files modified
    pub files_modified: Vec<String>,
    /// Total complexity
    pub total_complexity: u64,
    /// Total actions
    pub total_actions: u64,
}

/// Agent preferences
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentPreferences {
    /// Preferred code style (e.g., "rust", "python")
    pub code_style: Option<String>,
    /// Preferred response length ("brief", "detailed", "adaptive")
    pub response_length: String,
    /// Whether to show thinking process
    pub show_thinking: bool,
    /// Preferred editor for large edits
    pub preferred_editor: Option<String>,
    /// Auto-save session context
    pub auto_save_context: bool,
    /// Maximum context entries to remember
    pub max_context_entries: usize,
    /// Custom key-value preferences
    pub custom: HashMap<String, String>,
}

/// LMDB-backed Agent state manager
pub struct AgentStateDb {
    env: Env,
    /// Memory storage: id -> MemoryEntry
    memory: Database<Str, SerdeBincode<MemoryEntry>>,
    /// Session history: session_id -> SessionContext
    sessions: Database<Str, SerdeBincode<SessionContext>>,
    /// Key-value state: key -> JSON value
    state: Database<Str, Str>,
    /// Tag index: "tag:tagname" -> list of memory IDs (JSON array)
    tags: Database<Str, Str>,
}

impl AgentStateDb {
    /// Open or create Agent state LMDB at given path
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAX_DB_SIZE)
                .max_dbs(8)
                .open(path)?
        };

        let mut wtxn = env.write_txn()?;
        let memory = env.create_database(&mut wtxn, Some("memory"))?;
        let sessions = env.create_database(&mut wtxn, Some("sessions"))?;
        let state = env.create_database(&mut wtxn, Some("state"))?;
        let tags = env.create_database(&mut wtxn, Some("tags"))?;
        wtxn.commit()?;

        log::info!("Agent State LMDB opened at {:?}", path);
        Ok(Self { env, memory, sessions, state, tags })
    }

    // ========================================================================
    // MEMORY OPERATIONS
    // ========================================================================

    /// Store a memory entry
    pub fn remember(&self, entry: MemoryEntry) -> Result<String> {
        let id = entry.id.clone();

        // Update tag index
        for tag in &entry.tags {
            self.add_to_tag_index(tag, &id)?;
        }

        let mut wtxn = self.env.write_txn()?;
        self.memory.put(&mut wtxn, &id, &entry)?;
        wtxn.commit()?;

        Ok(id)
    }

    /// Create and store a new memory
    pub fn create_memory(
        &self,
        content: &str,
        memory_type: MemoryType,
        tags: Vec<String>,
        source: &str,
    ) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let counter = MEMORY_COUNTER.fetch_add(1, Ordering::SeqCst);
        let id = format!("mem_{}_{}", now, counter);

        let entry = MemoryEntry {
            id: id.clone(),
            content: content.to_string(),
            memory_type,
            tags,
            source: source.to_string(),
            created_at: now,
            last_accessed: now,
            access_count: 0,
            relevance: 1.0,
            expires_at: match memory_type {
                MemoryType::Working => now + 86400, // 24 hours
                MemoryType::Pinned => 0,             // Never
                _ => now + 604800,                   // 7 days
            },
        };

        self.remember(entry)
    }

    /// Recall a memory by ID
    pub fn recall(&self, id: &str) -> Result<Option<MemoryEntry>> {
        let rtxn = self.env.read_txn()?;
        let entry = self.memory.get(&rtxn, id)?;
        drop(rtxn);

        // Update access stats
        if let Some(mut e) = entry.clone() {
            e.last_accessed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            e.access_count += 1;

            let mut wtxn = self.env.write_txn()?;
            self.memory.put(&mut wtxn, id, &e)?;
            wtxn.commit()?;
        }

        Ok(entry)
    }

    /// Search memories by content (simple substring match)
    pub fn search_memories(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.memory.iter(&rtxn)?;

        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        for result in iter {
            let (_, entry) = result?;
            if entry.content.to_lowercase().contains(&query_lower) {
                matches.push(entry);
                if matches.len() >= limit {
                    break;
                }
            }
        }

        // Sort by relevance * recency
        matches.sort_by(|a, b| {
            let score_a = a.relevance * (a.last_accessed as f64);
            let score_b = b.relevance * (b.last_accessed as f64);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(matches)
    }

    /// Get memories by tag
    pub fn get_by_tag(&self, tag: &str) -> Result<Vec<MemoryEntry>> {
        let key = format!("tag:{}", tag);
        let rtxn = self.env.read_txn()?;

        let ids: Vec<String> = match self.tags.get(&rtxn, &key)? {
            Some(json) => serde_json::from_str(json)?,
            None => return Ok(Vec::new()),
        };

        let mut entries = Vec::new();
        for id in ids {
            if let Some(entry) = self.memory.get(&rtxn, &id)? {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Get memories by type
    pub fn get_by_type(&self, memory_type: MemoryType) -> Result<Vec<MemoryEntry>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.memory.iter(&rtxn)?;

        let mut entries = Vec::new();
        for result in iter {
            let (_, entry) = result?;
            if entry.memory_type == memory_type {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Forget a memory
    pub fn forget(&self, id: &str) -> Result<bool> {
        // Remove from tag index
        if let Some(entry) = self.recall(id)? {
            for tag in &entry.tags {
                self.remove_from_tag_index(tag, id)?;
            }
        }

        let mut wtxn = self.env.write_txn()?;
        let deleted = self.memory.delete(&mut wtxn, id)?;
        wtxn.commit()?;
        Ok(deleted)
    }

    /// Expire old memories
    pub fn expire_memories(&self) -> Result<u64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let rtxn = self.env.read_txn()?;
        let iter = self.memory.iter(&rtxn)?;

        let mut to_delete = Vec::new();
        for result in iter {
            let (id, entry) = result?;
            if entry.expires_at > 0 && entry.expires_at < now {
                to_delete.push(id.to_string());
            }
        }
        drop(rtxn);

        let count = to_delete.len() as u64;
        for id in to_delete {
            self.forget(&id)?;
        }
        Ok(count)
    }

    // ========================================================================
    // TAG INDEX
    // ========================================================================

    fn add_to_tag_index(&self, tag: &str, id: &str) -> Result<()> {
        let key = format!("tag:{}", tag);
        let rtxn = self.env.read_txn()?;

        let mut ids: Vec<String> = match self.tags.get(&rtxn, &key)? {
            Some(json) => serde_json::from_str(json)?,
            None => Vec::new(),
        };
        drop(rtxn);

        if !ids.contains(&id.to_string()) {
            ids.push(id.to_string());
            let json = serde_json::to_string(&ids)?;

            let mut wtxn = self.env.write_txn()?;
            self.tags.put(&mut wtxn, &key, &json)?;
            wtxn.commit()?;
        }
        Ok(())
    }

    fn remove_from_tag_index(&self, tag: &str, id: &str) -> Result<()> {
        let key = format!("tag:{}", tag);
        let rtxn = self.env.read_txn()?;

        let mut ids: Vec<String> = match self.tags.get(&rtxn, &key)? {
            Some(json) => serde_json::from_str(json)?,
            None => return Ok(()),
        };
        drop(rtxn);

        ids.retain(|i| i != id);
        let json = serde_json::to_string(&ids)?;

        let mut wtxn = self.env.write_txn()?;
        self.tags.put(&mut wtxn, &key, &json)?;
        wtxn.commit()?;
        Ok(())
    }

    /// List all tags
    pub fn list_tags(&self) -> Result<Vec<String>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.tags.iter(&rtxn)?;

        let mut tags = Vec::new();
        for result in iter {
            let (key, _) = result?;
            if key.starts_with("tag:") {
                tags.push(key[4..].to_string());
            }
        }
        Ok(tags)
    }

    // ========================================================================
    // SESSION MANAGEMENT
    // ========================================================================

    /// Start a new session
    pub fn start_session(&self, session_id: &str, working_dir: &str) -> Result<SessionContext> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Check for parent session (most recent)
        let parent = self.get_latest_session()?.map(|s| s.session_id);

        let ctx = SessionContext {
            session_id: session_id.to_string(),
            parent_session: parent,
            started_at: now,
            ended_at: 0,
            working_dir: working_dir.to_string(),
            active_project: None,
            summary: String::new(),
            files_modified: Vec::new(),
            total_complexity: 0,
            total_actions: 0,
        };

        let mut wtxn = self.env.write_txn()?;
        self.sessions.put(&mut wtxn, session_id, &ctx)?;
        wtxn.commit()?;

        Ok(ctx)
    }

    /// End a session
    pub fn end_session(&self, session_id: &str, summary: &str) -> Result<()> {
        let rtxn = self.env.read_txn()?;
        let mut ctx = self.sessions.get(&rtxn, session_id)?
            .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;
        drop(rtxn);

        ctx.ended_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        ctx.summary = summary.to_string();

        let mut wtxn = self.env.write_txn()?;
        self.sessions.put(&mut wtxn, session_id, &ctx)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Update session context
    pub fn update_session(&self, ctx: &SessionContext) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.sessions.put(&mut wtxn, &ctx.session_id, ctx)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Get session by ID
    pub fn get_session(&self, session_id: &str) -> Result<Option<SessionContext>> {
        let rtxn = self.env.read_txn()?;
        Ok(self.sessions.get(&rtxn, session_id)?)
    }

    /// Get most recent session
    pub fn get_latest_session(&self) -> Result<Option<SessionContext>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.sessions.iter(&rtxn)?;

        let mut latest: Option<SessionContext> = None;
        for result in iter {
            let (_, ctx) = result?;
            if latest.as_ref().map_or(true, |l| ctx.started_at > l.started_at) {
                latest = Some(ctx);
            }
        }
        Ok(latest)
    }

    /// Get session chain (this session and all parents)
    pub fn get_session_chain(&self, session_id: &str) -> Result<Vec<SessionContext>> {
        let mut chain = Vec::new();
        let mut current = session_id.to_string();

        while let Some(ctx) = self.get_session(&current)? {
            let parent = ctx.parent_session.clone();
            chain.push(ctx);
            match parent {
                Some(p) => current = p,
                None => break,
            }
        }

        Ok(chain)
    }

    /// Record file modification in session
    pub fn record_file_modified(&self, session_id: &str, file_path: &str) -> Result<()> {
        let rtxn = self.env.read_txn()?;
        let mut ctx = self.sessions.get(&rtxn, session_id)?
            .ok_or_else(|| anyhow!("Session not found"))?;
        drop(rtxn);

        if !ctx.files_modified.contains(&file_path.to_string()) {
            ctx.files_modified.push(file_path.to_string());
            self.update_session(&ctx)?;
        }
        Ok(())
    }

    /// Increment session counters
    pub fn increment_session_stats(&self, session_id: &str, complexity: u64) -> Result<()> {
        let rtxn = self.env.read_txn()?;
        let mut ctx = self.sessions.get(&rtxn, session_id)?
            .ok_or_else(|| anyhow!("Session not found"))?;
        drop(rtxn);

        ctx.total_complexity += complexity;
        ctx.total_actions += 1;
        self.update_session(&ctx)
    }

    // ========================================================================
    // STATE (Key-Value)
    // ========================================================================

    /// Get a state value
    pub fn get_state(&self, key: &str) -> Result<Option<String>> {
        let rtxn = self.env.read_txn()?;
        Ok(self.state.get(&rtxn, key)?.map(|s| s.to_string()))
    }

    /// Set a state value
    pub fn set_state(&self, key: &str, value: &str) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.state.put(&mut wtxn, key, value)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Get typed state value
    pub fn get_state_typed<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        match self.get_state(key)? {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    /// Set typed state value
    pub fn set_state_typed<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.set_state(key, &json)
    }

    /// Delete a state key
    pub fn delete_state(&self, key: &str) -> Result<bool> {
        let mut wtxn = self.env.write_txn()?;
        let deleted = self.state.delete(&mut wtxn, key)?;
        wtxn.commit()?;
        Ok(deleted)
    }

    /// List all state keys
    pub fn list_state_keys(&self) -> Result<Vec<String>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.state.iter(&rtxn)?;

        let mut keys = Vec::new();
        for result in iter {
            let (key, _) = result?;
            keys.push(key.to_string());
        }
        Ok(keys)
    }

    // ========================================================================
    // PREFERENCES
    // ========================================================================

    /// Get Agent preferences
    pub fn get_preferences(&self) -> Result<AgentPreferences> {
        self.get_state_typed::<AgentPreferences>("preferences")?
            .ok_or_else(|| anyhow!("Preferences not initialized"))
            .or_else(|_| Ok(AgentPreferences::default()))
    }

    /// Set Agent preferences
    pub fn set_preferences(&self, prefs: &AgentPreferences) -> Result<()> {
        self.set_state_typed("preferences", prefs)
    }

    /// Update a single preference
    pub fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        let mut prefs = self.get_preferences()?;
        prefs.custom.insert(key.to_string(), value.to_string());
        self.set_preferences(&prefs)
    }

    // ========================================================================
    // INITIALIZATION
    // ========================================================================

    /// Initialize with defaults
    pub fn init_defaults(&self) -> Result<()> {
        // Only init if not already initialized
        if self.get_state("initialized")?.is_some() {
            return Ok(());
        }

        // Default preferences
        self.set_preferences(&AgentPreferences {
            code_style: None,
            response_length: "adaptive".to_string(),
            show_thinking: false,
            preferred_editor: None,
            auto_save_context: true,
            max_context_entries: 100,
            custom: HashMap::new(),
        })?;

        // Initial memories
        self.create_memory(
            "SPF Smart Gateway provides AI self-governance with complexity-based enforcement",
            MemoryType::Fact,
            vec!["spf".to_string(), "system".to_string()],
            "system",
        )?;

        self.create_memory(
            "User prefers concise responses without emojis unless requested",
            MemoryType::Preference,
            vec!["style".to_string()],
            "system",
        )?;

        self.set_state("initialized", "true")?;
        self.set_state("version", "1.0.0")?;

        log::info!("Agent State LMDB initialized with defaults");
        Ok(())
    }

    /// Get context summary for session start
    pub fn get_context_summary(&self) -> Result<String> {
        let mut summary = String::new();

        // Last session info
        if let Some(last) = self.get_latest_session()? {
            if !last.summary.is_empty() {
                summary.push_str(&format!("Last session: {}\n", last.summary));
            }
            if !last.files_modified.is_empty() {
                summary.push_str(&format!(
                    "Files modified: {}\n",
                    last.files_modified.len()
                ));
            }
        }

        // Active instructions
        let instructions = self.get_by_type(MemoryType::Instruction)?;
        if !instructions.is_empty() {
            summary.push_str("\nActive instructions:\n");
            for inst in instructions.iter().take(5) {
                summary.push_str(&format!("- {}\n", inst.content));
            }
        }

        // Recent context
        let context = self.get_by_type(MemoryType::Context)?;
        if !context.is_empty() {
            summary.push_str("\nRecent context:\n");
            for ctx in context.iter().take(3) {
                summary.push_str(&format!("- {}\n", ctx.content));
            }
        }

        Ok(summary)
    }

    /// Get database stats
    pub fn db_stats(&self) -> Result<(u64, u64, u64, u64)> {
        let rtxn = self.env.read_txn()?;
        let memory_stat = self.memory.stat(&rtxn)?;
        let sessions_stat = self.sessions.stat(&rtxn)?;
        let state_stat = self.state.stat(&rtxn)?;
        let tags_stat = self.tags.stat(&rtxn)?;
        Ok((
            memory_stat.entries as u64,
            sessions_stat.entries as u64,
            state_stat.entries as u64,
            tags_stat.entries as u64,
        ))
    }
}
