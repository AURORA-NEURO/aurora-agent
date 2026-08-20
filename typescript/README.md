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

Provider failures expose a stable, redacted `ProviderRuntimeError` contract for operators and
retry controllers. `code` distinguishes `http_4xx`, `http_5xx`, `transport`, `timeout`,
`aborted`, `circuit_open`, `protocol`, `invalid_response`, and bounded configuration/request
failures; `provider`, `operation`, `attempt`, `statusCode`, `requestId`, `retryable`, and the
bounded `retryAfterMs` hint are optional non-secret context. Upstream response bodies, headers
other than the bounded request-id/retry hint, authorization material, and thrown transport errors
are not retained. Caller aborts are terminal and never retried or counted toward the circuit;
deadlines are retryable when the configured attempt budget allows it. HTTP 4xx failures are
terminal except for explicitly retryable statuses such as 408/409/425/429, while 5xx failures
follow the bounded retry policy. Retry delays honor provider `Retry-After` values only within the
SDK's 60-second ceiling, and an external abort interrupts the delay immediately. Invocation
observers receive the matching `failureClass`, `failureCode`, `requestId`, and `retryable`
projection without prompt, response, tool, or credential data.

Model ids can be refreshed from the provider instead of being copied from stale configuration:

```typescript
const discovery = await setup.discoverModels(session, "groq");
const candidates = setup.modelCandidates(discovery, {
  context_window_tokens: 8_000,
  max_output_tokens: 512,
  quality: 0.8,
  latency_ms: 500,
  cost_per_million_tokens: 30,
  reliability: 0.9,
});
agent.registerModels(candidates);
```

Discovery uses the same short-lived credential handle, calls the provider's bounded `/models`
endpoint, and returns only ids, capacity metadata, active state, ownership, and two derived
capability labels. The raw catalog, authorization header, and key are discarded. Capacity comes
from the provider when available; quality, latency, cost, and reliability remain explicit
caller-owned priors rather than invented provider claims. A discovered model is not by itself
evidence that the provider will accept a future request, so invocation remains the live access
check and selection still applies credential, capability, capacity, approval, and health gates.

For restart-safe cross-runtime feedback, construct `AutonomousBrainControlPlaneBridge` around the
same `ApiClient` used for the Rust/Python brain tools and pass it as `modelHealthBridge` to
`AutonomousAgent`. The bridge mirrors invocation outcomes to `brain_model_health`, converts
caller-rehydrated replay cases to `brain_replay_evaluate`, and computes the shared evidence digest
without sending prompts, responses, tool payloads, credential handles, or keys. When no explicit
selector, learner, or contextual selector is supplied, the agent also reads persisted remote model
rows back into selection; local registration, credential readiness, capability, capacity, and
approval gates still decide eligibility. Use
`autonomousReplayEvidenceDigest()` when a replay artifact must be independently reproduced by
Python or Rust; all twelve built-in domain evaluator profiles use the same bounded signal policy.

## Autonomous orchestration across all domains

`AutonomousAgent` is the application-facing composition layer for the autonomous brain. It covers
the twelve reviewed domains (`coding`, `browser`, `data`, `science`, `biomedical`,
`neuroscience`, `operations`, `enterprise`, `multi_agent`, `multimodal`, `cross_domain`, and
`evaluation`) with deterministic vocabulary routing, explicit abstention, cross-domain review,
workflow stage dependencies, bounded prompt assembly, exact tool binding, provider selection, and
value-only online learning. `route()` and `blueprint()` are non-executing: they return digests,
omissions, required capabilities, approval triggers, and a plan that explicitly has not started.

For ambiguous or novel intake, `semanticRouteAutonomousTask()` adds an explicit provider-assisted
classification pass. It sends the private task only through the caller's approved local provider,
asks for structured domain scores against the reviewed twelve-domain catalogue, and maps the
provider's domain choice back to catalogue-authoritative capabilities and risk classes. The
deterministic route remains the safety baseline: provider/deterministic disagreement returns
`provider_disagreement` and preserves the deterministic route, while provider abstention and
malformed output remain explicit refusals. Routing still requires `approveProviderCall: true` and
never authorizes a tool, effect, or domain claim.

