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

The package also includes dependency-free authoring builders for digest-bound benchmark packs,
set-valued decision cells, deterministic metamorphic mutations, versioned oracle manifests,
evidence judgements, reference panels, evidence-conditioned BioCapability audit requests, evaluation requests, and typed metrics observations,
paired contrasts, and calibration forecasts. They validate local JSON and cross-field invariants,
then let `Workspace` delegate final decisions and arithmetic to the Rust kernel through
`pack_health_assess()`, `mutation_family()`, `oracle_combine()`,
`metrics_analytics_audit()`, `biocapability_evidence_audit()`, `bioql_compile()`, `world_claim_check()`, `lab_plan()`, `routing_decide()`, `fiber_compile()`, `fiber_refine()`, `fiber_explain()`, `fiber_verify()`, `projection_bundle()`, `repository_catalog()`, `repository_bundle()`, `repository_impact()`, `telemetry_project()`, `developer_workbench()`, `agent_mission()`, `capability_discover()`, and the evaluation helpers.
The repository helpers preserve bounded discovery and route completeness; telemetry requires an
explicit redaction policy and observed metric inputs. The FIBER helpers preserve the progressive-disclosure lifecycle: compile the minimal contract,
refine only when necessary, inspect omissions before trust, verify certificates, and opt into full
projection bodies explicitly.
The mission layer lets an agent preview or explicitly execute a bounded, allow-listed graph across
the existing domain tools while retaining refusals and blocking dependent work. The workbench
keeps authoring/notebook sessions, stale digests, capability holes, release posture, and review-only
CI planning in one evidence-bearing response; it does not pretend to execute a hosted UI or GitHub
runner. `MissionBinding` supports validated field-level dataflow between direct prerequisite steps,
and `CapabilityQuery` routes across the complete domain catalogue with optional tool schemas;
`capability_audit()` verifies the catalogue against the authoritative MCP schema set, and
`capability_route()` batches named needs without executing the returned candidates.
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
See
[`docs/PYTHON_SDK.md`](../docs/PYTHON_SDK.md) for the full authoring contract.
