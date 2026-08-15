# Blueprint coverage

The blueprint ships 973 content modules across 44 sections (990 files, less 17 section indexes).
This file records which of them the workspace actually cites, so the next batch of crates is chosen
from evidence rather than from whichever section came to mind.

**Measured, not asserted.** A module counts as covered when its id appears in a doc comment, a test,
or a design note under `crates/` or `docs/`. That is a weak criterion on purpose — a citation is not
an implementation — so read the numbers as *"someone has read this and taken a position on it"*,
never as *"this is done"*. The stronger criterion would be a conformance test per module. It does
not exist and is not being claimed.

The MCP integration layer currently exposes 122 callable tools. That count is intentionally
separate from this citation denominator: `pack_health_assess`, `sdk_registry_check`, and
`repository_impact` make existing typed contracts agent-callable, while `world_generate`,
`hub_submission_review`, and `telemetry_project` add bounded in-tree generation, public-hub
contract, and observability projection workflows. `factory_lifecycle_simulate`,
`hub_disclosure_review`, `hub_card_render`, `hub_leaderboard_render`, `release_audit`, and
`developer_delivery_audit` now
compose the factory recovery, public-hub publication, and release-evidence contracts while keeping
durable queues, identity, public-key signing, UI, OTLP export, and network publication explicit as unimplemented;
the Python MCP transport foundation is documented separately and does not imply the full SDK.
`trace_otel_ingest` adds a bounded, dependency-free OTLP JSON span importer with source-preserving
Event IR mapping and explicit semantic-loss accounting; it does not export to a collector or infer
vendor conventions into compilable evidence. `runtime_execution_simulate` adds deterministic
record/replay, budget, fault, and
fork evidence; `megafactory_twin_audit` and `megafactory_placement_audit` expose model-discrepancy,
oracle-eligibility, placement, attestation, fencing, and duplicate-effect predicates. None of
these turn foreign Python, gRPC, CI, UI, OTLP export, or network-publication artifacts
into implemented workspace code. `bioprism-api` now supplies a bounded REST/JSON-RPC gateway,
cursor-based event/SSE snapshots, and a signed retryable webhook outbox over the same MCP
dispatcher; it does not supply gRPC, TLS termination, durable storage, or an external delivery
worker. The repository also ships a dependency-free TypeScript Fetch SDK over that gateway, with
bounded request/response handling, structured error types, SSE parsing, typed cross-domain
facades, and webhook cursor lifecycle helpers; it does not clone every Rust domain type or add a
browser secret store. `registry_lifecycle_simulate` adds a continuation-safe local
publication log and artifact-integrity projection, while `metrics_profile_audit` adds the
per-capability coverage and uncontested-lead projection used by honest public cards.
`metrics_analytics_audit` adds a bounded domain-neutral arithmetic kernel for scalar summaries,
paired robustness/cross-modal/translation/design contrasts, cost and latency, replicate spread,
and probability calibration while keeping declared and missing evidence out of measured values.
`developer_workbench` now composes a validated authoring/notebook session, digest-bound dependency
ordering, stale-input audit, evidence-aware capability dashboard query, and deterministic GitHub
Actions YAML planning surface. It preserves notebook holes and release refusal predicates rather
than treating generated YAML or a local audit as executed CI, a hosted authoring UI, or a public
dashboard. The TypeScript and Python clients expose this contract without cloning the Rust model.
`developer_delivery_audit` composes the platform, repository, SDK, conformance, provider,
governance, and release checks into independently inspectable readiness gates. Its typed Python and
TypeScript projections preserve explicit release targets, blockers, fail-closed/no-implicit-release
state, foreign-artifact posture, and omitted optional evidence instead of collapsing an incomplete
delivery review into a green score.
`agent_mission` now connects those domain surfaces into an explicit mission DAG: agents can preview
deterministic dependency waves or execute an allow-listed sequence while preserving raw refusals,
blocking dependent work, bounding output, and refusing side-effect confirmations by default. Serial
execution remains the default; `parallel_waves` explicitly dispatches independent steps concurrently
inside a bounded in-process executor after reserving the worst-case wave budget. It is not a
distributed scheduler, durable queue, or truth synthesizer.
Every executed mission also returns a clock-free, sequence-addressed execution trace covering
mission and wave lifecycle, step starts, completions, refusals, blocks, argument digests, and byte
accounting; parallel thread completion order never changes that trace order.
The asynchronous HTTP boundary projects those authoritative trace events into a bounded `progress`
snapshot while a job is live and reconciles the snapshot against the terminal report. Python and
TypeScript clients type the same projection for operational dashboards, while replay, content
identity, and domain interpretation remain owned by the final report and trace.
The HTTP boundary also serves bounded cursor pages over retained mission trace events, with explicit
`next_after`, retention-gap, and dropped-event fields; this gives every domain workflow the same
replay/observability contract without inventing domain-specific event semantics.
The same retained rows are emitted as `mission.trace` events into the gateway's cursor, SSE, and
signed webhook surfaces, allowing lifecycle monitoring and delivery retry to share one event log
and one retention-gap contract with ordinary tool calls.
The gateway also accepts an optional `--mission-state` path for an atomic, 64 MiB-bounded mission
checkpoint. Terminal jobs restore their retained progress, traces, and size-limited reports;
queued/running jobs become explicit `failed` records with `recovered_after_restart` after a
restart, never falsely claiming that interrupted work resumed. This is restart-aware mission
inspection, not durable event storage, distributed scheduling, or effect rollback.
An independent optional `--event-state` path now checkpoints retained event rows and cursor
continuity under the same 64 MiB bound. It intentionally excludes webhook secrets, subscriptions,
and pending deliveries; the API and both SDKs expose status/flush checks so operators can verify
which recovery boundary is enabled instead of inferring it from a 2xx response.
The API crate also exposes a bounded `DeliverySender`/`ApiRouter::deliver_once` cycle for embedded
workers: successful signed sends are acknowledged, retryable failures advance through the existing
ten-attempt cap, and permanent or exhausted failures remain pending. Network/TLS and egress policy
remain outside the dependency-free gateway.
Delivery pages retain the last failure classification and error for operator inspection, while an
explicit replay endpoint resets a selected row to attempt one without changing its delivery ID or
pretending that the receiver accepted it.
Python now exposes typed inventory pages plus bounded sync/async waits that retain the last live
job on timeout; TypeScript exposes the same wait contract with abortable polling and a typed timeout
error. These helpers coordinate every domain mission without claiming durable scheduling.
The Python HTTP SDK also exposes typed event and signed-delivery pages with the same cursor ordering,
retention-gap, retry-attempt, signature, and pending-count fields already present in TypeScript.
Its dependency-free `event_stream()` parser now mirrors the TypeScript SSE field rules and preserves
the gateway's `x-next-after` cursor for resumable monitoring.
The HTTP listener also serves through a shared immutable router with atomic request IDs and cloned
ready MCP dispatch sessions, allowing independent domain requests to proceed concurrently while
keeping all mutable mission and event state bounded and synchronized.
Validated RFC 6901 bindings can copy fields from successful direct prerequisites into existing
argument slots, and each nested call records the content digest of its post-binding inputs.
The authoritative executor validates every known tool's materialized arguments against its
`tools/list` JSON Schema before dispatch, with bounded depth, schema-size, and diagnostic limits.
Static steps are checked at mission validation time; bound steps are checked again after dataflow
materialization in both serial and parallel modes. Schema refusals carry a schema digest and
JSON-pointer diagnostics and are distinct from the nested domain tool's own refusal envelope.
`capability_discover` makes the complete cross-domain catalogue searchable by intent, domain, group,
or tool and can attach the authoritative `tools/list` schemas for the ranked matches. The Python
`CapabilitySearchReport` and TypeScript `CapabilityDiscoverResult` projections preserve
each matched group's domains, crates, CLI entrypoints, Python artifacts, ranked fields, matched
tools, catalog digest, and optional schema attachment instead of collapsing discovery to an untyped
blob.
`capability_audit` proves that catalogue names and authoritative schemas remain in parity, reports
catalog-only and uncatalogued tools, and keeps intentional multi-group membership explicit.
It also validates every input schema's object shape, required/property consistency, and size bound,
so a name-parity pass cannot conceal a malformed contract.
The Python `CapabilityAuditReport` and TypeScript `CapabilityAuditResult` projections preserve the
audit's reconciliation totals, invariant flags, schema-quality findings, duplicate memberships, and
optional per-group coverage with explicit `catalogue_complete`/`fully_valid` inspection signals.
`capability_route` batches named needs across those groups into a single route id, preserves each
bounded search result, distinguishes explicit tool filters from ranked candidates, and never
executes the proposed tools. Each need now carries candidate domains, and `route_coverage` reports
resolved versus unresolved needs plus the groups, domains, and tools that actually contributed
candidates; this is routing evidence, not authorization, scientific readiness, or equivalence.
`capability_route_review` is the next bounded handoff checkpoint: it accepts the complete route and
caller-selected candidates, refuses malformed route envelopes, and returns structured blocked or
ready diagnostics for candidate membership, exactly-once need selection, explicit JSON arguments,
dependency references, and deterministic dependency waves. A ready result contains a mission draft
but remains `mission_preflight_required`; it never grants permission or executes a tool. Callers may
set `validate_schemas` to receive per-selected-tool authoritative schema digests and bounded issue
paths; a schema-clean result is still shape evidence, not domain validation or authorization.
Each result carries a deterministic content-addressed `review_id` derived from route provenance,
caller selections, and validation mode so operators can correlate handoff evidence without relying
on timestamps or mutable server state.
The HTTP event page and bounded SSE snapshot accept the same exact `review_id` filter, and
`/v1/route-reviews/{review_id}/evidence` provides a typed retained-evidence lookup. Missing
retained evidence is reported as an empty bounded window, never upgraded into a historical
non-existence claim.
The Python SDK now covers the complete FIBER progressive-disclosure lifecycle through typed sync,
async, and HTTP helpers: bounded world/query compilation at l0--l4, handle-or-source refinement,
compile-plan explanation, certificate verification, and opt-in graph/hypergraph/timeline/table
projection bundles. The local layer rejects traversal, malformed handles, and ambiguous source
selection; Rust remains authoritative for sufficiency, omission accounting, and fidelity.
The repository knowledge helpers now cover bounded module catalog discovery, route-specific
normative/exhaustive documentation bundles, conservative changed-module impact checks, and explicit
markdown disclosure ceilings. The telemetry helper carries the same cross-domain discipline into
operations: redaction policy and trace correlation are required, while optional metric projections
must carry observations and remain subject to Rust's refusal semantics.
`adapter_plan` makes the biological-format boundary callable: it matches explicit source formats
and source shapes against native CSV/inventory adapters and Python-delegated DICOM, NIfTI/BIDS,
AnnData/Zarr, VCF, FASTA, FASTQ, SAM, GFF3, PDB, SDF/MOL, mzML, BAM/CRAM, OME-Zarr, and FHIR routes. It reports declared semantic-loss surfaces and
distinguishes missing from unchecked optional dependencies before execution; it never sniffs
content, fetches sources, imports packages, or claims that heavyweight parsing has occurred.
The Python `AdapterPlanReport` and TypeScript `AdapterPlanResult` projections preserve the full
selected descriptor, candidate statuses and refusal reasons, accepted formats, source kinds,
conformance level, dependency posture, scope dimensions, semantic-loss vocabulary, and nested
non-executing limitations across MCP and HTTP. This makes biological and clinical route selection
auditable even when no adapter is executable.
The Python adapter layer now also contains a bounded text VCF reader: it validates the complete
record stream, preserves raw INFO/FORMAT spellings beside typed projections, retains source and
line hashes, and reports reference-build, provenance, type, and precision limitations without
inventing an indexed/compressed reader or a reference genome.
It also contains a bounded BIDS manifest audit: it validates relative paths and entity syntax,
directory/entity agreement, inherited JSON sidecars and equal-specificity conflicts, task metadata,
participant coverage, and derivative descriptions while explicitly leaving binary image parsing to
the format-specific adapter. Both reports disclose their limits and bind normalized input digests.
The parsed DICOM projection audit now adds UID hierarchy, duplicate SOP, dimensions, frame-of-
reference, orthonormal orientation, slice-position, enhanced multi-frame, provenance, and
privacy-safe digest checks. It distinguishes structural validity from publishability and does not
decode pixels or transfer syntaxes.
The parsed NIfTI projection audit now checks bounded shape and datatype declarations, affine
invertibility and last-row invariants, qform/sform agreement, voxel-size consistency, units, axis
codes, series consistency, and coordinate/provenance loss. It emits affine digests and does not
decode arrays, compression, extensions, or BIDS sidecars.
The parsed AnnData/Zarr projection audit now checks `n_obs`/`n_vars`, X/layer sparse shapes and
indices, obs/var index uniqueness, annotation lengths/categories, obsm/varm and obsp/varp shapes,
raw dimensions, `uns` kinds, provenance, and bounded disclosure without reading matrix values or
store chunks.
The parsed BAM/CRAM alignment projection audit now checks explicit reference dictionaries, CIGAR
query/reference accounting, 0-based coordinate bounds, flags, primary mate pairing, coordinate
sort order, mapping qualities, coverage, and reference-build/provenance loss without decoding read
payloads, indexes, or reference bases.
The Python adapter runtime now executes the complete set of concrete parsed-projection routes through one
bounded request/result envelope, preserves adapter descriptors and document digests, classifies
lossy/invalid/blocked/rejected outcomes, and explicitly refuses unavailable raw-byte routes without
silently falling back or sniffing content.
Its heterogeneous batch envelope now aggregates status, adapter, failure, validity, publishability,
scope, and semantic-loss evidence while retaining member digests and explicit omitted-request state;
stop-on-error batches are never marked accepted as complete.
Verified optional bindings now execute raw NIfTI headers and H5AD/Zarr metadata when nibabel or
anndata is installed, while preserving the same bounded auditors and refusing absent dependencies.
The readers avoid full image-array and matrix-value materialization.
The same dependency boundary now covers pydicom metadata-only DICOM and pysam indexed VCF/BCF plus
BAM/CRAM records; each delegates to the corresponding projection audit and reports absent packages
as typed unsupported execution rather than silently changing formats. The dependency-free FHIR
route covers JSON resources, Bundles, and Bulk Data NDJSON: it audits every resource record for
identity, bounded structure, profile declarations, privacy-safe references, duplicate keys, and
provenance without claiming profile or terminology conformance. NDJSON is normalized through the
same cross-record reference auditor, with record counts and byte bounds retained in the manifest.
The dependency-free FASTQ route validates complete multiline records, sequence/quality lengths,
printable quality ranges, duplicate read identifiers, and paired-read completeness; read identifiers,
bases, and qualities are source-bound digests or aggregate summaries rather than disclosed content.
The dependency-free SAM route validates bounded headers and sequence dictionaries, flags and mate
consistency, CIGAR query/reference accounting, coordinate bounds, optional-tag types, and declared
coordinate sort order without emitting read names, reference labels, sequences, qualities, or tag
values. Binary BAM/CRAM remains separate and dependency-gated.
The dependency-free mzML route audits bounded XML, spectrum identity and declared counts, MS levels,
scan-time summaries, binary-array type/compression/precision declarations, and encoded lengths; it
never decodes or emits m/z, intensity, or time arrays.
The dependency-free FASTA route audits multiline sequence records, duplicate identifiers, optional
nucleotide/protein alphabet claims, lengths, symbol counts, and GC totals without disclosing sequence
strings or headers.
The dependency-free GFF3 route audits bounded feature rows, coordinates, scores, strands, phases,
URL-encoded attributes, duplicate IDs, Parent resolution/cycles, directives, and embedded FASTA
boundaries without disclosing annotation values or feature identifiers.
The dependency-free BED route audits BED3--BED12 interval rows, zero-based half-open coordinates,
optional scores/strands/thick bounds/RGB fields, transcript-style block geometry, duplicate intervals
and names, and coordinate ordering without disclosing chromosome labels, item names, or track
metadata. Assembly/reference-build identity remains explicit loss rather than inferred from labels.
The dependency-free PDB route audits fixed-column atoms, models, chains, residues, coordinates,
alternate locations, crystallographic metadata, resolution, CONECT edges, and bounded geometry
without emitting raw structure records.
The dependency-free SDF/MOL route audits bounded MDL V2000 molecular graphs, fixed-column atom and
bond rows, elements, formal charge/isotope/radical properties, connected components, coordinate
summaries, duplicate data fields, and source-bound molecule/graph digests without emitting molecule
names, property values, or raw records. V3000 records are refused explicitly.
The OME-Zarr route now inspects Zarr metadata directly and audits multiscale axes, level shapes,
chunk bounds, spatial transforms, channel/label metadata, and provenance without reading image
chunks or pixel values.
The Python benchmark layer now complements Rust's descriptive metrics kernel with typed evidence
rows, distribution summaries, direction-aware paired contrasts, and deterministic observation- or
replicate-group bootstrap intervals. It preserves declared/missing/blocked populations and labels
the bootstrap as descriptive rather than inferential or causal.
These remain in-memory integrations of existing Rust contracts. `cache_invalidation_simulate` adds replayable
key completeness, dependency-graph invalidation, partial unknown-region handling, fail-closed
misses, and explicit re-proving; `storage_lifecycle_simulate` adds deterministic tiering, pin
protection, quota reserves, raw class attribution, and non-copyable delegation/absorption. These
are still local projections: they do not create durable cache indexes, external invalidation
feeds, storage movers, quota enforcement, or tenant isolation. `security_redteam_simulate` now
composes the in-tree section-13 red-team, disclosure, trust-boundary, incident-containment, and
audit/attestation contracts into one bounded replay surface. It keeps confirmed-only regression
cells, advisory-gated disclosure, deny-by-default crossings, across-trial evaluator feedback,
complete-lineage containment, and observed-versus-asserted evidence explicit; it does not create
fuzzers, runtime sandboxes, detectors, containment execution, notifications, signing, external
checkpoints, or durable security infrastructure.
`biocapability_evidence_audit` now composes the metrics profile with an explicit nine-dimension
evidence matrix: grounding, information acquisition, resource efficiency, temporal validity,
cross-modal consistency, causal identification, reproducibility, translation maturity, and
multi-agent coordination. It validates dimension-specific support, blocks temporal leakage and
unknown dimensions, distinguishes observed/reproduced evidence from declarations, and gates only
explicitly requested claims. Optional value-of-information, reference, worldline, and reexecution
subaudits are included as bounded projections; this does not create data acquisition, biological
assays, causal estimators, replication infrastructure, clinical validation, or multi-agent runtime.
`bioatlas_publication_audit` now binds that evidence posture to the public-hub publication path:
atlas coverage, optional evidence claims, moderation/card rendering, and leaderboard ranking are
reported as separate gates with explicit release targets. Numeric card scores require both disclosure
eligibility and evidence-conditioned readiness; absent requests, atlas holes, withheld scores, and
unranked entries stay visible rather than becoming implicit publication passes. This remains a
bounded in-memory composition and does not create a web publisher, identity service, assay runner,
leakage detector, or clinical approval path.
The Python SDK now exposes the same evidence-conditioned profile as a typed workflow: nine named
evidence dimensions, explicit observed/reproduced/declared/missing states, support-field validation,
claim prerequisite validation, duplicate-ID rejection, omission accounting, claim blockers, optional
subaudits, and sync/async/HTTP transport helpers. TypeScript exposes the same top-level result
projection. Rust remains authoritative for metric comparability, release posture, and nested
scientific audits.
The typed `BioAtlasPublicationAuditReport` and TypeScript `BioAtlasPublicationAuditResult` then
preserve the next publication boundary: atlas aggregation, evidence-conditioned numeric scores,
card disclosure, leaderboard ranking, ranked/unranked counts, and explicit target blockers remain
separate gates rather than collapsing into a publication score.
It also exposes `BioQlCompileRequest` with bounded query and schema transport across sync MCP,
async MCP, and HTTP; the compiler remains fail-closed and non-executing, with Rust retaining
authority over schema, units, frames, temporal semantics, provenance, access labels, and cost.
The Python SDK also exposes bounded envelope models and sync/async/HTTP helpers for
`world_claim_check`, `lab_plan`, and `routing_decide`; serialized world/lab/routing semantics remain
Rust-owned, including provenance support, privacy boundaries, evidence reachability, abstention, and
safe defaults.

