# Bioevaluation evaluator-mesh audit

bioeval_mesh_audit exposes the bioprism-bioevalx evaluator-mesh kernel as a bounded MCP projection. It audits what evaluators are, what artifacts they read, which evaluators represent one shared evidence source, and how their verdicts disagree.

The route is deliberately not a consensus engine. It never majority-votes, averages confidence, promotes a model judge over a deterministic property, or adjudicates a split. Its purpose is to keep the evidence topology visible before a downstream ladder, reference distribution, or human review process uses the verdicts.

## Request

~~~json
{
  "system_artifacts": ["system-weights", "system-prompt"],
  "evaluators": [
    { "id": "reader-a", "kind": "expert_review", "inputs": ["report-77"] },
    { "id": "reader-b", "kind": "expert_review", "inputs": ["report-77"] },
    { "id": "imaging", "kind": "expert_review", "inputs": ["mri-4"] },
    { "id": "molecular", "kind": "executable_analysis", "inputs": ["panel-9"] }
  ],
  "verdicts": [
    { "evaluator": "reader-a", "position": "progression" },
    { "evaluator": "reader-b", "position": "treatment-effect" },
    { "evaluator": "imaging", "position": "progression" },
    { "evaluator": "molecular", "position": "pseudoprogression" }
  ],
  "expected": "progression",
  "max_items": 100,
  "require_independence": true,
  "require_independent_ratings": false
}
~~~

system_artifacts identifies artifacts that constitute the system under evaluation. An evaluator may read the system's answer or output; that is normal for a grader. It may not be admitted when its derived_from set intersects system_artifacts, because an oracle built from the model it grades is circular.

Each evaluator has one of seven closed kinds:

- deterministic_property — machine-checkable properties;
- executable_analysis — executable analysis and state transitions;
- metamorphic_relation — metamorphic or differential relations;
- statistical_reference — statistical reference distributions;
- prospective_reveal — longitudinal or prospective reveals;
- expert_review — expert review; and
- calibrated_model_judge — a calibrated model judge.

The mesh maps these kinds to the fixed evidence ladder. Deterministic properties map to deterministic, executable analysis to execution, metamorphic relations to property, statistical/reference and prospective evaluators to statistical, and expert/model judges to judge. The route does not let each caller choose a stronger tier for an evaluator kind.

## Admission and circularity

Admission invokes the real Mesh::admit rule. Evaluator identifiers are unique. An evaluator with derived_from: ["system-weights"] is refused when system_artifacts contains system-weights. The refusal occurs before a verdict can be projected, so a circular oracle cannot first produce a number and only later be marked questionable.

derived_from and inputs are distinct. Reading the evaluated system's output is a normal evaluator input; being trained, distilled, or constructed from the system artifact is circular. The route retains both fields in evaluator rows.

All evaluator and artifact identifiers are bounded at 256 bytes. Meshes are bounded at 1024 evaluators and 1024 verdicts, with independent row and witness projections bounded by max_items and a 20 MB encoded input ceiling.

## Independence classes

The mesh does not count evaluator instances as independent evidence. It partitions evaluators by shared input and uses the transitive closure:

~~~text
reader-a -- report-77 -- reader-b -- panel-9 -- molecular
~~~

is one class even if reader-a never read panel-9 and molecular never read report-77. The middle evaluator connects the evidence chain. An evaluator with no declared inputs is placed in its own class but is marked inputs_undeclared; it has not proved independence merely by declaring nothing.

The summary distinguishes:

- evaluator_count — declared evaluator instances;
- independent_class_count — evidence classes after shared-input collapse;
- non_model_class_count — classes containing at least one non-model judge;
- independence_verified — true only when every evaluator declares at least one consumed input; and
- inputs_undeclared — the evaluator identifiers that prevent that claim.

require_independence converts any undeclared-input condition into a fail-closed independence_policy refusal. With the policy false, the report is still usable for review, but the unverified posture remains explicit.

## Disagreement is a witness, not a rate

For every pair of called verdicts with different positions, the real mesh returns a witness. It does not emit a disagreement percentage because the denominator is undefined: evaluator pairs, independent classes, cases, and called cases answer different questions.

Two categories are kept separate:

- within_class means evaluators sharing evidence disagree. This is a defect or reliability problem in the evaluator class; it does not show that the case itself is difficult.
- across_class means independent evidence classes disagree. This is a finding about the case and should remain unresolved or go to adjudication; it must not be resolved by majority count.

Every witness names both evaluator identifiers and both positions. A row also contains about_case, which is false for a within-class defect and true for an across-class evidence split. The route keeps full counts and bounded rows, so truncation cannot turn a large unresolved mesh into an apparently clean one.

