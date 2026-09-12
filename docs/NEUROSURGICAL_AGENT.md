# Neurosurgical research agent

`bioprism-neurosurgery` is the local, provider-neutral specialty surface added to AURORA. It
routes structured, de-identified research or education requests through read-only tools for:

- glioma and neuro-oncology;
- cranial-base surgery;
- craniosynostosis and craniofacial reconstruction;
- encephalocele and congenital skull-base anomalies;
- spina bifida and spinal dysraphism; and
- Chiari malformation and the craniocervical junction.

It uses no OpenAI API, model provider, credential, network, database, or patient-file access. A
deployment can feed its JSON output into a local model or a human workflow, but the crate itself
only inventories caller-supplied observations and evidence. The separate real-data path accepts a
versioned, de-identified snapshot generated from public authorities; it never turns population
records into a patient diagnosis or an operative recommendation.

## Run locally

From the repository root:

```powershell
cargo run -p bioprism-neurosurgery --offline -- --catalogue
cargo run -p bioprism-neurosurgery --offline -- --specialty-catalogue
Get-Content -Raw fixtures/neurosurgery/glioma_synthetic.json |
  cargo run -p bioprism-neurosurgery --offline
Get-Content -Raw fixtures/neurosurgery/glioma_synthetic.json |
  cargo run -p bioprism-neurosurgery --offline -- --audit-evidence
cargo run -p bioprism-neurosurgery --offline -- --validate-real-glioma data/neurosurgery/glioma_public_snapshot.json
'{"text":"enzastaurin","status":"completed","limit":4}' |
  cargo run -p bioprism-neurosurgery --offline -- --query-real-glioma data/neurosurgery/glioma_public_snapshot.json
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --real-glioma data/neurosurgery/glioma_public_snapshot.json

# Durable, stateless session checkpoints (the caller owns these files)
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --session-start --real-glioma data/neurosurgery/glioma_public_snapshot.json > work/neurosurgery-session.json
# Or execute the whole route in one bounded local call and retain its terminal checkpoint:
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --session-run --max-session-steps 32 --real-glioma data/neurosurgery/glioma_public_snapshot.json > work/neurosurgery-run.json
# Or use the single mission envelope (same route, with catalogue metadata and optional query):
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --mission --max-session-steps 32 --real-glioma data/neurosurgery/glioma_public_snapshot.json > work/neurosurgery-mission.json
# Revalidate a persisted mission before reuse; the request stays on stdin and snapshots are optional.
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --validate-mission work/neurosurgery-mission.json --real-glioma data/neurosurgery/glioma_public_snapshot.json
# Add --mission-query data/neurosurgery/glioma_real_query.json to bind a bounded public-record query.
# Build a deterministic, source-linked topic brief from the same snapshot (no provider or network):
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --research-brief --real-glioma data/neurosurgery/glioma_public_snapshot.json > work/glioma-research-brief.json
# Build the six-track source-grounded agenda from both validated real planes:
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --evidence-program `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json > work/glioma-evidence-program.json
# Repeat the next two commands until the checkpoint status is awaiting_human_review.
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --session-advance work/neurosurgery-session.json --real-glioma data/neurosurgery/glioma_public_snapshot.json > work/neurosurgery-session-2.json
Move-Item -Force work/neurosurgery-session-2.json work/neurosurgery-session.json
# Once status is awaiting_human_review:
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --session-finish work/neurosurgery-session.json --real-glioma data/neurosurgery/glioma_public_snapshot.json
```

The second command emits one JSON `AgentResponse`. `ready_for_human_review` means the declared
inputs are present; it does not mean that a diagnosis, prognosis, treatment, or procedure has been
validated. `needs_evidence` identifies missing, uninterpretable, or conflicting inputs. Every run
ends with a `human_review_hold`, and every tool has `effect: read_only`. The response also carries
a deterministic `response_digest` over the closed route, tool findings, evidence-gap projection,
and nested provenance summaries. Persisted responses can be checked with Rust
`AgentResponse::validate_integrity()`; `validate_for_request(...)` additionally rebinds the exact
request digest. These are structural replay checks, not clinical sufficiency or truth claims.
The standalone research brief is similarly self-validating: `brief_digest` covers its bounded topic
lanes, source links, counts, unknowns, and freshness binding, while `validate_for_inputs(...)`
rebuilds it from the exact local snapshot.
The evidence graph report follows the same rule: `graph_digest` covers the bounded node/edge
crosswalk and `validate_for_inputs(...)` replays it against the supplied glioma snapshot, while
isolated and omitted nodes remain explicit connectivity unknowns.
The shared intake audit carries `audit_digest` and `validate_for_request(...)`, so its granular
coverage states are replayed from the exact de-identified request before they feed a plan or mission.

### Natural-language intake

`neurosurgery_intake_plan` is the provider-free entry point for a short research question when a
caller does not yet know which specialty route to request. It normalizes a bounded question,
matches only explicit terms from the six closed profiles, and returns sorted candidates. Weak or
ambiguous wording abstains; an explicit `specialty` is accepted as a routing override but never
authorizes clinical use. The output carries a question digest rather than the question text, the
selected route (if any), caller-supplied evidence snapshot classes, reviewer roles, and safe next
research actions. Scores are routing units, not probabilities, severity, or clinical risk. The MCP
tool and the Python `LocalNeurosurgicalAgent.intake_plan()` and TypeScript
`LocalNeurosurgicalAgent.intakePlan()` facades share this contract. A selected glioma route names
both `real_glioma_snapshot` and `pubmed_snapshot`; the other specialties name the
`pubmed_snapshot` lane. Intake never fetches sources, invokes a model, reads patient files, or
executes downstream tools.
The closed vocabulary is subtopic-aware: it recognizes glioma histomolecular and treatment-effect
terms, skull-base compartments and cranial-nerve/CSF-leak terms, craniosynostosis suture and
syndrome terms, encephalocele variants, spinal dysraphism/tethering and neurogenic-bladder terms,
and Chiari measurements, cine-MRI, and CSF-flow terms. These matches are routing metadata only;
they are never promoted to patient findings.

`neurosurgery_intake_mission` adds the guarded composition layer (Python
`intake_mission()`, TypeScript `intakeMission()`). It first performs the same digest-only intake,
then abstains with no mission on ambiguity, returns `needs_evidence` when the required validated
snapshot is absent, or runs only a `research_synthesis` mission when evidence is present. Glioma
execution requires the real glioma snapshot; a validated PubMed snapshot is optional supplemental
context. The other five routes require the PubMed snapshot. The returned object contains the intake
plan and, when executed, a mission plus request digest—not the raw question or serialized request.
Every status remains `read_only`, `provider: "none"`, `network: false`, and
`human_review_required: true`.
An optional `case_request` field lets a caller carry a de-identified structured case (observations,
provenance, and evidence) into that mission. The core validates it before any snapshot query and
never echoes the case payload in the intake envelope. Without it, the mission deliberately uses an
empty research case and exposes the resulting observation gaps for review.
An optional `case_asset_manifest` plus `case_asset_manifest_query` carries real, de-identified
imaging/pathology/molecular/operative/functional/developmental/outcome/anatomical asset metadata
into the nested mission. The manifest is validated and digest-bound, but asset bytes are never
opened; explicit `not_collected`, `uninterpretable`, and `conflicting` states remain reviewer
obligations. The same pair is accepted by the Rust CLI (`--case-asset-manifest` and
`--case-asset-manifest-query`) alongside `--intake-mission`.
After a projection is persisted, `neurosurgery_case_asset_review_disposition` can apply a
caller-owned reviewer ledger to the emitted sequence numbers. Decisions are limited to
`reviewed`, `unresolved`, or `not_applicable`, are canonicalized and bound to the exact
`report_digest`, and cannot address omitted rows; unknown or duplicate sequences fail closed.
The ledger is workflow metadata only: it does not alter the manifest, open bytes, resolve
provenance, or create a clinical conclusion. Omitted, unresolved, and undecided rows remain
pending for the next review cycle.
The intake mission can also receive that persisted ledger directly (Python
`case_asset_review_disposition=...`, TypeScript `caseAssetReviewDisposition`, or the MCP
`case_asset_review_disposition` field). The route validates the ledger before any evidence handoff
and carries its digest/counts into synthesis, the evidence program, acquisition checkpoint, and
mission audit; DICOM/FHIR imports use the same binding when they are the asset source.
The offline CLI can replay the same ledger without a request or network call; pass the persisted
manifest report path and pipe a JSON decision array:

```powershell
Get-Content -Raw work/case-asset-decisions.json |
  cargo run -p bioprism-neurosurgery --offline -- `
    --case-asset-review-disposition work/case-asset-report.json
```

The envelope reports `ready_for_human_review` once the required snapshot has enabled execution;
the nested mission may still report `needs_evidence` when the transient question contained no
observations or other route evidence, preserving that gap for the reviewer.
For an executed question, the orchestrator derives the local real-data/PubMed text filter only
from matched closed-vocabulary terms (for example `idh` and `mgmt`), so the raw free text is not
echoed in query or brief reports. If the caller supplies only an explicit specialty hint, the
orchestrator uses that lane's canonical corpus term (for example `glioblastoma` for the glioma
lane) rather than falling back to the full question.

`neurosurgery_intake_portfolio` is the multi-lane extension. By default it preserves lexical
abstention and creates one selected-lane portfolio. With the explicit `include_all_specialties`
flag it performs corpus reconnaissance across all six independent PubMed lanes, preserving empty,
truncated, and review-queue states per lane. Any scope containing glioma also requires the
validated real glioma snapshot; the PubMed snapshot is always required. This mode may include a
single selected-lane mission, but it never synthesizes a combined specialty or clinical route.
When the portfolio is single-lane, callers may pass the same real de-identified
`case_asset_manifest`/`case_asset_manifest_query` pair and the nested mission will carry its
digest-bound projection. An explicit all-six-lane portfolio refuses that single-specialty asset
attachment so provenance cannot be assigned to the wrong lane.
The optional `case_asset_review_disposition` ledger can accompany that single-lane attachment; it
is revalidated against the projected report and carried into the nested mission's synthesis,
acquisition, and audit metadata. All-six-lane portfolios reject both the manifest and its ledger.
Both intake tools accept an optional `freshness` object with caller-supplied UTC `as_of`, bounded
`max_age_days`, and optional `source_id`. The posture is evaluated against the supplied snapshot
planes and returned in the nested report; it is never inferred from the host clock.

The Rust CLI exposes the same provider-free path for local workers. Pipe flat query JSON to
`--intake-mission` for one routed research mission, or to `--intake-portfolio` for one/all lanes:

```powershell
@'{"question":"Review glioma and Chiari evidence","include_all_specialties":true,"max_hits_per_lane":4}'@ |
  cargo run -p bioprism-neurosurgery --offline -- --intake-portfolio `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json
