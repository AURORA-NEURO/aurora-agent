# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| 0.1.x | yes |

## Reporting a vulnerability

Report vulnerabilities privately through
[GitHub Security Advisories](https://github.com/AURORA-NEURO/aurora-agent/security/advisories/new)
on this repository. Do not open a public issue for a security report.

Include the affected surface (CLI, `bioprism-mcp` stdio server, `bioprism-api` HTTP
gateway, or the packaged `.mcpb`), the exact command or request, and what you observed.
Paste certificates, verdicts, and error output verbatim.

## Scope

AURORA Agent is local-only software:

- The CLI and MCP server open no network connections; the workspace builds offline
  against pinned dependencies. The optional `bioprism-api` gateway binds to an address
  the operator chooses (loopback by default) and serves only callers presenting the
  configured bearer token.
- File access is confined to the configured `--root`; absolute paths, `..` traversal,
  and symlink escapes are refused.
- No telemetry, analytics, or data collection — see
  [PRIVACY.md](https://github.com/AURORA-NEURO/aurora-agent/blob/main/PRIVACY.md).

In scope: path-confinement escapes, certificate or parity forgery, denial of service
through crafted worlds or queries, token handling in `bioprism-api`, and integrity of
the released `.mcpb` artifact.

## Out of scope

Use of the research outputs for clinical decisions. Research and developer
infrastructure: it does not diagnose an individual, recommend treatment, triage care,
enroll participants, or claim medical-device functionality.
