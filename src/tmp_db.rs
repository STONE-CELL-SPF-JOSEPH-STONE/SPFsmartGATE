// SPF Smart Gateway - TMP LMDB
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// LMDB-backed metadata for /tmp and /projects device directories.
// Tracks file access logs, resource usage, and project isolation.
//
// Database: TMP_DB
// Storage: ~/SPFsmartGATE/LIVE/TMP/TMP.DB/

use anyhow::{anyhow, Result};
use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_DB_SIZE: usize = 50 * 1024 * 1024; // 50MB

/// Project trust level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustLevel {
    /// Untrusted - maximum restrictions
    Untrusted = 0,
    /// Low trust - basic operations only
    Low = 1,
    /// Medium trust - most operations allowed with prompts
    Medium = 2,
    /// High trust - operations allowed with minimal prompts
    High = 3,
    /// Full trust - all operations allowed (user's own project)
    Full = 4,
}

impl Default for TrustLevel {
    fn default() -> Self {
        TrustLevel::Low
    }
}

/// Project entry — tracked in TMP_DB LMDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Project root path (canonical)
    pub path: String,
    /// Display name for the project
    pub name: String,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Tools explicitly allowed for this project
    pub allowed_tools: Vec<String>,
    /// Tools explicitly denied for this project
    pub denied_tools: Vec<String>,
    /// Paths within project that are write-protected
    pub protected_paths: Vec<String>,
    /// Maximum file size for writes (bytes)
    pub max_write_size: usize,
    /// Maximum total writes per session
    pub max_writes_per_session: u32,
    /// Current session write count
    pub session_writes: u32,
    /// Total files accessed (read)
    pub total_reads: u64,
    /// Total files modified (write/edit)
    pub total_writes: u64,
    /// Total complexity accumulated
    pub total_complexity: u64,
    /// Created timestamp
    pub created_at: u64,
    /// Last accessed timestamp
    pub last_accessed: u64,
    /// Whether project requires explicit activation
    pub requires_activation: bool,
    /// Whether project is currently active
    pub is_active: bool,
    /// User notes about this project
    pub notes: String,
}

/// File access record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAccess {
    /// File path (relative to project root)
    pub path: String,
    /// Project this file belongs to
    pub project: String,
    /// Access type: "read", "write", "edit", "delete"
    pub access_type: String,
    /// Timestamp
    pub timestamp: u64,
    /// Session ID
    pub session_id: String,
    /// File size at access time
    pub file_size: u64,
    /// Whether access was allowed
    pub allowed: bool,
    /// Reason if denied
    pub deny_reason: Option<String>,
}

/// Resource usage for a project
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUsage {
    /// Total bytes read
    pub bytes_read: u64,
    /// Total bytes written
    pub bytes_written: u64,
    /// Total files created
    pub files_created: u64,
    /// Total files deleted
    pub files_deleted: u64,
    /// Total bash commands run
    pub bash_commands: u64,
    /// Total web requests
    pub web_requests: u64,
}

/// LMDB-backed project manager
pub struct SpfTmpDb {
    env: Env,
    /// Project registry: canonical_path → Project
    projects: Database<Str, SerdeBincode<Project>>,
    /// File access log: "timestamp:project:path" → FileAccess
    access_log: Database<Str, SerdeBincode<FileAccess>>,
    /// Resource usage: project_path → ResourceUsage
    resources: Database<Str, SerdeBincode<ResourceUsage>>,
    /// Active project marker: "active" → project_path
    active: Database<Str, Str>,
}

