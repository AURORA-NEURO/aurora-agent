# Evaluation worldline audit contract

`evaluation_worldline_audit` audits the four-clock `bioevalx::Worldline` boundary under
`bioprism-mcp/evaluation-worldline-audit/0.1`. It keeps future evidence leakage and missing
context references as separate evidence classes.

Each leak witness names the decision, observation, violating clock, decision instant, and the
instant at which the observation became accessible. The current admissibility rule is based on
`accessible`, not biological occurrence, measurement, or record time. The tool returns the numerator
of leakage witnesses and does not invent a leakage rate or denominator.

Dangling references are returned as typed `(decision, observation)` pairs. They are not counted as
leaks: an absent observation cannot prove that a decision saw future evidence, and dropping it from
the worldline must not hide a separate context-integrity defect. An optional `at` query returns the
observation ids that could have been available at that instant; omission of the query leaves the
cut explicitly absent.

Python's `EvaluationWorldlineReport` adds typed leak-witness and dangling-reference projections,
plus `accessibility_leakage_is_separate` and `admissibility_cut_is_explicit` predicates. TypeScript
types the schema, leak clock union, witness fields, pair-shaped dangling references, and optional
admissibility cut. Neither SDK reconstructs missing observations, chooses a denominator, or
silently treats an unavailable fact as an evaluation failure.
