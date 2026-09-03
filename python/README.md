# Prism Python SDK

This package is the Python integration layer above the Rust AURORA/Prism kernel. It speaks the
repository's newline-delimited JSON-RPC MCP transport using only the Python standard library.

```python
from prism_sdk import Client, Workspace

with Client(["../target/release/bioprism-mcp", "--root", ".."], cwd="python") as client:
    workspace = Workspace(client)
    report = workspace.developer_delivery_audit(
        request_id="notebook-1",
        targets=["developer_platform", "repository_scope"],
    )
    print(report["release_request"]["targets"])
```

The async client has the same lifecycle and result semantics:

```python
from prism_sdk import AsyncClient, AsyncWorkspace

async with AsyncClient(["../target/release/bioprism-mcp", "--root", ".."], cwd="python") as client:
    report = await AsyncWorkspace(client).developer_delivery_audit()
```

The SDK keeps JSON-RPC transport failures, protocol violations, server errors, and structured tool
refusals distinct. It never invokes a shell, accepts unbounded frames, turns a refusal into a
successful value, or recreates the Rust domain model. `Workspace` helpers are deliberately thin
facades over exact MCP tools; `tool()` remains available for every current and future domain.

For provider-free neurosurgical research, `LocalNeurosurgicalAgent` exposes the same Rust-backed
route without an OpenAI key or network provider. `query_public_literature()` searches the validated
six-specialty PubMed snapshot by specialty and source text, while `plan_with_public_literature()`
composes a bounded route using only the requested specialty lane as unverified citation metadata.
`public_literature_evidence_packet()` emits a digest-bound, PMID-only packet for a local model or
reviewer, and `public_literature_draft_audit()` accepts only non-patient claims that cite records
from that packet. Accepted claims are structurally `grounded_for_human_review`, not fact-checked
or clinically interpreted.

For reviewed live acquisition, `ReviewedPubMedRetrievalAdapter` adds a narrow deployment adapter
over the fixed PubMed specialty catalogue. `prepare()` is deterministic and performs no network
I/O; `execute()` requires literal `approve_source_dispatch=True`, revalidates the exact plan,
query-set digest, parser surface, and selected transport before every allow-listed E-utilities
GET, and returns a caller-owned transient bundle plus a metadata-only receipt:

```python
from prism_sdk import ReviewedPubMedRetrievalAdapter, ReviewedPubMedRetrievalConfig

adapter = ReviewedPubMedRetrievalAdapter(
    ReviewedPubMedRetrievalConfig(specialty_lanes=("glioma",), per_specialty_limit=10)
)
plan = adapter.prepare()  # pure review artifact
result = adapter.execute(plan, approve_source_dispatch=True)
print(result.receipt.to_dict())  # digests, counts, source IDs; no queries or article text
```

