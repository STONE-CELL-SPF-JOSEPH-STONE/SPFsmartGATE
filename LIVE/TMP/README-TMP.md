# TMP — Twin Folder System
# Copyright 2026 Joseph Stone - All Rights Reserved

## Structure
TMP/      <- Device twin (writable files on disk)
TMP.DB/   <- Binary twin (LMDB metadata, managed by Rust binary)

## How it works
- spf_write/spf_edit write to TMP/ (device twin)
- TMP.DB/ stores metadata: access logs, project associations, trust levels
- The Rust binary reads/writes TMP.DB/ automatically
- Device twin is on the write allowlist in validate.rs

## Update commands
# After editing files in TMP/ on device:
# The binary picks up changes on next spf_read or spf_fs_stat

# To inspect TMP.DB/ metadata:
spf-smart-gate status
# Or use MCP tools: spf_tmp_list, spf_tmp_stats, spf_tmp_get, spf_tmp_active

## Write allowlist path
/data/data/com.termux/files/home/SPFsmartGATE/LIVE/TMP/TMP/
