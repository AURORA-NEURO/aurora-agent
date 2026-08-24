# aurora-agent-mcp

Launcher for the **AURORA Agent (bioprism) MCP server** — a Rust, local-only
[Model Context Protocol](https://modelcontextprotocol.io) server built around
the FIBER decision-context compiler. Every compile returns a **Context
Certificate**: a receipt stating exactly what was omitted from the context and
whether the omission could have changed the decision. 259 evidence-bearing
tools: compile / refine / explain / verify, equal-engineering baseline
comparison, world validation and indexing, mission DAGs with least-authority
policies, and capability/operations evidence.

- Source: <https://github.com/AURORA-NEURO/aurora-agent> (Apache-2.0)
- MCP registry name: `io.github.MurariAmbati/aurora-agent`
- Privacy: fully local, no network access, no data collection —
  [PRIVACY.md](https://github.com/AURORA-NEURO/aurora-agent/blob/main/PRIVACY.md)

## Quick start

Claude Code:

```bash
claude mcp add aurora-agent -- npx -y aurora-agent-mcp
```

Any MCP client (`.mcp.json` / `mcpServers` style):

```json
{
  "mcpServers": {
    "aurora-agent": {
      "command": "npx",
      "args": ["-y", "aurora-agent-mcp"]
    }
  }
}
```

On first run the launcher downloads the signed release bundle (~15 MB) from
[GitHub Releases](https://github.com/MurariAmbati/aurora-agent-releases/releases),
verifies it against a SHA-256 pinned inside this package, caches it under
`%LOCALAPPDATA%\aurora-agent-mcp`, and starts the server confined to the
bundled reference fixtures. It refuses to run anything that fails the hash
check.

## Configuration

| Environment variable | Effect |
|---|---|
| `AURORA_AGENT_ROOT` | Data root the server is confined to (worlds, queries, schemas). Point it at a full `aurora-agent` checkout for the complete tool surface. If the checkout contains `target/release/bioprism-mcp(.exe)`, that binary is used and nothing is downloaded. |
| `AURORA_AGENT_MCP_EXE` | Explicit path to a `bioprism-mcp` binary; skips all resolution and downloading. |

The prebuilt bundle is **Windows-only** today. On other platforms, build from
source (`cargo build --release`) and set `AURORA_AGENT_ROOT` — the launcher
tells you exactly this if it cannot find a binary.

## Boundary

Research and developer infrastructure. It does not diagnose an individual,
recommend treatment, triage care, enroll participants, or claim medical-device
functionality.
