# Privacy Policy — AURORA Agent (bioprism)

Effective: 2026-08-24. Applies to the `bioprism` binaries (`bioprism`,
`bioprism-mcp`, `bioprism-api`) and the packaged Claude Desktop extension
(`aurora-agent.mcpb`).

## Summary

The AURORA Agent MCP server is a **local program**. It makes **no network
requests**, calls **no external services**, and **collects, stores, and
transmits no personal data**. There is no telemetry, no analytics, no
crash reporting, and no account.

## Details

- **Data processed**: the server reads only files inside the data root you
  configure (`--root`, or the extension's "AURORA data root" setting) and
  writes only the artifacts you explicitly request (e.g. certificate or index
  files at paths you supply). Absolute paths, `..` traversal, and symlink
  escapes are refused.
- **Network**: the MCP server and CLI open no network connections. The
  workspace builds offline against pinned dependencies. (The optional
  `bioprism-api` gateway binds to an address you choose — loopback by
  default — and serves only callers presenting your bearer token; it likewise
  calls no external services.)
- **Conversation data**: the server sees only the tool arguments the MCP
  client sends it and returns results to that client. It does not read,
  store, or transmit conversation history.
- **Third parties**: none. No data is shared with anyone, including the
  authors.

## Boundary

Research and developer infrastructure. It does not diagnose an individual,
recommend treatment, triage care, enroll participants, or claim
medical-device functionality.

## Contact

Issues: https://github.com/AURORA-NEURO/aurora-agent/issues