```

Both bundles are validated before execution. The portfolio reports six independent source-linked
lanes, keeps `synthetic_data: false`, and stops at a human-review handoff; it never merges lanes or
emits a diagnosis, treatment, triage, or procedural action. Omit a required snapshot to receive a
structured `needs_evidence` response instead of a partial run.

For unattended local refresh/review orchestration, use
`scripts/run_neurosurgical_intake_portfolio.ps1 -QueryPath <query.json>`. The worker validates the
last-known-good glioma and PubMed snapshots, refreshes public endpoints into separate candidates,
runs both refresh audits, and executes the intake portfolio against the validated candidates. It
returns one `bioprism-neurosurgery-autonomous-intake-worker/0.1` envelope containing the portfolio,
candidate paths, audit reports, and an explicit `promotion` hold. `-SkipRefresh` runs the same
worker against existing snapshots for offline/reproducible operation; no candidate is promoted.
Pass `-FreshnessQueryPath <query.json>` to carry an explicit caller-clocked source-age posture, or
`-CaseAssetManifestPath <manifest.json>` with the optional
`-CaseAssetManifestQueryPath <query.json>` for a selected-lane real de-identified asset manifest.
Use `-CaseAssetReviewDispositionPath <report.json>` to replay a persisted reviewer ledger against
that exact projection. The worker forwards these controls to the CLI and refuses a
single-specialty manifest when the
portfolio explicitly scans all six lanes. For a selected lane it also verifies that the nested
mission's `evidence_synthesis.case_asset_summary` digest/coverage and disposition digest match the
attached manifest before returning the worker envelope.

The session commands emit a checkpoint or final response on stdout; redirect each checkpoint to a
caller-owned file (or database) and pass it back with `--session-advance <path>` until its
`status` is `awaiting_human_review`, then use `--session-finish <path>`. Include
`--real-glioma <path>` on every operation when the session was started with real data. The CLI
does not retain hidden state and refuses request, bundle, route or event-chain drift.

## Real glioma data (no API key)

`data/neurosurgery/glioma_public_snapshot.json` is a checked-in, non-synthetic population snapshot
refreshed from five public authorities:

- ClinicalTrials.gov API v2: five records returned by the glioblastoma query (NCT identifiers,
  status, phase, title and registry update date, with optional study type, aggregate enrollment
  target, and intervention names preserved when the endpoint returns them);
- NCI Genomic Data Commons: the TCGA-GBM project and its aggregate released case count (617 in
  this snapshot);
- cBioPortal: seven public glioma study-catalog records with study identifiers, sequencing or
  proteogenomic scope, sample metadata fetched from each public study's sample endpoint where
  available, PMID links, and 54 molecular-profile metadata records describing available
  mutation, copy-number, expression, methylation, and other assay modalities (metadata only; no
  patient-level values). The summary preserves a deterministic modality distribution for these
  metadata rows (`COPY_NUMBER_ALTERATION=9`, `GENERIC_ASSAY=8`, `METHYLATION=3`,
  `MRNA_EXPRESSION=24`, `MUTATION_EXTENDED=6`, `PROTEIN_LEVEL=4`);
- NCI PDQ: the authoritative adult CNS tumor evidence reference; and
- PubMed (U.S. National Library of Medicine E-utilities): a bounded 20-record window of indexed glioblastoma molecular or
  genomic records with PMID, title, journal, publication date, DOI, normalized abstract text
  (bounded at 12,000 bytes), publication-type tags, and MeSH terms. Abstracts are source text for
  reviewer inspection, not generated conclusions.

Each source has an HTTPS URI, retrieval timestamp, record count, and a SHA-256 digest over the
canonical records embedded in the bundle. `RealGliomaBundle::validate` rejects missing sources,
unknown provenance links, altered hashes, non-public studies, impossible timestamps, synthetic
markers in record metadata, and `synthetic_data: true`. A real run adds the validated guideline
references to the evidence inventory and emits a `real_data` summary with the bundle digest and
source/record counts, a per-project aggregate genomic case breakdown, molecular-profile metadata
count, relationship count, and abstract-coverage
counts (all 20 literature records include an abstract; none are clipped). The summary also carries
a deterministic PMID crosswalk: in this snapshot, six cBioPortal studies link to indexed PubMed
citations, one portal study has no PMID and is not crosswalk-assessed, and 14 indexed citations are
not linked to a selected portal study. PubMed records are deliberately marked as unverified citation
metadata; a PMID link does not assert study quality, cohort identity, applicability, or a patient-
level finding.

The same summary preserves the complete registry-status distribution and newest supplied trial
update date (`RECRUITING=1`, `COMPLETED=2`, `SUSPENDED=1`, `TERMINATED=1`; latest update
`2025-03-13`), plus counts of records carrying study type, aggregate enrollment, and intervention
metadata (`5/5/5` in this snapshot). These are descriptive registry fields only: they do not
establish eligibility, trial quality, efficacy, safety, or clinical recency beyond the captured
source timestamp.

For broader glioma coverage, the checked-in
`data/neurosurgery/glioma_extended_snapshot.json` was generated by the same no-key refresh with
`-GdcProjectIds @("TCGA-GBM","TCGA-LGG")`. It keeps the GBM project (617 aggregate cases) and
adds the NCI GDC TCGA-LGG project (516 aggregate cases) as a separate provenance source. The
baseline snapshot is intentionally unchanged so existing digests and replay fixtures remain
stable; callers can pass the extended file anywhere a validated real-glioma bundle is accepted.
The extended citation plane uses the broader real PubMed query `(glioma OR glioblastoma OR
diffuse midline glioma OR oligodendroglioma OR astrocytoma) AND (molecular OR genomic OR IDH OR
MGMT OR methylation)` under source ID `pubmed_glioma_molecular`; the baseline keeps its legacy
GBM-only source ID for replay stability. Each extended GDC project also carries aggregate
file/data-type facets (somatic mutation, aligned reads, slide images, transcript fusions, and
other public modalities) without exposing files, samples, or assay values.

When a real bundle is present, the route inserts `real_data_inventory` immediately before
evidence synthesis. That read-only tool reports registry status counts, aggregate genomic cohort
size, public study/sample coverage, molecular-profile modality coverage, explicit study/profile/
publication relationship counts, and PMID linkage, with source IDs and the bundle digest in its
finding. It is an inventory operation—not a mutation, recommendation, or patient-level inference.

For targeted review, `neurosurgery_real_data_query` (or `RealGliomaBundle::query`) accepts a
bounded text and/or registry-status filter, exact registry `trial_phase` and
`trial_study_type` facets, inclusive `trial_updated_from`/`trial_updated_to` date bounds, optional
`publication_type`/`mesh_term` indexing facets, inclusive `publication_date_from`/`publication_date_to` bounds, `record_kind`, `source_id`, and `related_record_id` facets, and returns stable trial, genomic-project, portal-study,
guideline-reference, cBioPortal molecular-profile, or PubMed literature IDs with their source URI
and the same bundle digest. Hits expose explicit `related_records` edges for study↔profile and
study↔publication crosswalks. Trial text matching includes intervention names and optional
registry study type. Genomic-project hits also carry aggregate GDC file/data-type facets when the
snapshot includes them; these are availability metadata only and never file, sample, or assay
values. The exact case-insensitive `genomic_data_type` facet narrows project hits to one GDC
availability modality without fetching files. Clinical-trial hits preserve optional registry study type, aggregate
enrollment target, intervention names, phases, and last-update date; portal-study hits preserve
optional public sample counts; PubMed hits include publication date, a bounded abstract excerpt,
and indexing tags when available. Partial PubMed chronology (year-only or month-only source
dates) remains missing rather than being padded with an invented day. Missing upstream fields
remain absent rather than guessed. A
query is never a network fetch, and a missing match is reported as zero rather than
silently broadened. PMID/title/DOI/abstract matches remain metadata-only and require reviewer
verification before being used as substantive evidence.
Persisted query results expose `validate_integrity()` and exact `validate_for_inputs(...)` replay;
mission validation invokes those checks so query filters, hit identity/order, and count projections
cannot drift after a checkpoint is saved.

For a compact registry reconnaissance pass, `neurosurgery_real_data_trial_landscape` (or
`RealGliomaBundle::trial_landscape`, `LocalNeurosurgicalAgent.real_data_trial_landscape()`, /
`realDataTrialLandscape()`) aggregates only the ClinicalTrials.gov metadata already present in
the validated snapshot. It reports deterministic status, multi-label phase, study-design, and
intervention buckets, update-date range, source IDs, and explicit missingness/truncation review
reasons. Phase bucket totals are kept separate from the count of trials carrying phase metadata,
so multi-phase rows cannot be mistaken for trial counts. The nested query supports exact phase or
study-type facets and inclusive update-date bounds; `max_interventions` and query limits are
bounded. The report is digest-bound and replayable, performs no network fetch or provider call,
never ranks trials or infers eligibility/efficacy/safety/outcomes, and remains held for human
review.

For the molecular plane, `neurosurgery_real_data_molecular_coverage` (or
`RealGliomaBundle::molecular_coverage`, `LocalNeurosurgicalAgent.real_data_molecular_coverage()` /
`realDataMolecularCoverage()`) builds a digest-bound cBioPortal availability ledger from the
validated snapshot. Exact alteration-type and datatype facets, per-study profile counts,
analysis-visible/patient-level metadata flags, description coverage, explicit missing
alteration/datatype counts, aggregate GDC project file/data-type facets, source IDs, and
row/study/facet truncation or missing-facet reasons are returned as metadata. The
ledger never exposes mutation or expression values,
sample identifiers, or an inferred molecular result; missing descriptions and bounded rows remain
human-review obligations. The canonical real-data evidence packet includes this ledger
automatically alongside the comparative cohort landscape, and all bounds are replayable without
a provider or network.
For comparative genomic planning, `neurosurgery_real_data_cohort_landscape` (or
`RealGliomaBundle::cohort_landscape`, `LocalNeurosurgicalAgent.real_data_cohort_landscape()` /
`realDataCohortLandscape()`) compares the source-linked TCGA/GDC projects already present in a
validated bundle. It is also included automatically in newly generated real-data packets and
missions. It reports aggregate released-case inventory and bounded per-project
file/data-type availability, retaining explicit project truncation and missing-metadata review
reasons. It is read-only and provider-free: rows are citation surfaces, counts are descriptive
planning metadata, and it never opens files, exposes samples or molecular values, merges cohorts,
claims comparability, or emits clinical guidance.

Example bounded molecular projection:

```powershell
'{"query":{"molecular_alteration_type":"mutation_extended","molecular_datatype":"maf","limit":128},"max_studies":128}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-molecular-coverage data/neurosurgery/glioma_public_snapshot.json
```

Example comparative project projection against the extended real bundle:

```powershell
'{"query":{"genomic_data_type":"Aligned Reads","limit":8},"max_projects":8}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-cohort-landscape data/neurosurgery/glioma_extended_snapshot.json
```

To query the separate genomic availability plane, use the exact GDC facet on the public-record
query (the molecular-coverage projection intentionally rejects this cross-plane field):

```powershell
'{"genomic_data_type":"Annotated Somatic Mutation","limit":16}' |
  cargo run -p bioprism-neurosurgery --offline -- --query-real-glioma data/neurosurgery/glioma_extended_snapshot.json
```

Example bounded CLI projection (stdin carries only the query; the snapshot stays local):

```powershell
'{"query":{"trial_phase":"phase2","trial_updated_from":"2023-01-01","trial_updated_to":"2024-12-31"},"max_interventions":8}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-trial-landscape data/neurosurgery/glioma_public_snapshot.json
```

For corpus-level auditability, `neurosurgery_evidence_graph` (or
`RealGliomaBundle::evidence_graph`, `LocalNeurosurgicalAgent.evidence_graph()` /
`evidenceGraph()`) projects the same snapshot into a bounded, digest-addressed crosswalk. Every
node is public record metadata with its source URI. The only edges are explicit stable-ID links
already present in the bundle: portal study↔molecular profile and portal study↔PubMed article.
There is no inferred biological relationship, causal claim, cohort merge, or patient/sample value.
Callers may provide an exact `root_record_id` (and optional `root_record_kind`) to traverse its
connected component in both directions, or omit a root for the full bounded graph. `max_nodes`
and `max_edges` are capped at 512 and 1,024; `total_*`, `omitted_*`, `truncated`, component, and
isolated-node counts make incomplete connectivity explicit. The graph digest binds the bundle,
query, emitted nodes/edges, and pre-bound totals for reproducible review.

Example root traversal (the query is read from stdin; the bundle is never fetched):

```powershell
'{"root_record_id":"24120142","root_record_kind":"literature_article","max_nodes":64,"max_edges":128}' |
  cargo run -p bioprism-neurosurgery --offline -- --evidence-graph data/neurosurgery/glioma_public_snapshot.json > work/glioma-evidence-graph.json
