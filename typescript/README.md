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

For an atomic live-catalog reconciliation, the agent can perform discovery and registration as one
bounded operation:

```typescript
const refresh = await agent.refreshModels("groq", {
  context_window_tokens: 8_000,
  max_output_tokens: 512,
  quality: 0.8,
  latency_ms: 500,
  cost_per_million_tokens: 30,
  reliability: 0.9,
}, { credential: session.handle("groq"), replaceExisting: true });
```

Discovery is completed and every candidate is validated before the in-memory catalogue changes.
Non-replacing conflicts fail the full batch rather than partially registering new models.

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
The built-in workflow contracts are also mirrored across Python and TypeScript: each domain has
its own stage objectives, evidence outputs, evaluator signals, dependencies, and approval posture.
Workflow digests are identical across the two SDKs, which binds checkpoints and learning
trajectories to the same reviewed domain contract.

## Autonomous orchestration across all domains

### Keyless readiness audit

`AutonomousAgent.readiness()` is the TypeScript-side preflight for the complete autonomous
brain. It is intentionally safe to call before a user has entered a key and does not discover
models, invoke a provider, execute a tool, or serialize credential material. It audits all twelve
built-in domains in one report: provider registration and circuit state, opaque credential
readiness, model capability/capacity compatibility, exact live-tool coverage, domain workflow
digests, learning-context digests, and actionable next steps.

```typescript
const report = await agent.readiness();

for (const domain of report.domains) {
  console.log(domain.domain, domain.state, domain.next_actions);
}
// report.execution === "not_started; no_provider_or_tool_calls"
// report.secret_material === "never_returned"
```

The readiness state is deliberately more precise than a boolean: `model_catalogue_required`,
`provider_registration_required`, `credential_required`, `model_capability_gap`,
`ready_for_caller_approval`, or `partial`. An empty local model catalogue is valid input for the
audit (`agent.readiness({ candidates: [] })`) and produces the first state rather than an
exception. A model is eligible only when its provider is registered, its declared capabilities
and token limits satisfy the domain, its provider circuit is not open, and its credential gate is
ready. `ready_for_caller_approval` still does not authorize a provider call or an effect.

Tool names are compared against the caller-owned `ToolCatalogue` as metadata only. Missing tools
are reported per domain, while catalogue registration remains separate from tool authorization and
effect approval. Learning is likewise reported as configuration (`AutonomousOnlineLearner`) and
context identity; provider transport success is never converted into evaluator reward. The report
is digest-addressed under `bioprism-autonomous-agent-readiness/0.1`, contains no task text or
provider payload, and can be rendered directly by a setup or operations screen.

### Keyless capability activation and restart-safe tool admission

Readiness is descriptive; activation is the explicit lifecycle that turns a reviewed catalogue
into a narrow runtime allow-list. `AutonomousCapabilityActivation` records provider posture,
catalogue/profile/plan digests, twelve domain coverage rows, proposed bindings, approvals,
revision, and a state digest. It never stores a key, opaque credential handle, secret-manager
reference, task, prompt, provider response, tool arguments, or tool output. `ready` means that the
metadata path is approved; it does not authorize external effects or replace live provider checks.

The application flow is:

1. Register provider transports and model metadata; collect the user's key through
   `ProviderOnboarding`/`CredentialSession` and pass only the opaque handle at invocation.
2. Call `refreshActivation()` to record the keyless readiness audit and exact all-domain tool
   plan. This performs no provider or tool call.
3. Show the plan digest, coverage, missing tools, pending review tools, and proposed read-only
   names. Approve only names from that exact plan with `approveActivationBindings()`.
4. Persist the redacted state through a caller-owned adapter. On restart, restore metadata and
   collect credentials again; never restore a credential from activation state.
5. The agent enforces approved names for live tools, explicit `tools`, custom authorizers, and
   direct `executeToolCalls()` calls. A changed catalogue, profile, or plan invalidates prior
   approvals and becomes `stale`; `revokeActivation()` closes the gate immediately.

```typescript
const activation = new AutonomousCapabilityActivation({ activationId: "workspace-01" });
const agent = new AutonomousAgent(llm, {
  activation,
  toolCatalogue: liveCatalogue,
  toolExecutor: executeCallerOwnedTool,
});

const posture = await agent.refreshActivation();
const registry = await AutonomousDomainToolRegistry.create(liveCatalogue);
const plan = await registry.plan();
// Render posture.status, plan.plan_digest, plan.coverage, and plan.review_required_tools.
agent.approveActivationBindings(
  plan,
  plan.proposed_bindings.map((binding) => binding.name),
  liveCatalogue.definitions.length,
);

const activationStore = new AutonomousCapabilityActivationStore();
await agent.saveActivation(activationStore);
const persistence = new AutonomousCapabilityActivationPersistenceCoordinator(
  activationStore,
  callerOwnedJsonPersistence,
);
await persistence.flush();
```

The activation snapshot is bounded and SHA-256 sealed. It rejects unknown or secret-shaped fields,
duplicate providers/tools/domains, unsupported domains, oversized metadata, invalid revisions,
stale digests, and tampered persistence envelopes. Provider projection accepts only readiness
metadata such as registration, circuit, credential count, and next action; the key value itself is
structurally unrepresentable in activation state.

### Deterministic task-to-capability portfolios

The registry also exposes `planForTask(task)`, which makes the default tool decision explicit. It
reviews the workflow stages for every selected domain, binds only exact names present in the live
catalogue, ranks candidates by stage coverage, requested capability, local task relevance, and
read-only posture, and caps the result with `maxTools`. The planner is provider-free: task text is
used only for local ranking and the public result retains a digest, never the task. `blueprint()`
uses this portfolio automatically for single-domain, cross-domain child, and synthesis workflows;
`run()` then exposes only the names that survived blueprint compilation and activation filtering.

```typescript
const registry = await AutonomousDomainToolRegistry.create(liveCatalogue);
const portfolio = await registry.planForTask(
  "debug the repository, verify CI, and report reproducible findings",
  { domains: ["coding", "evaluation"], maxTools: 12 },
);

// Inspect before any provider or tool call.
console.log(portfolio.selected_tool_names, portfolio.coverage, portfolio.plan_digest);
// coverage.status explains selected, activation_required, catalogue_missing,
// provider_only, and capacity_limited stages.
```

This is selection metadata, not authorization. `selected_bindings` still carry approval posture,
the activation allow-list can only narrow the portfolio, and an effect boundary must approve every
effectful call. A missing catalogue definition is reported as `catalogue_missing`/`missing_tools`
instead of being silently treated as an available capability. The plan is sealed under
`bioprism-typescript-autonomous-capability-plan/0.1` and explicitly states that it made no
provider or tool calls.

### Stage-bound adapter execution and evidence

The live adapter path is stricter than portfolio selection. A workflow executor constructs an
`AutonomousWorkflowToolContext` containing the selected domain, exact workflow id/digest, and
current stage id. `AutonomousDomainToolRuntime` then rechecks the identity against the reviewed
workflow before dispatching each call. It requires the binding to satisfy the stage's explicit
capability aliases, refuses effectful tools in read-only stages, and refuses approval-gated tools
when the stage contract does not declare approval. A tool that is registered, live, and valid for
the domain can still be refused for the wrong stage.

```typescript
const result = await agent.executeToolCalls(
  [{ id: "call-1", name: "repository_catalog", arguments: {} }],
  {
    domains: ["coding"],
    approveEffects: false,
    workflowContext: {
      domain: "coding",
      workflow_id: blueprint.workflow.workflow_id,
      workflow_digest: blueprint.workflow.workflow_digest,
      stage_id: "inspect",
    },
  },
);

// Metadata-only adapter evidence: no raw arguments, result, prompt, or credential.
const receipts = agent.toolExecutionEvidence();
```

Each receipt binds the tool, capability, schema digest, workflow/stage identity, stage-contract
digest, required evidence-output labels, effect posture, result digest, and redacted outcome status.
`evidence_status: "tool_execution_only"` is intentional: dispatch success is not task success,
and a result digest is not evidence that the external world changed as intended. Evaluators still
must validate the stage's declared evidence and signals. `autonomousWorkflowStageContractDigest()`
is available when a caller-owned adapter or audit store needs to reproduce the exact stage identity.
Legacy domain-only `authorizeAndExecute()` calls remain available for direct integrations, but
durable workflow execution always uses the stricter stage-bound path.