The table below is a snapshot; the script is the live source.

```bash
BLUEPRINT=/path/to/distribution/root tools/coverage.sh
```

## The end state, and what is left

**An earlier snapshot reported 92.6% until an audit found an off-by-one in `tools/coverage.sh`.** The numerator
counted every cited module while the denominator excluded the ten programme sections, so a single
cited prose module — `21.07`, in `crates/bundle`, for the sentence deferring the signing scheme to
an ADR nobody wrote — inflated the count by one. The evidence was already in this file: it said 703
of 759 and "the remaining 57" in the same paragraph, and 759 − 703 is 56. `tools/backlog.sh` strips
prose from the uncovered list before counting, which is why its figure was the correct one all
along. 702 + 57 = 759 now reconciles.


Coverage is **93.7%** — 711 of 759 code-bearing modules. The remaining **48 are enumerated in
`docs/BACKLOG.md` and explained in `crates/residue`**, which holds one typed verdict per module
saying why no crate implements it, anchored to a sentence a classifying crate actually wrote. Its
reconciliation against the backlog is a test, so the two cannot drift apart silently.

The primary distribution over the 48: **37 process, 4 foreign artifact, 7 discharged elsewhere,
and 0 genuinely uncovered.** One module still carries work on a secondary reading. `crates/bioethics`
discharges §36's sandboxing module and in the same paragraph records that all thirteen of its
required controls need a process boundary, a network stack or a scanner, none of which exists here
— so the register carries a second verdict saying the control exists nowhere. A report hiding that
secondary work because the primary bucket is discharged would be the flattering answer, and this
file's own rule forbids it.

