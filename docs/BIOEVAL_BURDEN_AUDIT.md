# Bioevaluation burden and resource audit

bioeval_burden_audit exposes the bioprism-bioevalx nonrenewable-resource ledger as a bounded MCP
projection. It makes the material cost of evaluation explicit before an agent compares branches,
claims a result, or reports a resource-adjusted workflow.

The route is a ledger audit, not a cost-benefit optimizer. It does not invent prices for tissue,
compute, privacy, participant burden, expert time, or assay capacity. It does not produce a utility
score, select a preferred branch, or convert residual quantity into scientific value. Those are
separate declarations that require an explicit utility and policy model.

## Request

~~~json
{
  "root": "root",
  "resources": [
    { "id": "biopsy", "class": "tissue_aliquot", "initial": 100, "unit": "uL" },
    { "id": "compute", "class": "compute_and_money", "initial": 10, "unit": "hour" }
  ],
  "branches": [
    { "id": "candidate-a", "parent": "root" },
    { "id": "candidate-b", "parent": "root" }
  ],
  "draws": [
    {
      "branch": "root",
      "action": "extract",
      "resource": "biopsy",
      "amount": 30,
      "unit": "uL",
      "outcome": "wasted",
      "destructive": true
    },
    {
      "branch": "candidate-a",
      "action": "sequence-a",
      "resource": "biopsy",
      "amount": 60,
      "unit": "uL",
      "outcome": "productive",
      "destructive": true
    }
  ],
  "inspect_branches": ["root", "candidate-a"],
  "joint_branches": ["candidate-a", "candidate-b"],
  "max_items": 100,
  "require_joint_feasible": false,
  "require_no_wasted_nonrenewable": false
}
~~~

Resources are integer pools with a declared unit. The route compares units by exact equality. It
will not infer that mL and uL are compatible, because a hidden conversion factor could change the
amount of material attributed to an action. A draw with amount zero is legal and remains a
declared action; negative quantities cannot be represented.

The seven closed resource classes are:

- tissue_aliquot;
- viable_cells;
- assay_capacity;
- expert_time;
- participant_burden;
- privacy_access; and
- compute_and_money.

The ledger treats tissue aliquots, viable cells, participant burden, and privacy access as
nonrenewable. Assay capacity, expert time, and compute_and_money are not marked nonrenewable by
this kernel. That classification is a conservation rule, not a claim that renewable resources
are free or ethically unimportant.

## Branch inheritance

The root branch exists before any optional branch declaration. Each branch is forked from a
previously declared parent, and a missing parent means root. Parent declarations are ordered:
the SDK rejects a child whose parent has not appeared earlier, while the MCP route returns a
branch_validation refusal.

A child inherits every draw made by its ancestors. If root spends 30 uL and candidate-a spends
60 uL, the residual on candidate-a is 10 uL from an initial pool of 100 uL. The branch projection
reports:

- the branch id and parent;
- local draw count;
- local consumed quantity by resource;
- local wasted quantity by resource; and
- residual quantity after the complete ancestor path.

Local consumption and inherited residual are intentionally separate. A branch row does not pretend
that a child performed its parent draw, but it does not allow the child to spend an ancestor's
quantity twice.

## Draw admission

Every draw is admitted through the real Ledger::draw rule. The route refuses:

- an undeclared resource;
- an undeclared branch;
- a unit mismatch;
- a draw larger than the inherited remainder; or
- malformed outcome, action, resource, or unit fields.

Admission is sequential. A later draw sees the quantity consumed by earlier draws in the same
branch and by every ancestor. When a draw is refused, no success projection is returned, so a
caller cannot accidentally use a partially admitted ledger as if it were complete.

The outcome field has two values:

- productive means the funded action produced its intended result; and
- wasted means the funded action failed or produced no usable result.

Both outcomes consume the ledger. A failed assay that used 20 uL still consumes 20 uL. The route
does not permit a failed action to disappear from the denominator.

The destructive field is explicit. A non-destructive draw can consume capacity or time without
claiming that the underlying material was destroyed. Destructiveness matters to the selected
joint-feasibility check and the wasted nonrenewable projection.

## Fork feasibility

Ledger::fork intentionally permits exploration. It is legitimate to ask what candidate-a and
candidate-b would do with the same starting specimen. The error is reporting both destructive
uses as one jointly executed plan.

joint_branches invokes the real Ledger::joint_feasibility rule. The nested result has one of three
statuses:

- not_requested means the request did not select a branch set;
- accepted means the selected set has no detected nonrenewable double-spend; and
- refused means two selected branches destructively consume the same nonrenewable resource.

The route never turns refused into a lower score or silently drops one branch. The refusal contains
the resource and the conflicting branch witnesses from the kernel error. The optional
require_joint_feasible policy converts a nested refusal into a fail-closed MCP response at
joint_feasibility_policy. If the policy is true without joint_branches, the request is refused
because the required check was not supplied.