For an evaluator-ready adapter result, use `executeCapability()` or
`AutonomousCapabilityRuntime` above the same runtime. The capability envelope requires an
input digest, preserves workflow/stage identity, hashes arguments and transient output, and lets
the caller project bounded observations such as provenance, measurements, limitations, and
warnings. Only the returned `value` is transient; persist `result.record` and its
`evidence_digest`. A completed adapter call still reports `missing_required_outputs` until the
projector declares every stage evidence label, and `declared_for_evaluator` still does not mean
the task passed. Repeated completed requests replay from a bounded in-memory idempotency cache;
ordered batches make any stop-on-failure omissions explicit.

```typescript
const capability = await agent.executeCapability({
  call_id: "coding-scope-1",
  tool: "repository_catalog",
  arguments: {},
  workflow_context: {
    domain: "coding",
    workflow_id: blueprint.workflow.workflow_id,
    workflow_digest: blueprint.workflow.workflow_digest,
    stage_id: "scope",
  },
  input_digest: taskDigest,
}, {
  projectObservations: async (value) => [{
    id: "repository-observed",
    label: "scope",
    kind: "fact",
    status: "observed",
    value_digest: await digestJson(value),
  }],
});
// Durable storage: capability.record; transient application use: capability.value.
```

For restart-safe capability idempotency, provide a caller-owned
`InMemoryAutonomousCapabilityJournalStore` in tests or implement
`AutonomousCapabilityJournalStore` over the application database/queue. The journal accepts only
fresh `AutonomousCapabilityExecutionRecord` metadata, verifies each row with a SHA-256 hash chain,
rejects duplicate or conflicting request identities, and never stores arguments, prompts, raw values,
credentials, or response content. `AutonomousCapabilityJournalPersistenceCoordinator` flushes and
restores a complete digest-bound snapshot through a caller-owned persistence adapter:

```typescript
const journal = new InMemoryAutonomousCapabilityJournalStore();
const agent = new AutonomousAgent(llm, {
  toolCatalogue,
  toolExecutor,
  capabilityJournal: journal,
});

// On worker startup, after the caller restores the journal snapshot into `journal`:
await agent.restoreCapabilityJournal();
const replay = await agent.executeCapability(theSameRequest);
// replay.record.replay === "replayed"; replay.value === null after a restart.
```

Restart replay deliberately returns no adapter value: only the output digest and evaluator-facing
metadata are durable. This prevents a persistence layer from becoming an accidental raw-result
store. If the transient value is required, the caller must retain it in a separately governed
application store and bind that store to its own digest/provenance policy; the autonomous runtime
will not redispatch a completed capability merely to reconstruct a discarded value.

Direct capability learning has the same restart discipline. `evaluateCapabilityExecution()` and
`evaluateCapabilityExecutions()` pass only a metadata projection to the caller-owned evaluator,
then settle an explicit reward through the local bandit callback. Provide an
`InMemoryAutonomousCapabilityLearningSettlementStore` in tests or implement
`AutonomousCapabilityLearningSettlementStore` over the application database. The in-memory store
also implements `AutonomousCapabilityLearningSnapshotStore`: its bounded receipt snapshot is
SHA-256 bound, validates every nested settlement and next-state digest, rejects duplicate keys,
and can be flushed/restored with `AutonomousCapabilityLearningPersistenceCoordinator`. A replay
loads the receipt and adopts its persisted next bandit state without re-running the evaluator or
crediting the arm twice. Learning snapshots contain value-level reward and bandit metadata only;
capability arguments, adapter values, prompts, responses, credentials, and raw evaluator evidence
are rejected at the persistence boundary.

`AutonomousAgent` is the application-facing composition layer for the autonomous brain. It covers
the twelve reviewed domains (`coding`, `browser`, `data`, `science`, `biomedical`,
`neuroscience`, `operations`, `enterprise`, `multi_agent`, `multimodal`, `cross_domain`, and
`evaluation`) with deterministic vocabulary routing, explicit abstention, cross-domain review,
workflow stage dependencies, bounded prompt assembly, exact tool binding, provider selection, and
value-only online learning. `route()` and `blueprint()` are non-executing: they return digests,
omissions, required capabilities, approval triggers, and a plan that explicitly has not started.
Each generated blueprint includes a 64-character `learning_context_digest`. The digest scopes
local evaluator feedback by the canonical domain, capability, risk class, and task-family labels
shared with Rust and Python. `AutonomousOnlineLearner` prefers the matching contextual arm, uses a
global arm only as a cold-start prior, and keeps context-free updates compatible with the legacy
global ledger. Contextual state is bounded and replay-safe; prompts, responses, credentials, and
raw evaluator evidence are never placed in the digest or bandit snapshot. The remote value-only
control-plane update remains backward-compatible, while the TypeScript runtime retains the
contextual overlay locally when an older server does not persist it. Contextual calls now fail
closed unless the digest is exactly the SHA-256 of the normalized `{domain, capability, risk_class,
task_family}` identity (with an explicit `null` task family when absent), matching Rust and Python;
the synchronous digest helper is browser-safe and does not require Node crypto or provider access.
The local learner supports UCB1, seeded epsilon-greedy, and deterministic Thompson-sampling
policies, applies explicit failure-rate penalties, and records exploration metadata so a caller can
replay an adaptive decision without hidden randomness. Thompson rankings include bounded
Beta-posterior alpha, beta, and sample evidence. All three policies consume only explicit
evaluator rewards; provider transport success is never silently converted into learning credit.

For ambiguous or novel intake, `semanticRouteAutonomousTask()` adds an explicit provider-assisted
classification pass. It sends the private task only through the caller's approved local provider,
asks for structured domain scores against the reviewed twelve-domain catalogue, and maps the
provider's domain choice back to catalogue-authoritative capabilities and risk classes. The
deterministic route remains the safety baseline: provider/deterministic disagreement returns
`provider_disagreement` and preserves the deterministic route, while provider abstention and
malformed output remain explicit refusals. Routing still requires `approveProviderCall: true` and
never authorizes a tool, effect, or domain claim.

Model selection accepts caller-owned hard gates through `maxCostPerMillionTokens`, `maxLatencyMs`,
`minQuality`, and the optional `minSelectionConfidence` rank-separation floor. The same gates are
enforced by local health-aware ranking, contextual
Rust/Python selection, and `AutonomousOnlineLearner` before a model can be chosen. Refused models
remain explainable in the ranking; near-tied eligible models can now abstain with normalized
selection-confidence evidence, and an empty eligible set fails closed before provider dispatch.
Selection confidence is routing stability, not answer correctness.

For aggregate control over a composed run, pass `maxTotalCostUnits` or create one
`AutonomousCostBudget` and pass it through the composed boundary. The shared budget covers
semantic classification, provider failover attempts, every tool-loop turn, cross-domain specialist
fan-out, synthesis, and decision-cycle execution. Reservations are atomic for local concurrent
workers; provider attempts that reach dispatch remain charged, while pre-dispatch admission
failures release their reservation. This aggregate ceiling complements, rather than replaces, the
optional `AutonomousExecutionController` cost policy.

The provider-assisted semantic classifier used by `semanticRouting.enabled` is also a model
invocation, so decision cycles forward the caller's cost, latency, and quality gates to it before
transport. A successful classifier result does not authorize execution: the cycle still requires
the separate execution approval and applies the same gates again to the routed domain run.

Contextual model selections resolve exact `provider/model` IDs. A model-only ID is accepted only
when it matches one registered candidate; duplicate matches abstain before provider dispatch.

### Provider-assisted planning proposals

The TypeScript façade also exposes the provider-planning boundary used by the Python brain. Build
the deterministic blueprint first, then make a second, separately approved call when a model
should prioritize an existing workflow:

```typescript
const intake = await agent.blueprint("Debug this repository and verify the fix.", { domain: "coding" });
if (!intake.blueprint) throw new Error("routing did not produce a single-domain blueprint");

const proposal = await agent.planWithProvider(intake.blueprint, {
  approveProviderCall: true,
  credential: session.handle("openai"),
  maxOutputTokens: 1_024,
});

if (proposal.status !== "completed" || proposal.review_required) {
  await reviewQueue.enqueue(proposal); // caller-owned acceptance boundary
}
```

