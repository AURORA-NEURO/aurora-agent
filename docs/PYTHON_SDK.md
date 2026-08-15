# Python SDK

This document covers blueprint modules **11.04** (Python SDK), **11.05** (Python benchmark
authoring SDK), **11.15** (evaluator/oracle/mutation SDK), and **11.16** (environment and pack
authoring SDK). It also documents the typed client for the metrics analytics bridge. The package
remains dependency-free and keeps Rust authoritative for canonical bytes, scientific invariants,
metric arithmetic, oracle decisions, and release gates.

The repository ships `python/prism_sdk`, a standard-library client for the Rust MCP server. It is
the integration layer above the deterministic kernel described by [ADR-001](ADR-001-language-strategy.md):
Python can orchestrate and author requests, while Rust remains the owner of canonical bytes,
domain invariants, release gates, and evidence semantics.

## Lifecycle

Both clients enforce the MCP sequence:

```text
construct argv -> start child without a shell -> initialize -> notifications/initialized
       -> tools/list / tools/call / resources/read -> close stdin -> terminate or kill child
```

`Client` is synchronous and `AsyncClient` uses `asyncio.create_subprocess_exec`. Neither accepts a
shell command string. The caller supplies an argv sequence, an optional working directory, and an
optional environment overlay.

Every frame is UTF-8 JSON followed by one newline. Outbound and inbound frames have a default
20 MB bound, and every response has a finite timeout. A malformed frame, mismatched request id,
missing result, unexpected EOF, or process exit is a protocol/transport failure—not an empty tool
result.

## Error classes

The package distinguishes:

| Class | Meaning | Retry implication |
|---|---|---|
| `ArgumentError` | local argument or frame bound failure | fix the call; no request was sent |
| `LifecycleError` | start/initialize ordering error | fix client usage |
| `TransportError` / `ProcessExited` | process or stdio failure | retry only after inspecting process state |
| `ResponseTimeout` | peer exceeded the configured bound | retry only if the operation is safe to repeat |
| `ProtocolError` | peer violated JSON-RPC/MCP shape | do not interpret the result as evidence |
| `RemoteError` | JSON-RPC method-level error | use `code`, `message`, and `data` |
| `ToolRefusal` | valid tool payload with `ok: false` or `isError` | preserve the refusal; do not treat it as success |
| `ApiError` | HTTP gateway returned a bounded structured error | inspect status/payload; do not retry a domain refusal blindly |

`ToolResult` retains the raw MCP envelope, exposes all text blocks, decodes the server's JSON
projection, and provides `require_ok()` for callers that explicitly want an exception on a refusal.
Callers that need to render partial evidence can use `value()` without discarding the raw envelope.

## Domain helpers

`Workspace` and `AsyncWorkspace` are thin, typed facades. They do not duplicate domain models or
invent defaults:

- `developer_delivery_audit(...)` forwards exact nested evidence and creates a release request only
  when the caller supplies an id and target list.
- `bioatlas_publication_audit(atlas, ...)` preserves optional evidence, card, leaderboard, and
  weighting contracts and never implies publication without explicit targets.
- `BioAtlasPublicationAuditReport.from_wire(...)` plus the sync, async, and HTTP
  `bioatlas_publication_audit_report(...)` helpers type atlas aggregation, evidence-conditioned
  score gates, card/leaderboard availability, ranked/unranked counts, and explicit publication-target
  blockers. A ready target is contract eligibility, not publication, clinical authority, or network
  deployment.
- `BioCapabilityEvidenceAuditRequest`, `EvidenceItem`, and `ClaimRequest` provide a bounded typed
  input for `Workspace.biocapability_evidence_audit(...)` and its async/HTTP counterparts. They
  enumerate the nine evidence dimensions, keep observed/reproduced support distinct from declared
  status, validate claim prerequisites and duplicate IDs, and preserve optional information-value,
  reference, temporal, and reproducibility subaudits without deciding scientific truth locally.
- `BioCapabilityEvidenceAuditReport.from_wire(...)` plus the sync, async, and HTTP
  `biocapability_evidence_audit_report(...)` helpers type the returned evidence inventory, dimension
  rollups, domain counts, claim blockers/assumptions, omission bounds, optional subaudits, and
  fail-closed release posture. `report.ready_for_requested_claims` remains evidence of an internally
  complete contract, never a scientific or clinical truth claim.
