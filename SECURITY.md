# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in SPFsmartGATE, please
report it responsibly. **Do not open a public GitHub issue.**

### How to Report

Email: **joepcstone@gmail.com**

Subject line: `[SECURITY] SPFsmartGATE — Brief description`

Include:
- Description of the vulnerability
- Steps to reproduce
- Affected version(s)
- Impact assessment (what an attacker could do)
- Any suggested fix (optional)

### Response Timeline

- **Acknowledgment**: Within 48 hours
- **Initial assessment**: Within 7 days
- **Fix or mitigation**: Depends on severity
  - Critical: Patch within 72 hours
  - High: Patch within 14 days
  - Medium/Low: Next scheduled release

### What Qualifies

- Bypass of the gate enforcement pipeline
- Bypass of the Build Anchor protocol (write without read)
- Bypass of path blocking or content inspection
- Credential leakage through the MCP interface
- SSRF bypass (accessing blocked networks)
- Shell injection through the gate
- Unauthorized access to LMDB databases
- Denial of service against the gate server

### What Does Not Qualify

- Issues requiring physical device access
- Social engineering attacks
- Vulnerabilities in dependencies (report to upstream)
- Issues in Claude Code itself (report to Anthropic)
- Theoretical attacks without a proof of concept

### Disclosure Policy

- We follow coordinated disclosure — please allow time for a
  fix before any public disclosure.
- Credit will be given to reporters in the CHANGELOG unless
  anonymity is requested.
- We will not pursue legal action against researchers who
  follow this policy.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 2.0.x   | ✅ Active |
| < 2.0   | ❌ No     |

## Security Architecture

SPFsmartGATE enforces security through a compiled Rust binary
with no runtime configuration bypass. Key security features:

- **5-stage gate pipeline**: Rate limit → Calculate → Validate
  → Inspect → Max Mode Escalation
- **Build Anchor Protocol**: Must read before write
- **Default-deny tool allowlist**: Unknown tools blocked
- **SSRF protection**: Full IPv4/IPv6 private network blocking
- **Content inspection**: Credential, traversal, and injection
  detection
- **Compiled enforcement**: Rules are in the binary, not config

For full details, see [README.md](README.md).