`planWithProvider()` can reorder only the exact reviewed stage identifiers and return a focus
subset. `planCrossDomainWithProvider()` applies the same contract to the existing specialist
child identifiers. Both methods require explicit approval, use the normal model-selection,
credential, aggregate budget, health, retry, and abort gates, and return `approval_required` without
network dispatch when approval is absent. Pass `maxTotalCostUnits` for a planning-local ceiling or
one `AutonomousCostBudget` to charge planning and the eventual execution against the same caller-
owned accounting boundary. The returned `cost_budget` is a numeric value-only snapshot; a zero or
exhausted budget fails before provider dispatch. Provider output is checked for strict JSON shape, exact
permutations, dependency safety, abstention, and confidence bounds. A malformed structured
response is converted into a typed `provider_invalid` result with a digest-only failure receipt;
credential and transport failures remain typed runtime errors for the caller's retry policy.

For applications that want the whole bounded sequence in one explicit call, use
`agent.planAndRun()`. It routes and builds the blueprint, requests a provider proposal, pauses with
`plan_review_required` until `acceptPlan: true`, then invokes the accepted plan. Planning approval
and execution approval remain separate, and one caller-owned `AutonomousCostBudget` can be passed
to both phases so the planner cannot consume budget that the executor does not see:

```typescript
const budget = new AutonomousCostBudget(4);
const outcome = await agent.planAndRun(task, {
  planning: { approveProviderCall: true, costBudget: budget },
  costBudget: budget,
  acceptPlan: true,
  approveProviderCall: true,
});
```

The same method applies to cross-domain routes: the proposal reorders only existing child ids,
then the accepted digest is carried through every specialist and synthesis invocation. Omitting
`acceptPlan` never dispatches the execution provider. Callers that already hold a reviewed proposal
can instead pass `acceptedSingleDomainPlanRefinement` to `run()` or
`acceptedCrossDomainPlanRefinement` to `runCrossDomain()`.

These methods produce proposals, never authorization or execution. The returned records retain
only existing ids, bounded confidence, selected-model metadata, numeric budget accounting, and SHA-256 digests. They do not
retain the original task, prompt transcript, provider response, tool names, credentials, effects,
or new domain authority. A caller that accepts a completed proposal must still apply it through
its own workflow executor and re-check the task/blueprint digests at that boundary. The executor
then binds the accepted proposal digest into every checkpoint and stage context:

Every `AutonomousTaskBlueprint` also carries the `route_digest` that shaped it. A cross-domain
blueprint carries the parent route digest and copies it into every specialist and synthesis
blueprint, so a reviewed route cannot be silently replaced during planning or execution.

```typescript
const executor = new AutonomousWorkflowExecutor(agent, checkpointStore);
const first = await executor.start(task, {
  domain: "coding",
  candidates: agent.models(),
  approveProviderCall: true,
  maxStages: 2,
  acceptedPlanRefinement: proposal,
});

// Persist first.checkpoint. A restart must supply the same accepted proposal object/digest.
const resumed = await executor.resume(first.job_id, task, {
  candidates: agent.models(),
  approveProviderCall: true,
  acceptedPlanRefinement: proposal,
});
console.log(resumed.plan_refinement_digest); // value-only identity, never provider text
```

For cross-domain execution, pass the corresponding `AutonomousCrossDomainPlanRefinementResult`
as `acceptedCrossDomainPlanRefinement`. It may reorder only existing child ids; bounded workers,
child outputs, synthesis context, result ordering, and learning episodes all carry the same
`plan_refinement_digest`. Omitting acceptance preserves declaration order. A missing or changed
proposal is rejected before the next provider dispatch, so replay cannot silently substitute a
different plan. Workflow and cross-domain learning episodes retain this digest as metadata while
the evaluator still remains the only source of reward.

Structured autonomous responses are opt-in and capability-gated. Pass `requireJson: true` to
`AutonomousAgent.run()` or `runCrossDomain()` and optionally pass a JSON `responseSchema`; the
candidate must declare `structured_output`, the provider must not advertise
`structuredOutputMode: "disabled"`, and all normal readiness, capacity, budget, latency, quality,
and approval gates still apply before dispatch. `json_object` providers receive a portable wire hint
and are checked locally against the schema; `json_schema` providers also receive native strict schema
metadata. The parsed value is returned as `response.structured`, and malformed or schema-invalid
provider output is classified as `invalid_response`. Cross-domain children and synthesis inherit the
same contract, preventing a partial structured run from being mistaken for a fully structured one.

The composed execution wrappers preserve these options instead of rebuilding a weaker request:
decision-cycle attempts and replans forward the cost, latency, quality, JSON, and schema policy;
cross-domain cycles apply it to every specialist and synthesis call; and workflow stages apply it
again on every stage invocation. Each new provider selection is independently gated, so retries,
fan-out, and resume cannot silently bypass the caller's policy or downgrade structured output.

`runAutonomousDecisionCycle()` composes the single-domain path into one caller-controlled loop:
optional semantic routing, task-digest-validated route handoff, prompt and plan construction,
health/bandit model selection, provider invocation, and optional evaluator settlement. Semantic
routing approval and execution approval remain separate. Supplying an
`AutonomousLearningController` creates a pending episode; supplying an evaluator callback settles
it only from explicit bounded reward fields. The provider response, transport success, and model
self-report never become reinforcement automatically. Cross-domain cycles continue through
`runCrossDomain()` and `settleCrossDomain()` so specialist and synthesis episodes retain delayed
credit separately.

Both decision-cycle entry points can insert provider planning directly into this durable loop. Set
`providerPlanning` to request a proposal and set `acceptPlan: true` only after accepting a completed,
dependency-safe proposal. A review pause returns `plan_review_required` with no execution dispatch:

```typescript
const reviewed = await runAutonomousDecisionCycle(agent, task, {
  domain: "coding",
  cycleId: "job-2026-08-21-0001",
  decisionStateStore: stateStore,
  providerPlanning: { approveProviderCall: true },
  approveProviderCall: true,
});

const completed = await runAutonomousDecisionCycle(agent, task, {
  domain: "coding",
  cycleId: "job-2026-08-21-0001",
  decisionStateStore: stateStore,
  rehydrateRoute: () => reviewed.route,
  acceptedSingleDomainPlanRefinement: reviewed.plan_refinement,
  approveProviderCall: true,
});
```

The ordinary cursor adds `planning_pending` and retains only `plan_refinement_digest`. The proposal,
task, planner transcript, and credentials remain caller-owned. A restart with a persisted planning
digest must provide the exact accepted proposal; its task, base-plan, workflow/dependency, and
SHA-256 identity are checked before execution. Cross-domain cycles apply the same rule to existing
child ids and carry the accepted digest through specialist fan-out and synthesis. Planning and
execution can share one `AutonomousCostBudget`, keeping the planner inside the caller's aggregate
ceiling.

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

For a worker that must survive process loss, add a stable `cycleId` and a caller-owned
`AutonomousCycleReplanStateStore`:

```typescript
const cycleStore = new InMemoryAutonomousCycleReplanStateStore();
const first = await runAutonomousReplanCycle(agent, task, {
  cycleId: "review-2026-08-20-001",
  stateStore: cycleStore,
  domain: "coding",
  approveProviderCall: true,
  evaluate: (run) => evaluateWithCallerOwnedEvidence(run),
});
```

The state journal advances through `execution_pending`, `evaluation_pending`,
`settlement_pending`, `replan_handoff`, and `terminal`. It stores only route/outcome/evaluator/
learning digests, bounded IDs, statuses, and a generation-linked state digest. It refuses changed
task or mode contracts, stale generations, unsupported fields, credential-shaped metadata, raw
payload keys, oversized snapshots, and digest tampering. The same options work on
`runAutonomousCrossDomainReplanCycle()`; its state also records the exact specialist/synthesis
episode and trajectory identities.

When `providerPlanning` is enabled on a replan cycle, a plan-review pause is represented as an
`execution_pending` attempt with a `plan_refinement_digest`; this keeps the outer ledger resumable
without claiming that execution occurred. The next call must supply the matching accepted proposal
before the attempt can dispatch.