Registered deployments should also pass their actual paired `ncbi_tool` and developer
`ncbi_email` values to `ReviewedPubMedRetrievalConfig`. The adapter adds both identifiers to every
ESearch, ESummary, and EFetch request, but retains only a configured flag and integrity digest in
the config artifact, plan, source URI, and receipt. Supplying values does not register them with
NCBI; follow the [NCBI E-utilities usage policy](https://www.ncbi.nlm.nih.gov/books/NBK25497/).
The digest prevents silent substitution but is not anonymization of a guessable email address.

This is one bounded public-literature source, not general browsing or scientific validation.
Retrieved abstracts remain unverified source text and require independent evidence-quality and
claim review. The adapter does not provide a shared durable coordinator, uncertain-call
reconciliation, exactly-once delivery, or durable raw-bundle storage.

When an actual local model is available, `ollama_provider()` configures the Ollama
OpenAI-compatible loopback server (`http://127.0.0.1:11434/v1`) without reading or requiring an
API key. Register it with `LLMRuntime`, select an installed model, and retain the normal provider
and human-review approvals. If Ollama is unavailable, the runtime reports a provider failure; it
never substitutes a synthetic answer.
`LocalNeurosurgicalAgent.grounded_real_data_research()` is the composed local-model handoff:
it renders a digest-bound real-data context, invokes the explicitly approved credentialless
provider with a structured response contract, and sends returned claims through
`real_data_draft_audit()`. Use `approve_provider_call=True`; the result is still
`grounded_for_human_review` and does not fact-check or make patient/clinical claims. A missing
or unavailable Ollama server is a hard provider error, never a synthetic fallback.
Claims are additionally checked against the exact citation identities emitted in the bounded
model context; a citation omitted by query or character truncation fails closed before audit.
For bounded native autonomy, pass `tool_loop=True` (with `max_tool_turns` and `max_tool_calls`)
to either grounded helper. The local model may then call only credentialless snapshot tools: row
search, a ClinicalTrials.gov trial-landscape view, a molecular-coverage view for cBioPortal/GDC
availability, metadata review-queue, evidence-graph, evidence-acquisition, specialist evidence-map,
identifier-reconciliation, or (with an explicit freshness clock) freshness views. Every call executes against the caller-supplied bundle, projects bounded metadata,
and is retained as a sanitized `tool_trace` before the final citation audit. The two views return
descriptive aggregates plus the exact rows used for citation; they never expose patient-level
values or infer eligibility, efficacy, safety, or treatment.
The review-queue view exposes only explicit missing-link, abstract, date, or sample-count
obligations as bounded human-review tasks; it is citation-closed and never a clinical finding.
The real-data loop also exposes `neurosurgery_real_data_coverage_view` for source, record-kind,
temporal, assay, and explicit linkage inventory with digest-bound gaps. The public-literature
loop exposes `neurosurgery_public_literature_integrity_view` for bounded PubMed completeness and
identifier-hygiene counts/issues. Both views preserve caller scope, explicit omissions, and
human-review posture; neither scores evidence or infers patient/clinical meaning.
The real-data loop's `neurosurgery_real_data_reconciliation_view` projects canonical
PMID/normalized-DOI missing/shared rows and counts as citation-closed metadata review work; it
never repairs, merges, fetches, or interprets identifier relationships.
The loop also provides `neurosurgery_real_data_research_brief_view`, a deterministic extractor for
the fixed glioma topic lanes (molecular identity, genomics, imaging, pathology, trials, outcomes,
tumor microenvironment, and treatment effect). It returns bounded exact records and unknowns;
lexical membership is metadata-only, not evidence quality or clinical meaning, and abstracts are
omitted from the model-facing projection.
The cohort-landscape view (`neurosurgery_real_data_cohort_landscape_view`) compares source-linked
TCGA/GDC project rows already in the validated bundle. It exposes aggregate released-case and
file/data-type availability metadata with exact `genomic_project` citations, explicit truncation,
and missingness; it never opens files, exposes samples or molecular values, merges cohorts, or
makes clinical claims.
The specialist evidence-map view (`neurosurgery_specialty_evidence_map_view`) adds a bounded
identity/spatial/functional/temporal coverage ledger for the fixed lane, including explicit
missingness and reviewer questions. It never returns observation values or clinical inference and
fails closed if the returned specialty does not match the caller lane.
When `freshness={"as_of": "...Z", "max_age_days": ...}` is supplied, the loop also exposes a
caller-clocked freshness view for bounded source-age states and digest metadata. It refuses to run
without that explicit UTC clock and never fetches or substitutes synthetic data.
The search tool supports structured real-data facets (record kind, trial status/phase/study type and
date bounds, molecular/genomic selectors, linked publication/MeSH selectors, and source IDs) and
structured PubMed facets (publication type, MeSH term, and date bounds). The model may refine an
unbound facet or lexical text, but caller-provided facets and result limits remain immutable ceilings;
the selected literature specialty is never exposed as a model-controlled field.
The lexical field may be omitted for a facet-only search; it then falls back to the current bounded
question (or caller text).
Returned rows preserve bounded source metadata (trial status/phase/study type, molecular/genomic
labels, publication/MeSH labels, aggregate counts, and available abstract excerpts) without
projecting patient-level values. Existing `related_records` edges are preserved when they carry a
known record kind and relation (`published_as`, `describes_study`, `has_profile`, or
`profile_of_study`), so a reviewer can follow the real snapshot crosswalk without a new fetch.
Grounded HTTP providers are restricted to loopback (`localhost`, `127.0.0.1`, or `::1`); a
credentialless remote gateway is rejected before the evidence context is opened.
`grounded_real_data_research_loop()` and `grounded_public_literature_research_loop()` provide a
finite autonomous fan-out over those same gates: model-reported unknowns become bounded,
deduplicated metadata queries, every pass is independently audited, and the returned ledger is
digest-addressed and held for human review.
The real-data loop accepts the full structured `real_data_query` facet set (registry phase/type and
update bounds, molecular/genomic selectors, and publication/date selectors). Its normalized query
is retained in the ledger and included in the loop digest, so resuming with a different evidence
slice fails closed before another local-model call.
An explicit lexical `text` facet is honored on the initial pass; unknown-derived follow-up passes
replace only that text while retaining the registry, molecular, genomic, and provenance facets.
This keeps autonomous expansion executable without allowing it to widen the approved source slice.
Each persisted pass also carries a canonical digest of its claim payload; resume rejects missing or
altered claims before another local-model call.
The public-literature loop additionally accepts `public_literature_query` facets for specialty,
publication type, MeSH term, inclusive date bounds, and limit. Autonomous follow-ups replace only
the lexical text selector while preserving and digest-binding the remaining PubMed slice.
The provider-free capability router and domain tool profile expose the same complete neurosurgical
surface: FHIR/DICOM metadata imports, evidence programs and autonomous review waves, trial
landscapes, and public-literature refresh/link/integrity queue, workbench, and portfolio tools.
Capability selection is lexical and abstaining; it never grants credentials, patient-file access,
clinical authority, or effect execution.
Pass a prior ledger back as `resume_from=...` with a larger total `max_passes` to continue pending
queries after a restart; schema, source bundle, provider/model, and loop digest are revalidated
before any additional local-model call.
`grounded_research_portfolio()` coordinates both planes in one source-separated digest: the real
glioma population and specialty PubMed loops retain independent audit identities while counts
and pending work are aggregated for human review.
With both snapshots present it also attaches an exact PMID/normalized-DOI linkage audit, including
bounded unmatched and metadata-mismatch rows. This is a separate provenance artifact and never
asserts cohort overlap, causality, or clinical applicability.
Pass `case_asset_manifest=` with an optional `case_asset_manifest_query=` to attach a real,
de-identified multimodal inventory to the portfolio or `grounded_research_intake()`. The
authoritative projection carries only digest/coverage/reviewer metadata, is specialty-bound, and
never opens asset bytes or returns clinical values. The grounded CLI exposes the same boundary via
`--case-asset-manifest` and `--case-asset-manifest-query`; both paths remain separate from source
population and PMID claims.
The installed `aurora-agent grounded-portfolio` command is the operator-facing equivalent: it
loads the checked-in extended non-synthetic TCGA-GBM + TCGA-LGG snapshot by default (and the
six-specialty PubMed snapshot), invokes Ollama on loopback (or an explicit
`--provider local` fixture), and atomically writes a complete digest-bound ledger. It requires
`--approve-provider-call`, never accepts or prompts for an API key, refuses synthetic bundles,
and resumes only when `--resume` verifies the output store, question/provider/model identity,
source-plane selection, and child loop digests. Example:

```powershell
aurora-agent grounded-portfolio `
  --mcp-command "cargo run -p bioprism-mcp --offline -- --root ." `
  --question "Which molecular and citation metadata should a reviewer inspect for glioma?" `
  --model llama3.1 --approve-provider-call `
  --portfolio-output work/grounded-research-portfolio.json
```

Add `--tool-loop` to let the approved local model use bounded, read-only snapshot search,
coverage, and PMID/DOI reconciliation views against the same validated snapshots;
`--max-tool-turns 1..8` and `--max-tool-calls 1..32` fence the loop and
are retained in the resumable ledger. The persisted trace stores query digests and facet metadata,
not model-generated search text.

Use `--resume` with a larger `--max-passes` to continue bounded unknown-derived queries after a
restart. The command remains research-only: answers are caller-owned, source planes stay
separate, and every result carries `human_review_required=true`; no diagnosis, prognosis,
treatment, triage, or procedure/device advice is emitted.
Pass `--real-data-query-file query.json` to apply the same bounded registry, molecular, genomic,
and publication facets used by the SDK; the CLI retains that query in the digest-fenced ledger.
When current public evidence is needed, add `--refresh-real-data` and/or
`--refresh-public-literature` together with `--approve-network`. These opt-in refreshes use only
the allow-listed credentialless public endpoints, validate each candidate snapshot, and atomically
replace the selected files before the first model call. Refresh cannot be combined with `--resume`
because changing a source digest would invalidate the persisted loop; receipts are returned in
`source_refresh` and the run remains human-review gated.
For a free-text question, `aurora-agent grounded-autopilot` adds a provider-free intake gate before
the same loops. It deterministically routes the six-specialty vocabulary, returns `abstained` for
ambiguous wording, returns `needs_evidence` without a model call when the routed snapshot is absent,
and otherwise runs only the required source plane(s). Glioma requires the real glioma snapshot;
other specialties require the PubMed snapshot. The command has no API-key argument or prompt and
always returns a source-separated, human-review-gated envelope.
Use `--intake-output work/grounded-research-intake.json` to persist that envelope as an atomic,
digest-bound checkpoint. A later `--resume` verifies the store, question, route, source paths,
provider/model, and all bounded controls before handing any verified child loop ledgers back to
the intake worker; only `--max-passes` may increase. A changed or tampered checkpoint fails closed.
Add `--tool-loop` (with the same bounded turn/call flags) to enable the local model's snapshot-only
search tool after intake; the mode and budgets are resume-fenced.
The cross-platform `aurora-agent refresh-public-literature --approve-network` command refreshes
the six bounded PubMed lanes used by that intake. It uses only public NCBI E-utilities, requires
no API key, validates source hashes and `synthetic_data=false`, and atomically installs a
last-known-good replacement. Use `--per-specialty-limit 1..50` to bound retrieval; malformed or
empty lanes fail before the existing snapshot is touched.
The grounded commands can perform that refresh inline with
`--refresh-public-literature --approve-network` (and, for glioma, add
`--refresh-real-data`). Inline refresh is opt-in, refuses `--resume`, and returns source digests
and retrieval metadata in `source_refresh` so the model never runs against an unreported corpus.
The same receipt is retained in the digest-bound output store and replayed on `--resume` without
re-fetching sources.
For the full real glioma population snapshot, use the cross-platform
`aurora-agent refresh-real-glioma --approve-network` boundary. It gathers bounded aggregate
ClinicalTrials.gov, NCI GDC, cBioPortal, NCI PDQ, and PubMed metadata, computes the same Rust
source/bundle digests, and atomically installs the candidate only after validation. Repeat
`--gdc-project-id` or `--portal-study-id` to widen the public population scope; no patient-level
records, assay values, credentials, or synthetic fallback are accepted.
`grounded_public_literature_research()` provides the same composed handoff for the six-specialty
PubMed bundle, including cranial-base, craniofacial, encephalocele, spina-bifida, and Chiari lanes;
it uses `public_literature_draft_audit()` and retains PMID-scoped citations.
The underlying context now includes each bounded review obligation with its task ID, source
identity, and rationale, preserving unresolved metadata work for a resumable worker.
With `tool_loop=True`, the local model can also request
`neurosurgery_public_literature_review_queue_view` to inspect real missing DOI/abstract/MeSH/
publication-type and duplicate-identifier tasks. The view is fixed to the caller's specialty
and limit, returns only reviewer-owned queue metadata, and never treats missingness as negative
evidence.
The glioma tool loop additionally exposes `neurosurgery_real_data_evidence_graph_view` for
bounded study/profile/PMID crosswalk traversal. Nodes and edges are explicit identifiers only;
they are citation surfaces, not causal or clinical relationships.
It also exposes `neurosurgery_real_data_evidence_acquisition_view` for a bounded next-evidence
worklist. Each step is a local replay query with source metadata, match counts, and reviewer
obligations; no network fetch, patient inference, or clinical action is reachable from the view.
The public-literature loop exposes the parallel
`neurosurgery_public_literature_evidence_acquisition_view` for a fixed specialty lane. It compiles
the checked-in PubMed snapshot into the same bounded reviewer worklist with PMID-scoped references;
planned queries are not evidence, and the human-review boundary remains explicit.
`intake_plan()` is the free-text entry point: it performs bounded lexical routing across the six
specialty profiles, abstains on weak or ambiguous wording, and returns only a SHA-256 question
digest, route candidates, evidence snapshot classes, and reviewer-safe next actions. An explicit
specialty is a routing override, not clinical authorization; no provider, network, patient file,
diagnosis, or procedure capability is added.
`intake_mission()` composes this with a guarded research-only run: it abstains or reports the
missing validated snapshot before execution, requires real glioma data for glioma, and requires
the PubMed snapshot for the other specialties. Executed output is digest-only, provider-free,
network-free, read-only, and held for human review.
Pass an optional `case_request=` mapping to carry a de-identified structured case through the
same validation and route; the case is never echoed in the returned intake envelope.
Pass `case_asset_manifest=` with an optional `case_asset_manifest_query=` to carry real,
de-identified multimodal asset metadata into that mission; the report is digest-bound, explicit
about missingness, and never opens asset bytes.
Pass a persisted `case_asset_review_disposition=` ledger to resume reviewer state in the same
call. Its digest and counts are validated before handoff and rebound into synthesis, evidence
programming, acquisition, and the mission audit; it remains metadata-only.
The same `intake_mission()` call accepts `case_dicom_import=` and `case_fhir_import=` for
caller-sanitized metadata exports. They follow the same independently validated, digest-only
route, may be supplied together for one multimodal mission, and cannot be combined with a second
asset manifest.
Both intake methods accept an optional `freshness={"as_of": "...Z", "max_age_days": ...}`
policy. It is evaluated against the caller-supplied snapshot(s) and returned as a digest-bound
report; omission makes no freshness claim and the server never uses its own clock.
`intake_portfolio()` is the cross-specialty extension: it fans out matched terms across one or an
explicit all-six-lane PubMed portfolio, requires real glioma data whenever glioma is in scope, and
keeps every lane independent and reviewer-bound. A selected-lane portfolio may also carry the
same `case_asset_manifest=`/`case_asset_manifest_query=` metadata-only provenance; an explicit
all-six-lane portfolio refuses that single-specialty attachment. A selected-lane call can also
carry `case_asset_review_disposition=` so a persisted reviewer ledger remains bound through the
nested mission.
`evidence_synthesis()` is the cross-plane evidence handoff: it aligns a redacted case audit,
caller evidence, the validated real-glioma population snapshot, and/or the six-specialty PubMed
snapshot without merging their meanings. It returns source identifiers, optional freshness
reports, exact PMID links, bounded review obligations, and no patient-level conclusion; it never
needs an OpenAI key or network provider. Pass the optional `case_asset_manifest=` and
`case_asset_manifest_query=` arguments to bind a previously de-identified, metadata-only asset
projection into the ledger; the returned `case_asset_report_digest` is the explicit link, while
`case_asset_summary` provides bounded asset/observation/provenance counts, missing requested kinds,
review-item counts, and truncation without exposing asset bytes or identifiers; the optional
`case_asset_review_items` list carries the digest-only obligations themselves.
`intake_mission()`/mission responses include the same `evidence_synthesis` field automatically
when a bundle is attached, including a dual-plane linked report for a glioma mission with both
validated snapshots.
Mission envelopes also carry the bounded `evidence_acquisition` plan over those same snapshots,
which can be resumed with the explicit `evidence_acquisition_start()`,
`evidence_acquisition_advance()`, and `evidence_acquisition_finish()` methods; the accompanying
`evidence_acquisition_session` is already bound to the mission's request and snapshot digests.
`evidence_program()` builds the protocol-defined, source-grounded review agenda from the same
snapshots: each lane keeps bounded molecular/anatomy/imaging/development/function/outcome or
translation tracks, exact source IDs, required observation kinds, reviewer roles, and explicit
empty/truncated counts. Tracks additionally expose metadata-only observation coverage and
provenance gaps from the typed intake audit, so a local worker can prioritize missing caller
inputs without inventing clinical meaning. It is lexical retrieval metadata only, remains
provider-free and human-review gated, and never promotes a PMID or population row to a case
finding.
Pass `case_asset_manifest=` (and optionally `case_asset_manifest_query=`) to join each track's
required observation classes to a real, de-identified asset inventory. The response exposes
digest-only `asset_coverage` rows with `observed`, `present_not_observed`, or `missing` states
and explicit missing asset kinds; asset bytes and local identifiers remain outside the SDK.
Each track's `review_worklist` turns those gaps into bounded metadata-only obligations for a local
worker (for example, bind a source digest or review an unobserved class), never clinical actions.
When a persisted `case_asset_review_disposition` is passed with the manifest, its digest and
pending/resolved counts are propagated into the program and every acquisition lifecycle call;
mismatched or tampered ledgers are rejected.
Mission envelopes additionally expose `mission_audit`, a digest-bound assembly fuse that checks
request/snapshot identity, report-plane presence, and the provider-free review boundary. Its
`integrity_ok` flag is provenance metadata only, not a clinical readiness score. Its checks also
reject drift between a supplied case-asset manifest and the evidence program's asset-coverage
projection before a mission is handed to a reviewer.
`glioma_molecular_map()` grounds selected typed IDH1/IDH2, MGMT, EGFR, TERT, H3, 1p/19q,
methylation, and related marker terms against exact records in those validated snapshots. It keeps
assay missingness, source IDs, truncation, and human-review obligations explicit; zero hits are not
negative evidence and no population or citation match is treated as a patient result.
`case_asset_manifest()` is the real-data multimodal intake seam. It accepts only a caller-owned,
de-identified manifest for imaging, pathology, molecular, operative, functional/developmental,
longitudinal, or anatomical assets, binding each entry to a lowercase SHA-256 content digest and
source/timepoint metadata plus an explicit asset state. The Rust server refuses synthetic data, direct identifiers, malformed
or duplicate entries, and specialty drift; it returns digest-only asset references, per-kind
coverage, provenance/missingness review tasks, `provider: none`, `network: false`, and
`human_review_required: true`. Asset bytes are never opened or parsed, so this is provenance
intake—not a pixel/content DICOM, pathology, genomics, EHR, diagnosis, treatment, or operative parser—and it
requires no OpenAI key or network provider.
`case_fhir_import()` accepts the same boundary from a caller-sanitized FHIR `Bundle`: only
resource type/id and an explicit asset-kind hint are inspected, while identifiers, patient
references, narratives, measurements, codes, and raw text are refused. The bounded report hashes
resource/source identifiers, marks unclassified resources for review, and can be replayed against
the exact request, bundle, and hints without a provider or network.
`case_dicom_import()` accepts standard DICOM JSON metadata from a de-identified archive. It projects
only bounded series metadata and UID digests, rejects known patient-identifying tags and `PixelData`,
ignores unknown/private tags, and never opens pixels or DICOM bytes. Missing SeriesInstanceUID,
dates, modality, body region, and object-byte digests remain explicit reviewer obligations in a
replayable, non-synthetic report; no OpenAI key or network provider is required.
`case_dicom_evidence_workflow()` composes that digest-only DICOM projection with the validated
real-glioma and/or PubMed snapshot, evidence synthesis, the six-track evidence program, and a
resumable local acquisition checkpoint in one replayable report. It binds the projected manifest
digest into every nested worker and preserves case metadata, population records, and citations as
separate planes; it never opens pixels, fetches URLs, invokes a provider, or emits a clinical
conclusion. Pass `real_glioma_data=` for glioma (required) and `public_literature=` when desired,
with the bounded `query=` object controlling local filters and reference limits.
For a mission-level glioma dossier, pass `case_dicom_import=` to `run_research_mission()`.
The mission carries the DICOM receipt and verifies its manifest digest through evidence synthesis,
programming, and acquisition. This lane requires `real_glioma_data` and cannot be combined with a
second case-asset manifest or disposition; use `case_dicom_evidence_workflow()` when a
cross-specialty PubMed bundle is also needed.
The same helper accepts `case_fhir_import=` for a caller-sanitized FHIR metadata export. The
receipt is rebound through synthesis, evidence programming, acquisition, and mission audit for a
real glioma bundle, a cross-specialty PubMed bundle, or both. Resource payloads and clinical values
never enter the response; FHIR can be combined with DICOM for one multimodal digest-only manifest,
but not with a second asset manifest or disposition ledger.
FHIR and DICOM imports may be supplied together; their independently validated digest-only
projections are unioned into one multimodal manifest while each child receipt remains visible.
When replaying a persisted mission with `validate_mission()`, pass the original sanitized
`case_fhir_import=` or `case_dicom_import=` too; the server fails closed if a receipt is present
but its source export is omitted.
For a repeatable CLI worker over this mission-level lane, use
`scripts/run_neurosurgical_mission_with_dicom.ps1`; it validates the DICOM/manifest/synthesis
bindings and zero mission-audit failures before persisting the envelope.
That query may also include `real_data_reasoning_context=` and/or
`public_literature_reasoning_context=` to return a bounded, digest-addressable context handoff for
a caller-owned local model; each context remains source text/metadata, is bound to the same bundle
digest, and is never interpreted by the SDK.
`evidence_synthesis(..., case_asset_review_disposition=...)` can then resume a persisted reviewer
ledger for that exact projection; the returned digest and pending/resolved/unresolved counts are
workflow metadata only and remain human-review gated.
`case_asset_review_disposition(report, decisions=...)` records caller-owned human-review state for
the returned sequence-numbered obligations. It accepts only `reviewed`, `unresolved`, or
`not_applicable`, binds the canonicalized decision set to `report_digest`, rejects duplicates and
unknown rows, and keeps omitted/undecided rows pending. The stateless envelope never opens asset
bytes, stores local identifiers, invokes a provider, or represents a clinical conclusion.
`public_literature_matrix()` fans out the same bounded filters across selected lanes (or all six)
and returns independent packet/digest rows plus explicit empty and truncation counts.
`public_literature_reasoning_context()` renders one such packet into bounded, source-addressable
context for a caller-owned local model, preserving untrusted abstract excerpts and explicit
omission counts; missions expose the same context report and the ordered `research_plan`
automatically, so explicit intake gaps become caller-owned next-review tasks.
`plan_research()` turns the intake audit into bounded caller-owned acquisition, provenance-repair,
interpretation, corpus-review, and population-context tasks; optional snapshot references remain
source-linked context rather than patient findings.
`evidence_acquisition()` compiles the next autonomous, digest-bound local replay wave across both
validated real-glioma and PubMed planes. It preserves per-step trigger/source/query identity,
fallback scans, match/truncation state, freshness, missing-source obligations, and reviewer hold;
it never fetches, invokes a provider, or turns population/citation metadata into a case finding.
For checkpointed execution, `evidence_acquisition_start()`, `evidence_acquisition_advance()`, and
`evidence_acquisition_finish()` persist the same digest-bound session contract in caller-owned JSON;
changed snapshots or requests are refused before replay, and no method opens an API key.
`research_brief()` performs a deterministic topic-lane extraction over one validated real-glioma
or six-specialty literature snapshot. It returns exact matched terms, stable source IDs/URIs,
abstract and truncation counts, cross-topic overlap, explicit unknowns, and reviewer prompts for
the local/human handoff. It performs no model invocation, ranking, diagnosis, treatment inference,
or patient-evidence promotion, and always keeps `human_review_required` true; missions include the
same brief automatically.
`evidence_graph()` projects a validated real-glioma snapshot into a bounded, digest-addressed
study/profile/PMID crosswalk with explicit root traversal and omission/isolate counts; it never
infers biology, causality, or clinical action.
`real_data_coverage()` audits source and record-kind counts, trial/publication date axes, assay
modality coverage, abstract availability, and explicit linkage gaps while preserving missingness;
it exposes a reproducible coverage digest but never scores freshness or evidence quality.
`real_data_reconciliation()` performs a local, deterministic PMID/normalized-DOI cross-source
audit over that same snapshot. It surfaces portal citations absent from the bounded literature
window and duplicated PMIDs/DOIs as metadata-only review work, preserving counts under truncation
without fetching, merging, repairing, ranking, or making clinical claims.
`query_real_data()` also accepts case-insensitive PubMed `publication_type` and `mesh_term`
facets plus inclusive `publication_date_from`/`publication_date_to` bounds, narrowing indexed
literature metadata without turning labels or chronology into quality or clinical scores. The
refresh edge leaves year-only or month-only PubMed dates missing rather than inventing a day, so
bounded chronology never claims precision the source did not provide.
`real_data_freshness()` and `public_literature_freshness()` classify source retrieval age against
an explicit caller-supplied UTC `as_of` timestamp and `max_age_days` policy. They preserve
`current`, `stale`, and `future_dated` states, require review for future-dated metadata, and never
use the host clock or turn age into a quality or clinical score.
Real-data `run_research_mission()` responses also carry the default bounded
`real_data_trial_landscape`, `real_data_molecular_coverage`, `real_data_review_queue`,
`real_data_evidence_packet`, `real_data_evidence_graph`, and
`real_data_reasoning_context`, so a caller can inspect stable study/profile/PMID links, explicit
metadata obligations, and one digest-bound source-addressable handoff before reviewing the route
report. Trial landscape is a descriptive ClinicalTrials.gov inventory and molecular coverage is a
cBioPortal assay/profile metadata ledger; neither ranks trials, infers eligibility, or exposes
patient-level molecular values. Public-literature missions carry the corresponding
`public_literature_evidence_packet`.
They also run `public_literature_integrity_audit` automatically for the requested specialty lane
before assembling that packet, brief, or reasoning context, so missing identifiers and metadata
remain visible review obligations rather than silently shaping a synthesis.
Pass `freshness={"as_of": "...Z", "max_age_days": ...}` to carry the explicit source-age report in
the mission envelope.
`real_data_diff()` compares two validated snapshots for bounded added/removed/changed records and
source metadata, preserving before/after digests without copying abstracts or sample values.
`real_data_refresh_audit()` composes that diff with coverage, optional freshness, metadata review
obligations, and the deterministic research brief. It is restart-safe and digest-bound, reports
identity stability and explicit review reasons, and never accepts, merges, fetches, ranks, or writes
the candidate snapshot; `provider` remains `none`, `network` remains `False`, and human review is
required.
`public_literature_refresh_audit()` provides the equivalent restart-safe before/after reconciliation
for the six-specialty PubMed snapshot: it keeps stable PMID/source identities, changed field names,
lane coverage, truncation, and optional freshness explicit without copying abstracts or accepting the
candidate. It is provider-free and human-review-only.
`literature_link_audit()` joins the real glioma literature index to a selected public lane by exact
PMID/normalized DOI, exposing linked and unmatched windows plus metadata drift without inferring
cohorts, evidence quality, or clinical meaning.
`public_literature_integrity_audit()` is the pre-synthesis corpus gate: it reports source-linked
missing DOI/abstract/publication-type/MeSH metadata and duplicate normalized DOI groups in bounded
lane scopes, with no silent repair, quality score, or clinical interpretation.
`public_literature_review_queue()` converts those findings into bounded, stable, source-linked
`needs_human_review` tasks with PMID/source URIs, titles, reviewer roles, and explicit reasons;
missions include the same queue automatically and preserve any omitted rows.
`public_literature_workbench()` adds a lane-scoped navigation view: each selected specialty's
closed profile is joined to exact source/record/abstract counts, missing metadata, and integrity
obligations from the real snapshot. It is not a readiness or quality score, never ranks lanes or
repairs fields, and remains provider-free with `synthetic_data: false`, `provider: none`, and
`network: false`; public-literature missions include the request-specialty workbench.
Workbench lanes also expose non-exclusive metadata-derived design strata with exact PMIDs, keeping
human-indexed, animal/preclinical, in-vitro, review, imaging, surgical, developmental/genetic,
outcome, and interventional contexts separate for human review. Unclassified and overlapping rows
remain explicit obligations rather than quality scores or patient evidence.
`public_literature_portfolio()` composes one bounded exact-query/workbench/reviewer-queue handoff
per selected specialty (all six when `specialties` is omitted). The result is digest-bound to the
validated real snapshot and preserves explicit hit, review-item, omission, and truncation counts;
it never scores or ranks evidence, infers a clinical conclusion, or repairs missing metadata. The
helper preserves the provider-free posture (`provider: none`, `network: false`, `synthetic_data:
false`) and never accepts a key, fetches a URL, or writes durable state.
Pass `portfolio_query={...}` to `run_research_mission()` with `public_literature` to attach that
multi-lane report to the mission envelope; a glioma mission may also provide both
`real_glioma_data` and `public_literature`. In that dual-bundle mode the real snapshot drives the
session route, the PubMed snapshot remains independent citation context, and the envelope carries
an exact PMID/DOI `literature_link_audit`; no cohorts are merged and the route remains unchanged.
Pass `case_asset_manifest={...}` (and, optionally, `case_asset_manifest_query={...}`) to attach
the same digest-bound, real de-identified multimodal metadata projection to the mission envelope.
The manifest is never opened as bytes: asset references are hashed and missingness/provenance
obligations remain explicit for human review. The mission's `evidence_synthesis` ledger carries
the asset report digest as an explicit cross-plane binding. Supplying a query without its manifest
is refused. Pass `case_asset_review_disposition={...}` to carry a persisted reviewer ledger into
the mission; its manifest/synthesis digest bindings are checked and unresolved counts remain
visible for the next local replay.
The repository-level `scripts/run_neurosurgical_autonomous_mission.ps1` command can refresh both
validated snapshots and invoke the same dual-bundle mission without a provider key. It defaults
to the extended TCGA-GBM + TCGA-LGG glioma population and broad molecular PubMed lane;
`-SkipRefresh` replays those last validated files offline. Its `-GdcProjectIds`, `-PubMedTerm`,
and `-PubMedSourceId` parameters forward the exact population/citation scope into the real-data
refresh while preserving source identity and replay provenance.
`real_data_review_queue()` derives stable, caller-owned metadata-review tasks from one validated
snapshot (for example, an absent PMID crosswalk or clipped abstract) and never assigns clinical
urgency or imputes missing values.
The acquisition helpers also accept `case_asset_manifest=` and the optional
`case_asset_manifest_query=` pair. The returned plan/session carries the manifest report digest
and bounded asset review items, and every replay re-binds that digest before executing another
local snapshot query. Asset bytes are never opened; this makes multimodal provenance gaps part of
the same restart-safe worker handoff.
`real_data_review_disposition()` applies digest-bound `reviewed`, `unresolved`, or `not_applicable`
workflow state to those tasks; the envelope is stateless and leaves omitted or undecided work
pending.
`real_data_evidence_packet()` composes the validated summary, coverage, explicit crosswalk,
source-linked query hits, canonical ClinicalTrials.gov trial landscape, comparative GDC cohort
landscape, and review queue into one digest-addressed handoff for a local model or human reviewer.
Pass `freshness={"as_of": "...Z", "max_age_days": ...}` to carry an optional
digest-bound current/stale/future-dated source-age posture; the same optional field is accepted by
`public_literature_evidence_packet()` and no clock is invented when it is omitted.
`real_data_autonomous_workflow()` turns that packet into a deterministic, resumable
provenance/completeness/context review wave. It emits source-addressable metadata actions,
including an explicit `refresh_source_snapshot` action for stale sources and
`expand_evidence_projection` holds when bounded result limits omit rows,
accepts a persisted `dispositions=` report to close or reopen queue items, and keeps truncation,
unresolved work, and the final human-synthesis gate explicit. It does not rank clinical value or
auto-approve a conclusion.
`RealDataQueryHit` rows preserve optional ClinicalTrials.gov phases, update date, study type,
aggregate enrollment target, and intervention names, plus portal sample counts and PubMed
publication dates. Real-data queries also accept exact cBioPortal `molecular_alteration_type`/
`molecular_datatype` facets and exact GDC `genomic_data_type` file facets. These fields are copied source metadata; absent upstream fields stay absent and
are never treated as eligibility or outcome evidence.
`real_data_trial_landscape()` adds a bounded, digest-bound registry projection over those same
rows, with explicit multi-phase accounting, study/status/intervention buckets, update-date range,
and missingness/truncation review reasons. It is provider-free, network-free, and descriptive only.
`real_data_molecular_coverage()` adds the complementary cBioPortal availability ledger: exact
alteration-type/datatype facets, per-study profile rows, analysis-visible and patient-level
metadata flags, description coverage, explicit missing alteration/datatype counts, and explicit
profile/study truncation. It also reports aggregate GDC project file/data-type facets and missing
facet obligations when those real availability rows are present. It exposes no
mutation or expression values and never infers that an assay was run for a specimen. The
real-data evidence packet includes this ledger automatically (schema `/0.3`).
`real_data_cohort_landscape()` adds a bounded comparative view over source-linked TCGA/GDC
projects in the validated bundle. It reports aggregate released-case inventory and per-project
file/data-type availability with exact project citations, explicit truncation, and missingness;
it is read-only metadata and never opens files, exposes samples, merges cohorts, or makes clinical
claims.
`specialty_evidence_map()` provides the same request's lane-specific identity, spatial,
functional, and temporal coverage ledger for all six supported specialties. It preserves source
IDs, timestamp coverage, and missing/uninterpretable/conflicting states without echoing values or
interpreting them.
`real_data_reasoning_context()` renders that handoff into a bounded, source-addressable context
string for a caller-owned local model, with optional untrusted abstract excerpts, digest-bound
citations, and explicit omission/truncation counts. It performs no model invocation or clinical
interpretation.
`real_data_draft_audit()` is the matching local-model boundary: every structured claim must cite a
record emitted by that packet, `patient_case` and `clinical_action` postures are blocked, and
accepted rows remain `grounded_for_human_review` rather than clinically validated.
The bundle digest and provenance status remain in the response, and the route always ends at human
review rather than producing a diagnosis, prognosis, treatment, triage, or procedure.
The same bundle can be passed as `public_literature=` to `start_session()`,
`advance_session()`, `run_session_to_review()`, `finish_session()`, or `run_research_mission()`;
those checkpoints bind `public_literature_digest` and refuse request or bundle drift.
`NeurosurgicalObservation` accepts optional caller-supplied `observed_at` timestamps and
`timepoint` labels; `LocalNeurosurgicalAgent.temporal_audit()` exposes the nested digest-bound
alignment report without inferring a trajectory.

For a running `bioprism-api` gateway, the same standard-library package provides bounded HTTP
access:

```python
from prism_sdk import ApiClient

api = ApiClient("http://127.0.0.1:8787", bearer_token="0123456789abcdef")
print(api.capabilities()["tool_count"])
result = api.call_tool("modality_catalog", {})
page = api.events(after=0, limit=100)
```

`ApiClient` and `AsyncApiClient` cover health, capabilities, tools, typed capability discovery,
parity audits, route proposals, REST calls, event cursors, and the signed webhook outbox. HTTP
failures raise `ApiError` with the status and structured payload;
the client does not retry domain refusals or treat a transport `2xx` as scientific acceptance.
See [`docs/HTTP_API.md`](../docs/HTTP_API.md) for the route and delivery contract.

To connect the HTTP gateway to the autonomous domain runtime, compose the caller-configured
client with an exact, reviewed catalogue. The bridge performs no `tools()` discovery during an
agent call, accepts no key or credential argument, and preserves remote refusals as bounded typed
failures. The registry still owns the explicit domain/capability/risk mapping:

```python
from prism_sdk import (
    AutonomousDomainToolRegistry,
    AutonomousDomainToolRuntime,
    create_autonomous_api_tool_executor,
)

catalogue = api.tool_catalogue()  # snapshot, review, and bind exact names before dispatch
registry = AutonomousDomainToolRegistry()
registry.register_mcp_catalogue(
    catalogue,
    {
        "modality_catalog": {
            "domains": ["biomedical", "neuroscience"],
            "capability": "modality_catalogue",
        }
    },
    require_all=False,
)
runtime = AutonomousDomainToolRuntime(
    registry,
    executor=create_autonomous_api_tool_executor(api, catalogue=catalogue),
    receipt_sink=lambda receipt: journal.append(receipt.to_dict()),
)
```

`receipt_sink` is optional and caller-owned. It receives only the same digests, statuses, and
execution identity retained by `runtime.receipts`; arguments, outputs, HTTP envelopes, and keys
are never sent to it. A sink failure fails closed instead of turning a completed tool call into an
unobserved success. The adapter and sink contract are exercised across all twelve built-in domain
profiles, including refusal, malformed-response, transport, schema, and no-discovery paths.

For post-run operations, `AutonomousAgent.analyze_run_trace()` produces a digest-bound,
metadata-only analytics report without invoking a provider. For longitudinal operator state,
create an `AutonomousRunAnalyticsLedger` and bind it with
`agent.create_run_analytics_controller(ledger, persistence)`. The controller requires
`restore()` before reads, offers `analyze_and_ingest()` and `ingest()` with explicit
accepted/duplicate/conflict results, persists through caller-owned JSON/CAS storage, and returns
persistence failures separately from in-memory acceptance. Its summaries cover all twelve domains,
observed providers, and provider/model pairs while retaining no task text, prompts, responses,
tool payloads, credentials, or effect values. For an application worker that needs both operator
indexing and analytics, `agent.create_trace_registry_controller(...)` plus
`agent.create_run_observability_controller(...)` provide restore-before-read lifecycle,
single-snapshot publication/analysis, explicit partial-persistence outcomes, and restart-safe
all-domain projections without replaying the run.
An optional alert sink receives only digest-keyed threshold metadata and must treat `alert_id` as
its downstream idempotency key; sink failures are returned separately from trace and analytics
results.

For deployment-owned authority, `AutonomousAuthorizationLedger` binds caller-issued grants to a
tenant, actor, session, authorization digest, domain set, operation, capability, risk class,
expiry, and bounded use count. `AutonomousAuthorizationGate` performs the final fail-closed check
before a caller-owned provider/source/tool/effect callback. The ledger supports hash-linked
metadata events, request-digest replay protection, canonical restore, and optional CAS persistence
across all twelve domains; it never receives prompts, task text, credentials, headers, payloads,
arguments, or results. Identity authentication, grant issuance, encryption, and distributed
coordination remain deployment responsibilities.
evidence, tool payloads, credentials, or cost claims.

For actual provider execution, bind an `AutonomousAuthorizationContext` (constructed from the
caller-issued grant) to `LLMRuntime.invoke()`, `invoke_stream()`, `collect_stream()`, or
`invoke_tool_loop()`, or pass it through the high-level brain/orchestrator options. The runtime
mints a fresh metadata-only request immediately before each provider attempt and each tool-loop
turn. Authorization is checked before credential resolution, quota reservation, observers, effect
journaling, and transport, so a denied scope or exhausted grant cannot contact a provider. The
context carries no key, prompt, message, response, or tool result; credentials remain caller-owned
opaque handles.

Pass the same context into connector dispatch and domain-tool/effect execution as well. Connector
dispatch checks `connector_dispatch` after the reviewed selection and approval checks but before
the registered executor; read-only domain tools check `tool_execution`; effectful tools and
`AutonomousEffectBoundary.execute()`/`execute_stream()` check `effect_dispatch` before journal
transitions and before user code. The requests contain only exact domains, capability/risk
metadata, call identity, and a SHA-256 resource digest. Refused tool calls return the explicit
`authorization_required` status and never invoke the executor. Connector replay remains a
metadata-only replay and does not consume a fresh authorization allowance.

```python
scoped_tools = tool_runtime.scoped(
    execution_id="run-1",
    domain="coding",
    authorization_context=authorization_context,
)
tool_results = scoped_tools((provider_tool_call,))
```

Use `context.for_domain("coding")` / `context.forDomain("coding")` for workers narrowed from a
multi-domain grant. Parent and child contexts share one monotonic request sequence, preventing a
fan-out child from reusing a request ID already minted by its parent. The authorization context
is optional for legacy local adapters, but once a deployment relies on caller-issued grants it
must be forwarded to every connector, tool, and effect boundary; registration and approval alone
do not establish authority.

For provider-backed evidence, `AutonomousConnectorRegistry` and `AutonomousConnectorRuntime`
provide the corresponding caller-owned connector process. Register a typed
`DomainEvidenceProviderConnectorManifest` and an executor that may close over a short-lived
credential session; the runtime enforces exact manifest domains/capabilities and approval before
calling it. The request and returned value are transient, while the dispatch receipt retains only
request/payload/manifest digests and bounded failure classes. `AutonomousConnectorReceiptJournal`
can persist connector receipts as bounded, fsynced, hash-chained JSONL with identity-conflict
detection and safe replay barriers:

```python
from prism_sdk import (
    AutonomousConnectorDispatchRequest,
    AutonomousConnectorRegistry,
    AutonomousConnectorRuntime,
    AutonomousConnectorRegistration,
    AutonomousConnectorReceiptJournal,
)

connector_registry = AutonomousConnectorRegistry([registration])
selection_plan = connector_registry.select_for_domains(("science",), capability="literature_search")
connector_journal = AutonomousConnectorReceiptJournal("state/connector-receipts.jsonl")
connector_runtime = AutonomousConnectorRuntime(connector_registry, receipt_store=connector_journal)
result = connector_runtime.dispatch_from_plan(
    selection_plan,
    AutonomousConnectorDispatchRequest(
        dispatch_id="evidence-dispatch-1",
        execution_id="run-1",
        call_id="evidence-call-1",
        connector_id=registration.manifest.connector_id,
        domains=("science",),
        capability="literature_search",
        request={"query": "caller-supplied transient query"},
        selection_plan_digest=selection_plan.plan_digest,
        approved=True,
    )
)
```

After a restart, dispatching the same execution/dispatch/call/attempt identity returns
`result.replay == "replayed"` with `result.value is None`; the journal is a replay barrier, not a
provider-response cache. A retry must deliberately use a new `attempt_id` (and normally a new
dispatch/call identity). The journal never claims distributed exactly-once delivery: external
provider idempotency and cross-process locking remain caller-owned.

`select_for_domains()` is the reviewable decision boundary before that dispatch. It deterministically
selects the lexicographically first registered manifest for each requested domain/capability, retains
all candidate and manifest digests for review, and binds the registry snapshot to `plan_digest`.
`dispatch_from_plan()` re-verifies the live registry and requires the request's
`selection_plan_digest`; a changed manifest, missing domain, or connector mismatch fails before the
executor is called. This is deliberately deterministic and inspectable—provider health, cost,
latency, and evaluator-driven connector ranking are caller-owned plan inputs when supplied.
When those inputs are available, pass bounded per-connector `selection_signals` to
`select_adaptive_for_domains()` (health, success rate, evaluator reward in `[-1, 1]`, latency,
cost, and eligibility). The weighted score is deterministic, ties resolve by connector ID, and the
plan retains only normalized scores, eligibility, and a signal digest. `AutonomousAgent` exposes
the same process through `connector_selection_plan()`, `connector_catalogue()`, and
`dispatch_connector()`; the façade still never accepts a key or turns evaluator reward into
authorization.

The registry never accepts a raw key and does not perform network I/O. A caller-owned executor
can invoke the existing source-plan, provider-handoff, or external-payload APIs and return a
typed transient observation. Credential handles, raw queries, provider responses, and headers are
never serialized into the dispatch receipt. Both connector dispatch and journal reopening are
tested across all twelve built-in domains, including approval, scope, capability, tamper,
capacity, secret-field, and executor-failure paths.

`create_autonomous_api_source_connector_executor(api, use_tool_route=True)` is the ready-made
bridge for the existing source-plan/source-execute gateway routes. It validates the connector kind
and manifest scope, snapshots the exact plan through the configured `ApiClient`, and uses the
returned `plan_digest` when executing; a caller-provided digest cannot redirect the fetch. It is
still key-agnostic and does not call `tools()` discovery.

The provider runtime and [`AutonomousBrain`](../docs/AUTONOMOUS_BRAIN.md) add the caller-approved
LLM boundary: BYOK credentials become short-lived opaque handles, model selection is health- and
capability-gated, prompts and mission plans are bounded, and external effects require explicit
approval. `BrainEpisodicMemory` provides optional restart-safe, hash-chained metadata memory.
Automatic task runs derive bounded digest-only task facets for related-lesson retrieval, never
persisting task text; explicit evaluator rewards remain the only learning authority.
Adaptive provider, native tool-loop, and mission-proposal calls compile a
`bioprism-autonomous-model-continuation/0.1` plan from the first selection. It keeps the complete
bounded eligible ladder in deterministic order, records candidate metadata digests, and separates
model-scoped timeout recovery from provider-scoped failures. A provider outage may skip several
sibling arms without spending multiple failover transitions. `BrainRunResult.continuation_plan`
exposes the plan, failover receipts include its digest, and
`create_model_continuation_state()`, `advance_model_continuation_state()`, and
`complete_model_continuation_state()` provide a digest-fenced restart cursor. Health updates still
inform later runs but cannot silently reorder the current run; task text, prompts, credentials,
arguments, evaluator bodies, and provider responses remain transient.
`AutonomousGoalLedger` adds the restart-safe objective layer above that memory: bounded attempts,
criterion/evidence digests, blockers, optimistic revisions, and a hash-chained lifecycle work for
every built-in domain without retaining goal text or provider payloads.
`AutonomousGoalScheduler` adds deterministic multi-goal admission above the ledger. It applies
explicit priority/urgency/deadline signals, aging fairness, dependency closure, retry policy,
concurrency/cost budgets, per-domain quotas, and required-domain coverage, then returns a
digest-bound metadata-only schedule. `claim_autonomous_goals()` rechecks every expected revision
before moving admitted goals to `running`; stale schedules and dependency cycles fail closed.
The Python schedule digest is portable with the TypeScript scheduler for the same goal projection.
`AutonomousGoalWorker` closes the execution loop: it preflights caller-owned task rehydration,
claims the admitted rows, invokes a caller-owned executor, and settles result status, criterion
updates, evaluator digests, and retry-safe failure state. Rehydrated task text and executor output
remain transient; `AutonomousGoalWorkerBatch.to_dict()` contains only the schedule, claim, outcome
digests, bounded error classes, and aggregate counts. The worker supports all twelve catalogue
domains, including `cross_domain`, and its single-attempt digest is portable with TypeScript.
For process-loss recovery at the executor boundary, pass an `AutonomousGoalWorkerJournal` and a
stable `batch_id`. Its bounded hash chain records only prepared/claimed/dispatch/settlement
metadata. `recover()` pauses a claim that died before dispatch so it can be retried, but blocks a
claim that reached dispatch with `goal-reconciliation-review`; it never replays an uncertain
provider effect. `JsonAutonomousGoalWorkerJournalPersistence` and
`AutonomousGoalWorkerJournalPersistenceCoordinator` provide canonical caller-owned snapshot
storage with optional compare-and-swap fencing. Journal snapshots exclude task text, prompts,
parameters, credentials, and executor results just like the goal ledger.
The worker also verifies `goal_task_digest(resolved_task)` against the immutable ledger identity
before it claims anything, so a stale or mis-keyed protected queue cannot execute a different task.
When parameters are present, each journal event carries only an `execution_binding_digest`; this
binds transient action handoffs and other executor inputs across prepared/claimed/dispatch/settled
phases without serializing them. `active_for()` and `assert_no_active()` expose a fail-closed
restart fence: the caller must recover or explicitly reconcile an in-flight boundary before a new
worker pass can resolve or dispatch that goal.
`AutonomousGoalControlLoop` continues those bounded worker passes until every goal is terminal,
no safe work is admissible, or an explicit cycle/run budget is exhausted. Its optional
`options_factory(context)` receives only prior cycle metadata and ledger counts, so a caller can
refresh priorities, urgency, dependencies, retry policy, and required-domain coverage without
reintroducing task payloads. Paused objectives can be re-admitted on a later cycle, while failed,
blocked, or concurrently running objectives produce an explicit `no_admissible_work` stop rather
than being reported as success. The result contains cycle digests, domain/status counts, and live
executor values only on the initiating process.
Pass `evaluator(cycle)` to close the loop with explicit quality credit. The callback must return one
metadata-only reward packet per run; rewards are finite values in `[-1, 1]`, bound to the goal's
attempt and worker outcome digest, and never inferred from transport or executor status. With no
custom learner, `AutonomousGoalBanditLearner` updates domain admission priorities using a bounded
UCB-style value state. Custom learners can return only validated scheduling signals and a learning
state digest. Goal records receive evaluator/learning digests through an optimistic revision fence;
raw evidence, evaluator values, tasks, prompts, credentials, and live results stay transient.
`AutonomousGoalAgentRuntime` connects this loop to the real autonomous facade. Prefer
`agent.goal_agent_runtime(...)` or `agent.run_goal_control_loop(...)`: Python now binds the
worker to the complete `AutonomousAgent`, just like the TypeScript runtime, so each claimed goal
uses the same credential-session, model-inventory, prompt, routing, provider, connector/tool,
evaluator, and learning boundaries as a direct run. A caller-owned task resolver rehydrates each
goal's text only after admission, and a run-options factory supplies transient model candidates,
opaque credential handles, approval callbacks, memory, tools, and policy at execution time. All
twelve domains, including `cross_domain`, use the same facade path; no resolver or provider value
is copied into goal or loop metadata.

For one ceiling across a composed run, create an `AutonomousCostBudget` and pass its
`reserve` callback through the agent options. The same callback covers semantic/planning calls,
provider failover, tool-loop turns, workflow stages, cross-domain fan-out, synthesis, and
evaluator-driven retries. Reservations are atomic for local concurrent workers; only a refusal
before provider dispatch releases one. Streams reserve on first iteration, while an already
dispatched or transport-ambiguous attempt remains charged. Use `budget.snapshot()` and
`AutonomousCostBudget.from_snapshot(...)` for caller-owned restart handoff; this is estimate
accounting, not billing truth, and never stores prompts, responses, credentials, or keys.

For one metadata-only observability record across the complete batch, call
`runtime.run_with_trace(trace_store=trace_store, run_id="goal-batch-001", ...)`. The trace spans
goal planning, rehydrated worker execution, selector/provider lifecycle, evaluator settlement,
learning metadata, and the final loop status across all twelve domains. The returned
`AutonomousGoalAgentTracedRunResult.result` stays live in the initiating process; its
`to_dict()` projection, trace summary, and trace snapshot retain only digests, counters, bounded
identifiers, and failure metadata. Task text, prompts, parameters, credentials, provider
responses, and live results are never serialized, and caller observers/callbacks remain composed.

For an operator index across many runs, import validated snapshots into
`AutonomousRunTraceRegistry`. It provides deterministic cursor pages and filters for run, domain,
status, provider, and model, plus separate retained-event inspection. Set `retain_events=False`
to keep summary counters and provider/model identities without retaining event rows. The bounded
`max_runs`/`max_events`/`max_bytes` policy evicts only the oldest eligible terminal records and
protects running, partial, paused, and unknown work by default; if no safe eviction exists, the
import fails without mutating the registry. JSON and transactional-CAS persistence adapters are
available, and restoration is an observability operation only—it never resumes provider work or
authorizes a tool or effect.

```python
from prism_sdk import AutonomousRunTraceRegistry

registry = AutonomousRunTraceRegistry({"max_runs": 2_000, "max_events": 100_000, "max_bytes": 20_000_000})
registry.import_snapshot(trace_store.snapshot())
page = registry.query({"domain": "biomedical", "status": "completed", "limit": 100})
events = registry.events({"run_id": page.records[0].run_id, "phase": "provider_invocation_finished"})
```

The high-level traced bridge can publish the projection as part of the same boundary with
`runtime.run_with_trace(trace_store=trace_store, trace_registry=registry, run_id=...)`.
`traced.trace_registry` reports imported/replaced/unchanged state, source and registry digests,
eviction count, or a sanitized failure. Publication is idempotent and best-effort, so an index
outage never changes provider settlement or causes an already-dispatched goal to retry.

`AutonomousProviderEffectReconciliationCoordinator` provides the corresponding restart admission.
Pass it to `BrainWorker`, `RemoteBrainJobWorker`, or `AsyncRemoteBrainJobWorker` as
`effect_reconciliation`; the lifecycle runs one bounded status pass before claiming work, caches
the result for concurrent stages, and blocks fresh provider dispatch while any external effect is
uncertain or reconciliation errors remain. After caller-owned resolution, call
`reset_effect_reconciliation()`. Only bounded counts, statuses, and digests cross the worker
boundary.

For deployments that already have an operator-approved action record, add an
`action_handoff_resolver(goal, row, task)` to the runtime (or to
`run_goal_control_loop`). It may return the verified handoff directly, or
`{"handoff": handoff, "request": {"domain": ..., "hints": ..., "allow_cross_domain": ...}}`
when replay needs routing inputs that are not encoded in the handoff. The runtime validates the
handoff during rehydration and again after the scheduler claim, then calls
`agent.execute_action_handoff(...)`; a goal cannot silently fall back to the raw run path when
this resolver is configured. Handoff metadata is transient worker input, while credentials,
callbacks, provider options, and effect authority remain supplied by `run_options_factory`.

```python
result = agent.run_goal_control_loop(
    ledger,
    task_resolver=lambda goal, _row: protected_queue.read(goal.goal_id),
    run_options_factory=lambda _goal, _row: {
        "credentials": session,
        "model_candidates": agent.models(enabled_only=True),
        "approve_provider_call": True,
    },
    evaluator=evaluate_goal_cycle,
    schedule_options={"max_selected": 12, "max_concurrent": 4},
)
```

The callbacks and their values remain process-local; the ledger and control-loop checkpoint
retain only digests, statuses, counts, and value-only bandit state.
For long-running loops, pass a stable `run_id` and a `checkpoint(snapshot)` callback. The sealed
checkpoint records contiguous cycle summaries, aggregate counters, evaluator digests, learned
signals, and the built-in bandit's value-only arm state; it never records task text, prompts,
parameters, credentials, callbacks, provider output, or evaluator evidence. The
`JsonAutonomousGoalControlLoopSnapshotPersistence` adapter canonicalizes that projection, while
`TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence` adds stale-writer fencing through
`write_if_unchanged`. `AutonomousGoalControlLoopPersistenceCoordinator.flush` is directly usable
as the callback:

```python
coordinator = AutonomousGoalControlLoopPersistenceCoordinator(
    TransactionalJsonAutonomousGoalControlLoopSnapshotPersistence(store),
)
loop.run(run_id="research-mission-001", checkpoint=coordinator.flush, max_cycles=128, max_total_runs=8192)

# In a new process, restore the digest-bound image before supplying fresh transient callbacks.
snapshot = coordinator.restore()
fresh_loop.run(run_id="research-mission-001", resume_snapshot=snapshot, checkpoint=coordinator.flush, max_cycles=128, max_total_runs=8192)
```
Resume continues at the next cycle and restores the built-in bandit generation without replaying
the completed worker batch. The caller still rehydrates task text, model candidates, opaque
credentials, tools, memory, and approval policy after each new claim. Checkpoint generations are
content-addressed and linked to their predecessor; a tampered image, non-contiguous cycle, changed
run identity, or compare-and-swap conflict fails closed before execution.

For provider-backed evidence that must choose and recover across multiple approved adapters, use
`AutonomousLLMEvidenceAdapterRegistry` with `AutonomousLLMEvidenceAdapterSelector`. The selector
returns a digest-bound plan that can be persisted alongside a run;
`select_adaptive_for_domains` accepts only metadata-only health signals and can promote a
provider/model route after explicit evaluator credit. `InMemoryAutonomousLLMEvidenceAdapterHealthStore`
is a hash-chained learning ledger for acquisition outcomes and evaluator rewards, while
`JsonAutonomousLLMEvidenceAdapterHealthPersistence` and its transactional coordinator provide
restart-safe conditional persistence. Pass the plan, registry, and
`AutonomousLLMEvidenceFailoverAcquirer` created by
`create_autonomous_llm_evidence_adapter_failover_acquirer` to the existing evidence runtime or
agent façade. Failover is bounded and retries only retryable provider transport failures;
malformed prompt, argument, and credential failures stop immediately. Health snapshots and
failover events contain adapter/model manifest digests, statuses, bounded timings, and error
classes only—never keys, prompts, requests, provider responses, or raw error messages.

`AutonomousLLMEvidenceReadinessAuditor` adds the provider-free operational audit for that route.
It evaluates coverage, the exact selection-plan digest, selected-manifest health, circuit state,
and the caller's `AutonomousLLMEvidenceReadinessPolicy` across all twelve domains. Strict startup
policy reports unobserved or below-threshold routes as `blocked`; a caller can explicitly use
`require_health=False` to show the same routes as `degraded` for onboarding. A healthy route is
reported `ready`, while absent adapter coverage is `missing`. The report is canonical,
digest-addressed, byte-bounded, and restorable with strict field/aggregate checks. Pass
`evidence_readiness={"registry": registry, "health_store": health, "options": {...}}` to
`AutonomousAgent.readiness()` to compose those rows with model, credential, tool, and learning
readiness. This remains a projection only: it never dispatches a source, invokes an LLM, or
converts route health into evidence truth.

The provider-contract boundary makes that route executable without making the SDK a provider
client. `AutonomousEvidenceProviderContractRegistry` binds each approved adapter to its provider,
protocol, operation vocabulary, domain, capability, source kind, authentication posture, freshness,
pagination mode, and required request metadata. `create_acquirer_for_adapter()` verifies the
registry and contract immediately before invocation, so a changed adapter manifest, missing
operation, unsupported capability, or stale registry fails closed before a provider call. The
contract projection contains only digests and bounded metadata; credentials, prompts, requests,
and responses remain caller-owned.

`create_autonomous_evidence_source_acquirer()` adds the provenance admission boundary around that
contract acquirer. A caller supplies a source descriptor callback that returns only source identity,
source digest, authority, status, observation time, expiry, citation digest, and bounded limitations.
`AutonomousEvidenceSourcePolicy` evaluates freshness, future skew, authority, partial status, and
digest requirements, while `AutonomousEvidenceSourceLedger` records a metadata-only hash chain of
accepted and refused observations. JSON and compare-and-swap persistence coordinators support
restart recovery without retaining raw source values or locators. The contract/source tests exercise
all twelve domains, failover, refusal, secret-shaped output rejection, canonical round trips, and
stale-writer protection.

`AutonomousEvidenceRetryPolicy` separates bounded same-route retry from candidate failover. The
retry wrapper classifies only typed transient failures, applies capped exponential backoff, and
emits attempt number, status, failure class, delay, and latency as value-free telemetry. The
failover acquirer now applies that policy to every selected candidate and can compose the source
boundary inside each retry route, so a successful source receipt is admitted only after the exact
provider contract and source policy pass. Credential, argument, source-admission, and malformed
response failures do not get retried or silently promoted to another provider.

For explicit multi-source adjudication, `AutonomousEvidenceSourceReconciler` prepares a
digest-bound `AutonomousEvidenceReconciliationPlan` from caller-owned routes. Execution requires
`approve_source_dispatch=True`, fans out at a bounded concurrency, optionally normalizes values
under a named/versioned callback, and classifies the result as `consensus`,
`consensus_with_dissent`, `disagreement`, `insufficient_evidence`, or `failed`. The returned
`AutonomousEvidenceReconciliationResult` keeps source values and normalized values transient while
its canonical projection retains route/request/value/normalization digests, failure classes,
quorum, and disagreement metadata. Plan and result projections round-trip strictly and reject
route drift, normalizer drift, tampering, secret-shaped metadata, and oversized values. The
all-domain tests exercise consensus, dissent, disagreement, explicit approval, and bounded fan-out.

`AutonomousTaskOrchestrator.run_goal_step(...)` wires one bounded objective attempt into the normal
route, planning, model-selection, provider, evaluator, and approval lifecycle, returning raw
runtime output only transiently and persisting a value-only settlement.
`run_cross_domain_goal_step(...)` applies the same contract to specialist fan-out and synthesis,
retaining only outcome, evaluator, learning-state, and progress digests.
`run_goal_learning_step(...)` and `run_cross_domain_goal_learning_step(...)` connect those durable
goals to the existing online, trajectory, and bounded replan learners. They feed explicit
evaluator rewards into the bandit and persist only digests of evaluator decisions, next state, and
attempt identities; `cycle_id` is value-only correlation metadata. Model candidates, memory,
approval, and opaque credential handles still come from the caller, and no raw key is accepted.
`run_workflow_learning(...)` remains a report-only stage learner: a failed evaluator can request a
replan, but it never silently replays a provider call. Applications that explicitly want bounded
automatic recovery can call `run_workflow_cycle(...)` on `AutonomousBrain`,
`AutonomousTaskOrchestrator`, or `AutonomousAgent`. A cycle retries the complete prepared
workflow under the same exact route, model candidates, opaque credentials, reviewed tool set,
approval boundary, and execution controller. Only a failed evaluator decision can request the
next attempt, and the cycle has a hard three-replan ceiling. Its `AutonomousWorkflowCycleCheckpoint`
contains task/workflow/attempt/outcome/bandit digests only; the caller must rehydrate the protected
retry context after a restart because raw task text, provider responses, credentials, tool
arguments, and evaluator instructions are never persisted. This cycle is tested against every
built-in autonomous domain.
`run_adaptive_mission_learning_cycle()` combines recall, evaluator reward, bandit state updates,
and bounded pre-dispatch replanning without retaining provider text, tool arguments, or secrets.
`BrainJobStore` adds resolver-backed leases and checkpoints for restart-safe learning jobs, while
`DomainEvaluatorRegistry` supplies reusable evidence-only evaluator profiles for engineering,
research, operations, data, and biomedical workflows. `BrainControlPlane` adds cursorable job
events and explicit approval routing; `BrainWorker` renews leases across process boundaries and
can feed `BrainModelHealthStore` into future model selection. `BrainReplayEngine` re-evaluates
caller-rehydrated evidence across every built-in domain and optionally advances a caller-owned
bandit updater without replaying provider calls.

Provider invocation receipts can be evaluated through the same explicit model-learning boundary.
`AutonomousProviderOutcomeEvaluator` accepts only redacted provider metadata, digests, and
bounded caller evidence; prompt/response text, messages, request bodies, headers, credentials,
and raw provider errors are rejected. The transport status is never treated as task quality. The
default `agent.evaluate_provider_receipts(...)` updater applies the evaluator's bounded reward to
the existing `provider/model` contextual bandit state, while deployments may provide a custom
updater and persist `next_learning_state` through their own snapshot/CAS boundary. Reusing that
state makes a repeated receipt idempotent and does not rerun a provider. This bridge is an
evaluation and adaptation mechanism, not a factual, clinical, scientific, or operational truth
oracle.

`create_autonomous_cycle_evaluator_bridge()` closes the callback-plumbing seam for the automatic
learning paths. It validates a complete twelve-domain autonomous evaluator registry, exposes
stable catalogue and policy digests, and returns exact-domain evaluators for single-domain
learning plus a routed composite for cross-domain specialists and synthesis:

```python
from prism_sdk import create_autonomous_cycle_evaluator_bridge

bridge = create_autonomous_cycle_evaluator_bridge(
    lambda context: {
        "domain": context["domain"],
        "capability": "caller_review",
        "risk_class": "read_only",
        "signals": {signal: 1.0 for signal in context["required_signals"]},
    }
)
single_evaluator = bridge.evaluator_for_domain("coding")
cross_evaluator = bridge.evaluator_for_cross_domain(("coding", "data"))
```

The evidence factory receives only bounded run/status/digest metadata, the selected role, and
the evaluator's required signal contract. Task text, prompts, provider responses, tool values,
credentials, and evidence bodies never enter that context. Evidence is generated transiently and
validated by the existing domain adapters; provider completion is never treated as reward. Pass
`single_evaluator` to `run_learning()` or pass the bridge itself as `evaluator_bridge=` to
`run_auto()`'s automatic learning path; use `cross_evaluator` for direct cross-domain
learning/replan settlement. Inline evidence is rejected by the bridge so callers cannot
accidentally bypass the independent evidence boundary.

Deployments with explicit source and evaluator governance can make those decisions mandatory at
the same boundary:

```python
gated = create_autonomous_cycle_evaluator_bridge(
    evidence_for=caller_owned_value_evidence,
    source_receipt_for=lambda context: source_ledger_receipt_for(context),
    evaluator_calibration_for=lambda context: calibration_report_for(context["domain"]),
)
```

`source_receipt_for` must return a validated accepted observation with a source digest and
non-`caller_declared` authority. `evaluator_calibration_for` must return a validated report whose
exact routed evaluator is ready on calibration/holdout evidence. Both callbacks run before the
evidence callback and reward settlement; missing, stale, refused, tampered, or mismatched
metadata fails closed. Only receipt/report digests and bounded source labels enter the context;
the SDK still does not decide external truth, provision credentials, or infer reward from provider
completion.

### Digest-bound next-action handoff

When a caller needs an approval screen or scheduler input without executing the task, use
`agent.action_plan(...)`. It composes the provider-free route, evidence plan, domain policy,
task intent, and task decision into a metadata-only projection with candidate workflows,
approval requirements, policy/review reasons, and one deterministic `next_action`:

```python
action = agent.action_plan(
    task="coordinate a reproducible data and neuroscience review",
    hints=("data", "neuroscience"),
    allow_cross_domain=True,
)
print(action["status"], action["next_action"], action["plan_digest"])
```

Pass `domain="data"` when the caller has already selected a domain; this still creates an
explicit route digest and uses the same domain policy, evidence plan, workflow, and task-decision
contract. `AutonomousActionPlan.from_dict()` verifies the plan and every candidate digest for
restart replay. The projection never contains task text, prompts, provider values, credential
handles, source observations, tool arguments, or authorization. It is a review handoff, not a
replacement for the explicit provider, evidence, connector, tool, evaluator, launch-admission,
queue, or effect boundary.

For the final caller-owned handoff, `agent.admit_action_plan(...)` records explicit approvals
against the plan digest and `agent.execute_action_plan(...)` replays the transient task and route
inputs before delegating to `run_auto()`. Missing review, evidence, plan, effect, evaluator, or
provider gates return an `AutonomousActionExecution` with no credential or provider access. An
admitted plan maps `workflow`, `planning`, `evidence_first`, and `cross_domain` decisions to the
existing execution controls; credentials and all external authority remain caller-owned:

```python
plan = agent.action_plan(task="analyze a bounded dataset", domain="data")
execution = agent.execute_action_plan(
    task="analyze a bounded dataset",
    plan=plan,
    domain="data",
    approvals={gate: True for gate in plan["required_approvals"]},
    reviewed=True,
    credentials=caller_credentials,
)
if execution.status != "completed":
    print(execution.admission.next_action)
```

When the operator process has emitted a verified dispatch handoff, use
`agent.execute_action_handoff(...)` instead of manually unpacking its plan and admission. The
method revalidates the outer digest and embedded identities, replays the transient task/domain
against the plan, reproduces the approved gate set, and delegates to the same `run_auto()`
boundary. It does not turn the handoff into a credential or bypass provider, evidence, tool,
evaluator, or effect controls.

The execution method rejects a changed task or route-options digest and refuses tampered
admission records before dispatch. Its metadata projections retain only plan/admission identity,
selected domains, gate state, and bounded next actions; task text, prompts, credentials, source
values, tool arguments, provider responses, and secret material remain transient.

For a restart-safe operator workflow, persist the handoff through
`InMemoryAutonomousActionAdmissionLedger` and
`TransactionalJsonAutonomousActionAdmissionSnapshotPersistence`. Submit revision one, then call
`ledger.review(...)` with the exact predecessor digest, an explicit reviewer authorization digest,
and the gate approvals. The ledger derives a new admission from the stored plan, increments the
revision, and retains the predecessor link. It refuses stale reviewers, plan/admission mismatch,
tampered status, missing reviewer identity, and admitted records without an operator digest.
Snapshots contain only the redacted plan/admission projections and their digests; the caller still
owns task rehydration, credentials, provider invocation, evaluator truth, and effect authority.

`AutonomousActionAdmissionController` adds the operator view above the ledger. `queue()` projects
all domains and gate state without task text, `review()` requires the deployment's authorization
digest plus the expected current record digest, and `dispatch_handoff()` refuses held/blocked
records while returning the redacted plan/admission projections and downstream gates. It is a
review controller, not an execution or authorization oracle.

For a complete keyless operator process, the CLI now composes planning, durable submission,
optimistic-concurrency review, and downstream handoff:

```bash
python -m prism_sdk action-plan --task "review a bounded dataset" --all-domains > plan.json
python -m prism_sdk action-admission-submit \
  --admission-store action-admissions.json --plan-file plan.json --action-id dataset-review-42
python -m prism_sdk action-admission-status --admission-store action-admissions.json
python -m prism_sdk action-admission-review \
  --admission-store action-admissions.json --action-id dataset-review-42 \
  --authorization-digest "$REVIEW_AUTHORIZATION_DIGEST" \
  --expected-record-digest "$CURRENT_RECORD_DIGEST" --reviewed \
  --approve-gate provider_call
python -m prism_sdk action-admission-handoff \
  --admission-store action-admissions.json --action-id dataset-review-42
```

`action-plan` is provider-free and `--all-domains` compiles the same metadata-only contract for
all twelve domains. The CLI never accepts a key as an argument, restores and validates the
canonical action ledger before each operation, uses atomic compare-and-set persistence, and
persists only plan/admission/reviewer digests. The authorization digest is an external reviewer
identity, not a provider credential. The final handoff lists downstream gates but does not invoke
a provider, source, tool, evaluator, learner, connector, or effect.

Workers should call `validate_autonomous_action_dispatch_handoff()` after loading a handoff from
the queue or another process. It rehydrates the plan and admission, checks their exact digests,
admitted status, selected/requested-domain closure, downstream-gate list, and outer handoff
digest. It proves metadata continuity only; external reviewer authorization, credential and
provider readiness, source truth, evaluator quality, and effect approval remain independent.

The remote worker can bind that verified object directly to a durable job. Prefer
`submit_handoff()` when the action admission is the worker's dispatch boundary: it validates the
handoff, derives `action_plan_digest`, `action_admission_digest`, and `action_handoff_digest`,
and includes all three in the opaque job identity. The resolver may then return only
`action_handoff`; the worker revalidates it after the process boundary, checks domain coverage,
and refuses stale or swapped metadata before it calls a runner. `AsyncRemoteBrainJobWorker`
provides the same `await submit_handoff(...)` path. The handoff digest is continuity metadata,
not a provider credential, reviewer authorization, or execution token.

### One-call deployment-managed execution

When an application has already registered an environment source or secret-manager resolver,
`AutonomousAgent.run_with_provisioned_credentials()` and
`run_auto_with_provisioned_credentials()` compose the complete request boundary. Each call
creates a fresh `CredentialSession`, provisions only opaque handles, optionally refreshes the
authenticated model inventory, runs the normal explicit or automatic/cross-domain path, and
closes the session in `finally` on success, refusal, discovery failure, or provider failure.
The application never passes a raw key to the brain:

```python
agent.register_environment_credential_source("openai", variable="OPENAI_API_KEY")
run = agent.run_auto_with_provisioned_credentials(
    task="review the bounded implementation plan",
    credential_providers=("openai",),
    provision_environ=deployment_environment,
    approve_provider_call=True,
)
answer = run.result                 # transient caller-owned provider result
safe_event = run.to_dict()          # no result text, keys, or handles
```

For a provider-free model gate view, `agent.model_inventory_readiness()` joins the current
catalogue with every built-in domain's reviewed model capabilities, requested token capacity,
provider registration, opaque-credential readiness, and live circuit state. It returns a
digest-bound `ready`/`partial`/`missing` report with compatible and eligible `provider/model` arms
for each domain. This is an operator projection only: it performs no discovery or invocation,
keeps ineligible metadata visible for diagnosis, and does not replace evaluator evidence, launch
admission, or effect approval. The existing `0.1` persisted discovery snapshot remains
backward-compatible; the readiness projection is additive.

For domain-aware structured answers, Python exposes the same twelve-domain response contract as
the TypeScript SDK through `structured_domain_response=True` on `prepare()`, `run()`,
`prepare_auto()`, `run_auto()`, and cross-domain execution. The selected workflow is bound into a
digest-addressed JSON Schema requiring an answer, ordered stage rows, observations, inferences,
uncertainty, evidence gaps, next actions, and domain-specific detail fields. The provider result
is validated in memory and remains caller-owned:

```python
result = agent.run_auto(
    task="review this change and produce a verifiable handoff",
    structured_domain_response=True,
    approve_provider_call=True,
)
response = result.result.response.structured  # transient validated value
evaluation = result.result.response_evaluation  # value-only composition feedback
```

`response_evaluation` scores contract integrity only—stage/reporting coverage, domain-field
coverage, uncertainty and evidence-gap disclosure, and next-action coverage. It is not a truth,
quality, source, or external-effect oracle. `evaluate_autonomous_domain_response()` and
`replay_autonomous_domain_response_evaluation()` provide deterministic offline scoring and drift
detection; no provider call or credential is needed for those operations. Credential-shaped fields
and values are refused before the response can cross into durable learning metadata.

Direct structured execution defaults to admission control: when the response is structurally valid
but its value-only composition score is below the reviewed threshold, the result is returned with
`status="response_review_required"` instead of being projected as completed. The provider response
and evaluation remain available to the caller for review, but transport success is not promoted to
an accepted autonomous answer. Set `require_response_review=False` only when an application needs
the legacy `completed` projection; the failed evaluation remains visible and is not converted into
task-quality reward. Cross-domain children and synthesis defer to the parent fan-in gate.

The same boundary applies when `learn=True`: a weak structured response can update only the
separate value-only response-composition arm, while task-quality evaluation, episodic memory
receipts, and evaluator-guided replanning remain pending behind review. This prevents parseable
JSON from receiving task credit. `require_response_review=False` explicitly restores the legacy
learning projection.

Cross-domain structured runs automatically apply a second fan-in gate before synthesis. The gate
revalidates every specialist against its reviewed response contract and returns a digest-only
`response_assessment` with structural scores, domain coverage, alignment metadata, and bounded
next actions. Provider text and structured values remain on the caller-owned child/synthesis
results; they are not copied into the assessment or execution receipt. Structural admission is
enabled by `structured_domain_response=True` and does not fabricate semantic agreement. Set
`require_response_alignment=True` to require explicit caller/reviewer-owned pairwise alignment
records before synthesis. A missing pair, high-confidence contradiction, unresolved alignment,
low-confidence alignment, weak response, or missing domain coverage returns
`status="response_review_required"`, `execution_receipt.next_action="review_response_gate"`,
and no synthesis result. The thresholds `minimum_response_reward`,
`minimum_response_alignment_confidence`, and `response_contradiction_confidence_threshold` are
bounded fractions for deployments with a stricter review policy. With alignment disabled, the
gate still performs structural admission and includes a third synthesis row after fan-in.

The durable `BrainWorker` applies the same contract at both sides of the fan-in boundary.
`response_review_required` is the pre-synthesis checkpoint and retains only the specialist
assessment digest. If synthesis returns a structurally blocked or otherwise non-complete
response, the worker creates `synthesis_response_review_required`, binding the synthesis outcome
digest and the post-synthesis assessment digest while keeping the result caller-owned. A resolver
must return that exact `completed_synthesis_result` for review-based continuation, or the caller
must set `retry_synthesis_after_response_review=True` to record an explicit retry authorization
before a new provider call. The worker never marks the job complete from a stale or unreviewed
synthesis value, and tampering with either digest fails before dispatch or settlement.

Workflow stages receive the same value-only treatment. A valid structured stage response is
scored by `evaluate_autonomous_workflow_stage_response()` for contract integrity only—stage
identity, status, evidence, uncertainty, bounded notes, next actions, and response-digest
binding. `run_workflow_learning()` and `run_workflow_trajectory_learning()` record this signal
under a separate evaluator and idempotency key, preserving the normal task-quality evaluator and
delayed trajectory credit as independent learning channels. `AutonomousWorkflowCheckpoint`
round-trips the digest-bound evaluation projection without retaining provider text or credentials;
`replay_autonomous_workflow_stage_response_evaluation()` rejects response or evaluator drift.
The stage evaluator is not a truth, quality, source, or external-effect oracle.

The structural signal can be settled into the bandit explicitly after a process boundary. The
settlement only adapts response composition; task correctness still requires the caller's normal
domain evaluator:

```python
blueprint = agent.prepare(
    task="review this change and produce a verifiable handoff",
    domain="coding",
    structured_domain_response=True,
)
run = agent.run(
    task="review this change and produce a verifiable handoff",
    domain="coding",
    structured_domain_response=True,
    approve_provider_call=True,
)
episode = agent.prepare_learning_episode(run, ledger=learning_ledger)
decision, settlement = agent.settle_structured_response(
    run,
    episode=episode,
    bandit_state=agent.learning_state(),
    # Pass the contract when the transient response is available for replay verification.
    contract=blueprint.response_contract,
    ledger=learning_ledger,
)
```

For long-running transcripts, the same agent entry point accepts an explicit context budget:

```python
run = agent.run(
    task="compare the reviewed evidence and produce the next bounded action",
    domain="research",
    model_candidates=model_candidates,
    credentials=credentials,
    approve_provider_call=True,
    context_budget={
        "max_input_tokens": 12_000,
        "preserve_recent_messages": 10,
        "max_messages": 96,
    },
)
```

The budget is applied before selection and after each approved tool result. System/developer
instructions, the latest user task, recent turns, and the newest assistant/tool continuation are
protected; older tool turns are removed only as complete atomic units. The `context_budget` result projection contains
only counts, dropped indexes, structural digests, and retention metadata. Protected overflow
fails closed with `invalid_request`, and no summarization model is called.

For live UI or worker consumption, `AutonomousStreamRuntime.open()` (also available through
`AutonomousBrain.open_stream()`) exposes the same transient provider-neutral event contract with
pre-event-only failover, required terminal `done` events, explicit abandonment, and a
metadata-only completion receipt. See
[`docs/AUTONOMOUS_STREAMING.md`](../docs/AUTONOMOUS_STREAMING.md) for the cross-language contract.

After a restart, rehydrate only `episode.to_dict()` and the value-only
`response_evaluation`; omit `contract` when the provider response is no longer retained. The
settlement refuses altered evaluation digests, mismatched episode/run identities, duplicate
episodes, and credential-shaped values. When `learn=True` is used on the normal provider or
cross-domain learning path, the same structural signal is recorded as a separate
`kind="structured_response"` evaluator update with its own idempotency key, so it cannot collide
with task-quality credit or be mistaken for external truth.

Set `refresh_inventory=True` with explicit `inventory_priors` or an
`inventory_prior_factory` when deployments should discover and reconcile model arms before the
task. Inventory failure is raised instead of silently executing against a stale catalogue.
`run_auto_with_provisioned_credentials()` preserves route abstention, provider planning review,
workflow/cross-domain learning, evaluator, and checkpoint options through the same `run_auto`
surface. `AutonomousProvisionedRun.to_dict()` is metadata-only; `.result` is deliberately not a
durable payload.

Provider planning results carry `planner_context` and `planner_context_digest`, the exact
`{domain, capability, risk_class, task_family}` identity used for contextual model selection.
`AutonomousAgent.settle_planning_quality()` verifies and credits that embedded identity; legacy
planning results without the fields use the explicit settlement arguments for compatibility.

`RemoteBrainJobWorker` is the Python high-level queue adapter when the durable job authority is
remote (HTTP, MCP, or `DurableBrainControlPlaneAdapter`) rather than a local `BrainJobStore`.
`submit()` sends only a bounded idempotency key, composite request/mode/policy digest, domain,
capability, risk class, priority, and attempt ceiling. `run_once()` and `run()` claim the job,
renew its lease, rehydrate the private request through a caller-owned resolver, and dispatch every
supported brain path: `autonomous`, `workflow`, `workflow_learning`, `workflow_cycle`,
`workflow_trajectory_learning`, `cross_domain`, `cross_domain_learning`,
`cross_domain_trajectory_learning`, and `cross_domain_replan`. The resolver is never passed
to the control plane, and the worker rejects spec drift, malformed projections, unsupported modes,
private fields in remote responses, and domain mismatches before dispatch.

Callers may additionally pass `plan_digest` and `route_digest` to `submit()` to bind approval to a
reviewed private blueprint and routing proposal. The helpers
`autonomous_remote_brain_plan_digest()` and `autonomous_remote_brain_route_digest()` hash the
caller-owned value representation; the plan, route, prompt, and credentials still never cross the
remote boundary. If a resolver returns the corresponding private `blueprint` or `route`, the worker
rehashes it before dispatch and refuses a changed plan or route. Both bindings are optional, and
omitting them preserves the pre-extension job digest for existing callers.

The remote worker also accepts `action_plan_digest` and `action_admission_digest`. The resolver
returns the matching metadata-only `action_plan` and `action_admission`; both are parsed and
rehashed before a claim can invoke a runner. The admission must be `admitted`, must reference the
exact plan digest, and must reproduce the composite job digest. This is enforced identically by
the synchronous and asynchronous workers. A stale or swapped plan is rejected in preflight with
zero runner calls, while generic durable provider approval remains a separate gate. Existing
remote jobs omit these fields and retain their original digest behavior.

The remote worker uses the same approval, retry, and uncertainty contract as the local worker. It
parks provider or route approval before dispatch, forces the provider approval bit on the
rehydrated retry, renews leases during long calls, retries only typed preflight failures when
configured, and quarantines post-dispatch uncertainty for explicit `reconcile()` evidence. Raw
tasks, prompts, credentials, provider responses, tool arguments, evaluator payloads, and exception
messages remain caller-owned and are never serialized in `RemoteBrainJobSubmission`,
`RemoteBrainJobRun.to_dict()`, or the remote job journal. `autonomous_remote_brain_job_spec_digest()`
is the shared identity helper for admission and resolver verification. The plan and route digest
helpers provide the optional reviewed-identity bindings without serializing those private values.
`AsyncRemoteBrainJobWorker` exposes the identical contract over `AsyncBrainControlClient`; it
offloads a synchronous `AutonomousBrain` runner to a worker thread (or awaits a native async
runner), so async HTTP/MCP hosts retain responsive event loops without creating a second lifecycle
implementation.

Modern `brain_plan` responses also carry deterministic `execution_waves`, a bounded
`max_parallelism`, `critical_path_cost`, and round/peak-width estimates. Use
`validate_brain_plan_schedule()` or `BrainPlanSchedule.from_plan()` to validate this metadata
before handing it to a caller-owned executor. The validator checks that waves preserve the exact
topological order, dependencies never share or follow an invalid wave, costs agree with the step
graph, and no scheduling field was tampered with. It does not authorize provider/tool work or
provide distributed leases; older projections without these fields remain readable.

Both remote worker variants support bounded draining with `max_parallelism` and
`continue_on_non_terminal`. The default remains serial (`max_parallelism=1`) and the requested
limit is always finite. Increasing the bound claims independent jobs concurrently while each job
keeps its own lease heartbeat, approval gate, protected credential session, result digest, and
post-dispatch reconciliation boundary. The batch projection reports `requested_count`,
`max_parallelism`, and `stopped_on_non_terminal`; approval, retry, or reconciliation backpressure
stops new claims by default after already-started leases settle. Set
`continue_on_non_terminal=True` only when the deployment deliberately wants to continue through
other queued jobs:

```python
batch = worker.run(
    limit=32,
    max_parallelism=8,
    continue_on_non_terminal=False,
)
```

The async worker uses the same controls with `await worker.run(...)`. Parallel scheduling never
places task text, prompts, provider responses, evaluator values, credentials, or tool payloads in
the remote job or batch metadata.
Contextual model adaptation is shared with the Rust and TypeScript contracts: the canonical
domain/capability/risk/task-family digest selects a nested contextual arm ledger, while global arms
remain only a cold-start prior. Evaluator settlement sends the digest and bounded context identity
through `brain_outcome_record`; replay receipts bind that identity, so reward from one domain cannot
silently update another domain's model policy.
`AutonomousEvaluatorMesh` is a drop-in `BrainOutcomeEvaluator` for higher-assurance runs: it calls
two through eight caller-owned evaluator members over the same projected input, accepts only bounded
reward spread plus pass/fail, failure-class, and replan agreement, and refuses bandit credit on
disagreement or member error. Its detailed result and value-only replay seam retain member identities
and digests only, so the same quorum gate applies to provider runs, tool loops, missions, workflows,
and all twelve autonomous domain contexts.

`AutonomousAgent.capability_portfolio(task)` is the task-aware tool admission proposal for the
Python façade. It selects a bounded exact-name portfolio by reviewed workflow-stage coverage,
capability, local relevance, read-only posture, activation state, and stable tie-breaks. Task text is
used transiently and represented publicly by a digest; the portfolio never invokes a provider or
tool and never grants authority. Automatic `run()`, workflow, and cross-domain paths use it when
the caller has not supplied explicit provider tools, while custom tools remain a compatibility
fallback when no reviewed candidate is available. Missing catalogue entries and activation-gated
stages are explicit rather than optimistic.

The proposal can also enforce a caller-selected risk ceiling with
`max_risk_class="read_only" | "reversible_effect" | "external_effect" | "high_impact_effect"`.
Risk, read-only, approval, allow-list, and learning-disabled checks are separate eligibility
gates. Each coverage row exposes a bounded `candidate_ranking` with the best eligible choice and
the highest-priority rejected alternative, plus `selection_rationale` and
`selection_constraints`. Candidate rows retain only reviewed tool identity, posture, deterministic
relevance/utility metadata, value-only pull/failure counters, and typed rejection reasons such as
`risk_budget_exceeded` or `approval_required`; task text, prompts, arguments, outputs, credentials,
and evaluator prose remain absent. The two-candidate stage window preserves the existing 128 KiB
all-domain plan bound while making risk-budget decisions auditable. `run()`, automatic workflows,
and cross-domain child/synthesis blueprints propagate the same ceiling without turning selection
into authorization.

`AutonomousAgent.execute_capability(...)` is the deterministic application seam for a capability
already selected by a plan, workflow, queue, or operator. It rechecks the exact registered tool
schema, domain/stage contract, approval posture, and caller-owned executor, then returns the raw
adapter value only transiently. `AutonomousCapabilityExecutionRecord` and the optional
`InMemoryAutonomousCapabilityJournalStore` retain only bounded metadata, digests, observation
labels, evidence completeness, and replay identity. `execute_capability_batch(...)` supports up to
64 ordered requests with bounded parallelism and in-flight deduplication; after restart,
`restore_capability_journal()` rehydrates completed replay barriers without replaying adapters.
Independent evaluator evidence remains required before online-learning or bandit credit.

`evaluate_capability_execution(...)` and `evaluate_capability_executions(...)` complete this
boundary: they send only metadata/digests and bounded caller evidence to an explicit evaluator,
settle a value-only bandit update, and use the learning ledger as a restart-safe idempotency
barrier. `reconciliation_required` results cannot receive credit until the caller explicitly
resolves the uncertain effect. Adapter values, prompts, arguments, credentials, and raw evidence
are never passed to the evaluator or retained in the settlement report.

The shared caller-owned bandit state supports `ucb1`, `epsilon_greedy`, and deterministic
`thompson_sampling` policies. Thompson selection forms a fractional Beta posterior from explicit
evaluator rewards, emits posterior metadata for audit/replay, and still respects capability,
cost, credential, circuit, and approval gates. Adaptive model selection also accepts an optional
`min_selection_confidence` floor; near-tied eligible ranks fail closed with a typed abstention
instead of becoming an overconfident provider call. The value is routing stability, not answer
correctness. Provider transport success never becomes reward.

Approval-required missions are never completed as proposals: the worker parks them in
`waiting_approval`, records only a request digest and bounded scope, and requeues them only after
the caller-authenticated approval router releases the checkpoint. The runtime also exposes an
optional value-only provider observation callback; it can report transport failures without
exposing prompts, responses, headers, credential handles, or raw provider payloads.

BrainControlClient and AsyncBrainControlClient expose the same control-plane lifecycle over
the existing HTTP ApiClient/AsyncApiClient or stdio Client/AsyncClient. Their typed request
objects compute and verify replay evidence digests locally, require proof digests for approval
decisions, and emit only bounded job metadata, health values, and normalized evaluator signals.
They are intentionally not credential clients: collect keys through ProviderOnboarding, invoke
through LLMRuntime, and report only value-only outcome metadata back to the control plane.

For a restart-safe application-owned host, `DurableBrainControlPlaneAdapter` exposes the same
`brain_job_*` tool names over `BrainJobStore` rather than duplicating the state machine in an HTTP
handler. It supports deterministic priority-ordered `claim_next` dequeue and cancellation that
quarantines jobs after a dispatched/unknown side-effect boundary. Mutations fail closed until the
host supplies an authorization callback; the callback receives only operation metadata and
digests. `AsyncDurableBrainControlPlaneAdapter` runs the SQLite transaction in a worker thread
for async hosts. Both adapters project idempotency keys, checkpoint bodies, failure text, result
metadata, cancellation reasons, and reconciliation notes as digests only:

```python
from prism_sdk import (
    BrainControlClient,
    BrainJobStore,
    DurableBrainControlPlaneAdapter,
)

with BrainJobStore("brain-jobs.sqlite3") as store:
    transport = DurableBrainControlPlaneAdapter(
        store,
        authorizer=lambda operation, metadata: application_policy_allows(operation, metadata),
    )
    control = BrainControlClient.from_durable(transport)
```

The adapter does not collect or inspect provider keys and does not rehydrate prompts, plans,
responses, or domain evidence. The embedding application remains responsible for its protected
resolver, identity provider, HTTP/MCP authentication, and external-effect verification.

The package also includes dependency-free authoring builders for digest-bound benchmark packs,
set-valued decision cells, deterministic metamorphic mutations, versioned oracle manifests,
evidence judgements, reference panels, evidence-conditioned BioCapability audit requests, evaluation requests, and typed metrics observations,
paired contrasts, and calibration forecasts. They validate local JSON and cross-field invariants,
then let `Workspace` delegate final decisions and arithmetic to the Rust kernel through
`pack_health_assess()`, `mutation_family()`, `oracle_combine()`,
`metrics_analytics_audit()`, `biocapability_evidence_audit()`, `bioql_compile()`, `world_claim_check()`, `lab_plan()`, `routing_decide()`, `fiber_compile()`, `fiber_refine()`, `fiber_explain()`, `fiber_verify()`, `projection_bundle()`, `repository_catalog()`, `repository_bundle()`, `repository_impact()`, `telemetry_project()`, `developer_workbench()`, `developer_workbench_verify()`, `developer_workbench_import()`, `developer_workbench_query()`, `developer_workbench_get()`, `agent_mission()`, `capability_discover()`, and the evaluation helpers.
The repository helpers preserve bounded discovery and route completeness; telemetry requires an
explicit redaction policy and observed metric inputs. The FIBER helpers preserve the progressive-disclosure lifecycle: compile the minimal contract,
refine only when necessary, inspect omissions before trust, verify certificates, and opt into full
projection bodies explicitly.
The mission layer lets an agent preview or explicitly execute a bounded, allow-listed graph across
the existing domain tools while retaining refusals and blocking dependent work. The workbench
keeps authoring/notebook sessions, stale digests, capability holes, release posture, and review-only
CI planning in one evidence-bearing response; it does not pretend to execute a hosted UI or GitHub
runner. The registry helpers retain structurally valid workbench reports behind bounded digest
queries and restart-safe checkpoints; they never execute or re-evaluate the retained report.
The high-level agent also exposes `plan_ordered_steps_with_provider()` for mission and scheduler
graphs that are not represented by an `AutonomousWorkflow`. It validates a bounded graph before
provider contact, derives a stable task/plan identity, selects through the contextual model and
credential gates, and returns only a digest-bound `AutonomousOrderedStepPlanRefinementResult`.
The provider may reorder existing IDs and choose a focus subset, but cannot add steps, alter
dependencies, authorize tools, or create effects; caller acceptance and mission preflight remain
separate gates. Provider, credential, malformed-response, abstention, and policy outcomes stay
explicit and safe to persist, while step objectives and provider messages remain transient.
Completed ordered-step proposals can be passed to `AutonomousAgent.settle_planning_quality()`
with the same evaluator and contextual-bandit contract as workflow and cross-domain proposals.
The protected graph-contract digest is included in the planning learning identity, so evaluator
credit cannot replay across two different caller-owned graphs; transport success alone still earns
no reward.
For a typed connector `MissionRequest`, `AutonomousAgent.run_connector_mission_with_provider_planning()`
provides the corresponding two-phase bridge: the provider receives only the redacted step
catalogue, a completed non-review proposal must match the order-independent protected mission
digest and dependency graph, and `accept_plan=True` is still separate from connector
`approved=True`. Review-required proposals return without connector calls. On restart, callers can
pass the previously retained `accepted_plan_refinement` to avoid replaying the planner provider;
the connector checkpoint and output rehydration rules remain unchanged.
`run_connector_mission_with_provider_planning_and_launch_admission()` performs the same two-phase
handoff only after all mission domains pass the process-boundary launch admission.
The provider-evidence registry helpers extend the same retention contract to provider-observed CI
artifacts, logs, and attestations. They re-audit before import, preserve failed/unknown runs, expose
digest-ordered provider/run/plan queries and exact lookup, and carry separate record-family digests
for lineage joins. Query requests can require minimum local-byte hash and attestation subject-digest
binding counts, and result rows retain those counts. The shared registry is bounded and restart-safe when configured, but it never
fetches remote bytes, authenticates a provider, verifies provider signatures, or approves a release.
The typed `BundleVerifyArgs`/`BundleVerifyReport` surface also supports the bundle layer's explicit
Ed25519 public-key verification while keeping key-registry identity and release authority external.
`MissionBinding` supports validated field-level dataflow between direct prerequisite steps,
and `CapabilityQuery` routes across the complete domain catalogue with optional tool schemas;
`capability_audit()` verifies the catalogue against the authoritative MCP schema set, and
`capability_route()` batches named needs without executing the returned candidates.
`capability_route_plan()` composes caller-selected candidates into a route review plus authoritative
mission preflight, returning the generated mission, `plan_digest`, schema diagnostics, and explicit
`dispatch: "not_started"` status without dispatching any nested tool. Its typed
`CapabilityRoutePlanRequest` rejects `policy.execute=True`; `CapabilityRoutePlanReport` preserves
blocked route or preflight outcomes for inspection before `agent_mission`.
`mission_preflight()` now reviews that graph locally against a live `ToolCatalogue`: it reports
content digests, deterministic waves, missing/cyclic dependencies, binding targets, execution
allow-list and side-effect policy findings, and per-step schema warnings before any mission POST.
Set `MissionPolicy(execution_mode="parallel_waves", max_parallelism=4)` to request bounded
concurrent batches for independent steps in each wave; serial execution remains the default, and
preflight reserves the worst-case per-wave output budget before the Rust executor can launch it.
It is a transport and orchestration review only; the Rust `agent_mission` tool remains authoritative
for actual execution and refusal propagation. The Rust boundary validates known tool arguments
against the authoritative `tools/list` schema before dispatch, and repeats that check after
bindings are materialized; schema refusals include bounded pointer diagnostics and a schema digest.
Executed reports carry a clock-free
`execution_trace`; `MissionExecutionReport.from_wire()` validates contiguous sequencing and
mission lifecycle boundaries without hiding the raw report. `mission_from_route()` then turns a completed
`capability_route()` response into a provenance-preserving `MissionAssembly`, but only after the
caller selects exactly one candidate tool and supplies its arguments for every need; unresolved
and out-of-candidate selections are refused rather than guessed.
The HTTP clients additionally expose `submit_mission()`, `mission_status()`, and
`cancel_mission()` through a typed `MissionJob`. Cancellation is cooperative: it prevents future
step or parallel-batch dispatch while allowing an in-flight tool to return, and the terminal report
records cancelled steps instead of implying rollback. `delete_mission()` releases a terminal job
from the bounded process-local registry.
`preflight_mission()` calls the synchronous Rust-owned `/v1/missions/preflight` route; it validates
the original execution policy and returns a planned report without creating a job or dispatching
any domain tool. The existing `mission_preflight()` method remains the local catalogue review.
`missions(status=..., limit=...)` lists bounded lifecycle summaries for operator dashboards without
materializing unbounded terminal reports.
Each `MissionJob` also exposes an optional typed `MissionProgress` projection for queued, running,
and terminal views. It includes phase, wave, active/completed counts, outcome counters, returned
bytes, and the latest trace sequence/event; the authoritative terminal report remains the source of
truth for replay and domain interpretation.
Executable jobs additionally expose `MissionJob.execution_provenance`; the sync and async
`mission_provenance()` helpers read its retained review, gate digest, domain-evaluator evidence,
and accepted-dispatch event correlation without converting it into a readiness claim.
`MissionRequest.claim_requests` adds bounded caller-authored claim rows with explicit required
steps and evidence mode. Terminal reports preserve the non-semantic `claim_lineage` projection,
and `mission_claim_lineage()`/its async counterpart read the dedicated `/claims` route as a typed
`MissionClaimLineage`; each claim may add explicit `MissionClaimEvaluatorBinding` rows for
adapter/domain/output-pointer coverage. Retained output is evidence posture only, never claim truth.
`mission_trace(mission_id, after=..., limit=...)` pages the retained authoritative trace through a
typed `MissionTracePage`; `gap` and `dropped_events` remain visible when a cursor falls behind the
bounded retention window.
For any current or future domain without a handwritten wrapper, `tool_catalogue()` snapshots the
live `tools/list` or `/v1/tools` definitions, `plan_tool()` performs conservative shape-only
preflight, and `tool_checked()` executes the reviewed call. Plans are digest-bound and carry no
domain-success claim; unsupported schema keywords remain warnings, and remote refusals remain
distinct from transport validation.
`AdapterRegistry` and `adapter_plan()` add a dependency-free format boundary for tabular and
biological sources: explicit DICOM, NIfTI/BIDS, AnnData/Zarr, VCF, FASTA, FASTQ, SAM, GFF3, PDB, SDF/MOL, mzML, BAM/CRAM, OME-Zarr, and FHIR routes
are delegated to the mature Python ecosystem, while dependency missingness, scope dimensions, and
semantic-loss declarations remain visible before parsing. The planners never sniff or fetch bytes.
`BidsAdapter` and `audit_bids()` add a dependency-free BIDS manifest path: they validate bounded
relative paths, entities, directory labels, JSON sidecar inheritance and conflict precedence,
participant coverage, task metadata, and derivative descriptions. They accept parsed projections
and never claim to parse NIfTI or other binary image bytes.
`DicomAdapter` and `audit_dicom()` provide the corresponding parsed DICOM projection audit: they
check UID hierarchy, duplicate SOP instances, dimensions, frame-of-reference and image geometry,
slice spacing, enhanced multi-frame positions, provenance, and bounded privacy-safe summaries.
Structural validity is reported separately from publishability when coordinate or provenance loss
is blocking; pixel decoding remains an explicit optional-reader responsibility.
`NiftiAdapter` and `audit_nifti()` add the header/affine counterpart: shape, datatype, qform/sform,
affine invertibility, voxel-size agreement, units, axis codes, series consistency, provenance, and
privacy-safe affine digests are checked without reading image arrays or compressed files.
`AnnDataAdapter` and `audit_anndata()` add the single-cell/multimodal projection counterpart:
`n_obs`/`n_vars`, X/layer sparse metadata, obs/var indices and annotations, embeddings, pairwise
matrices, raw dimensions, `uns` summaries, provenance, and index digests are checked without reading
HDF5/Zarr chunks or matrix values.
`AlignmentAdapter` and `audit_alignments()` add the parsed BAM/CRAM projection counterpart: explicit
reference dictionaries, CIGAR spans, 0-based coordinate bounds, flags, mate pairing, sort order,
mapping qualities, coverage, reference-build provenance, and read-identity digests are checked
without decoding sequences, qualities, auxiliary tags, indexes, or reference bases.
The parsed FHIR projection audit now checks Bundle structure, resource identity, profile
declarations, duplicate resource keys, privacy-safe patient/resource references, bounded nesting,
and provenance. It rejects duplicate JSON keys and non-standard numbers before auditing raw files;
profile invariants, terminology expansion, clinical values, narratives, extensions, and external
reference resolution remain explicit limitations.
`AdapterRuntime`, `ProjectionRequest`, and `execute_projection()` provide one typed gateway over
the complete set of concrete projection audits. They normalize successful, lossy, invalid, blocked, rejected,
and unsupported outcomes; unavailable raw-byte routes refuse explicitly without fallback, and the
request envelope records payload keys rather than echoing payload values.
`ProjectionBatchRequest`, `ProjectionBatchResult`, and `execute_projection_batch()` compose
heterogeneous FHIR, imaging, single-cell, variant, alignment, and manifest requests while keeping
each member's document, refusal state, digest, and semantic-loss classification. Aggregates also
report adapter/failure counts, validity/publishability totals, scope dimensions, and visible versus
omitted semantic-loss evidence. Batch limits and stop-on-error omissions are explicit, and an
incomplete batch is not accepted as complete.
Each `AdapterExecutionResult` can be converted with
`to_adapter_execution_evidence_request(...)` into the shared MCP/HTTP evidence handoff. The
bridge requires caller-supplied subject identity and source/input digests, retains output and
semantic-loss evidence, and preserves rejected, blocked, and dependency-missing outcomes. Batch
conversion requires a digest map for every source id, so evidence coverage cannot be inferred from
successful members or from source/subject label overlap.
Source-backed projections add `SourceAdapterProjectionResult.to_adapter_execution_evidence_request(...)`.
It verifies the explicit parser-input digest against the retained raw-content digest, preserves
source-plan and response parents, carries partial transport state into execution evidence, and
retains truncated/binary/omitted-body refusals even when no parser ran. A declared adapter version
is required for those pre-parser refusals; no source locator is reopened.
`submit_adapter_execution_evidence(...)`, `submit_projection_batch_evidence(...)`, and their async
counterparts then hand these requests to `ApiClient`/`Workspace`-compatible sinks. Batch conversion
happens before network calls; `continue_on_error=True` preserves per-source transport failures,
while a returned remote refusal remains a retained, typed report rather than a transport error.
`AdapterConformanceProfile` covers every concrete runtime route with family-specific required
checks. `evaluate_adapter_conformance(...)` reports `verified`, `partial`, `unsupported`, or
`refused` without promoting structural parsing into clinical, biological, or release readiness;
its stable report digest can be carried as an explicit evidence parent.
`domain_report_from_adapter_execution(...)`,
`domain_report_from_provider_normalization(...)`, and
`domain_report_from_external_provider_normalization(...)` compose those observations into the
canonical `DomainReportProjectRequest`. They use declared cross-domain MCP tool names at the
report boundary while retaining typed adapter/provider identity inside the payload, and retain typed evidence, refusal/observed posture,
caller lineage, and non-claims about execution, authenticity, scientific validity, and readiness;
external-payload bridges preserve receipt/materialization lineage and never reopen a locator.
`ApiClient`, `AsyncApiClient`, `Workspace`, and `AsyncWorkspace` additionally expose
`domain_report_from_adapter_execution(...)`, which sends the typed request through the canonical
`domain_report_project` operation and returns the combined `AdapterDomainReportResult` with
evidence-to-report lineage and the server's explicit non-readiness boundary.
The same transport facades expose
domain_report_from_provider_normalization(...) and
domain_report_from_external_provider_normalization(...) for inline and receipt-verified
provider evidence; both return ProviderDomainReportResult with explicit mode, digest lineage,
compact report evidence, and no locator reopening or readiness claim.
Provider normalization reports now expose a parallel evidence handoff: payload, request, shape,
row-index, intake, catalogue, and normalization digests become explicit parents/output identity,
while connector outcome and structural status remain distinct. External receipt-verified
normalization adds receipt/materialization and byte-length lineage; callers still provide adapter
and source identities, and no provider authenticity or external execution is inferred.
When installed, `read_nifti_header()` and `read_anndata_projection()` provide verified raw-file
bindings for nibabel and anndata-backed H5AD/Zarr metadata. They feed the same auditors without
loading image arrays or matrix values; missing optional packages remain typed refusals.
The same runtime has dependency-gated pydicom and pysam bindings for metadata-only DICOM,
indexed/compressed VCF/BCF, and BAM/CRAM records. Each feeds the corresponding audited projection;
absent pydicom/pysam packages produce explicit unsupported results.
`OmeZarrAdapter`, `audit_ome_zarr()`, and `read_ome_zarr()` cover multiscale axes, level shapes,
chunks, scale/translation transforms, channels, labels, and provenance using only Zarr metadata;
image chunks and pixel values are not loaded.
The FHIR JSON and Bulk Data NDJSON readers are dependency-free and use the same auditor for raw
files and parsed documents; every NDJSON record is validated, and no patient identifiers are
echoed in the projection.
`parse_fastq()` and `read_fastq()` add a dependency-free sequencing-read boundary: multiline records,
quality lengths, printable quality ranges, duplicate identifiers, and paired-read completeness are
validated while read identifiers, bases, and qualities remain source-bound digests or aggregates.
`parse_sam()` and `read_sam()` add a dependency-free alignment boundary: headers and sequence
dictionaries, flags and mate evidence, CIGAR query/reference accounting, coordinate bounds, typed
optional tags, and declared coordinate sort order are audited without disclosing read names,
reference labels, sequences, qualities, or tag values. Binary BAM/CRAM remains a separate
dependency-gated route.
`parse_fasta()` and `read_fasta()` add a dependency-free reference/assembly boundary: multiline
records, duplicate sequence identifiers, optional nucleotide/protein alphabet claims, lengths, symbol
counts, and GC bases are audited while sequence strings and headers remain source-bound digests.
`parse_gff3()` and `read_gff3()` add a dependency-free annotation boundary for GFF3/GTF-style rows:
coordinates, scores, strands, phases, URL-encoded attributes, duplicate IDs, Parent resolution,
cycles, directives, and embedded FASTA boundaries are audited without disclosing attribute values.
`parse_bed()` and `read_bed()` add a dependency-free interval boundary for BED3--BED12 rows:
zero-based half-open coordinates, optional score/strand/thick/RGB fields, transcript-style block
geometry, duplicate intervals/names, coordinate ordering, and source-bound chromosome/name digests
are audited without disclosing labels or track metadata. Assembly identity remains caller-supplied.
`parse_pdb()` and `read_pdb()` add a dependency-free structural-biology boundary: fixed-column atom
fields, models, chains, residues, coordinates, alternate locations, crystallographic cells, resolution,
CONECT edges, and geometry summaries are audited without emitting raw structure records.
`parse_sdf()` and `read_sdf()` add a dependency-free small-molecule boundary for bounded MDL V2000
records: atom/bond counts, elements, formal charges, isotopes, radicals, connected components,
coordinates, duplicate data fields, and source-bound molecule/graph digests are audited without
disclosing molecule names, property values, or raw molfile records. V3000 records are refused
explicitly instead of being guessed.
`parse_mzml()` and `read_mzml()` add a dependency-free mass-spectrometry boundary: bounded XML,
spectrum identity, declared counts, MS levels, scan-time summaries, binary-array types, compression,
precision, and encoded-length evidence are retained while binary m/z/intensity/time arrays are never
decoded or emitted.
`parse_vcf()` provides the first concrete Python biological reader: it performs bounded structural
and typed VCF validation, preserves raw values, hashes source and disclosed records, and reports
reference-build, provenance, type, and precision limitations with source locations. It validates
the full record stream even when callers request only a bounded preview; indexed or compressed VCF
access remains an explicit `pysam` adapter responsibility.
`BenchmarkObservation`, `summarize_distribution()`, and `paired_effect()` provide reproducible
notebook-side distribution and paired-contrast ergonomics across agent, biological, multimodal,
operations, and coordination domains. They keep unmeasured evidence out of arithmetic and make
bootstrap seed, confidence, resampling unit, and limitations explicit; they do not perform
significance testing or causal inference.

`agent.domain_audit()` (or `audit_autonomous_domain_contracts()`) is the provider-free
pre-dispatch gate for the reviewed autonomous surface. It checks every built-in domain's profile
schema, default capability, workflow identity and DAG, stage evidence/evaluator contracts, exact
tool binding posture, and caller-owned evidence coverage. Supplying live tool names and evidence
identifiers upgrades runtime status from `unassessed` to `ready_for_review` or `partial`; the
audit never resolves credentials, invokes a model, acquires a source, executes a tool, or mutates
learning. Rows and the aggregate report are SHA-256 digest-bound metadata only.

```python
report = agent.domain_audit(
    available_tool_names=["repository_catalog", "engineering_manifest_audit"],
    available_evidence=["scope", "acceptance_criteria"],
)
validate_autonomous_domain_audit_report(report)
```

`agent.launch_preflight()` composes that structural audit with the agent's model/provider
readiness and deployment-owned capability gates into one twelve-domain review artifact. Each row
has a combined `blocked`, `partial`, or `ready_for_review` state and bounded next actions; the
report also includes source-report digests and an explicit zero-dispatch ledger. It is useful for
startup dashboards and approval UX, but it never authorizes a provider, source, tool, effect, or
learning update.

`agent.launch_admission(preflight, decision="approve", authorization_digest=...)` records the
next explicit caller review boundary against that exact preflight digest. It projects an admission
state for every domain (`approved`, `held`, `blocked`, or `not_selected`) and stores only the
authorization/reason digests, bounded actions, and gate identities. A blocked or partial preflight
cannot become approved through this method, and the admission record still does not grant provider,
source, tool, queue, learner, credential, or effect authority; the deployment-owned executor must
bind it to its own authorization and dispatch policy.

For an execution-bound check, use `authorize_autonomous_launch_domains(...)` or the facade methods
`run_with_launch_admission(...)` and `run_cross_domain_with_launch_admission(...)`. They enforce
approval before credential resolution/orchestration and still require the ordinary provider,
tool, learner, and effect approvals.
The focused and workflow facades expose the same ordering through
`run_capability_with_launch_admission(...)`,
`run_workflow_with_launch_admission(...)`,
`run_workflow_with_trace_and_launch_admission(...)`, and their learning/cycle/trajectory
variants. Cross-domain learning and replanning variants validate every specialist domain before
they enter the shared credential or execution controller.
The same process gate is available for direct learning, traced runs, evidence-backed and
resumable evidence execution, connector workflows/missions, and reviewed capability dispatch:
`run_learning_with_launch_admission(...)`,
`run_with_trace_and_launch_admission(...)`,
`run_cross_domain_with_trace_and_launch_admission(...)`, the
`run_with_*_evidence_with_launch_admission(...)` and
`run_resumable_*_evidence_with_launch_admission(...)` variants,
`run_connector_workflow_with_launch_admission(...)`,
`run_connector_mission_with_launch_admission(...)`, and
`execute_capability_with_launch_admission(...)`/`execute_capability_batch_with_launch_admission(...)`.
They authorize before trace, evidence, connector, tool, learner, credential, or provider setup;
omitted evidence domains are conservatively treated as the complete twelve-domain scope.
Provisioned credential execution, approved model-arm invocation, direct connector dispatch, and
workflow portfolio/evidence execution have corresponding gates:
`run_with_provisioned_credentials_with_launch_admission(...)`,
`run_auto_with_provisioned_credentials_with_launch_admission(...)`,
`run_approved_model_selection_with_launch_admission(...)`,
`dispatch_connector_with_launch_admission(...)`, and the
`execute_workflow_portfolio*_with_launch_admission(...)` variants. They check the reviewed
domain set before opening credentials or entering the provider, connector, tool, or evidence
runtime.
`run_auto_with_launch_admission(...)` provides the same gate for automatic single/cross-domain
routing and refuses provider-assisted semantic routing until that classifier boundary is separately
reviewed.
The batch counterparts (`run_batch_with_launch_admission(...)`,
`run_auto_batch_with_launch_admission(...)`, `run_cross_domain_batch_with_launch_admission(...)`,
and `run_resumable_batch_with_launch_admission(...)`) preview every item route first and bind one
admission to the complete selected-domain union before credential resolution, checkpoint
rehydration, or provider dispatch. Per-item option factories are evaluated once and replayed after
admission; automatic batches reject semantic routing unless it is separately reviewed.

`run_resumable_batch(..., mode="auto")` now binds an automatic-execution policy digest into its
metadata-only checkpoint. The digest covers normalized model candidates, route controls, planning,
prompt and learning controls, connector/tool/structured-output options, domain-policy gates,
approval, workflow, and decision-cycle settings without persisting their values. Changed
automatic controls fail closed before protected-result rehydration or a new provider call.
`AutonomousAutomaticBatchProtectedRehydration` is the strict auto-mode receipt adapter, and
`AutonomousBrainBatchJobController(..., automatic_protected_rehydration=...)` uses it for restart
recovery while explicit `rehydrate_result` callbacks retain precedence.

The CLI exposes the same handoff with `--launch-admission-file` on `run` and `batch-run`. The file
must be a bounded, digest-verified `agent.launch_admission(...)` record. The CLI checks its status
and route coverage with an offline agent before collecting a user credential or opening MCP, then
passes the same record into the SDK execution gate. Batch mode previews each request and verifies
the union of explicit, automatic, or cross-domain domains; tampering, held/blocked status, and
under-scoped approval fail closed. The returned CLI projection contains only admission identity,
status, digest, and approved-domain metadata. It never exposes the approval reason, task text,
prompt, credential, or provider value.

`AutonomousBrainControlPlaneMonitor` and `AsyncAutonomousBrainControlPlaneMonitor` provide the
operator-side lifecycle for jobs returned by `BrainControlClient`. They fan out bounded status
reads across the twelve domains, validate hash-chained event cursors, issue explicit approval
requests/decisions, and wait with bounded polling and restart cursors. Remote projections are
validated as metadata-only before being returned; task text, prompts, credentials, provider
responses, tool arguments, and effect values are refused at the monitor boundary.

See
[`docs/PYTHON_SDK.md`](../docs/PYTHON_SDK.md) for the full authoring contract.