`runAutonomousDecisionCycle()` composes the single-domain path into one caller-controlled loop:
optional semantic routing, task-digest-validated route handoff, prompt and plan construction,
health/bandit model selection, provider invocation, and optional evaluator settlement. Semantic
routing approval and execution approval remain separate. Supplying an
`AutonomousLearningController` creates a pending episode; supplying an evaluator callback settles
it only from explicit bounded reward fields. The provider response, transport success, and model
self-report never become reinforcement automatically. Cross-domain cycles continue through
`runCrossDomain()` and `settleCrossDomain()` so specialist and synthesis episodes retain delayed
credit separately.

`runAutonomousReplanCycle()` adds the bounded adaptive control loop for callers that want the
evaluator to decide whether one answer deserves another attempt. Each completed attempt is sent
to a caller-owned evaluator that returns reward/pass/failure metadata plus `replan_requested` and,
when requested, a bounded instruction. The SDK caps additional attempts at three, reuses the
task-digest-validated route, and inserts only a required transient replan context chunk; it cannot
change the reviewed domain, capability requirements, tool allow-list, budgets, or approval state.
Approval-required, abstained, disagreement, invalid, and route-review outcomes terminate without
being evaluated as successful work. With `learning`, every completed attempt gets a distinct
pending episode and immediate value-only settlement, so later replanning cannot overwrite earlier
evidence. Attempt metadata contains evaluator fields and digests, while the final local cycle
result retains the normal local prompt/response boundary. Use a unique `episodePrefix` for each
logical cycle when persistence is enabled; no provider response or raw evaluator instruction is
sent to the remote learning plane.

When a controller is supplied, thrown semantic/provider dispatch failures, replan-transition
failures, and controller-completion failures fail the shared execution before being rethrown unless
the caller selects `executionLifecycle: "observe_only"` for an enclosing manager. Absent HTTP status
codes remain typed `null` metadata instead of causing a secondary journal validation error.

Long-running callers can pass an `AutonomousExecutionController` through the same run options.
`AutonomousExecutionPolicy` bounds steps, provider calls, provider failovers, tool calls, effectful
calls, replans, and caller-defined cost units. The runtime admits each provider request before
dispatch and each tool intent before authorization; a rejected admission prevents the external
operation. `InMemoryAutonomousExecutionJournal` is a hash-chained reference journal, while the
`AutonomousExecutionJournal` interface accepts caller-owned SQLite, IndexedDB, or object-store
adapters. Journal events retain only identifiers, counts, status, digests, timing, and evaluator
values. Controllers can pause, resume only against the same policy digest, and terminate
explicitly; prompts, responses, credentials, tool arguments, and raw outputs are rejected from
metadata. Replan cycles own the controller lifecycle across attempts so a later failure cannot
silently reset earlier provider or learning accounting.

`InMemoryAutonomousExecutionJournal.snapshot()` produces an integrity-checked, metadata-only
snapshot suitable for a caller-owned durable adapter. `AutonomousExecutionPersistenceCoordinator`
restores that snapshot before a worker resumes and flushes the current hash chain after a bounded
transition. Snapshot restore validates row sequence, event schemas, every event digest, the head
digest, event count, and byte capacity; tampered or payload-bearing snapshots are refused.

`stop_on_error` defaults to true: a non-retryable provider or failed tool outcome changes the
controller to a halted `error` projection until the caller explicitly fails the execution. Retryable
provider failures remain resumable so the runtime can perform bounded failover. `pause_on_approval`
controls whether an approval-required tool intent is projected as `approval_required`; it never
authorizes the effect, which still requires the caller's approval callback.

`AutonomousRuntime.invoke()` performs bounded provider failover when a selected provider returns a
retryable `ProviderRuntimeError`: it removes that provider from the next ranking, selects again
from the remaining eligible providers, and marks the retry as a failover admission. The default
limit comes from `execution.policy.max_provider_failovers`; without an execution controller the
default is zero, while callers can opt into a bounded standalone limit with
`maxProviderFailovers`. Tool loops may fail over only before the first provider-issued tool call;
after a tool request is observed, the runtime fails closed instead of replaying a potentially
effectful loop.

