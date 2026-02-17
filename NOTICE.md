# NOTICE

## Copyright

Copyright 2026 Joseph Stone. All Rights Reserved.

SPFsmartGATE is licensed under the PolyForm Noncommercial
License 1.0.0. See LICENSE.md for the full license text.
Commercial use requires a separate license. See
COMMERCIAL_LICENSE.md for details.

## Proprietary Intellectual Property

### SPF Complexity Formula

The StoneCell Processing Formula (SPF) for AI Self-Governance
is the proprietary intellectual property of Joseph Stone. This
includes the complete formula system:

    C = basic^1 + deps^7 + complex^10 + files * 10

    a_optimal(C) = W_eff * (1 - 1 / ln(C + e))

And all associated components:

- Complexity weight system (per-tool basic, dependencies,
  complex, and files weights)
- Tier classification system (SIMPLE, LIGHT, MEDIUM, CRITICAL)
- Analyze/Build percentage allocation per tier
- Dynamic complexity factors and escalation logic
- Max mode enforcement and CRITICAL tier escalation

The formula, its mathematical relationships, its implementation
in source code (calculate.rs), and all derivative works are
protected by copyright. The source code expression is a fixed
work in a tangible medium under US Copyright Law.

### SPFsmartGATE System

The SPFsmartGATE security gateway system, including its
architecture, enforcement pipeline, protocol designs, and
implementation, is the proprietary work of Joseph Stone. This
includes:

- Gate enforcement pipeline (Rate Limit, Calculate, Validate,
  Inspect, Max Mode Escalation, Decision)
- Build Anchor Protocol (read-before-write enforcement)
- 6-database LMDB architecture
- Default-deny tool allowlist system
- Content inspection engine
- MCP server implementation

## Third-Party Dependencies

SPFsmartGATE uses the following open-source libraries. Their
inclusion does not alter the licensing terms of SPFsmartGATE
itself.

| Crate       | Version | License        | Purpose                    |
|-------------|---------|----------------|----------------------------|
| heed        | 0.20    | MIT            | LMDB state storage         |
| serde       | 1.0     | MIT/Apache-2.0 | Serialization              |
| serde_json  | 1.0     | MIT/Apache-2.0 | JSON serialization         |
| clap        | 4.5     | MIT/Apache-2.0 | CLI argument parsing       |
| thiserror   | 1.0     | MIT/Apache-2.0 | Error type derivation      |
| anyhow      | 1.0     | MIT/Apache-2.0 | Error handling             |
| log         | 0.4     | MIT/Apache-2.0 | Logging facade             |
| env_logger  | 0.11    | MIT/Apache-2.0 | Logging implementation     |
| chrono      | 0.4     | MIT/Apache-2.0 | Date and time              |
| reqwest     | 0.12    | MIT/Apache-2.0 | HTTP client                |
| html2text   | 0.6     | MIT            | HTML to text conversion    |
| sha2        | 0.10    | MIT/Apache-2.0 | SHA-256 checksums          |
| hex         | 0.4     | MIT/Apache-2.0 | Hex encoding               |

All third-party licenses are available in their respective
crate repositories on crates.io.

## Trademarks

SPFsmartGATE, StoneCell Processing Formula, and SPF are
trademarks of Joseph Stone. Use of these marks requires
written permission from the trademark holder, except as
necessary to describe the origin of the software in
compliance with the license terms.
