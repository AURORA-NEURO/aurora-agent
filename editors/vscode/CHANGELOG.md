# Changelog

## 0.1.3

Initial release, versioned alongside the platform's v0.1.3 binaries.

- MCP server definition provider ("AURORA Agent", stdio `bioprism-mcp --root <root>`) for VS Code
  1.101+, plus a clipboard `.mcp.json` snippet for other MCP clients.
- Activity Bar views: Workflows (capability groups from `workflow catalogue --json`, with
  scaffold/instantiate context actions), Autopilot Reports (watched directory with `final_status`
  badges), Pipelines (last 10 GitHub Actions runs via `gh`, informational row when `gh` is absent).
- Palette commands for `context compile/verify/explain`, `world validate`,
  `autopilot grant-template/run/verify` (real runs behind a grant-summary confirmation modal;
  dry-run without one), gateway start/stop as a task, and a doctor.
- Faithful summary rendering of certificates and reports: verdict/final_status, selection counts,
  omission accounting, full digests, limitations verbatim, raw-JSON link.
- Binary resolution across settings, `AURORA_AGENT_ROOT`, workspace folders, home checkouts, and a
  managed, SHA-256-pinned download of the v0.1.3 release archives.
- `node:test` unit tests for the pure logic; zero runtime dependencies.
