// SPF Smart Gateway - LMDB Filesystem
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// Real filesystem backed by LMDB using heed.
// Provides: read, write, mkdir, ls, rm, stat, rename
// Hybrid storage: small files in LMDB, large files on disk.
// All operations gated through SPF complexity formula.

use anyhow::{anyhow, Result};
use heed::types::{SerdeBincode, Str, Bytes};
use heed::{Database, Env, EnvOpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// CONSTANTS
// ============================================================================

const MAX_INLINE_SIZE: usize = 1_048_576; // 1MB - files larger go to disk
const MAP_SIZE: usize = 4 * 1024 * 1024 * 1024; // 4GB
const MAX_DBS: u32 = 8;

// ============================================================================
// TYPES
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub file_type: FileType,
    pub size: u64,
    pub mode: u32,
    pub created_at: i64,
    pub modified_at: i64,
    pub checksum: Option<String>,
    pub version: u64,
    pub vector_id: Option<String>,
    pub real_path: Option<String>,
}

impl FileMetadata {
    pub fn new_file(size: u64) -> Self {
        let now = unix_now();
        Self {
            file_type: FileType::File,
            size,
            mode: 0o644,
            created_at: now,
            modified_at: now,
            checksum: None,
            version: 1,
            vector_id: None,
            real_path: None,
        }
    }

    pub fn new_dir() -> Self {
        let now = unix_now();
        Self {
            file_type: FileType::Directory,
            size: 0,
            mode: 0o755,
            created_at: now,
            modified_at: now,
            checksum: None,
            version: 1,
            vector_id: None,
            real_path: None,
        }
    }
}

// ============================================================================
// SPF FILESYSTEM
// ============================================================================

pub struct SpfFs {
    env: Env,
    metadata: Database<Str, SerdeBincode<FileMetadata>>,
    content: Database<Str, Bytes>,
    index: Database<Str, Str>,
    blob_dir: PathBuf,
}

