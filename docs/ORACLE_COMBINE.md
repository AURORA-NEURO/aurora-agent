# Oracle combination contract

`oracle_combine` is the set-valued evidence-mesh boundary. It combines serialized
`bioprism-oracle::Judgement` records at one UTC instant while preserving every input in a named
ledger. The stable MCP schema is `bioprism-mcp/oracle-combine/0.1`.

The mesh never majority-votes, averages confidence, or promotes a judge because it is certain.
The evidence ladder determines precedence: deterministic, execution, property, statistical, then
judge. Same-tier committed positions remain a set; a split is `underdetermined` and carries a
typed disagreement record with source classification, settlement route, and resolution state.

## Output layers

The top-level report exposes:

- `basis`: `decided`, `no_admissible_oracle`, `no_applicable_oracle`, or `below_policy_floor`;
- `confidence`: an observed low/high envelope over deciding judgements, never a mean;
- `contributing`: judgements at the deciding tier;
- `withheld`: admissible judgements that were below the deciding tier or out of scope;
- `inadmissible`: expired, not-yet-valid, or superseded judgements;
- `suppressed`: weaker non-abstaining positions that attempted to override the deciding tier; and
- `disagreements`: same-tier splits, with `source`, `would_be_settled_by`, and `resolution`.

Each returned judgement preserves oracle identity/version, effective and declared tier, position,
confidence, optional belief distribution, planes established/disclaimed, findings, admissibility,
and rationale. Omission counts are retained beside every bounded ledger. A row omitted for the
response limit is not a row that was absent from the mesh.

`establishes` and `does_not_establish` are plane projections, not biological truth claims. A
contradiction establishes nothing, and an underdetermined verdict establishes nothing by
construction. `acceptable` is the Rust verdict's explicit one-position acceptability projection;
it is not a probability.

## Disagreement evidence

`source` classifies the plumbing or scientific reason for a split:

- `version_mismatch` means the same oracle identity changed version;
- `scope_mismatch` means the oracles cover different planes;
- `independence_violation` means a circular/shared-input oracle is present; and
- `genuine_ambiguity` means independent, same-scope, same-version oracles genuinely differ.

The settlement list remains a route, not an action: higher-tier evidence, version alignment,
independent review, artifact repair, or a longitudinal observation. `resolution` starts `open` and
can later be `upheld`, `overturned`, or `unresolvable` in an adjudication workflow. This tool does
not perform that workflow.

## SDK boundary

Python keeps the existing `OracleCombineRequest` authoring API and adds typed nested projections to
`OracleCombineReport`: `OracleJudgementProjection`, `OracleRefProjection`,
`OracleSuppressedOverrideProjection`, `OracleDisagreementProjection`, `OracleBasisProjection`,
and `OracleConfidenceProjection`. The raw row tuples remain available for forward compatibility;
the typed records are additive views. `Workspace`, `AsyncWorkspace`, `ApiClient`, and
`AsyncApiClient` all use the same report parser.

TypeScript now types `OracleRefResult`, `OracleJudgementResult`, `OracleSuppressedOverrideResult`,
`OracleDisagreementResult`, `OracleBasisResult`, and `OracleConfidenceResult` inside
`OracleCombineResult`. The existing `oracleCombine` method and request shape are unchanged.

Neither SDK authenticates an oracle, runs an evaluator, resolves a disagreement, or infers
biological truth. Those remain explicit limitations in the Rust response and in the raw envelope.
