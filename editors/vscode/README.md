# AURORA Agent for VS Code

Context engineering, with receipts — compile decision context, drive grant-authorised autonomous
workflows, and manage the AURORA Agent platform from VS Code.

The extension is a front-end for the `bioprism` command line, the `bioprism-mcp` MCP server, and
the `bioprism-api` HTTP gateway. It ships no platform logic of its own: every result comes from the
CLI's own JSON, summaries show limitations verbatim and digests in full, and every summary links to
its raw JSON — the file on disk when the CLI wrote one, otherwise a read-only virtual document
holding the CLI's exact output (dry-run outcomes, verify/explain/validate results).

Research and developer infrastructure: it does not diagnose an individual, recommend treatment,
triage care, enroll participants, or claim medical-device functionality.

## What it does

- **Compile context** — pick a world and a query, run `context compile` with `--certificate-out`
  and `--section-out` into `.aurora/`, and read the certificate as a faithful summary: verdict,
  selection counts, the omission-accounting table, full digests, and the certificate's
  `limitations` array verbatim. Verify and explain commands wrap `context verify --certificate`
  and `context explain`.
- **Workflows view** — lists the capability groups from `workflow catalogue --json` with their
  tools (missing tools stay visible). Context-menu actions scaffold a group (the generated steps
  open as an untitled document you review and save) and instantiate it. Instantiate runs the
  CLI's authoritative *no-dispatch* preflight; nothing is executed by either action.
- **Autopilot** — `grant-template` opens an editable grant document (authority for autonomous
  dispatch comes only from that document; there is no default grant). Run shows a confirmation
  modal listing the grant's `allowed_tools`, `allow_side_effects`, and `max_attempts` before real
  dispatch; Dry-Run needs no confirmation because `--dry-run` dispatches nothing and writes
  nothing. Reports land in the reports directory and render with `final_status`, totals, attempts,
  digests, and limitations verbatim; `autopilot verify --report` checks a report's digest.
- **Autopilot Reports view** — watches the reports directory (default
  `<workspace>/.aurora/reports`) and badges each report by its `final_status`
  (`succeeded` / `exhausted` / `refused`).
- **Pipelines view** — if the `gh` CLI is installed and authenticated, lists the last 10 GitHub
  Actions runs for the repository named by the workspace's `remote.origin.url`; a click opens the
  run in the browser. Without `gh` the view shows a single informational row.
- **Gateway** — starts `bioprism-api` as a VS Code task bound to `127.0.0.1:8787` (no `--token`
  is supplied; the gateway's own default is loopback), and stops it.
- **Doctor** — status-bar item plus a command that prints binary resolution, `bioprism --version`,
  root, MCP API availability, and `gh` presence to the "AURORA Agent" output channel. That channel
  also logs every command line the extension spawns.

Exit code 1 from the CLI is treated as what it is — a verdict, not an error. Failures surface the
CLI's own `retryability` word (`terminal`, `retryable_after_change`, `retryable_as_is`).

## Install

There is no Marketplace listing yet; sideload the `.vsix` from the release:

1. Download `aurora-agent-0.1.3.vsix` from the v0.1.3 release
   (https://github.com/AURORA-NEURO/aurora-agent/releases/tag/v0.1.3).
2. `code --install-extension aurora-agent-0.1.3.vsix`
   (or Extensions view → `...` → *Install from VSIX...*).

### Build from source (alternative)

From `editors/vscode`, run `npm install && npm run package` — it produces the same
`aurora-agent-0.1.3.vsix`, installed with the same `code --install-extension` step.

### Binary resolution

The extension finds the platform binaries in this order:

1. `auroraAgent.binaryDir` setting (a directory containing `bioprism(.exe)` and friends)
2. `AURORA_AGENT_ROOT` environment variable (a checkout with `target/release/`)
3. any workspace folder containing `target/release/bioprism(.exe)`
4. `~/aurora-agent`, then `~/bioprism`
5. the managed download cache

If nothing is found, commands offer to download the v0.1.3 release archive for your platform from
GitHub Releases. The download is verified against a SHA-256 pinned in the extension source; on
mismatch the file is deleted and nothing is extracted. Archives exist for Windows x64, macOS
x64/arm64, and Linux x64 (zip extraction uses the system `tar`).

## MCP / AI integration

On VS Code **1.101 or later** the extension registers an MCP server definition provider
(contribution point `mcpServerDefinitionProviders`, API
`vscode.lm.registerMcpServerDefinitionProvider`), publishing an "AURORA Agent" stdio server that
runs `bioprism-mcp --root <root>`. MCP-aware AI in VS Code — including Copilot agent mode — can
then call the platform's 259 tools (path-confined to the root). The API is stable in 1.101, which
is why `engines.vscode` is `^1.101.0`; a runtime capability check still guards registration so the
extension degrades gracefully in builds that lack the API.

For other MCP clients, **AURORA Agent: Copy MCP Configuration Snippet** puts a ready
`mcpServers` block (absolute binary path plus `--root`) on the clipboard for pasting into
`.mcp.json` or equivalent.

## Settings

| Setting | Default | Meaning |
| --- | --- | --- |
| `auroraAgent.binaryDir` | (empty) | Directory containing the `bioprism`, `bioprism-mcp`, `bioprism-api` binaries; highest-priority source. |
| `auroraAgent.root` | (empty) | Data root passed as `--root`; overrides the detected checkout root. |
| `auroraAgent.reportsDir` | (empty) | Reports directory for the Autopilot Reports view and `--report-out`; empty means `<first workspace folder>/.aurora/reports`. |

## Limitations

- No Marketplace listing yet — install is by sideloaded `.vsix` only.
- The managed download contains binaries only: its default root has **no fixtures**, so
  fixture-dependent features (the shipped reference worlds, for instance) need `auroraAgent.root`
  pointed at a checkout.
- Autopilot has no scheduling or recurrence, no cross-process resume, and no wall-clock deadlines;
  a run lives and dies with the process that started it.
- MCP registration depends on the `vscode.lm.registerMcpServerDefinitionProvider` API.
  `engines.vscode` (`^1.101.0`) already blocks installation on older VS Code, so the runtime
  guard exists for builds that report 1.101+ but do not implement the API (some VS Code-derived
  editors): there the extension's other features work and no MCP server is registered. Doctor
  reports which case applies.
- Tests are `node:test` unit tests of the extension's pure logic (pinned-hash table, resolution
  ordering, CLI envelope parsing, `gh` run mapping, grant summarising, summary rendering). There
  are no VS Code UI tests.
- Research and developer infrastructure: it does not diagnose an individual, recommend treatment,
  triage care, enroll participants, or claim medical-device functionality.