After a restart, private material is supplied only through explicit rehydrators. `rehydrateRun`
restores a completed provider outcome when evaluation or settlement was interrupted;
`rehydrateRoute` restores the previously reviewed route by digest; `rehydrateEvaluation` restores
an evaluator packet after the `settlement_pending` boundary; and
`rehydrateReplanInstruction` restores transient guidance after `replan_handoff`. The SDK never
serializes the task, prompt, response, tool arguments, evaluator instruction, credentials, or
raw learning payload. A terminal state is idempotent: a duplicate worker returns the durable
projection without invoking a provider again. The journal is an orchestration cursor, not an
exactly-once provider or database transaction; production rehydrators and learning/effect stores
must use stable idempotency keys and reconcile side effects at their own boundary.

Use `AutonomousCycleReplanPersistenceCoordinator` with a caller `read()`/`write()` adapter to
flush and restore bounded, hash-bound snapshots. The in-memory store is intended for tests and
small workers; production deployments should place it beside the existing execution, learning,
memory, effect, and result stores while keeping private payloads in separately access-controlled
storage.

### Restart-safe ordinary decision cycles

The same process-loss boundary is available when a caller wants one ordinary decision cycle rather
than the evaluator-driven replan loop. Pass a stable `cycleId` and an
`AutonomousDecisionCycleStateStore` to `runAutonomousDecisionCycle()` or
`runAutonomousCrossDomainDecisionCycle()`:

```typescript
const stateStore = new InMemoryAutonomousDecisionCycleStateStore();
const snapshotStore = new AutonomousDecisionCyclePersistenceCoordinator(stateStore, {
  read: () => applicationStore.read("decision-cycle-snapshot"),
  write: (snapshot) => applicationStore.write("decision-cycle-snapshot", snapshot),
});

const result = await runAutonomousDecisionCycle(agent, task, {
  cycleId: "job-2026-08-21-0001",
  decisionStateStore: stateStore,
  domain: "coding",
  approveProviderCall: true,
});
await snapshotStore.flush();
```

The state machine is deliberately smaller than a result store. It advances through
`route_pending`, `planning_pending`, `execution_pending`, `evaluation_pending`, `settlement_pending`, and `terminal`,
retaining only task, route, plan, selection, provider-outcome, evaluator, episode, trajectory, and
settlement digests plus bounded statuses and a hash-chain generation. Task text, prompts, plans,
provider responses, tool arguments, credentials, evaluator evidence, and final result objects remain
caller-owned. Snapshot validation is atomic, digest-bound, capacity-limited, duplicate-ID aware,
and rejects private/payload-shaped metadata before restore.

A restart never guesses whether a provider call is safe to repeat. For a persisted route, supply
`rehydrateRoute`; for `execution_pending`, `evaluation_pending`, or `settlement_pending`, supply
`rehydrateRun` with the caller-owned completed run. If evaluation had already started, supply
`rehydrateEvaluation`; if the state is terminal, supply `rehydrateResult`. Each callback is checked
against the persisted route/outcome/evaluation digest and cycle schema. Missing or mismatched
rehydration fails closed before another provider dispatch. Evaluated settlements use a stable
`decision:<cycleId>:<episodeId>` idempotency key, and cross-domain learning uses the stable reviewed
trajectory identity, so a worker can recover around a learner or database interruption without
double-crediting the bandit.

The ordinary persistence contract is domain-neutral: the same state machine and privacy rules cover
coding, browser, data, science, biomedical, neuroscience, operations, enterprise, multi-agent,
multimodal, evaluation, and explicit cross-domain fan-out. A terminal replay returns the private
caller result without invoking the provider again. The journal is still an orchestration cursor,
not a transaction across the provider, evaluator, learning ledger, memory store, or external tools;
those boundaries must keep their own idempotency and reconciliation records.

Provider-assisted semantic routing is part of this restart boundary. If a worker stops before a
route result exists, restored `route_pending` state refuses to implicitly replay the classifier:
supply `rehydrateRoute` to reuse a caller-owned reviewed route, or explicitly set
`retrySemanticRoutingOnRestart: true` to authorize one new route attempt under the original
approval and selection gates. Once a route result exists, it is committed as a route receipt and
must be rehydrated by its task/route digests before execution continues. The route task digest uses
the canonical `{ task }` envelope; older raw-task digests are accepted only as a bounded migration
path. Deterministic routing and an explicit `routeOverride` remain local and do not require a
provider retry.

When a controller is supplied, thrown semantic/provider dispatch failures, replan-transition
failures, and controller-completion failures fail the shared execution before being rethrown unless
the caller selects `executionLifecycle: "observe_only"` for an enclosing manager. Absent HTTP status
codes remain typed `null` metadata instead of causing a secondary journal validation error.

Tool-loop outcomes are intentionally not collapsed into provider success. A loop that receives a
final assistant response returns `status: "completed"`; a caller authorization callback that
declines any requested tool returns `status: "approval_required"` with
`tool_loop.status: "authorization_required"`; and a loop that reaches its bounded turn/tool-call
budget returns `status: "turn_limit_reached"`. An uncertain external effect returns
`status: "reconciliation_required"` with `tool_loop.status: "reconciliation_required"` until
the caller-owned effect resolver confirms the outcome. The public `AutonomousToolLoopStatus` and
`AutonomousToolLoopSummary` types make this lifecycle explicit to workflow and evaluator code.

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

### Restart-safe external effects

An execution journal records that a tool was intended, but that alone cannot make a network write
safe across a process crash. For effectful domain tools, attach an `AutonomousEffectBoundary` to
the agent (or pass one through `AutonomousRunOptions.effectBoundary`) and persist its
`InMemoryAutonomousEffectJournal` snapshot through `AutonomousEffectPersistenceCoordinator`:

```typescript
const effectJournal = new InMemoryAutonomousEffectJournal();
const effectBoundary = new AutonomousEffectBoundary({
  journal: effectJournal,
  resolver: {
    async resolve(record) {
      // Query the caller-owned system of record by effect_id or idempotency key.
      return await applicationEffectStore.resolve(record);
    },
  },
});
const agent = new AutonomousAgent(runtime, {
  toolCatalogue,
  toolExecutor: async (tool, arguments_, effect) =>
    applicationEffectStore.execute(tool.name, arguments_, effect?.idempotency_key),
  effectBoundary,
});
```

The boundary persists `prepared → dispatching → dispatched` before entering the caller's effect
executor and records only digests, bounded identifiers, status, and a deterministic idempotency
key digest. A successful call becomes `completed`; an exception after the dispatch marker becomes
`uncertain`, and the next attempt returns `reconciliation_required` until the resolver confirms
`completed`, `failed`, or an explicitly safe `not_found` retry. The resolver receives only the
metadata record. Raw arguments, output, provider response, credentials, and thrown error bodies
remain application-owned. This is not a promise of exactly-once execution: the external system
must enforce the supplied idempotency key, and callers must reconcile uncertain outcomes before
retrying. Read-only tools bypass the effect ledger while retaining ordinary authorization.
For a caller-supplied `authorizeAndExecute` callback, use the boundary's
`authorizeAndExecute()` adapter or call `effectBoundary.execute()` inside the callback after the
caller has approved the specific call; the SDK does not guess whether an arbitrary callback has
already crossed an external side-effect boundary.

Effect snapshots are hash-chained and reject tampered rows, unsupported fields, secret-shaped
metadata, oversized payloads, and altered head or snapshot digests. `AutonomousExecutionController`
mirrors each transition as `effect_reconciliation` metadata; an uncertain effect places the
execution in `reconciliation_required`, which can be resolved without replaying the operation.
Tool-loop results preserve that status rather than misreporting an uncertain write as a normal
approval pause or successful model turn.

`AutonomousRuntime.invoke()` performs bounded provider failover when a selected provider returns a
retryable `ProviderRuntimeError`: transport, circuit, and provider HTTP failures remove that
provider from the next ranking, while an isolated timeout removes only the timed-out model so a
healthy sibling model on the same provider can be selected. Every retry is re-ranked against the
remaining eligible arms and marked as a failover admission. The default
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
selection/outcome digests, reviewed domain/capability/risk/workflow labels, the verified
`context_digest` for that learning identity, caller-authored tags/lessons, and explicit evaluator
metadata. `retrieve()` can match the exact context digest or task family and is deterministic;
it can be attached to either decision cycle;
recalled rows become low-priority context with an explicit “prior metadata, not verified truth”
warning. `AutonomousMemoryPersistenceCoordinator` connects the store to a caller-owned database or
object-store adapter. Snapshots and event chains are content-addressed, and raw prompts, provider
responses, tool payloads, credentials, and secret-shaped fields are rejected.

