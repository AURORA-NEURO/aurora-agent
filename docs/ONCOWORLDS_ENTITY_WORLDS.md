# OncoWorlds entity-world safeguards

`oncoworlds_entity_world_check` composes the structural safeguards for blueprint sections
30.20–30.24 into one auditable report. It is deliberately a check surface, not a classifier,
effect estimator, or clinical authorization boundary. The server returns schema
`bioprism-mcp/oncoworlds-entity-world-check/0.1` and keeps each requested section independent.

## Sections

- `provenance` accepts two `TissueProvenance` values and `selection_modelled`. Diagnostic,
  recurrence, and postmortem material is admissible for pooling only when selection is modelled.
- `alterations` accepts two `AlterationMechanism` values and an optional `estimand`. Mechanism
  differences cannot be collapsed into a pathway-level result without a declared estimand.
- `benchmark` accepts a macro score and `per_class_counts`. A published rare-class result retains
  class counts and feasibility; zero-count classes refuse publication instead of becoming a hidden
  macro-average limitation.
- `lesion_analysis` accepts lesion and participant counts, `cluster_declared`, a `LesionEndpoint`,
  a `FollowUpEvent`, and `EventHandling`. Cluster and competing-event decisions are reported
  separately, so an invalid local-control analysis cannot hide a valid event-handling result.

At least one section is required. Each section has its own `allowed` state and typed refusal
fields. The top-level `all_admissible` and `refusal_count` are reconciliations over the requested
sections, not a replacement for inspecting the individual evidence.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/oncoworlds-entity-world-check/0.1",
  "outcome_kind": "report",
  "all_admissible": false,
  "check_count": 2,
  "refusal_count": 1,
  "checks": {
    "provenance": {"allowed": false, "refusal_kind": "unmodelled_provenance_selection"},
    "benchmark": {"allowed": true, "feasibility_kind": "feasible"}
  }
}
```

The Python SDK provides `OncoWorldsEntityWorldCheckArgs`,
`OncoWorldsEntityWorldCheckReport`, and
`oncoworlds_entity_world_check_report(...)`. The TypeScript client provides
`oncoworldsEntityWorldCheck(...)`. Both preserve the nested Rust evidence as structured values
while validating section names, outcome kind, counts, and refusal consistency at the SDK edge.