```

The report remains `human_review_required`, `provider: "none"`, `network: false`, and
`effect: "read_only"`; it cannot evaluate study quality, applicability, diagnosis, prognosis,
treatment, triage, eligibility, or procedure.

For a compact corpus audit, `neurosurgery_real_data_coverage` (or
`RealGliomaBundle::coverage_report`, `LocalNeurosurgicalAgent.real_data_coverage()` /
`realDataCoverage()`) reports source-declared versus observed counts, record-kind and source
facets, trial-update and literature-publication date axes, assay-modality counts, abstract
missingness, and explicit study/profile/PMID linkage gaps. Unknown dates stay missing and
inclusive year facets exclude them; retrieval timestamps are exposed for reviewer interpretation,
not converted into a freshness or evidence-quality score. The coverage digest binds the exact
bundle, query, counts, axes, and gap projection, so callers can reproduce a review against the
same real snapshot without a network call or API key.

Every real-data mission also attaches `real_data_trial_landscape` and
`real_data_molecular_coverage`. The first is a bounded ClinicalTrials.gov registry inventory; the
second is a bounded cBioPortal assay/profile availability ledger. Both replay against the exact
bundle digest, preserve truncation and missing metadata as reviewer obligations, and deliberately
avoid trial ranking, eligibility inference, patient-level assay values, or clinical conclusions.

For an explicit refresh-age posture, use `neurosurgery_real_data_freshness` (or
`RealGliomaBundle::freshness_report`, `LocalNeurosurgicalAgent.real_data_freshness()` /
`realDataFreshness()`). The query requires a caller-owned UTC `as_of` timestamp and bounds the
review policy with `max_age_days`; each source is classified as `current`, `stale`, or
`future_dated`, and any future-dated metadata makes the report `requires_review`. This is a
replayable retrieval-age audit only—not a quality, applicability, completeness, prognosis, or
clinical score—and it never uses the host clock, network, provider, or API key.

For cross-source identifier hygiene, use `neurosurgery_real_data_reconciliation` (or
`RealGliomaBundle::reconcile`, `LocalNeurosurgicalAgent.real_data_reconciliation()` /
`realDataReconciliation()`). The pass is local and deterministic: it compares portal PMIDs with
the literature window and normalizes DOI spellings to expose missing or shared identifiers. It
does not fetch, merge, repair, rank, or interpret records; findings are bounded metadata-only
review obligations and the report remains provider-free (`provider: "none"`, `network: false`).
For the CLI, pipe an optional `{"max_issues":64}` query to
`--real-data-reconciliation <snapshot>`.
The autonomous workflow also emits a provenance-stage `reconcile_identifiers` action whenever
the validated snapshot contains one of these findings, so identifier drift cannot silently pass
through to the human synthesis gate.

```powershell
'{"as_of":"2027-08-31T00:00:00Z","max_age_days":30}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-freshness `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-freshness.json
```

For refresh monitoring, `neurosurgery_real_data_diff` (or
`RealGliomaBundle::diff`, `LocalNeurosurgicalAgent.real_data_diff()` /
`realDataDiff()`) compares two independently validated snapshots. It reports bounded added,
removed, and changed public records, source-metadata changes, changed field names, and before/after
bundle digests without copying abstracts or sample values. This is a structural refresh audit, not
a freshness score, cohort merge, quality judgment, or clinical conclusion.

Example zero-diff check (the two paths may instead point to successive reviewed snapshots):

```powershell
'{}' |
  cargo run -p bioprism-neurosurgery --offline -- --diff-real-glioma `
    data/neurosurgery/glioma_public_snapshot.json `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-diff.json
```

For a single restart-safe refresh handoff, use `neurosurgery_real_data_refresh_audit` (or
`RealGliomaBundle::refresh_audit`, `LocalNeurosurgicalAgent.real_data_refresh_audit()` /
`realDataRefreshAudit()`). It compares two independently validated snapshots and composes the
structural diff, coverage, optional caller-owned freshness policy, review queue, and deterministic
research brief. The report binds both snapshot digests, states whether source and record identities
remain stable, and lists explicit reasons for refresh review. It never fetches, merges, accepts,
scores, or writes a candidate snapshot; `provider` is `none`, `network` is `false`, and human review
is required.

```powershell
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-refresh-audit `
    data/neurosurgery/glioma_public_snapshot.json `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-refresh-audit.json