The same loop is now available on ordinary `AutonomousAgent.run()` and `runCrossDomain()` calls:
pass `memoryStore` on the agent or per run, and the façade derives bounded task-facet digests,
retrieves relevant episodes before prompt assembly, and records one parent episode after the run.
Cross-domain children and synthesis inherit the retrieved metadata without creating duplicate
episodes. `memoryRunId` gives a caller a stable idempotency identity across restarts;
`recordMemory: false` and `retrieveMemory: false` provide explicit composition controls. The
returned `memory` projection contains only episode IDs, digests, event identity, and a redacted
error class. Memory failures never turn an otherwise valid provider result into a fabricated
success or silently widen authorization. Prior episodes remain advisory context: they cannot add
tools, providers, permissions, effects, budgets, or factual authority.

Direct runs can also hand their completed, selected-model identity to the online-learning
controller in the same call. Supply `learning: controller` and a stable `learningEpisodeId`; the
run returns `learning_episode_status: "prepared"` and a pending `learning_episode_id` only after
provider completion. The controller then requires an independent evaluator packet through
`settleRun()` before updating the bandit. Provider transport success, approval, or a model's own
claim never becomes reward. Incomplete runs return `not_eligible`, and adapter preparation errors
return `failed` with a redacted error class while preserving the valid provider result. Cross-domain
runs continue to prepare specialist and synthesis episodes as one delayed-credit trajectory.

When the agent has an episodic `memoryStore`, the learning controller automatically uses that same
caller-owned store unless its constructor receives an explicit `memoryStore`. A completed direct
run links its pending learning episode to the recorded memory episode; `settleRun()` then writes
the exact evaluator values (never provider transport status) back to memory. The settlement
projection exposes `memory_evaluation.status` as `recorded`, `not_linked`, `not_configured`, or
`failed`. A memory write failure is observable and non-fatal to valid bandit credit, so a provider
result is never replayed merely because a secondary memory sink is unavailable. Stable
`memoryRunId` plus `learningEpisodeId` identities make this join restart-safe; nested callers can
pass a per-settlement `memoryStore` when the run used an override store.

Memory retrieval supports `ranking: "relevance" | "quality" | "planning"`, an optional
`min_reward`, and `require_plan_refinement`. Direct runs default to the advisory `planning`
ranking, which prefers evaluated episodes containing an accepted plan-refinement digest while
still preserving the warning that recalled metadata is a hypothesis aid, not authority. The
`memoryRecall` run option can select another ranking policy. These controls only change bounded
context ordering and filtering; they cannot add tools, providers, permissions, budgets, effects,
or factual evidence.

For bounded automatic recall, `taskFacetDigests(task)` projects short identifier-like task terms into
at most 32 namespaced SHA-256 digests. The original task vocabulary is never stored. Callers may put
those digests in `AutonomousMemoryEpisodeInput.task_facets` and query them through
`AutonomousMemoryQuery.task_facets`; matching is a weak lexical relevance signal, not semantic truth,
authorization, or a replacement for evaluator feedback. Single- and cross-domain decision cycles
derive these facets automatically when no exact task digest or explicit facet query is supplied;
caller-provided exact filters remain authoritative.

`InMemoryAutonomousGoalLedger` supplies the objective layer above episodic recall. It carries a
digest-only task identity, bounded criterion/evidence digests, attempt budget, blockers, next-action
digest, optimistic revisions, and a hash-chained lifecycle across every built-in domain. Required
criteria must be satisfied or explicitly waived before completion; snapshots can be flushed through
`AutonomousGoalPersistenceCoordinator`, and raw goal text, prompts, responses, tools, and credentials
are never retained.
`AutonomousAgent.runGoalStep(...)` connects one bounded objective attempt to the routed planning and
provider runtime, turns approval/partial/failure/completion outcomes into resumable goal state, and
keeps the runtime result transient while persisting only a value-only outcome digest.
`runCrossDomainGoalStep(...)` applies the same lifecycle to specialist fan-out and synthesis and
retains only outcome, evaluator, learning-state, and progress digests.
`runGoalLearningStep(...)` and `runCrossDomainGoalLearningStep(...)` bind the same goal ledger to
the evaluator-guided replan cycles and `AutonomousLearningController`; the goal snapshot stores
only evaluator, learning-settlement, and progress digests. Supply a stable `cycleId` plus a
caller-owned cycle `stateStore` for restart rehydration. Provider responses, task text, evaluator
instructions, credentials, and evidence remain transient, and provider approval is still
required.

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

When an application supplies both `apiClient` and `toolCatalogue` but omits `toolExecutor`,
`AutonomousAgent` automatically installs `createAutonomousApiToolExecutor()`. The bridge calls
the already-reviewed catalogue through `ApiClient.toolChecked()` and preserves the API/MCP
refusal envelope as a typed runtime failure; it performs no tool discovery and never accepts or
stores key material. The caller's `ApiClient` remains responsible for its own user credential or
session. Supply `toolExecutor` explicitly when a local adapter, queue, sandbox, or Python/Rust
boundary should own dispatch instead.

The selector can be supplied directly, backed by `AutonomousOnlineLearner`, or bridged to
`ApiClient.brainModelSelectContextual()`. The bridge sends only model descriptors, health,
domain/capability/risk context, and bounded digests—not credentials, prompt transcripts, tool
arguments, or provider responses. `recordEvaluatorReward()` updates a local UCB learner only from
explicit evaluator feedback and can optionally submit the same value-only update to the control
plane. This is adaptation infrastructure, not an automatic truth signal: feedback must be produced
by a caller-owned evaluator, and provider health is kept separate from task quality.

Remote settlement adopts the control plane's returned bandit projection, including normalized
generations, replay receipts, contextual rows, and first-run arm hydration. A selected model may be
credited even when the caller's persisted state began with no arm for it; direct low-level updates
remain strict about unknown arms.
Learner-backed provider results retain the exploration draw and ranking evidence used for the actual
selection, and malformed restored generations, duplicate arms, or conflicting remote policy fields
are rejected before they can influence a later run.
Live model refreshes are provider-scoped atomic reconciliations: `replaceExisting` removes stale
discovered arms as well as registering new models and replacing changed metadata, with the removed
IDs returned as value-only receipt metadata. `agent.refreshModelCatalogue()` composes bounded
discovery for multiple registered providers, uses a caller-owned `credentialFor(provider)` resolver,
supports bounded parallelism, and returns `completed`/`partial`/`failed` redacted status without
erasing a healthy provider's catalogue when another provider is unavailable. The resulting shared
catalogue feeds every built-in domain, workflow, cross-domain, mission, and goal invocation through
the same capability and health gates.

The catalogue can also cross a process restart without serializing a key or provider response. Call
`agent.snapshotModels()` (or `AutonomousModelCataloguePersistenceCoordinator.flush()`) to write a
SHA-256-bound projection containing only validated model ids, capabilities, capacities, caller-owned
quality/cost/latency priors, and credential-required/enabled flags. On startup, call `restore()` before
readiness or execution. Restoration validates the snapshot, catalogue digest, snapshot digest, model
bounds, duplicate ids, and allowed metadata keys before replacing the live map atomically; a rejected
or tampered snapshot cannot partially change selection inputs. Use a caller-owned JSON/SQLite/IndexedDB/
Postgres adapter and resolve credentials again after restart—the snapshot's `secret_material` marker is
an assertion, not a key store.

Provider transport health has its own ledger. `LLMRuntimeHealthPersistenceCoordinator` persists
only bounded provider/model counters, last status metadata, and circuit-open deadlines. Restore it
after registering the same provider transports; unknown providers are refused and the current
runtime is replaced only after the complete snapshot verifies. This state is intentionally not
task quality or evaluator reward: the model-health ledger remains the separate source for quality
feedback, and credentials are collected into a fresh process-local session after restart.

