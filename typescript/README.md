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

## BYOK onboarding and provider invocation

The autonomous runtime is BYOK: the application owns the key-entry or secret-manager boundary,
while `LLMRuntime` owns only short-lived, process-local credential handles. Keys must never be
placed in MCP arguments, prompts, tool results, model-selection state, telemetry, or durable job
records. A protected UI can submit a value through `collectUserCredential()`; a deployment with
no human key entry can register an environment variable or an external resolver through
`ProviderSetup` and `CredentialProvisioner` provide the complete setup process. The catalog ships
presets for OpenAI, Anthropic, DeepSeek, Groq, Mistral, OpenRouter, and xAI, with their official
wire protocols, paths, and conventional environment-variable names. A UI can render a redacted
setup plan before asking for a key:

```typescript
import { LLMRuntime, ProviderSetup } from "@aurora-neuro/prism-sdk";

const runtime = new LLMRuntime();
const setup = new ProviderSetup(runtime);
setup.registerProvider("openai");
const instructions = setup.instructions("openai");
// instructions.next_action === "collect_user_credential"
const session = setup.startSession({ ttlMs: 15 * 60_000 });
// The value must come from the app's password/secure-input boundary.
setup.collectUserCredential(session, "openai", userEnteredKey);
// pass session.handle("openai") to AutonomousRuntime/LLMRuntime, then close the session
```

For deployments without a human entry path, `CredentialProvisioner`:

```typescript
import { CredentialProvisioner, LLMRuntime, openaiProvider } from "@aurora-neuro/prism-sdk";

const runtime = new LLMRuntime();
runtime.registerProvider(openaiProvider());
const sources = new CredentialProvisioner(runtime.onboarding);
sources.registerEnvironment("openai", { variable: "OPENAI_API_KEY" });
// Or: await sources.registerResolver("openai", "secret-manager/openai", resolveSecret);

const session = runtime.onboarding.startSession({ ttlMs: 15 * 60_000 });
const provisioned = await sources.provision(session);
if (!provisioned.ready) throw new Error("provider credential provisioning was refused");

const answer = await runtime.invoke("openai", {
  model: "gpt-4.1-mini",
  messages: [{ role: "user", content: "Return a bounded answer." }],
  maxOutputTokens: 512,
}, { credential: session.handle("openai") });
session.close();
```

`providerPresets()` and `setup.plan()` are safe to send to a setup screen or readiness endpoint:
they contain provider metadata and the next action, never key values, handles, prompt text, or
resolver references. The protected UI should use a password input, disable echo/history and
browser autofill where appropriate, submit only over the application's authenticated transport,
and clear its local value immediately after `collectUserCredential()` returns. The SDK does not
persist the key or provide a fake “connected” state: readiness becomes usable only when the
provider is registered and an unexpired handle exists, and the first real invocation remains the
provider-access check.

`status()`, `instructions()`, and `plan()` return redacted readiness metadata for a UI or
operator dashboard. `CredentialSession.close()` and expiry revoke its handles; a process restart
requires fresh source registration and resolution. The runtime supports OpenAI Responses,
OpenAI-compatible Chat Completions, and Anthropic Messages, including bounded retries, circuit
breaking, streaming, structured-output validation, authorized tool loops, provider/model health,
and invocation outcome callbacks. `AutonomousRuntime` composes the selected-model handoff with
the invocation boundary: it gates disabled, unregistered, circuit-open, credential-unready, and
capacity-incompatible candidates, uses bounded health-weighted fallback ranking when no selector
is supplied, or accepts a value-only selector backed by the Rust/Python contextual bandit plane.
Provider failures remain typed and credential failures are not silently converted into generic
transport errors.

## Autonomous orchestration across all domains

`AutonomousAgent` is the application-facing composition layer for the autonomous brain. It covers
the twelve reviewed domains (`coding`, `browser`, `data`, `science`, `biomedical`,
`neuroscience`, `operations`, `enterprise`, `multi_agent`, `multimodal`, `cross_domain`, and
`evaluation`) with deterministic vocabulary routing, explicit abstention, cross-domain review,
workflow stage dependencies, bounded prompt assembly, exact tool binding, provider selection, and
value-only online learning. `route()` and `blueprint()` are non-executing: they return digests,
omissions, required capabilities, approval triggers, and a plan that explicitly has not started.