- `BioQlCompileRequest` and `Workspace.bioql_compile(...)` provide the same bounded entry point
  over sync MCP, async MCP, and HTTP. Query text and explicit schema JSON are size-checked locally;
  lexical, unit/frame, temporal, provenance, access-label, and cost semantics remain authoritative
  in Rust, and compilation never executes a query.
- `WorldClaimCheckRequest`, `LabPlanRequest`, and `RoutingDecisionRequest` expose typed envelope
  helpers for world support checks, no-execution acquisition planning, and unseen-task routing over
  sync MCP, async MCP, and HTTP. They bound serialized maps, action/evidence counts, budgets, and
  task identity while leaving provenance support, privacy crossings, reachability, abstention, and
  safe-default decisions to Rust.
- `FiberCompileRequest`, `FiberRefineRequest`, `FiberExplainRequest`, `FiberVerifyRequest`, and
  `ProjectionBundleRequest` make the full FIBER progressive-disclosure lifecycle typed across sync
  MCP, async MCP, and HTTP. `Workspace.fiber_compile(...)` validates relative world/query paths and
  l0--l4 layers; `fiber_refine(...)` requires either a bounded content-addressed handle or an
  explicit world/query pair; `fiber_explain(...)` exposes the plan and omission rationale;
  `fiber_verify(...)` checks a certificate before trust; and `projection_bundle(...)` keeps complete
  view bodies opt-in. Rust remains authoritative for compilation, sufficiency, omission accounting,
  certificate verification, and projection fidelity.
- `compile_context(world, query, ...)` remains the compatibility helper for the lower-level mapping
  form and its policy/profile choices.
- `RepositoryCatalogRequest`, `RepositoryBundleRequest`, and `RepositoryImpactRequest` provide
  bounded repository knowledge navigation across sync MCP, async MCP, and HTTP. The helpers expose
  prefix/limit discovery, normative or exhaustive route traversal, denied-label and edge-vocabulary
  controls, opt-in markdown ceilings, and conservative changed-module impact checks without
  dumping repository bodies or treating graph impact as a semantic diff.
- `TelemetryProjectRequest` exposes the operations observability boundary across the same transports.
  It couples optional metric definitions to observations, requires an explicit redaction policy and
  trace id, and leaves unclassified emission, missing treatments, asserted-only evidence, and zero
  denominators as Rust-owned refusals rather than locally inventing telemetry truth.
- `trace_otel_ingest(trace_id, otlp_json=... | document=..., ...)` invokes the bounded OTLP JSON
  importer and preserves its semantic-loss/readiness report.
- `developer_workbench(session, dashboard=..., ci=...)` composes the Rust authoring-session and
  notebook audit with optional hole-preserving capability queries and review-only GitHub Actions
  planning. The facade validates only the outer mappings; Rust validates digests, dependencies,
  evidence posture, release readiness, safe paths, and deterministic YAML.
- `developer_delivery_audit(...)` composes platform, repository, SDK, conformance, provider,
  governance, and release evidence without executing publication or CI. `DeveloperDeliveryAuditReport`
  plus `Workspace.developer_delivery_audit_report(...)`, `AsyncWorkspace.developer_delivery_audit_report(...)`,
  and the HTTP counterparts expose typed readiness gates, explicit target blockers, fail-closed
  release-request state, foreign-artifact posture, and preserved check evidence. A report is release
  evidence only when a caller supplied an explicit target request; it never creates implicit approval.
- `agent_mission(mission_id, goal, steps, policy=...)` previews or executes a cross-domain tool DAG.
  `MissionStep` and `MissionPolicy` preserve domain labels, dependencies, explicit execution
  allow-lists, side-effect posture, refusal propagation, and output budgets; the server remains the
  authority for ordering and execution. Set `execution_mode="parallel_waves"` for bounded concurrent
  dispatch of independent steps in each deterministic wave (`max_parallelism` is capped at 16);
  serial execution is the default and the executor reserves the worst-case wave output budget before
  launching work. `MissionBinding` can route a JSON-pointer field from a
  successful direct prerequisite into an existing argument slot.
- `mission_preflight(request, catalogue=...)` adds a no-side-effect client review before mission
  dispatch. It returns a request digest, live-catalogue digest, deterministic waves, per-step
  schema reports, binding/dependency findings, execution authorization issues, and explicit
  limitations. It never turns a plan into a domain result; `agent_mission(...)` remains the Rust
  authority for execution, refusal propagation, and output accounting. Executed reports include a
  clock-free `execution_trace`; `MissionExecutionReport.from_wire()` validates its contiguous
  lifecycle sequence while preserving the raw report.