Learning episodes can only be prepared from a completed autonomous run; approval pauses, provider
refusals, child failures, and tool-loop limits cannot be converted into evaluator or bandit credit.
Trajectory settlement is resumable after a later episode failure: matching already-settled reward
projections are skipped, while changed reward evidence is refused.
Every settlement now also has a caller-owned, value-only receipt boundary. Pass
`settlementReceipts` to `AutonomousLearningController` with a durable implementation of
`AutonomousLearningSettlementReceiptStore` (the in-memory implementation is for tests and small
workers). Single episodes default to `episode:<episode_id>` keys; trajectory steps use deterministic
hashed keys, and callers may supply a stable trajectory-level `idempotencyKey`. Receipts bind the
episode/trajectory digest, normalized evaluator request, and settlement projection. A retry after
restart returns the exact prior projection, repairs a pending episode/trajectory commit when
possible, and does not dispatch a provider or double-credit the local bandit. Contradictory
reward, remote/local mode, identity, or receipt digest evidence is refused. Receipt payloads reject
prompts, responses, credentials, tool arguments, raw results, and other private provider material;
the cross-domain result itself is never persisted in a trajectory receipt and must be rehydrated by
the caller.

For applications that need a worker boundary across those stores, `AutonomousLearningController`
also exposes `enqueueRunSettlement()`, `enqueueTrajectorySettlement()`, and `dispatchFeedback()`.
The caller supplies `feedbackOutbox` implementing `AutonomousLearningFeedbackOutboxStore`; the
in-memory implementation is a bounded test/single-worker model, while a production adapter should
implement `claim()` and the status transitions with a conditional database update or equivalent
lease. Commands contain only normalized evaluator values, target/request digests, and a remote-mode
flag. Dispatch leases a command, applies the existing receipt-idempotent settlement, and marks the
command applied by result digest. A worker crash after learner credit but before acknowledgement is
safe to retry; transient failures are retried with bounded exponential backoff, while malformed or
conflicting commands become terminal failures. Lease ownership prevents two workers from applying
the same command concurrently, and no outbox command retains task text, prompts, responses,
credentials, tool arguments, or raw evidence.

Decision cycles, replan cycles, workflow cycles, cross-domain fan-out, mission learning, and goal
learning can opt into the same boundary with `outbox: { workerId, leaseMs }` on their learning
options. They preserve their normal settlement return values: the surface enqueues and dispatches
the command, then rehydrates the receipt. This keeps direct runs, delayed-credit trajectories,
workflow stages, cross-domain work, and mission learning on one retry/idempotency contract instead
of giving each orchestration layer a separate settlement implementation.

Workflow evaluation also verifies evidence identity instead of trusting a caller-provided digest.
Stage identifiers and signal keys are normalized into an order-independent evidence packet, hashed
with SHA-256, and compared with any supplied `evidence_digest`; a mismatch refuses evaluation before
learning credit can be produced. The packet includes the blueprint's contextual learning identity,
but never includes task text, prompts, responses, credentials, or raw provider evidence.

For higher-assurance deployments, `AutonomousEvaluatorMesh` runs two through eight independent
evaluator members and returns only value-level member projections. Learning credit is accepted only
when members agree on pass/fail and failure class and their rewards stay within the configured
spread bound. A disagreement, invalid member result, or member exception returns a refusal status
and cannot become bandit credit; evaluator errors are represented only by bounded error classes and
digests. The same mesh can evaluate any of the twelve built-in domain workflows because it consumes
the shared `AutonomousRunResult` contract rather than domain-specific private payloads.

Ambiguous tasks now have a real fan-out/fan-in path. `blueprint()` returns a
`cross_domain_blueprint` containing one child workflow per selected domain plus a cross-domain
synthesis workflow. `runCrossDomain()` executes those children under the same provider approval,
model selection, tool catalogue, and effect approval boundaries, then gives synthesis
bounded local child outputs with child status and digests. Fan-out uses deterministic serial
dispatch by default and a bounded worker pool when `maxParallelChildren` is explicitly set from
1 through 4, while preserving child
declaration order in results and learning episode IDs. A failed or approval-blocked child stops
synthesis by default; already in-flight children may finish, but no new child is scheduled after a
bounded failure. `allowPartial: true` makes partial synthesis an explicit caller choice, and
`synthesize: false` returns the specialist results without pretending that integration occurred.
Calling `run()` without an explicit domain automatically uses this path when routing selects more
than one domain.

Provider tool-loop exhaustion remains `turn_limit_reached` when it occurs in a specialist or the
fan-in synthesis stage. It is not rewritten as a successful partial result or as an opaque child
failure, so a caller can distinguish a bounded retry/escalation decision from an authorization
pause or an unexpected child exception.

For long-running fan-out, `AutonomousCrossDomainExecutor` provides the restart-safe counterpart
to `runCrossDomain()`. It dispatches at most `maxSteps` calls per invocation (one child or the
final synthesis step), persists a metadata-only checkpoint after every completed child, and
emits a bounded predecessor-linked event chain. The checkpoint binds the task digest, route
digest, base cross-domain plan digest, accepted-plan digest, ordered child prefix, child result
digests, synthesis digest, execution-contract digest, and generation. It never stores task text,
prompts, credentials, provider responses, tool payloads, or raw error messages.

```typescript
const store = new InMemoryAutonomousCrossDomainCheckpointStore();
const executor = new AutonomousCrossDomainExecutor(agent, store, { learning });
const first = await executor.start(task, {
  jobId: "research-2026-01",
  subtasks,
  candidates: agent.models(),
  acceptedCrossDomainPlanRefinement: acceptedPlan,
  approveProviderCall: true,
  maxSteps: 1,
});

// Keep this in caller-owned storage. The checkpoint contains only its digest.
const childCache = new Map(first.step_results.map((step) => [step.item_id, step.run]));
const next = await executor.resume("research-2026-01", task, {
  subtasks,
  candidates: agent.models(),
  acceptedCrossDomainPlanRefinement: acceptedPlan,
  approveProviderCall: true,
  maxSteps: 1,
  resolveChildResult: (childId) => childCache.get(childId) ?? null,
});
```

Durable fan-out can also begin with provider-assisted semantic routing by setting
`semanticRouting.enabled`. Routing approval is separate from child/synthesis approval, and the
result exposes `semantic_route_status` plus the reviewed route. The route is passed into blueprint
construction through a digest-checked `routeOverride`, so provider-selected child domains cannot
silently fall back to a different deterministic fan-out. New checkpoints bind the exact route
digest. On restart, a semantic route must be rehydrated through the caller-owned `routeOverride`;
the executor refuses to replay the classifier implicitly because the route digest alone cannot
reconstruct its selected-domain evidence. Provider disagreement, abstention, invalid output, and
approval refusal all stop before child dispatch. This preserves the same route-bound behavior for
fan-outs that combine any of the reviewed domain profiles.

Before continuation, the executor rehydrates exactly the checkpointed ordered prefix and checks
each result digest, child task digest, and optional output digest. A mismatch fails before the
next provider dispatch. Approval pauses preserve the same `next_child_id`; they do not advance
the checkpoint. Once all children are complete, `synthesize: false` leaves a `synthesis_pending`
checkpoint, while a later approved call can perform only the synthesis step. Provider failures
return redacted typed error metadata (`error_class`, `error_code`, `retryable`, and `status_code`)
without retaining the provider message. `InMemoryAutonomousCrossDomainCheckpointStore` supports
integrity-checked snapshots and a caller-owned persistence adapter through
`AutonomousCrossDomainPersistenceCoordinator`; production workers can implement the same store
interface over their durable database or queue.

An uncertain child or synthesis dispatch is different from an ordinary pause: it creates a
`reconciliation_required` checkpoint and a metadata-only `reconciliation_required` event. A
normal `resume()` returns that quarantine without rehydrating completed child payloads or calling
the provider again. After the caller inspects its effect ledger or provider system of record and
decides that replay is safe, it must pass `retryReconciliation: true`; the executor then creates
one or more new hash-linked checkpoint generations, records `reconciliation_retry_authorized`,
and re-enters the ordinary approval and execution gates. The SDK does not infer that an uncertain
operation was lost, and this flag must not be used as a substitute for an idempotency key or an
external reconciliation record.

