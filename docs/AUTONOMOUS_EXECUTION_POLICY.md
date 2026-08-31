# Joint autonomous execution policy

`AutonomousJointExecutionPolicy` is the value-only decision layer between a task decision and a
provider/tool/effect boundary. It is intentionally small enough to run in a browser, worker, or
server process, while its state can be checkpointed and restored by a caller-owned store.

The policy answers one question: given a reviewed context and the currently available execution
arms, which arm is the best bounded next action? An arm can represent a provider/model pair, an
evidence-first route, a workflow, a planning route, a cross-domain fan-out, or a tool loop. The
policy does not create those arms and does not execute them.

## Decision pipeline

1. The caller supplies a context digest, requested domains, required/preferred capabilities, and
   resource/risk limits. Task text is never accepted by this module.
2. The caller supplies current candidate metadata. Candidates are rejected before scoring when
   their domain, path, capability, evidence, structured-output, effect, availability, cost,
   latency, or risk contract is incompatible with the context.
3. Eligible candidates receive a deterministic contextual UCB score. The score blends the
   candidate's quality prior and reliability with the arm's evaluator history, a bounded
   exploration bonus, capability match, and cost/latency/risk penalties. Ties are resolved by
   exploitation, reliability, and arm ID, so identical inputs produce identical decisions.
4. The result is `selected`, `review_required`, or `refused`. A review posture may still identify
   the best arm, but it never authorizes a provider, source, tool, credential, or effect.
5. After execution, the caller supplies an explicit evaluator ID/version, outcome digest, reward,
   and pass state. Only then does `settle` update the arm posterior. Reusing a settlement ID with
   identical credit is idempotent; conflicting reuse is rejected.

Evaluator rewards are bounded to `[-1, 1]`, so a reviewer can express both positive evidence and
explicit negative evidence. Negative credit is accumulated in the value-only arm aggregate and is
accepted by snapshot/decision validation; this lets later selection down-rank a repeatedly poor
provider or route without treating transport failure as an automatic reward.

## State and safety

The persisted state contains only arm counts, bounded reward aggregates, outcome digests, and
evaluator identity. It is generation-numbered, content-digested, and carries a predecessor digest
for every post-initial state, so a caller can detect stale snapshot rollback. State and decision
validators reject malformed or forged metadata. The state has bounded arm and settlement
capacities, and the public projections carry explicit retention and `never_returned` markers.

Transport success is not reward. A caller must perform domain/evaluator review before settlement;
the policy has no callback that can inspect provider output and no path that can infer quality from
HTTP status. Provider credentials, prompts, responses, tool arguments, task text, and external
effect payloads therefore remain outside both SDK implementations.

The TypeScript and Python implementations intentionally share schemas, field names, defaults,
scoring weights, tie-breaks, digest inputs, and replay rules. Import the namespaced public API:

```ts
import {
  AutonomousJointExecutionPolicy,
  AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS,
} from "@aurora-neuro/prism-sdk";

const policy = new AutonomousJointExecutionPolicy({ exploration: 0.35 });
const decision = policy.select(
  {
    context_digest: "<sha256>",
    requested_domains: [...AUTONOMOUS_JOINT_EXECUTION_POLICY_DOMAINS],
    required_capabilities: ["reasoning"],
    max_cost_units: 100,
    max_latency_ms: 5_000,
    max_risk: 0.5,
  },
  reviewedCandidates,
);
```

The Python package exposes the same contract using the `AutonomousJointExecutionPolicy*`
names. Keep the policy instance with the caller's durable learning lifecycle, and pass its
snapshot through the existing persistence/rehydration boundary when a worker restarts.