- `capability_discover(...)` searches the complete domain catalogue by intent, domain, group, or
  tool and can attach authoritative MCP schemas for the returned routing matches.
- `CapabilitySearchReport.from_wire(...)` plus `Workspace.capability_discover_report(...)`,
  `AsyncWorkspace.capability_discover_report(...)`, and the corresponding HTTP helpers validate
  ranked groups, cross-domain metadata, result counts, digest provenance, and optional tool
  schemas. `report.domains` and `report.tools` provide deterministic coverage projections.
- `capability_audit(include_groups=...)` verifies catalogue/schema parity, input-schema quality,
  coverage gaps, and intentional multi-group membership.
- `CapabilityAuditReport.from_wire(...)` plus `Workspace.capability_audit_report(...)`,
  `AsyncWorkspace.capability_audit_report(...)`, and the corresponding HTTP helpers expose typed
  parity counts, catalog-only/advertised-only gaps, duplicate memberships, invariant flags, bounded
  schema findings, and optional per-group coverage. Use `report.catalogue_complete` and
  `report.schema_quality.fully_valid` as explicit inspection signals; they are evidence for planning,
  not authorization or domain validity.
- `capability_route(goal, needs, ...)` batches named needs into a digest-bound, non-executing route
  proposal, preserving explicit tool matches separately from ranked candidates. Its raw result also
  includes per-need candidate domains and a `route_coverage` ledger for resolved/unresolved needs.
- `CapabilityRouteReport.from_wire(...)` validates that route-level counts reconcile with per-need
  evidence, recommended-tool overflow, and the aggregate domain/group/tool ledger. The
  `capability_route_report(...)` parser accepts either a decoded stdio payload or an HTTP REST
  envelope; `Workspace.capability_route_report(...)`, `AsyncWorkspace.capability_route_report(...)`,
  `ApiClient.capability_route_report(...)`, and its async counterpart provide bounded typed views
  without executing any candidate. `report.route_coverage.fully_resolved` is routing evidence only,
  not authorization, domain validity, or scientific readiness.
- `CapabilityRouteReviewRequest` and `capability_route_review(...)` validate caller-selected
  handoff inputs, while `CapabilityRouteReviewReport.from_wire(...)` and the corresponding sync,
  async, and HTTP `capability_route_review_report(...)` helpers expose blocked/ready findings,
  candidate mismatches, missing selections, and deterministic dependency waves. A ready report
  contains a mission draft but explicitly remains `mission_preflight_required`. Pass
  `validate_schemas=True` to request authoritative per-tool schema digests and bounded issue paths
  in `report.schema_review`. The resulting `report.review_id` is a deterministic,
  content-addressed correlation key for the route provenance, selections, and validation mode.
- `ApiClient.route_review_evidence(...)` and `AsyncApiClient.route_review_evidence(...)` expose
  bounded retained event evidence for that exact id as `RouteReviewEvidence`; `event_page(...)`,
  `event_stream(...)`, and raw `events(...)` also accept `review_id=...` for transport-native
  filtering. An empty page is explicitly “not found in the retained cursor window,” not proof that
  the review never existed.
- `mission_from_route(route, mission_id, selections, policy=...)` converts that route into a
  provenance-preserving `MissionAssembly` only after every need has one caller-selected candidate,
  explicit JSON arguments, and domain-labelled mission metadata. It refuses unresolved or
  out-of-candidate tools, performs no transport call, and is intended to feed `mission_preflight()`
  before `agent_mission()`.