For resumable single-domain workflows, `AutonomousWorkflowExecutor` turns the reviewed workflow
DAG into bounded stage calls. For ambiguous intake, set `semanticRouting.enabled` and approve
that classifier separately from `approveProviderCall`; the resulting route is exposed on the
execution result and its `route_digest` is stored in every new checkpoint. The executor accepts
an already-reviewed `routeOverride` as a digest-checked handoff, and a resumed job uses its
persisted domain/workflow identity without replaying the semantic classifier. Semantic routing is
still a routing hypothesis: disagreement, abstention, malformed output, or cross-domain selection
returns `route_review_required` before a stage is dispatched. This same path is available for all
reviewed single-domain profiles, including coding, browser, data, science, biomedical,
neuroscience, operations, enterprise, multi-agent, multimodal, and evaluation.

```typescript
const first = await executor.start(task, {
  candidates: agent.models(),
  semanticRouting: { enabled: true, approveProviderCall: true, allowCrossDomain: false },
  approveProviderCall: true,
  maxStages: 2,
});
// Persist only first.checkpoint; it contains route/workflow digests, never task or provider text.
const resumed = await executor.resume(first.job_id, task, {
  candidates: agent.models(),
  approveProviderCall: true,
});
```

The executor checkpoints after each completed stage, pauses after
`maxStages`, records a metadata-only event chain, and resumes only when the caller supplies the
original task and the rebuilt workflow/plan digests match. `InMemoryAutonomousWorkflowCheckpointStore`
is suitable for tests and small workers; production applications should implement
`AutonomousWorkflowCheckpointStore` over their durable store. Checkpoints never contain task text,
prompts, provider responses, credentials, or tool payloads, and restart recovery explicitly
projects thrown provider failures as redacted `error_code`, `retryable`, `status_code`, and bounded
`error_class` metadata so workers can choose whether to retry or escalate without retaining the
provider message or response body. A failed stage remains terminal until the caller explicitly
rehydrates and chooses a new execution policy.
Recovery always requires caller-owned task and credential rehydration.

Workflow stage options use the same hard selection and output contract as direct agent runs:
`maxCostPerMillionTokens`, `maxLatencyMs`, and `minQuality` are enforced before each provider
dispatch. Unlike a free-form direct run, the workflow owns the stage output contract and always
requires structured JSON. Every stage must return exactly `stage_id`, `status`, `evidence`,
`uncertainty`, `notes`, and `next_actions`; `stage_id` is bound to the current stage and only
`status: "completed"` can advance the DAG. `proposed`, `blocked`, `not_attempted`, missing, or
unsupported fields fail closed. Declared `blocked`, `proposed`, and `not_attempted` stages return
typed `stage_blocked`, `stage_proposed`, and `stage_not_attempted` execution statuses; malformed
output returns `failed` with a `stage_output_invalid` checkpoint outcome. Caller
`requireJson`/`responseSchema` options cannot weaken this contract.
A checkpoint or previous stage admission does not authorize a later stage; readiness, capacity,
approval, and output validation are repeated at the stage boundary.

The structured stage result is also exposed locally through `stage_results`: bounded evidence,
uncertainty, notes, next actions, the declared status, and validation errors are retained for the
caller’s evaluator/learning boundary, while the durable checkpoint retains only digests and typed
failure metadata. The built-in stage schema caps evidence and action arrays at 32 entries, each
entry at 4,096 bytes, and notes at 16,000 bytes. This makes workflow completion an evidence-bearing
state transition rather than an inference from provider transport success.

Blocked-stage recovery is an explicit caller decision. Resuming a checkpoint with a blocked,
proposed, or not-attempted terminal stage returns the same typed status without dispatching another
provider call. Pass `retryBlocked: true` only after revising the caller-owned evidence, prompt
context, model policy, or approval decision. The executor creates a new checkpoint generation,
retains the prior failure in the hash-linked event history, and then retries the stage under the
same provider, tool, credential, and effect gates.

For a paused job resumed in a new process, pass exact caller-owned prior JSON responses through
`stageOutputs: { [stageId]: rawJson }` when downstream stages need their dependency evidence. Each
entry must belong to a completed checkpoint stage, match its stored response digest, and pass the
same stage schema before it is placed into the next prompt. Missing entries remain metadata-only;
the executor never invents evidence, and the raw response is never written to the checkpoint or
learning state.

New checkpoints persist only an `execution_contract_digest` for the effective candidates, selection
limits, structured-output/schema requirement, tool definitions, failover limit, and enclosing
execution policy. `resume()` refuses a changed digest before provider dispatch. Older checkpoints
remain readable but are intentionally unbound; pass `rebindLegacyExecutionContract: true` to perform
an explicit one-time migration that creates a new linked checkpoint.

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

For applications that want one high-level supervisor, `runAutonomousWorkflowCycle()` composes
the durable executor, explicit evaluator, delayed-credit settlement, and bounded replanning
boundary. The callback must return caller-owned evidence for the stages that ran; it may also
request a retry with a short evaluator instruction. A retry gets a fresh child workflow job ID
(`root`, then `root:attempt-2`, and so on), so the prior checkpoint and workflow contract remain
immutable. The instruction is screened for credential-shaped material, placed only in transient
prompt context, and included in the returned digest-only evaluation projection. The supervisor
never treats a provider response, transport success, or model self-report as evidence or reward.

```typescript
const cycle = await runAutonomousWorkflowCycle(task, executor, {
  domain: "coding",
  candidates: agent.models(),
  approveProviderCall: true,
  jobId: "delivery-cycle-42",
  maxReplans: 1,
  learning: { controller: learning, trajectoryIdPrefix: "delivery-trajectory" },
  evaluate: async (execution) => ({
    evidence: {
      stages: execution.stage_results.map((stage) => ({
        stage_id: stage.stage.id,
        signals: Object.fromEntries(stage.stage.evaluator_signals.map((signal) => [signal, 1])),
      })),
    },
    replan_requested: false,
  }),
});
```

The cycle is domain-neutral and exercises the same reviewed stage contract for coding, browser,
data, science, biomedical, neuroscience, operations, enterprise, multi-agent, multimodal,
cross-domain, and evaluation workflows. Each attempt remains independently checkpointed by the
executor store; the cycle result retains local provider results while exposing only bounded
evaluation and learning projections for persistence. Approval, blocked-stage recovery, effect
reconciliation, credential rehydration, and provider selection remain caller-controlled gates.

For a caller that wants the supervisor to make the retry decision from the evaluator result,
set `automaticReplan: true`. When the evaluator does not pass a completed attempt and the caller
did not already request a retry, the cycle derives a bounded instruction from missing, rejected,
or below-threshold signal names, then applies the normal `maxReplans` ceiling. It never copies
signal values, provider output, task text, credentials, or evaluator evidence into the instruction,
and it cannot add tools, stages, permissions, effects, or claims. This makes the retry decision
autonomous while leaving quality authority with the evaluator. An explicit `evaluator` override
selects the cycle evaluator when no learning controller is present; with learning enabled it must
be the controller's exact evaluator instance so settlement and decision use the same contract.

Use `autonomousWorkflowEvaluatorForDomain(domain)` to obtain the exact content-addressed
evaluator profile for any built-in domain without duplicating signal names or weights:

```typescript
const evaluator = await autonomousWorkflowEvaluatorForDomain("neuroscience");
const cycle = await runAutonomousWorkflowCycle(task, executor, {
  domain: "neuroscience",
  evaluator,
  automaticReplan: true,
  maxReplans: 2,
  approveProviderCall: true,
  evaluate: (execution) => callerHeldOutEvidence(execution),
});
```

Automatic replanning is still bounded and reviewable: a failed evaluator gate with
`maxReplans: 0` returns `completed_without_replan` with a digest-only request, while a positive
budget creates a fresh child checkpoint for each retry. This prevents a model's self-reported
completion from silently becoming an unbounded autonomous loop.

