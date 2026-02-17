// SPF Smart Gateway - Session State
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// In-memory session state. Persisted to LMDB on checkpoints.
// Tracks: action_count, files_read, files_written, complexity history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Active session state — lives in RAM, flushed to LMDB periodically
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub action_count: u64,
    pub files_read: Vec<String>,
    pub files_written: Vec<String>,
    pub last_tool: Option<String>,
    pub last_result: Option<String>,
    pub last_file: Option<String>,
    pub started: DateTime<Utc>,
    pub last_action: Option<DateTime<Utc>>,
    pub complexity_history: Vec<ComplexityEntry>,
    pub manifest: Vec<ManifestEntry>,
    pub failures: Vec<FailureEntry>,
    /// Per-minute action timestamps for rate limiting (circular buffer)
    #[serde(default)]
    pub rate_window: Vec<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityEntry {
    pub timestamp: DateTime<Utc>,
    pub tool: String,
    pub c: u64,
    pub tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub timestamp: DateTime<Utc>,
    pub tool: String,
    pub c: u64,
    pub action: String, // "ALLOWED" or "BLOCKED"
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureEntry {
    pub timestamp: DateTime<Utc>,
    pub tool: String,
    pub error: String,
}

impl Session {
    pub fn new() -> Self {
        Self {
            action_count: 0,
            files_read: Vec::new(),
            files_written: Vec::new(),
            last_tool: None,
            last_result: None,
            last_file: None,
            started: Utc::now(),
            last_action: None,
            complexity_history: Vec::new(),
            manifest: Vec::new(),
            failures: Vec::new(),
            rate_window: Vec::new(),
        }
    }

    /// Track a file read for Build Anchor Protocol
    pub fn track_read(&mut self, path: &str) {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => {
                if path.contains("..") {
                    let flagged = format!("[TRAVERSAL REJECTED] {}", path);
                    if !self.files_read.contains(&flagged) {
                        self.files_read.push(flagged);
                    }
                    return;
                }
                path.to_string()
            }
        };
        if !self.files_read.contains(&canonical) {
            self.files_read.push(canonical);
        }
    }

    /// Track a file write
    pub fn track_write(&mut self, path: &str) {
        let canonical = match std::fs::canonicalize(path) {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => {
                if path.contains("..") {
                    let flagged = format!("[TRAVERSAL REJECTED] {}", path);
                    if !self.files_written.contains(&flagged) {
                        self.files_written.push(flagged);
                    }
                    return;
                }
                path.to_string()
            }
        };
        if !self.files_written.contains(&canonical) {
            self.files_written.push(canonical);
        }
    }

    /// Record an action (called after every tool use)
    pub fn record_action(&mut self, tool: &str, result: &str, file_path: Option<&str>) {
        self.action_count += 1;
        self.last_tool = Some(tool.to_string());
        self.last_result = Some(result.to_string());
        self.last_file = file_path.map(|s| s.to_string());
        let now = Utc::now();
        self.last_action = Some(now);

        // Record timestamp for rate limiting and prune expired entries
        self.rate_window.push(now);
        let one_minute_ago = now - chrono::Duration::seconds(60);
        self.rate_window.retain(|ts| *ts > one_minute_ago);
    }

    /// Record complexity calculation
    pub fn record_complexity(&mut self, tool: &str, c: u64, tier: &str) {
        self.complexity_history.push(ComplexityEntry {
            timestamp: Utc::now(),
            tool: tool.to_string(),
            c,
            tier: tier.to_string(),
        });
        // Keep last 100 entries
        if self.complexity_history.len() > 100 {
            self.complexity_history.remove(0);
        }
    }

    /// Record manifest entry (allowed/blocked)
    pub fn record_manifest(&mut self, tool: &str, c: u64, action: &str, reason: Option<&str>) {
        self.manifest.push(ManifestEntry {
            timestamp: Utc::now(),
            tool: tool.to_string(),
            c,
            action: action.to_string(),
            reason: reason.map(|s| s.to_string()),
        });
        if self.manifest.len() > 200 {
            self.manifest.remove(0);
        }
    }

    /// Record failure
    pub fn record_failure(&mut self, tool: &str, error: &str) {
        self.failures.push(FailureEntry {
            timestamp: Utc::now(),
            tool: tool.to_string(),
            error: error.to_string(),
        });
        if self.failures.len() > 50 {
            self.failures.remove(0);
        }
    }

    /// Build Anchor ratio: reads / writes
    pub fn anchor_ratio(&self) -> String {
        if self.files_written.is_empty() {
            "N/A (no writes)".to_string()
        } else {
            format!("{}/{}", self.files_read.len(), self.files_written.len())
        }
    }

    /// Status summary string
    pub fn status_summary(&self) -> String {
        format!(
            "Actions: {} | Reads: {} | Writes: {} | Last: {} | Anchor: {}",
            self.action_count,
            self.files_read.len(),
            self.files_written.len(),
            self.last_tool.as_deref().unwrap_or("none"),
            self.anchor_ratio(),
        )
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}
