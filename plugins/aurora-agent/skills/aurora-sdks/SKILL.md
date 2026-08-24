---
name: aurora-sdks
description: Integrate an application with the aurora-agent backend over HTTP or stdio. Use when asked to start or configure the bioprism-api gateway, call the backend from TypeScript or Python, pick between REST and MCP transports, or test MCP wiring without building the Rust binary.
---

# Integrating with aurora-agent

Three transports, one Rust implementation — a REST call and an MCP
`tools/call` reach exactly the same code and produce the same evidence-bearing
result.

## HTTP gateway (`bioprism-api`)

Launch (from the checkout root; `docs/HTTP_API.md` is the full reference):

```bash
cargo run -p bioprism-api -- --root . --bind 127.0.0.1:8787 \
  --token <at-least-16-visible-bytes> \
  --mission-state .local/mission-state.json \
  --mission-queue-state .local/mission-queue.json \
  --event-state .local/event-state.json \
  --reconciliation-state .local/reconciliation-state.json \
  --artifact-state .local/artifact-state.json
```

- Health (public): `GET /healthz`, `GET /readyz`. Machine contract:
  `GET /v1/openapi.json`; link index `GET /v1`.
- Any of the MCP tools: `POST /v1/tools/{name}` with a JSON object body;
  JSON-RPC at `POST /v1/rpc`.
- Bearer auth, constant-time compare; CORS for local tools; every response
  carries `X-Request-Id`. No TLS/HTTP2 — put a reverse proxy in front.
- Refusal semantics ride INSIDE the envelope: check
  `mcp.result.isError` plus the inner `{"ok": false, "error": ...}` — outer
  HTTP 200 does not mean the tool succeeded.

## TypeScript SDK

`typescript/` in the checkout — `@aurora-neuro/prism-sdk`, ESM,
**zero runtime dependencies**. `ApiClient` covers essentially the whole REST
surface (~349 methods) plus helpers: `waitMission`, `eventStream` (SSE),
`toolChecked` (schema-aware preflight then call). Build: `npm ci
--ignore-scripts && npm run build`; tests use Node's built-in runner.

## Python SDK

`python/` in the checkout — `prism-sdk`, `requires-python >= 3.11`,
**zero dependencies**, console script `aurora-agent`
(`python -m prism_sdk` also works). Sync + asyncio MCP clients and
`ApiClient`/`AsyncApiClient` for HTTP. Security invariant worth preserving in
anything you build on it: *the CLI parser deliberately has no API-key, token,
header, or secret argument* — keys come only from a no-echo prompt or a named
environment variable, and no shell is ever started by that boundary.

## Testing wiring without the Rust binary

`python/tests/fake_mcp_server.py` (and `autonomous_brain_mcp_server.py`) are
ready-made stdio harnesses: point your MCP client at them to test framing,
initialize handshakes, and error paths without a cargo build.