```

To turn explicit snapshot gaps into caller-owned work, use
`neurosurgery_real_data_review_queue` (or `RealGliomaBundle::review_queue`,
`LocalNeurosurgicalAgent.real_data_review_queue()` / `realDataReviewQueue()`). The queue emits
stable tasks for missing portal-publication links, unlinked citations, absent or clipped abstracts,
missing registry update dates, and missing public sample counts. Each row carries its source URI,
stable record ID, structural class, reviewer roles, and `needs_human_review` status; no value is
imputed and no clinical urgency is assigned.

Example bounded queue:

```powershell
'{"max_items":32}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-review-queue `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-review-queue.json
```

Persist that queue and apply only explicit human workflow state with
`neurosurgery_real_data_review_disposition` (or
`RealDataReviewQueueReport::apply_dispositions`, `LocalNeurosurgicalAgent.real_data_review_disposition()` /
`realDataReviewDisposition()`). Each emitted task can be marked `reviewed`, `unresolved`, or
`not_applicable`; the report verifies the queue digest, sorts decisions canonically, and keeps
omitted or undecided tasks pending. These dispositions do not edit the snapshot, fill missing
metadata, or make a clinical claim.

```powershell
'[{"task_id":"real-review-missing_portal_publication_link-portal_study-gbm_tcga_gdc","disposition":"reviewed","reviewer_id":"reviewer-1"}]' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-review-disposition work/glioma-review-queue.json > work/glioma-review-disposition.json
```

For a local model or human reviewer that needs one bounded context envelope, use
`neurosurgery_real_data_evidence_packet` (or `RealGliomaBundle::evidence_packet`,
`LocalNeurosurgicalAgent.real_data_evidence_packet()` / `realDataEvidencePacket()`). It composes
the validated summary, coverage axes, explicit study/profile/PMID graph, source-linked query hits,
and open metadata-review obligations under one packet digest. Nested bounds, omissions, and
unknowns remain explicit; no cohort, biological, or clinical inference is added. The packet query
also accepts optional `freshness` (`as_of`, `max_age_days`, and `source_id`) and carries the same
digest-bound retrieval-age posture as the standalone freshness tool; when omitted, no host clock
is consulted and no freshness claim is made. Every packet also carries the canonical bounded trial
landscape over the snapshot, so registry status/phase/design/intervention metadata and its
missingness/truncation obligations arrive in the same reviewer handoff. It also carries a
canonical cBioPortal molecular-availability ledger with per-study profile/modalities, exact
alteration/datatype buckets, explicit description gaps, and boundedness. This is assay metadata
only; no mutation or expression values are exposed or inferred. Extended real bundles also expose
the exact GDC project/data-type file facets in the context header (aggregate availability only),
so a local model can distinguish available modalities from absent or uncollected molecular
results without receiving file contents or sample identifiers.
The packet also carries a canonical PMID/normalized-DOI reconciliation ledger; missing or shared
identifiers remain explicit provenance-review obligations before a local model can rely on the
crosswalk.
Persisted packets expose `validate_integrity()` and `validate_for_inputs(...)`; nested coverage,
graph, query, trial-landscape, molecular-coverage, reconciliation, queue, and freshness projections must retain one bundle digest and
exact persisted bounds before a local worker can consume the handoff.
The packet schema is `bioprism-neurosurgery-real-data-evidence-packet/0.4`; older `/0.1`, `/0.2`,
and `/0.3` packet artifacts must be regenerated because the canonical trial, molecular, and
identifier reconciliation ledgers are now part of the digest-bound handoff.

For a restart-safe autonomous pass, use `neurosurgery_real_data_autonomous_workflow` (or
`RealGliomaBundle::autonomous_workflow`, `LocalNeurosurgicalAgent.real_data_autonomous_workflow()` /
`realDataAutonomousWorkflow()`). It composes that packet into a deterministic provenance →
completeness → context wave, emits only explicit metadata tasks (crosswalks, missing dates,
abstract bounds, sample-count gaps, assay-inventory inspection, and caller-supplied freshness
policy checks), and accepts an optional
persisted review-disposition report to resume the next wave. Queue truncation, unresolved work,
and the final human-synthesis gate remain explicit; “autonomous” means orchestration only, never
clinical priority or automatic approval.
Persisted waves expose `validate_integrity()` and `validate_for_inputs(...)`, verifying packet
binding, action dependency closure, bounded truncation, open-obligation counts, and exact snapshot
replay before another worker resumes them.
The generic autonomous capability router is closed over this same inventory in both SDKs. It can
select the FHIR/DICOM import, evidence-program, autonomous-review-wave, trial-landscape, and
public-literature refresh/link/integrity queue, workbench, or portfolio capability by explicit
lexical terms, while abstaining on weak matches. Selection is metadata-only: it does not grant
source credentials, patient-file access, clinical authority, or effect execution.

For the specialist view of a caller request, use `neurosurgery_specialty_evidence_map` (or
`NeurosurgicalAgent::specialty_evidence_map`, `LocalNeurosurgicalAgent.specialty_evidence_map()` /
`specialtyEvidenceMap()`). It decomposes every supported lane into identity, spatial, functional,
and temporal dimensions and reports source/time coverage plus explicit missing, uninterpretable,
and conflicting states. This is a deterministic input inventory and reviewer handoff; it does not
read asset bytes or infer a diagnosis, prognosis, treatment, triage, or procedure.

```powershell
'{"query":{"text":"glioblastoma","limit":8},"graph":{"max_nodes":32,"max_edges":64},"review_queue":{"max_items":32}}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-evidence-packet `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-evidence-packet.json
```

The autonomous review wave can then be generated from the same snapshot. Its stdin is a
`RealDataAutonomousWorkflowQuery`; a persisted disposition report is supplied inside that query
when resuming a later wave.
If the packet is bounded below its available rows, the wave emits an
`expand_evidence_projection` completeness action and enters `needs_snapshot_expansion`; an
action queue capped below the candidate count enters the same hold so omitted actions cannot be
mistaken for a complete handoff. An explicit freshness clock emits `refresh_source_snapshot` for stale sources and keeps
future-dated timestamps as verification holds. These actions are worker instructions only: the
core never fetches or promotes a candidate snapshot.

```powershell
'{"packet":{"query":{"text":"glioblastoma","limit":8},"review_queue":{"max_items":32}},"max_actions":32}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-autonomous-workflow `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-autonomous-wave.json
```

When a caller needs a compact prompt input rather than the full packet, use
`neurosurgery_real_data_reasoning_context` (or `RealGliomaBundle::reasoning_context`,
`LocalNeurosurgicalAgent.real_data_reasoning_context()` / `realDataReasoningContext()`). It
renders a deterministic, Unicode-bounded context with digest-bound headers, source-addressable
record blocks, and an optional explicitly enabled abstract excerpt. Every included record is
returned as a citation; query and character-budget omissions are reported, and a truncated
context is never presented as a complete corpus. The context begins with a compact
`CITATION_INDEX` before verbose aggregate ledgers, so a tight character budget still leaves
exact source IDs/URIs available for grounded claims; detailed record blocks remain bounded and
may be clipped independently. The context header also carries the packet's compact registry
landscape counts, cBioPortal assay counts, and GDC project/data-type file
availability rows with review reasons, followed by bounded reviewer-owned obligation rows with
task IDs, source identity, and rationale. A local worker can therefore preserve the exact open
metadata work instead of collapsing an unresolved queue into a generic missingness count. Source
text is marked untrusted and no model is invoked by the tool.

```powershell
'{"packet":{"query":{"text":"glioblastoma","limit":8}},"max_chars":12000,"include_abstracts":true}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-reasoning-context `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-reasoning-context.json
```

The one-call `neurosurgery_mission` envelope includes the ordered source-linked research plan,
coverage audit, bounded `real_data_trial_landscape` and `real_data_molecular_coverage` inventories,
metadata review queue, bounded `real_data_evidence_packet`, default
`real_data_evidence_graph`, and default `real_data_reasoning_context` automatically whenever a
real glioma bundle is supplied. The packet and graph are bounded stable study/profile/PMID
crosswalks; the queue preserves explicit missing metadata obligations; the context is a
character-bounded, source-addressable handoff for a caller-owned local model. Public-literature
missions include the corresponding bounded PMID packet. An autonomous caller therefore receives
corpus shape, provenance links, explicit review work, and a review-gated context before it consumes
the route's research report. The trial inventory is descriptive registry metadata and the molecular
inventory is cBioPortal assay/profile availability metadata; neither ranks trials, infers
eligibility, or exposes patient-level molecular values.
The real-data context is bound to the mission's same query and optional freshness scope as its
packet, and persisted contexts provide `validate_integrity()` plus `validate_for_inputs(...)` for
exact snapshot replay. A context that fails either check must remain outside model or reviewer
handoff.
The cross-specialty PubMed packet and context expose the same integrity and exact-replay methods,
keeping glioma and congenital/craniocervical lanes on one persisted-handoff contract.
The matrix, workbench, and portfolio projections also expose digest/exact-replay checks so a
multi-lane handoff cannot drift from its per-lane query, queue, or specialty profile.
The underlying public-literature integrity audit is replayable as well, keeping missingness and
identifier-reconciliation obligations tied to the exact source snapshot.

`neurosurgery_research_brief` (or `RealGliomaBundle::research_brief`,
`LocalNeurosurgicalAgent.research_brief()` / `researchBrief()`) is the compact reconnaissance
layer between a query and a model-facing context. It deterministically assigns matching records
to specialty-specific lanes—for example integrated molecular identity, imaging, histopathology,
trials, and outcomes for glioma, or anatomy, CSF/vascular relationships, repair, and function in
the congenital/craniocervical lanes. Each returned row keeps its record kind, stable ID, source
URI, matched terms, publication/indexing metadata, and an optional bounded abstract excerpt. The
report carries a SHA-256 brief digest, bundle/request digests, source-query truncation, per-lane
truncation, cross-lane overlap, abstract counts, and explicit unknowns. Lexical membership is
not relevance ranking or fact checking; empty or truncated lanes are unresolved review obligations,
not negative findings. Bounds are `max_topics 1..24`, `max_records_per_topic 1..32`, and at most
32 caller focus terms. The brief is read-only, provider-free, network-free, population/citation
context only, and always held for human review.

### Temporal alignment

Every de-identified `Observation` may include optional `observed_at` (strict UTC
`YYYY-MM-DDTHH:MM:SSZ`) and a caller-owned `timepoint` label. The evidence-audit response nests a
`temporal_alignment` report, and the CLI exposes the same projection with `--temporal-audit`.
It preserves source IDs and input order, groups exact timestamps, counts date coverage by required
observation class, and emits explicit findings for undated records, label-only records, duplicate
timestamps, and order inversions. `complete` means only that the supplied metadata is date-complete
and chronologically ordered; it never means that a clinical trajectory, progression, response,
prognosis, diagnosis, or procedure has been established.

To keep a local model's output accountable, use `neurosurgery_real_data_draft_audit`. Submit the
same real snapshot, optional packet bounds, and structured draft claims. Each claim declares a
kind (`source_observation`, `population_summary`, `research_hypothesis`, `limitation`, or the
blocked `clinical_action`), a non-patient scope, and one or more stable record citations. The
audit composes the packet again, checks that citations are in the emitted bounded record set,
canonicalizes claim order, and returns a digest-bound per-claim result. `grounded_for_human_review`
means only that these structural checks passed; it is not semantic fact checking or clinical
validation. Patient-case scope and clinical-action posture are blocked.

For a single local-model handoff, the Python
`LocalNeurosurgicalAgent.grounded_real_data_research()` /
`grounded_public_literature_research()` and TypeScript
`LocalNeurosurgicalAgent.groundedRealDataResearch()` /
`groundedPublicLiteratureResearch()` helpers compose the appropriate reasoning context,
an explicitly approved credentialless provider call, and the matching draft audit. Register the
first-class `ollama_provider()`/`ollamaProvider()` preset to use an Ollama model without an
OpenAI key, or register another caller-owned in-memory/local provider. The helper requires
`approve_provider_call=True`, sends only the bounded source context, requires structured
`answer`/`unknowns`/`claims`, and returns the model transport plus context/bundle digests. A
provider outage is a hard failure—there is no synthetic fallback. The returned status remains
`grounded_for_human_review`; citations and research posture are checked structurally, while
semantic truth, patient findings, diagnosis, prognosis, treatment, triage, and procedures remain
outside this research-only boundary.
The Python and TypeScript bridges also enforce citation closure against the context's exact
citation allowlist: if a model cites a valid snapshot record that was omitted by the
character/query bound, the pass fails closed before the packet audit rather than presenting an
unseen source as grounded.
Set `tool_loop=True` / `toolLoop: true` to give the approved local model bounded, read-only
snapshot tools. Alongside the row search, the model can request
`neurosurgery_real_data_trial_landscape_view` (registry status/phase/study-type/intervention
counts), `neurosurgery_real_data_molecular_coverage_view` (assay/profile and GDC availability
counts), `neurosurgery_real_data_cohort_landscape_view` (comparative TCGA/GDC project availability),
or `neurosurgery_real_data_review_queue_view` (explicit metadata obligations). Each view executes only against the validated bundle, returns a compact
aggregate plus the exact rows used for citation, and records a sanitized `tool_trace`; tool-
returned citations are included in closure and the final audit uses the same caller facets without
opening a network or clinical-action capability.
The review-queue view additionally exposes only explicit missing-link, abstract, date, or
sample-count obligations as bounded human-review tasks; those rows are citation-closed and never
represent patient findings or clinical urgency.
The `neurosurgery_real_data_reconciliation_view` adds the canonical PMID/normalized-DOI
crosswalk ledger to the same loop. It exposes bounded missing/shared-identifier rows and counts
for human review, keeps each row citation-addressable, and refuses to repair, merge, fetch, or
interpret identifier relationships as biological or clinical evidence.
The same glioma loop exposes `neurosurgery_real_data_research_brief_view`, a deterministic
topic-lane projection for integrated molecular identity, genomics, imaging, histopathology, trials,
outcomes, tumor microenvironment, and treatment-effect metadata. It returns bounded exact source
rows and explicit unknowns; lexical topic membership is not relevance, evidence quality, biological
meaning, or clinical advice, and abstracts are omitted from this local-model view.
For the cross-specialty PubMed plane, `neurosurgery_public_literature_review_queue_view` exposes
the real snapshot's missing DOI/abstract/MeSH/publication-type and duplicate-identifier obligations
as specialty-scoped, citation-closed reviewer tasks. The local model may inspect these tasks, but
cannot widen the caller's lane or limit, and missing metadata is never negative evidence.
The glioma tool loop also exposes `neurosurgery_real_data_evidence_graph_view` for bounded
study/profile/PMID crosswalk traversal. Returned nodes are citation identities and edges are
explicit provenance relationships only; the model must not turn them into causal, biological,
eligibility, or treatment claims.
The loop also exposes `neurosurgery_real_data_evidence_acquisition_view`. This returns a bounded,
digest-addressed next-evidence worklist compiled from the validated real-glioma bundle and a fixed
research request. Steps are local replay queries with source, trigger, status, counts, and bounded
public references. They are reviewer-owned obligations rather than evidence conclusions: the view
does not fetch URLs, open case assets, call a provider, or authorize diagnosis, prognosis,
treatment, triage, or procedure.
The cross-specialty literature loop exposes the parallel
`neurosurgery_public_literature_evidence_acquisition_view` for a fixed PubMed specialty lane.
It returns the same bounded local replay steps with PMID-scoped source references and explicit
reviewer obligations. A planned step is not proof that a citation exists, and it cannot fetch,
interpret a patient, call a provider, or authorize a clinical action.
The real-glioma loop also exposes `neurosurgery_real_data_coverage_view`, a digest-bound inventory
of source, record-kind, temporal, assay, and explicit linkage coverage plus bounded gaps. It
preserves caller scope and omissions and remains descriptive reviewer planning metadata; it never
becomes a freshness/quality score or clinical conclusion.
The cross-specialty loop exposes `neurosurgery_public_literature_integrity_view`, which returns
bounded PubMed completeness and identifier-hygiene counts, review reasons, and exact PMID-linked
metadata issues for the fixed lane. These issues are citation-closed reviewer work, not negative
evidence or clinical findings.
Both grounded planes expose `neurosurgery_specialty_evidence_map_view` as an additional planning
surface. It reports fixed-lane identity, spatial, functional, and temporal coverage states,
missingness counters, and reviewer questions without observation values; the bridge verifies the
returned specialty against the caller lane and keeps the result provider-free, read-only, and
human-review-only.
With an explicit caller UTC `freshness` clock, each plane also exposes a bounded freshness view
(`neurosurgery_real_data_freshness_view` or `neurosurgery_public_literature_freshness_view`). It
reports source-age state and digest metadata only; absence of the clock is an explicit hold, and
the view never consults the host clock, fetches a source, or generates synthetic evidence.
The real-data search schema exposes bounded record-kind, trial status/phase/study type and date,
molecular/genomic, linked publication/MeSH, and source-ID facets. The PubMed search schema exposes
publication type, MeSH, and publication-date facets. A tool call can add a facet the caller left
open or change lexical text, while caller facets and limits remain hard ceilings; specialty is fixed
by the caller and cannot be selected by the model. Persisted traces retain facet metadata and only a
digest/byte count for search text.
The text field is optional: a facet-only call uses the current bounded question (or caller text),
then passes through the same normalizer and date/order checks.
Compact tool rows retain source-native bounded metadata—trial status/phase/study type, molecular or
genomic datatype labels, publication/MeSH labels, aggregate counts, and optional abstract excerpts—
while omitting patient-level values. Aggregate view payloads retain only counts, labels, digests,
and explicit review reasons; they are planning context, not evidence of efficacy, eligibility, or
patient state.
For the grounded helpers, credentialless HTTP is accepted only on loopback (`localhost`,
`127.0.0.1`, or `::1`); remote no-key gateways are refused before any evidence call.
The loop helpers (`grounded_real_data_research_loop()` / `grounded_public_literature_research_loop()`
and their TypeScript counterparts) add bounded autonomous query expansion. They replay the same
source context and draft audit for each pass, derive only metadata search strings from explicit
unknowns, deduplicate and cap pending work, and return a digest-bound pass ledger with a human-
review hold; they do not fetch, synthesize, or make clinical decisions.
The real-data loop accepts the same `real_data_query` facets as the canonical query tool (trial
phase/type and update bounds, molecular/genomic selectors, and publication/date bounds). The
normalized facet set is retained in the loop ledger and digest, and resume rejects facet drift
before another provider call.
The public-literature loop accepts a parallel `public_literature_query` with specialty, publication
type, MeSH term, inclusive date bounds, and limit. Follow-up passes replace only its lexical `text`
selector; all other facets remain in the context/audit query and are bound into the loop digest.
If an explicit lexical `text` facet is supplied, it controls pass one only. Later unknown-derived
passes replace that text and retain all other facets, ensuring the follow-up queue performs real
bounded searches instead of repeating the initial lexical slice.
Each persisted pass also carries a canonical digest of its claim payload; resume rejects missing or
altered claims before another local-model pass.
Pass that ledger back as `resume_from` (Python) or `resumeFrom` (TypeScript) with a larger total
pass budget to continue pending queries after restart. The SDK revalidates the schema, source
bundle, provider/model identity, and loop digest before dispatching another local-model pass.
`grounded_research_portfolio()` / `groundedResearchPortfolio()` coordinates the real glioma and
PubMed loops in one source-separated envelope. It aggregates only exact loop digests, bundle
identities, counts, and pending metadata; it never merges source planes into a clinical or causal
conclusion.
When both snapshots are supplied, the portfolio also runs the existing provider-free literature
link audit and carries its bounded exact PMID/normalized-DOI links, unmatched identifiers, and
metadata mismatches as a separate reviewer artifact. These rows do not imply cohort overlap,
causality, or clinical applicability.
The same portfolio/intake seam accepts an optional real, de-identified `case_asset_manifest` and
bounded `case_asset_manifest_query`. It invokes the authoritative case-asset projection and
retains only the manifest digest, bounded kind coverage, and reviewer obligations. Asset bytes,
direct identifiers, and clinical values are never opened or passed to the local model; the case
plane is specialty-bound and never merged into population or literature claims. The Python CLI
uses `--case-asset-manifest` and `--case-asset-manifest-query` for this attachment.

For an operator-facing process boundary, the Python package exposes the same workflow as
`aurora-agent grounded-portfolio`:

```powershell
aurora-agent grounded-portfolio `
  --mcp-command "cargo run -p bioprism-mcp --offline -- --root ." `
  --question "Which source-linked molecular and literature metadata should be reviewed for glioma?" `
  --model llama3.1 --approve-provider-call `
  --portfolio-output work/grounded-research-portfolio.json
```

The command defaults to the checked-in extended non-synthetic TCGA-GBM + TCGA-LGG glioma and
six-specialty PubMed snapshots,
uses only an Ollama loopback provider (or `--provider local` for an explicit in-memory fixture),
and has no credential argument or prompt. The output ledger is atomically replaced and bound to a
store digest; `--resume` rechecks the question/provider/model, source-plane selection, portfolio
digest, and child loop digests before continuing pending unknown-derived queries. Increase
`--max-passes` to continue after a bounded pause. Every answer remains caller-owned,
`human_review_required`, and research-only; no diagnosis, prognosis, treatment, triage,
procedure, or device instruction is produced.
Add `--tool-loop` to expose the bounded snapshot search plus trial-landscape, molecular-coverage,
cohort-landscape, and identifier-reconciliation views over the same validated snapshot. Bound it with `--max-tool-turns 1..8` and
`--max-tool-calls 1..32`; the mode and budgets are included in the loop digest, and persisted
traces retain only facet metadata, summary digests, and a search-text digest.

When current public evidence is needed, add `--refresh-real-data` and/or
`--refresh-public-literature` together with `--approve-network`. These opt-in refreshes use only
the allow-listed credentialless public endpoints, validate each candidate snapshot, and atomically
replace the selected files before the first model call. Refresh cannot be combined with `--resume`
because changing a source digest would invalidate the persisted loop; the resulting `source_refresh`
receipts include source digests and retrieval metadata, and the run remains human-review gated.

For free-text routing, use the intake-gated operator command:

```powershell
aurora-agent grounded-autopilot `
  --mcp-command "cargo run -p bioprism-mcp --offline -- --root ." `
  --question "What source-linked Chiari literature should a reviewer inspect?" `
  --specialty chiari_malformation --without-real-data `
  --model llama3.1 --approve-provider-call
