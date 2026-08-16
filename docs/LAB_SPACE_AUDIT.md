# Inference-lab architecture-space audit

`lab_space_audit` exposes the structural boundary underneath holdout measurement, rollback, and
evolution cards. It validates an ordered set of immutable `CandidateArchitecture` bundles through
the real `bioprism-lab::ArchitectureSpace`, then projects lineage and deterministic component
diffs for selected configurations. It is useful before an experiment because it answers a more
basic question than “which candidate won?”: “what exactly is admissible, and what changed between
the bundles being compared?”

The endpoint is intentionally non-executing. It does not instantiate a component, resolve a
provider, run a model, inspect a benchmark, estimate cost, or infer performance from graph
structure. A valid space is admissible architecture structure, not evidence that a candidate is
useful or safe in operation.

## Request

```json
{
  "cost_ceiling": 100,
  "candidates": [
    {
      "id": "v1",
      "components": [
        { "id": "select", "kind": "context_selector", "feeds": ["run"] },
        { "id": "run", "kind": "executor", "feeds": ["stop"] },
        { "id": "stop", "kind": "terminator" }
      ],
      "cost_units": 0
    },
    {
      "id": "v2",
      "derived_from": "v1",
      "components": [
        { "id": "select", "kind": "context_selector", "feeds": ["run"] },
        { "id": "run", "kind": "executor", "feeds": ["stop"] },
        { "id": "stop", "kind": "terminator" }
      ],
      "cost_units": 2
    }
  ],
  "inspect": ["v2"],
  "comparisons": [{ "before": "v1", "after": "v2" }],
  "include_components": true,
  "max_rows": 100
}
```

Candidates are ordered. A derived bundle must name a parent that was already registered, so the
input order is part of the provenance contract. The registry refuses id rebinding; `v1` can never
silently acquire the components or cost of a later submission. The input is bounded to 512
candidates, 512 inspections, 512 comparisons, 20 MB, and 1,000 rows per projection.

## Kernel validation

Each bundle passes `CandidateArchitecture::validate(cost_ceiling)` before registration. The
kernel checks:

- component ids are unique;
- every `feeds` edge names a component in the same bundle;
- the component graph is acyclic and therefore has an execution order;
- the required `context_selector`, `executor`, and `terminator` kinds are present;
- no declared protected surface is touched;
- declared cost does not exceed the supplied ceiling.

Registration then checks duplicate configuration ids and parent registration. A failure is
structured with `ok: false`, `fail_closed: true`, a stage (`candidate_decode`,
`candidate_validation`, or `candidate_registration`), the candidate index, and the serialized
`SpaceError` when the kernel has one. The endpoint does not return a usable partial space after a
failure. Prefix rows may be retained to explain where ordered registration stopped, but
`space_committed` remains false.

Protected surfaces are not ordinary component kinds. The kernel's catalogue includes permission
core, audit log, secrets, benchmark splits, and release rules, together with a rationale for why
each is outside an evolvable candidate. The candidate declaration is still caller-supplied and
cannot prove that an implementation is honest about what it touches; an independent review gate
must close that gap.

## Candidate rows

The successful `candidate_rows` projection reports, for every registered bundle (subject to
`max_rows`):

- identifier and `derived_from` parent;
- declared cost, component count, edge count, and parameter count;
- the distinct component kinds and protected-surface declaration;
- validation and registration status;
- optional complete component declarations when `include_components` is true.

`candidate_rows_omitted` reconciles the bounded projection to `candidate_count`. The SDK rejects a
successful response where those counts do not agree.

## Lineage inspection

`inspect` selects configuration ids for a complete read-only projection. If omitted, all
registered ids are inspected. Each row is resolved through `ArchitectureSpace::lineage`, which
walks stored parent edges rather than trusting a cached root. It includes:

- the id-first lineage, with the oldest ancestor last;
- lineage depth and root id;
- the immediate parent;
- component ids and declared cost;
- optional component declarations.

Unknown ids, empty ids, and a deserialized registry cycle fail closed. Lineage is not decorative:
holdout contamination uses the same parent chain, so renaming or re-bundling a child cannot make
an exposed ancestor disappear from the audit.

## Component and parameter diffs

`comparisons` accepts `{before, after}` pairs. For each pair the kernel calls
`CandidateArchitecture::diff` instead of comparing opaque JSON or trusting ids. The result names
added and removed components, kind changes, parameter additions/removals/changes, and cost
changes. It also reports both complete lineages and whether `after.derived_from` directly names
`before`.

An empty `changes` list is meaningful: the bundles are structurally identical despite having
different identifiers. It is not upgraded to “an improvement” and it does not establish a causal
change. Comparison rows are separately bounded and carry their own `comparison_count` and
`comparison_rows_omitted` reconciliation.

## Successful projection

Schema is `bioprism-mcp/lab-space-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/lab-space-audit/0.1",
  "candidate_count": 2,
  "registered_count": 2,
  "space_committed": true,
  "space": {
    "registered_ids": ["v1", "v2"],
    "root_ids": ["v1"],
    "root_count": 1,
    "lineage_depth_max": 2,
    "required_component_kinds": ["context_selector", "executor", "terminator"],
    "protected_surfaces": ["..."]
  },
  "candidate_rows": ["..."],
  "candidate_rows_omitted": 0,
  "inspection_rows": ["..."],
  "inspection_count": 1,
  "inspection_rows_omitted": 0,
  "comparison_rows": ["..."],
  "comparison_count": 1,
  "comparison_rows_omitted": 0,
  "max_rows": 100,
  "guarantees": ["..."],
  "limitations": ["..."]
}
```

The successful report commits the complete validated space in memory for the duration of the
audit and exposes its roots, required kinds, protected-surface rationales, and maximum lineage
depth. It is still an offline projection; no state is persisted or changed outside the request.

## SDK surfaces

- Python exposes `LabSpaceAuditArgs`, `LabSpaceAuditReport`, and
  `lab_space_audit_report(...)` through `Workspace`, `AsyncWorkspace`, `ApiClient`, and
  `AsyncApiClient`. `LabSpaceAuditReport.complete` is true only when all three bounded row
  projections are complete.
- TypeScript exposes `labSpaceAudit(...)`, `LabSpaceAuditArgs`, and `LabSpaceAuditResult`.
  Nested bundle, lineage, and diff records remain JSON objects so the Rust kernel remains the
  source of schema truth.
- The MCP catalogue places the route under `inference_lab`, alongside planning, Pareto, risk
  branching, holdout, evolution, and routing audits.

Use this audit to establish structural provenance before `lab_holdout_audit` or
`lab_evolution_audit`. It is not a performance benchmark, provider readiness check, execution
trace, safety approval, or release gate.