Three categories in that table were discovered rather than planned, each by a crate that read its
section and refused to pad:

- **Process** — describes what people do. `crates/stewardship` found 12 of §14's 18.
- **Foreign artifact** — code-bearing, precise, testable, and not Rust and not in this repository.
  `crates/devplat` now finds 3 of 20 in §11 and §19: the two GitHub Action modules and the full
  Python SDK surface. The TypeScript gateway and Python pack/oracle/evaluation authoring clients
  are now in this repository over authoritative Rust contracts.
- **Discharged elsewhere** — the content exists under a different section's id. **11 verdicts name
  their own author as the discharger**, a crate that built the capability without ever citing the
  module, which a token scan structurally cannot see.

Ten modules are **contested** — `atlasx` says §33's remainder defines nothing once the shared blocks
are stripped, `metrics` says the buildable part is already built — and the register keeps both
readings rather than adjudicating.

## Where the 973 modules are

Ten sections are programme documents rather than specifications of behaviour: start-here material,
strategy, system-architecture narrative, the research and implementation programmes, growth and
community, the ADR log, sources, and templates. They total **214 modules and describe no code**.
Counting them in a coverage denominator would be flattering and meaningless.

§02 is the borderline call. It is architecture narrative the crate layout already reflects without
citing it, so excluding it slightly understates coverage rather than overstating it.

