# Autonomous provider-neutral streaming

The SDKs now expose a first-class live stream boundary between autonomous model selection and
the application that renders or consumes model output. It is intentionally a transport contract,
not a second planning engine: the brain still owns domain routing, model eligibility, prompt
assembly, plan approval, effect authority, evaluator credit, and learning policy.

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

These tests do not require an API key or network access. Production callers supply their own
credential handles and provider transport; no key is read, generated, logged, or embedded by this
contract.