The live tool boundary is opt-in and catalogue-backed. Create a `ToolCatalogue` from the gateway's
tool definitions, pass it with a caller-owned executor, and inspect
`AutonomousDomainToolRegistry.plan()` before allowing a run. Read-only tools can be proposed
automatically; reversible or external-effect tools remain review rows and
`AutonomousDomainToolRuntime` requires explicit approval for each call. Registration is never
authorization, tool arguments/results are bounded and secret-shaped values are refused, and
receipts retain metadata and digests rather than hidden payloads.

The selector can be supplied directly, backed by `AutonomousOnlineLearner`, or bridged to
`ApiClient.brainModelSelectContextual()`. The bridge sends only model descriptors, health,
domain/capability/risk context, and bounded digests—not credentials, prompt transcripts, tool
arguments, or provider responses. `recordEvaluatorReward()` updates a local UCB learner only from
explicit evaluator feedback and can optionally submit the same value-only update to the control
plane. This is adaptation infrastructure, not an automatic truth signal: feedback must be produced
by a caller-owned evaluator, and provider health is kept separate from task quality.

## Contract boundaries

- Requests and responses are bounded. The client enforces a request byte ceiling, incrementally
  reads response streams, aborts at a timeout, and rejects non-object JSON responses.
- The bearer token is only sent in the `Authorization` header. Secrets are never copied into
  subscription views or client-side logs by the SDK.
- `callTool(name, arguments)` is the escape hatch for all current and future MCP tools. The typed
  helpers `traceOtelIngest`, `metricsProfileAudit`, `metricsAnalyticsAudit`, `bioCapabilityEvidenceAudit`,
  `bioAtlasPublicationAudit`, `repositoryCatalog`, `repositoryBundle`, `repositoryImpact`,
  `telemetryProject`, `developerDeliveryAudit`, `developerWorkbench`, `developerWorkbenchVerify`,
  `developerWorkbenchImport`, `developerWorkbenchQuery`, `developerWorkbenchGet`, `agentMission`,
  `capabilityDiscover`, `capabilityAudit`, `capabilityRoute`, `adapterPlan`,
  `capabilityRoutePlan`, `capabilityRoutePlanVerify`, and `capabilityRoutePlanVerifyRest`,
  `domainWorkflowPortfolio` and `domainWorkflowPortfolioQuery`,
  `domainWorkflowPortfolioVerify` and `domainWorkflowPortfolioVerifyQuery`,
  `domainWorkflowVerify` and `domainWorkflowVerifyQuery`,
  `runtimeExecutionSimulate`, `packCatalogue`, `packHealthAssess`, `securityRedteamSimulate`, and
  `worldGenerate`, `factoryLifecycleSimulate`, `storageLifecycleSimulate`, and
  `registryLifecycleSimulate`, and `cacheInvalidationSimulate`
  cover the highest-value
  cross-domain workflows without pretending
  to type every domain payload twice. Repository helpers keep catalog, route traversal, and
  changed-module impact requests explicit; `telemetryProject` preserves the event, treatment
  policy, trace, and optional observed-metric boundary without silently treating projected
  telemetry as a claim. Its `TelemetryProjectionResult` keeps record metadata, exact dropped /
  coarsened loss, and the supported-versus-refused metric union typed in the REST/MCP envelope.
- `ciProviderEvidenceImport`, `ciProviderEvidenceQuery`, and `ciProviderEvidenceGet` provide the
  retained provider-observed CI evidence index over MCP, with matching REST helpers. Imports are
  re-audited and idempotent; query rows preserve provider/run/plan identity and separate
  artifact/log/attestation record-family digests, including failed and unknown runs. Query arguments
  can require minimum local-byte hash and attestation subject-digest binding counts, and rows retain
  those counts. The client
  never treats these joins as fetched bytes, verified provider signatures, provider authentication,
  or release approval. `bundleVerify` separately exposes explicit Ed25519 bundle verification with
  typed key-validity and fail-closed refusal fields; it does not authenticate a key registry.
