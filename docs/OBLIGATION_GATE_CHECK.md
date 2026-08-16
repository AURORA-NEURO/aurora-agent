# Obligation gate checks

`obligation_gate_check` evaluates a serialized `bioprism_obligation::Action` against a serialized
`ObligationGraph`. It is the action-permission boundary for the agent: a high-regret action is not
allowed merely because its caller supplied a plausible-looking boolean. The Rust obligation kernel
validates dependency structure, computes effective states, and returns the full typed `Gate`.

## Request

The request requires `graph` and `action`, with an optional `max_items` projection bound:

```json
{
  "graph": {
    "goal": "publish a validation report",
    "obligations": {
      "identity": {
        "id": "identity",
        "statement": "the specimen identity is established",
        "value": 3.0,
        "mandatory": true,
        "history": [{
          "state": "satisfied",
          "actor": "reviewer",
          "at": "2026-08-14T00:00:00Z",
          "confidence": 0.95,
          "evidence": ["evidence://identity"]
        }]
      },
      "validation": {
        "id": "validation",
        "statement": "the assay validation is complete",
        "depends_on": ["identity"],
        "value": 5.0,
        "mandatory": true,
        "history": []
      }
    }
  },
  "action": {
    "id": "publish",
    "description": "publish the validation result",
    "regret": "irreversible",
    "prerequisites": [{
      "obligation": "validation",
      "accept": ["satisfied"],
      "min_confidence": 0.0
    }]
  },
  "max_items": 100
}
```

The graph and action are deserialized by Rust. SDKs validate their envelopes and projection bound,
but do not duplicate the obligation state machine, prerequisite semantics, or timestamp schema.
State records remain append-only and require the actor, time, confidence, and the evidence/reason
fields demanded by their state.

## Gate semantics

The kernel applies these fail-closed rules:

1. A cycle or dangling dependency makes the graph unverifiable and blocks the action.
2. High-regret and irreversible actions with no declared prerequisites are blocked. Omitting a gate
   is not a way to pass it.
3. An unknown prerequisite blocks; deleting an obligation cannot satisfy a predicate.
4. A prerequisite is evaluated against its effective dependency-capped state, not only its latest
   recorded state. An obligation recorded `satisfied` can remain only `partially_supported` when
   an upstream dependency is open.
5. The accepted state set and minimum confidence are part of each predicate. A waiver is not
   accepted unless the predicate explicitly includes `waived_with_reason`.
6. Irreversible actions additionally require every mandatory obligation to be evidentially
   discharged, including mandatory obligations the action did not name.

The result is `outcome_kind: "allowed"` only when the kernel returns `Gate::Allowed`; otherwise it
is `"blocked"` and `refusal` carries the serialized typed block reason. The complete `gate` is
retained for replay, including checked prerequisites and unmet prerequisite rows.

## Graph projection

`graph` in the response is a bounded inspection projection, not a replacement for the graph. It
includes validation state, a content SHA-256, obligation count, mandatory IDs, topological order,
effective states, the actionable frontier, and undischarged obligations. Each row family has an
omitted count when `max_items` is smaller than the graph. The frontier is ordered by declared
decision value and tie-broken deterministically by obligation ID.

The digest binds downstream reviews to the exact serialized graph used by the gate. It does not
authenticate the caller, the evidence locators, or the actor identity; those remain separate
authority and provenance concerns.

## Non-claims

An allowed result is permission under the supplied graph and policy declarations, not execution,
authorization, evidence acquisition, scientific truth, or calibrated probability. The handler
does not run the action, query a database, read a specimen, authenticate an actor, or turn stated
confidence into statistical confidence. A bounded projection with omitted rows is never a claim
that the omitted obligations do not exist.

`lab_plan` remains the acquisition planner: it orders caller-declared evidence actions under
budget and privacy constraints. `obligation_gate_check` answers the separate question of whether
an already-declared high-regret action may run against the current obligation graph.

The Python SDK exposes `ObligationGateCheckArgs`, `ObligationGateCheckReport`, and
`obligation_gate_check_report`; the TypeScript SDK exposes `ObligationGateCheckArgs`,
`ObligationGateCheckResult`, and `obligationGateCheck`.