- `ApiClient.submit_mission(request)` and `AsyncApiClient.submit_mission(request)` submit the same
  typed `MissionRequest` to the bounded asynchronous HTTP executor. `mission_status(mission_id)`
  returns a typed `MissionJob` with the raw authoritative report when terminal, and
  `cancel_mission(mission_id, reason=...)` requests cooperative cancellation. Cancellation stops
  future dispatch between nested calls or parallel batches; it does not force-kill an in-flight
  tool or imply rollback.
  `MissionJob.progress` is a typed `MissionProgress` snapshot with phase, wave, active/completed
  counts, outcome counters, byte totals, and the latest trace cursor/event; it is safe for bounded
  dashboards but does not replace the terminal report or claim domain success.
  `mission_trace(mission_id, after=..., limit=...)` returns a typed `MissionTracePage` with ordered
  `MissionTraceEvent` rows, an exclusive `next_after` cursor, and explicit retention-gap metadata.
  `mission_inventory(...)` returns a typed bounded `MissionInventoryPage` with progress, outcome
  counters, and lifecycle links. `wait_mission(...)` and its async counterpart poll only until a
  terminal state inside an explicit timeout/poll interval bound; `MissionWaitTimeout` retains the
  last authoritative live `MissionJob` so callers can resume, cancel, or inspect without parsing
  an exception string.
  If the gateway is started with `--mission-state`, restored jobs expose
  `recovered_after_restart`; interrupted queued/running work is an explicit failed record, and
  `result_omitted` carries bounded byte-count/SHA-256 metadata when a persisted report was too large.
  `delete_mission(mission_id)` removes a terminal job when the caller has consumed its report;
  active jobs are refused so cleanup cannot discard in-flight work.
  `mission_persistence()` and `flush_mission_persistence()` provide typed operator checks for
  the optional checkpoint without implying durable event cursors or webhook delivery state.
  `event_persistence()` and `flush_event_persistence()` provide the corresponding typed event
  cursor checkpoint check; subscription secrets and pending deliveries are explicitly non-durable.
- `ToolCatalogue`, `ToolCallPlan`, and `tool_checked(...)` provide a checked escape hatch for the
  complete live MCP catalogue, including domains that do not yet have a handwritten convenience
  method. The catalogue is copied from `tools/list` or `/v1/tools`, deduplicated, bounded, and
  digest-addressed; preflight enforces only conservative JSON Schema shape (`required`, types,
  arrays, enums, bounds, and common combinators). Unsupported schema features are warnings, not
  hidden passes. A plan has no side effects, and remote refusals remain `ToolRefusal` rather than
  becoming a successful result.
- `AdapterRegistry` and `adapter_plan(...)` expose a dependency-free biological source planner.
  It matches explicit formats and source shapes to native or Python-delegated routes, optionally
  checks installed optional packages without importing them, and reports the adapter's declared
  semantic-loss and scope surface. `Workspace.adapter_plan(...)`, `ApiClient.adapter_plan(...)`,
  and their async counterparts forward the same request over MCP or HTTP. Planning never sniffs,
  fetches, parses, executes, or grants credentials; DICOM, NIfTI/BIDS, AnnData/Zarr, VCF, BAM/CRAM,
  OME-Zarr, FASTA, FASTQ, SAM, GFF3, PDB, SDF/MOL, mzML, and FHIR readers remain responsible for source-specific conformance in the Python layer.
- `AdapterPlanReport.from_wire(...)` plus `adapter_plan_report(...)`,
  `Workspace.adapter_plan_report(...)`, `AsyncWorkspace.adapter_plan_report(...)`, and the
  corresponding HTTP helpers project the complete authoritative envelope: selected descriptor,
  every candidate status/reason, optional dependency posture, conformance level, accepted formats,
  scope dimensions, declared semantic-loss kinds, and non-executing limitations. The projection
  reconciles top-level and nested `executable` state and keeps a dependency-blocked candidate
  distinguishable from an unsupported format or source shape.
- `TabularIngestRequest` and `tabular_ingest(...)` execute the Rust CSV/TSV adapter only after an
  explicit profile and exactly one inline string or root-confined document are supplied. The
  typed `TabularIngestReport` returned by `tabular_ingest_report(...)` preserves the source and
  profile manifest digests, bounded facts and omissions, semantic-loss variant, and every
  independent conformance check. `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`
  expose the same request/result boundary; a passed report is still mapping/accounting evidence,
  not proof that the source declarations are scientifically true.
- `ConformanceRunArgs` and `conformance_run(...)` run the shipped fixture-verified FIBER suite;
  `ConformanceRunReport` preserves fixture drift, suite digest, pyramid counts, bounded case
  outcomes, and the noncompensatory release decision. `conformance_run_report(...)` is available
  on `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient`; blocked decisions retain
  every unmet gate and actionable evidence, while `results=None` explicitly means details were
  not requested rather than that the suite had no cases.