**The section boundary is the wrong granularity, and §14 proved it.** `crates/stewardship` read all
eighteen of §14's uncovered modules and classified twelve as process rather than code — councils,
recusal, cadence, budgets, appeals — using the test *"is the detailed design a set of predicates
over an artifact, or a description of what people do?"*. Those twelve are counted in the 759 and
will never be covered by anything, because a `Council::vote()` would assert only that a council met.
Two of them do carry one code-bearing clause each, and both were already implemented elsewhere:
14.07's "repeated queries reduce holdout status" is `lab`'s exposure ledger, and 14.13's "authors do
not solely certify their own systems" is `registry`'s reviewer-independence rule.

The denominator is not being adjusted for this. Twelve modules out of 759 is inside the noise of the
citation criterion itself, and hand-tuning a denominator downward until the number looks better is
exactly the move this file exists to avoid. What it does mean is that **100% is not the target and
never was** — some remaining modules are prose that no crate should implement, and the honest end
state is a backlog whose residue is explained rather than empty.

| | modules |
|---|---|
| total content modules | 973 |
| programme / prose modules | 214 |
| **code-bearing modules** | **759** |
| cited | 711 |
| **code-bearing coverage** | **93.7%** |

## Per section

Worst-covered code-bearing sections first. **This table is a snapshot from an earlier batch and is
now stale** — headline coverage has moved from 40.6% to 93.7% since it was taken. Regenerate with
`tools/coverage.sh` rather than trusting the rows below for anything load-bearing; they are kept
because the *shape* they show is still the argument, and the shape has not changed.