`runAutonomousCrossDomainDecisionCycle()` composes the corresponding fan-out/fan-in path. It can
semantically review an ambiguous task, requires a cross-domain route with at least two reviewed
domains, then runs bounded specialists and optional synthesis under the same provider and effect
approvals. Its evaluator callback must return an exact reward map for the returned episode IDs;
settlement applies delayed credit across the actual specialist/synthesis order. Partial runs and
`synthesize: false` remain settleable without inventing a synthesis episode.
If provider execution succeeds but evaluator settlement or memory projection fails afterward, the
cycle fails the shared execution controller before rethrowing, unless the caller explicitly selected
`executionLifecycle: "observe_only"` for a larger composition.

`InMemoryAutonomousEpisodicMemory` provides a bounded TypeScript reference for the same durable
memory boundary already available in the Python façade. It stores only task/route/prompt/plan/
selection/outcome digests, reviewed domain labels, caller-authored tags/lessons, and explicit
evaluator metadata. `retrieve()` is deterministic and can be attached to either decision cycle;
recalled rows become low-priority context with an explicit “prior metadata, not verified truth”
warning. `AutonomousMemoryPersistenceCoordinator` connects the store to a caller-owned database or
object-store adapter. Snapshots and event chains are content-addressed, and raw prompts, provider
responses, tool payloads, credentials, and secret-shaped fields are rejected.

`InMemoryAutonomousModelHealthStore` adds the restart-safe selection feedback plane. It records
separate value-only invocation and evaluator-quality observations, aggregates success/failure,
latency, quality, and circuit projections per provider/model arm, and exposes a deterministic
`AutonomousModelHealthController` selector plus invocation observer. Restore the content-addressed
snapshot through `AutonomousModelHealthPersistenceCoordinator` before constructing a worker; the
controller then makes historical quality influence the next model ranking without sending old
prompts or responses to a provider. `AutonomousOfflineReplayEngine` re-evaluates caller-rehydrated
numeric signal cases for all twelve domains, compares expected reward/pass/digest witnesses, and
never invokes a provider or turns a replay mismatch into authorization.
For the high-level facade, pass `modelHealthStore` in `AutonomousAgent` options; it automatically
uses the persisted selector and records invocation observations for single- and cross-domain runs.

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

Ambiguous tasks now have a real fan-out/fan-in path. `blueprint()` returns a
`cross_domain_blueprint` containing one child workflow per selected domain plus a cross-domain
synthesis workflow. `runCrossDomain()` executes those children under the same provider approval,
model selection, tool catalogue, and effect approval boundaries, then gives synthesis
bounded local child outputs with child status and digests. Fan-out uses a bounded worker pool
(default four, configurable with `maxParallelChildren` from 1 through 4) while preserving child
declaration order in results and learning episode IDs. A failed or approval-blocked child stops
synthesis by default; already in-flight children may finish, but no new child is scheduled after a
bounded failure. `allowPartial: true` makes partial synthesis an explicit caller choice, and
`synthesize: false` returns the specialist results without pretending that integration occurred.
Calling `run()` without an explicit domain automatically uses this path when routing selects more
than one domain.

For resumable single-domain workflows, `AutonomousWorkflowExecutor` turns the reviewed workflow
DAG into bounded stage calls. It checkpoints after each completed stage, pauses after
`maxStages`, records a metadata-only event chain, and resumes only when the caller supplies the
original task and the rebuilt workflow/plan digests match. `InMemoryAutonomousWorkflowCheckpointStore`
is suitable for tests and small workers; production applications should implement
`AutonomousWorkflowCheckpointStore` over their durable store. Checkpoints never contain task text,
prompts, provider responses, credentials, or tool payloads, and restart recovery explicitly
projects thrown provider failures as redacted `error_code`, `retryable`, `status_code`, and bounded
`error_class` metadata so workers can choose whether to retry or escalate without retaining the
provider message or response body. A failed stage remains terminal until the caller explicitly
rehydrates and chooses a new execution policy.
requires caller-owned task and credential rehydration.

`InMemoryAutonomousWorkflowCheckpointStore.snapshot()` and `restore()` provide an integrity-checked
multi-job restart boundary. `AutonomousWorkflowPersistenceCoordinator` connects it to a
caller-owned `read()`/`write()` adapter. Restore verifies checkpoint digests, generation metadata,
event predecessor digests, exact metadata-only field sets, job/event bounds, and the bounded snapshot
digest before mutating the store. Event history may be retention-truncated, but a truncated first
event must retain its predecessor digest; payload-bearing or recomputed snapshots are refused.

