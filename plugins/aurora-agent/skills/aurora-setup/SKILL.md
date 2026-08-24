---
name: aurora-setup
description: Set up, build, or troubleshoot the AURORA Agent (bioprism) MCP server used by the aurora-agent plugin. Use when the aurora-agent MCP server fails to start, when its tools are missing from a session, when asked to install or build the aurora-agent backend, or when a bioprism build or test behaves strangely on Windows.
---

# AURORA backend setup

The `aurora-agent` plugin launches `bioprism-mcp`, the MCP server of the
AURORA Agent workspace (264 tools; JSON-RPC 2.0 over newline-delimited stdio;
stdout is JSON-RPC only, diagnostics go to stderr).

## Where the backend lives

The plugin's launcher (`scripts/aurora-mcp.mjs`) resolves the checkout in this
order and refuses with a precise stderr message if nothing matches:

1. `AURORA_AGENT_ROOT` environment variable
2. `~/aurora-agent`
3. `~/bioprism`

A directory qualifies when it contains `Cargo.toml`. Set `AURORA_AGENT_ROOT`
when the checkout lives anywhere else.

## Build

```bash
git clone https://github.com/AURORA-NEURO/aurora-agent
cd aurora-agent
cargo build --release --offline -p bioprism-mcp
```

The workspace builds **offline against pinned versions** (`.cargo/config.toml`
sets `net.offline = true`); you cannot add external crates. A release build of
the full three-binary set is `-p bioprism-cli -p bioprism-mcp -p bioprism-api`
and takes a long time (`codegen-units = 1`, thin LTO) — the MCP server alone is
faster.

## Windows gotcha: os error 4551

If a freshly linked binary reports `os error 4551` ("never executed"), Windows
Application Control blocked it — the program NEVER RAN, even though cargo
reports a failure. Touch a source file to force a relink and rebuild. In test
suites this silently truncates `--workspace` totals; a suite that never ran
looks like a suite that failed.

## What `--root` means

The launcher passes `--root <checkout>`. Root is a **confinement boundary**:
every path a tool receives resolves inside it — absolute paths, `..` traversal,
and symlink escapes are refused. The reference fixtures
(`fixtures/fiber-v0.1/radiogenomic_world.json`,
`fixtures/fiber-v0.1/leakage_query.json`) live inside root, so tool calls can
name them by that relative path.

Known caveat: the `conformance_run` tool bakes a build-machine fixtures path at
compile time; it only works when the checkout has not moved since it was built.

## Boundary

This backend is research and developer infrastructure. It does not diagnose an
individual, recommend treatment, triage care, enroll participants, or claim
medical-device functionality. `bioprism-onco` carries a typed research
boundary; do not route around it.