| § | section | cited | total | crate |
|---|---|---:|---:|---|
| 35 | MILLION_SCALE_BENCHMARK_FACTORY_AND_INFRASTRUCTURE | 0 | 18 | — |
| 04 | INGESTION_AND_INTEROP | 1 | 6 | `adapter` |
| 06 | BENCHMARK_COMPILER | 2 | 15 | — |
| 27 | BENCHMARK_FACTORY_AND_HUB | 2 | 22 | `factory` |
| 38 | REFERENCE_BIOWORLDS_AND_VERTICAL_SLICES | 2 | 16 | `examples` |
| 09 | INFERENCE_LAB | 3 | 11 | — |
| 12 | DATA_AND_INFRASTRUCTURE | 3 | 22 | `ledger` |
| 41 | GRAPH_FIRST_KNOWLEDGE_AND_NAVIGATION | 3 | 16 | — |
| 10 | REGISTRY_AND_HUB | 4 | 22 | `registry`, `hub` |
| 11 | DEVELOPER_PLATFORM | 4 | 25 | `sdk` |
| 19 | REFERENCE_EXAMPLES | 4 | 22 | `examples` |
| 28 | BIOLOGY_DATA_AND_STANDARDS | 4 | 21 | `standards` |
| 33 | BIOCAPABILITY_ATLAS_AND_METRICS | 4 | 19 | `atlas` |
| 42 | GRAPH_NATIVE_EVALUATION_HUB_AND_UI | 4 | 31 | — |
| 13 | SECURITY_PRIVACY_AND_SAFETY | 5 | 26 | — |
| 34 | BIOATLAS_PUBLIC_HUB_AND_ECOSYSTEM | 6 | 23 | `hub` |
| 14 | GOVERNANCE_AND_QUALITY | 7 | 25 | `governance` |
| 30 | NEURO_ONCOLOGY_ONCOWORLD | 7 | 30 | `onco` |
| 08 | ADAPTIVE_EVALUATION | 7 | 8 | `adaptive` |
| 31 | BIOLOGICAL_ORACLES_AND_REFERENCE_STANDARDS | 8 | 17 | `oracle` |
| 05 | EXECUTION_RUNTIME | 9 | 12 | `runtime` |
| 07 | EVALUATION_ENGINE | 9 | 13 | `evalengine` |
| 25 | BIOLOGICAL_IR_AND_LANGUAGE | 9 | 23 | `bioir` |
| 03 | CORE_SPECIFICATIONS | 10 | 12 | `section`, `fiber` |
| 32 | BIOLOGICAL_MUTATION_AND_STRESS_PROGRAM | 11 | 23 | `stress` |
| 23 | AGENT_INTERWEAVE_FABRIC | 12 | 50 | `weave` |
| 26 | BIO_EVALUATION_ENGINE | 12 | 24 | `bioeval` |
| 39 | TOKEN_EFFICIENT_BIOLOGICAL_INFERENCE | 14 | 25 | `fiber`, `section` |
| 36 | BIOLOGY_SECURITY_PRIVACY_ETHICS_AND_GOVERNANCE | 15 | 22 | `policy` |
| 24 | BIOPRISM_FOUNDATION | 17 | 17 | `foundation` |
| 40 | BUILD_READY_ENGINEERING_CONTRACTS | 21 | 45 | spread across all |
| 29 | BIOLOGY_CAPABILITY_AND_BENCHMARK_PACKS | 22 | 22 | `packs` |
| 15 | BENCHMARK_PACKS | 26 | 26 | `packs` |
| 43 | FIBER_QUERY_COMPILED_EPISTEMIC_CALCULUS | 39 | 50 | `fiber`, `section`, `ids` |