This rule is intentionally narrower than total budget addition. Two branches can both use
compute_and_money without a specimen contradiction, while two destructive uses of one tissue pool
are mutually exclusive even if each branch individually fits its inherited remainder.

## Failed-action waste

wasted_nonrenewable contains destructive draws from a nonrenewable resource whose outcome is
wasted. Each row retains branch, action, resource, amount, unit, outcome, and destructive status.
The total and omitted fields remain present when max_items truncates the rows.

The optional require_no_wasted_nonrenewable policy refuses at waste_policy when an inspected branch
contains such a row. This is a bounded safety policy, not an assertion that every failed assay was
avoidable. The kernel reports material destroyed for a failed action because that fact is
auditable; it does not infer a counterfactual alternative or label the action negligent.

The inspected branch set is separate from joint_branches. Omitting inspect_branches projects every
branch. Supplying an empty list projects no branch rows and therefore makes waste policy vacuous
for the projection; callers that require a waste audit should name the branches explicitly.

## Successful projection

Schema is bioprism-mcp/bioeval-burden-audit/0.1.

~~~json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-burden-audit/0.1",
  "workflow": "bioeval_burden_audit",
  "burden": {
    "root": "root",
    "resource_count": 2,
    "branch_count": 3,
    "draw_count": 4,
    "nonrenewable_resource_count": 1,
    "resource_class_counts": {
      "compute_and_money": 1,
      "tissue_aliquot": 1
    },
    "inspected_branch_count": 3,
    "policies": {
      "require_joint_feasible": false,
      "require_no_wasted_nonrenewable": false
    }
  },
  "resources": {
    "rows": [],
    "returned": 0,
    "total": 2,
    "omitted": 2
  },
  "branches": {
    "rows": [],
    "returned": 0,
    "total": 3,
    "omitted": 3
  },
  "draws": {
    "rows": [],
    "returned": 0,
    "total": 4,
    "omitted": 4
  },
  "joint_feasibility": {
    "status": "refused",
    "branches": ["candidate-a", "candidate-b"],
    "refusal": "fork candidate-a and fork candidate-b both consume biopsy"
  },
  "wasted_nonrenewable": {
    "rows": [],
    "returned": 0,
    "total": 1,
    "omitted": 1
  },
  "findings": {
    "wasted_nonrenewable_actions": {
      "ids": ["extract"],
      "total": 1,
      "omitted": 0
    },
    "joint_feasibility_refused": true,
    "failed_draws_still_counted": 2
  },
  "guarantees": [
    "failed actions retain their resource consumption",
    "branch residuals include inherited ancestor draws",
    "nonrenewable fork double-spends remain refusals rather than plausible totals",
    "unit mismatch is refused without guessing a conversion",
    "wasted destructive nonrenewable draws remain visible"
  ],
  "limitations": [
    "no utility, price, optimal policy, or accuracy trade-off is inferred",
    "declared resources and draws are not independently authenticated",
    "joint feasibility only evaluates the selected branch set"
  ]
}
~~~

Every bounded collection has returned, total, and omitted fields. The omission count is part of
the evidence: a short preview is not allowed to look like a complete ledger.

## Refusal stages

The route uses structured fail-closed stages:

- resource_deserialization and resource_validation protect the resource vocabulary;
- resource_declaration protects duplicate pool identity;
- branch_deserialization, branch_validation, and branch_declaration protect the fork tree;
- draw_deserialization, draw_validation, and draw_admission protect the sequential ledger;
- branch_selection and joint_selection protect projection scope;
- joint_feasibility_policy protects a required cross-branch conservation check; and
- waste_policy protects a required failed-action waste policy.

The refusal includes guarantees and limitations, so a downstream agent can distinguish a
non-admissible ledger from an admissible ledger with a nested joint-feasibility finding.

## SDK surfaces

Python exposes BioevalBurdenResourceArgs, BioevalBurdenBranchArgs, BioevalBurdenDrawArgs,
BioevalBurdenAuditArgs, BioevalBurdenAuditReport, and bioeval_burden_audit_report through
Workspace, AsyncWorkspace, ApiClient, and AsyncApiClient. The typed argument layer validates
resource class, branch ordering, resource and branch references, integer quantities, exact
outcome vocabulary, and bounded input size before transport.

TypeScript exposes resource-class, draw-outcome, resource, branch, draw, audit-argument, and
audit-result interfaces plus bioevalBurdenAudit. Both SDK families retain nested joint refusal
and waste findings rather than converting them to a single boolean score.

## Composition and boundaries

The burden audit can feed a matched-evaluation or lab report as a conservation witness. It can
also sit beside bioeval_design_audit: the design route describes what contrast was declared, while
this route describes whether the material use of each branch can coexist. It does not replace
evaluation_worldline_audit, which governs temporal evidence availability, and it does not replace
an explicit utility or metric plane.

The key boundary is between possibility and preference. A branch with more residual tissue is not
automatically better. A productive draw is not automatically scientifically valid. A joint refusal
does not say which branch to choose. This route preserves those distinctions so later policy can
make them explicitly.
