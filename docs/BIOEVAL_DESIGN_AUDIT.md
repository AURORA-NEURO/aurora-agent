# Bioevaluation factorial-design audit

`bioeval_design_audit` exposes the `bioprism-bioevalx` factorial-design kernel
as a bounded MCP projection. It turns a complete set of caller-declared arms
into the contrasts that are actually attributable to one component, reports
the arms that cannot support a single-component claim, and checks whether
interaction questions have all of their required cells.

The route is a design and attribution audit. It does not run an arm, estimate
an effect size, randomize an intervention, pair seeds, measure cost or latency,
or infer a biological mechanism. A valid design makes a question askable; it
does not make the answer true.

## Request

```json
{
  "cell_id": "cell-7",
  "factors": ["planner", "verifier"],
  "baseline": "base",
  "arms": [
    {
      "id": "base",
      "levels": { "planner": "react", "verifier": "off" },
      "conclusion": "fail",
      "tier": "execution"
    },
    {
      "id": "planner-tree",
      "levels": { "planner": "tree", "verifier": "off" },
      "conclusion": "pass",
      "tier": "execution"
    },
    {
      "id": "verifier-on",
      "levels": { "planner": "react", "verifier": "on" },
      "conclusion": "pass",
      "tier": "execution"
    },
    {
      "id": "both-changes",
      "levels": { "planner": "tree", "verifier": "on" },
      "conclusion": "pass",
      "tier": "execution"
    }
  ],
  "controlled": true,
  "max_items": 100,
  "require_contrasts": true,
  "require_complete_interactions": true,
  "require_attribution": true
}
```

`cell_id` is the frozen decision state from which every arm resumes. The route
does not compare arms from different cells. `factors` is the complete declared
factor vocabulary; every arm must assign every factor exactly once, and an arm
cannot introduce an undeclared factor. The baseline is named explicitly and
must match one arm. It is never inferred from which arm happened to have the
best conclusion.

Requests are bounded to 256 factors, 4096 arms, 256-byte identifiers and level
names, 1000 returned rows per projection, and a 20 MB encoded request. Duplicate
arm identifiers, duplicate cells, partial assignments, undeclared factors, and
missing baselines fail closed before contrasts are generated.

## Arm conclusions and evidence tiers

An arm carries the evalengine conclusion vocabulary:

`pass`, `unsupported_pass`, `contradicted_pass`, `partial_credit`, `fail`,
`vetoed`, `disputed`, `justification_unexamined`, `unknown`, or `abstained`.
The evidence tier is one of `judge`, `statistical`, `property`, `execution`, or
`deterministic`. These are carried into the real `MatchedFork` and attribution
kernel. The design route does not reinterpret `unknown`, `vetoed`, or
`disputed` as a low score; the attribution kernel can return an indeterminate
direction when a pair cannot be ordered.

The tier of an attribution is the weaker side's tier. A clean one-factor fork
with two judge conclusions is still judge-tier evidence. A caller cannot raise
the tier by declaring `controlled`.

## Single-factor contrasts

The real `FactorialDesign::single_factor_contrasts` enumerates arm pairs that
differ in exactly one factor. The result names:

- `factor` — the one changed factor;
- `baseline` and `variant` — the ordered arm identifiers;
- `from_level` and `to_level` — the level transition.

Pairs differing in two or more coordinates are absent from `contrasts`. This
absence is intentional: a comparison from `react/off` to `tree/on` cannot say
which component caused the difference. Such arms are still valuable for
interaction coverage, but not for a single-component attribution.

The baseline arm influences orientation. When the baseline participates in a
pair, the contrast is stated from the baseline's level to the variant's. Other
one-factor pairs retain their declaration order. The route never silently
reverses an effect to make a preferred direction appear.

`require_contrasts: true` turns an otherwise valid design with no one-factor
pair into a fail-closed `contrast_coverage` refusal. Leaving it false returns
the design and an explicit `no_single_factor_contrasts` finding so the caller
can decide whether the design is useful for a different question.

## Unattributable arms

`unattributable_from_baseline` identifies an arm that differs from the explicit
baseline in zero or more than one factor. A multi-factor arm is not discarded:
it remains in the arm projection and in `findings.unattributable_arms`, but it
does not become a component effect by proximity to the baseline.

This distinction matters in a factorial design. The `both-changes` arm in the
example is needed to complete the two-by-two lattice, yet its baseline contrast
is not a planner effect and not a verifier effect. Reusing it as either would
confound the attribution.

## Interaction coverage

For every pair of declared factors, the route collects the levels actually used
by the arms and asks the real `missing_for_interaction` kernel for every cell in
the observed two-by-two sub-lattice. An interaction row contains:

```json
{
  "factors": ["planner", "verifier"],
  "estimable": true,
  "missing_cells": []
}
```

If the `tree/on` cell is absent, the same row becomes:

```json
{
  "factors": ["planner", "verifier"],
  "estimable": false,
  "missing_cells": [
    { "level_a": "tree", "level_b": "on" }
  ]
}
```