For process-restart recovery, pass `cycleId` and an `AutonomousWorkflowCycleStateStore`. The
supervisor records hash-linked `execution_pending`, `evaluation_pending`,
`settlement_pending`, `replan_handoff`, and `terminal` phases without storing
task text, prompts, provider output, evaluator evidence, or transient instructions. A worker
must supply `rehydrateExecution` after provider work has completed, `rehydrateEvaluation` after a
settlement interruption, and `rehydrateReplanInstruction` after evaluator handoff. Every callback
is checked against the persisted outcome/evidence/instruction digests before the supervisor can
continue. `InMemoryAutonomousWorkflowCycleStateStore` and
`AutonomousWorkflowCyclePersistenceCoordinator` provide the bounded reference snapshot bridge
for a caller-owned durable adapter.

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
- `AutonomousMissionExecutor` is the local application-owned execution path when a TypeScript
  embedding needs to run the same reviewed mission contract without delegating lifecycle state to
  the remote queue. It reuses `missionPreflight()`, binds the live catalogue digest into every
  checkpoint, validates all twelve built-in domains, executes dependency waves with serial or
  bounded parallel scheduling, resolves RFC 6901 bindings from caller-owned result storage, and
  emits a hash-chained metadata-only trace. The checkpoint stores step status, output byte counts,
  result digests, retry state, the next wave, and a digest-only route/plan/prompt/model-selection
  decision receipt; it never stores task arguments, raw outputs, prompts, credentials, or provider
  bodies.
- A mission can additionally bind its top-level goal to `semanticRouting`. The classifier is a
  separate approval-gated route-review call; its selected domains must exactly cover the explicit
  mission step domains before any step dispatch. The resulting `route_digest` is persisted in every
  checkpoint, and a restart requires the caller to rehydrate the exact `routeOverride` rather than
  replaying the classifier implicitly. Provider abstention, disagreement, malformed output, or
  missing route approval returns `route_review_required` with no new checkpoint. Supplying
  `maxTotalCostUnits` creates one caller-owned `AutonomousCostBudget` shared by semantic
  classification and provider-backed step adapters through `cost_budget`.
- `InMemoryAutonomousMissionCheckpointStore` is a deterministic reference store; production
  callers should implement `AutonomousMissionCheckpointStore` over their own transactional storage
  and pair it with `AutonomousMissionPersistenceCoordinator` for snapshot flush/restore. Raw step
  values belong in an `AutonomousMissionResultStore`, which must be rehydrated before a binding
  step resumes. Missing or digest-mismatched values produce `recovery_required`, not an invented
  input. `approval_required` and `reconciliation_required` likewise retain the current wave so a
  later call can continue without silently replaying or finalizing the unresolved step.
- `agentMissionStepExecutor(agent, ...)` composes the local executor with the full autonomous
  brain: routing/blueprinting and model selection remain in `AutonomousAgent.run()`, while the
  adapter requires the provider to invoke exactly the declared mission tool with exactly the
  resolved argument digest. Tool admission, caller approval, execution budgets, and the durable
  `AutonomousEffectBoundary` are reused at the final dispatch boundary. For deterministic local
  adapters, supply `executeStep` directly and keep model invocation outside the mission callback.
- `runAutonomousMissionReplanCycle()` adds bounded evaluator-guided retries around the durable
  executor. Each attempt receives a new mission ID, while a protected contract digest refuses
  changes to tools, arguments, domains, dependencies, policy, claims, route review, credentials,
  or effect authority. A replan may refine objectives or reorder independent steps; the default
  replanner injects screened evaluator guidance transiently, and applications can supply their
  own proposal callback. `checkpointSink` receives only attempt status, request/evaluation
  digests, and learning trajectory IDs, so restart orchestration can persist metadata without
  retaining the guidance or mission payload.
- Replanning now carries the approved mission `route_digest` across attempts instead of invoking
  semantic classification again. If the orchestration state is restored, the caller must provide
  the original `execute.routeOverride`; a stored digest alone cannot authorize a reconstructed
  route. A `maxTotalCostUnits` or `costBudget` supplied to the cycle becomes one shared budget for
  routing and every attempt, and the state/checkpoint projections retain only its numeric
  max/consumed/remaining snapshot. A changed route or budget fails before provider dispatch.

```typescript
const missionExecutor = new AutonomousMissionExecutor({
  agent, // required only when semanticRouting is enabled
  catalogue: liveCatalogue,
  checkpointStore: durableMissionStore,
  resultStore: callerOwnedResultStore,
  executeStep: agentMissionStepExecutor(agent, {
    toolsForStep: (step) => toolsFor(step.tool),
    approveEffects: false,
  }),
});

const first = await missionExecutor.start(mission, {
  approveProviderCall: true,
  semanticRouting: { enabled: true, approveProviderCall: true },
  maxTotalCostUnits: 500,
  max_waves: 2, // bounded continuation; persist first.checkpoint through the store
});
const next = first.next_wave === null
  ? first
  : await missionExecutor.resume(mission, { approveProviderCall: true, routeOverride: first.route });
```

An evaluator-guided mission loop can be layered over the same executor:

```typescript
const replanned = await runAutonomousMissionReplanCycle(missionExecutor, mission, {
  maxReplans: 2,
  evaluate: async (execution) => evaluateMissionWithCallerOwnedEvidence(execution),
  learning: { adapter: learning, trajectoryIdPrefix: "mission-learning" },
  checkpointSink: durableAttemptMetadata.write,
});
```

The evaluator must explicitly return a bounded reward and exact rewards for any learning episode
IDs emitted by successful steps. A failed or partial attempt can be evaluated and replanned, but
approval, recovery, reconciliation, and cancellation states return to the caller without an
automatic retry. Replan instructions are transient and digest-bound; they cannot add tools,
permissions, credentials, effects, or a new domain. The cycle has a hard three-replan ceiling.

For restart-safe orchestration, pass an `InMemoryAutonomousMissionReplanStateStore` in tests or a
caller-owned implementation backed by SQLite, Postgres, IndexedDB, or object storage. It stores
only the root identity, protected-contract digest, attempt/evaluation projections, learning
settlement projections, checkpoint digests, and a generation-linked state digest. Its explicit
phases are `execution_pending`, `evaluation_pending`, `replan_handoff`, and `terminal`. Saves are
idempotent for the same digest and otherwise require a contiguous generation pointing to the
previous state digest, so stale or forked workers fail closed.

The state store never serializes a proposal, mission arguments, provider output, evaluator
instruction, credentials, or raw evidence. After a restart on a non-root attempt,
`rehydrateMission` reconstructs the private mission and the SDK rechecks its identity, protected
contract, catalogue, policy, and preflight authorization. After a restart at `replan_handoff`,
`rehydrateReplanInstruction` restores transient guidance by digest; evaluation and already-settled
learning are not replayed. The executor checkpoint/result stores remain separate: the state table
is the orchestration cursor, while the executor store and caller result cache own wave metadata
and private values. `AutonomousMissionReplanPersistenceCoordinator` flushes/restores bounded,
hash-bound multi-root snapshots through a caller-owned `read()`/`write()` adapter.

To connect provider-backed steps to delayed-credit learning, give the adapter the same
`AutonomousLearningController` used by direct runs. Successful steps receive stable episode IDs;
approval, refusal, failed, and uncertain-effect steps do not create episodes. The mission
checkpoint retains only those IDs, while the controller stores the selected arm and outcome
digests in its caller-owned learning store:

```typescript
const learning = new AutonomousLearningController(agent, {
  episodes: durableEpisodeStore,
  trajectories: durableTrajectoryStore,
});
const missionExecutor = new AutonomousMissionExecutor({
  catalogue: liveCatalogue,
  checkpointStore: durableMissionStore,
  resultStore: callerOwnedResultStore,
  executeStep: agentMissionStepExecutor(agent, { learning: { adapter: learning } }),
});
const execution = await missionExecutor.resume(mission, { approveProviderCall: true });
const rewards = evaluatorRewardsFor(execution); // caller-owned evaluator; exact episode IDs required
const settlement = await settleAutonomousMissionLearning(execution, learning, {
  trajectoryId: "mission-42-trajectory-0",
  rewards,
});
```

`settleAutonomousMissionLearning()` includes only successful episode IDs from the durable
checkpoint, requires an exact evaluator reward for each, and applies the existing bounded
discounted return-to-go logic. It never infers reward from HTTP success, tool completion, model
confidence, or mission status. Replaying the same completed run with the same episode identity is
idempotent; a different run identity is rejected as an episode conflict rather than silently
overwriting learning evidence.

The local executor is an orchestration boundary, not a claim of exactly-once delivery: effectful
steps must still use an idempotency-aware effect adapter, and an uncertain effect must be resolved
before the mission can progress. `onStepOutcome` is an optional caller-owned hook for evaluator
signals or online-learning settlement; the executor never invents rewards from transport success.
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