An abstention is not a dissent. A verdict with abstained: true is retained in verdict rows and findings.abstaining_evaluators, but it does not create a disagreement pair. Its empty position is valid only because it abstained. When contributions are requested, it becomes Conclusion::Unknown, never Fail.

## Independent ratings

independent_ratings is the mesh's join with the reference-panel machinery. It produces at most one rating per independence class. Members of an internally consistent class are joined into the rater identity with +, preserving the composition (for example, reader-a+reader-b).

If a class has called verdicts at more than one position, the rating projection is refused with a ClassSplit error. The route reports independent_ratings.status: "refused" and keeps the disagreement witnesses; it does not pick the class's majority position. An all-abstaining class contributes no rating, while its abstention remains visible.

require_independent_ratings turns that class-split refusal into a fail-closed rating_projection response. With the policy false, the broader mesh audit remains successful and the refusal is nested in the rating projection.

## Optional ladder contributions

When expected is supplied, the route invokes the real Mesh::contributions projection. A called evaluator at the expected position becomes Pass; a called evaluator at another position becomes Fail; an abstention becomes Unknown. Each contribution carries the evaluator's fixed kind-derived tier and a note explaining the call.

This is a typed evidence feed, not a final score. A downstream ladder may keep a deterministic failure above a judge pass. The route does not call compose, choose an unknown policy, or infer biological truth. When expected is absent, the contribution projection is explicitly not_requested.

## Successful projection

Schema is bioprism-mcp/bioeval-mesh-audit/0.1.

~~~json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-mesh-audit/0.1",
  "workflow": "bioeval_mesh_audit",
  "mesh": {
    "system_artifacts": ["system-weights"],
    "evaluator_count": 5,
    "independent_class_count": 4,
    "non_model_class_count": 4,
    "independence_verified": true,
    "kinds_present": ["expert_review", "executable_analysis", "statistical_reference"],
    "inputs_undeclared": []
  },
  "classes": {
    "rows": [{ "members": ["reader-a", "reader-b"], "size": 2, "inputs_declared": true }],
    "returned": 1,
    "total": 4,
    "omitted": 3
  },
  "disagreements": {
    "rows": [{
      "disagreement": {
        "kind": "within_class",
        "left": "reader-a",
        "left_position": "progression",
        "right": "reader-b",
        "right_position": "treatment-effect"
      },
      "about_case": false,
      "witness": {
        "left": "reader-a",
        "left_position": "progression",
        "right": "reader-b",
        "right_position": "treatment-effect"
      }
    }],
    "returned": 1,
    "total": 4,
    "omitted": 3,
    "within_class_count": 1,
    "across_class_count": 3
  },
  "independent_ratings": {
    "status": "refused",
    "rows": [],
    "refusal": "class split ..."
  },
  "contributions": {
    "status": "accepted",
    "expected": "progression",
    "rows": [],
    "refusal": null
  },
  "findings": {
    "inputs_undeclared": { "ids": [], "total": 0, "omitted": 0 },
    "unreported_evaluators": { "ids": [], "total": 0, "omitted": 0 },
    "abstaining_evaluators": { "ids": ["silent"], "total": 1, "omitted": 0 },
    "within_class_disagreement_count": 1,
    "across_class_disagreement_count": 3,
    "rating_projection_refused": true
  },
  "guarantees": [
    "shared inputs collapse into transitive classes",
    "same-class and across-class disagreement remain distinct",
    "abstentions remain unknown rather than failures"
  ],
  "limitations": [
    "declared inputs are not independently inspected",
    "the route does not adjudicate or majority-vote"
  ]
}
~~~

Evaluator, class, verdict, disagreement, and finding projections each carry bounded totals. independent_class_count is never substituted with the number of evaluator rows, and no suite-wide or mesh-wide agreement percentage is emitted.

## Composition and boundaries

The mesh audit composes with:

- bioeval_reference_audit, where class-collapsed ratings can form a more honest reference input;
- bioeval_evaluator_audit, which separates harness health from task outcome before a verdict enters this mesh;
- bioeval_design_audit, which carries kind-derived tiers into component attribution; and
- bioeval_waiver_audit, which can preserve a release exception without making disagreement disappear.

It does not replace the separate oracle_combine route. oracle_combine operates on already-built oracle judgements, tiered suppression, and same-tier settlement records. This route audits evaluator declarations and shared-input topology before those judgements are trusted as independent evidence.

## SDK surfaces

- Python exposes BioevalMeshEvaluatorArgs, BioevalMeshVerdictArgs, BioevalMeshAuditArgs, BioevalMeshAuditReport, and bioeval_mesh_audit_report(...) through Workspace, AsyncWorkspace, ApiClient, and AsyncApiClient.
- TypeScript exposes typed evaluator-kind, evaluator, verdict, audit-argument, and audit-result interfaces plus bioevalMeshAudit(...).
