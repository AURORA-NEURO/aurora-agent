# Bioeval reference audit contract

`bioeval_reference_audit` validates and projects a `ReferenceStandard` without collapsing it to a
categorical label. Its MCP schema is `bioprism-mcp/bioeval-reference-audit/0.1`.

The reference can be:

- `distribution`: named states with normalized mass and explicit dispersion attribution;
- `unresolved`: the reference applies but cannot decide, with a reason; or
- `not_evaluable`: the case is outside the reference's scope, with a reason.

For a distribution, the response preserves the mass map, derived resolution, modal state and mass,
modal confidence, Shannon entropy, queried-state mass, and dispersion. A distributed reference
cannot certify a clean categorical pass: its modal mass is a ceiling imposed by the measurement
process, not a penalty score. A missing state is not equivalent to explicit zero mass.

Dispersion remains separate from resolution. `aleatoric` means irreducible biological or
measurement variation; `annotation_error` means better adjudication could reduce the spread;
`mixed` carries an `aleatoric_fraction`; and `unattributed` refuses to invent a cause. The audit
does not score predictions or adjudicate an oracle.

Python's `BioevalReferenceAuditReport` now adds typed `BioevalReferenceProjection`,
`BioevalResolutionProjection`, and `BioevalDispersionProjection` records while retaining the raw
reference and response. Convenience predicates distinguish distributed references, actionable
categorical references, and unattributed dispersion. TypeScript types the reference union,
resolution union, dispersion labels, and stable schema on `BioevalReferenceAuditResult`.

Both SDKs remain projection layers: mass normalization, reference semantics, and clean-pass
eligibility are Rust-owned. They do not infer missing states, renormalize mass, or turn unresolved
and not-evaluable references into failures.