- `ReleaseAuditArgs`, `ReleaseAuditCheckRequest`, and `release_audit(...)` compose up to 32 exact
  delegated release checks across registry, bundle, conformance, research CI, quality, operations,
  pack health, repository impact, and developer-platform diagnostics. `ReleaseAuditReport` keeps
  required gates, advisory-only observations, evaluated failures, invocation refusals, result
  digests, fail-closed blockers, guarantees, and limitations distinct. Its parser rechecks ordered
  row indexes, count parity, blocker references, advisory null gates, and the Rust aggregator's
  strict conjunction, so a forged or compensating top-level `release_ready` value is rejected.
  `Workspace`, `AsyncWorkspace`, `ApiClient`, and `AsyncApiClient` expose the same typed boundary.
- `BidsAdapter` and `audit_bids(...)` provide a bounded, dependency-free audit of a caller-supplied
  BIDS manifest: relative paths, entity syntax, directory/entity agreement, JSON sidecar
  inheritance, equal-specificity metadata conflicts, task metadata, participant coverage, and
  derivative pipeline descriptions. The report hashes normalized input and states that it did not
  read image bytes; NIfTI/DICOM/EEG/MEG interpretation remains a separate adapter concern.
- `DicomAdapter` and `audit_dicom(...)` provide a bounded parsed-projection audit for study/series/
  SOP identity, duplicate instances, dimensions, frame-of-reference consistency, orthonormal image
  geometry, slice positions, enhanced multi-frame positions, and provenance. It returns digest-bound
  UID summaries without echoing arbitrary patient tags, and separates structural validity from
  publishability when coordinate or provenance losses are blocking. Pixel decoding and transfer
  syntax decompression remain the responsibility of the optional binary reader.
- `NiftiAdapter` and `audit_nifti(...)` provide a bounded parsed-header audit for shape, datatype,
  effective affine, qform/sform declarations, voxel-size agreement, axis codes, spatial/time units,
  series consistency, and coordinate-frame provenance. The result returns an affine digest rather
  than raw matrices in its summary, separates structural validity from publishability, and makes
  clear that image arrays, compression, extensions, and BIDS sidecars were not decoded.
- `AnnDataAdapter` and `audit_anndata(...)` provide a bounded parsed projection audit for `n_obs`/
  `n_vars`, X/layer sparse structure, obs/var index identity, annotation lengths and categories,
  obsm/varm embeddings, obsp/varp pairwise matrices, raw dimensions, and safe `uns` summaries. It
  returns index digests rather than index values, separates structural validity from provenance-
  gated publishability, and does not read HDF5/Zarr chunks or matrix payloads.
- `AlignmentAdapter` and `audit_alignments(...)` provide a bounded parsed BAM/CRAM projection audit
  using explicit 0-based half-open coordinates. It checks the reference dictionary, CIGAR query and
  reference spans, coordinate bounds, flags, primary mate pairing, coordinate sort order, mapping
  qualities, and per-reference coverage. Read identities are source-bound digests; sequences,
  qualities, auxiliary tags, indexes, and reference bases are not decoded.
- `FhirAdapter`, `audit_fhir(...)`, and `parse_fhir_json(...)` provide a dependency-free clinical
  interoperability boundary for FHIR resources and Bundles. They validate resource identity,
  Bundle structure, bounded nesting, duplicate resource keys, profile declarations, reference
  scope, unresolved local-looking references, and coded objects without terminology systems.
  Patient and resource identifiers are source-bound digests; clinical values, narratives,
  extensions, profile invariants, terminology expansion, and external-reference resolution remain
  explicit semantic-loss surfaces rather than being guessed.
- `AdapterRuntime`, `ProjectionRequest`, and `execute_projection(...)` close the planning-to-
  execution handoff for the concrete parsed projection routes across VCF, BIDS, DICOM, NIfTI,
  AnnData, alignment, FASTA, FASTQ, GFF3, PDB, SDF/MOL, mzML, OME-Zarr, and FHIR. The envelope normalizes succeeded, lossy,
  invalid, blocked, rejected, and unsupported states, carries the authoritative adapter descriptor,
  preserves the audit document digest, and returns typed unsupported outcomes when a selected raw
  reader is unavailable. Payload values are not echoed in the request envelope.
- `ProjectionBatchRequest`, `ProjectionBatchResult`, and `execute_projection_batch(...)` compose
  heterogeneous source requests under a bounded ordered envelope. They preserve member-level
  documents, refusal/error states, status counts, adapter/failure counts, validity and publishability
  totals, semantic-loss summaries, scope dimensions, document digests, and a batch digest; optional
  stop-on-error execution reports omitted requests and marks the aggregate unaccepted instead of
  making an incomplete batch look complete.