```

`grounded-autopilot` invokes the deterministic `neurosurgery_intake_plan` before any model call.
Ambiguous questions abstain, while a canonical one-word specialty anchor (for example `glioma`
or `chiari`) is sufficient to select its single lane; missing routed snapshots return
`needs_evidence`, and only a ready route reaches the local model. Glioma requires the real-glioma snapshot; the other five specialties
require the PubMed snapshot. The output keeps source planes separate, carries intake and envelope
digests, and remains research-only with `human_review_required=true`. Set `--intake-output` to
write an atomic, digest-bound intake checkpoint. With `--resume`, the CLI verifies the question,
routed specialty, source paths, provider/model, controls, intake envelope, and any nested portfolio
ledger before continuing. Only a larger `--max-passes` budget is accepted; tampering or source
drift fails closed, and the store retains no credentials or patient data.
Add `--tool-loop` (with the same bounded turn/call flags) to enable the local model's snapshot-only
search tool after intake; the mode and budgets are resume-fenced.
Pass `--public-literature-query-file query.json` (or the corresponding SDK
`public_literature_query`/`publicLiteratureQuery` option) to keep a bounded specialty, publication
type, MeSH, date-range, and limit slice across every autonomous PubMed pass and resume.

The same `--refresh-public-literature --approve-network` (and optional
`--refresh-real-data`) flags are available on `grounded-autopilot`. Inline refresh is deliberately
opt-in, refuses `--resume`, and records the refreshed source digests before routing so a local model
cannot run against an unreported corpus.
The receipt is retained in the digest-bound output store and replayed on `--resume` without
re-fetching sources.

The PubMed snapshot can be refreshed without an API key from any Python-supported platform:

```powershell
aurora-agent refresh-public-literature `
  --output data/neurosurgery/neurosurgical_public_literature_snapshot.json `
  --per-specialty-limit 10 --approve-network
```

This is the only network ingestion edge for the literature plane. It calls the public NCBI
E-utilities endpoints, keeps citation metadata/abstracts bounded, computes the same canonical
source and bundle digests as the Rust validator, and atomically installs the candidate only after
all six lanes pass provenance and non-synthetic checks. A failed request leaves the existing
last-known-good snapshot untouched; the resulting corpus still requires qualified human review.
The Python CLI also exposes `refresh-real-glioma --approve-network` for the complete aggregate
population plane. It retrieves bounded public metadata from ClinicalTrials.gov, NCI GDC,
cBioPortal, NCI PDQ, and PubMed, computes Rust-compatible hashes for every source, validates the
mandatory registry/genomic/portal/guideline records, and atomically installs a candidate only
after the local contract passes. The output contains no patient rows, assay values, image bytes,
credentials, or synthetic fallback; promotion remains a reviewer-owned snapshot decision.

```powershell
'{"query":{"query":{"text":"glioblastoma","limit":8},"graph":{"max_nodes":32,"max_edges":64}},"claims":[{"claim_id":"trial-metadata","kind":"source_observation","scope":"public_record_metadata","text":"The packet contains a public registry record.","citations":[{"record_kind":"clinical_trial","record_id":"NCT00005955"}]}]}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-draft-audit `
    data/neurosurgery/glioma_public_snapshot.json > work/glioma-draft-audit.json
```

Example coverage facet:

```powershell
'{"record_kind":"clinical_trial","from_year":2020,"to_year":2025}' |
  cargo run -p bioprism-neurosurgery --offline -- --real-data-coverage data/neurosurgery/glioma_public_snapshot.json > work/glioma-coverage.json
```

The network boundary is explicit and scriptable. To refresh the snapshot without a provider key:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File scripts/refresh_glioma_public_data.ps1
```

The refresh defaults to a bounded 20-record PubMed window (`-PubMedLimit 1..50`); callers can
choose a smaller or larger window explicitly while retaining the same abstract and provenance
bounds. `-PubMedTerm` and `-PubMedSourceId` allow a caller to widen the citation vocabulary while
preserving the exact query and source identity in every record; the checked-in extended bundle
uses the glioma/histomolecular query documented above. Candidate promotion is atomic even when
the destination snapshot already exists, with its short-lived replacement backup removed after
the swap.

The script fetches only the public endpoints above, normalizes compact metadata, asks the offline
Rust CLI to compute canonical hashes, and writes the new snapshot. Review the resulting diff and
run `--validate-real-glioma` before using it. The Rust agent itself remains offline and
deterministic; a refresh is never implicit. Source IDs remain stable across retrieval dates so
refresh audits can separate content or metadata drift from true source identity changes.

For an end-to-end candidate workflow, use `scripts/run_glioma_refresh_review.ps1`. It validates the
current baseline, writes a separate candidate, executes `--real-data-refresh-audit`, and writes the
digest-bound report. It never replaces the baseline or promotes a candidate automatically. The
wrapper forwards `-GdcProjectIds`, `-PubMedTerm`, and `-PubMedSourceId` to the refresh script, so
broader population/citation scopes are audited as explicit candidate provenance rather than
silently mixed into the baseline.

## Cross-specialty public literature (no API key)

The six-domain snapshot at
`data/neurosurgery/neurosurgical_public_literature_snapshot.json` is a separate, source-linked
PubMed corpus for glioma, cranial base, craniosynostosis, encephalocele, spina bifida, and Chiari
malformation. The checked-in snapshot contains 145 records (25 glioma, 25 cranial-base, 25
craniosynostosis, 23 encephalocele, 25 spina-bifida, and 22 Chiari) with 138 abstracts. It contains
citation metadata, bounded abstracts, publication types, and MeSH terms;
it contains no patient files, synthetic cases, model-generated conclusions, or treatment rules. Each
lane is hashed independently and the Rust validator rejects altered records, unknown source links,
future retrieval timestamps, duplicate PMIDs, synthetic-fixture markers, and missing provenance.

Validate or query it locally without network access:

```powershell
cargo run -p bioprism-neurosurgery --offline -- --validate-public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json
'{"specialty":"chiari_malformation","text":"decompression","limit":8}' |
  cargo run -p bioprism-neurosurgery --offline -- --query-public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json
```

Refresh from PubMed E-utilities with the bounded script
`scripts/refresh_neurosurgical_public_literature.ps1`. The script is the only network boundary;
it retrieves a small, explicit PMID window, asks the offline Rust CLI for canonical hashes, and
writes a reviewable snapshot. The agent consumes only a caller-supplied validated file. A public
literature run attaches citations only from the requested specialty lane, labels them
`unverified`, records the bundle digest, and ends at `human_review_hold`; it never interprets an
abstract as a diagnosis, prognosis, surgical plan, or patient-level measurement.
Each lane keeps a stable source ID across retrieval dates; the retrieval timestamp and canonical
hash carry freshness/change information for review.

For a non-destructive before/after refresh, use
`scripts/run_neurosurgical_public_literature_refresh_review.ps1`. It validates the existing
six-lane baseline, writes a separate candidate, runs `neurosurgery_public_literature_refresh_audit`,
and emits a digest-bound report containing source/PMID drift, lane coverage changes, and explicit
review reasons. The candidate is never promoted or copied over the baseline; promotion is a separate
human action. An optional `-AuditQueryPath` supplies bounded matrix, freshness, and change limits.

The same bundle can drive a digest-fenced resumable session or mission. Use
`--public-literature <path>` with `--session-start`, `--session-advance`, `--session-run`,
`--session-finish`, or `--mission`; resend the identical bundle on every checkpoint operation.
The checkpoint records `public_literature_digest`, and the core refuses request, route, event, or
bundle drift before the next tool runs. Missions accept a `--mission-query` JSON document parsed
as `PublicLiteratureQuery` when this bundle is selected and return the query result alongside the
terminal run. The MCP `neurosurgery_session`/`neurosurgery_mission` tools and the Python and
TypeScript facades expose the same public-literature-backed lifecycle.
In a dual-bundle mission, keep `--mission-query` for the real-data side and use
`--mission-public-literature-query` for an independent PubMed-side filter.
Pass an optional `--mission-portfolio-query` JSON document (or the MCP `portfolio_query` field)
to a public-literature mission to attach the bounded multi-lane portfolio to the same envelope.
It retains its own snapshot-bound digest and omission counts without changing the
request-specialty session route. A glioma mission may now pass both `--real-glioma` and
`--public-literature`: the real snapshot drives the digest-bound route, the PubMed snapshot adds
independent citation context, and `literature_link_audit` exposes exact PMID/DOI matches and
unmatched records without merging cohorts or inferring biology.

For a single repeatable no-key workflow, run
`scripts/run_neurosurgical_autonomous_mission.ps1 -RequestPath <request.json>`. It refreshes the
extended real glioma (TCGA-GBM + TCGA-LGG) and six-lane PubMed snapshots through the existing
bounded public-source scripts, validates and hashes both candidates before promotion, persists the mission to
`work/neurosurgical-mission.json` (override with `-MissionOutputPath`), replay-validates that
checkpoint against the exact request and snapshots, then emits only the dual-bundle mission JSON.
Use `-SkipRefresh` to replay the last validated snapshots offline. To intentionally run the
compact GBM-only baseline, pass its path and matching `-GdcProjectIds`, `-PubMedTerm`, and
`-PubMedSourceId` values explicitly.
For a real de-identified case, add `-CaseAssetManifestPath <manifest.json>` and optionally
`-CaseAssetManifestQueryPath <query.json>`; the mission envelope then carries the digest-bound
metadata projection while the runner and Rust core leave all asset bytes unopened.

The MCP tool `neurosurgery_public_literature_query` and the Python/TypeScript
`LocalNeurosurgicalAgent.query_public_literature` facades expose the same bounded, deterministic
query. Results include a direct `record_uri` for human inspection, plus optional publication-type,
MeSH-term, and inclusive publication-date filters. `planWithPublicLiterature`/`plan_with_public_literature`
compose the route for any supported specialty. The cross-specialty bundle intentionally cannot satisfy glioma registry/profile tools;
those require the distinct `RealGliomaBundle` snapshot and its stronger population-data contract.

For a local model or reviewer handoff, `--public-literature-evidence-packet <path>` emits a
bounded packet containing the validated bundle digest, requested specialty/text/date filters, and
only matching PMID records (including direct PubMed URIs, bounded abstracts, publication types, and
MeSH terms). `--public-literature-draft-audit <path>` accepts the same nested query plus structured
claims whose citations must name PMIDs emitted by that packet. It returns
`grounded_for_human_review` only after structural citation and posture checks; it does not
fact-check abstracts, assess study quality, or interpret clinical actions. The nested packet query
also accepts optional `freshness` (`as_of`, `max_age_days`, and `source_id`) so the handoff can
carry a digest-bound current/stale/future-dated source-age posture without consulting a host clock.

For a provider-free local-model handoff, `--public-literature-reasoning-context <path>` renders the
same packet as bounded, deterministic context. Every included PMID remains source-addressable,
abstract excerpts are explicitly untrusted when requested, and character/citation omissions are
reported in the JSON envelope. The renderer never summarizes, interprets, calls a provider, fetches
the network, or accepts an OpenAI key. Public-literature missions attach this context automatically
as `public_literature_reasoning_context` alongside the digest-bound route report.

```powershell
'{"query":{"specialty":"chiari_malformation","text":"decompression","limit":8}}' |
  cargo run -p bioprism-neurosurgery --offline -- --public-literature-evidence-packet `
    data/neurosurgery/neurosurgical_public_literature_snapshot.json > work/chiari-literature-packet.json
