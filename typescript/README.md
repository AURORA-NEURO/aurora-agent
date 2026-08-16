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
  `capabilityDiscover`, `capabilityAudit`, `capabilityRoute`, `adapterPlan`,
  `runtimeExecutionSimulate`, `packCatalogue`, `packHealthAssess`, `securityRedteamSimulate`, and
  `worldGenerate`, `factoryLifecycleSimulate`, `storageLifecycleSimulate`, and
  `registryLifecycleSimulate`, and `cacheInvalidationSimulate`
  cover the highest-value
  cross-domain workflows without pretending
  to type every domain payload twice. Repository helpers keep catalog, route traversal, and
  changed-module impact requests explicit; `telemetryProject` preserves the event, treatment
  policy, trace, and optional observed-metric boundary without silently treating projected
  telemetry as a claim.
- `packHealthAssess` keeps observed calibration counts, discrimination, health findings, digest
  binding, and score withholding in one raw REST/MCP envelope. A saturated, contaminated, or
  otherwise unreportable pack remains inspectable, but its numeric score is explicitly absent;
  `reportable: false` is not a zero.
- `securityRedteamSimulate` returns separately typed regression, disclosure, trust-boundary,
  incident-containment, audit-chain, and attestation evidence. It models safety contracts only:
  permitted crossings are not observed transfers and requested containment is not execution.
- `worldGenerate` keeps synthetic world/query documents, exact digests, structural counts, and
  validation diagnostics visible in one bounded response; it performs no file, network, model,
  clinical, or publication action.
- `factoryLifecycleSimulate` keeps the ordered lifecycle trace, lease/recovery variants,
  staged-versus-committed output boundary, final job snapshots, quarantine/dead-letter views, and
  fail-closed refusals typed without pretending the Fetch client is a queue or worker runtime.
- `storageLifecycleSimulate` keeps caller-epoch tier plans, pin-held and skipped-tier reasons,
  dry-run/application state, reserve-aware quota rows, and non-copyable child allowance accounting
  typed without pretending the Fetch client is a storage scheduler or backend.
- `registryLifecycleSimulate` keeps pack preflight, serialized-index integrity, append-only action
  rows, final verification, and continuation state typed without pretending the Fetch client is a
  signed or networked package registry.
- `cacheInvalidationSimulate` keeps component-complete keys, dependency opacity, partial plans,
  explicit apply state, hit proofs, reasoned misses, unproven entries, and attributed reproofs typed
  without pretending the Fetch client is a cache scheduler.
- `hubDisclosureReview` keeps digest-keyed disclosure ratchets, contamination witnesses,
  split-integrity findings, caveated headline labels, withheld scores, and fail-closed action
  refusals typed without pretending the Fetch client detects leaks or publishes a hub page.
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
  serial execution remains the default. Executed `agentMission()` responses expose the authoritative
  clock-free `execution_trace` with contiguous lifecycle, wave, refusal, block, and byte-accounting
  events.
- `submitMission()`, `missionStatus()`, and `cancelMission()` provide typed asynchronous mission
  jobs. Cancellation is cooperative between nested calls or parallel batches, and terminal reports
  preserve the authoritative Rust trace rather than claiming force-kill or rollback. `deleteMission()`
  removes only terminal jobs from the bounded process-local registry.
- `preflightMission()` calls the synchronous Rust-owned `/v1/missions/preflight` route; it validates
  the original execution policy and returns a planned report with `dispatch: "not_started"` without
  creating a job or invoking a domain tool. `missionPreflight()` remains the local catalogue review.
- `missions(status, limit)` returns a deterministic bounded inventory of lifecycle summaries and
  links without materializing unbounded terminal reports.
- `MissionJob.progress` is a typed bounded live projection with phase, current wave,
  active/completed counts, outcome counters, returned bytes, and the latest trace cursor/event;
  terminal results and traces remain authoritative.
- `missionTrace(missionId, after, limit)` pages the retained authoritative trace into a typed
  `MissionTracePage`; the exclusive `next_after` cursor and any retention gap are explicit.
- `missionFromRoute()` converts a completed `capabilityRoute()` response into a provenance-preserving
  mission assembly only after every need has one caller-selected candidate and explicit JSON
  arguments. It refuses unresolved or out-of-candidate tools, performs no network call, and is
  designed to feed `missionPreflight()` before `agentMission()`.
- The Rust mission executor performs the final authoritative JSON Schema check against the live
  `tools/list` definitions, including a second check after bindings are materialized; schema
  refusals carry bounded JSON-pointer diagnostics and a schema digest before nested dispatch.
- `eventStream` parses the gateway's bounded SSE snapshot and returns the `x-next-after` cursor;
  it is deliberately not a long-lived socket or an implicit reconnect loop.
- Webhook delivery is poll/send/acknowledge: `deliveries`, `retry`, `replay`, and `acknowledge` operate on
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