- `read_nifti_header(...)` and `read_anndata_projection(...)` are verified optional bindings for
  installed `nibabel` and `anndata` environments. They inspect NIfTI headers with memory mapping
  and H5AD/Zarr metadata, then delegate to the same projection auditors; they never call a full
  floating-point image load or disclose matrix values. Missing packages surface as
  `OptionalDependencyUnavailable` and the runtime preserves that refusal.
- The runtime also contains dependency-gated bindings for pydicom metadata-only DICOM reads,
  pysam indexed/compressed VCF/BCF reads, and pysam BAM/CRAM record reads. These feed the DICOM,
  VCF, and alignment auditors respectively, require explicit paths and bounded record limits, and
  never turn an absent pydicom/pysam installation into a fallback parser.
- `OmeZarrAdapter`, `audit_ome_zarr(...)`, and `read_ome_zarr(...)` complete the multiscale image
  route: they validate axes, level shapes, chunks, scale/translation transforms, OME channels,
  labels, and provenance, then inspect only Zarr group/array metadata. Image chunks and pixel values
  are never loaded by the reader.
- `read_fhir_json(...)` and `read_fhir_ndjson(...)` are bounded raw-file bindings for UTF-8 FHIR
  JSON and Bulk Data NDJSON. They reject duplicate object keys and non-standard JSON numbers,
  validate every NDJSON record, and delegate to the same FHIR auditor, so a raw clinical document
  cannot silently take a different validation path than a parsed manifest.
- `FastqAdapter`, `parse_fastq(...)`, and `read_fastq(...)` provide a dependency-free sequencing
  boundary. Multiline records, sequence/quality length equality, printable quality bounds, duplicate
  read identifiers, paired-read completeness, and source-bound read/sequence/quality digests are
  retained without disclosing base or quality strings.
- `SamAdapter`, `parse_sam(...)`, and `read_sam(...)` provide a dependency-free text-alignment
  boundary. They validate SAM headers and sequence dictionaries, flag and mate consistency, CIGAR
  semantics, coordinate bounds, typed optional tags, and declared coordinate sort order while
  retaining only bounded counts, source-bound digests, and privacy-safe alignment previews. BAM/CRAM
  decoding and indexing remain separate dependency-gated capabilities.
- `FastaAdapter`, `parse_fasta(...)`, and `read_fasta(...)` provide a dependency-free reference and
  assembly boundary. They validate multiline record framing, duplicate identifiers, optional
  nucleotide/protein alphabet claims, lengths, symbol counts, and nucleotide GC totals without
  disclosing sequence strings or headers.
- `Gff3Adapter`, `parse_gff3(...)`, and `read_gff3(...)` provide a dependency-free GFF3/GTF-style
  annotation boundary. They validate coordinates, scores, strands, phases, URL-encoded attributes,
  duplicate feature IDs, Parent references and cycles, directives, and embedded FASTA boundaries
  without disclosing attribute values or feature identifiers.
- `BedAdapter`, `parse_bed(...)`, and `read_bed(...)` provide a dependency-free BED3--BED12 interval
  boundary. They validate zero-based half-open coordinates, optional score/strand/thick/RGB fields,
  block counts and non-overlap, duplicate intervals and names, and coordinate ordering while
  retaining source-bound chromosome/name digests rather than labels. BED track metadata is counted
  but never emitted, and assembly/reference-build identity remains an explicit caller responsibility.
- `PdbAdapter`, `parse_pdb(...)`, and `read_pdb(...)` provide a dependency-free structural-biology
  boundary. They validate fixed-column atoms, models, chains, residues, coordinates, alternate
  locations, crystallographic metadata, resolution, and CONECT references while retaining only
  bounded geometry summaries and source-bound structure digests.
- `SdfAdapter`, `parse_sdf(...)`, and `read_sdf(...)` provide a dependency-free small-molecule
  boundary for bounded MDL V2000 records. They validate atom/bond counts and fixed columns, element
  symbols, formal charge/isotope/radical property blocks, connectivity, coordinate summaries, and
  duplicate data fields while retaining source-bound molecule/graph digests. Molecule names,
  property values, and raw records are never emitted; V3000 records are explicitly refused.
- `MzmlAdapter`, `parse_mzml(...)`, and `read_mzml(...)` provide a dependency-free mass-spectrometry
  metadata boundary. They validate bounded XML, spectrum IDs and counts, MS levels, binary-array
  declarations, compression, precision, and encoded lengths without decoding binary arrays or
  asserting peak-level scientific interpretation.