- `brainJobSubmit`, `brainJobStatus`, `brainJobEvents`, `brainJobApproval`, `brainModelHealth`,
  and `brainReplayEvaluate` expose the value-only autonomous-brain control plane. They accept
  metadata, bounded signals, and digests only; prompts, task payloads, provider responses, and
  credentials remain in the application-owned worker and `LLMRuntime` boundary.
- `traceOtelIngest` returns a typed normalized Event IR preview, OTLP mapping counts, loss-category
  ledger, and compilation-readiness state; it never implies OTLP export or collector connectivity.
- `qualityGateRun` returns typed serialized quality check unions, concrete failure witnesses,
  not-runnable reasons, and the distinct passed/failed/indeterminate verdict structure.
- `atlasReport` returns typed measured entries, explicit holes and coverage debt, family/depth
  evidence, inconsistency rows, bounded omissions, and fail-closed composite eligibility.
- `adaptivePanel` returns typed clustered audit totals, coverage shortfalls, stopping and estimate
  evidence, deterministic candidate selection, comparisons, and refusal states.
- `posteriorGate` returns the capability vector separately from an optional rationale-bearing
  release scalar and capability-wise dominance comparison. Its types preserve clustered means,
  ICC/effective sample, vetoes, provenance gaps, gate sensitivity, incomparable capabilities, and
  fail-closed policy/coverage refusals. See [`docs/POSTERIOR_GATE.md`](../docs/POSTERIOR_GATE.md).
- `ledgerIngest` keeps recorded, duplicate, and quarantined admission unions, causal releases,
  chain/clock evidence, temporal cuts, and digest-only latest-by-subject projections typed without
  turning the Fetch client into a durable event store.
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
- `hubCardRender` keeps moderation-derived publication states and the published/withheld score
  union typed, including provenance, limitations, non-claims, disclosure labels, and fail-closed
  attachment refusals without pretending the Fetch client publishes HTML.
- `hubLeaderboardRender` keeps rankable entries, typed unranked reasons, disclosure labels, and
  scoped headline nonclaims visible; `bioatlasPublicationAudit` keeps atlas/evidence/card/
  leaderboard gates and explicit release blockers separate from network publication.
- `hubSubmissionReview` keeps submission acceptance, moderation stage, event history, verification,
  publication state, and fail-closed refusal evidence typed without pretending the Fetch client is
  an identity provider or durable moderation service.
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
- Executable jobs retain `execution_provenance`, and `missionProvenance(missionId)` reads the
  correlated review, gate digest, domain-evaluator evidence, and accepted-dispatch event as a
  bounded audit projection; it never claims readiness or scientific validity.
- `AgentMissionArgs.claim_requests` carries bounded caller-authored claim rows. Terminal reports
  expose their non-semantic `claim_lineage`, and `missionClaimLineage(missionId)` reads the
  dedicated `/claims` projection with explicit retained-output and omission posture. Each claim can
  declare evaluator bindings with adapter id, domain, source step, and output pointer coverage.
- `missionFromRoute()` converts a completed `capabilityRoute()` response into a provenance-preserving
  mission assembly only after every need has one caller-selected candidate and explicit JSON
  arguments. It refuses unresolved or out-of-candidate tools, performs no network call, and is
  designed to feed `missionPreflight()` before `agentMission()`.
- `capabilityRoutePlan()` composes those explicit selections through the Rust route-review and
  mission-preflight boundaries. It returns a typed `CapabilityRoutePlanResult` with the reviewed
  mission, `plan_digest`, schema/preflight findings, and `dispatch: "not_started"`; it rejects
  `policy.execute: true` and leaves all tool selection and execution authorization with the caller.
- `capabilityRoutePlanVerify()` and `capabilityRoutePlanVerifyRest()` recheck a retained plan without
  dispatch. Supplying the original route and selections enables route-review replay; otherwise the
  typed result explicitly reports `verified_without_route_replay`.
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
