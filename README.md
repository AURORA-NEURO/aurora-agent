# bioprism

## Autonomous agent process boundary

The Python SDK includes a secret-safe operator entry point for the autonomous brain:

```bash
cd python
python -m prism_sdk catalogue
python -m prism_sdk evidence-plan --domain science
python -m prism_sdk provider-status --provider openai
```

Use `run` with a caller-owned MCP server when you are ready to invoke a provider. Keys are accepted
only through a hidden prompt or an explicitly named environment variable; they are never command
line arguments, MCP arguments, plans, or persisted state. See [the autonomous brain guide](docs/AUTONOMOUS_BRAIN.md#operator-process-boundary)
for model discovery, durable inventory refresh, model-selection, approval, and credential-lifecycle details.

**Query-compiled inference for executable biology.**

Implementation of the AURORA BioPRISM / OncoWorld / FIBER blueprint (v0.6, 935 registered spec
modules). A Rust workspace whose central idea is that **context assembly is a compiler pass**:
instead of retrieval plus summarisation plus vibes-based compaction, a typed decision query is
compiled into the smallest decision-sufficient evidence region, delivered as a **Decision
Section**, and accompanied by a **Context Certificate** that states exactly what was omitted and
whether the omission could have changed the decision.

> Compile the smallest decision-sufficient evidence region. Never traverse the whole knowledge
> structure by default.

## What the measurements actually say

The reference world ships 761 facts, 750 of them exploratory distractors that all consume the same
protected `cohort_id` hub. FIBER compiles the query down to **11 facts (1.45% of the world)** and
the deterministic oracle still returns the correct verdict with all four leakage witnesses.

**It is not alone in doing so.** Under equal tuning, a 5-hop incidence walk and a BM25 retriever at
k=11 select *exactly the same eleven facts*. The distribution's own `compare_baselines.py` measures
the graph baseline only at depth 7 and unbounded — the two settings where it returns everything —
and reports a 69× advantage that vanishes under equal tuning. That is a strawman comparison, and
correcting it is what 43.38 and 43.41 require.

So the reference world cannot tell these methods apart. [`crates/worldgen`](crates/worldgen) makes
the structure a parameter and builds one that can — distractors attached near the target instead of
at a hub leaf, decisive facts behind a relay chain, and distractor tags camouflaged to tokenise into
the protected vocabulary:

| Strategy | Facts | Sound? | Closure | Admissible |
|---|---:|:-:|---:|:-:|
| full-context | 762 | yes | 100% | yes |
| graph-5-hop | 750 | **no** | 0% | **no** |
| graph-7-hop | 750 | **no** | 0% | **no** |
| graph-11-hop | 761 | yes | 100% | yes |
| lexical-top-11 (BM25) | 11 | yes | **91%** | **no** |
| **fiber** | **11** | **yes** | **100%** | **yes** |

Three distinct failure modes appear. The graph walk has **no usable depth**: 5–10 pull in 98% of the
world *and still miss every decisive witness*; 11 is the first sound setting and by then it has
taken everything. BM25 reaches the *right verdict* from a **91% protected closure** — right by luck,
having dropped a protected fact that happened not to matter, and raising k to 50 never recovers it.
FIBER is the only admissible strategy: right verdict **and** full closure, at 11 facts.

That last failure is why the harness ranks on admissibility rather than verdict alone — ranking on
verdict would have crowned the strategy that violated the mandatory closure and got away with it.

This does not show FIBER wins generally: the discriminating world was built to expose these modes,
just as the reference world was built to expose hub expansion. Both are single points, the full
sweep is not done, and an embedding retriever and a *directed* dependency walk are still missing
from the panel. Full analysis: [docs/FINDINGS.md](docs/FINDINGS.md). How much of the blueprint the
workspace actually covers, and which sections have nothing standing in for them:
[docs/COVERAGE.md](docs/COVERAGE.md). The crate layout and the blueprint path:
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Status

**77 crates, 375,037 lines, zero clippy warnings across the workspace.** Byte-level parity with the
CPython reference runtime is enforced by test and holds across *three* implementations: CPython, the
Rust eager path, and the Rust indexed store.

The table below is generated. It used to be hand-maintained and drifted to claiming twenty-three
crates and 820 tests — the same hand-copy drift [`crates/devx`](crates/devx)'s exit-code audit
exists to catch, sitting in the README of the repository that wrote the audit. Regenerate it, and
the test count, with:

```bash
tools/status.sh --tests
```

The **Blueprint** column is derived rather than declared: it lists the sections whose module ids a
crate actually cites in its own source, using the token rule [`tools/coverage.sh`](tools/coverage.sh)
runs. A crate that stops citing a section drops it here without anyone remembering to edit a row.

How much of the blueprint is covered, and what the remainder is:
[docs/COVERAGE.md](docs/COVERAGE.md) and [docs/BACKLOG.md](docs/BACKLOG.md). Every uncovered module
carries a typed verdict in [`crates/residue`](crates/residue) explaining why nothing implements it.

<!-- generated by tools/status.sh at 725fa10 -->

| Crate | Blueprint | What it does |
|---|---|---|
| [`bioprism-adapter`](crates/adapter) | 04,28,40,43 | Data adapter contract with mandatory semantic-loss reporting |
| [`bioprism-adaptive`](crates/adaptive) | 08,43 | Adaptive evaluation: capability posterior, information-gain suite selection, parent-aware uncertainty |
| [`bioprism-atlas`](crates/atlas) | 03,33,43 | BioCapability atlas and metrics: capability ontology, coverage, failure atlas |
| [`bioprism-atlashub`](crates/atlashub) | 09,27,34 | BioAtlas surfaces: world cards, connector registry, value-of-experiment, federated evaluation, research CI |
| [`bioprism-atlasx`](crates/atlasx) | 34 | Capability atlas and public-hub remainder: coverage debt as a derived claim, and the failure-atlas browsing surface |
| [`bioprism-backends`](crates/backends) | 32,43 | Physical backend portfolio: variable elimination, worst-case-optimal joins, structural estimation and the honest fallback |
| [`bioprism-baseline`](crates/baseline) | 43 | Equal-engineering context baselines: full-context, k-hop incidence, connected component, lexical top-k and query-graph |
| [`bioprism-benchcompiler`](crates/benchcompiler) | 06,35 | Benchmark compiler: trajectory to decision cell, first causal divergence, minimization, oracle synthesis |
| [`bioprism-bioethics`](crates/bioethics) | 13,30,36 | Section 36 remainder: biology security, privacy, ethics and governance beyond policy and safety |
| [`bioprism-bioeval`](crates/bioeval) | 26,31,43 | Biological evaluation engine: scoring planes, partial credit, biological error classes |
| [`bioprism-bioevalx`](crates/bioevalx) | 07,26 | Bio evaluation engine remainder: scoring planes, reader models, adjudication and the evaluation contract |
| [`bioprism-bioir`](crates/bioir) | 25,39 | Biological IR: BioWorld, specimen lineage, AssayLens, cohort and split, uncertainty and reference standards |
| [`bioprism-biolang`](crates/biolang) | 25,28,39,43 | The biological IR family and BioQL: typed world, state, intervention, worldline, oracle, mutation and bundle representations |
| [`bioprism-bioworlds`](crates/bioworlds) | 30,38,43 | Reference bioworlds and vertical slices: worlds built to make blocked platform claims exercisable |
| [`bioprism-bundle`](crates/bundle) | 10,12,13,21,23,34,43 | Signed result bundles and reproduction: attestation, replay, and what symmetric authentication cannot promise |
| [`bioprism-choreography`](crates/choreography) | 23 | Multiparty choreography: session types with projection, bounded protocol model checking, adjudication, quorum with checked independence, and sagas with honest compensation |
| [`bioprism-cli`](crates/cli) | 40,43 | The bioprism command-line interface |
| [`bioprism-conformance`](crates/conformance) | 14,40,43 | Conformance suites, the test pyramid and release quality gates |
| [`bioprism-cookbook`](crates/cookbook) | 03,11,13,14,19,21,38,39,40,41,43 | Reference examples: worked recipes with the claim each one demonstrates and the property a reader can check |
| [`bioprism-dataops`](crates/dataops) | 12 | Section 12 remainder: storage topology, relational catalog, SLOs, compute placement and federated deployment, each answer carrying the basis it was known from |
| [`bioprism-devplat`](crates/devplat) | 11,19 | Developer platform contracts: evidence-aware authoring workbench, cross-domain mission DAGs, digest-bound capability discovery, notebook/session audit, capability dashboard queries, CI planning, and reference-example predicates |
| [`bioprism-devx`](crates/devx) | 11,23,38,39,40,41,43 | Developer platform: machine-actionable diagnostics, compile introspection, the local-loop invalidation contract and the 23.32 debugger surface model |
| [`bioprism-docgraph`](crates/docgraph) | 39,41,43 | Documentation graph: module registry, edge vocabulary, context cards, task routes, bundle compiler, change impact |
| [`bioprism-epistemic`](crates/epistemic) | 43 | The remaining FIBER calculus: decision-equivalence quotient, coverage-aware selection, separator protocol, rate-distortion and value of information |
| [`bioprism-evalengine`](crates/evalengine) | 06,07,43 | Evaluation engine: the deterministic-first scoring ladder and causal component attribution |
| [`bioprism-examples`](crates/examples) | 19,38,39,40,43 | Reference BioWorlds and runnable vertical slices |
| [`bioprism-fabric`](crates/fabric) | 23,43 | Interweave fabric above the microkernel: composition algebra, effect and information flow, contextual reputation, common ground, semantic lifecycle |
| [`bioprism-factory`](crates/factory) | 40 | Job, worker, lease and recovery lifecycle with idempotency-aware retry |
| [`bioprism-fiber`](crates/fiber) | 39,40,43 | The FIBER query compiler: protected closure, dependency slicing, temporal cut and certificate emission |
| [`bioprism-foundation`](crates/foundation) | 24,40 | BioPRISM foundation objects: the executable-biology thesis made typed |
| [`bioprism-governance`](crates/governance) | 14,25,40,43 | Schema versioning, migration, deprecation and compatibility gates |
| [`bioprism-graph`](crates/graph) | 40,41,42,43 | Generated graph, hypergraph, timeline and table projections over compiled decision regions |
| [`bioprism-hub`](crates/hub) | 34,36,43 | BioAtlas public hub: submission, moderation, provenance and ecosystem contracts |
| [`bioprism-hubapi`](crates/hubapi) | 10 | Registry and hub surface: discovery, resolution, mirroring, offline operation and trust propagation |
| [`bioprism-ids`](crates/ids) | 11,40,43 | Canonical serialization, content hashing, and typed identifiers for AURORA BioPRISM |
| [`bioprism-influence`](crates/influence) | 43 | Sound numeric influence bounds: the formal influence bounds the reference slicer's limitation string says it lacks |
| [`bioprism-infra`](crates/infra) | 12,40 | Data infrastructure: provable cache hits, invalidation that reports its completeness, quality gates, tiering, lifecycle and storage quota |
| [`bioprism-interweave`](crates/interweave) | 23 | Section 23 remainder: interweave modules weave, fabric, choreography and weavelang did not claim |
| [`bioprism-lab`](crates/lab) | 05,09,39 | Inference Lab: hypothesis separation, architecture search, Pareto fronts, evolution cards, holdout and rollback policy |
| [`bioprism-ledger`](crates/ledger) | 12,40 | Append-only event ledger with valid/record/release time, projections and checkpoints |
| [`bioprism-lens`](crates/lens) | 03,33,42,43 | Graph lens grammar: the typed lens catalogue behind the evaluation hub, and the non-visual contract |
| [`bioprism-mcp`](crates/mcp) | 11,43 | Model Context Protocol server exposing the FIBER context compiler to agents |
| [`bioprism-megafactory`](crates/megafactory) | 35 | Section 35 remainder: million-scale factory modules scale and factory did not claim |
| [`bioprism-metrics`](crates/metrics) | 03,33,43 | BioCapability metrics: aggregation/comparability, bounded descriptive analytics, and what a capability number may not claim |
| [`bioprism-modalities`](crates/modalities) | 28,30,43 | Modality data standards: what each assay family measures, what it cannot, and when two modalities are comparable |
| [`bioprism-mutation`](crates/mutation) | 03,40 | Metamorphic mutations with executable postconditions, lineage, deduplication and effective-diversity accounting |
| [`bioprism-obligation`](crates/obligation) | 39 | Decision obligation graph, BioContext capsule and the token budget controller |
| [`bioprism-onco`](crates/onco) | 30,43 | OncoWorld: neuro-oncology domain model, longitudinal tumour worldlines, response criteria, molecular classification |
| [`bioprism-oncoworlds`](crates/oncoworlds) | 30 | OncoWorld domain depth: identity spine, clonal evolution, methylation classes, cross-modal and cross-system transport, era and site shift |
| [`bioprism-ops`](crates/ops) | 40 | Operational contracts of blueprint §40: configuration and feature flags, observability and audit, the capacity model, hardening, and the alpha acceptance criteria as predicates |
| [`bioprism-oracle`](crates/oracle) | 11,31,40 | Oracle mesh: provider SDK, the deterministic-to-judge evidence ladder, set-valued combination and disagreement adjudication |
| [`bioprism-oraclex`](crates/oraclex) | 31,32 | Reference standards as claims about measurement processes, and the mutation validation program that decides whether a transformed case may be released |
| [`bioprism-packs`](crates/packs) | 03,15,29 | Benchmark pack taxonomy and portfolio definitions |
| [`bioprism-policy`](crates/policy) | 13,36,39,43 | Policy, privacy and information-flow fibers: consent, purpose, residency, role visibility, redaction |
| [`bioprism-prism`](crates/prism) | 03,40,43 | Decision Cells, matched counterfactual forks, state minimization and attested result bundles |
| [`bioprism-registry`](crates/registry) | 10,27,40,43 | Benchmark packs, promotion, trust tiers and the CI release gate |
| [`bioprism-residue`](crates/residue) | — | The explained residue: every uncovered blueprint module with the reason no crate implements it |
| [`bioprism-routing`](crates/routing) | 09,43 | Evaluation-conditioned inference routing: pick a context architecture from prior evidence |
| [`bioprism-runtime`](crates/runtime) | 05 | Execution runtime: run orchestrator, executor providers, WorldTape, fork/replay, virtualization, effects broker, budget controller |
| [`bioprism-safety`](crates/safety) | 05,13,40 | Platform security and safety: threat model, trust boundaries, prompt injection, poisoning, supply chain, disclosure |
| [`bioprism-scale`](crates/scale) | 35,40 | Million-scale factory: effective size, hidden-family splits, prospective escrow, cost accounting, content-addressed storage |
| [`bioprism-scope`](crates/scope) | 43 | Typed scope base: identity, region, specimen, time, coordinate, ontology and policy validity contexts |
| [`bioprism-sdk`](crates/sdk) | 11,23,40,43 | Plugin and extension SDK: registration, capability declaration, version negotiation |
| [`bioprism-section`](crates/section) | 39,43 | Decision Section IR and Context Certificate: the model-facing context ABI and its omission receipt |
| [`bioprism-services`](crates/services) | 10,40 | Build-ready service contracts: request/response shapes, error taxonomy, versioning, the process graph |
| [`bioprism-standards`](crates/standards) | 25,28,39,43 | Biology data standards: ontology binding, units, coordinate frames, reference builds |
| [`bioprism-stewardship`](crates/stewardship) | 14,43 | Governance and quality: the checkable parts of section 14, and an honest account of which modules are process rather than code |
| [`bioprism-store`](crates/store) | 43 | Content-addressed indexed world storage: point lookups that do not scale with corpus size |
| [`bioprism-stress`](crates/stress) | 30,32,38 | Biological stress program: prevalence shift, batch and site effects, assay uncertainty |
| [`bioprism-sweep`](crates/sweep) | 03,04,05,08,10,13,39,43 | The small remainders: core specifications, ingestion, execution runtime, adaptive, registry and safety tails |
| [`bioprism-tokens`](crates/tokens) | 39 | Token-efficient biological inference: golden context fixtures, staleness and recomputation, ablation design, multi-agent projection, summarisation contracts |
| [`bioprism-trace`](crates/trace) | 03,39 | Trajectory ingestion, decision segmentation, first-divergence localization and Decision Cell compilation |
| [`bioprism-weave`](crates/weave) | 23 | The Weave microkernel: typed acts, commitment and epistemic ledgers, attenuating authority, affine budgets, context capsules and continuations |
| [`bioprism-weavelang`](crates/weavelang) | 23 | WeaveLang and WeaveIR: surface syntax, canonical IR schema, compiler pipeline, operational semantics |
| [`bioprism-world`](crates/world) | 40,43 | FIBER world model: local evidence sections, typed factors and the causal event structure |
| [`bioprism-worldfactory`](crates/worldfactory) | 03,10,27,34,35 | Parent bioworld authoring and the biomutator: observed, semi-synthetic and mechanistic worlds, assay-fault and contradiction programs |
| [`bioprism-worldgen`](crates/worldgen) | 38,43 | Synthetic structural benchmark families: worlds whose topology, depth and tag informativeness vary independently |

### Cross-language parity

Certificate hashes are taken over canonical bytes, so Rust and Python must agree exactly or a
certificate produced by one cannot be replayed by the other. Both the Decision Section and the
Certificate are byte-identical to `reference/fiber_runtime/fiber_compile.py`:

```
certificate_sha256      c0da17ffc80465258345c8a538171bfd868100cd883e9a20780a0dc5477e7ea4
decision_section_sha256 7439b2262c52c1c794b59be86d922b723a2ea5646362d529f57fb11b5f7e93ce
world_sha256            b3809731cf93040fcd8aef43deb2a552492064b49154e07ea58caa724c10cbb5
```

Getting there required matching CPython in two places a naive port gets wrong: `repr` float
formatting (CPython switches to exponential at a different threshold than Rust and zero-pads the
exponent) and JSON object iteration order, which the reference relies on when building leakage
witnesses.

## Quickstart

```bash
cargo build --release --offline
```

```bash
./target/release/bioprism context explain --world fixtures/fiber-v0.1/radiogenomic_world.json --query fixtures/fiber-v0.1/leakage_query.json
```

That prints a database-style explain plan: which passes ran and what each retained, the backend,
selection ratios, omissions grouped by influence class, the oracle verdict with its witnesses,
and — importantly — **which passes did not run and why**.

```bash
./target/release/bioprism --json context compile --world fixtures/fiber-v0.1/radiogenomic_world.json --query fixtures/fiber-v0.1/leakage_query.json --certificate-out cert.json
```

```bash
./target/release/bioprism context verify --certificate cert.json
```

### Scale

Compiling from a JSON document parses the whole world on every query. Index it once instead:

```bash
./target/release/bioprism world index --world big-world.json --store big-world.bpw
```

`--world` then accepts the store directory anywhere it accepted a document, and the certificate is
identical. On a one-million-fact world this takes query time from **26.5 s to 41.6 ms (638×)**, and
compile cost becomes roughly logarithmic in corpus size rather than linear. The reasoning and the
full measurements are in [ADR-001](docs/ADR-001-language-strategy.md).

### Exit codes

Ten codes, and **every failure code carries exactly one retry decision**, so a caller holding
nothing but the process status can decide whether to re-send. `bioprism --help` prints the table;
`--json` puts the same decision in the envelope as `error.retryability`.

| code | | decision | code | | decision |
|---:|---|---|---:|---|---|
| 0 | `ok` | — | 5 | `io` | `retryable_as_is` |
| 1 | `assertion_failed` | — | 6 | `conflict` | `terminal` |
| 2 | `usage` | `terminal` | 7 | `policy_denied` | `retryable_after_change` |
| 3 | `invalid_input` | `terminal` | 8 | `indeterminate` | `retryable_after_change` |
| 4 | `compile_failed` | `retryable_after_change` | 9 | `stale` | `retryable_as_is` |

Codes 0 and 1 report a verdict rather than a failure — the checked property held, or it did not —
so they publish no retry decision rather than a third state every consumer would special-case.

**This is a breaking change.** The registry previously had six codes, and 6–9 were all `4
compile_failed`. Two of them are the reason for the split: a script reading exit 4 could not tell a
policy refusal from an oracle abstention from a snapshot that had moved under it, and `stale` was
advertised as *not* retryable when re-reading and re-sending the identical request is exactly what
clears it. `bioprism-devx`'s exit-code audit found both against blueprint 40.36 and now reports
neither; the registry it found them in is retained there as the audit's known-positive input.

## Using it from an agent

```bash
./target/release/bioprism-mcp --root .
```

Speaks JSON-RPC 2.0 over newline-delimited stdio. The session follows the MCP lifecycle: the client
calls `initialize`, waits for the `notifications/initialized` acknowledgement, and only then calls
tools or resources. `fiber_compile` returns the **L0 decision contract** — goal, verdict, what was
omitted, whether the sufficiency claim holds — plus a versioned, content-addressed refinement
handle, and *not* the evidence. An agent passes that handle to `fiber_refine` only when the contract
is insufficient to act; the server recompiles and verifies the certificate digest before disclosing
the requested layer. On the reference world L0 is ~204 estimated tokens against ~1,900 for the full
section.

The invariant that makes that safe: **omissions are reported at every layer**, so an agent that
stops at L0 still knows what it does not have. Layering hides volume, never the fact of an
omission. Paths are confined to `--root`; absolute paths, `..`, and symlink escapes are refused.
The three shipped JSON schemas and the capability catalog are available through read-only MCP
resources, so a client can build valid documents and route work without reading arbitrary files.
`world_index` previews its write unless called with `confirm: true`.

The repository also ships a dependency-free Python client in [`python/`](python/README.md). It
supports synchronous and asyncio MCP sessions, enforces the initialize/initialized lifecycle,
keeps transport/protocol/remote-refusal errors distinct, bounds JSON-RPC frames, and provides thin
helpers for `developer_delivery_audit`, `developer_workbench`, `developer_workbench_verify`, `developer_workbench_import`, `developer_workbench_query`, `developer_workbench_get`, `ci_provider_normalize`, `ci_execution_evidence_audit`, `agent_mission`, `capability_discover`, `mission_evaluator_discover`, `mission_evaluator_review`, `mission_evaluator_replay`, `capability_audit`, `capability_dashboard`, `capability_route`, `adapter_plan`, `tabular_ingest`, `conformance_run`, `release_audit`, `operations_catalog`, `ops_acceptance`, `safety_release_gate`, `medical_boundary_check`, `biocapability_evidence_audit`, `bioql_compile`, `world_claim_check`, `observed_world_declare`, `lineage_audit`, `preanalytic_apply`, `contradiction_review`, `lab_plan`, `onco_boundary_check`, `onco_response_assess`, `onco_worldline_view`, `onco_classification_check`, `oncoworlds_identity_join`, `onco_outcome_analyze`, `oracle_combine`, `oracle_reference_panel`, `oracle_missingness`, `bioeval_reference_audit`, `evaluation_worldline_audit`, `evaluation_reproduction_check`, `evaluation_trajectory_check`, `routing_decide`, `repository_catalog`, `repository_bundle`, `repository_impact`, `telemetry_project`, `bioatlas_publication_audit`, and the full `fiber_compile` → `fiber_refine`/`fiber_explain`/`fiber_verify` → `projection_bundle` lifecycle. Typed evaluator-candidate discovery, reviewed binding, replay, audit, dashboard, delivery, evidence, publication, adapter-plan, tabular-ingest, conformance, release-audit, operations, safety, lineage, pre-analytic, contradiction, inference-lab, oracle/evaluation, and oncology-boundary projections retain cross-domain metadata, schema-quality evidence, parity gaps, readiness gates, explicit blockers, claim prerequisites, omission accounting, publication gates, candidate refusal reasons, semantic-loss boundaries, conformance checks, fixture drift, delegated refusal state, advisory-only observations, storage promise parity, service-contract divergence, metric debt, three-way acceptance verdicts, risk-gate decision drivers, unrated dimensions, structured clinical refusal, partial aggregate release, privacy exclusions, tiered evidence ledgers, temporal leakage witnesses, reproducibility divergence, and strict release-conjunction evidence for operators and SDK callers. It is an
integration foundation above the Rust kernel, not a claim that the full Python data-adapter,
benchmark-statistics, or biological-format ecosystem is complete. Its authoring layer now builds
digest-bound packs, decision cells, deterministic mutation plans, versioned oracle judgements,
reference-panel requests, evaluation requests, and bounded FHIR JSON/NDJSON, FASTA, FASTQ, SAM, GFF3, PDB, SDF/MOL, mzML, DICOM, NIfTI, AnnData, VCF, BAM, and OME-Zarr
projection audits, plus bounded heterogeneous projection batches, while leaving final health and
oracle decisions to Rust. `prism_sdk.ApiClient` and
`AsyncApiClient` also speak the bounded HTTP gateway described in [`docs/HTTP_API.md`](docs/HTTP_API.md).

The Python clients also expose `capability_route_plan`, which composes caller-selected route
candidates with authoritative mission preflight across MCP and REST. It returns a digest-bound
mission and `plan_digest` with explicit `dispatch: "not_started"`; route-review and preflight
blockers remain structured, and no nested domain tool is dispatched.
They also expose `capability_route_plan_verify`, which rechecks a retained plan without dispatch;
supplying the original route and selections enables full route-review replay, while a shape-only
check is reported explicitly as `verified_without_route_replay`.

For every current or future MCP domain, the Python layer also exposes a schema-aware fallback:
`tool_catalogue()` snapshots the live definitions, `plan_tool()` performs bounded transport-shape
preflight, and `tool_checked()` executes only after that review. This does not claim domain
validity or suppress refusals; unsupported schema features remain visible as warnings.
Mission requests can additionally pass through `mission_preflight()` for digest-bound graph,
wave, binding, authorization, and per-step schema review before the Rust mission executor is
called. The executor is serial by default; an explicit `execution_mode: "parallel_waves"` policy
dispatches independent wave members concurrently with bounded width and reserved output budget.
Executed missions also return a deterministic clock-free trace of lifecycle, wave, step, refusal,
block, digest, and byte-accounting transitions.
Mission requests can additionally provide bounded caller-authored `claim_requests`; terminal reports
then include a non-semantic `claim_lineage` projection that maps each claim to explicit step results,
retained-output digests, omission states, and durable non-claims. The HTTP gateway exposes the same
projection at `/v1/missions/{mission_id}/claims`, and the Python/TypeScript clients provide typed
helpers for it. `claimable` describes retained evidence posture only: it never means the claim is true
or release-ready. Claims can also declare explicit evaluator/adapter bindings to source-step output
pointers; coverage and pointer/refusal/omission posture are reported separately, so every domain can
plug in a named evaluator without giving the orchestration layer semantic authority. Multiple
retained evaluator outputs also expose canonical-digest agreement/disagreement as an explicit
witness, never as an automatic adjudication.
Retained outcomes also distinguish refused, blocked, cancelled, output-omitted, pointer-missing, and
successful evaluator rows, including output source/type/size and digest groups. A ready
`mission_evaluator_review` can be supplied back as `evaluator_review`; `agent_mission` rechecks its
catalogue digest and exact binding rows before any nested call, then preserves review provenance in
the report and claim lineage.
The Rust executor also performs bounded authoritative JSON Schema preflight against the live
`tools/list` definitions: static arguments are checked before a mission is accepted or planned,
and bound arguments are checked again after upstream payloads are materialized, before either
serial or parallel nested dispatch. Refusals include the schema digest and bounded JSON-pointer
diagnostics, so malformed calls cannot be mistaken for domain-level refusals or successes.
The HTTP gateway adds bounded asynchronous mission jobs with typed status polling and cooperative
cancellation between nested calls or parallel batches; a cancellation report records what completed
and what was never dispatched rather than implying force-kill or rollback.
`POST /v1/missions/preflight` provides the matching synchronous handoff: it validates the original
execution policy and static schemas, returns the authoritative digest-bound plan, and forcibly
marks dispatch as `not_started`. It never creates a job or invokes a domain tool.
`GET /v1/missions` provides a bounded deterministic inventory with status filtering, lifecycle
links, and step/refusal/byte summaries without returning unbounded terminal reports.

For browser and Node consumers, [`typescript/`](typescript/README.md) provides the corresponding
dependency-free Fetch client. It enforces request/response bounds, timeout and abort semantics,
typed API errors, SSE cursor parsing, webhook outbox lifecycle, and typed facades for the evidence,
BioAtlas, OTLP, runtime, bioethics, and developer-delivery workflows. See [`docs/TYPESCRIPT_SDK.md`](docs/TYPESCRIPT_SDK.md)
for the compatibility, workbench, mission, secret-handling, and schema-aware full-catalogue
invocation contract. `toolCatalogue()` and `planTool()` make arbitrary domain calls reviewable
before `toolChecked()` executes them; `missionPreflight()` extends that review across dependency
graphs, bindings, and execution policy before `agentMission()` is sent. Remote refusals remain
visible rather than becoming success. `missionFromRoute()` connects the generic capability
catalogue to that review while keeping candidate selection and arguments explicit.

The repository ships `bioprism-api` for deployments that need a network boundary:

```bash
cargo run -p bioprism-api -- --root . --bind 127.0.0.1:8787 --token <visible-token> \
  --mission-state .local/mission-state.json --mission-queue-state .local/mission-queue.json \
  --event-state .local/event-state.json \
  --reconciliation-state .local/reconciliation-state.json
```

It exposes the exact MCP tool catalogue through REST and JSON-RPC, bounded health/capability
routes, cursor-addressable event pages/SSE snapshots, receipt-correlated event queries, signed webhook outbox registration, retry,
and acknowledgement. `--mission-state` adds an optional bounded, atomic checkpoint for mission
status, progress, traces, and size-limited result metadata; interrupted queued/running missions
are marked failed after restart instead of being falsely resumed. Mission checkpoints emit schema
2 with a content SHA-256 `state_digest`; schema-1 snapshots are
accepted for migration and rewritten after startup, while tampered schema-2 state is rejected.
Persistence status reports both the digest and observation-time `integrity_verified` state.
`--mission-queue-state` adds a separate content-addressed factory checkpoint for mission leases,
idempotency class, attempts, staged/committed output boundaries, and explicit startup recovery.
`--mission-queue-max-jobs` and `--mission-queue-max-active-leases` add explicit local queue
backpressure; the queue status reports per-resource-class fair-share limits and observed lease
occupancy. Each lease attempt is also a fencing token, preventing stale attempts from committing
after recovery. The queue checkpoint is now an execution-authority envelope: queue state and a
bounded hash-chained transition journal are atomically replaced together, and cooperating API
processes sharing the same local filesystem serialize mutations through a bounded lock. Status
reports both the queue digest and authority digest, revision, event count, lock state, and
integrity result. `POST /v1/missions/queue/authority/release-lock` is an attributed, audited
operator override for a lock whose owner is known to be gone. This is local shared-file
coordination; it does not provide tenant isolation, multi-host consensus, or network-partition
tolerance.
`GET /v1/missions/queue` exposes that queue projection without returning the original mission
specification. Expired idempotent work is requeued and ambiguous non-idempotent work is quarantined,
but no recovered job is automatically dispatched; the authority is a local recovery and audit
boundary, not multi-host scheduling, provider authentication, or proof of external effect
completion.
`--event-state` checkpoints retained events, subscription metadata, and signed pending outbox rows while never persisting
webhook secrets; the current schema-5 checkpoint is content-addressed with a SHA-256
`state_digest`, and startup rejects tampering before restoring rows. It also retains a bounded,
cursor-addressable delivery-attempt journal for enqueue, send, retry, replay, acknowledgement,
and secret-rebind outcomes without claiming receiver state beyond explicit worker acknowledgement;
receipt-bearing attempts also retain the validated receipt ID and content digest for exact joins.
Schema-1 through schema-4 checkpoints remain readable for migration and are upgraded on the next flush. Restored
subscriptions pause until an explicit in-memory `/rebind` call. It deliberately reports
gRPC, TLS termination, distributed scheduling, and external delivery as absent rather than
inferring them from an HTTP listener.
`GET /v1/recovery` and the Python/TypeScript `recovery_matrix`/`recoveryMatrix` helpers provide
one operator matrix that keeps mission restoration, event rows, subscription metadata, pending
outbox evidence, delivery-attempt provenance, secrets, and external effects separate. The
`GET /v1/operations/snapshot?after=N&limit=M` route and matching typed SDK helpers compose that
matrix with one bounded event page, event metrics, mission status counts, persistence digests,
capability transport flags, exact domain-group/tool coverage, and actionable operator follow-ups.
The same snapshot includes `reconciliation_summary` plus reconciliation checkpoint status:
stored report counts are split into completion statuses, structural-ready rows, explicit review
requirements, integrity-invalid rows, and evidence-invalid rows. These are derived audit counters
only; they do not authorize execution or upgrade a domain, scientific, clinical, safety, or release
claim.
The summary also carries a per-workflow status matrix and distinct workflow count, so the
cross-domain view cannot hide an unobserved or failed capability group inside one aggregate.
The domain projection compares the authoritative workspace capability groups with the advertised
tool catalogue, preserving missing names and omission counts without inferring semantic readiness.
It is designed as a dashboard
bootstrap and handoff surface: it never returns unbounded mission reports, executes no tools, and
does not turn local observations into scientific validity, receiver acceptance, or automatic
recovery claims. The event cursor remains authoritative, so consumers should persist
`recent_events.next_after` and inspect `gap` before declaring continuity.
`POST /v1/operations/handoff` turns caller-selected domains or capability groups into a
content-addressed, non-executing `capability_route` request. It preserves unresolved selectors,
catalogue gaps, complete-group omissions, and explicit next steps through capability review and
mission preflight; it never dispatches the generated route or authorizes execution.
`GET /v1/operations/domains?after=N&limit=M` adds bounded local activity observations per
capability group, allowing operators to distinguish catalogued-but-unobserved tools from tools
that actually emitted events in the requested cursor page. This is activity evidence only, not
runtime, scientific, safety, or release readiness.
`GET /v1/operations/gates?after=N&limit=M` turns the same bounded page into separate catalogue,
activity, transport-completion, pooled evaluation, domain-evaluator, safety, and release evidence
gates for every capability group. Domain-evaluator evidence is bound to a completed evaluation
tool by exact name or the workspace catalogue; it does not assert scientific validity, evaluator
calibration, or independence. A completed local call is never promoted into a readiness verdict: groups remain
`catalogue_blocked`, `insufficient_evidence`, or `review_required`, with `readiness_claimed: false`.
Each group also carries `gates.reconciliation_evidence`, joined only by the exact capability-group
`workflow_id` against the bounded digest-valid reconciliation registry. `missing` means no retained
matching report and never passes by inference; `incomplete` or `invalid` retained posture forces
`insufficient_evidence`; `structurally_ready` remains review-required evidence and is never a release,
safety, clinical, or scientific authorization. The summary exposes `groups_reconciliation_blocked`,
and the same posture is typed by the Python and TypeScript SDKs, so all currently advertised workspace
groups receive the same fail-closed join contract.
The same gate response now carries an advisory `gates.artifact_evidence` posture for every group.
It counts only records already admitted to the digest-verified artifact registry, matching explicit
registration domains after case normalization or an artifact body's explicit `group_id`; it never
infers membership from subjects, kind names, or free text. The posture reports artifact families,
verification states, parent-linked records, match basis, and registry generation/size. Missing
artifact evidence remains visible but is not a required gate and cannot change `gate_state` or
create readiness. Python exposes a typed `OperationsArtifactEvidencePosture` with an explicit
legacy-response fallback, and TypeScript exposes the corresponding group/summary posture fields.
Handoffs now carry an `operations_gate_acceptance` execution prerequisite; preflight binds the
mission’s exact tools to matching capability groups and the current `gate_digest`, while executable
HTTP missions are refused until an operator acceptance covers every required gate for every group.
Operators can persist that acceptance through `POST /v1/operations/gate-reviews` and replay it by
content-addressed `review_id`; executable missions require the retained review record to survive
the same event checkpoint and still match current evidence.
Accepted executable missions retain a `bioprism-mission-execution-provenance/0.1` projection in
mission status, inventory, and `/v1/missions/{mission_id}/provenance`. It correlates the review,
gate digest, domain-evaluator evidence, bounded preflight projection, and the accepted-dispatch
event; mission checkpoints retain it when `mission_state_path` is configured. It is an audit and
replay boundary, never a readiness or scientific-validity claim.
`GET /v1/webhooks/subscriptions/{id}/attempts` route and matching SDK helpers expose the
provenance cursor with explicit retention gaps and dropped-row accounting. Receipt-bearing rows
are also available through `/v1/delivery-receipts/{receipt_id}/attempts`, which joins the same
evidence across subscriptions without claiming external receiver state.
Embedded Rust consumers can plug an egress-controlled `DeliverySender` into
`ApiRouter::deliver_once(...)` to acknowledge successful signed webhook sends and classify bounded
retryable/permanent failures without giving the gateway arbitrary network access.
Delivery pages expose pending, retryable, failed, exhausted, and `secret_rebind_required` state with the last transport error;
`POST .../{id}/replay` is an explicit operator reset that preserves the delivery ID, resets the
attempt budget, and re-signs without claiming delivery.
The serving path uses one immutable shared router across connection threads, atomically allocates
request IDs, and clones ready MCP dispatch sessions per request. Mission, event, subscription, and
delivery state remain independently bounded and synchronized, so unrelated domain calls do not
serialize behind a global router mutex.

The same server exposes the broader workspace: `world_validate` checks a world before compilation,
`context_compare` runs the equal-engineering baseline panel, `bioworlds_catalog` runs the reference
vertical slices, `modality_catalog` exposes assay resolution and failure-mode contracts,
`modality_support_check` evaluates typed claim eligibility and analysis-unit independence across
the 17 modality families; `modality_transport_check` reports loss and fidelity, and
`modality_comparability_check` preserves modality-first refusals,
and `literature_bind_check` binds source claims to typed populations and historical horizons while
keeping citation support separate from biological measurement support; reviews cannot be silently
laundered into primary evidence, unstated populations refuse, and flagged sources require a
recorded warrant,
`mutation_family` validates metamorphic families with effective diversity, `prism_minimize` reduces
and re-checks a diagnostic world, `registry_gate` fail-closes attested benchmark packs,
`registry_lifecycle_simulate` replays the local content-addressed publication lifecycle with
continuation state, append-only events, supersession, withdrawal, promotion, demotion and integrity
verification,
`operations_catalog` executes the local/team topology parity and service-contract audit while
keeping undefined metrics explicit,
`capability_rank` compares serialized metric vectors without collapsing holes or trade-offs and
can apply a declared weighting with sensitivity evidence, while `research_ci_check` runs the
claim, split, figure, regression, environment, egress, non-claim and provenance predicates,
`metrics_profile_audit` emits per-capability leaders, measured populations, missing systems and
uncontested-lead warnings for public-card construction without inventing a scalar score,
`biocapability_evidence_audit` composes metric profiles with explicit evidence states across
grounding, information acquisition, resource efficiency, temporal validity, cross-modal agreement,
causal identification, reproducibility, translation maturity, and multi-agent coordination. It
validates support fields, blocks future evidence and unknown dimensions, keeps declared evidence
visible without counting it as measured, and releases only explicitly requested claims whose required
dimensions are eligible; optional information-value, reference-distribution, worldline, and
reexecution subaudits remain bounded projections rather than biological truth or clinical inference,
`cache_invalidation_simulate` rebuilds typed cache keys and replays dependency-aware invalidation,
partial unknown regions, fail-closed lookup misses and explicit re-proving, while
`storage_lifecycle_simulate` plans pin-aware hot/warm/cold tiering and non-copyable quota
delegation with reserve-protected accounting,
`policy_screen` enforces caller-supplied policy rules before selection and preserves typed
refusals, `safety_posture` reports section-13 threat populations without claiming runtime
enforcement, and `safety_release_gate` applies the complete dual-use risk gate with unrated dimensions
still blocking, `hub_search` performs bounded federated exact-facet discovery with typed authority,
tier, digest, freshness, and near-miss provenance, `measurement_compare` checks standards
declarations without silent unit or ontology coercion and returns typed conversion receipts and
first blocking reasons; `governance_schema_check` checks the shipped schema contracts,
`medical_boundary_check` admits research use cases and structurally refuses clinical outputs,
`tabular_ingest` runs the real CSV/TSV adapter with independent conformance and loss accounting,
`observed_world_declare` seals pinned observed-world declarations, `world_claim_check` enforces
the provenance claim ladder, `hub_resolve` resolves a federated pack request with digest and
freshness provenance, `hub_lock` builds a transitive provenance-preserving dependency lock,
`safety_posture` reports residual threat populations, and `security_redteam_simulate` replays the
section-13 safety loop across confirmed-finding regression cells, sequential vulnerability
disclosure, evaluator/artifact trust boundaries, across-trial feedback paths, incident blast-radius
containment gates, forensic timelines, hash-linked audit records, and observed-versus-asserted
attestations. It keeps the crucial nonclaims beside every result: this is a bounded contract
simulation, not a fuzzer, runtime sandbox, detector, credential revoker, incident channel,
containment executor, notification service, or durable audit store. `weave_protocol_catalog`
exposes typed agent-act antecedents.
`bioatlas_publication_audit` composes atlas coverage, optional evidence-conditioned claim readiness,
moderation/card rendering, and leaderboard ranking into explicit publication targets. It keeps atlas
holes, withheld scores, unranked entries, and absent evidence visible; numeric public scores require
both the disclosure-gated card result and an evidence audit, and no release claim is emitted without
an explicit target request. It remains an in-memory contract workflow rather than a web publisher,
identity service, assay runner, leakage detector, scientific truth oracle, or clinical approval.
`bioethics_action_review` partitions research plans from physical actions and only produces an
external referral after both required human approvals are present; `bioethics_human_subject_screen`
keeps institutional review, consent, and return-of-results checks separate; `bioethics_dual_use_review`
adds an explicit misuse-surface assessment in front of the section-13 release gate;
`bioethics_validation_check` audits evidence completeness and independent reproduction; and
`bioethics_representation_audit` preserves unmeasured and small-cell-suppressed strata while
refusing attribution across unmatched resource context.
`influence_analyze` computes caller-scoped numeric influence bounds over declared factor regions,
defaults to structural-only analysis, and keeps unknown preconditions distinct from vacuous bounds.
The Python and TypeScript SDKs expose the same factor-region request and report boundary, including
hard budgets, attempted-method provenance, exact versus conservative validity, and typed unknown
reasons; they never turn an uncomputable influence into infinity or a fabricated numeric bound.
`routing_decide` selects only among an explicitly approved architecture panel, abstains on weak
coverage or margins, and refuses held-out evidence leakage when a task identity is supplied;
the Python and TypeScript SDKs preserve the selected architecture, structured abstention reason,
considered panel, neighbourhood evidence, confidence score, and holdout check without treating a
safe-default abstention as a routing win;
`token_context_plan` checks mandatory token closure, dry-run restricted-data privacy, and policy-only
comparisons while preserving estimator provenance; `bioql_compile` type-checks explicit biological
schemas for units, frames, builds, clocks, labels, provenance, and cost bounds without executing a
query; `weavelang_compile` compiles source to
deterministic WeaveIR and can inspect or replay its local semantics, with replay as the default and
world-mutating transitions refused.
`projection_bundle` derives graph, hypergraph, timeline, and table views from the same compiled
section and certificate, preserving provenance, fidelity, and unresolved-obstruction coverage;
view bodies are opt-in and are never treated as proof.
`lens_catalogue` exposes the implemented section-42 questions, evidence requirements, scope
preconditions, and declared refusals before a run; `lens_leakage_check` executes the typed cohort
leakage lens with sealed nonvisual witness rows, explicit underdetermination, and no split repair.
`choreography_check` checks serialized multiparty protocols, projects every role, and preserves
bounded or inconclusive model-checking results; `conformance_run` verifies shipped fixture
digests before running the FIBER suite and returns its noncompensatory release decision.
`provider_capability_gate` gates runtime/provider claims on passed correctness and security
evidence, keeps performance values as measurements without invented thresholds, and marks
cross-provider comparisons indeterminate when either side is untested.
The Python and TypeScript SDKs expose the same evidence boundary with typed claim states, gate
outcomes, run witnesses, measurement counts, and differential drift; a cleared gate never implies
that runtime execution occurred or that unmeasured capabilities are safe.
`scale_family_split_verify` verifies imported benchmark tiers against lineage roots and refuses
family straddles; `stewardship_review_check` concludes evaluator reviews only when mandatory
dimensions, corpus support, and independence hold, keeping unreviewed dimensions explicit.
`quality_gate_run` preserves pass, fail-with-witness, and not-runnable data-quality outcomes; the
Python SDK exposes typed witnesses, not-runnable reasons, check-level outcomes, and the separate
failed-versus-obstructed verdict sets, while TypeScript preserves the serialized gate/check union
and report shape without turning an indeterminate run into a pass;
`ledger_ingest` appends bitemporal events while exposing quarantine, idempotency, causal release,
hash-chain, clock-anomaly, temporal-cut, and digest-only projection state; the Python and
TypeScript SDKs preserve those admission, release, and projection witnesses without implying
durable storage or a live clock.
`fabric_synthesize` evaluates typed agent-composition candidates against hard effects, privacy,
budget, assurance, and terminal-state constraints, then returns the rejection map and Pareto
frontier without inventing a weighted winner.
`interweave_workflow_catalogue` exposes the six reference workflows and derives their 54 owed
deliverables from the typed catalogue, keeping specification inventory separate from artefact
availability.
`epistemic_voi` prices explicit evidence actions and non-adaptive bundles while keeping gross risk
reduction, declared cost, net value, action changes, complementarity, and exhaustive limits visible;
the Python and TypeScript SDKs expose the same boundary with typed problem, belief, acquisition,
value, bundle, action-identity, and fail-closed refusal projections;
`epistemic_adaptive_acquisition` extends that boundary with an exact finite-horizon policy tree:
each outcome can stop or choose a different unused acquisition, while expected terminal risk,
expected scalarized cost, posterior branches, state caps, conditional-independence assumptions,
and fail-closed refusals remain visible; it plans only and never executes an acquisition or claims
causal, clinical, biological, or predictive truth. See
[`docs/EPISTEMIC_ADAPTIVE_ACQUISITION.md`](docs/EPISTEMIC_ADAPTIVE_ACQUISITION.md);
`epistemic_adaptive_execute` is the explicit next boundary: it requires a plan-scoped provider
grant, validates one provider outcome against the selected branch at a time, preserves partial and
refused prefixes, and replays through a receipt-only executor with no live fallback. The built-in
MCP adapter is simulation-only and labels its rows `simulated`; Python and TypeScript expose typed
receipt/provenance projections. See
[`docs/EPISTEMIC_ADAPTIVE_EXECUTION.md`](docs/EPISTEMIC_ADAPTIVE_EXECUTION.md);
`epistemic_adaptive_costed` exposes the same exact finite-horizon planner with component-wise
tokens/compute/latency/money/privacy/specimen/expert budgets and explicit scalar weights; Python
and TypeScript preserve the canonical seven-dimension request/result contract. See
[`docs/EPISTEMIC_COST_VECTORS.md`](docs/EPISTEMIC_COST_VECTORS.md);
the versioned `fiber-query/0.5` contract carries the same unperformed-acquisition semantics into
the FIBER compiler and returns a certificate-bound named policy tree with `execution:
"not_started"` and `authorization: "not_granted"`; the Python and TypeScript SDKs expose a
typed replay-safe projection of that boundary.
The interweave catalogue now has a typed workflow execution binding that carries workflow identity,
capabilities, effect prohibitions, plan digests, explicit grants, and receipt-only replay across
all six reference workflow identities without claiming generic release authority. The
`interweave_workflow_execute` MCP route and the Python/TypeScript facades expose deterministic
simulation, structured no-grant refusal, and same-binding receipt replay. See
[`docs/WORKFLOW_EXECUTION_BINDING.md`](docs/WORKFLOW_EXECUTION_BINDING.md).
Workflow receipts can also be converted into portable, digest-checked evidence with
`interweave_workflow_execution_evidence`, then imported, queried, and fetched without re-running
the workflow. Evidence retains caller-owned domain/subject labels and separates observed,
simulated, and replayed provenance; registry presence remains review evidence rather than release
authority.
`benchmark_trace_analyze` adds the deeper benchmark compiler's causal, episode, boundary, and
repetition analysis; the Python and TypeScript SDKs expose typed trace events, causal score
components, divergence/verdict variants, boundaries, episodes, repetitions, and fail-closed
refusals; and `pack_catalogue` exposes the agent and biological pack portfolio without
turning declarations into measured scores.
`pack_health_assess` runs the typed pack-health gate over observed calibration, trivial baselines,
contamination, oracle posture, and materialization, binding every finding to the pack digest and
refusing a numeric score for an unreportable revision.
`pack_catalogue` exposes the corresponding bounded declaration inventory with typed axes, oracle
ceilings, release sequencing, and duplicate-signature review candidates; it does not turn a
portfolio declaration into observed performance.
`foundation_contract_check` validates falsifiable-contract admissibility, safe refinement, claim
applicability, world-class counterfactual strength, reveal policy, and transition-plane consistency
as separate gates.
The Python and TypeScript SDKs expose those gates as typed subreports and keep a transport-success
response distinct from an admitted contract or an authorized biological claim.
`world_generate` creates deterministic synthetic world/query pairs from a bounded `WorldSpec`,
parses both through the typed runtime, and returns exact digests and structural validation;
`hub_submission_review` checks the public submission contract and can replay append-only moderation
with independent verification attestations; `telemetry_project` applies typed redaction with a
semantic-loss report and optionally evaluates observed-versus-asserted operational metrics. These
three surfaces are local contract workflows only: they do not publish to a network, authenticate
identities, persist a hub ledger, export OTLP, execute models, or make clinical claims.
`factory_lifecycle_simulate` adds deterministic lease, expiry, idempotency, compensation, quarantine,
and atomic-commit replay; `factory_authority_verify` audits the durable queue envelope and bounded
transition chain without dispatching work; `hub_disclosure_review`, `hub_card_render`, and `hub_leaderboard_render`
`artifact_registry_audit` indexes exact-content mission, evaluator, reconciliation, and domain
artifacts across the capability surface. It preserves verification posture, declared parent edges,
missing parents, and bounded lineage traversal while explicitly refusing to infer causal provenance,
scientific validity, clinical safety, publication authority, or external-effect completion from a
digest or registry presence.
Trusted boundaries also project mission reports, evaluator replays, verified evidence-bundle
imports, and digest-valid workflow reconciliations into this shared index automatically. Each
response carries an `artifact_registry` projection with the exact registry digest or an explicit
indexing failure; generic domain-tool outputs remain unindexed unless the caller registers them
deliberately.
`artifact_registry_audit` with `operation: "domain_evidence_lineage"` is the intake-specific read
model over that same index. It filters any of the 29 capability groups by exact content, request,
response, intake, source-plan, subject, source-tool, outcome, or domain identity; each returned row
keeps the recoverable request/response digests, direct declared parent states, source-plan
`plan_digest` versus indexed content-digest binding, and reverse direct child links. The MCP
operation, `GET /v1/domain-evidence/lineage`, `bioprism evidence domain-lineage`, and the sync/
async Python and TypeScript facades all preserve cursor bounds and the distinction between a
missing parent, a retained parent, and a digest that is merely declared. It is a structural
lineage view only: no digest, parent edge, child edge, or intake outcome becomes execution,
causal provenance, scientific, clinical, provider, release, or readiness authority.
`domain_decision_readiness_audit` is the next cross-domain policy gate. It accepts the caller's
same-subject canonical reports and explicit link roles, then evaluates required groups/domains,
support and qualification floors, contradiction/refusal policy, review posture, report linkage,
and optional lineage-parent requirements. Its `blocked`, `incomplete`, `review_required`, and
`ready_for_human_review` states are structural dispositions, not scientific, clinical, release,
execution, or truth claims; `readiness_claimed` remains false and execution remains `not_started`.
The MCP tool is available to every current domain group, and the generic REST dispatcher plus
sync/async Python and TypeScript clients preserve the same digest-bound audit and indexed artifact.
`domain_decision_readiness_query` and `GET /v1/domain-decision-readiness` provide a bounded,
digest-ordered retained read model with exact subject/state/policy filters, cursor pagination, and
opt-in audit bodies; `readiness query` exposes the same query against a local artifact checkpoint.
Workflow portfolios and reconciliations can carry a validated `readiness_audit` summary and opt
into `policy.require_readiness`. That gate remains separate from mission preflight and completion:
it records structural decision posture, never execution authorization or domain truth.
carry disclosure ratchets, fail-closed score publication, comparability conditions, and typed
unranked entries into agent-callable public-hub projections. `release_audit` composes required
registry, bundle, quality, conformance, research-CI, operations, and pack-health gates while
retaining repository impact and developer-platform diagnostics as advisory evidence. These
surfaces remain bounded and local: they do not create durable queues, identity providers, web UI,
CI execution, deployment, or network publication. The bundle layer now has deterministic offline
Ed25519 verification plus an explicit caller-supplied key-registry policy layer for roles,
delegation, rotation, revocation, producer binding, and validity. The registry is a bounded local
snapshot, not an external identity, transparency, timestamp, or release-authorization service.
The Python and TypeScript SDKs expose the factory result as an ordered, typed trace: successful
leases, recovery variants, staged-output invisibility, committed-result snapshots, quarantined and
dead-lettered jobs, and fail-closed action refusals remain independently inspectable across sync,
async, MCP, and HTTP facades.
`storage_lifecycle_simulate` adds the matching typed storage boundary: caller-epoch hot/warm/cold
plans, pinned-object protection, skipped-tier witnesses, explicit dry-run/application accounting,
reserve-aware quota charges, releases, non-copyable delegation/absorption, and raw reconstructible
class attribution remain inspectable without moving bytes or creating a scheduler.
`registry_lifecycle_simulate` carries the same evidence discipline into benchmark publication:
attested pack preflight, serialized-index integrity, publish/promote/reassess/supersede/withdraw,
lookup/history/revision/verification actions, append-only log state, and continuation indexes are
typed while invalid packs and failed operations remain independent fail-closed rows.
`cache_invalidation_simulate` adds the corresponding reproducibility boundary: component-complete
key schemas, cross-build policy, declared versus opaque dependency graphs, complete versus partial
invalidation, explicit dry-run/application state, reasoned pre/post misses, unproven entries, and
attributed reproofs are typed without serving an entry whose currentness cannot be proved.
`hub_disclosure_review` adds the public-hub disclosure boundary: immutable digest-keyed ratchets,
contamination witnesses, split-integrity verdict folding, headline caveats, and score-withholding
refusals remain distinct and replayable. A clean split verdict does not become a secrecy claim,
and a visible benchmark cannot become a bare headline number without explicit acknowledgement.
`hub_card_render` adds the renderer boundary: cards carry moderation-derived publication state,
access, verification, provenance, limitations, non-claims, and a tagged published/withheld score;
failed disclosure or publication gates preserve the card while keeping its numeric score null.
`hub_leaderboard_render` and `bioatlas_publication_audit` complete the composed public surface:
ranked and unranked entries retain typed reasons and scoped nonclaims, while atlas coverage,
evidence-conditioned claims, card score attachment, and explicit release targets remain separate
gates with fail-closed blockers.
`hub_submission_review` exposes the preceding acceptance and moderation state machine with
append-only events, reasons, verification attestations, and withdrawal tombstones; refusal stages
remain explicit, and the endpoint still does not authenticate or publish externally.
`runtime_execution_simulate` runs bounded serialized effect programs against the deterministic
in-process world, returns policy and budget evidence, proves complete replay, and can open a forked
suffix with observable state and divergence comparison. The Python and TypeScript SDKs expose typed
runtime-effect, tape, and simulation projections plus the full bioethics review family, preserving
authorization/refusal, simulated provenance, partial replay, physical referral, institutional
review, dual-use assessment, validation maturity, and representation gaps across MCP and HTTP
without claiming host execution or institutional clearance. `megafactory_twin_audit` qualifies
mechanistic counterfactuals against alternative models while withholding oracle status on sign
instability; `megafactory_placement_audit` checks worker capability, attestation, oracle
independence, locality transfer, fencing, and duplicate-effect classes. These remain local
contract workflows: they do not provide containers, restoration of external state, real workers,
durable fencing, biological calibration, or distributed scheduling.
`trace_analyze` ingests native JSONL trajectories with explicit import loss, while
`trace_otel_ingest` maps bounded OTLP JSON spans into the same Event IR with source preservation,
parent resolution, and semantic-loss accounting; the Python and TypeScript SDKs preserve the
normalized event preview, mapping counts, loss categories, and compilation-readiness boundary.
The trajectory tools validate causal ordering,
rank decision-bearing review candidates, and compare lossless passing traces for first divergence.
They return review-gated `CellProposal` previews; they do not replay tools, minimize state, export
OTLP, or publish a Decision Cell.
`lineage_audit` checks specimen ancestry, mass, time, material, artifacts, and identity evidence;
`preanalytic_apply` runs the real pre-measurement mutation postconditions, family null control,
response availability, and optional caller-threshold detectability.
`contradiction_review` poses multimodal readings, filters admissible explanations, detects answer
cues, ranks discriminating evidence, and keeps resolved, not-yet-examined, and unresolvable states
distinct without choosing a correct modality.
`lab_plan` orders declared evidence; `obligation_gate_check` enforces high-regret gates, while
`atlas_report` preserves capability coverage debt,
failure inconsistencies, measured-versus-unmeasured holes, and optional gated composites; the
Python and TypeScript SDKs keep hole omission, evidence depth, family darkness, and composite
refusals visible without rendering unmeasured capabilities as zero.
`atlas_surface_audit` adds the atlasx publication surface: denominator-carrying CapabilityGrid
coverage, named debt discharge, withheld failure browsing, explicit rate denominators, and
declaration soundness. The SDKs preserve the same layers and fail-closed policy stages; see
[`docs/ATLAS_SURFACE_AUDIT.md`](docs/ATLAS_SURFACE_AUDIT.md).
`ops_acceptance` reports typed operational acceptance findings without turning unverifiable criteria
into passes. `ops_capacity` projects qualified work and demand, refusing unbounded work or silent
degradation. `bundle_verify` recomputes carried result-bundle content and keeps referenced,
unrecomputed, and provenance-limited entries explicit; it also accepts an explicit Ed25519
PubliclyAttestedBundle plus verification key, checking purpose, key identity, signed instant, and
caller-declared key validity without claiming registry-backed identity or external closure fetching.
The complete wire format, threat-model boundary, and SDK/MCP mapping are documented in
[`docs/BUNDLE_SIGNATURES.md`](docs/BUNDLE_SIGNATURES.md).
`oracle_reference_panel` preserves independent reader calls, minority evidence, adjudication
blinding, and unresolved splits. `oracle_missingness` checks missingness informativeness,
complete-case admissibility, and small-cell egress under an explicit caller policy.
`adaptive_panel` audits clustered evaluation evidence, selects the next bounded candidate batch,
and refuses reportable estimates below coverage or stopping floors; the Python and TypeScript SDKs
preserve audit totals, coverage shortfalls, withheld estimates, clustered-versus-naive intervals,
selection records, and comparison refusals. `posterior_gate` keeps
capability-level posterior vectors separate from rationale-bearing release scalars, coverage
floors, vetoes, and sensitivity; the Python and TypeScript SDKs preserve these layers and typed
fail-closed refusals. `oracle_combine` combines tiered judgements without majority
voting, retaining underdetermination, suppressed overrides, inadmissible evidence, and
disagreement witnesses; its Python and TypeScript SDK projections now preserve nested oracle
identities, admissibility, settlement routes, and resolution state. See
[`docs/ORACLE_COMBINE.md`](docs/ORACLE_COMBINE.md).
`bioeval_reference_audit` validates reference mass normalization and reports distributed truth,
modal confidence, entropy, dispersion attribution, unresolved scope, and not-evaluable scope
without treating an omitted state as zero or collapsing the reference to a label; the SDKs preserve
the typed reference, resolution, and dispersion layers. See
[`docs/BIOEVAL_REFERENCE_AUDIT.md`](docs/BIOEVAL_REFERENCE_AUDIT.md).
`evaluation_worldline_audit` separates future leakage from dangling context references,
`evaluation_reproduction_check` certifies rerun outputs without promoting reproducibility to
biological validity, and `evaluation_trajectory_check` evaluates declared path properties with
bounded immediate/downstream suffixes. The SDKs preserve typed accessibility-clock leak witnesses,
dangling-reference pairs, ordered reproduction verdicts, reconciled divergence/missing counts,
fail-closed validity refusals, step/property/outcome ledgers, recovery transitions, and bounded
suffix completeness; see
[`docs/EVALUATION_WORLDLINE_AUDIT.md`](docs/EVALUATION_WORLDLINE_AUDIT.md) and
[`docs/EVALUATION_REPRODUCTION_CHECK.md`](docs/EVALUATION_REPRODUCTION_CHECK.md) and
[`docs/EVALUATION_TRAJECTORY_CHECK.md`](docs/EVALUATION_TRAJECTORY_CHECK.md).
`runtime_effect_check` authorizes effects under an explicit
deny-by-default policy without executing them, while `runtime_tape_verify` verifies hash-chained
world tapes, typed checkpoint restoration and artifact ledgers, simulated provenance, and first
divergence. See [`docs/RUNTIME_TAPE_VERIFY.md`](docs/RUNTIME_TAPE_VERIFY.md).
`runtime_execution_simulate` runs bounded programs in the deterministic in-process world and
preserves typed recording/replay, policy-journal, budget, and fork evidence; see
[`docs/RUNTIME_EXECUTION_SIMULATE.md`](docs/RUNTIME_EXECUTION_SIMULATE.md).
`onco_boundary_check` keeps research output separate from individualized clinical use, preserving
partial-release counts, escalation routing, and fail-closed identifier refusals; see
[`docs/ONCO_BOUNDARY_CHECK.md`](docs/ONCO_BOUNDARY_CHECK.md). `onco_response_assess` keeps
post-treatment progression, threshold sensitivity, and non-identifiable change states explicit.
Its versioned projection separates call kind, unconfirmed reading, treatment-window metadata,
criterion divergence, sensitivity flips, and hypothesis identifiability; see
[`docs/ONCO_RESPONSE_ASSESS.md`](docs/ONCO_RESPONSE_ASSESS.md).
`onco_worldline_view` keeps acquisition, recording, release, and agent-visibility clocks distinct,
reports indexed biological and record orders, and exposes a versioned visibility partition at a
caller-supplied cutoff. Its typed SDK projection rejects forged clock copies, order indices, and
leakage partitions; see [`docs/ONCO_WORLDLINE_VIEW.md`](docs/ONCO_WORLDLINE_VIEW.md).
`onco_classification_check` runs the integrated molecular criteria table without treating
uncollected assays as negative, and its typed projection preserves all five resolution states,
obligations, satisfied evidence, and panel-state accounting; see [`docs/ONCO_CLASSIFICATION_CHECK.md`](docs/ONCO_CLASSIFICATION_CHECK.md).
`oncoworlds_identity_join` checks participant,
lesion, specimen, disease-epoch, relation, and permissible-use boundaries and returns a versioned
decision record with typed join refusals, evidence counts, and bridge warrants rather than silently
discarding cross-modal mismatches; see [`docs/ONCOWORLDS_IDENTITY_JOIN.md`](docs/ONCOWORLDS_IDENTITY_JOIN.md).
`oncoworlds_model_transport` checks whether a model-system result can carry a declared, lossy
research claim toward patients and returns a versioned projection of model identity, passage-specific
fidelity, establishment selection, technical/biological replication, transport assumptions, and
typed fail-closed refusal kinds; see [`docs/ONCOWORLDS_MODEL_TRANSPORT.md`](docs/ONCOWORLDS_MODEL_TRANSPORT.md).
`oncoworlds_methylation_classify` preserves QC abstention,
threshold, calibration, and tumour-content caveats, while `oncoworlds_methylation_compare` keeps
classifier-version disagreement version-conditioned. Both expose versioned outcome/divergence,
threshold, score-coverage, and classifier-change projections; see
[`docs/ONCOWORLDS_METHYLATION.md`](docs/ONCOWORLDS_METHYLATION.md). `oncoworlds_radiogenomic_check` checks
participant-safe splits, training-only feature fitting, specimen-versus-tumour target scope,
mechanism strata, and declared transport assumptions before admitting a cross-modal claim. Its
versioned projection retains the blocked sentence, design summary, required/declared transport
assumptions, and refusal taxonomy even when support is denied; see
[`docs/ONCOWORLDS_RADIOGENOMIC_CHECK.md`](docs/ONCOWORLDS_RADIOGENOMIC_CHECK.md).
`onco_outcome_analyze` requires an explicit estimand before interpreting one subject’s follow-up,
keeps loss to follow-up and competing death as censoring distinctions, and reports delayed-entry
bias. Its versioned typed projection binds endpoint strategy, event/censoring tags, delayed-entry
exposure, and complete versus informative bias flags; see [`docs/ONCO_OUTCOME_ANALYZE.md`](docs/ONCO_OUTCOME_ANALYZE.md).
`oncoworlds_clonal_history_check` audits candidate histories against cellular fractions and keeps
multiple compatible histories as typed ambiguity rather than selecting one; its versioned projection
retains per-candidate refusal kinds and candidate accounting (see [`docs/ONCOWORLDS_CLONAL_HISTORY_CHECK.md`](docs/ONCOWORLDS_CLONAL_HISTORY_CHECK.md)).
`oncoworlds_clonal_evidence_check` extends that boundary to specimen promotion, recurrence-
resistance explanation sets, assay sensitivity, declared copy-number conversion, and treatment
attribution. It preserves sampled-region bounds and temporal-causation refusal rather than
inventing a single phylogeny or treatment mechanism; see
[`docs/ONCOWORLDS_CLONAL_EVIDENCE.md`](docs/ONCOWORLDS_CLONAL_EVIDENCE.md).
`oncoworlds_entity_world_check` composes provenance-selection, alteration-mechanism, rare-class
benchmark, lesion-clustering, and competing-event safeguards. Requested sections retain separate
admissibility and refusal evidence, while the top-level report reconciles only the requested
sections; see [`docs/ONCOWORLDS_ENTITY_WORLDS.md`](docs/ONCOWORLDS_ENTITY_WORLDS.md).
`literature_bind_check` exposes the same fail-closed literature boundary through MCP and both SDKs:
binding a source to a scope is not permission to use it as a measurement, and a successful bound
claim may be citable only as `published_claim_support`; see
[`docs/LITERATURE_BIND_CHECK.md`](docs/LITERATURE_BIND_CHECK.md).
The modality support boundary is documented in
[`docs/MODALITY_SUPPORT_CHECK.md`](docs/MODALITY_SUPPORT_CHECK.md).
The modality transport boundary is documented alongside the SDK contracts in
`docs/MODALITY_TRANSPORT_CHECK.md`.
The Python and TypeScript SDKs expose the OncoWorlds workflows as typed MCP and HTTP projections,
retaining domain refusals, QC abstention, version-conditioned disagreement, transport assumptions,
clonal ambiguity, era/site comparability, resource absence, descriptor boundaries, and subgroup
intervals without claiming clinical classification or patient-level truth. See
[`docs/ONCOWORLDS_SHIFT_EQUITY.md`](docs/ONCOWORLDS_SHIFT_EQUITY.md).
`stress_profile` and `stress_report` sweep biological stress families and report breaking points,
generator defects, confounding, effective sample size, and unresolved measurements without reducing
robustness to a single score.
The Python and TypeScript SDKs expose both stress workflows with bounded serialized request types
and typed projections for the intensity ladder, identifiability, required/probed relations, and
guarded worst-family comparison; defective or non-identifiable families remain excluded from that
comparison rather than being ranked as if they were evidence.
`developer_platform_status` verifies the cookbook, walkthrough standing, diagnostics, exit-code
audit and declared change-impact surfaces while keeping foreign SDK/CI artifacts explicit.
The Python and TypeScript SDKs expose that projection with reconciled walkthrough standings,
module classification counts, cookbook omission accounting, diagnostic and exit-code rows,
declared contract surfaces, foreign-artifact posture, and optional full-detail evidence; a clean
local check never implies that foreign SDK, CI, gRPC, or live-debugger surfaces were executed.
`sdk_registry_check` validates serialized plugin manifests, computes whole/core digests, reports
attributed trust evidence, and attempts deterministic registry admission under an explicit host
policy; invalid declarations and capability conflicts return no partial resolution.
The Python and TypeScript SDKs expose both refusal stages and the successful digest/trust/
registration projection without implying dynamic loading, signatures, isolation, or plugin
execution.
`developer_delivery_audit` composes those local platform and repository contracts with optional
impact, SDK admission, conformance, provider, governance-document, and release evidence. It
requires explicit readiness targets, keeps missing foreign SDK/CI artifacts visible, and never
turns a partial green result or an unguarded walkthrough claim into a release.
`workspace_capabilities` reports every
major biological, evaluation, mutation, safety, orchestration, operations and documentation
surface with its actual transport, while `repository_catalog`/`repository_bundle` provide bounded,
route-aware access to the documentation graph. Documentation bundles preserve protected closure,
route defects, traversal completeness and omission influence; requesting rendered markdown is
explicit and fails rather than truncating over a caller-supplied limit. `repository_impact` computes
conservative incoming-dependent closure and typed propagation stops for a changed module, with
affected task routes retained as explicit invalidation evidence rather than a semantic-diff claim.
Cross-domain `capability_route` responses additionally report per-need candidate domains and an
aggregate `route_coverage` ledger, allowing the agent to see whether its proposed route spans the
intended domains before constructing an explicit mission. The Python SDK exposes the same evidence
through `CapabilityRouteReport.from_wire(...)` and sync/async `capability_route_report(...)` helpers;
they reconcile the per-need counts, candidate ledgers, and bounded recommendation overflow while
preserving the raw route for audit. `capability_route_review` then provides a cross-domain handoff
checkpoint: it checks caller-selected candidates and dependency waves, emits blocked or ready
diagnostics, and keeps mission preflight and execution explicitly separate. Its optional
`validate_schemas` mode reports authoritative selected-tool schema digests and issue paths without
turning schema conformance into domain readiness. Every review also carries a deterministic,
content-addressed `review_id` derived from the route provenance, caller selections, and validation
mode, making the same handoff correlate cleanly across transports and event records.
Modern route responses also attach a separate `evidence_digest` over the selected candidate-group
artifact and workflow-reconciliation postures, registry generations, and bounded counts. Each need
retains its `candidate_group_evidence` rows so discovery can show missing or observed retained
evidence before review; this is an advisory point-in-time observation, not an execution, readiness,
authorization, scientific-validity, or release claim, and it is intentionally not folded into the
catalogue-bound `route_id`.
`capability_route_review` now validates and carries that digest/scope through `evidence_binding`,
the review identity, and the generated mission draft. Its explicit `carried_forward_not_recomputed`
posture prevents retained discovery observations from being silently dropped or promoted into
execution/readiness claims; legacy routes report `present: false`.
`capability_route_plan` closes the public handoff seam by composing a complete route review with
the authoritative mission preflight boundary. It accepts only caller-selected candidates and
explicit arguments, carries optional claim/evaluator/workflow bindings, returns the generated
mission and `plan_digest`, and fails closed with `dispatch: "not_started"` when route review or
preflight is blocked. It never dispatches a nested tool or turns routing evidence into
authorization; callers must inspect the preflight before invoking `agent_mission`.
`capability_route_plan_verify` provides the matching non-executing replay boundary: it reruns mission
preflight, optionally recomputes route review from caller-supplied inputs, and exposes digest and
identity mismatches without treating missing replay inputs as proof of current membership.
The reviewed handoff can now be supplied directly as `route_review` on `agent_mission` or
`/v1/missions/preflight`. The mission boundary requires the ready review to match the submitted
goal and exact serialized steps, binds its review/route/catalogue identities into the plan digest,
and retains compact evidence posture without granting permission or readiness. A changed draft,
stale finding, or tampered evidence binding is refused before dispatch; legacy no-evidence reviews
remain structurally supported with an explicit absent binding.
The same reviewed handoff may cross the workflow-template boundary through
`domain_workflow_instantiate`. After normalized steps are constructed, the generated mission
retains the exact route review; the durable queue exposes only its `spec_digest` and compact
provenance, mission checkpoints preserve that projection across restart, evaluator replay marks it
`absent`, `valid`, or `invalid`, and workflow reconciliation compares it against the instantiated
workflow. These joins add integrity evidence without turning route review into authorization,
execution, or a domain conclusion.
`domain_workflow_catalogue` closes the next gap between discovery and planning: it materializes
one deterministic, digest-bound workflow template for each of the 29 capability groups, including
available versus missing tool definitions, per-tool schema/evidence contracts, and advisory lexical
stages. Every template also carries a domain contract that makes scope review, tool availability,
argument preflight, execution policy, evidence retention, refusal/omission accounting, and
completion review explicit without inventing domain semantics. `domain_workflow_instantiate`
requires an explicit workflow, mission, goal, and step list; it refuses tools outside that group's
declared scope or absent from authoritative `tools/list`, rejects out-of-scope policy allow-lists,
validates the mission DAG, derives a least-scope allow-list for requested execution, emits a
step-level evidence plan, and attaches authoritative no-dispatch MCP schema preflight. MCP, REST,
CLI, Python, and TypeScript all expose the same kernel. A valid workflow remains a plan, not
permission, scientific evidence, clinical guidance, deployment readiness, or execution.
`domain_workflow_scaffold` is the bounded planning shortcut across the same 29 groups: it selects
one live available tool per advisory stage by default, or accepts an explicit tool list and
per-tool argument map, then materializes a deterministic `domain_workflow_instantiate` payload.
Each catalogue tool contract carries bounded argument-schema facts, and the scaffold runs the
authoritative MCP preflight before returning. Missing required arguments are an explicit blocked
preflight result; they are never replaced with benign defaults. The response always preserves
`execution: "not_started"`, `dispatch: "not_started"`, and `readiness_claimed: false`, so this
convenience surface cannot silently become an executor or readiness credential. MCP, REST, Python,
and TypeScript expose the same scaffold contract.
The instantiated mission also carries a bounded `workflow_binding` containing the workflow,
catalogue, domain-contract, and evidence-plan digests plus the contract snapshots needed to
reconstruct that exact scope after dispatch. The binding is validated as structure and provenance;
it is not an authorization token, readiness claim, or domain conclusion.
`domain_workflow_portfolio` composes up to 64 explicit workflow instantiations for multi-domain
planning. It runs each group independently, adds authoritative no-dispatch preflight, retains
per-item refusal diagnostics, and makes complete-catalogue versus partial scope explicit. A
portfolio can be inspected as a whole without hiding the domain-specific arguments that still
need caller completion; `portfolio_ready` never grants execution or domain validity.
The CLI exposes the same boundary as `bioprism workflow portfolio --requests <path>`, accepting
either a JSON request array or an object with `requests` and optional `policy`; `--allow-partial`
and `--require-complete-catalogue` make the two most important scope decisions visible in shell
automation. A blocked portfolio returns its full per-item diagnostics in `--json` mode and uses
the assertion-failed verdict when it is not ready, while preserving `dispatch` and `execution` as
`not_started`.
`domain_workflow_portfolio_verify` is the retained multi-domain audit continuation: it recomputes
the portfolio digest and coverage, verifies every retained item independently, optionally replays
an index-aligned array of original requests, and retains digest, identity, replay, and mission
preflight mismatches per item. The CLI exposes this as `workflow portfolio-verify --portfolio
<path> [--replay-requests <path>] [--require-replay]`; REST, MCP, Python, and TypeScript expose
the same bounded contract. Verification remains review evidence only: it never dispatches,
retries, resumes, grants readiness, or establishes domain validity.
`domain_workflow_verify` is the retained-handoff gate before re-review: it validates the current
catalogue and contract identities, checks the workflow binding and mission projection, reruns
authoritative mission preflight, and optionally replays the original bounded instantiation request.
It reports exact mismatch codes with compact digest witnesses, distinguishes full replay from
`verified_without_replay`, and remains strictly non-executing with `dispatch` and `execution` both
`not_started`.
`developer_workbench_verify` provides the analogous authoring/notebook handoff audit: it recomputes
the current session audit, replays a retained dashboard query, and optionally replays the original
CI request while comparing report and audit digests. MCP, `POST /v1/developer-workbench/verify`, the
CLI (`bioprism workbench verify`), Python, and TypeScript expose the same mismatch witnesses and
policy controls. It never executes cells, writes YAML, contacts GitHub, runs CI, or grants release
or domain authority.
The retained workbench registry makes that audit durable without pretending to be a workbench
database: `developer_workbench_import`, `developer_workbench_query`, and `developer_workbench_get`
accept only structurally valid, digest-normalized reports, provide deterministic digest-ordered
filters/cursors, and return full reports only when explicitly requested. The same contract is
available at `POST/GET /v1/developer-workbench/reports` and
`GET /v1/developer-workbench/reports/{workbench_report_digest}`, with atomic restart-safe
checkpointing via `--workbench-state` and explicit persistence status/flush routes. The CLI
provides `workbench import`, `workbench query`, and `workbench get`; Python and TypeScript expose
typed MCP and REST facades. The registry is bounded to 512 reports and a 32 MiB snapshot, verifies
every report and snapshot digest on import/restore, and never executes, re-evaluates, or authorizes
the retained workbench output.
`domain_workflow_reconcile` is the corresponding post-execution audit: it binds a retained
`agent_mission` report or verified evidence bundle back to the instantiation, checks plan/result/
trace consistency, preserves refusals and omissions, and makes structural completion readiness
explicit without retrying or dispatching tools. Its `complete` status is evidence posture only and
still requires review before any domain claim.
The reconciliation registry continuation makes that audit durable and searchable: import a
digest-valid report through `POST /v1/domain-workflows/reconciliations`, query compact
mission/workflow/plan/status index rows with bounded cursors, and fetch one record by its
`reconciliation_digest`. `--reconciliation-state <file>` enables an atomic restart-safe
checkpoint; startup verifies the snapshot and every report digest, while the explicit persistence
status/flush routes expose the checkpoint posture. MCP exposes the same import/query/get tools,
the CLI provides `workflow reconciliation-import` and `workflow reconciliation-query`, and the
Python/TypeScript SDKs expose typed REST and MCP helpers. Registry presence is an audit lookup only:
it never resumes, retries, or re-evaluates a mission and never authenticates provenance or a
scientific, clinical, safety, or release claim.
When an executable mission includes a valid `workflow_binding`, the authoritative MCP executor
automatically runs this structural reconciliation after terminal execution and imports the full
digest-valid record into the shared REST/MCP registry. The mission response exposes only a compact
`workflow_reconciliation` link, completion/evidence/integrity posture, and idempotent import result;
the full record remains available through the reconciliation lookup route. A reconciliation failure
is retained as an explicit `fail_closed` response and never upgrades mission success into readiness.
API synchronous calls checkpoint this shared registry before returning when reconciliation
persistence is configured; asynchronous mission workers checkpoint it before publishing terminal
job state. This makes the same post-dispatch audit visible to operations gates and restart recovery
without making a gate pass automatic.
The same artifact index is checkpointed by synchronous REST/MCP dispatch and asynchronous mission
workers when `--artifact-state` is configured. Automatic indexing is an audit projection only: it
does not add provenance, scientific validity, authorization, or release readiness.
`mission_evaluator_discover` complements tool routing with a digest-bound catalogue of explicit
evaluator candidates for every workspace capability group. It filters by intent, group, domain,
mission level, or adapter ID and returns purpose, candidate evidence tools, and RFC 6901 pointer
examples. Every row is marked `candidate_only`: discovery never runs an evaluator, validates
domain semantics, or adjudicates a claim. The Python and TypeScript SDKs expose the same typed
projection so callers can choose an adapter before adding an explicit `evaluator_bindings` row to
`agent_mission`.
`mission_evaluator_review` is the non-executing checkpoint after discovery: it binds caller-selected
claim IDs to digest-fresh candidate adapters, validates candidate membership, domain support, unique
selection IDs, per-claim limits, and RFC 6901 output pointers, then returns either a ready binding
scaffold or bounded correction findings. A ready review still requires `agent_mission` validation;
the checkpoint never executes an evaluator or a domain tool.
`mission_evaluator_replay` is the non-executing audit after mission completion: it rechecks retained
adapter/domain rows, output-digest shape, outcome counts, disagreement posture, refusal/omission
states, and structural coverage against all 29 evaluator groups. It can emit four non-semantic fixture
variants for every adapter, while preserving `execution: "not_started"`; replay is an audit and
coverage instrument, not evaluator execution or a scientific/clinical/release verdict.
The durable HTTP route `/v1/missions/{mission_id}/evaluator-replay` adds bounded restart-aware
querying: `retention.mode: "full"` exposes the retained replay, while `"summary_only"` exposes
digest, count, coverage, finding, and omission evidence after a large report body is trimmed.
Python and TypeScript clients preserve this distinction in typed query helpers; neither mode
reconstructs raw output or dispatches an evaluator.
The adjacent `/evaluator-replay/compare` route detects catalogue-digest drift and checks whether
referenced adapters remain bound in the current catalogue. It deliberately reports the boundary
between digest-level comparison and exact historical row diffs. Reviews now retain a bounded,
content-addressed snapshot of all 29 adapter rows, so valid snapshots produce exact added/removed/
changed/unchanged IDs and changed-field lists; legacy digest-only checkpoints remain explicit about
their row-diff limitation. The durable `/evidence-bundle` route then exports mission status,
retention and omission proofs, optional raw result/trace, replay, catalogue drift, execution
provenance, navigable links, and a deterministic bundle digest in one bounded artifact.
`POST /v1/evidence-bundles/verify` and the MCP `mission_evidence_bundle_verify` tool recompute that
artifact's canonical and retained-result digests without executing any domain or evaluator tool.
Both routes remain structural and non-executing, and the Python/TypeScript SDKs expose the same
comparison, export, and verification contracts.
The registry continuation adds `POST /v1/evidence-bundles` for independently verified, idempotent
import; digest-ordered mission/domain queries; content-hash lookup; and an atomic restart-safe
checkpoint enabled with `--evidence-state <file>`. MCP exposes the same import/query/get kernel and
the CLI provides `evidence import` and `evidence query`. Restored bundles are reverified but never
resume execution or become provenance, scientific, clinical, or release claims.
`capability_dashboard` provides the bounded operator view beneath those routes: it binds the live
catalogue to authoritative MCP schemas, reports callable/partial/declared-only groups, keeps
crate/CLI/Python/MCP surface counts separate, and labels missing transports without pretending a
declared surface has been executed. Its `dashboard_digest`, filters, and truncation warnings make
the inventory reproducible before a caller selects tools for a mission; see
[`docs/CAPABILITY_DASHBOARD.md`](docs/CAPABILITY_DASHBOARD.md).
Mission/delegated-check handoffs are similarly available through the digest-bound
[`docs/EXECUTION_PROVENANCE.md`](docs/EXECUTION_PROVENANCE.md) projection.
`ci_execution_evidence_audit` closes the next authoring boundary: it regenerates the canonical
workbench CI plan, binds caller-supplied run evidence to its digest and exact check set, requires
per-check result digests, and keeps provider/caller provenance separate from structural verification.
Complete passing evidence can produce a bounded `ci_evidence_ready` handoff signal, never a claim
that GitHub was contacted, logs were fetched, a signature was verified, or deployment/scientific
validity was established.
`ci_provider_normalize` accepts a bounded GitHub Actions-shaped, GitLab CI, or generic provider payload and
projects it into the exact `CiRunEvidence` envelope consumed by that audit. Missing provider result
digests are derived from the supplied check object and labeled, while unknown and non-passing states
remain visible; normalization never contacts a provider, verifies signatures, fetches logs, or turns
caller-supplied data into authenticated execution truth.
For GitHub consumers, the repository also provides the dependency-free composite action
[`github-actions-evidence`](.github/actions/github-actions-evidence/action.yml). It supports both a
manual bounded checks file and an authenticated discovery mode that retrieves one run and its jobs
through the GitHub API. With `collect-evidence: true`, discovery also retrieves at most 128 artifact
metadata rows and derives bounded job-log locators from the job response; neither locator is
followed by default. An explicit `download-evidence: true` switch follows those HTTPS locators (or
manual artifact/log URIs) under 16 MiB per response and 256 MiB per collection, then replaces the
row digest with SHA-256 over the locally retrieved response bytes. Rows retain an explicit digest
scope (`provider_metadata`, `caller_declared`, or `local_response_bytes`), and optional attestation
`subject_digest` values are checked against the named row or run digest. Redirects remain HTTPS-only
and never receive the GitHub token. Archives are not extracted, logs are not interpreted, and
attestations are not signature-verified. Both modes produce the same canonical provider payload and
digest, while collection mode emits a separate envelope/digest, row counts, and stable download
mode/count/byte outputs. Oversized or partial job, artifact, or locator lists are refused, and the
token is never copied into any output. When a caller also supplies an explicit `ci` plan and
`evidence-output`, the action emits the exact `CiProviderEvidenceRequest` accepted by the Rust
provider-evidence audit/registry. This remains an ingestion handoff: metadata or local-byte digests
are not authenticated provider truth, checks are not executed, and no release is approved; see
[`docs/CI_EVIDENCE.md`](docs/CI_EVIDENCE.md).
`ci_provider_evidence_audit` extends the same handoff with bounded artifact, log, and attestation rows:
it validates unique ids, content-digest syntax, provider/run/check bindings, and attestation subjects,
preserves the original rows, and emits separate deterministic record digests. Its `conformance_ready`
signal is structural only; the route does not fetch remote bytes, execute checks, authenticate providers,
or cryptographically verify attestation statements.
The retained provider-evidence registry makes this handoff durable and joinable: imports re-run the
canonical audit, retain failed and unknown provider runs as explicit evidence, and expose deterministic
provider/run/plan queries plus exact digest lookup through MCP, REST, CLI, Python, and TypeScript. The
response carries separate artifact/log/attestation counts and record-family digests, while preserving
the boundary that provider locators are not fetched bytes and supplied digests are not verified
signatures. `--ci-provider-evidence-state` enables atomic restart-safe persistence with 512-record,
32 MiB snapshot, and 256-row query bounds; snapshot and per-record digests are checked on restore.
Import summaries and compact query rows also retain local-byte hash and attestation subject-digest
binding counts, and queries can require minimum thresholds for those counts without loading full
audits. This makes provenance posture queryable while keeping it distinct from provider authentication.
The registry remains an audit index: it never contacts GitHub/GitLab, executes CI, or grants release
authority.
`developer_delivery_audit` can compose that normalization directly through an explicit `ci_provider`
argument; it returns both the normalized provider projection and the downstream `ci_evidence` audit,
while rejecting simultaneous `ci_provider` and canonical `ci_evidence` inputs.
The deeper `ci_provider_evidence` argument composes artifact, log, and attestation conformance into
an independent delivery target; `developer_delivery_receipt` carries its complete projection digest,
and `developer_delivery_receipt_verify` detects tampering in that retained evidence row. The three
provider evidence paths remain mutually exclusive and structural-only.
When a delivery decision needs this signal, `developer_delivery_audit` accepts the exact
`ci_evidence` payload and exposes a separate `ci_execution_evidence` target; missing evidence blocks
that target without changing the semantics of other delivery targets.
The same delivery audit accepts an optional `execution_provenance` payload and exposes an independent
`execution_provenance` target, so callers can require mission-trace handoff, CI evidence, or both
without conflating structural evidence with execution authority.
`execution_provenance_audit` closes the adjacent mission handoff: it reconciles the returned plan,
terminal results, deterministic trace, and delegated check digests into one structural artifact.
It flags missing, duplicated, or identity-mismatched evidence, but never replays the mission or
upgrades caller/provider evidence into execution authority.
`developer_delivery_receipt` turns the resulting delivery audit into a deterministic,
content-addressed structural handoff with canonical target/evidence rows and joinable digests; it
still does not execute checks, contact providers, or approve a release.
`developer_delivery_receipt_verify` recomputes that handoff against a completed delivery audit and
surfaces tampering by dimension, so downstream consumers can verify record consistency without
mistaking it for provider authentication or release authority.
Typed discovery projections now preserve the complete matched group context—domains, Rust crates,
CLI entrypoints, Python artifacts, ranked fields, matched tools, catalog digest, and optional
authoritative tool schemas—so cross-domain routing can inspect coverage without falling back to
unvalidated nested JSON.
The HTTP boundary exposes that evidence through exact `review_id` filtering on event pages and a
bounded `/v1/route-reviews/{review_id}/evidence` lookup; the Python and TypeScript SDKs provide
typed helpers while preserving retention gaps and the explicit “not found in retained window”
meaning of an empty result. Delivery receipts have the parallel
`/v1/delivery-receipts/{receipt_id}/events` join and `receipt_id` event filter; oversized tool
responses retain only a bounded receipt projection, never an unverified release claim.

## Evaluating a context policy

```bash
./target/release/bioprism prism fork --world w.json --query q.json --bundle-out bundle.json
```

Freezes a Decision Cell from the full-context verdict, then runs every architecture from that
identical state — so a difference is attributable to the context policy and nothing else. On the
discriminating world:

| Architecture | Facts | Verdict | Closure | Cell |
|---|---:|---|---:|:-:|
| fiber | 11 | invalid | 100% | pass |
| full-context | 762 | invalid | 100% | pass |
| graph-5-hop | 750 | valid | 0% | **fail** |
| lexical-top-11 | 11 | invalid | 91% | **fail** |

Exit 1 when any architecture fails, so it gates CI. Acceptance is set-valued (03.07) and names its
failure mode: `graph-5-hop` fails on verdict, `lexical-top-11` on closure.

```bash
./target/release/bioprism prism minimize --world w.json
```

Reduces the world to a 1-minimal set preserving the oracle signature, then re-verifies it. On the
reference world: **761 facts → 6**, in 762 oracle evaluations.

That 6 is worth reading carefully against FIBER's 11. Only six facts are *causally* required for
the verdict; the other five are protected-closure facts that participate in no witness. FIBER is
deliberately **not** minimal — 43.13 makes identity, policy and negative-evidence closure mandatory
whether or not it moves this particular decision. Minimization measures what the verdict rests on;
closure decides what must be present regardless.

## Composing agents

`bioprism-weave` is the microkernel of §23 — a deliberately small trusted computing base that
enforces what cannot be delegated to untrusted participants and refuses to do anything else. Per
23.49 it "should not decide scientific truth, write patches, plan tasks, summarize evidence, or
choose a model", and it does not: it never inspects an act's payload for meaning.

What it does enforce, each with a conformance test named after it:

- **typed acts** — you cannot accept what was not proposed, challenge what was not claimed, or
  discharge what was not accepted, and a commitment cannot be discharged twice;
- **rejected acts never enter the ledger** — an unauthorised or unfunded move must not be able to
  write history;
- **attenuating authority** — delegation can only narrow, and revocation is transitive over the
  whole subtree;
- **affine budgets** — `Budget` does not implement `Clone`, so duplicating an allowance is a
  compile error rather than a runtime check; splitting *moves* it;
- **hash-chained ledgers** — claims and their challenges both survive; contradiction is preserved,
  not resolved into a score;
- **continuations** — a handle bound to a stale ledger head is refused rather than silently
  rebased; forking from a superseded point is the supported move.

Where Weave meets FIBER is the Context Capsule. A capsule is a recipient-specific projection of a
compiled Decision Section, so it inherits the certificate: a participant learns what the *compiler*
omitted from the world and, separately, what the *projection* withheld from it. A filtered capsule
reports `supports_sufficiency_claim: false` regardless of the compiler's own verdict — a
participant reasoning from a partial view cannot vouch for completeness it never observed.

## Generating a benchmark family

```bash
./target/release/bioprism mutate family --world w.json --out-dir family/
```

Applies eight metamorphic relations, each declaring what the oracle must do — four invariances
(rename, reorder, add distractors, camouflage tags) and one repair per leakage mechanism. **A
mutation does not get to mark its own homework**: the postcondition is checked by running the
oracle, and a mutation whose declared relation does not hold is rejected rather than shipped.

The headline number is deliberately not the instance count:

```
8 validated instances from 1 audited parent across 8 mutation families,
providing 8 independent equivalence classes (inflation ×1.00).
Instance count is not benchmark count.
```

An *equivalence class* is a distinct (parent, mutation family, oracle signature) triple — a counted
quantity, not a modelled one. Generate twenty reorderings of the same world and you get twenty
instances, **one** equivalence class, and an inflation ratio of ×20; the family is reported as a
robustness check rather than a benchmark. This is the executive summary's constraint made
operational: *a million paraphrases are not a million benchmarks*.

Deduplication hashes semantic content — facts, factors, events — and deliberately **not**
`world_id`, so a generator cannot defeat it by renaming.

## What is deliberately not implemented

The blueprint describes far more than exists here, and the gap is reported by the software rather
than buried in prose. Every compile returns `deferred_passes`, and `bioprism context explain`
prints them:

| Pass | Why it cannot run |
|---|---|
| Gluing and obstruction tests (43.06) | Requires a declared cover; `fiber-world/0.1` carries none |
| Abstract interpretation (43.11) | Requires an abstract-domain registry absent from the wire schema |
| FIBER wire integration of decision-equivalence quotient (43.10) | `fiber-query/0.3` now carries a bounded explicit loss/utility matrix and permitted-action boundary, and FIBER executes the exact quotient; 0.1/0.2 remain deferred |
| FIBER wire integration of rate-distortion optimisation (43.12) | `fiber-query/0.4` now binds a normalized prior, ordered observed evidence pool, compatibility floor and tolerance; FIBER executes identification, exhaustive frontier and minimal sufficiency. The 16-item bound and caller-declared model inputs remain explicit |
| FIBER wire integration of adaptive acquisition (43.15) | `fiber-query/0.5` now binds a normalized prior, complete outcome likelihood partitions, scalarized path budget, and finite horizon; FIBER executes the exact policy under 16/16/65,536 caps and returns certificate-bound planning provenance. It does not schedule, authorize, execute, or receipt an acquisition |

The backend portfolio of 43.19–43.24 (FAQ/InsideOut, worst-case-optimal joins, tensor networks,
decision diagrams, incremental view maintenance) is **not built**. `Backend` enumerates them so
the plan descriptor is honest about which one ran; only
`backward_factor_slice_reference` exists today.

Per 43.43, nothing here claims to have invented sheaves, factor graphs, semirings, tensor
networks, abstract interpretation, rate-distortion theory, or database query optimisation.

### Two honesty mechanisms worth knowing about

**Zero influence is not unknown influence.** The omission manifest classes every omitted group as
`zero`, `bounded`, `inaccessible_by_policy`, `deferred_acquisition` or `unknown`. Only `zero` and
`bounded` support a sufficiency claim; a single `unknown` group voids it. The reference v0.1
certificate has one `classification` *string* for all omissions and cannot express this, which is
why `--profile extended` exists.

**Zero-influence claims state their assumption.** Facts with no backward dependency path are
classed `zero` *conditional on the declared factor graph being complete* — the reason string says
so, because an incomplete factor graph turns a zero-influence claim into an unknown-influence one.

## Defects found in the v0.6 distribution

1. **`machine/module_registry.jsonl` is stale and omits FIBER entirely.** It carries 935 rows and
   **zero** from section 43, while `context_cards.jsonl` and `doc_graph.json` both carry all 51
   FIBER modules (994 rows each). `machine/README.md` claims one row per module. Any agent routing
   off the registry never sees the canonical runtime.
2. **The reference runtime hard-codes a radiogenomic goal string** into every Decision Section,
   and compares label timestamps **lexicographically as strings** rather than as parsed instants.
   Both are reproduced for parity, both are flagged: see `REFERENCE_GOAL` in
   [`qir.rs`](crates/fiber/src/qir.rs) and the note on `temporal_witnesses` in
   [`oracle.rs`](crates/fiber/src/oracle.rs).

Only 131 of the 935 registered modules are marked `Build-Ready Specification`; **400 are
`Planned`**. Sections 01–19 and 23–29 are 0% build-ready, including `03_CORE_SPECIFICATIONS`, all
50 files of `23_AGENT_INTERWEAVE_FABRIC` and all 24 of `25_BIOLOGICAL_IR_AND_LANGUAGE`. Those need
design work before implementation, not just coding.

## Repository layout

```
crates/           the workspace, bottom of the dependency DAG first
  ids/            canonical serialization + hashing + typed ids  (no internal deps)
  scope/          typed scope base                               (ids)
  world/          FIBER world model                              (ids, scope)
  section/        Decision Section + Context Certificate         (ids)
  fiber/          the query compiler                             (ids, scope, world, section)
  baseline/       equal-engineering comparators                  (ids, world, section, fiber)
  worldgen/       synthetic structural benchmark families        (world)
  store/          content-addressed indexed storage              (ids, scope, world)
  mcp/            Model Context Protocol server                  (fiber, section, store, world)
  prism/          decision-state evaluation                      (baseline, fiber, section, world)
  weave/          the multi-agent microkernel                    (ids, section)
  mutation/       metamorphic instance generation                (fiber, section, world)
  cli/            the bioprism binary                            (all)
docs/             ARCHITECTURE, FINDINGS, COVERAGE, the ADRs, and generated comparisons
fixtures/         golden worlds, queries and reference artifacts
reference/        the CPython reference runtime, vendored as the parity oracle
schemas/          fiber-world / fiber-query / fiber-context-certificate JSON Schemas
tools/            golden regeneration and ground-truth generation
```

`section` deliberately depends on neither `world` nor `fiber`: a consumer — an MCP client, an
evaluator, a CI gate — must be able to read and verify a compiled context without linking the
engine that produced it.

## Development

```bash
cargo test --workspace --offline
```

```bash
cargo clippy --workspace --all-targets --offline
```

```bash
python tools/regenerate_golden.py
```

The last one re-derives the golden artifacts from the CPython reference. A diff there is a change
to the wire format and needs a schema version bump.

Builds are offline by default (`.cargo/config.toml`) against pinned dependency versions.

## Boundary

Research and developer infrastructure. It does not diagnose an individual, recommend treatment,
triage care, autonomously enroll participants, or claim medical-device functionality. Compression
or abstraction never authorizes crossing a data-use, consent, privacy, or clinical boundary.

## Engineering manifest audit

The engineering_manifest_audit route adds the build-ready engineering artifact surface:
technology baseline, package dependency topology, ticket-to-package contracts and readiness, ADR
supersession, RACI ownership, independent-review separation, canonical digest, and explicit
warning/blocking issue semantics. It validates declared coherence only; it does not inspect a
checkout, run CI, query GitHub, or grant release authority. See
docs/ENGINEERING_MANIFEST_AUDIT.md.

The `engineering_execution_plan` route builds a deterministic, bounded implementation schedule on
top of that artifact: ticket states, dependency-aware waves, critical path, truncation policy,
and explicit schedule gates. It still does not mutate a tracker, run CI, inspect the checkout, or
authorize delivery. See docs/ENGINEERING_EXECUTION_PLAN.md.

## Release-pipeline audit

The `release_pipeline_audit` route adds a bounded delivery contract over stage DAGs, artifact
digests and lineage, provenance/signature bindings, environment protection, approval floors,
promotion order, and explicit rollback targets. `release_ready` is derived from blocking issues;
it is not evidence that CI ran or that a deployment succeeded. The route never executes commands,
contacts CI or registries, verifies cryptographic signatures, or mutates deployment state. See
docs/RELEASE_PIPELINE_AUDIT.md.

## Operational-readiness audit

The `operational_readiness_audit` route adds a bounded service-operability contract over declared
objectives, observed indicators with digest-bound evidence, dependency failure fallbacks, reviewed
runbooks, incident timelines/postmortems, and on-call/observability/backup/access controls.
`operationally_ready` is derived from blocking issue rows; it does not mean telemetry was queried,
an operator was paged, a fallback or restore was executed, an incident system was updated, or a
deployment was authorized. See docs/OPERATIONAL_READINESS_AUDIT.md.

## Security/privacy governance audit

The `security_privacy_audit` route adds an artifact-level governance contract over data-asset
classification, purpose/retention/residency/deletion, authorized flows, identity hardening,
high-risk threat treatment, independent review evidence, and required security/privacy controls.
`security_privacy_ready` is derived from named blocking issues; it is not legal compliance, a live
security scan, proof of authentication or encryption, executed red-team evidence, or a data-erasure
claim. See docs/SECURITY_PRIVACY_AUDIT.md.

## Sandbox admission audit

The `sandbox_admission_audit` route adds a bounded admission contract for untrusted code and
research artifacts: content-addressed artifact identity and lineage, rootless/read-only/no-
escalation profiles, network and mount boundaries, exact dangerous capabilities, finite resource
ceilings, quarantine, and reviewed output release. `sandbox_ready` is derived from six independent
row families and blocking issue rows; it is not proof that code ran safely or that an external
runtime enforced the declaration. The route never executes code, mounts paths, opens sockets,
reads secrets, or mutates quarantine. See docs/SANDBOX_ADMISSION_AUDIT.md.

## Sandbox runtime simulation

The `sandbox_runtime_simulate` route is the deterministic process-side companion to admission. It
selects an admitted profile, evaluates an ordered bounded request trace against exact capability
targets and resource ceilings, charges cumulative usage, and preserves `simulated`, `refused`, and
`not_run` rows plus a trace digest. `sandbox_runtime_ready` requires valid admission and a fully
simulated trace; a refusal is never charged and, by default, stops the remaining requests. This is
still a decision simulation: it does not start a process, execute code, resolve host paths, open
sockets, read secrets, or enforce namespaces, syscalls, cgroups, credentials, or network policy.
See docs/SANDBOX_RUNTIME_SIMULATION.md.

## Security, safety, and red-team program audit

The `security_program_audit` route audits the program around red-team work: authorized scope,
independent campaign review, immutable evidence, finding-to-remediation closure, incident
containment/closure, sequential disclosure, public-safety review, regression witnesses, and
explicit program controls. `security_program_ready` is derived from seven independent row
families and blocking issues; it is not proof that scanners ran, incidents were contained,
disclosures were sent, or controls are live. See docs/SECURITY_PROGRAM_AUDIT.md.

## License

Apache-2.0