Excluded as prose: §00 (16), §01 (7), §02 (10), §16 (20), §17 (26), §18 (23), §20 (45), §21 (12),
§22 (26), §37 (29).

## What the shape of this table says

Three findings, none of them flattering.

**The deepest section is the best covered, and that is survivorship.** §43 (FIBER, 50 modules) sits
at 78% because it is the thesis and was built first. §23 (Agent Interweave Fabric, also 50 modules)
sits at 24% because `weave` deliberately stayed a microkernel. Those two numbers are not comparable
quality signals — one is depth, the other is restraint — and averaging them would hide both.

**§40 is the most valuable uncovered surface.** It is the only section marked build-ready rather
than planned: frozen contracts, not design prose. 24 of its 45 modules are untouched. Every crate
that worked from a §40 module had an easier time than the ones working from `Planned` text, so at
equal size an uncovered §40 module should be preferred over an uncovered module anywhere else.

**Whole capability areas had no crate at all, and now all six do.** §13 security and safety (26),
§42 graph-native evaluation and UI (31), §35 million-scale infrastructure (18), §41 graph-first
navigation (16), §06 benchmark compiler (15) and §09 inference lab (11) were 117 modules — 15% of
the code-bearing blueprint — with nothing standing in for them. `safety`, `lens`, `scale`,
`docgraph`, `benchcompiler` and `lab` closed that set. The remaining gaps are depth inside sections
that already have a crate, which is a different and easier problem than a blank area.

