# Inference-lab evolution claim audit

`lab_evolution_audit` is the capstone boundary between an offline architecture experiment and a
claim that a change improved a metric. It is deliberately narrow: it can produce an
`improvement_claimed` result only when the Rust kernel has minted both a clean baseline
measurement and a clean candidate measurement on the same certifying holdout surface, and when
the resulting `EvolutionCard` accepts the proposal, rollback, metric, direction, protected
surface, and defeater obligations.

The endpoint does not run an architecture, execute a benchmark, estimate uncertainty, deploy
traffic, or approve a release. It audits whether a supplied before/after measurement story is
eligible to become an evolution claim. Caller-supplied values remain caller-supplied evidence;
the important guarantee is that a value cannot be relabeled as clean merely by serializing a
`CleanMeasurement`-shaped object.

## Request

```json
{
  "cost_ceiling": 100,
  "candidates": [
    {
      "id": "v1",
      "components": [
        { "id": "select", "kind": "context_selector" },
        { "id": "run", "kind": "executor" },
        { "id": "stop", "kind": "terminator" }
      ],
      "cost_units": 0
    },
    {
      "id": "v2",
      "derived_from": "v1",
      "components": [
        { "id": "select", "kind": "context_selector" },
        { "id": "run", "kind": "executor" },
        { "id": "stop", "kind": "terminator" }
      ],
      "cost_units": 0
    }
  ],
  "baseline": "v1",
  "candidate": "v2",
  "holdout": {
    "id": "private-a",
    "partition": "rotating_private_certification",
    "query_budget": 4
  },
  "measurements": [
    { "configuration": "v1", "metric": "admissible_rate", "value": 0.70 },
    { "configuration": "v2", "metric": "admissible_rate", "value": 0.83 }
  ],
  "card_id": "card-v2",
  "proposal": {
    "id": "proposal-v2",
    "rationale": "widen the protected closure",
    "target_failure_clusters": ["cluster:missing-closure"],
    "changed_artifacts": ["component select depth 3 -> 5"],
    "regression_cells": ["cell:closure"],
    "touches_protected": []
  },
  "rollback_handle": "v1",
  "direction": "higher_is_better",
  "would_have_to_be_true": [
    "the gain survives a second rotating private set"
  ],
  "max_rows": 100
}
```

Exactly two architecture bundles are accepted. Each bundle is validated through
`ArchitectureSpace` before the holdout is created: required component kinds, graph integrity,
protected surfaces, cost ceiling, duplicate ids, and registered lineage are all kernel checks.
The `baseline` and `candidate` names must be distinct registered configurations. The proposal is
deserialized into the kernel's `ChangeProposal`; its changed-artifact and protected-surface
declarations are therefore available to the card validator rather than being treated as free-form
metadata.

## Clean evidence boundary

Every measurement is attempted against a fresh append-only `HoldoutLedger`. The ledger decides
whether the configuration, metric, value, and partition may yield a `CleanMeasurement`. The
endpoint stores the kernel-minted value internally and passes those typed measurements directly
to `EvolutionCard::measured`; it never accepts a caller-deserialized clean measurement.

The measurement rows make the boundary inspectable:

- `clean_measurement` contains the serialized measurement minted by the ledger;
- `measurement_refused` contains a typed refusal, `fail_closed: true`, and no numeric score;
- `measurement_count`, `max_rows`, and `measurement_rows_omitted` reconcile the bounded output;
- the first refusal is retained as contamination evidence, even when later rows are also
  refused.

A repeated measurement, a non-certifying partition, a selected/search-exposed configuration, a
burned ancestor or descendant, a retired holdout, an exhausted query budget, and a non-finite
value all remain refusals. Rollback is not available as an escape hatch here: a fresh ledger is
used for the audit, and the ledger's append-only semantics are the authority whenever a caller
has already consumed a holdout elsewhere.

## Result state machine

The endpoint distinguishes audit completion from claim success:

| State | Meaning | Claim allowed |
| --- | --- | --- |
| `improvement_claimed` | Two clean measurements formed a valid card and the requested direction is positive. | Yes |
| `claim_refused` | Two clean measurements formed a card, but the delta is not an improvement or a card obligation fails. | No; the negative result is retained. |
| `contaminated` | At least one measurement was refused. A contaminated card records the refusal. | No, permanently for this card. |
| structured `ok: false` | Architecture, holdout, proposal, or completeness failed before a claimable card existed. | No; fail closed. |

For a clean card, the kernel still checks that baseline and candidate measure the same metric and
surface, that the candidate is genuinely related to the proposed change, that a valid rollback
handle exists, and that the required defeater statements are present. `higher_is_better` and
`lower_is_better` are explicit; the endpoint never guesses the direction from the sign of a
number.

## Successful projection

Schema is `bioprism-mcp/lab-evolution-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/lab-evolution-audit/0.1",
  "status": "improvement_claimed",
  "claimable": true,
  "card": { "id": "card-v2", "surface": { "surface": "rotating_private_certification" } },
  "claim": { "card_id": "card-v2", "delta": 0.13 },
  "sentence": "...",
  "measurement_count": 2,
  "measurement_rows": ["..."],
  "measurement_rows_omitted": 0,
  "max_rows": 100,
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

The `claim` and `sentence` are only present for `improvement_claimed`. A contaminated or
claim-refused result keeps the `card`, `claim_refusal`, measurement rows, and explicit
limitations, but its `claimable` flag is false. The SDK parsers reject a response whose status
and `claimable` flag disagree, whose claimability is missing its claim/refusal, or whose bounded
row counts do not reconcile.

## Fail-closed stages

Structural and evidentiary failures retain a stage so a caller can route the result without
parsing a prose error:

- `architecture_validation` — a bundle violates the architecture space contract;
- `architecture_registration` — duplicate or unresolvable lineage prevents a complete space;
- `measurement_completeness` — the baseline or candidate has no clean measurement;
- `card_validation` — the measured before/after pair cannot satisfy evolution-card obligations.

These refusals use `ok: false`, `fail_closed: true`, a typed `error` where the kernel exposes one,
and a bounded evidence projection when measurements had already been attempted. No partial card
is presented as a claim.

## SDK surfaces

- Python exposes `LabEvolutionAuditArgs`, `LabEvolutionAuditReport`, and
  `lab_evolution_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`. The report has `accepted`, `refused`, `contaminated`, `claimable`, and
  omission-aware measurement properties.
- TypeScript exposes `labEvolutionAudit(...)`, `LabEvolutionAuditArgs`, and
  `LabEvolutionAuditResult`; nested kernel card/claim records stay JSON objects so the Rust
  schema remains authoritative.
- The MCP catalog advertises this route under `inference_lab`, alongside planning, Pareto,
  risk-branch, holdout, and routing audits.

This is an evolution-claim gate, not a benchmark runner, statistical test, causal inference
engine, biological validity certificate, deployment controller, or release approval. Pair it with
benchmark integrity, oracle, counterfactual, holdout, and delivery audits before making a larger
scientific or operational claim.
