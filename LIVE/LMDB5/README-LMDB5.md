# LMDB5 — Claude Code Working Directory + Agent State
# Copyright 2026 Joseph Stone - All Rights Reserved

## Structure
.claude.json   <- Claude Code config (MCP servers, allowedTools)
.claude/       <- Claude Code project settings + session data
CLAUDE.md      <- Project instructions (boot sequence, rules)
LMDB5.DB/      <- Binary twin (LMDB agent state: memory, preferences, sessions)

## How it works
- Claude Code boots from this directory: HOME=~/SPFsmartGATE/LIVE/LMDB5 claude
- LMDB5.DB/ stores agent memory, preferences, session context
- Managed by Rust binary via spf_agent_* MCP tools
- This directory is NOT on the write allowlist (read-only to AI)

## Update commands
# Agent state is managed through MCP tools:
# spf_agent_stats        - View agent state statistics
# spf_agent_memory_search - Search agent memory by query
# spf_agent_memory_by_tag - Search agent memory by tag
# spf_agent_session_info  - View session information
# spf_agent_context       - Get full agent context

## Boot command
HOME=~/SPFsmartGATE/LIVE/LMDB5 claude