## Boilerplate, and why the numbers are not strictly comparable

Sixteen sections have now been measured, each by the agent that built against it. Most are heavily
repetitive; one is not, and the exception matters more than the average.

| § | boilerplate | distinguishing lines per module |
|---|---:|---|
| 42 | 93.6% | 5 — title, module id, H1, one outcome sentence, one diagram label |
| 35 | 82.3% | 14–16 |
| 32 | 79.3% | 19 median |
| 41 | 72.6% | 14.1 mean |
| 06 | 70.8% | 17–25 |
| 14 | 70% | 19.4 of ~65 non-blank |
| 09 | 68.8% | 17–31, median 19 |
| 13 | 67.5% | 19–32, median 21 |
| 28 | 52% | 34.5 of 71.5 non-blank |
| **23** | **16.2% verbatim / 51.2% rare-term** | **~54 median** |
| 12 | — | ~15 per 100-line file |
| 11 | — | 18 unique in a 93-line module, frontmatter and title included |

**§23 is the exception and it is a real one.** Measured three ways over all 48 modules: 16.2% of
lines appear in more than one module, 51.2% by the rare-term method used for §28, and only 11.6% of
802 headings recur verbatim. It is the most content-dense section in the blueprint, and its
repetition is *shape* — frontmatter, Purpose, a taxonomy list, a pseudo-code fence, evaluation hooks
— rather than text. Six of its modules yielded roughly 300 distinguishing lines.

