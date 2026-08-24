# AURORA plugin marketplace

This repo doubles as a Claude Code plugin marketplace: the manifest is
`.claude-plugin/marketplace.json`, plugins live under `plugins/`.

```bash
# local checkout
claude plugin marketplace add /path/to/aurora-agent
# once hosted
claude plugin marketplace add AURORA-NEURO/aurora-agent

claude plugin install aurora-agent@aurora
```

## The plugin: `aurora-agent`

Everything needed to use the backend from any project, in one install:

- **MCP server** — the bioprism server (260 tools) via a root-resolving
  launcher (`scripts/aurora-mcp.mjs`: `AURORA_AGENT_ROOT` → `~/aurora-agent`
  → `~/bioprism`; precise stderr with the build command if the binary is
  missing).
- **Commands** — `/aurora-agent:compile`, `:explain`, `:compare`,
  `:validate`, `:verify` (FIBER CLI with the 10-exit-code retryability
  matrix interpreted), `:preflight` (missions), `:health` (operations).
- **Agent** — `ops-auditor`: control-plane evidence reported verbatim, never
  collapsed into "ready".
- **Skills** — `aurora-setup` (build + os error 4551 + boundary),
  `fiber-decision-context` (workflow + faithful-rendering rules),
  `mission-lifecycle` (DAG shape, fail-closed gates), `aurora-sdks`
  (gateway + TS/Python SDKs).
- **Eval** — `evals/compile-verdict/` (`prompt.md` + graders; `claude plugin
  eval` is early-access-gated at the time of writing).

Prerequisite: a built checkout — `cargo build --release --offline -p
bioprism-cli -p bioprism-mcp` (add `-p bioprism-api` for the HTTP gateway).

## Optional packs

| Plugin | Gives you |
|---|---|
| `aurora-honesty` | The honest-labelling discipline for **any** codebase (4 skills) |
| `aurora-workspace` | Skills for working inside this repo itself (5 skills, scope-prefixed) |

## Maintaining

- `.agents/skills/` is the **source of truth** for mirrored skills; after
  editing run `python tools/sync_plugin_skills.py` (mirrors carry a
  do-not-edit banner).
- Validation gate: `claude plugin validate . --strict` (marketplace) and
  `claude plugin validate plugins/<name> --strict` per plugin.
- Releases: bump `version` in the plugin's `plugin.json` AND its marketplace
  entry (they must agree), then `claude plugin tag plugins/<name>`.
- `plugins/` is outside `tools/coverage.sh`'s walk (`crates/` + `docs/`
  only) — nothing here can inflate the blueprint coverage figure. Keep
  NN.MM-shaped tokens out of plugin text anyway.

## Known limits (stated, not implied away)

- The plugin does not download binaries; it requires a locally built checkout.
- The `conformance_run` MCP tool bakes a build-machine fixtures path and only
  works when the checkout has not moved since it was built.
- This marketplace redistributes research and developer infrastructure. It
  does not diagnose an individual, recommend treatment, triage care, enroll
  participants, or claim medical-device functionality.