- `parse_vcf(...)` is a bounded dependency-free text VCF reader for the first concrete biological
  adapter. It validates headers, INFO/FORMAT declarations, sample cardinality, allele indexes, and
  finite numeric values; preserves raw spellings beside typed projections; hashes the source and
  each disclosed line; and emits source-located coordinate-frame, provenance, type, and precision
  losses. It validates all records while bounding disclosed variants and loss rows, and it does not
  pretend to provide indexed/compressed/random-access functionality that belongs to `pysam`.
- `BenchmarkObservation`, `summarize_distribution(...)`, `PairedBenchmarkObservation`, and
  `paired_effect(...)` provide dependency-free notebook statistics across agent, oncology,
  multimodal, infrastructure, and coordination domains. They preserve measured versus declared,
  missing, blocked, and not-applicable rows; use deterministic quantiles and sample variance;
  optionally compute a specified-seed percentile bootstrap over observations or declared replicate
  groups; and retain limitations about exchangeability, causal interpretation, and clinical use.
- `tool(name, arguments)` remains available for every current and future MCP domain. Prefer
  `tool_checked(name, arguments)` when a live schema snapshot is available: it makes the transport
  shape inspectable before execution while preserving the server as the authority for domain
  semantics, policy, evidence, and refusal decisions. `plan_tool(...)` is the no-side-effect review
  boundary for agents that need to assemble a call graph before running it.

`ApiClient` and `AsyncApiClient` provide the same standard-library SDK posture for the HTTP
gateway: health/capability discovery, typed `capability_discover`, `capability_audit`,
`capability_route`, and `adapter_plan` helpers, REST tool calls, cursor-based event pages, and signed
webhook subscription/delivery acknowledgement. They preserve status and JSON error payloads in
`ApiError` and do not recreate Rust domain semantics. `event_page()` and `delivery_page()` add
typed `EventPage`, `ApiEvent`, and `DeliveryPage` projections with ordered cursors, retention-gap
signals, retry attempts, signatures, and pending counts; the original raw `events()` and
`deliveries()` methods remain available for forward-compatible payload inspection. `event_stream()`
parses the bounded SSE snapshot into `SseSnapshot`/`SseEvent` records and preserves `x-next-after`;
the parser rejects malformed retry fields and NUL-containing IDs before application code sees them.
Delivery rows also type `state`, `last_error`, and `last_error_retryable`; `replay()` resets selected
rows to attempt one while preserving their delivery IDs, so an operator can distinguish retryable,
permanent, and exhausted transport outcomes before choosing recovery.

## Metrics analytics across domains

`MetricObservation`, `PairedObservation`, and `CalibrationObservation` are small typed request
models for the Rust `metrics_analytics_audit` kernel. They deliberately use caller-owned
`dimension`, `domain`, `unit`, and `condition` strings, so one request can describe verification,
oncology, multimodal agreement, translation, runtime cost, or multi-agent coordination without a
new SDK release for every domain:

```python
from prism_sdk import (
    AnalyticsDirection,
    CalibrationObservation,
    MetricObservation,
)

report = workspace.metrics_analytics_audit(
    [MetricObservation(
        id="world-1",
        dimension="verification",
        domain="oncology",
        system="agent-a",
        value=0.82,
        direction=AnalyticsDirection.HIGHER_IS_BETTER,
        unit="fraction",
        condition="pack/4",
        replicate_group="parent-world-1",
        cost=4.2,
        latency_ms=180.0,
        evidence="reproduced",
    )],
    calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
)
```

The response preserves measured versus declared/missing/blocked populations, descriptive
performance/cost/latency summaries, replicate spread, paired deltas and retention for robustness
or cross-modal/translation/design review, and equal-width calibration bins with Brier and expected
calibration error. `Workspace` and `AsyncWorkspace` send exact wire models; they do not impute
missing rows, pool dependent trials, run an assay, infer a causal effect, or convert a contrast
into clinical evidence. An empty measured population stays `null` rather than becoming zero.

## Authoring packs, cells, and mutations

The authoring layer constructs the exact JSON shapes consumed by the Rust pack, decision-cell, and
mutation crates. It validates local invariants before a request is sent and computes the same
canonical SHA-256 content address as `bioprism-ids`:

