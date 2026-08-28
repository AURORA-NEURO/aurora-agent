# Autonomous provider-neutral streaming

The SDKs now expose a first-class live stream boundary between autonomous model selection and
the application that renders or consumes model output. It is intentionally a transport contract,
not a second planning engine: the brain still owns domain routing, model eligibility, prompt
assembly, plan approval, effect authority, evaluator credit, and learning policy.

The TypeScript application façade exposes the same boundary above the raw runtime:

```typescript
const handle = await agent.runAutoStream("debug and verify this change", {
  domain: "coding",
  candidates,
  credentialFor: (provider) => credentials.get(provider),
  approveProviderCall: true,
});

for await (const item of handle.events) {
  if (item.kind === "provider") ui.append(item.event.textDelta);
}
const completion = await handle.completion;
```

`AutonomousAgent.runStream()` performs a provider-free high-level preflight first, so route,
task decision, blueprint, structured-output, memory, cost, policy, and caller approval gates are
still authoritative. `runAutoStream()` is the deterministic automatic route wrapper. Provider-
planned automatic execution remains a separate review/acceptance flow and is rejected by the
stream wrapper until a caller has explicitly accepted the plan. `runCrossDomainStream()` adds
bounded specialist fan-out, transient child lifecycle events, and synthesis over bounded local
specialist text. Its event channel distinguishes `direct`, `child`, and `synthesis` stages.

Cross-domain stream output is never inserted into the completion receipt or memory/learning
stores. The fan-in buffer is process-local and bounded per child; callers should consume the
single-consumer event iterator promptly and close it when abandoning a stream. A caller abort
signal cancels the child/synthesis boundary, while each child keeps the same credential resolver,
execution controller, cost budget, and provider failover policy.

## Contract shape

TypeScript opens the full autonomous path:

```typescript
const handle = await autonomousRuntime.invokeStream(plan, {
  credentialFor: (provider) => credentialSession.handle(provider),
  maxProviderFailovers: 1,
  selectionEventCallback: (event) => trace.append(event),
});

for await (const event of handle.events) {
  ui.append(event.textDelta);
}
const completion = await handle.completion;
```

`invokeStream()` performs these operations before it returns the handle:

1. validate the task, candidates, request, capabilities, cost/latency gates, and credential
   readiness;
2. compact the request under the explicit `contextBudget`, if one was supplied;
3. run the configured value-only selector or deterministic health/utility selector exactly once;
4. compile the immutable eligible continuation ladder; and
5. return the selected model metadata, continuation plan, compacted-budget receipt, transient
   event iterable, and a completion promise.

The stream does not buffer output. `ProviderStreamEvent` is the only live payload surface and is
discarded by the runtime after the caller consumes it. `AutonomousStreamCompletion` contains only
event count, UTF-8 delta byte count, `done` observation, provider invocation receipts, bounded
failover metadata, safe error code/class, and retention markers.

Python's runtime is synchronous and accepts the ranked arm order produced by the caller's already
approved brain selection:

```python
from prism_sdk import AutonomousStreamRuntime

handle = AutonomousStreamRuntime(runtime).open(
    request,
    provider="primary",
    model="reasoning-model",
    fallbacks=(
        {"provider": "backup", "model": "efficient-model"},
    ),
    credential_for=lambda provider: credentials.get(provider),
    max_provider_failovers=1,
    context_budget={
        "max_input_tokens": 12_000,
        "preserve_recent_messages": 10,
        "max_messages": 96,
    },
)

for event in handle.events:
    render(event.text_delta)
completion = handle.completion
```

`AutonomousBrain.open_stream(request, **options)` is a convenience wrapper around the same
Python runtime. Python keeps the arm order explicit because the Rust/MCP brain's selection report
is the authoritative decision surface; the stream bridge never re-ranks or silently overrides a
reviewed choice. `AutonomousStreamArm` can carry an in-memory credential capability for a specific
arm, but credentials never appear in `selection`, `continuation_plan`, completion receipts, or
event values.

The Python application façade now exposes the same direct preflight boundary:

```python
handle = agent.run_stream(
    task="review this implementation",
    domain="coding",
    credentials=credential_session,
    model_candidates=catalogue,
    approve_provider_call=True,
)

for item in handle.events:
    if item.kind == "provider":
        render(item.event.text_delta)
receipt = handle.completion
```

