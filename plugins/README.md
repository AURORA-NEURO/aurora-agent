# AURORA plugin marketplace

This repo doubles as a Claude Code plugin marketplace: the manifest is
`.claude-plugin/marketplace.json`, and each plugin lives under `plugins/`.

```bash
# local checkout
claude plugin marketplace add /path/to/aurora-agent
# once hosted
claude plugin marketplace add AURORA-NEURO/aurora-agent

claude plugin install aurora-backend@aurora   # etc.
```

## Plugins

| Plugin | Gives you | Needs the checkout built? |
|---|---|---|
| `aurora-backend` | The bioprism MCP server (259 tools) wired into any session, plus a setup/troubleshooting skill | yes (`cargo build --release --offline -p bioprism-mcp`) |
| `aurora-context` | `/aurora-context:compile`, `:explain`, `:compare`, `:validate`, `:verify` commands + the FIBER decision-context skill + a faithfulness eval | yes (`-p bioprism-cli`) |
| `aurora-missions` | Mission DAG authoring/lifecycle skill + `/aurora-missions:preflight` | gateway or aurora-backend |
| `aurora-honesty` | The honest-labelling discipline for **any** codebase (4 skills) | no |
| `aurora-workspace` | The in-repo engineering skills (add-module, check-parity, verify-crate, classify-blueprint-modules, measure-section-boilerplate) | works only inside this repo |
| `aurora-integrate` | HTTP gateway + TS/Python SDK integration skill | no (teaches; gateway optional) |
| `aurora-ops` | `ops-auditor` subagent + `/aurora-ops:health` — evidence postures reported verbatim | gateway |

## How the backend is located

`aurora-backend`'s launcher (`plugins/aurora-backend/scripts/aurora-mcp.mjs`)
resolves the checkout from `AURORA_AGENT_ROOT`, then `~/aurora-agent`, then
`~/bioprism`, verifies `target/release/bioprism-mcp(.exe)`, and execs it with
`--root <checkout>`. A missing binary produces a precise stderr message with
the build command. The `aurora-context` commands resolve the CLI the same way.

## Maintaining

- `.agents/skills/` is the **source of truth** for the mirrored skills
  (aurora-workspace's five, aurora-honesty's `keep-a-claim-honest` and
  `prove-a-scanner-fires`). After editing them run
  `python tools/sync_plugin_skills.py` — mirrors carry a banner and must not
  be edited directly.
- Validation gate (CI-grade): `claude plugin validate . --strict` for the
  marketplace and `claude plugin validate plugins/<name> --strict` per plugin.
  All eight pass as of this commit.
- Releases: bump `version` in the plugin's `plugin.json` AND its marketplace
  entry (they must agree), then `claude plugin tag plugins/<name>` to create
  the `{name}--v{version}` tag.
- `plugins/` is deliberately outside `tools/coverage.sh`'s walk (`crates/` and
  `docs/` only), so nothing here can inflate the blueprint coverage figure.
  Keep NN.MM-shaped tokens out of plugin text anyway.
- The eval under `plugins/aurora-context/evals/` follows the documented
  `prompt.md + graders/*.md` form; `claude plugin eval` is early-access-gated
  at the time of writing.

## Known limits (stated, not implied away)

- `aurora-backend` requires a locally built checkout; the plugin does not
  download binaries.
- The `conformance_run` MCP tool bakes a build-machine fixtures path and only
  works when the checkout has not moved since it was built.
- This marketplace redistributes research and developer infrastructure. It
  does not diagnose an individual, recommend treatment, triage care, enroll
  participants, or claim medical-device functionality.
