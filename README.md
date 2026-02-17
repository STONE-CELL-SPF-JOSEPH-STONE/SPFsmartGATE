# SPFsmartGATE

**AI Security Gateway -- Compiled Rust enforcement between AI agents and your system.**

[![License: PolyForm Noncommercial 1.0.0](https://img.shields.io/badge/License-PolyForm%20NC%201.0.0-blue.svg)](LICENSE.md)

---

> **Free for personal use. Commercial use requires a license.**
> See [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md) for details.

---

## What Is SPFsmartGATE?

SPFsmartGATE is a compiled Rust security gateway that sits
between AI agents and your system. Every tool call from any
AI agent must pass through the gate before touching the
filesystem, running commands, or accessing the web.

It enforces the **StoneCell Processing Formula (SPF)** for
AI self-governance -- a mathematical complexity model that
calculates risk, allocates resources, and gates operations
in real time.

The gate is compiled into a native binary. There is no
runtime configuration bypass. The rules are in the code.

## Core Features

### Gate Enforcement Pipeline

Every tool call passes through a 5-stage pipeline:

1. **Rate Limiting** -- Per-minute operation caps by tool
   category (write: 60/min, web: 30/min, read: 120/min)
2. **Complexity Calculation** -- The SPF formula computes a
   complexity score C for every operation
3. **Validation** -- Build Anchor protocol, write allowlist,
   path blocking, dangerous command detection
4. **Content Inspection** -- Credential detection, path
   traversal, shell injection scanning
5. **Max Mode Escalation** -- Violations escalate to CRITICAL
   tier instead of blocking, forcing maximum scrutiny

### SPF Complexity Formula

```
C = basic^1 + deps^7 + complex^10 + files * 10

a_optimal(C) = W_eff * (1 - 1 / ln(C + e))
```

The formula produces a complexity score that determines:
- **Tier assignment**: SIMPLE, LIGHT, MEDIUM, CRITICAL
- **Resource allocation**: Analyze vs Build percentage
- **Approval requirements**: Per-tier escalation policy

### Build Anchor Protocol

AI agents must read a file before they can edit or overwrite
it. This prevents blind modifications and forces the agent
to understand existing code before changing it.

### Default-Deny Security

Unknown tools are blocked until explicitly added to the gate
allowlist. 44 known-safe tools are explicitly allowed. 10
system-only tools are hard-blocked. Everything else is denied.

### SSRF Protection

Full IPv4 and IPv6 validation on all web requests. Blocks
loopback, private networks, link-local, cloud metadata
endpoints, and IPv4-mapped IPv6 addresses.

### Content Inspection

Scans all write operations for:
- Credential patterns (API keys, tokens, private keys)
- Path traversal sequences
- Shell injection patterns
- References to blocked paths

## Architecture

SPFsmartGATE uses a 6-database LMDB architecture:

| Database    | Purpose                          |
|-------------|----------------------------------|
| SPF_FS      | Virtual filesystem (system-only) |
| CONFIG      | Configuration and rules          |
| PROJECTS    | Project registry                 |
| TMP_DB      | Metadata tracking and trust      |
| AGENT_STATE | Agent memory and sessions        |
| SESSION     | Runtime session persistence      |

The MCP (Model Context Protocol) server communicates via
stdio JSON-RPC 2.0, making it compatible with any MCP client.

## Installation

### Requirements

- Rust toolchain (rustup.rs)
- LMDB system library

### Build

```bash
cd SPFsmartGATE
cargo build --release
```

The binary is built at `target/release/spf-smart-gate`.

### Initialize Configuration

```bash
./target/release/spf-smart-gate init-config
```

### Start the MCP Server

```bash
./target/release/spf-smart-gate serve
```

### Start Claude/agent  
Must copy CLI files to flat LMDB5 
and then export to LMDB5.db

in new terminal

HOME=~/SPFsmartGATE/LIVE/LMDB5 claude

## CLI Commands

| Command         | Description                        |
|-----------------|------------------------------------|
| `serve`         | Start the MCP server               |
| `gate`          | Process a single tool call         |
| `calculate`     | Calculate complexity for a tool    |
| `status`        | Show SPF gateway status            |
| `session`       | Show current session state         |
| `reset`         | Reset session state                |
| `init-config`   | Initialize configuration database  |
| `refresh-paths` | Refresh blocked/allowed paths      |

## License

SPFsmartGATE is licensed under the
[PolyForm Noncommercial License 1.0.0](LICENSE.md).

**Personal, educational, and noncommercial use is free.**

**All commercial use requires a separate license.**
See [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md).

## Copyright

Copyright 2026 Joseph Stone. All Rights Reserved.

The StoneCell Processing Formula (SPF) and SPFsmartGATE are
proprietary intellectual property. See [NOTICE.md](NOTICE.md)
for full attribution and third-party dependency information.