`AutonomousAgent.run_stream()` compiles the ordinary domain blueprint and model selection with
provider approval forced off, then lazily invokes the selected arm only after the caller has
approved the call and consumes the iterator. `run_auto_stream()` adds deterministic routing and
now fans out when the route selects multiple domains. An explicit cross-domain stream can also be
opened with caller-declared specialist subtasks:

```python
handle = agent.run_cross_domain_stream(
    task="Compare implementation risk with the scientific evidence",
    subtasks=(
        {"id": "engineering", "task": "Review implementation risk.", "domain": "coding"},
        {"id": "evidence", "task": "Review evidence quality and uncertainty.", "domain": "science"},
    ),
    credentials=credential_session,
    model_candidates=catalogue,
    max_parallelism=2,
    allow_partial=True,
    approve_provider_call=True,
)

for item in handle.events:
    if item.kind == "provider":
        render(item.event.text_delta)
    elif item.phase in {"child_started", "child_completed", "synthesis_started", "synthesis_completed"}:
        trace_lifecycle(item)
receipt = handle.completion
```

The cross-domain façade runs a provider-free parent preflight first. On consumption it opens a
bounded worker pool for the specialist streams, multiplexes child deltas with typed lifecycle
events, retains each specialist's text only in a bounded process-local synthesis buffer, and
opens synthesis only when the partial-result policy permits it. `allow_partial=False` fails closed
without synthesis after any child failure; `allow_partial=True` synthesizes only from completed
children and never turns a missing specialist into evidence. Semantic provider routing, provider
planning, evaluator settlement, missions, and tool loops remain separate authority boundaries.
Python streaming rejects durable execution-controller inputs: a partial provider transcript must
be rehydrated by the caller rather than implicitly replayed after a restart.

## Failover and partial-output safety

Failover is deliberately asymmetric:

- before the first event, a retryable provider failure may advance to the next bounded arm;
- after any event has been observed, the stream is terminal and is never replayed onto another
  model; and
- a normal stream must emit at least one `done=true` event before it is marked `completed`.

This prevents concatenating two model answers and prevents replaying a streamed tool intent that
the caller may already have displayed, logged, or authorized. A failed completion remains a typed
provider failure to the live caller, while the completion receipt records only safe failure
metadata. A consumer that closes the iterator before terminal completion receives an explicit
`abandoned` completion state. The handle is single-consumer; a second iterator is refused rather
than creating racing provider calls or ambiguous accounting.

The continuation plan is fixed at opening time. Later provider health changes cannot reorder the
active stream. A provider-scoped outage can consume one failover transition while skipping the
provider's remaining arms according to the compiled policy; no unbounded retry loop is possible.
The runtime's existing provider quota, execution controller, cost reservation, effect boundary,
observer, and caller abort signal remain attached to each attempted arm.

## Context and domain coverage

The optional context budget is applied to the exact request that will be dispatched. System and
developer instructions, the latest user task, recent messages, and the newest assistant/tool
continuation remain protected. Older removable turns are dropped atomically, and protected
overflow fails closed. The budget projection has counts, indexes, structural shape digests, and
retention markers only; it is not an LLM summary and does not claim semantic equivalence.

The event contract is domain-neutral. The same bridge carries coding, browser, data, science,
biomedical, neuroscience, operations, enterprise, multi-agent, multimodal, cross-domain, and
evaluation work. Domain-specific permissions and effect policy remain upstream gates; streaming
does not grant a model tools, credentials, mission dispatch, or external authority.

## Offline verification

Both SDKs include in-memory provider fixtures that exercise:

- selected-model streaming with deterministic context compaction;
- pre-event failover from a retryable primary to a backup arm;
- refusal to replay after a partial delta;
- explicit abandonment and single-consumer enforcement;
- completion redaction checks; and
- all twelve autonomous domains through the same provider-neutral event shape.

The Python cross-domain façade additionally covers lazy approval, bounded concurrent child fan-out,
typed child/synthesis lifecycle events, partial failure policy, transient synthesis fan-in, and
metadata-only stage receipts. These tests use in-memory providers and do not require an API key or
network access.

These tests do not require an API key or network access. Production callers supply their own
credential handles and provider transport; no key is read, generated, logged, or embedded by this
contract.
