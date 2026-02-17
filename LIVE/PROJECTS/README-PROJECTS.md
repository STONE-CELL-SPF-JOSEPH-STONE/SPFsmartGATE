# PROJECTS — Twin Folder System
# Copyright 2026 Joseph Stone - All Rights Reserved

## Structure
PROJECTS/      <- Device twin (writable project files on disk)
PROJECTS.DB/   <- Binary twin (LMDB project registry, managed by Rust binary)

## How it works
- spf_write/spf_edit write to PROJECTS/ (device twin)
- PROJECTS.DB/ stores registry: project names, paths, trust levels, metadata
- The Rust binary reads/writes PROJECTS.DB/ automatically
- Device twin is on the write allowlist in validate.rs

## Update commands
# After adding/editing project files on device:
# Register with MCP tools: spf_projects_set, spf_projects_list

# To inspect PROJECTS.DB/ registry:
spf-smart-gate status
# Or use MCP tools: spf_projects_list, spf_projects_get, spf_projects_stats

## Write allowlist path
/data/data/com.termux/files/home/SPFsmartGATE/LIVE/PROJECTS/PROJECTS/
