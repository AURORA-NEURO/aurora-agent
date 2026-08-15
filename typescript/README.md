# Prism TypeScript SDK

This package is a dependency-free TypeScript client for the bounded `bioprism-api` gateway. It
works in browsers, Node 18+, Deno-compatible fetch environments, and test harnesses that inject a
fetch implementation. The package does not recreate the Rust domain model or silently turn an
HTTP success into a scientific success: every tool call keeps the REST envelope and nested MCP
result available to the caller.

```typescript
import { ApiClient } from "@aurora-neuro/prism-sdk";

const api = new ApiClient({
  baseUrl: "http://127.0.0.1:8787",
  bearerToken: "0123456789abcdef",
});

const capabilities = await api.capabilities();
const result = await api.traceOtelIngest({
  trace_id: "notebook-trace-1",
  otlp_json: JSON.stringify({ resourceSpans: [] }),
  include_events: true,
});

if (result.mcp.result?.isError) {
  console.warn("The importer refused the trace", result.mcp.result);
}
```

## Contract boundaries

- Requests and responses are bounded. The client enforces a request byte ceiling, incrementally
  reads response streams, aborts at a timeout, and rejects non-object JSON responses.
- The bearer token is only sent in the `Authorization` header. Secrets are never copied into
  subscription views or client-side logs by the SDK.
- `callTool(name, arguments)` is the escape hatch for all current and future MCP tools. The typed
  helpers `traceOtelIngest`, `metricsProfileAudit`, `metricsAnalyticsAudit`, `bioCapabilityEvidenceAudit`,
  `bioAtlasPublicationAudit`, `repositoryCatalog`, `repositoryBundle`, `repositoryImpact`,
  `telemetryProject`, `developerDeliveryAudit`, `developerWorkbench`, `agentMission`,
  `capabilityDiscover`, `capabilityAudit`, `capabilityRoute`, `adapterPlan`, and
  `runtimeExecutionSimulate` cover the highest-value cross-domain workflows without pretending
  to type every domain payload twice. Repository helpers keep catalog, route traversal, and
  changed-module impact requests explicit; `telemetryProject` preserves the event, treatment
  policy, trace, and optional observed-metric boundary without silently treating projected
  telemetry as a claim.
- `toolCatalogue()` snapshots the live `/v1/tools` definitions into a bounded SHA-256 catalogue;
  `planTool()` performs conservative JSON-shape preflight without a POST; and `toolChecked()`
  executes the reviewed call while preserving the raw refusal envelope. This covers every current
  or future domain even when no handwritten helper exists. Unsupported schema keywords remain
  warnings, and preflight never represents domain validity, authorization, or scientific success.
- `missionPreflight()` performs the same no-side-effect review for `AgentMissionArgs`, while
  `assertMissionPreflight()` turns a failed report into a typed local error: together they return
  request and catalogue digests, deterministic dependency waves, JSON-pointer binding findings,
  execution allow-list failures, execution-mode budget checks, and per-step schema reports before
  `agentMission()` is sent. `execution_mode: "parallel_waves"` is an explicit opt-in for bounded
  concurrent dispatch of independent steps; `max_parallelism` caps each batch at 16 or less, and
  serial execution remains the default.
- `missionFromRoute()` converts a completed `capabilityRoute()` response into a provenance-preserving
  mission assembly only after every need has one caller-selected candidate and explicit JSON
  arguments. It refuses unresolved or out-of-candidate tools, performs no network call, and is
  designed to feed `missionPreflight()` before `agentMission()`.
- `eventStream` parses the gateway's bounded SSE snapshot and returns the `x-next-after` cursor;
  it is deliberately not a long-lived socket or an implicit reconnect loop.
- Webhook delivery is poll/send/acknowledge: `deliveries`, `retry`, and `acknowledge` operate on
  signed outbox envelopes. The SDK never opens arbitrary outbound connections to subscription
  endpoints.

## Development

```bash
npm install
npm test
```

The runtime has no production dependencies. TypeScript is a development-only compiler dependency;
consumers receive ESM plus declarations in `dist/`.

See [`docs/TYPESCRIPT_SDK.md`](../docs/TYPESCRIPT_SDK.md) for the complete route, error, safety,
and browser/Node integration contract.
