# AURORA research-contract/1.0

These JSON Schemas describe the public, versioned envelopes implemented by
`bioprism-foundation`. They are transport schemas, not scientific truth schemas: evidence state,
omissions, uncertainty, provenance and policy receipts remain mandatory parts of the wire model.
The release-review schema additionally makes a passing production verdict impossible without
complete provenance.
The research-ingestion schema binds source and normalized-ingest digests to a
conformance-verified artifact while requiring `raw_data_local: true`.
The protocol-simulation schema carries deterministic preflight outcomes, and the replication
schema carries independent-site dispositions while preserving null and contradictory results.
The quality-control schema carries modality thresholds and distinguishes blocked, warning, pass,
and unknown data states.
The research-context schema carries Decision Section and certificate identities, protected-closure
proof, sufficiency state, and unresolved-obligation counts.
The replay-audit schema carries equivalent, diverged, or invalid status with baseline/candidate
identities and the first observable difference.
The workflow-execution schema carries deterministic node order, dry-run or succeeded status,
execution identity, budget remainder, and the content-addressed artifact digest.
The evaluation-card schema carries cost-normalized metrics, Wilson uncertainty, baseline counts,
explicit omissions, and a measurement-only release verdict.

The boundary is permanently preclinical. Human-subject or clinical-source data, diagnosis,
treatment, triage, enrollment and clinical decisions are outside the product.

Rust (`foundation`), Python (`prism_sdk.research_contracts`) and TypeScript
(`research-contracts.ts`) use the same version string and field names. Consumers must preserve
unknown fields when forwarding a newer minor version and reject an unknown major version.
