// SPF Smart Gateway - Projects LMDB
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// LMDB-backed project registry. Empty on init, ready for project data.
//
// Database: PROJECTS
// Storage: ~/SPFsmartGATE/LIVE/PROJECTS/PROJECTS.DB/

use anyhow::Result;
use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};
use std::path::Path;

const MAX_DB_SIZE: usize = 20 * 1024 * 1024; // 20MB

/// LMDB-backed project registry
pub struct SpfProjectsDb {
    env: Env,
    /// General key-value store for project data
    data: Database<Str, Str>,
}

impl SpfProjectsDb {
    /// Open or create projects LMDB at given path
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAX_DB_SIZE)
                .max_dbs(8)
                .open(path)?
        };

        let mut wtxn = env.write_txn()?;
        let data = env.create_database(&mut wtxn, Some("projects"))?;
        wtxn.commit()?;

        log::info!("PROJECTS LMDB opened at {:?}", path);
        Ok(Self { env, data })
    }

    /// Initialize defaults (no seeding -- starts empty)
    pub fn init_defaults(&self) -> Result<()> {
        log::info!("PROJECTS LMDB initialized");
        Ok(())
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let rtxn = self.env.read_txn()?;
        Ok(self.data.get(&rtxn, key)?.map(|s| s.to_string()))
    }

    /// Set a key-value pair
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.data.put(&mut wtxn, key, value)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Delete a key
    pub fn delete(&self, key: &str) -> Result<bool> {
        let mut wtxn = self.env.write_txn()?;
        let deleted = self.data.delete(&mut wtxn, key)?;
        wtxn.commit()?;
        Ok(deleted)
    }

    /// List all entries
    pub fn list_all(&self) -> Result<Vec<(String, String)>> {
        let rtxn = self.env.read_txn()?;
        let iter = self.data.iter(&rtxn)?;
        let mut entries = Vec::new();
        for result in iter {
            let (key, value) = result?;
            entries.push((key.to_string(), value.to_string()));
        }
        Ok(entries)
    }

    /// Get database stats
    pub fn db_stats(&self) -> Result<(u64, u64, u64)> {
        let rtxn = self.env.read_txn()?;
        let data_stat = self.data.stat(&rtxn)?;
        Ok((data_stat.entries as u64, 0, 0))
    }
}