If `execution` is supplied in workflow run options, every stage forwards that same controller to
provider and tool admission, including bounded failover and custom read-only classification. The
workflow uses `executionLifecycle: "observe_only"` for nested stage calls so a stage cannot
silently complete the enclosing multi-stage execution; the caller owns final controller completion
after workflow reconciliation.

Pass an `AutonomousLearningController` to the executor when staged model outcomes should enter the
learning lifecycle:

```typescript
const learning = new AutonomousLearningController(agent, { episodes: durableEpisodeStore });
const executor = new AutonomousWorkflowExecutor(agent, durableCheckpointStore, { learning });
const execution = await executor.start(task, { domain: "coding", approveProviderCall: true, maxStages: 2 });
```

Each completed stage then creates a pending, digest-only episode and the checkpoint retains its
episode ID. After the caller-owned evaluator has produced signal scores, `settleWorkflow()` builds
the pending stage trajectory and applies delayed credit. Approval pauses, failed stages, and
provider refusals never create reward episodes. On restart, the caller rehydrates the task and
credential, reloads the checkpoint and episode store, and can continue the same value-only learning
trajectory without replaying private payloads.

When a deployment also needs a server-visible queue, `AutonomousDurableJobController` bridges that
local worker to the value-only `brain_job_submit`, `brain_job_status`, `brain_job_events`, and
`brain_job_approval` operations. Submission sends only an idempotency key, task/spec digest,
domain, capability, risk class, retry budget, priority, and optional checkpoint digest. The server
record is a control-plane projection: it never receives the task, prompt, model transcript, tool
payload, credential, or provider response. A worker reads the projection, honors a server-side
approval pause, rehydrates the task and an unexpired BYOK credential locally, and then runs the
matching domain workflow through `AutonomousWorkflowExecutor`. The controller validates the server
domain against the built-in catalogue and does not claim server completion; an external worker or
reconciliation process must record that relationship in its own system.

## Evaluator feedback and delayed-credit learning

`AutonomousWorkflowEvaluator` is the explicit reward boundary for the TypeScript brain. It derives
the signal contract from the selected workflow for all twelve built-in domains, accepts only
caller-declared bounded scores for declared stage signals, reports missing and rejected signals,
and produces a digest-bound reward. A provider response, HTTP success, model self-report, or
checkpoint status never becomes reward on its own. `builtinAutonomousDomainEvaluatorProfiles()`
exposes the evaluator ID, version, signal vocabulary, equal-weight defaults, and pass threshold for
readiness or review tooling.

`AutonomousLearningController.prepareRun()` converts a completed local run into a restart-safe
episode containing only run identity digests, selected provider/model, domain, capability, and
workflow identity. `settleRun()` applies an evaluator assessment to the caller-owned
`AutonomousOnlineLearner`, or sends the same value-only run identity and assessment through
`brain_outcome_record` when `remote: true`. `InMemoryAutonomousLearningEpisodeStore` is a bounded
reference store; a production worker should replace it with a durable store and rehydrate pending
episodes after restart. Settled episode identities cannot be reused.

For staged workflows or cross-domain fan-out, `prepareTrajectory()` and `settleTrajectory()` apply
bounded discounted return-to-go in reverse order. The trajectory persists episode IDs, arm IDs,
outcome digests, and settlement digests—not prompts, responses, task text, credentials, or raw
evaluator evidence. This gives delayed credit assignment without allowing later stages or a model
to rewrite earlier evaluator judgments.

Pass the same controller to `runCrossDomain()` through its `learning` option to automatically create
episodes for each completed specialist and the synthesis run. The returned
`learning_episode_ids` preserve declaration order, so `settleCrossDomain()` can require an exact
reward packet covering every pending child/synthesis episode. Partial or approval-blocked fan-out
returns only episodes for specialists that actually completed; no blocked child is silently rewarded.

For persistence, `InMemoryAutonomousLearningStateStore` is a bounded reference implementation that
combines episode and trajectory rows. Its `snapshot()` is content-addressed and its `restore()`
verifies the snapshot digest before accepting rows. Pair it with
`AutonomousLearningPersistenceCoordinator` and an application adapter implementing `read()` and
`write()` for SQLite, Postgres, IndexedDB, or object storage. The SDK never chooses a filesystem,
stores a provider secret, or assumes that a serialized snapshot is authorization to execute.

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
