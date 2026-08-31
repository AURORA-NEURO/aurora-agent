# Codex/Claude MCP integration contract

`integrations/codex-claude` is a dependency-free, provider-neutral configuration adapter. One
`AuroraServerSpec` and one Aurora capability registry feed both host profiles; rendering emits
deterministic Codex-style TOML or Claude-style JSON and *refuses by name* every provider-specific
setting this adapter cannot prove compatible.

The package emits **configuration only**. It performs no network connectivity, no OAuth, no
provider SDK execution, starts no process, and never resolves an environment variable. Readiness is
therefore always `unmeasured`; nothing here should be read as a claim that a generated file has
ever been executed by either host.

## The single registry

`AURORA_CAPABILITIES` is one static snapshot of MCP tool names taken from the `bioprism-mcp`
launcher contract (`crates/mcp/src/main.rs`, the tool list printed by `--help`). Allowlists are
validated against it and fail closed on unknown names, duplicates and empty sequences. The live,
authoritative catalogue remains what the server itself advertises via `tools/list` and the
`bioprism://capabilities/0.1` resource; `tests/test_registry.py` re-asserts that every snapshot
name still appears in the launcher source, so drift fails loudly instead of silently shrinking an
allowlist.

```python
from aurora_codex_claude import aurora_stdio_spec, render_config

spec = aurora_stdio_spec(root=".", allowlist=["fiber_compile", "fiber_verify"])
print(render_config("codex", spec))
print(render_config("claude", spec))
```

## Generated shapes

Claude-style output is canonical compact JSON under `mcpServers` (sorted keys, LF, placeholders
byte-for-byte visible). Stdio entries mirror the repository root `.mcp.json` contract:

```json
{"mcpServers":{"aurora":{"args":["--root","."],"command":"./target/release/bioprism-mcp","env":{}}}}
```

Codex-style output is deterministic TOML under `[mcp_servers.<name>]`:

```toml
[mcp_servers.aurora]
command = "./target/release/bioprism-mcp"
args = ["--root", "."]
env = { MODE = "offline" }
```

The default command matches `.mcp.json`; on Windows the built artifact gains an `.exe` suffix and
the caller passes the path appropriate for its checkout. The adapter never probes the filesystem.
Remote endpoints are emitted for both profiles (`url`-shaped entries) but remain configuration:
nothing in this package opens a connection.

## Compatibility matrix

| Capability | Claude-style | Codex-style | Why |
|---|---|---|---|
| stdio argv entry (`command`/`args`) | emitted | emitted | documented on both surfaces |
| `env` map on stdio entries | emitted (always stated, mirroring `.mcp.json`) | emitted only when non-empty | inline-table convention |
| remote HTTP entry | emitted: `{"type":"http","url",...}` | emitted: `url`-only table | both hosts document URL entries |
| headers on remote entries | emitted | **refused** | Codex header key names have varied across versions; not provable |
| `${VAR}` / `${VAR:-default}` placeholders | kept literal; host expands at connect time (unset stays literal) | **refused anywhere** | expansion inside `config.toml` values is not documented behaviour we can rely on; a placeholder could reach the server as literal text |
| secrets in env/headers | literals rejected; placeholders only | literals rejected; placeholders refused with them | see above — provide secrets in the server process environment |
| per-tool allowlist | emitted as `mcp__<server>__<tool>` permission entries | **refused** (stays adapter-side) | no proven per-server allowlist surface |
| OAuth / auth acquisition | refused (`oauth`, `auth`, `bearer_token_env_var`, `headersHelper`) | refused | credentials are out of scope for generated files |
| SSE / WebSocket transports | refused (`sse`, `ws`) | refused | only stdio and streamable HTTP shapes are emitted |
| host timeout keys (`startup_timeout_*`, `tool_timeout_*`, `timeout`) | refused | refused | key names and units differ across host versions |
| readiness probing | always `unmeasured` | always `unmeasured` | configuration is not execution |

Every refusal raises `UnsupportedFeatureError` (or `ConfigError`/`UnsafeCommandError` for malformed
or unsafe input) naming the offending field. Nothing is ever silently dropped from a request.

## Secret handling

Values are never resolved. A value under a secret-bearing name (`TOKEN`, `SECRET`, `PASSWORD`,
`API_KEY`, `AUTHORIZATION`, …, matched case-insensitively with `-` normalized to `_`) must be a
full-string placeholder where the profile permits placeholders at all, and is otherwise rejected;
diagnostics redact such literals rather than repeat them. As a belt-and-braces check every dump
function scans its own output against `os.environ` and aborts if a secret-bearing variable's value
appears — including variables nothing referenced, because the leak path of a bug would be unknown
by construction.

## Limits

This package does not implement OAuth, token acquisition, transport negotiation, retries, process
supervision or credential rotation, and it does not embed the live capability catalogue (see *The
single registry*). Host config *locations* (`~/.claude.json`, project `.mcp.json`,
`~/.codex/config.toml`) and version-specific extras are deliberately out of scope; the adapter
writes document text, not user profiles.

## Verification

From `integrations/codex-claude`:

```text
python -m unittest discover -s tests -t . -v
python -m compileall -q aurora_codex_claude tests
```

`tests/test_registry.py` grounds the snapshot against `crates/mcp/src/main.rs` when run inside the
repository and skips with a named reason in standalone checkouts; `tests/test_render_determinism.py`
normalizes the repository's own `.mcp.json` through the adapter as a parity check.
