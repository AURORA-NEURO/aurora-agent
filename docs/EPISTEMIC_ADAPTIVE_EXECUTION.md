# Adaptive execution and replay boundary

The adaptive planner and an acquisition provider live on different evidence planes. A policy tree
contains model-relative likelihoods and an expected objective; it does not prove that a laboratory
assay, repository test, literature query, or operational check happened. The
`bioprism-epistemic/adaptive-execution/0.1` contract is the narrow handoff between them.

## Lifecycle

```text
DecisionProblem + Belief + Acquisitions
                 │
                 ▼
          AdaptivePlan + digest
                 │
       explicit plan-scoped grant
                 │
                 ▼
     provider request ──► one observation
                 │              │
                 │              ├─ outcome label must be declared
                 │              ├─ acquisition/provider identities must match
                 │              └─ evidence digest must be valid
                 ▼
       prefix receipt ──► next policy branch or terminal action
                 │
                 └────────► ReceiptReplayExecutor (no live source)
```

`AdaptivePlan::new` computes the exact policy and binds it to all inputs. `AdaptivePlan::from_policy`
is the bridge for a policy produced by another layer, such as FIBER; it rechecks tree identity,
outcome partitions, path uniqueness, posterior shape, path budget, and enumeration metadata before
allowing a handoff. `AdaptivePlan::digest` hashes the complete bound plan, not only its root node.

## Authorization and provider identity

An `ExecutionGrant` carries a private grant id, the plan digest, and a provider id. The grant is
created only after the caller's domain gate has recorded whatever consent, review, or operational
authority applies. The epistemic crate can check that the grant scopes the exact plan and provider,
but it cannot authenticate a human, sign a provider, establish chain of custody, or create clinical
or release authority.

The provider implements the small `AcquisitionExecutor` trait:

```rust
trait AcquisitionExecutor {
    fn provider_id(&self) -> &str;
    fn acquire(
        &mut self,
        request: &AcquisitionRequest,
    ) -> Result<AcquisitionObservation, String>;
}
```

This is intentionally domain-neutral. Adapters can represent a read-only data check, a software
test, an evidence lookup, a research assay, or an operational inspection without changing the
planner. The kernel itself performs no external I/O.

## Receipt states

- `completed` means the selected policy reached a `Stop` node and the receipt has a terminal action
  and risk.
- `partial` means at least one validated observation exists, but the next provider call failed or
  the provider returned an invalid identity/outcome/digest.
- `refused` means no terminal claim is allowed. Missing authorization, a scope mismatch, an
  undeclared outcome, a budget violation, and a malformed policy are all refusal states.

Every validated prefix remains in the receipt. A provider failure is never changed into a stop
decision, and an unobserved branch is never filled with its model-predicted most likely outcome.
Observation provenance is one of `observed`, `simulated`, or `replayed`; the report also exposes
reconciled counts for all three.

## Replay

`ReceiptReplayExecutor` contains only receipt rows, a cursor, and a replay provider identity. It
has no live provider field and refuses if the next request's plan digest, sequence, or acquisition
id differs from the recorded row. Replaying a simulated receipt produces `replayed` rows; it does
not upgrade them to `observed`. Replaying a receipt is therefore a deterministic audit operation,
not a second external acquisition.

## MCP and SDK surface

`epistemic_adaptive_execute` accepts the original decision inputs, optional
`authorization: {grant_id, provider}`, and either:

- `mode: "simulate"` with scripted `{acquisition_id, outcome_label}` rows. The built-in MCP
  adapter labels every row `simulated`;
- `mode: "replay"` with a prior receipt. The server uses receipt-only replay and never reaches a
  live source.

Calling without authorization returns a structured no-call refusal. The Python SDK exposes
`AdaptiveExecutionRequest`, `AdaptiveExecutionReport`, synchronous/asynchronous Workspace and
HTTP helpers, and provenance-count validation. TypeScript exposes the corresponding request,
receipt, observation, and result interfaces plus `client.epistemicAdaptiveExecute`.

The MCP route is a contract adapter, not a provider marketplace. Real domain providers should
implement the Rust seam and attach their own authentication, privacy, consent, cancellation,
chain-of-custody, and release evidence before declaring an observation `Observed`.

For vector-aware planning, `epistemic_adaptive_costed` is a separate route documented in
[`EPISTEMIC_COST_VECTORS.md`](EPISTEMIC_COST_VECTORS.md). It must not be confused with execution:
it produces a component-feasible policy and no provider request. For the cross-domain workflow
catalogue, `bioprism-interweave::workflow_execution` binds an adaptive plan to a workflow identity
and carries the receipt through deterministic replay; it remains receipt-only and does not grant
external release authority. See
[`WORKFLOW_EXECUTION_BINDING.md`](WORKFLOW_EXECUTION_BINDING.md).