The missing-cell list is actionable. It says which arm would make the question
askable; it does not impute the outcome or report an interaction estimate.
`require_complete_interactions: true` converts any missing pair into a
fail-closed `interaction_coverage` refusal. With the policy false, all holes
remain visible and `interactions.missing_count` prevents an empty returned row
set from being mistaken for complete coverage.

The route checks coverage only for levels observed in the submitted design. It
does not invent all possible levels of a factor from a registry. A factor with
ten possible levels may be intentionally studied at two levels; the caller
must provide a separate scope contract if broader level coverage is required.

## Real attribution and causal labels

Every generated one-factor contrast becomes a real `MatchedFork`, with all
other factors in `held_fixed` and the caller's `controlled` declaration carried
through. The route invokes `bioprism-evalengine::attribute` and returns its
tagged attribution or refusal, plus the kernel explanation.

When a fork is controlled, an attributed result carries a `causal` claim label
under the evaluated distribution. When it is uncontrolled, the same clean
single-factor contrast carries a `descriptive` label. The route reports this
label; it does not independently verify randomization, exchangeability,
intervention fidelity, or biological causality.

Attribution direction remains four-way: `improved`, `regressed`, `unchanged`,
or `indeterminate`. `unchanged` means comparable conclusions were the same;
`indeterminate` means at least one conclusion was unknown, disputed, vetoed, or
otherwise not orderable. They are not merged.

`require_attribution: true` fails closed at `attribution_policy` if any
generated contrast is refused. By default, refused attributions remain in the
bounded attribution rows and are counted in `refused_count` so review can
distinguish a valid design from an attributable result.

## Successful projection

Schema is `bioprism-mcp/bioeval-design-audit/0.1`.

```json
{
  "ok": true,
  "schema": "bioprism-mcp/bioeval-design-audit/0.1",
  "workflow": "bioeval_design_audit",
  "design": {
    "cell_id": "cell-7",
    "factors": ["planner", "verifier"],
    "baseline": "base",
    "arm_count": 4,
    "contrast_count": 4,
    "unattributable_arm_count": 1,
    "controlled": true,
    "valid": true
  },
  "interactions": {
    "rows": [
      {
        "factors": ["planner", "verifier"],
        "estimable": true,
        "missing_cells": []
      }
    ],
    "returned": 1,
    "total": 1,
    "omitted": 0,
    "estimable_count": 1,
    "missing_count": 0
  },
  "attributions": {
    "rows": [
      {
        "fork": {
          "fork_id": "cell-7::planner",
          "cell_id": "cell-7",
          "baseline": { "arm": "base", "conclusion": "fail", "tier": "execution" },
          "variant": { "arm": "planner-tree", "conclusion": "pass", "tier": "execution" },
          "held_fixed": ["verifier"],
          "controlled": true
        },
        "attribution": {
          "attribution": "attributed",
          "component": "planner",
          "from": "react",
          "to": "tree",
          "direction": "improved",
          "claim": "causal",
          "supporting_tier": "execution"
        },
        "refused": false,
        "causal": true
      }
    ],
    "returned": 1,
    "total": 4,
    "omitted": 3,
    "refused_count": 0,
    "causal_count": 4
  },
  "findings": {
    "unattributable_arms": { "ids": ["both-changes"], "total": 1, "omitted": 0 },
    "missing_interactions": { "ids": [], "total": 0, "omitted": 0 },
    "no_single_factor_contrasts": false,
    "attribution_refusal_count": 0
  },
  "guarantees": [
    "partial assignments and undeclared factors fail before contrast generation",
    "multi-factor baseline differences are not component effects",
    "missing interaction cells remain explicit",
    "bounded projections retain total and omitted counts"
  ],
  "limitations": [
    "no arm execution or effect estimation",
    "controlled is caller-supplied and not independently verified",
    "interaction coverage is not an interaction estimate"
  ]
}
```

The arm, contrast, interaction, attribution, and identifier projections are
bounded independently by `max_items`. The total counts remain semantic counts
over the complete validated design. A truncated `rows` list is never evidence
that a design had no holes or no refusals.

## Composition and boundaries

The design audit composes with:

- `bioeval_plane_audit`, which can keep the resulting component dimensions
  scored, unscored, or inapplicable;
- `bioeval_evaluator_audit`, which can explain an arm whose conclusion is
  unscored because its evaluator was unhealthy;
- `bioeval_metamorphic_audit`, which can test whether the declared component
  response survives irrelevant or directional transformations; and
- `bioeval_waiver_audit`, which can preserve an explicit release exception
  without rewriting a design failure into a passing gate.

It does not replace `benchmark_counterfactual_check`: that route grades one
benchmark pair against a declared invariant or must-change expectation, while
this route audits a multi-arm factorial design and produces all valid
single-factor forks. It also does not replace a statistical interaction model,
an experimental runner, or a biological interpretation layer.

## SDK surfaces

- Python exposes `BioevalDesignArmArgs`, `BioevalDesignAuditArgs`,
  `BioevalDesignAuditReport`, and `bioeval_design_audit_report(...)` through
  `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`.
- TypeScript exposes typed conclusion, tier, arm, audit-argument, and
  audit-result interfaces plus `bioevalDesignAudit(...)`.