```

```powershell
'{"packet":{"query":{"specialty":"chiari_malformation","text":"decompression","limit":8}},"max_chars":12000,"include_abstracts":true}' |
  cargo run -p bioprism-neurosurgery --offline -- --public-literature-reasoning-context `
    data/neurosurgery/neurosurgical_public_literature_snapshot.json > work/chiari-literature-context.json
```

For corpus reconnaissance, `--public-literature-matrix <path>` fans one bounded text/tag/date
query across selected lanes (or all six when `specialties` is omitted). Each lane carries its own
packet and digest; the report counts empty and truncated lanes but never merges cohorts or infers
cross-specialty biology.

`neurosurgery_public_literature_freshness` applies the same explicit-clock retrieval-age audit to
this six-specialty snapshot. It preserves stale and future-dated source states for human review;
it is not a study-quality, applicability, or clinical score.

`neurosurgery_literature_link_audit` joins the real glioma literature index to one selected
public-literature lane by exact PMID and normalized DOI only. The report keeps linked rows,
unmatched bounded windows, metadata field names, and identifier conflicts explicit (the checked-in
glioma lane has 12 exact overlaps). It never treats an identifier as cohort identity, evidence
quality, biological support, or a patient finding, and remains provider-free with mandatory human
review. Invoke it with `--literature-link-audit <real> <public>` or the
`LocalNeurosurgicalAgent.literature_link_audit()` / `literatureLinkAudit()` facades.

`neurosurgery_public_literature_integrity_audit` is the corpus gate before a matrix, brief, or
local-model handoff. It validates the snapshot, scopes one or all specialty lanes, and reports
source-linked missing DOI/abstract/publication-type/MeSH metadata plus duplicate normalized DOI
groups. Issue rows are bounded and deterministic; missingness is never treated as negative
evidence, and the audit never scores studies, repairs records, merges lanes, or emits a clinical
conclusion. Invoke it with `--public-literature-integrity-audit <public>` or
`LocalNeurosurgicalAgent.public_literature_integrity_audit()` /
`publicLiteratureIntegrityAudit()`.

`neurosurgery_public_literature_review_queue` turns those integrity issues into a bounded,
source-linked queue of `needs_human_review` tasks with stable task IDs, PMID/source URIs, titles,
lane-specific reviewer roles, and explicit reasons. It preserves omitted rows and the upstream
integrity digest, never repairs or deduplicates records, and is available through
`--public-literature-review-queue`, `LocalNeurosurgicalAgent.public_literature_review_queue()` /
`publicLiteratureReviewQueue()`. Public-literature missions attach this queue automatically.
Persisted queue reports expose `validate_integrity()` and `validate_for_inputs(...)`, keeping
task rows tied to the exact integrity audit and source snapshot.

`neurosurgery_public_literature_workbench` joins the same validated snapshot to the closed
specialty profile for every selected lane. Each lane reports identity/spatial/temporal axes,
review questions, confounders, reviewer roles, exact source and record counts, abstract and
metadata missingness, and bounded integrity-review obligations. It is a deterministic reviewer
navigation surface—not a readiness score or clinical inference—and never ranks lanes, repairs
metadata, fetches a provider, or emits diagnosis, prognosis, treatment, triage, or procedural
action. Invoke it with `--public-literature-workbench <public>` and a JSON query on stdin, or via
`LocalNeurosurgicalAgent.public_literature_workbench()` / `publicLiteratureWorkbench()`; public
literature missions attach the request-specialty workbench automatically.
Each lane also exposes non-exclusive design strata derived only from PubMed publication types,
MeSH labels, and bounded source text: human-indexed, animal/preclinical, in-vitro/cell-line,
review/synthesis, imaging/diagnostic, surgical/procedural, developmental/genetic,
outcome/follow-up, and interventional. Overlap and unclassified records remain explicit review
obligations; a stratum is corpus metadata, not a quality grade or clinical conclusion.

`neurosurgery_evidence_program` is the next review layer when a worker needs a domain-specific
agenda rather than a flat corpus scan. It expands each selected lane into six bounded protocol
tracks and searches their controlled terms against exact records in the supplied snapshot(s).
Glioma tracks cover histomolecular identity, imaging phenotype, surgery/function,
response/longitudinal endpoints, invasion/microenvironment, and translation/trial design; the
other five lanes have corresponding anatomy, development, function, repair, CSF, or outcome
tracks. Each track reports the required caller observation classes, metadata-only measured/
unmeasured/uninterpretable/conflicting coverage copied from the typed intake audit, missing
classes, provenance gaps, reviewer roles, exact source IDs/URIs, match and truncation counts, and
omitted-reference counts. Real-plane references additionally preserve optional trial study type,
aggregate enrollment, intervention names, phases/update dates, portal sample counts, and PubMed
publication dates without interpreting them. These are lexical retrieval
observations, not evidence scores or clinical claims: an empty track is unknown, and a matched
PMID or population record is never promoted to a case finding. The report is digest-bound to the
request and snapshot, uses no provider, API key, network, credential, model, patient file, or
asset bytes, and always requires human review. Mission envelopes include the request-specialty
program automatically; the Rust `NeurosurgicalAgent::evidence_program`, Python
`evidence_program()`, and TypeScript `evidenceProgram()` facades expose it directly.
When a validated de-identified case-asset manifest is supplied, tracks also expose an
observation-to-asset join: `asset_coverage` records `observed`, `present_not_observed`, or
`missing` inventory states for imaging, pathology, molecular, operative, functional,
developmental, longitudinal, and anatomical classes, with explicit missing kinds and provenance
counts. This is a metadata-only export worklist; it never opens bytes or implies clinical
sufficiency. The Python facade accepts `case_asset_manifest=` and the TypeScript facade exposes
`evidenceProgramWithCaseAssets()`.
Every track also emits a bounded `review_worklist` with explicit observation/provenance and asset
class obligations, so an autonomous local worker can sequence metadata checks while preserving
the human-review boundary.
The report now validates its canonical lane/track protocol, coverage and count projections,
reference bounds, freshness bindings, provider boundary, and digest. Mission audits replay the
program against the exact request, snapshots, asset manifest, and optional disposition ledger;
query or source drift therefore fails closed instead of being hidden behind a recomputed hash.
Passing the persisted `case_asset_review_disposition` ledger carries the same reviewer state into
the program and acquisition plan/checkpoint; report-digest and count mismatches fail closed.
Persisted asset projections can be checked with `CaseAssetManifestReport::validate_integrity()`
and `validate_for_request(...)`; synthesis, evidence-program, and acquisition joins perform the
same guard and fail closed on report tampering or request drift. This verifies assembly integrity,
not the clinical truth of an upstream asset.
For an all-lane local pass, the checked-in query template
`data/neurosurgery/evidence_program_query.json` can be supplied with
`--evidence-program-query` alongside the six-specialty PubMed snapshot.
Every composed mission also carries `mission_audit`, a final digest/boundary fuse that checks
request identity, snapshot identity, required report-plane presence, asset-to-synthesis and
asset-to-evidence-program bindings, and the provider-free human-review contract. `integrity_ok`
is a provenance/assembly invariant,
not an evidence-quality, clinical-readiness, or treatment decision.

For a bounded autonomous pass across several lanes, use
`neurosurgery_public_literature_portfolio`. It runs one exact metadata query, the same workbench
coverage projection, and one reviewer queue per selected specialty (all six lanes by default),
then binds the combined handoff to the snapshot digest. Results retain real PMID/source links,
truncation and omission counts, and explicit human-review posture; they never rank evidence,
repair metadata, or emit diagnosis, prognosis, treatment, triage, or procedural action. The pass
is stateless and provider-free (`provider: none`, `network: false`, `synthetic_data: false`): it
does not fetch URLs, use credentials, retain patient files, notify anyone, or write durable state.
Invoke it with `--public-literature-portfolio <public>` and JSON on stdin, or via
`LocalNeurosurgicalAgent.public_literature_portfolio()` / `publicLiteraturePortfolio()`.

For a restart-safe before/after refresh handoff, use
`neurosurgery_public_literature_refresh_audit` (or
`PublicLiteratureBundle::refresh_audit`,
`LocalNeurosurgicalAgent.public_literature_refresh_audit()` /
`publicLiteratureRefreshAudit()`). It validates both snapshots, reports bounded source and PMID
identity changes by field name, recomputes the six-lane matrix from the candidate, and optionally
attaches the caller's freshness clock. Stable IDs, empty lanes, truncation, missing abstracts, and
freshness states remain explicit. This is a structural review report—not a corpus merge, evidence
quality score, biological inference, or clinical recommendation—and it never fetches, accepts, or
promotes the candidate.

```powershell
'{}' |
  cargo run -p bioprism-neurosurgery --offline -- --public-literature-refresh-audit `
    data/neurosurgery/neurosurgical_public_literature_snapshot.json `
    data/neurosurgery/neurosurgical_public_literature_snapshot.json > work/neurosurgical-refresh-audit.json
```

```powershell
'{"specialties":["glioma","chiari_malformation"],"query":{"text":"molecular","limit":4}}' |
  cargo run -p bioprism-neurosurgery --offline -- --public-literature-matrix `
    data/neurosurgery/neurosurgical_public_literature_snapshot.json > work/neurosurgical-matrix.json
```

## Request contract

The request contains a specialty, one of the three permitted uses (`research_synthesis`,
`synthetic_case_simulation`, or `educational_review`), a question, optional typed observations,
and provenance-bearing evidence records. Direct-identifier fields and clinical-use values are
refused before any domain tool runs. `not_collected`, `uninterpretable`, and `conflicting`
observation states remain distinct from `observed`; the agent never turns an absence into a
negative finding.

The response includes:

- a SHA-256 request digest for deterministic replay;
- a specialty-specific tool plan and the tool run statuses;
- a specialty profile with identity, spatial, temporal, evidence-question, confounder and human-review axes;
- explicit evidence gaps and research hypotheses (never clinical diagnoses);
- known inputs, uncertainties, and next research questions; and
- an ordered `research_worklist` whose items distinguish missing caller evidence from
  uninterpretable/conflicting evidence and carry required observation kinds plus reviewer roles;
- a permanent non-clinical-use notice and prohibited-action list.

The worklist is a deterministic handoff for a caller-owned research workflow. It does not schedule
acquisition, read a patient system, select a test, or recommend an intervention.

For a pre-route intake view, call `neurosurgery_evidence_audit` (or
`LocalNeurosurgicalAgent.audit_evidence()` / `auditEvidence()`). It returns a digest-bound matrix
for the specialty's research observation classes, with separate measured, unmeasured,
uninterpretable, conflicting, and provenance-gap counts, evidence-tier counts, reviewer roles,
and next research questions. `coverage_complete` means only that the declared intake classes are
observed and source-labelled; it is never clinical sufficiency.

For a domain-wide specialist coverage view, call `neurosurgery_specialty_evidence_map` (or
`LocalNeurosurgicalAgent.specialty_evidence_map()` / `specialtyEvidenceMap()`) or run the CLI with
`--specialty-evidence-map`. The map keeps identity, spatial/anatomic, functional/intervention,
and longitudinal dimensions explicit for all six lanes and reports source IDs, timestamp coverage,
missing provenance, and reviewer questions. It is a metadata inventory only: no asset bytes,
clinical interpretation, diagnosis, prognosis, treatment, or procedure is produced.

For a multi-plane evidence handoff, call `neurosurgery_evidence_synthesis` (or
`LocalNeurosurgicalAgent.evidence_synthesis()` / `evidenceSynthesis()`). The synthesis ledger
keeps four planes distinct: redacted case observations, caller-supplied evidence, validated real
glioma population records, and validated six-specialty PubMed records. It returns stable source
and record identifiers, capability-level counts, explicit missingness/truncation obligations,
and (when a freshness policy is supplied) the source freshness reports used for review. When both
public bundles are supplied, exact PMID correspondences are retained as links with match kinds;
they are never interpreted as cohort overlap, biological evidence, or patient applicability.
`max_references` and source-query limits are enforced before the digest is computed, and raw case
labels/values are not copied into temporal audit rows. The report is always provider-free,
network-free, read-only, and `human_review_required: true`; attached public bundles must be
non-synthetic. `synthetic_data` is false for real/educational research requests and is true only
when the caller explicitly labels the case `synthetic_case_simulation`.

Example CLI handoff using the checked-in public bundles:

```powershell
Get-Content -Raw work/evidence_synthesis_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --evidence-synthesis `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json `
    --evidence-synthesis-query work/evidence_synthesis_query.json > work/evidence-synthesis.json