```python
from prism_sdk import (
    DecisionCellBuilder,
    InputRef,
    PackBuilder,
    Workspace,
)

pack = (
    PackBuilder(
        pack_id="demo.pack",
        version=(1, 0, 0),
        schema_range=(1, 2),
        title="Decision evidence",
        measures="sufficiency of evidence selection",
        blueprint_module="15.01",
        axis="mechanism",
        capabilities=[{"agent": "evidence_acquisition"}],
        domains=["science"],
        owners=["aurora"],
        license="Apache-2.0",
    )
    .parent("world:demo", 3)
    .decision_family("smallest-sufficient-context")
    .mutation_relation("preserves_verdict")
    .oracle("deterministic")
    .authored_instances(8)
    .build()
)

cell = (
    DecisionCellBuilder(
        "cell-1",
        "select evidence",
        InputRef.from_document("world.json", {"world": 1}),
        InputRef.from_document("query.json", {"query": 1}),
    )
    .accepting("valid", "equivalent")
    .requiring_witness("closure")
    .build()
)
```

`PackArtifact.to_mcp_arguments()` prepares a digest-bound `pack_health_assess` request;
`Workspace.pack_health_assess()` then delegates health, calibration, contamination, and
reportability to Rust. `MutationPlan.standard()` mirrors the deterministic Rust suite and
`Workspace.mutation_family()` runs it against a root-confined world. The authoring layer never
marks a benchmark healthy, a mutation postcondition held, or a cell scientifically correct on its
own. `DecisionCell.accepts()` only evaluates the declared set-valued contract: wrong verdicts,
missing witnesses, and incomplete protected closure remain distinct failures.

## Oracle mesh and evaluation workflows

`OracleManifest`, `JudgementBuilder`, and `PositionDistribution` author the complete evidence
record used by `bioprism-oracle`: versioned `namespace:name` identities, declared versus effective
tiers, independence demotion, explicit evidential planes, fixed UTC validity windows, admissibility
states, checkable findings, abstentions, and tied distributions. A circular oracle is demoted by
the local builder before its judgement is serialized, so a caller cannot accidentally claim a
stronger rung than its declaration permits:

```python
from prism_sdk import (
    EvidenceTier,
    Independence,
    JudgementBuilder,
    OracleManifest,
    OracleRef,
    OracleVersion,
    Position,
    ValidityWindow,
)

manifest = OracleManifest(
    OracleRef("demo:checksum", OracleVersion(1, 0, 0)),
    EvidenceTier.DETERMINISTIC,
    frozenset({"artifact"}),
    frozenset({"biological", "causal"}),
    ValidityWindow("2025-01-01T00:00:00Z", "2030-01-01T00:00:00Z"),
    independence=Independence(),
)
judgement = JudgementBuilder(
    manifest, "2026-01-01T00:00:00Z", Position.SUPPORTED
).rationale("canonical artifact check passed").build()
```

`Workspace.oracle_combine()` forwards these judgements to the Rust mesh, which preserves
same-tier disagreement, suppressed weaker overrides, inadmissible evidence, and unknown states.
`oracle_reference_panel()` and `oracle_missingness()` cover independent-reader consensus and
small-cell/missingness boundaries. `bioeval_reference_audit()`, `evaluation_worldline_audit()`,
`evaluation_reproduction_check()`, and `evaluation_trajectory_check()` expose reference
uncertainty, future-evidence firewalls, reproducibility divergence, and bounded trajectory
properties. These helpers never locally choose a biological truth or convert an abstention into a
negative result.

The package deliberately does not claim to implement DICOM/NIfTI/AnnData, indexed/compressed VCF,
binary BIDS image parsing, inferential statistics, OTLP export, a notebook UI, or CI deployment. It
now ships bounded text VCF, BIDS manifest, parsed DICOM metadata, parsed NIfTI header/affine, and
parsed AnnData/Zarr matrix audits plus descriptive/cluster-bootstrap utilities above the Rust kernel.
It also ships a parsed BAM/CRAM alignment audit. The repository still keeps gRPC, durable event
storage, external webhook delivery, heavyweight binary biological readers, and statistical estimators
as separate contracts.

## Verification

From the repository root:

```bash
cd python
python -W error::ResourceWarning -m unittest discover -s tests -v
python -m compileall -q prism_sdk tests
```

The tests use a subprocess fake MCP peer so lifecycle, framing, protocol, remote errors, structured
refusals, sync/async parity, and cleanup are exercised through actual pipes rather than direct
function calls.