**These figures were produced by different methods and are not a single scale.** Only §23 was
measured three ways, and its own two headline numbers differ by 35 points. A verbatim-duplication
count and a rare-term count answer different questions, and no agent was given a common definition.
Read the column as evidence that a section is repetitive or is not, and distrust small differences
between rows.

There is now a measured instance of exactly that hazard, and it has been **resolved**. Three agents
independently measured §23's verbatim duplication over the same 6,001 lines, all describing the
metric the same way, and reported **16.2%**, **10.7%** and **10.4%**. A fourth found the cause: the
YAML front matter. Seven lines per file across fifty files, five of them byte-identical, contributes
250 duplicated line occurrences and **4.3 points**. Counting it gives ~16%; stripping it gives ~12%.
Both are right about different corpora.

Recomputed independently, as written and front-matter-stripped: **967 of 6,001 (16.1%)** and
**667 of 5,651 (11.8%)**. Nobody was wrong; the corpus was never agreed.

The number that does *not* move under any preprocessing is the sharper one. Only **2.1% of distinct
line-strings** are shared at all — stable at 1.6–2.1% across four filters — and those few strings
account for the whole 16%. §23's modules share *formatting*, not content. A section is not
repetitive because it contains a lot of repeated text; it is repetitive because a small template is
stamped many times, and counting distinct shared strings sees that directly while counting
occurrences sees it through the size of the template.

## Three disagreements, three distinct causes

Agents measuring the same section disagreed three times, and each was run down to a specific
methodological choice. None was a mistake; each was a different unstated definition.

| § | figures | cause |
|---|---|---|
| 23 | 16.2 / 10.7 / 10.4% | **YAML front matter** — 7 lines × 50 files, worth 4.3 points |
| 39 | 30.6% vs 6.5% at a higher threshold | **a corpus split at the file level** — 13 of 25 modules use the skeleton, the rest are free prose, so the answer is decided by where the threshold falls relative to 13/25 |
| 32 | 79.3% vs 73.4% | **blank lines counted as shared content** — §32 has 2,147 raw lines, 1,702 shared by all 23 modules (79.3% exactly), of which **483 are blank** |

The blank-line case is the one to guard against generally: whitespace is identical across every
file by construction, so counting it inflates any section by its own whitespace density — 22% of
§32's lines.

## What actually moves these numbers

Threshold, unit and definition, in that order of *usual* importance — but the ordering is not a law,
and two agents found the exception.

- **Threshold is usually inert.** §27, §28, §26, §07 and five of six sections in `crates/sweep`'s
  scope move by literally zero between thresholds of 0.3 and 0.9, because their sharing is bimodal:
  a line is either in one module or in all of them. §39 and §10 are the exceptions, and both are
  corpora split at the *file* level.
- **Unit often dominates.** Line versus heading-block costs ~10 points in §31 and §32 and ~20 in
  `sweep`'s six. §34's character figure is 8 points above its line figure, because what varies
  between its modules is short bullets while what repeats is an eight-step flow and a JSON object —
  by weight it is 82% the same document twenty-three times.
- **Definition matters in proportion to the share of the document the definitional slice covers.**
  This is `crates/sweep`'s correction and it is arithmetic rather than editorial. Front matter moved
  §43 by 15.1 points and §23 by 4.3, but only 1.8–2.5 in six sections whose modules run ~70 non-blank
  lines — seven front-matter lines cannot move that fraction further. It predicts §43's swing from
  its module length rather than from anything about its content.

**Instances and distinct texts answer different questions.** §11's shared core is 1,125 line
instances but only **40 distinct texts**, because two lines repeat inside each module. §19's is 66
instances from **2 distinct texts** — a horizontal rule and a date. §23's is 2.1% of distinct
strings accounting for 16% of occurrences. A section is repetitive because a small template is
stamped many times, and the distinct-string count sees that directly.

Use the distinguishing-lines-per-module column, and treat every percentage as a band with its method
attached.