```

The Python and TypeScript facades pass the same optional bundles and query object through MCP.
They expose the report as typed dictionaries/interfaces, so a caller-owned local model or
qualified reviewer can inspect the source-addressable ledger without an OpenAI key or network
provider. The ledger does not draft a diagnosis, prognosis, treatment, triage, or procedure.
Mission helpers include the same `evidence_synthesis` report automatically (single-plane for a
one-bundle mission and dual-plane with exact links when both glioma and PubMed snapshots are
attached), so a long-running worker does not need a second unguarded join step.
Persisted synthesis reports expose `validate_integrity()` for structural envelope checks and
`validate_for_inputs(...)` for exact replay against the request, real/public snapshots, and any
case-asset review state. Mission audits invoke the replay validator, so nested molecular maps,
freshness reports, reference caps, lane projections, and reviewer dispositions cannot drift while
retaining an otherwise plausible top-level digest.

For the next bounded handoff, call `neurosurgery_research_plan` (or
`LocalNeurosurgicalAgent.plan_research()` / `planResearch()`). The planner turns every explicit
intake gap into a caller-owned task: acquire a missing observation, repair provenance, resolve an
uninterpretable or conflicting state, review an evidence corpus, or review population context. If
the caller supplies the validated glioma population snapshot or six-specialty PubMed snapshot,
the planner also runs bounded local queries and attaches source IDs and human-inspectable URIs.
Those references remain population/citation context; they are never promoted to patient evidence.
If a lexical observation keyword has no local hit, the report makes that recovery explicit by
omitting `text` and retrying only within the same specialty/record-kind facet; a bounded scan is
never presented as an exhaustive search of the literature.
Both task count and references per task are bounded (`1..64` and `1..16`), the report carries the
request and bundle digests plus a `plan_digest` over the complete worklist, and
`human_review_required` remains true. Persisted plans expose `validate_integrity()` and
`validate_for_inputs(...)`; mission audits replay those bounds and source queries against the
exact request and snapshot. The planner performs no
network access, model invocation, credential handling, file writes, diagnosis, prognosis,
treatment selection, triage, or procedure/device action.

Example CLI handoff:

```powershell
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --research-plan `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --research-plan-max-tasks 12 --research-plan-max-references 4 > work/glioma-research-plan.json
```

The Python and TypeScript facades expose the same arguments and preserve the Rust report as the
source of truth. A planner report is a research queue for a qualified reviewer, not an autonomous
clinical actor and not a substitute for source verification.

For a bounded autonomous acquisition wave, add `--autonomous-acquisition` to the research-plan
route. This permits both validated evidence planes and emits a deterministic, digest-bound local
worklist; optionally pass `--autonomous-acquisition-query <query.json>` with `max_steps`,
`max_references_per_step`, and an explicit freshness policy:

```powershell
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --research-plan --autonomous-acquisition `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json `
    --autonomous-acquisition-query work/evidence_acquisition_query.json > work/evidence-acquisition.json
```

The report is a caller-owned replay queue, not a fetch scheduler: no network/provider/credential
access or case-asset bytes are available. A zero-match step is labeled `no_local_matches` and is
never treated as negative evidence; missing snapshots appear in `required_sources`.

The same tool is a resumable local worker when `operation` is set. `start` returns a plan plus a
caller-persisted session whose event chain is content-addressed; `advance` replays at most 16
already-planned steps against the exact validated snapshots and returns the next checkpoint;
`finish` refuses incomplete steps or missing required source planes and returns only a metadata
execution report. The checkpoint is safe to store as JSON, but it is not durable server state and
does not authorize a clinical action. A changed request, query, or snapshot digest fails closed.
The acquisition lifecycle also accepts the optional `case_asset_manifest` and
`case_asset_manifest_query` pair. It carries only the projected manifest digest and bounded
review items into the plan/session; every start, advance, and finish re-validates that digest and
request binding. This keeps imaging, pathology, molecular, operative, functional, developmental,
outcome, and anatomy provenance obligations visible to a local worker without opening asset
bytes. The projection is available through MCP, Python, TypeScript, and the CLI flags
`--case-asset-manifest` / `--case-asset-manifest-query` with `--autonomous-acquisition`.

The offline CLI exposes the same lifecycle without an API key:

```powershell
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --research-plan --autonomous-acquisition `
    --autonomous-acquisition-operation start `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json > work/acquisition-session-start.json

# Persist only the `session` object from the start response, then replay a bounded wave:
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --research-plan --autonomous-acquisition `
    --autonomous-acquisition-operation advance --autonomous-acquisition-session work/session.json `
    --autonomous-acquisition-max-steps 4 `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json
```

Python uses `evidence_acquisition_start()`, `evidence_acquisition_advance()`, and
`evidence_acquisition_finish()`; TypeScript uses the corresponding camel-case methods. Each
surface returns the same schema and remains read-only and held for human review.
For a repeatable local run, [`scripts/run_neurosurgical_acquisition_worker.ps1`](../scripts/run_neurosurgical_acquisition_worker.ps1)
drives the same start/advance/finish loop, persists a UTF-8 checkpoint without credentials, and
writes only the bounded JSON reports requested by the caller.
Use `-CaseAssetManifestPath` (and optionally `-CaseAssetManifestQueryPath` plus
`-CaseAssetReviewDispositionPath`) to keep a real, de-identified multimodal review projection
and its persisted reviewer state attached to every acquisition checkpoint.

For a source-linked reconnaissance handoff before drafting, call `neurosurgery_research_brief`
with exactly one validated `real_glioma_data` or `public_literature` bundle. The optional query
supports the corresponding bounded source query, `focus_terms`, lane/record limits,
`include_abstracts`, and an explicit freshness policy. The same report is included in mission
responses, so a provider-free worker can inspect topic coverage and unknowns before passing any
context to a caller-owned local model.

### Typed glioma molecular coverage

Glioma requests may include `glioma_molecular`, a bounded
`bioprism-neurosurgery-glioma-molecular/0.1` panel. Each marker is declared at most once and
uses an explicit `present`, `absent`, `not_collected`, `uninterpretable`, or `conflicting` state.
The current vocabulary keeps IDH1/IDH2, 1p/19q, H3 K27/G34, MGMT, TERT, EGFR, chromosome 7
gain/10 loss, CDKN2A/B, ATRX, TP53, PTEN, BRAF V600E, NTRK, mismatch repair, methylation
classifier, and tumour mutational burden separate. Present/absent calls are counted as assay
calls, while missing assay, specimen, or caller-owned provenance fields become research gaps and
prevent the molecular tool from claiming complete coverage. A panel digest and marker-by-marker
status are returned in `glioma_molecular`, alongside measured and provenance-complete counts.
The panel supplies molecular evidence coverage only: it cannot produce a tumour class, grade,
prognosis, treatment, or operative action, and the route still requires independent histology and
human review.

For marker-level grounding, call `neurosurgery_glioma_molecular_map` (Rust
`NeurosurgicalAgent::glioma_molecular_map`, Python `agent.glioma_molecular_map()`, or TypeScript
`agent.gliomaMolecularMap()`). It searches only caller-supplied validated real-glioma and/or
six-specialty PubMed snapshots using controlled aliases for each requested marker. Each row keeps
the typed caller state, exact source reference IDs, match/truncation metadata, and review
obligations. A zero-hit search is explicitly not negative evidence; population/citation matches
never impute a case value. The report is digest-bound, provider-free, network-free, and always
held for qualified human review.
Its integrity validator recomputes the report digest, checks canonical marker/search-term rows,
source-plane references, freshness bindings, and bounded review metadata. Mission audits rebuild
the map from the exact caller request and supplied snapshots, so a well-formed map rebound to a
different case or snapshot fails closed before handoff.

### Real multimodal case-asset manifest

The `neurosurgery_case_asset_manifest` tool is the safe intake seam for a real, de-identified
multimodal case. Rust `NeurosurgicalAgent::case_asset_manifest`, Python
`LocalNeurosurgicalAgent.case_asset_manifest()`, TypeScript
`LocalNeurosurgicalAgent.caseAssetManifest()`, and the CLI
`--case-asset-manifest <manifest.json>` all share the
`bioprism-neurosurgery-case-asset-manifest/0.1` contract. A manifest may name the existence of
imaging series, pathology reports, molecular assays, operative notes, neurofunctional or
developmental assessments, longitudinal outcomes, and anatomical models. Each entry carries only
de-identified metadata plus a required explicit state, a SHA-256 digest, source kind, optional
modality/body region, and observation timepoint. The implementation never opens or parses asset bytes and never echoes
caller asset/source IDs; it emits digest-only references, per-kind coverage, explicit missingness,
provenance/timepoint gaps, and bounded reviewer tasks.

The manifest must declare `synthetic_data: false`, match the request specialty, contain no direct
identifiers, and use unique IDs and lowercase 64-character SHA-256 values. Synthetic, malformed,
duplicate, specialty-drifted, or over-bounded manifests are refused before projection. This is a
real-data provenance/intake capability, not an imaging, pathology, genomics, EHR, diagnostic,
treatment, or operative parser. It is read-only, provider-free, network-free, and always held for
qualified human review.
When used together with `--mission`, `--intake-mission`, or a selected-lane `--intake-portfolio`,
the same flag attaches this projection to the mission envelope; it is not a second evidence or
session route. An all-specialty intake portfolio refuses a single-specialty manifest.

Example offline projection (the request remains on stdin):

```powershell
Get-Content -Raw work/case-request.json |
  cargo run -p bioprism-neurosurgery --offline -- --case-asset-manifest work/case-asset-manifest.json `
    --case-asset-manifest-query work/case-asset-manifest-query.json
```

For systems that already export FHIR, `neurosurgery_case_fhir_import` and
`NeurosurgicalAgent::case_fhir_import` accept a sanitized FHIR `Bundle` through the
`bioprism-neurosurgery-case-fhir-import/0.1` contract. The CLI equivalent is
`--case-fhir-import <import.json>` with the research `CaseRequest` on stdin. The import is
metadata-only: every resource must have a de-identified `resourceType` and `id`, a matching
`resource_hints[]` row with an explicit asset kind, status, and provenance (the caller-defined
asset-kind extension can corroborate that mapping), and no identifiers,
patient references, narratives, codes, measurements, or free text. At most 256 resources and 2 MiB
of serialized JSON are accepted. Unclassified resources are retained as hashed reviewer tasks;
they are never guessed into a clinical class. The source Bundle is not returned, all references are
hashed, and the report is provider-free, network-free, read-only, non-synthetic, and replayable
against the exact request, Bundle, and hints. This is a safe metadata handoff—not a FHIR/EHR
content, imaging, pathology, genomics, diagnostic, treatment, or operative parser.

For imaging archives that export standard DICOM JSON, `neurosurgery_case_dicom_import` and
`NeurosurgicalAgent::case_dicom_import` accept the
`bioprism-neurosurgery-case-dicom-import/0.1` contract. The CLI equivalent is
`--case-dicom-import <import.json>` with the research `CaseRequest` on stdin. The importer accepts
one dataset object or a bounded array, projects only digest-bound series metadata (Modality,
BodyPartExamined, Study/Series/SOPInstanceUID references, dates, descriptions, and SeriesNumber),
refuses known patient-identifying tags and `PixelData`, ignores private/unknown tags, and never
opens DICOM bytes or interprets images. Missing SeriesInstanceUID can be projected only with an
explicit index fallback and remains a review obligation; missing modality, body region, dates, and
object-byte SHA-256 digests are likewise surfaced. The report is de-identified, non-synthetic,
provider-free, network-free, read-only, human-review gated, and replayable against the exact
request and metadata export.

Example DICOM metadata import:

```powershell
Get-Content -Raw work/case-request.json |
  cargo run -p bioprism-neurosurgery --offline -- --case-dicom-import work/dicom-metadata.json
```