impl SpfFs {
    /// Open or create the LMDB filesystem at the given path
    pub fn open(storage_path: &Path) -> Result<Self> {
        let fs_path = storage_path.join("SPF_FS.DB");
        let blob_dir = storage_path.join("blobs");

        std::fs::create_dir_all(&fs_path)?;
        std::fs::create_dir_all(&blob_dir)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAP_SIZE)
                .max_dbs(MAX_DBS)
                .open(&fs_path)?
        };

        let mut wtxn = env.write_txn()?;
        let metadata = env.create_database(&mut wtxn, Some("fs_metadata"))?;
        let content = env.create_database(&mut wtxn, Some("fs_content"))?;
        let index = env.create_database(&mut wtxn, Some("fs_index"))?;
        wtxn.commit()?;

        let fs = Self { env, metadata, content, index, blob_dir };

        // Initialize root structure if empty
        if !fs.exists("/")? {
            fs.init_structure()?;
        }

        log::info!("SPF FS opened at {:?}", fs_path);
        Ok(fs)
    }

    /// Initialize the virtual filesystem structure
    fn init_structure(&self) -> Result<()> {
        log::info!("Initializing SPF FS structure...");

        // Create root directories — mount point stubs per build spec
        self.mkdir_internal("/")?;
        self.mkdir_internal("/system")?;        // LMDB 1 — read-only system
        self.mkdir_internal("/config")?;        // mount → LMDB 2
        self.mkdir_internal("/tools")?;         // legacy — no active mount
        self.mkdir_internal("/tmp")?;            // mount → LMDB 4 (writable TMP)
        self.mkdir_internal("/home")?;
        self.mkdir_internal("/home/agent")?;    // mount → LMDB 5
        // /home/agent/ full tree (build spec lines 1230-1249 + containment spec)
        self.mkdir_internal("/home/agent/.claude")?;
        self.mkdir_internal("/home/agent/.claude/projects")?;
        self.mkdir_internal("/home/agent/.claude/file-history")?;
        self.mkdir_internal("/home/agent/.claude/paste-cache")?;
        self.mkdir_internal("/home/agent/.claude/session-env")?;
        self.mkdir_internal("/home/agent/.claude/todos")?;
        self.mkdir_internal("/home/agent/.claude/plans")?;
        self.mkdir_internal("/home/agent/.claude/tasks")?;
        self.mkdir_internal("/home/agent/.claude/shell-snapshots")?;
        self.mkdir_internal("/home/agent/.claude/statsig")?;
        self.mkdir_internal("/home/agent/.claude/telemetry")?;
        self.mkdir_internal("/home/agent/bin")?;
        self.mkdir_internal("/home/agent/bin/claude-code")?;
        self.mkdir_internal("/home/agent/tmp")?;            // routes to /tmp (LMDB 4)
        self.mkdir_internal("/home/agent/.config")?;
        self.mkdir_internal("/home/agent/.config/settings")?;
        self.mkdir_internal("/home/agent/.local")?;
        self.mkdir_internal("/home/agent/.local/bin")?;
        self.mkdir_internal("/home/agent/.local/share")?;
        self.mkdir_internal("/home/agent/.local/share/history")?;
        self.mkdir_internal("/home/agent/.local/share/data")?;
        self.mkdir_internal("/home/agent/.local/state")?;
        self.mkdir_internal("/home/agent/.local/state/sessions")?;
        self.mkdir_internal("/home/agent/.cache")?;
        self.mkdir_internal("/home/agent/.cache/context")?;
        self.mkdir_internal("/home/agent/.cache/tmp")?;
        self.mkdir_internal("/home/agent/.memory")?;
        self.mkdir_internal("/home/agent/.memory/facts")?;
        self.mkdir_internal("/home/agent/.memory/instructions")?;
        self.mkdir_internal("/home/agent/.memory/preferences")?;
        self.mkdir_internal("/home/agent/.memory/pinned")?;
        self.mkdir_internal("/home/agent/.ssh")?;
        self.mkdir_internal("/home/agent/Documents")?;
        self.mkdir_internal("/home/agent/Documents/notes")?;
        self.mkdir_internal("/home/agent/Documents/templates")?;
        self.mkdir_internal("/home/agent/Projects")?;       // future: PROJECTS LMDB gateway
        self.mkdir_internal("/home/agent/workspace")?;
        self.mkdir_internal("/home/agent/workspace/current")?;

        log::info!("SPF FS structure initialized");
        Ok(())
    }

    /// Internal mkdir without parent creation
    fn mkdir_internal(&self, path: &str) -> Result<()> {
        let path = normalize_path(path);
        let mut wtxn = self.env.write_txn()?;
        self.metadata.put(&mut wtxn, &path, &FileMetadata::new_dir())?;
        wtxn.commit()?;
        Ok(())
    }

    // ========================================================================
    // CORE OPERATIONS
    // ========================================================================

    /// Check if path exists
    pub fn exists(&self, path: &str) -> Result<bool> {
        let path = normalize_path(path);
        let rtxn = self.env.read_txn()?;
        Ok(self.metadata.get(&rtxn, &path)?.is_some())
    }

    /// Get file/directory metadata
    pub fn stat(&self, path: &str) -> Result<Option<FileMetadata>> {
        let path = normalize_path(path);
        let rtxn = self.env.read_txn()?;
        Ok(self.metadata.get(&rtxn, &path)?)
    }

    /// Read file content
    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let path = normalize_path(path);
        let rtxn = self.env.read_txn()?;

        let meta = self.metadata.get(&rtxn, &path)?
            .ok_or_else(|| anyhow!("File not found: {}", path))?;

        if meta.file_type != FileType::File {
            return Err(anyhow!("Not a file: {}", path));
        }

        // Hybrid: check if content is on disk
        if let Some(ref real_path) = meta.real_path {
            return Ok(std::fs::read(real_path)?);
        }

        // Content is in LMDB
        let content = self.content.get(&rtxn, &path)?
            .ok_or_else(|| anyhow!("Content missing for: {}", path))?;

        Ok(content.to_vec())
    }

    /// Write file content (creates parent directories if needed)
    pub fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let path = normalize_path(path);

        // Ensure parent directories exist
        if let Some(parent) = parent_path(&path) {
            self.mkdir_p(&parent)?;
        }

        let checksum = sha256_hex(data);
        let size = data.len() as u64;

        let mut meta = self.stat(&path)?.unwrap_or_else(|| FileMetadata::new_file(size));
        meta.size = size;
        meta.modified_at = unix_now();
        meta.checksum = Some(checksum.clone());
        meta.version += 1;
        meta.file_type = FileType::File;

        let mut wtxn = self.env.write_txn()?;

        // Hybrid storage: large files go to disk
        if data.len() > MAX_INLINE_SIZE {
            let blob_path = self.blob_dir.join(&checksum);

            // Write blob with cleanup on failure (handles disk full)
            if let Err(e) = std::fs::write(&blob_path, data) {
                let _ = std::fs::remove_file(&blob_path);
                return Err(anyhow!("Failed to write blob (disk full?): {}", e));
            }

            meta.real_path = Some(blob_path.to_string_lossy().to_string());
            // Don't store content in LMDB
            let _ = self.content.delete(&mut wtxn, &path);
        } else {
            meta.real_path = None;
            self.content.put(&mut wtxn, &path, data)?;
        }

        self.metadata.put(&mut wtxn, &path, &meta)?;
        wtxn.commit()?;

        Ok(())
    }

    /// Create directory (single level)
    pub fn mkdir(&self, path: &str) -> Result<()> {
        let path = normalize_path(path);

        if self.exists(&path)? {
            return Err(anyhow!("Already exists: {}", path));
        }

        // Ensure parent exists
        if let Some(parent) = parent_path(&path) {
            if !self.exists(&parent)? {
                return Err(anyhow!("Parent directory does not exist: {}", parent));
            }
        }

        self.mkdir_internal(&path)
    }

    /// Create directory and all parents (mkdir -p)
    pub fn mkdir_p(&self, path: &str) -> Result<()> {
        let path = normalize_path(path);

        if self.exists(&path)? {
            return Ok(());
        }

        // Build path components and create each
        let mut current = String::new();
        for component in path.split('/').filter(|s| !s.is_empty()) {
            current.push('/');
            current.push_str(component);

            if !self.exists(&current)? {
                self.mkdir_internal(&current)?;
            }
        }

        Ok(())
    }

    /// List directory contents
    pub fn ls(&self, path: &str) -> Result<Vec<(String, FileMetadata)>> {
        let path = normalize_path(path);
        let rtxn = self.env.read_txn()?;

        // Verify it's a directory
        let meta = self.metadata.get(&rtxn, &path)?
            .ok_or_else(|| anyhow!("Directory not found: {}", path))?;

        if meta.file_type != FileType::Directory {
            return Err(anyhow!("Not a directory: {}", path));
        }

        // Prefix scan for children
        let prefix = if path == "/" { "/".to_string() } else { format!("{}/", path) };
        let depth = prefix.matches('/').count();

        let mut results = Vec::new();
        let mut seen = HashSet::new();

        let iter = self.metadata.iter(&rtxn)?;
        for item in iter {
            let (key, value) = item?;

            // Check if this is a direct child
            if key.starts_with(&prefix) && key != path {
                let child_depth = key.matches('/').count();

                // Only direct children (one level deeper)
                if child_depth == depth {
                    let name = key.rsplit('/').next().unwrap_or(key);
                    if seen.insert(name.to_string()) {
                        results.push((name.to_string(), value.clone()));
                    }
                }
            }
        }

        Ok(results)
    }

    /// Remove file or empty directory
    pub fn rm(&self, path: &str) -> Result<()> {
        let path = normalize_path(path);

        if path == "/" {
            return Err(anyhow!("Cannot remove root directory"));
        }

        let rtxn = self.env.read_txn()?;
        let meta = self.metadata.get(&rtxn, &path)?
            .ok_or_else(|| anyhow!("Not found: {}", path))?;

        // If directory, check if empty
        if meta.file_type == FileType::Directory {
            let children = self.ls(&path)?;
            if !children.is_empty() {
                return Err(anyhow!("Directory not empty: {}", path));
            }
        }

        // Remove blob file if exists
        if let Some(ref real_path) = meta.real_path {
            let _ = std::fs::remove_file(real_path);
        }

        drop(rtxn);

        let mut wtxn = self.env.write_txn()?;
        self.metadata.delete(&mut wtxn, &path)?;
        let _ = self.content.delete(&mut wtxn, &path);
        wtxn.commit()?;

        Ok(())
    }

    /// Remove directory recursively
    pub fn rm_rf(&self, path: &str) -> Result<()> {
        let path = normalize_path(path);

        if path == "/" {
            return Err(anyhow!("Cannot remove root directory"));
        }

        // Collect all paths to delete
        let rtxn = self.env.read_txn()?;
        let prefix = format!("{}/", path);

        let mut to_delete = vec![path.clone()];

        let iter = self.metadata.iter(&rtxn)?;
        for item in iter {
            let (key, _) = item?;
            if key.starts_with(&prefix) {
                to_delete.push(key.to_string());
            }
        }
        drop(rtxn);

        // Delete all collected paths
        let mut wtxn = self.env.write_txn()?;
        for p in &to_delete {
            // Check for blob files to clean up
            if let Ok(Some(meta)) = self.stat(p) {
                if let Some(ref real_path) = meta.real_path {
                    let _ = std::fs::remove_file(real_path);
                }
            }
            self.metadata.delete(&mut wtxn, p)?;
            let _ = self.content.delete(&mut wtxn, p);
        }
        wtxn.commit()?;

        Ok(())
    }

    /// Rename/move file or directory
    pub fn rename(&self, old_path: &str, new_path: &str) -> Result<()> {
        let old_path = normalize_path(old_path);
        let new_path = normalize_path(new_path);

        if !self.exists(&old_path)? {
            return Err(anyhow!("Source not found: {}", old_path));
        }

        if self.exists(&new_path)? {
            return Err(anyhow!("Destination already exists: {}", new_path));
        }

        // Ensure parent of destination exists
        if let Some(parent) = parent_path(&new_path) {
            self.mkdir_p(&parent)?;
        }

        let rtxn = self.env.read_txn()?;
        let meta = self.metadata.get(&rtxn, &old_path)?
            .ok_or_else(|| anyhow!("Source not found: {}", old_path))?
            .clone();
        let content = self.content.get(&rtxn, &old_path)?.map(|b| b.to_vec());
        drop(rtxn);

        let mut wtxn = self.env.write_txn()?;

        // Copy to new location
        self.metadata.put(&mut wtxn, &new_path, &meta)?;
        if let Some(data) = content {
            self.content.put(&mut wtxn, &new_path, &data)?;
        }

        // Delete old
        self.metadata.delete(&mut wtxn, &old_path)?;
        let _ = self.content.delete(&mut wtxn, &old_path);

        wtxn.commit()?;
        Ok(())
    }

    // ========================================================================
    // VECTOR INDEX (Reverse RAG Lookup)
    // ========================================================================

    /// Index a file with a vector ID for reverse lookup
    pub fn index_vector(&self, path: &str, vector_id: &str) -> Result<()> {
        let path = normalize_path(path);

        let mut wtxn = self.env.write_txn()?;

        // Update metadata
        if let Some(mut meta) = self.stat(&path)? {
            meta.vector_id = Some(vector_id.to_string());
            self.metadata.put(&mut wtxn, &path, &meta)?;
        }

        // Add to index
        self.index.put(&mut wtxn, vector_id, &path)?;
        wtxn.commit()?;

        Ok(())
    }

    /// Reverse lookup: vector_id → path
    pub fn vector_to_path(&self, vector_id: &str) -> Result<Option<String>> {
        let rtxn = self.env.read_txn()?;
        Ok(self.index.get(&rtxn, vector_id)?.map(|s| s.to_string()))
    }

    // ========================================================================
    // UTILITIES
    // ========================================================================

    /// Get total size of all files
    pub fn total_size(&self) -> Result<u64> {
        let rtxn = self.env.read_txn()?;
        let mut total = 0u64;

        let iter = self.metadata.iter(&rtxn)?;
        for item in iter {
            let (_, meta) = item?;
            total += meta.size;
        }

        Ok(total)
    }

    /// Get file count
    pub fn file_count(&self) -> Result<u64> {
        let rtxn = self.env.read_txn()?;
        let mut count = 0u64;

        let iter = self.metadata.iter(&rtxn)?;
        for item in iter {
            let (_, meta) = item?;
            if meta.file_type == FileType::File {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Get directory count
    pub fn dir_count(&self) -> Result<u64> {
        let rtxn = self.env.read_txn()?;
        let mut count = 0u64;

        let iter = self.metadata.iter(&rtxn)?;
        for item in iter {
            let (_, meta) = item?;
            if meta.file_type == FileType::Directory {
                count += 1;
            }
        }

        Ok(count)
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Normalize a path: resolve . and .., ensure leading /, no trailing /
fn normalize_path(path: &str) -> String {
    let mut components: Vec<&str> = Vec::new();

    for part in path.split('/') {
        match part {
            "" | "." => continue,
            ".." => { components.pop(); }
            _ => components.push(part),
        }
    }

    if components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", components.join("/"))
    }
}

/// Get parent path
fn parent_path(path: &str) -> Option<String> {
    let path = normalize_path(path);
    if path == "/" {
        return None;
    }

    let idx = path.rfind('/')?;
    if idx == 0 {
        Some("/".to_string())
    } else {
        Some(path[..idx].to_string())
    }
}

/// Current Unix timestamp
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// SHA256 hash as hex string
fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path("/home/user"), "/home/user");
        assert_eq!(normalize_path("/home/user/"), "/home/user");
        assert_eq!(normalize_path("/home/../home/user"), "/home/user");
        assert_eq!(normalize_path("/home/./user"), "/home/user");
        assert_eq!(normalize_path("relative"), "/relative");
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(parent_path("/"), None);
        assert_eq!(parent_path("/home"), Some("/".to_string()));
        assert_eq!(parent_path("/home/user"), Some("/home".to_string()));
    }

    #[test]
    fn test_basic_operations() -> Result<()> {
        let dir = tempdir()?;
        let fs = SpfFs::open(dir.path())?;

        // Test exists
        assert!(fs.exists("/")?);
        assert!(fs.exists("/home/user")?);
        assert!(!fs.exists("/nonexistent")?);

        // Test write and read
        fs.write("/home/user/test.txt", b"Hello, SPF!")?;
        let content = fs.read("/home/user/test.txt")?;
        assert_eq!(content, b"Hello, SPF!");

        // Test stat
        let meta = fs.stat("/home/user/test.txt")?.unwrap();
        assert_eq!(meta.file_type, FileType::File);
        assert_eq!(meta.size, 11);

        // Test ls
        let entries = fs.ls("/home/user")?;
        assert!(entries.iter().any(|(name, _)| name == "test.txt"));

        // Test rm
        fs.rm("/home/user/test.txt")?;
        assert!(!fs.exists("/home/user/test.txt")?);

        Ok(())
    }
}
