# Benchmark integrity audit

`benchmark_integrity_audit` is the portfolio-level companion to
[`benchmark_decision_audit`](BENCHMARK_DECISION_AUDIT.md). It checks whether a benchmark corpus is
actually providing independent, uncontaminated, calibrated evidence. It composes the typed
`bioprism-benchcompiler` kernels and keeps every denominator separate.

## Inputs and bounds

`instances` is required and contains serialized `Instance` values. Each instance carries semantic
content, accepted verdicts, required witnesses, explicitly declared identifier strings, and an
optional caller-measured behavioural signature. The endpoint refuses duplicate `instance_id`
values; ids, titles, and descriptions are never used as evidence that two instances differ.

Optional `panel_runs` contain architecture, capability tier, and pass/fail outcomes. `known_instances`
is the denominator ledger: an id with no run is returned as `unmeasured`, not as a failed run that
would silently become a zero. `safety_vetoes` marks cases intentionally retained even when easy.

`exposure` maps ids to caller-declared `ExposureLedger` values. `probes` maps ids to `LeakProbe`
arrays. The server has no clock, network, search index, or agent runner, so it never discovers
publication and never executes probes. An absent exposure entry becomes `Unassessed`, not `Clean`.

`bench_instances` supplies the `(parent_digest, mutation_family, oracle_signature)` triples used
for effective diversity. `private_share` selects the deterministic private hash bucket (default
20%), and `rotating_panels` optionally assigns even non-private buckets to reproducible rotating
panels. `max_items` bounds each row projection from 1–1000 (default 100). Each large input family
is bounded at 100,000 entries and the combined request is bounded at 20 MB.

## Returned integrity layers

The response schema is `bioprism-mcp/benchmark-integrity-audit/0.1`.

### Deduplication

`dedup` reports exact content groups, structural groups after alpha-renaming declared identifiers,
and oracle-equivalent groups. Content and structural groups contribute to `distinct` and
`removed`; oracle-equivalent groups are review candidates and are deliberately not deleted because
different states can legitimately exercise the same contract. The report says explicitly that no
semantic similarity or entailment model ran.

### Holdout assignment

`holdout.rows` binds each id to its content fingerprint and `Public`, `Private`, or `Rotating`
assignment. `holdout.counts` is the complete denominator even when rows are truncated. Assignment
is a hash bucket, not random state, so another site can reproduce the split without receiving the
private answer set.

### Contamination

`contamination` preserves the compiler's severity order:

1. a solved leak probe is `leaks_through_channel` and blocks admissibility;
2. a searchable answer is `answer_searchable`;
3. published but unprobed is `published_and_unprobed`;
4. missing assessment is `unassessed`;
5. only an assessed, non-searchable, non-leaking record is `clean`.

`admissible` counts only `clean`. It is not a quality score and it does not imply the instance is
hard, realistic, or representative.

### Calibration

`calibration` keeps discriminating, trivial-cue, universally-passed, universally-failed,
unmeasured, and safety-veto counts separate. A weak or rule-based policy solving an instance marks a
trivial cue; a panel where everyone passes or everyone fails does not become a capability claim.
No hierarchical difficulty model or drift recalibration is fitted in this offline endpoint.

### Effective diversity

`effective_diversity.equivalence_classes` is the effective sample size: distinct parent digest,
mutation family, and oracle signature triples. `instances` and `inflation_ratio` remain visible so
raw generated volume cannot masquerade as independent evidence. `is_publishable` is intentionally
not inferred by the MCP layer; the library's conservative three-class criterion and all caveats
remain in the typed report.

## SDKs and nonclaims

Python exposes `BenchmarkIntegrityAuditArgs`, `BenchmarkIntegrityAuditReport`, and
`benchmark_integrity_audit_report(...)` through sync/async MCP, HTTP, and workspace facades.
TypeScript exposes `BenchmarkIntegrityAuditArgs`, `BenchmarkIntegrityAuditResult`, and
`client.benchmarkIntegrityAudit(...)`.

The endpoint does not run agents, discover web exposure, fit latent difficulty, infer semantic
duplicates, publish a holdout, or declare a benchmark ready for release. Every bounded projection
has an omission count, and transport success is distinct from corpus admissibility.
