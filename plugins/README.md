# AURORA plugin marketplace

This repository doubles as a Claude Code plugin marketplace. The marketplace
manifest is `.claude-plugin/marketplace.json`, and plugins live under
`plugins/`.

```bash
claude plugin marketplace add /path/to/aurora-agent
# once hosted
claude plugin marketplace add AURORA-NEURO/aurora-agent
```

## Plugin catalog

| Plugin | Gives you | Needs a built checkout? |
|---|---|---|
| `aurora-agent` | Full backend integration, MCP, FIBER commands, mission preflight, operations audit, and SDK/workflow skills | yes |
| `aurora-backend` | The bioprism MCP server and setup/troubleshooting launcher | yes |
| `aurora-context` | FIBER compile/explain/compare/validate/verify commands, decision-context skill, and faithfulness eval | yes |
| `aurora-missions` | Mission DAG authoring, preflight, gate acceptance, and execution lifecycle | gateway or backend |
| `aurora-integrate` | HTTP gateway and TypeScript/Python SDK integration guidance | no |
| `aurora-ops` | Operations health command and evidence-first ops-auditor | gateway |
| `aurora-honesty` | Honest-labelling, claim hygiene, scanner proof, and Windows test truth | no |
| `aurora-science` | Reproducible measurement and evaluation methodology | no |
| `aurora-workspace` | In-repository module, parity, crate, and blueprint analysis skills | this repo |

Install the complete integration or a focused package as needed:

```bash
claude plugin install aurora-agent@aurora
# or, for focused installs:
claude plugin install aurora-context@aurora
claude plugin install aurora-missions@aurora
```

## Backend discovery

The backend launcher resolves the checkout from `AURORA_AGENT_ROOT`, then
`~/aurora-agent`, then `~/bioprism`. It verifies the release binary before
starting it and reports the exact local build command when it is missing. The
context commands use the same resolution contract.

Prerequisite for backend-backed plugins:

```bash
cargo build --release --offline -p bioprism-cli -p bioprism-mcp
```

Add `-p bioprism-api` when using the HTTP gateway.

## Maintaining

- `.agents/skills/` is the source of truth for mirrored workspace and honesty
  skills. After editing, run `python tools/sync_plugin_skills.py`; generated
  mirrors carry a do-not-edit banner.
- Validate the marketplace and each plugin with
  `claude plugin validate . --strict` and
  `claude plugin validate plugins/<name> --strict`.
- Keep each plugin's `plugin.json` version synchronized with its marketplace
  entry before tagging a release.
- `plugins/` is outside `tools/coverage.sh`'s crate/docs walk; plugin text
  cannot inflate blueprint coverage.

## Known limits

Plugins do not download binaries; backend-backed packages require a locally
built checkout. The `conformance_run` MCP tool also requires the checkout path
used when the binary was built. The project reports medical and operational
boundaries explicitly and does not diagnose, recommend treatment, triage care,
or claim medical-device functionality.