impl SpfTmpDb {
    /// Open or create project LMDB at given path
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAX_DB_SIZE)
                .max_dbs(8)
                .open(path)?
        };

        let mut wtxn = env.write_txn()?;
        let projects = env.create_database(&mut wtxn, Some("projects"))?;
        let access_log = env.create_database(&mut wtxn, Some("access_log"))?;
        let resources = env.create_database(&mut wtxn, Some("resources"))?;
        let active = env.create_database(&mut wtxn, Some("active"))?;
        wtxn.commit()?;

        log::info!("TMP_DB LMDB opened at {:?}", path);
        Ok(Self { env, projects, access_log, resources, active })
    }

    // ========================================================================
    // PROJECT MANAGEMENT
    // ========================================================================

    /// Register a new project project
    pub fn register_project(&self, path: &str, name: &str, trust_level: TrustLevel) -> Result<Project> {
        let canonical = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let project = Project {
            path: canonical.clone(),
            name: name.to_string(),
            trust_level,
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            protected_paths: vec![".git".to_string(), ".env".to_string()],
            max_write_size: 100_000,
            max_writes_per_session: 100,
            session_writes: 0,
            total_reads: 0,
            total_writes: 0,
            total_complexity: 0,
            created_at: now,
            last_accessed: now,
            requires_activation: trust_level < TrustLevel::High,
            is_active: false,
            notes: String::new(),
        };

        let mut wtxn = self.env.write_txn()?;
        self.projects.put(&mut wtxn, &canonical, &project)?;
        self.resources.put(&mut wtxn, &canonical, &ResourceUsage::default())?;
        wtxn.commit()?;

        Ok(project)
    }

    /// Get a project project
    pub fn get_project(&self, path: &str) -> Result<Option<Project>> {
        let canonical = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());

        let rtxn = self.env.read_txn()?;
        Ok(self.projects.get(&rtxn, &canonical)?)
    }

    /// Update a project project
    pub fn update_project(&self, project: &Project) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.projects.put(&mut wtxn, &project.path, project)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Find project containing a file path
    pub fn find_project_for_path(&self, file_path: &str) -> Result<Option<Project>> {
        let canonical = std::fs::canonicalize(file_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string());

        let rtxn = self.env.read_txn()?;
        let iter = self.projects.iter(&rtxn)?;

        // Find the most specific (longest) matching project path
        let mut best_match: Option<Project> = None;
        let mut best_len = 0;

        for result in iter {
            let (project_path, project) = result?;
            if canonical.starts_with(project_path) && project_path.len() > best_len {
                best_match = Some(project);
                best_len = project_path.len();
            }
        }

        Ok(best_match)
    }

    /// List all registered projects
    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.projects.iter(&rtxn)?;

        let mut projects = Vec::new();
        for result in iter {
            let (_, project) = result?;
            projects.push(project);
        }
        Ok(projects)
    }

    /// Delete a project
    pub fn delete_project(&self, path: &str) -> Result<bool> {
        let canonical = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());

        let mut wtxn = self.env.write_txn()?;
        let deleted = self.projects.delete(&mut wtxn, &canonical)?;
        self.resources.delete(&mut wtxn, &canonical)?;
        wtxn.commit()?;
        Ok(deleted)
    }

    // ========================================================================
    // TRUST & PERMISSIONS
    // ========================================================================

    /// Set project trust level
    pub fn set_trust_level(&self, path: &str, level: TrustLevel) -> Result<()> {
        let mut project = self.get_project(path)?
            .ok_or_else(|| anyhow!("Project not found: {}", path))?;
        project.trust_level = level;
        project.requires_activation = level < TrustLevel::High;
        self.update_project(&project)
    }

    /// Check if a tool is allowed for a project
    pub fn is_tool_allowed(&self, project_path: &str, tool: &str) -> Result<bool> {
        let project = match self.get_project(project_path)? {
            Some(s) => s,
            None => return Ok(true), // No project = no restrictions
        };

        // Explicit deny takes precedence
        if project.denied_tools.contains(&tool.to_string()) {
            return Ok(false);
        }

        // Explicit allow
        if project.allowed_tools.contains(&tool.to_string()) {
            return Ok(true);
        }

        // Trust-level based default
        Ok(match project.trust_level {
            TrustLevel::Untrusted => false,
            TrustLevel::Low => matches!(tool, "Read" | "Glob" | "Grep"),
            TrustLevel::Medium => !matches!(tool, "Bash"),
            TrustLevel::High | TrustLevel::Full => true,
        })
    }

    /// Check if a path within project is protected
    pub fn is_path_protected(&self, project_path: &str, file_path: &str) -> Result<bool> {
        let project = match self.get_project(project_path)? {
            Some(s) => s,
            None => return Ok(false),
        };

        // Get relative path
        let relative = file_path.strip_prefix(&project.path)
            .unwrap_or(file_path)
            .trim_start_matches('/');

        for protected in &project.protected_paths {
            if relative.starts_with(protected) || relative == *protected {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Add a protected path to a project
    pub fn add_protected_path(&self, project_path: &str, protected: &str) -> Result<()> {
        let mut project = self.get_project(project_path)?
            .ok_or_else(|| anyhow!("Project not found: {}", project_path))?;

        if !project.protected_paths.contains(&protected.to_string()) {
            project.protected_paths.push(protected.to_string());
            self.update_project(&project)?;
        }
        Ok(())
    }

    // ========================================================================
    // ACTIVE PROJECT
    // ========================================================================

    /// Set the currently active project
    pub fn set_active(&self, path: &str) -> Result<()> {
        let canonical = std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string());

        // Deactivate current
        if let Some(current) = self.get_active()? {
            let mut project = self.get_project(&current)?
                .ok_or_else(|| anyhow!("Active project not found"))?;
            project.is_active = false;
            self.update_project(&project)?;
        }

        // Activate new
        let mut project = self.get_project(&canonical)?
            .ok_or_else(|| anyhow!("Project not found: {}", canonical))?;
        project.is_active = true;
        project.last_accessed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.update_project(&project)?;

        let mut wtxn = self.env.write_txn()?;
        self.active.put(&mut wtxn, "active", &canonical)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Get the currently active project path
    pub fn get_active(&self) -> Result<Option<String>> {
        let rtxn = self.env.read_txn()?;
        Ok(self.active.get(&rtxn, "active")?.map(|s| s.to_string()))
    }

    /// Clear active project
    pub fn clear_active(&self) -> Result<()> {
        if let Some(current) = self.get_active()? {
            if let Some(mut project) = self.get_project(&current)? {
                project.is_active = false;
                self.update_project(&project)?;
            }
        }
        let mut wtxn = self.env.write_txn()?;
        self.active.delete(&mut wtxn, "active")?;
        wtxn.commit()?;
        Ok(())
    }

    // ========================================================================
    // ACCESS LOGGING
    // ========================================================================

    /// Log a file access
    pub fn log_access(
        &self,
        file_path: &str,
        project_path: &str,
        access_type: &str,
        session_id: &str,
        file_size: u64,
        allowed: bool,
        deny_reason: Option<&str>,
    ) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let access = FileAccess {
            path: file_path.to_string(),
            project: project_path.to_string(),
            access_type: access_type.to_string(),
            timestamp: now,
            session_id: session_id.to_string(),
            file_size,
            allowed,
            deny_reason: deny_reason.map(|s| s.to_string()),
        };

        let key = format!("{}:{}:{}", now, project_path, file_path);
        let mut wtxn = self.env.write_txn()?;
        self.access_log.put(&mut wtxn, &key, &access)?;
        wtxn.commit()?;

        // Update project stats
        if let Some(mut project) = self.get_project(project_path)? {
            if allowed {
                match access_type {
                    "read" => project.total_reads += 1,
                    "write" | "edit" | "delete" => {
                        project.total_writes += 1;
                        project.session_writes += 1;
                    }
                    _ => {}
                }
            }
            project.last_accessed = now;
            self.update_project(&project)?;
        }

        // Update resource usage
        if allowed {
            self.update_resources(project_path, access_type, file_size)?;
        }

        Ok(())
    }

    /// Get recent access log for a project
    pub fn get_access_log(&self, project_path: &str, limit: usize) -> Result<Vec<FileAccess>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.access_log.rev_iter(&rtxn)?;

        let mut log = Vec::new();
        for result in iter {
            let (_, access) = result?;
            if access.project == project_path {
                log.push(access);
                if log.len() >= limit {
                    break;
                }
            }
        }
        Ok(log)
    }

    /// Prune access log older than N seconds
    pub fn prune_access_log(&self, max_age_secs: u64) -> Result<u64> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff = now.saturating_sub(max_age_secs);

        let rtxn = self.env.read_txn()?;
        let iter = self.access_log.iter(&rtxn)?;

        let mut to_delete = Vec::new();
        for result in iter {
            let (key, access) = result?;
            if access.timestamp < cutoff {
                to_delete.push(key.to_string());
            }
        }
        drop(rtxn);

        let count = to_delete.len() as u64;
        let mut wtxn = self.env.write_txn()?;
        for key in to_delete {
            self.access_log.delete(&mut wtxn, &key)?;
        }
        wtxn.commit()?;

        Ok(count)
    }

    // ========================================================================
    // RESOURCE TRACKING
    // ========================================================================

    fn update_resources(&self, project_path: &str, access_type: &str, size: u64) -> Result<()> {
        let rtxn = self.env.read_txn()?;
        let mut usage = self.resources.get(&rtxn, project_path)?
            .unwrap_or_default();
        drop(rtxn);

        match access_type {
            "read" => usage.bytes_read += size,
            "write" => {
                usage.bytes_written += size;
                usage.files_created += 1;
            }
            "edit" => usage.bytes_written += size,
            "delete" => usage.files_deleted += 1,
            "bash" => usage.bash_commands += 1,
            "web" => usage.web_requests += 1,
            _ => {}
        }

        let mut wtxn = self.env.write_txn()?;
        self.resources.put(&mut wtxn, project_path, &usage)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Get resource usage for a project
    pub fn get_resources(&self, project_path: &str) -> Result<ResourceUsage> {
        let rtxn = self.env.read_txn()?;
        Ok(self.resources.get(&rtxn, project_path)?.unwrap_or_default())
    }

    /// Reset session counters (call at session start)
    pub fn reset_session_counters(&self) -> Result<()> {
        let projects = self.list_projects()?;
        for mut project in projects {
            project.session_writes = 0;
            self.update_project(&project)?;
        }
        Ok(())
    }

    // ========================================================================
    // VALIDATION
    // ========================================================================

    /// Validate a file operation against project rules
    pub fn validate_operation(
        &self,
        file_path: &str,
        operation: &str,
        size: u64,
    ) -> Result<(bool, Option<String>)> {
        // Find containing project
        let project = match self.find_project_for_path(file_path)? {
            Some(s) => s,
            None => return Ok((true, None)), // No project = allowed
        };

        // Check if project is active (if required)
        if project.requires_activation && !project.is_active {
            return Ok((false, Some(format!(
                "Project '{}' requires activation before file operations",
                project.name
            ))));
        }

        // Check trust level for write operations
        if matches!(operation, "write" | "edit" | "delete") {
            if project.trust_level == TrustLevel::Untrusted {
                return Ok((false, Some("Untrusted project: write operations denied".to_string())));
            }

            // Check protected paths
            if self.is_path_protected(&project.path, file_path)? {
                return Ok((false, Some(format!(
                    "Path is protected in project '{}'",
                    project.name
                ))));
            }

            // Check write size limit
            if size > project.max_write_size as u64 {
                return Ok((false, Some(format!(
                    "File size {} exceeds project limit {}",
                    size, project.max_write_size
                ))));
            }

            // Check session write limit
            if project.session_writes >= project.max_writes_per_session {
                return Ok((false, Some(format!(
                    "Session write limit ({}) reached for project '{}'",
                    project.max_writes_per_session, project.name
                ))));
            }
        }

        Ok((true, None))
    }

    /// Get database stats
    pub fn db_stats(&self) -> Result<(u64, u64, u64)> {
        let rtxn = self.env.read_txn()?;
        let projects_stat = self.projects.stat(&rtxn)?;
        let access_stat = self.access_log.stat(&rtxn)?;
        let resources_stat = self.resources.stat(&rtxn)?;
        Ok((projects_stat.entries as u64, access_stat.entries as u64, resources_stat.entries as u64))
    }
}