For a single auditable handoff, `neurosurgery_case_dicom_evidence_workflow` composes that
metadata-only DICOM projection with the validated real glioma snapshot (and optional PubMed
snapshot), evidence synthesis, the six-track review program, and a resumable acquisition
checkpoint. The CLI equivalent is `--case-dicom-evidence-workflow <import.json>` with
`--real-glioma <snapshot.json>` and/or `--public-literature <snapshot.json>`; an optional
`--case-dicom-evidence-workflow-query <query.json>` supplies bounded query/reference limits.
The report binds the DICOM manifest digest into every nested worker, preserves population and
case planes separately, and remains `provider: "none"`, `network: false`, `effect: "read_only"`,
`synthetic_data: false`, and `human_review_required: true`. It is an autonomous research/review
worklist—not image interpretation, diagnosis, prognosis, treatment, triage, or an operative plan.
For a mission-level glioma dossier, `neurosurgery_mission` accepts `case_dicom_import` (the CLI
equivalent is `--mission-case-dicom <import.json>` with `--mission --real-glioma`). The imported
receipt is carried in the mission and its manifest digest is verified across synthesis, evidence
programming, and acquisition. This convenience lane is real-glioma-only and can be composed with
a sanitized FHIR import for one multimodal digest-only manifest, but not with a second asset
manifest or disposition.
For repeatable local execution, `scripts/run_neurosurgical_mission_with_dicom.ps1` validates the
mission schema, DICOM/manifest/synthesis digest bindings, provider-free boundary, and zero audit
failures before writing the persisted envelope.
Set `query.real_data_reasoning_context` and/or `query.public_literature_reasoning_context` to
attach bounded source-addressable context for a caller-owned local model or reviewer. Each context
report is independently digest-validated and bound to the same public bundle digest; abstract text
is included only when its nested query explicitly requests it and remains untrusted source text.

Example real glioma composition:

```powershell
Get-Content -Raw data/neurosurgery/glioma_real_request.json |
  cargo run -p bioprism-neurosurgery --offline -- `
    --case-dicom-evidence-workflow fixtures/neurosurgery/dicom_metadata.json `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --case-dicom-evidence-workflow-query data/neurosurgery/dicom_evidence_workflow_query.json > work/dicom-evidence-workflow.json
```

For a repeatable caller-owned worker wrapper, use
`scripts/run_neurosurgical_dicom_evidence_workflow.ps1`. It validates input paths, runs the
offline CLI, checks the provider-free/non-synthetic/human-review envelope, and writes only the
requested report path. It never promotes snapshots, stores credentials, or opens the DICOM
payload.

Example import (the import file is caller-owned and must be de-identified before handoff):

```powershell
Get-Content -Raw work/case-request.json |
  cargo run -p bioprism-neurosurgery --offline -- --case-fhir-import work/sanitized-fhir-import.json
```

Example offline CLI run:

```powershell
Get-Content -Raw work/glioma_molecular_map_request.json |
  cargo run -p bioprism-neurosurgery --offline -- --glioma-molecular-map `
    --real-glioma data/neurosurgery/glioma_public_snapshot.json `
    --public-literature data/neurosurgery/neurosurgical_public_literature_snapshot.json `
    --glioma-molecular-map-query work/glioma_molecular_map_query.json
```

Example fragment:

```json
{
  "glioma_molecular": {
    "schema_version": "bioprism-neurosurgery-glioma-molecular/0.1",
    "observations": [
      {
        "marker": "idh1_mutation",
        "state": "present",
        "assay": "research-panel-v1",
        "specimen": "tumour-baseline",
        "source_id": "caller-source-1",
        "observed_at": "2026-08-29T00:00:00Z"
      }
    ]
  }
}
```

## Resumable agent lifecycle

For a long-running local agent, use `NeurosurgicalAgent::start_session`,
`advance_session`, and `finish_session` (or the MCP `neurosurgery_session` tool with
`operation: "start"`, `"advance"`, and `"finish"`). `start` creates a deterministic route. Each
`advance` executes exactly one read-only tool and returns a JSON checkpoint containing the next
ordinal, tool status, finding digest, previous-event digest, and chained event digest. The caller
owns persistence and can resume in another process; the server stores no session state. `finish`
recomputes the report and refuses to promote a checkpoint if the request, real-data bundle, route,
event order, or finding digests differ. For a bounded autonomous worker, use
`NeurosurgicalAgent::run_session_to_review` or MCP `operation: "run"`; it executes the same
checkpoint chain in-process and returns both the final report and terminal checkpoint, with
`max_steps` capped at 256.
Checkpoint validation also binds the session identity, specialty, canonical route, event status,
and terminal state; mutating those fields cannot turn a stored checkpoint into a different
workflow.

Example MCP shape:

```json
{
  "operation": "start",
  "request": { "case_id": "...", "specialty": "glioma", "request_use": "research_synthesis", "question": "..." }
}
```

The `start` response contains the complete route and its initial chain digest. Send that returned
checkpoint as `session` for each subsequent `advance`, then send the final checkpoint to `finish`.

The lifecycle is deliberately not an autonomous clinical actor: there is no patient-file access,
diagnosis, treatment selection, triage, urgent alerting, procedure generation, device control, or
unreviewed downstream action. It is an auditable research-workflow engine whose final state is
always held for qualified human review.

Python applications can use the same MCP lifecycle without recreating Rust semantics:

```python
from prism_sdk import Client, LocalNeurosurgicalAgent

with Client(["cargo", "run", "-p", "bioprism-mcp", "--offline"], cwd=".") as client:
    agent = LocalNeurosurgicalAgent(client)
    report = agent.run_session(request, real_glioma_data=public_snapshot)
```

`LocalNeurosurgicalAgent.iter_session` yields each checkpoint for a UI, queue worker, or audit
stream. The Python facade validates only transport-shaped mappings; the Rust MCP server remains
the source of truth for specialty, evidence, digest, and safety decisions. Python workers that
want the server-side shortcut can call `run_session_to_review`, which returns the report and
terminal checkpoint together.

The dependency-free TypeScript SDK exposes the same boundary. It accepts an existing `ApiClient`
or any client implementing `tools()` and `callTool()`; no provider credential is part of the
constructor or request shape:

```ts
import { ApiClient, LocalNeurosurgicalAgent } from "@aurora-neuro/prism-sdk";

const client = new ApiClient({ baseUrl: "http://127.0.0.1:8787" });
const agent = new LocalNeurosurgicalAgent(client);
const report = await agent.runSession(request, {}, publicSnapshot, 32);
```

`runSession` drives start/advance/finish through the MCP boundary and returns only after the
Rust session reaches `awaiting_human_review`; `runSessionToReview` uses the single MCP `run`
operation and returns the report plus terminal checkpoint; `iterateSession` yields each
digest-bound checkpoint for a caller-owned queue or UI. The SDK is a transport facade, not a
second source of clinical semantics.

For an end-to-end provider-free worker, the `neurosurgery_mission` MCP tool (or
`run_research_mission`/`runResearchMission` in the Python/TypeScript SDKs) composes catalogue
discovery, an optional bounded query over the validated public bundle, and the resumable session
into one mission envelope. A glioma mission must include `glioma_public_snapshot.json` (or another
bundle that passes the same validator), so the convenience layer cannot silently run an ungrounded
or synthetic glioma workflow. The mission returns only read-only results, query hits, a terminal
checkpoint, an explicit human-review requirement, and (for a real glioma bundle) the same
deterministic source/temporal/assay/linkage coverage audit exposed by
`neurosurgery_real_data_coverage`, plus the default stable-ID graph exposed by
`neurosurgery_evidence_graph`. Every bundle-backed mission also includes the
`evidence_synthesis` ledger; dual-bundle glioma missions populate its exact cross-plane links.
It also carries an `evidence_acquisition` plan over the same bound snapshots, making the mission
itself a resumable source-query worklist rather than only a one-shot report; callers can pass that
plan to the acquisition `start`/`advance`/`finish` lifecycle without changing the evidence bytes.
The companion `evidence_acquisition_session` is the initial empty checkpoint, already bound to the
mission's request and snapshot digests, so a worker can persist it and advance immediately.
The direct `neurosurgery_evidence_synthesis` tool and both SDK facades also accept the optional
case-asset manifest/query pair; when supplied, `case_asset_report_digest` binds that projection
into the synthesis ledger without opening asset bytes. The companion `case_asset_summary` gives
bounded asset/observation/provenance counts, missing requested kinds, review-item counts, and
truncation so a caller can route review work without inspecting the underlying assets;
`case_asset_review_items` carries the bounded digest-only obligations themselves.
The same synthesis call accepts a persisted `case_asset_review_disposition` ledger. Its digest and
counts are revalidated against the exact projection before joining; synthesis exposes only the
disposition digest plus pending/resolved/unresolved counts, keeping reviewer progress resumable
without turning a disposition into a clinical conclusion.
Mission callers can pass that same ledger to carry reviewer progress through the final mission
envelope; when supplied, `mission_audit` verifies the manifest and synthesis bindings before the
worker resumes.
The ledger is also bound into the mission's evidence-program and acquisition projections, so a
resumed worker cannot silently revert to an unreviewed case-asset state.
Pass `--mission-freshness <path>` (or the MCP/SDK `freshness`
argument) with an explicit UTC `as_of` policy to carry a digest-bound source-age report into the
mission; omission leaves freshness unclaimed and never consults the host clock.
The offline CLI can attach the same projection directly to standalone synthesis:
```powershell
cargo run -p bioprism-neurosurgery --offline -- --evidence-synthesis --real-glioma <snapshot> `
  --case-asset-manifest <manifest.json> --case-asset-manifest-query <query.json>
```
For a mission replay, add `--mission-case-asset-review-disposition <report.json>` to carry a
persisted reviewer ledger through the final mission audit.

The mission also accepts `case_fhir_import` for the same caller-sanitized FHIR metadata boundary.
The CLI form is `--mission-case-fhir <import.json>` with `--mission` and a validated real-glioma
and/or public-literature snapshot; Python and TypeScript pass `case_fhir_import`/
`caseFhirImport` to their mission helpers. The FHIR receipt's digest-only manifest is rebound
through synthesis, the six-track evidence program, acquisition, and the final mission audit. This
supports glioma and the cross-specialty PubMed lanes, but never returns resource payloads,
references, identifiers, narratives, measurements, codes, or clinical values. DICOM and FHIR
imports may be supplied together; their independently validated digest-only projections are
unioned into one multimodal manifest while both child receipts remain visible. A separate asset
manifest or disposition ledger cannot be mixed into an import-backed mission.

For public-literature missions, the envelope also runs `public_literature_integrity_audit`
automatically for the requested specialty lane before the packet, brief, and local-model context
are handed off. Missing DOI/abstract/publication-type/MeSH fields and duplicate-identifier groups
remain bounded review obligations; they never become negative evidence or a clinical conclusion.
The mission also attaches a lane-scoped `public_literature_workbench` so the profile and exact
coverage posture remain visible alongside the packet and review queue.
When `portfolio_query` is supplied, the mission additionally attaches
`public_literature_portfolio`, covering each requested lane with an exact query result, workbench
coverage, and reviewer queue while retaining the provider-free, human-review-only boundary.
An optional `case_asset_manifest` (and bounded `case_asset_manifest_query`) attaches a real,
de-identified multimodal metadata projection to the same mission envelope. The projection is
digest-bound and metadata-only: the Rust core never opens imaging, pathology, molecular, note, or
other asset bytes, and missing provenance remains an explicit human-review obligation. The
`evidence_synthesis` ledger repeats the emitted asset report digest so the multimodal provenance
plane is bound into the same reviewer handoff.

The Rust core exposes the same `NeurosurgicalAgent::run_research_mission` method and the
`bioprism-neurosurgery --mission` CLI flag, keeping MCP, SDK, and offline command-line callers on
one implementation of the provenance and step-bound checks.

Persisted mission replay is a first-class local operation. Call
`NeurosurgicalMissionResult::validate_integrity()` to check the envelope, terminal response,
session event chain, provider boundary, and nested digest receipts without re-supplying data.
Call `validate_for_inputs(request, real_glioma_data, public_literature)` before reuse when the
original population inputs are available; it deterministically rebuilds `mission_audit` and rejects
changed request or snapshot bytes. For a mission carrying a DICOM or FHIR receipt, use
`validate_for_inputs_with_case_imports(...)` and pass the original sanitized metadata export too;
the CLI accepts `--mission-case-dicom <path>` or `--mission-case-fhir <path>` alongside
`--validate-mission`. The MCP `neurosurgery_mission` tool uses `operation: "validate"` with a
persisted `mission` object and optional case-import objects, while Python
`LocalNeurosurgicalAgent.validate_mission()`, TypeScript `validateMission()`, and the CLI expose
the same read-only gate. Validation is provenance/assembly integrity only and never establishes a
clinical conclusion or authorizes an action.

The crate is intentionally not an imaging or pathology parser, a literature retrieval service, a
clinical decision-support device, or an operating-room controller. Those integrations remain
deployment-owned and must preserve the boundary in the response.
