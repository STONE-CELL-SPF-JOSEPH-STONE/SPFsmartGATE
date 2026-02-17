// SPF Smart Gateway - LMDB Storage
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// Persists session state to LMDB at LIVE/SESSION/SESSION.DB.
// Used for: session checkpoints, complexity history, manifest, failures.

use crate::session::Session;
use anyhow::Result;
use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};
use std::path::Path;

/// LMDB storage for SPF gateway state
pub struct SpfStorage {
    env: Env,
    /// Main key-value store: string keys → JSON values
    db: Database<Str, Str>,
}

const SESSION_KEY: &str = "current_session";
const MAX_DB_SIZE: usize = 50 * 1024 * 1024; // 50MB — plenty for state data

impl SpfStorage {
    /// Open or create LMDB at the given path
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(MAX_DB_SIZE)
                .max_dbs(4)
                .open(path)?
        };

        let mut wtxn = env.write_txn()?;
        let db = env.create_database(&mut wtxn, Some("spf_state"))?;
        wtxn.commit()?;

        log::info!("SPF LMDB opened at {:?}", path);
        Ok(Self { env, db })
    }

    /// Save session state to LMDB
    pub fn save_session(&self, session: &Session) -> Result<()> {
        let json = serde_json::to_string(session)?;
        let mut wtxn = self.env.write_txn()?;
        self.db.put(&mut wtxn, SESSION_KEY, &json)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Load session state from LMDB
    pub fn load_session(&self) -> Result<Option<Session>> {
        let rtxn = self.env.read_txn()?;
        match self.db.get(&rtxn, SESSION_KEY)? {
            Some(json) => {
                let session: Session = serde_json::from_str(json)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// Store arbitrary key-value pair
    pub fn put(&self, key: &str, value: &str) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.db.put(&mut wtxn, key, value)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Retrieve a value by key
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let rtxn = self.env.read_txn()?;
        Ok(self.db.get(&rtxn, key)?.map(|s| s.to_string()))
    }

    /// Delete a key
    pub fn delete(&self, key: &str) -> Result<bool> {
        let mut wtxn = self.env.write_txn()?;
        let deleted = self.db.delete(&mut wtxn, key)?;
        wtxn.commit()?;
        Ok(deleted)
    }

    /// Get storage size in bytes
    pub fn size_bytes(&self) -> Result<u64> {
        let rtxn = self.env.read_txn()?;
        let stat = self.db.stat(&rtxn)?;
        // Approximate: entries * average size
        Ok((stat.entries as u64) * 256)
    }

    /// Get entry count
    pub fn entry_count(&self) -> Result<u64> {
        let rtxn = self.env.read_txn()?;
        let stat = self.db.stat(&rtxn)?;
        Ok(stat.entries as u64)
    }
}
