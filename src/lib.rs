// SPF Smart Gateway - Library Root
// Copyright 2026 Joseph Stone - All Rights Reserved
//
// All modules exported here for use by the binary and tests.

pub mod paths;
pub mod calculate;
pub mod config;
pub mod gate;
pub mod inspect;
pub mod mcp;
pub mod session;
pub mod storage;
pub mod validate;
pub mod web;

// ============================================================================
// LMDB MODULES - 6-Database Architecture
// ============================================================================

/// SPF_FS: LMDB-backed virtual filesystem
pub mod fs;

/// SPF_CONFIG: LMDB-backed configuration storage
pub mod config_db;

/// PROJECTS: LMDB-backed project registry
pub mod projects_db;

/// TMP_DB: LMDB-backed TMP and projects metadata tracking
pub mod tmp_db;

/// AGENT_STATE: LMDB-backed Agent persistent state
pub mod agent_state;
