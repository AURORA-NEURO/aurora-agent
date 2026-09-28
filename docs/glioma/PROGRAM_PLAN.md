# Glioma research engine program plan

This is the implementation map for the executable glioma slice. The source tree is organized
by product program, not by infrastructure layer: every folder owns a bounded research capability
surface and its `mod.rs` is the ownership boundary. Each program has 32 stable feature slots,
formed by eight capability archetypes at four operating scales:

`scientific_algorithm | typed_data_primitive | agent_automation | workflow_orchestration |
researcher_interaction | api_protocol_integration | verification_safety | operations_federation`

at `local_single_study | multimodal_multi_study | prospective_high_throughput |
federated_continual`. A feature is admitted only when it has a named consumer, typed inputs and
outputs, a shipped artifact, a reproducible acceptance gate, and a route into at least one other
program. The portfolio is preclinical-only and never makes a clinical decision.

## Execution order

The engine follows this product flow rather than a collection of disconnected algorithms:

`intent → evidence → typed knowledge → multimodal readiness → decision context → mechanism →
experiment design → protocol simulation → instrument preflight → computation → interpretation /
replication → research-object release → federated benchmark`

Promotion waves are: (1) local deterministic behavior, (2) multimodal multi-study behavior,
(3) prospective campaign behavior, and (4) federated continual behavior. A later wave cannot
silently bypass an earlier wave's omission, provenance, safety, or replay gate.

## Folder charters

### P01 — `p01_evidence_surveillance`

- Consumer: evidence curator and autonomous research director.
- Product contract: bounded literature/evidence surveillance with novelty, temporal-shift,
  contradiction, source-calibration, acquisition, and refresh routes.
- Primary artifacts: `EvidenceSurveillance`, `EvidenceAcquisitionPlan`, `EvidenceTriangulation`,
  and `EvidenceRefreshCampaign`.
- Downstream edges: P02 typed knowledge, P03 modality requirements, P05 mechanism candidates,
  and P10 replication/contradiction review.
- Promotion gate: source identity, retrieval omission accounting, negative evidence retention,
  and replayable priority ranking.
- Current implementation: 32/32 slots. Maintain with independent federation reproductions,
  continual benchmark promotion, and provider-backed execution evidence.

### P02 — `p02_evidence_knowledge`

- Consumer: knowledge engineer and autonomous workflow planner.
- Product contract: typed claim graphs, consistency/closure, study alignment, drift, federated
  continual consensus, action planning, multimodal synchronization, and workflow admission.
- Primary artifacts: `TypedKnowledge`, `FederatedContinualKnowledge`, `LocalResearchWorkflow`,
  `MultimodalKnowledgeWorkflow`, and `ResearchWorkflowAdmission`.
- Downstream edges: every program; P02 is the typed handoff between evidence and execution.
- Promotion gate: explicit support/contradiction/unknown states, dependency-closed plans,
  preclinical boundary, deterministic branch selection, and honest omission reporting.
- Current implementation: 32/32 slots. Maintain with long-horizon calibration transport,
  campaign outcome assimilation, and independently reproducible protocol benchmarks.

### P03 — `p03_multimodal_ingestion_qc`

- Consumer: data steward, computational scientist, and modality operator.
- Product contract: ingestion, harmonization, concordance, missingness, reliability, drift,
  spatial/temporal fusion, and quality remediation across glioma modalities.
- Primary artifacts: `MultimodalQcReport`, harmonized vectors, quality schedules, and recovery
  campaigns.
- Downstream edges: P02 readiness, P05 mechanism state, P06 design, P09 computation, and P10
  interpretation.
- Promotion gate: modality-level quality provenance, dropout stress, cross-study alignment, and
  no complete-case shortcut when missingness is informative.
- Current implementation: 32/32 slots. Maintain with new modality adapters and benchmark worlds.

### P04 — `p04_decision_context`

- Consumer: researcher and decision-context agent.
- Product contract: bounded context compilation, value optimization, admission, action graphs,
  branch planning, omission certification, and campaign control.
- Primary artifacts: `DecisionContext`, `DecisionActionGraph`, `DecisionOmissionCertificate`,
  `DecisionBranchEvidence`, and adaptive branch campaigns.
- Downstream edges: P05 mechanism actions, P06 experiment design, P07 protocol simulation, and
  P08 instrument preflight.
- Promotion gate: every action has typed prerequisites, value/risk budget, unresolved context,
  and a falsifiable stop condition.
- F06 contract: each study may provide at most one digest-addressed outcome per selected action.
  Completed, negative, failed, blocked, and unknown remain distinct; absence means no outcome was
  supplied. Policy-denied studies cannot contribute outcomes. The multi-study artifact retains the
  eligible outcome ledger and holds any action with an adverse result out of its shared frontier.
  F30 replays those outcome states and refuses temporal promotion after negative, failed, blocked,
  or unknown history. F07 satisfies only the corresponding local dependency when a completed result
  exists and does not schedule that action again at that study; it retains the completion digests.
  This remains a research planning contract, not execution authority.
- F02 contract: decision-context replay retains its exact epoch snapshots, local action outcomes,
  and promotion threshold in the output. Verification validates every child context and recomputes
  the transitions and outcome partitions from that retained ledger, rejecting digest-restamped
  summary changes. Epoch, outcome, action-row, request-byte, and report-byte limits bound the replay
  record; the output remains a planning artifact and does not grant execution authority.
- F29 contract: the consortium decision steward supplies 2–32 consecutive, digest-valid
  `FederatedDecisionContextReport` values for one exact objective plus explicit temporal-stability,
  site, independent-group, support, uncertainty, risk, heterogeneity, and influence thresholds.
  The promotion evaluator retains the exact aggregate reports and their digests, requires the same
  top-ranked qualified branch throughout the configured consecutive-epoch window, refuses reused
  observation identities across epochs, and holds any candidate with negative, contradicted,
  failed, unknown, underpowered, or policy-ineligible evidence in its stability window. It emits a
  replayable promotion report with the full epoch ledger, adverse-evidence lineage, reason codes,
  and a route back to P04's decision cycle or researcher review. Acceptance requires exact objective
  and contiguous epoch binding, each child report's own digest validation, bounded report size,
  deterministic decision replay, and no raw-data transfer or autonomous assay execution. It depends
  on P04-F04 and P04-F24; it does not establish site identity, clinical utility, or release authority.
- F30 contract: a study-context steward supplies 2–32 consecutive `MultiStudyDecisionContextArtifact`
  values for one exact objective, with temporal stability, study/group quorum, support, and
  disagreement thresholds. The replay validates every source digest, requires stable study-to-group
  identity, derives each action's per-epoch disposition from the recorded frontier and explicit
  metrics, and retains additions, retirements, qualification loss, omissions, and typed completed,
  negative, failed, blocked, and unknown local outcomes with their result digests. An action is
  stable only after the configured consecutive qualified window and without historical adverse or
  conflicted action evidence. Output verification deterministically
  replays the full retained source ledger; size and row limits bound the request/report, and no
  assay or instrument is dispatched. It depends on P04-F06 and routes only to the P04 research
  cycle, review, or further workflow planning.
- F31 contract: a workflow operator supplies the exact F07 workflow plan, its F06 source artifact,
  digest-valid cross-program `GliomaExecutionReceipt` values, and explicit task/receipt/stage
  bindings. The adapter checks study identity and requires the bound engine stage to match the
  action's declared stage kind; approval-held tasks and stage reuse are rejected. It retains a
  bounded, value-free source snapshot and maps completed stages to completed, blocked failures to
  failed, clean blocks to blocked, and generic negative/partial engine stages to unknown. The
  generic engine receipt cannot distinguish a dry-run negative label from a measured biological
  negative, so this boundary never promotes that label to negative evidence. The output can be
  converted into per-study F06 outcomes, replays its sanitized receipt/task ledger, and routes
  missing receipts or unresolved results explicitly. It does not dispatch work or make scientific
  or clinical conclusions. It depends on P04-F06, P04-F07, and the shared glioma engine receipt.
- Current implementation: 24/32 slots. The matching BioPRISM source blueprint is not bundled in
  this checkout; the next wave is to check the remaining P04 slots against that source before
  assigning further feature work.

### P05 — `p05_mechanism_exploration`

- Consumer: mechanism scientist and computational biologist.
- Product contract: competing mechanism portfolios, identifiability, invariance, dynamics,
  state filtering/smoothing, pathway activity, counterfactual ensembles, and intervention value.
- Primary artifacts: `MechanismPortfolio`, mechanism graphs, posterior state trajectories, and
  robust intervention portfolios.
- Downstream edges: P06 discriminating experiments, P07 simulated protocols, P10 causal/replication
  review, and P12 transport benchmarks.
- Promotion gate: rival mechanisms remain visible, uncertainty is calibrated, interventions are
  preclinical research actions only, and counterfactual claims are not treated as observations.
- Current implementation: 32/32 slots. Maintain with federated continual mechanism assurance,
  adversarial replay, cross-site safety promotion, and new evidence-driven extensions.

### P06 — `p06_experiment_design`

- Consumer: experimentalist and assay allocation agent.
- Product contract: power-aware, blocked, sequential, adaptive, multi-fidelity, dose/synergy,
  clone-panel, replication, and mechanism-validation design.
- Primary artifacts: `ExecutableExperimentDesign`, allocation campaigns, power-stress surfaces,
  and validation protocols.
- Downstream edges: P07 simulation, P08 instrument preflight, P09 computation, and P10 replication.
- Promotion gate: estimand, randomization, controls, power uncertainty, null-result path, and
  resource/budget constraints are explicit before execution.
- Current implementation: 32/32 slots. Maintain with assay-specific design plugins and prospective
  re-estimation benchmarks.

### P07 — `p07_protocol_simulation`

- Consumer: lab operations lead and research director.
- Product contract: resource-feasible protocol simulation, branch optimization, compensation,
  autonomous campaign scheduling, and multi-program mission execution planning.
- Primary artifacts: `ProtocolSimulationReport`, branch plans, campaign state, and mission plans.
- Downstream edges: P08 signed instrument preflight, P09 computation placement, P11 release, and
  P12 benchmark runs.
- Promotion gate: no physical effect before simulation, interlocks, compensation, resource budget,
  and deterministic replay pass.
- Current implementation: 32/32 slots. Maintain with queue contention, fault injection, and
  long-horizon campaign worlds.

### P08 — `p08_instrument_robotics`

- Consumer: instrument operator and institution-local gateway.
- Product contract: calibration, signal extraction, batch stability, multichannel concordance,
  fleet scheduling, signed preflight, human authorization, and emergency-stop handling.
- Primary artifacts: `InstrumentPreflight`, instrument plans, fleet campaigns, and assay evidence.
- Downstream edges: P03 QC, P07 protocol state, P09 computation, and P11 research-object release.
- Promotion gate: A3 physical execution requires signed preflight, interlocks, revocation checks,
  local-only raw data, and honest partial-execution compensation.
- Current implementation: 16/32 slots. Failure recovery and queue-aware fleet routing are already
  implemented and registered. The existing F16 science loop and F25 research-frontier handoff now
  carry an explicit instrument-to-analysis provenance contract: assay observations bind by
  `(run_id, action_id)`, assessments retain campaign order and each execution digest, and the loop
  input digest commits to the exact request and observations. `validate_against` rechecks the
  requested preflight/run binding and recomputes each assessment from its run-scoped observations;
  F25 consumes the run-bound assessments directly and commits to the source-loop digest. This
  closes the known multi-run attribution gap without allocating a new slot. The upstream source
  blueprint is still absent, so do not name another P08 feature wave until its contract is checked.

### P09 — `p09_reproducible_computation`

- Consumer: computational scientist and workflow runtime.
- Product contract: checkpointed multimodal DAGs, resource placement, replay, recovery,
  robustness-guided computation, and interpretation-frontier compilation.
- Primary artifacts: `ComputationRun`, replay tapes, robustness suites, and computation portfolios.
- Downstream edges: P10 interpretation, P11 release, and P12 federated benchmark aggregation.
- Promotion gate: byte-stable canonicalization, resource termination, crash/retry recovery,
  negative-result retention, and independent replay.
- Current implementation: 13/32 slots registered in the research feature catalog: F01 robustness,
  F02 reproducibility, F03 artifact-lineage joins, F10 execution, F11 portfolio planning, F12
  portfolio execution, F13 campaigns, F14 workflow compilation, F15 robustness-guided computation,
  F20 worker placement, F24 operating cycle, F25 recovery campaign, and F26 interpretation-frontier
  compilation. The earlier next-wave items for placement optimization, artifact-lineage joins, and
  robustness-guided recomputation are therefore already delivered; do not count them again. The
  matching source blueprint for the remaining 19 slots is not bundled in this checkout, so name
  their next implementation wave only after checking that source contract.

### P10 — `p10_interpretation_replication`

- Consumer: methods reviewer and replication scientist.
- Product contract: uncertainty-aware causal interpretation, transportability, mediation,
  dynamic policies, contradiction adjudication, replication closure, and adaptive frontier work.
- Primary artifacts: `AnalysisReplicationRecord`, causal claim adjudications, and replication
  closure campaigns.
- Downstream edges: P06 follow-up design, P11 release verdicts, and P12 consortium comparisons.
- Promotion gate: estimand clarity, uncertainty, sensitivity, rival explanations, independent
  reproduction, and explicit null/negative outcomes.
- P10-F32 — longitudinal replication transport. Consumer: replication scientist. Inputs are
  independent study-level effect observations on one declared shared time grid, each tied to a
  local artifact, source model system, and population signature. The compiler retains complete,
  incomplete, low-quality, and too-distant study states; it uses one common eligible study set at
  every timepoint, then reports similarity-weighted fixed-point effects, heterogeneity,
  leave-one-study-out influence, pooled longitudinal slope, and directional agreement. Missing
  timepoints are never imputed. Model-system identity is categorical: a different source system
  incurs the maximum 1000 gap, and signature similarity continues to weight any source admitted
  under an explicit request threshold. Promotion requires study quorum, full time-grid coverage,
  acceptable model gap, heterogeneity and influence, and stable direction; a stable sub-threshold
  trend is retained as negative evidence. Inputs are bounded to 4,096 studies, 16,384 observations,
  and effects within ±1,000,000,000 milli-units. Leave-one-study-out effects are recomputed by
  removing each study's weighted contribution from precomputed totals. Dependencies: P10-F13 trajectory analysis, P10-F17 model
  transportability, and P10-F22 validation replication gate. Downstream edges: P11 provenance and
  P12 aggregate replication comparison. Acceptance requires deterministic request/source/output
  digests, local de-identified artifact references, explicit exclusion reasons, replay validation,
  and negative/null outcome retention. The source blueprint distribution is not configured in this
  checkout, so these thresholds and completeness rules are explicitly repository-defined rather
  than claimed as a verbatim blueprint contract.
- P10-F02 — multi-study pairwise effect concordance (`scientific_algorithm` at
  `multimodal_multi_study`). Consumer: replication scientist. This repository-defined algorithm
  compares local preclinical study effects only when the caller binds the exact same estimand and
  effect unit. Each study carries a source-report digest, unique independence group, model system,
  quality, replicate count, uncertainty interval, explicit estimate/null/unresolved state, and a
  local de-identified artifact. Low-quality and unresolved rows remain in the output but cannot
  pass the evidence floor. Same-model study pairs are compared by their full effect intervals;
  cross-model pairs are reported as incomparable instead of being pooled or treated as transport.
  Opposite, confidently non-null intervals are explicit discordance; interval overlap, interval
  gap, expected-direction support, and expected-direction refutation remain separate fields.
  Admission, independent-group, directional-support, quality, and direction-concordance floors are
  explicit request fields.
  The output reports per-study and pairwise states, not a pooled effect estimate, so it complements
  P10-F09 meta-analysis and P10-F17 transportability. States are `insufficient_evidence`,
  `cross_model_only`, `negative`, `heterogeneous`, `discordant`, and `concordant`. Bounds: 512
  study rows and 130,816 pair rows; effect/uncertainty magnitudes are capped at 1,000,000,000
  milli-units. Dependencies: P10-F10 causal contrast, P10-F09 meta-analysis, and P10-F17
  transportability. Downstream edges: P10-F03 contradiction planning and P11 provenance.
  Acceptance requires estimand/unit and source binding, unique independent groups, local artifacts,
  complete same-model pair accounting, explicit cross-model and null/unresolved states,
  deterministic replay/tamper checks, and zero external dispatch. The detailed source blueprint is
  not configured in this checkout; this contract is repository-defined.
- P10-F03 — prospective contradiction resolution (`scientific_algorithm` at
  `prospective_high_throughput`). Consumer: methods reviewer and experimental design lead. This
  is distinct from P01-F08's audit-cut planner: it compiles a bounded prospective experiment
  portfolio to discriminate caller-declared mutually exclusive rival hypotheses. The request
  binds one estimand/effect unit and at least two digest-bound hypotheses. Evidence records bind a
  hypothesis, independent group, assessment (`supports`, `refutes`, `null`, `unresolved`), quality,
  and local de-identified source artifact. A source group is counted once; null, unresolved, and
  low-quality evidence stay visible and do not count as support. A contradiction is active only
  after independent support floors establish at least two rivals, or one supported rival also has
  an independent refutation. Candidate resolver designs arrive from P06 with one predicted effect
  interval per rival, feasibility, risk, cost, and a digest-bound local protocol artifact. A rival
  pair is covered only when prediction intervals are separated by the declared minimum margin.
  The deterministic budgeted selector chooses designs by newly covered rival pairs per cost, with
  feasibility/risk gates and stable tie-breaks; it reports uncovered pairs and skipped candidates.
  States are `insufficient_evidence`, `no_contradiction`, `no_discriminator`, `budget_blocked`,
  `partial`, and `plan_ready`. `plan_ready` means only that the predeclared discriminating design
  covers all rival pairs; it never means an experiment ran or a hypothesis was resolved. Bounds:
  16 rivals, 16,384 evidence records, 1,024 candidates, 32 selected designs, and signed effect /
  uncertainty magnitudes capped at 1,000,000,000 milli-units. Dependencies: P10-F07 claim
  adjudication, P10-F19 interpretation synthesis, and P06 experiment design. Downstream edges: P06
  protocol planning, P11 provenance, and P12 independent replication comparison. Acceptance
  requires independent-group accounting, local artifact binding, candidate interval completeness,
  full-pair coverage or explicit unresolved-pair accounting, deterministic replay/tamper checks,
  budget/risk enforcement, and zero external dispatch. The detailed source blueprint is not
  configured in this checkout; this contract is repository-defined.
- P10-F04 — registered-outcome reporting completeness (`scientific_algorithm` at
  `multimodal_multi_study`). Consumer: research-integrity reviewer. This repository-defined audit
  compares digest-bound local registry protocols with reviewed result-report metadata. Each study
  binds one unique independence group, a registry report, protocol artifact, optional registration
  and first-enrollment days, completion status/day, and at least one registered primary outcome.
  An optional result report lists only outcome identifiers and reporting state (`reported`,
  `incomplete`, or `ambiguous`); effect values, direction, significance, and raw records are never
  accepted. Registration timing is derived from the supplied day fields and is explicit as
  prospective, retrospective, or unknown. Primary outcomes become due only for a completed study
  after the caller-declared reporting lag; recent, ongoing, terminated, and unknown-status studies
  are retained without being treated as missing completed results. The audit marks overdue planned
  outcomes absent from a report as missing, reports unregistered outcomes separately, and retains
  every study/outcome row. It reports completeness thresholds and review signals only: absence does
  not establish an unpublished, negative, or selectively suppressed result, and retrospective
  registration or an unregistered reported outcome is not itself proof of misconduct or bias.
  States are `insufficient_evidence`, `insufficient_follow_up`, `material_reporting_gap`,
  `registration_anomaly`, `uncertain`, and `coverage_threshold_met`. Bounds: 512 studies, 64
  registered outcomes per study, 128 reported outcomes per study, 32,768 total outcome rows, a
  200,000-day absolute day ceiling, and a 3,650-day maximum reporting lag. Dependencies: P10-F02
  multi-study concordance and P01 registry/source evidence. Downstream edges: P10-F03 prospective
  contradiction planning and P11 negative-result disclosure. Acceptance requires exact source-report
  coverage, independent-group accounting, local de-identified artifacts, canonical replay/tamper
  validation, complete overdue-primary accounting, and zero external dispatch. The detailed source
  blueprint is not configured in this checkout; this contract is repository-defined.
- P10-F05 — registered-outcome evidence record (`typed_data_primitive` at `local_single_study`).
  Consumer: research-integrity reviewer. This repository-defined record binds one study's exact
  outcome/estimand/unit, independent-group identity, registry report, optional result report, local
  de-identified artifacts, and reviewed availability state (`estimate`, `null`, `missing`,
  `incomplete`, `ambiguous`, `not_due`, or `unresolved`). Estimate/null rows require bounded effect,
  uncertainty, and quality values; unavailable states carry no effect values. Its schema rejects
  undeclared fields, status/value mismatches, direct identifiers, and non-local artifacts. The
  record is content-digested and independently replayable. It makes no multi-study claim and does not
  interpret effect direction. Acceptance requires all status shapes, artifact boundaries, digest
  tamper checks, deterministic replay, and zero external dispatch. The detailed source blueprint is
  not configured in this checkout; this contract is repository-defined.
- P10-F06 — registered-outcome evidence panel (`typed_data_primitive` at `multimodal_multi_study`).
  Consumer: replication lead and evidence integrator. This repository-defined panel combines at
  least two and at most 512 validated F05 records for the exact same outcome/estimand/unit, each from
  a unique independent group. It binds the complete canonical registry/result report digest sets,
  sorts records by study ID, preserves each availability state, and emits explicit availability
  counts plus a replayable panel digest. It is a typed data boundary only: no effects are pooled,
  directional support is inferred, or missing values imputed. Acceptance requires exact record and
  digest coverage, independent-group uniqueness, schema-compatible source records, permutation-
  stable replay/tamper checks, local de-identified artifacts, and zero external dispatch. The
  detailed source blueprint is not configured in this checkout; this contract is repository-defined.
- The companion missing-outcome sensitivity route is a derived view over the same per-study row
  shape: it evaluates caller-declared bounded scenarios separately from observed results and never
  changes the typed F05/F06 evidence records.
- Current implementation: 32/32 slots. F02 is implemented as a repository-defined multi-study
  pairwise concordance analysis, F03 as a repository-defined prospective contradiction planner,
  F04 as a repository-defined registered-outcome completeness audit, F05 as the single-study typed
  outcome record, and F06 as its multi-study typed evidence panel.

### P11 — `p11_research_object_release`

- Consumer: reproducibility steward and public research commons.
- Product contract: portable research-object assembly, replay campaigns, release gates, limitations,
  provenance, and immutable release lifecycle.
- Primary artifacts: `SignedResearchObject`, release manifests, replay reports, and release gates.
- Downstream edges: P12 federation and every upstream program's publication handoff.
- Promotion gate: complete provenance, methods/limitations, replay evidence, policy-compliant
  localization, signed checksums, and negative-result disclosure.
- Current implementation: 20/32 slots: the base research-object manifest (F01), typed disclosure
  register (F02), multi-study disclosure panel (F03), prioritized disclosure batch ledger (F04),
  multimodal bundle (F05), migration planning (F06), dependency closure (F07), replay campaign (F10), archival
  replay history reconciliation (F12), local release workflow (F13), multi-study release
  reconciliation (F14), prospective release batch queue (F15), federated continual release
  change control (F16), local steward review workbench (F17), portfolio review workbench (F18),
  prospective batch review workbench (F19), release gate (F20), local signature protocol (F21),
  signed local trust policy (F22), and release operating cycle (F24). The MCP
  program catalog reports implementation status from the research crate's explicit feature
  registry, so the remaining slots stay distinguishable from executable capabilities.
- F12 contract: the reproducibility steward supplies one research-object digest, at least two
  ordered epochs (up to 32), a minimum two-site quorum (up to 64 sites in any epoch), and at most
  512 bounded site commitments paired with already validated replay campaigns. The output is a
  canonical, digest-chained aggregate with per-epoch counts and reproducible, divergent,
  insufficient, or unresolved states. It depends on
  P11-F10 campaign validation and P09 reproducible-computation content hashes. It does not retrieve
  archives, execute old code, authenticate site commitments, or replace P12's consortium benchmark.
  Site commitments must be opaque and preferably keyed; the caller is responsible for archive
  retrieval, identity authentication, and signer verification. Acceptance requires input identity
  and duplicate checks, canonical ordering, bounded work, tamper-detecting chain/report digests,
  preserving negative and unresolved outcomes, and no task/objective/artifact data in the aggregate.
- F02 contract: the local reproducibility steward supplies one valid P11-F01 manifest. The
  compiler emits one sorted, digest-bound row for each negative-result or limitation statement,
  binding each row to the exact manifest digest and single-study identity. Rows contain
  category-specific statement digests without copying the statement text. The repository-defined
  bounds are 256 total rows, 4096 characters per source statement, and 65536 total source
  characters. Acceptance requires complete disclosure coverage, unique statements within each
  category, stable ordering, a verified output digest, and explicit false values for scientific-
  truth verification and release authorization. This structural register does not establish that a
  statement is scientifically correct or that the manifest is ready to sign. The source blueprint
  distribution is absent from this checkout; F02 is a repository-defined contract derived from
  P11's typed-data/local-study catalog slot. Statement digests are unkeyed integrity commitments,
  not confidentiality protection; callers must not use them to conceal guessable disclosure text.
- F03 contract: the reproducibility steward supplies one research identity, an expected cohort of
  2 to 128 unique study/manifest commitments with caller-declared independence groups, and zero or
  more observed F01 manifests paired with their F02 disclosure registers. Every observed child is
  revalidated and must match exactly one expected study and manifest digest. Missing expected
  studies remain explicit; complete means only that every expected manifest/register pair was
  supplied. The panel preserves per-study negative-result/limitation counts and child register
  digests without pooling counts or copying disclosure text. Acceptance requires at least two
  declared independence groups, exact expected-cohort accounting, unique study/manifest identity,
  deterministic row ordering, bounded inputs, and a replay-validated panel digest. Independence
  remains caller-declared and unauthenticated; scientific truth and release authorization remain
  false. The source blueprint distribution is absent from this checkout; F03 is a repository-defined
  contract derived from P11's typed-data/multi-study catalog slot. Child statement digests are
  unkeyed and do not provide confidentiality.
- F04 contract: the queue steward predeclares 2 to 128 P11-F03 panel submissions, each with a
  unique item ID, bounded priority, research identity, and exact F03 request digest. Submitted
  items carry one F03 panel request and its F01/F02 study inputs; each panel is recomputed and
  matched to its predeclared identity and digest. The ledger sorts by priority and item ID, keeps
  omitted items as awaiting submission, and reports complete or partial state with per-panel study
  counts. It never pools disclosure counts, returns statement text, dispatches work, authenticates
  independence, verifies scientific truth, or authorizes release. Acceptance requires deterministic
  order, unique item and request identities, bounded inputs, exact digest binding, and replay
  validation. The source blueprint distribution is absent from this checkout; F04 is a
  repository-defined contract derived from P11's typed-data/prospective-high-throughput catalog
  slot. Statement commitments remain unkeyed and do not provide confidentiality.
- F13 contract: the local reproducibility steward supplies one matching multimodal package and
  replay request, a dependency depth/program-coverage policy, and accountable release-gate policy.
  The workflow compiles the package, computes transitive dependency closure, and only invokes the
  caller-owned replay executor when package and closure are closed; it then evaluates the release
  gate and returns an operator handoff. It depends on P11-F05/F07/F10/F20. Acceptance requires all
  request policy checks before executor invocation, strict identity and digest binding across every
  stage, fail-closed behavior for partial or blocked closure, exact phase ordering, explicit
  negative/unresolved evidence, and no signature, upload, publication, or raw-data movement.
- F14 contract: the release steward supplies a complete ordered set of opaque study commitments
  and bounded P11-F13 workflow reports for one research identity. The output preserves each
  study's manifest/workflow digests and state while reporting missing, ready, held, unresolved,
  blocked, and non-reproducible counts. It never merges artifacts or promotes one study's result
  to another. Acceptance requires exact expected-cohort accounting, unique study identities and
  commitments, validated child workflow digests, canonical digest chaining, size limits, and a
  ready disposition only when every expected study is ready for its own accountable signing
  review. Caller-supplied study commitments and review identities are not authenticated here.
- F15 contract: the local release queue owner submits up to 128 prioritized P11-F13 candidates,
  one shared replay budget, and a workflow-count cap. The controller validates and prepares every
  candidate before using the caller-owned executor, then dispatches serially in stable priority,
  submission, and job order. It defers candidates whose minimum admitted task budget is unavailable,
  records per-job budget and workflow digests, and stops after an execution failure whose resource
  use cannot be reconciled. It depends on P11-F13 and the bounded replay controller P11-F10.
  Acceptance requires no executor calls before whole-queue preflight, global spend never exceeding
  the supplied budget, deterministic order, explicit budget/workflow-limit deferrals, and no
  continuation after unknown spend. This is bounded local batch dispatch, not parallel execution.
- F16 contract: the federation release steward supplies one stable expected cohort, two to 32
  ordered release epochs, validated P11-F14 portfolio reports, and a bounded independent-review
  quorum for manifest changes. The coordinator compares each study's manifest to the immediately
  preceding epoch, binds each supplied review to the exact old/new digest pair, and preserves
  missing epochs, missing studies, continuity gaps, held workflows, and divergent outcomes. Only
  complete portfolios with no unresolved continuity and sufficient independent change reviews
  become ready for signing review. It composes P11-F14 portfolio reports and follows P11-F20's
  accountable-review boundary. Acceptance
  requires stable research/cohort identity, report and request limits, unique reviewer commitments,
  review binding to actual changed manifests, deterministic digest chaining, and fail-closed
  treatment of missing or non-independent evidence. Caller-supplied site and reviewer commitments
  are not authenticated; the coordinator does not sign, publish, upload, contact sites, or move raw
  data.
- F17 contract: the local reproducibility steward supplies a validated P11-F13 workflow and
  digest-bound checklist responses for provenance/package, dependency closure, replay, release
  gate, and limitations/negative evidence. The workbench compiles a fixed review packet, records
  pending, acknowledged, action-required, or unresolved responses, and returns a digest-chained
  review record. A completed checklist is not itself a P11-F20 approval, signer authorization, or
  publication. Acceptance requires validated child-workflow identity, a bounded checklist and
  response set, unique known item commitments, no acknowledgement of unavailable evidence,
  canonical item ordering, explicit incomplete/upstream negative states, and a report that marks
  reviewer identity as unauthenticated and release authorization as false. It depends on P11-F13
  local workflow and the P11-F20 gate evidence.
- F18 contract: the portfolio reviewer supplies a validated P11-F14 cohort report and up to 64
  P11-F17 local review packets, each mapped to one opaque expected study commitment. The workbench
  checks exact workflow-digest and disposition binding, then preserves per-study missing, pending,
  corrective, unresolved, held, blocked, and non-reproducible states. Portfolio review completes
  only when every expected study has a completed checklist and its F14 workflow is ready for its
  own signing review. Acceptance requires exact cohort accounting, unique study mappings, child
  packet validation, bounded payloads, canonical digest chaining, and no cross-study promotion.
  Reviewer and study commitments remain caller-supplied and unauthenticated; the report does not
  satisfy a signing quorum or authorize, sign, or publish.
- F19 contract: the high-throughput queue steward supplies a validated P11-F15 batch report and
  up to 128 P11-F17 local review packets keyed by job ID. The workbench binds each packet to the
  exact produced workflow digest and disposition, keeps deterministic queue order, and preserves
  budget deferrals, workflow failures, and jobs not attempted after a failure. It marks the review
  queue complete only when all produced workflows have complete review packets and the batch has
  no deferred or failed jobs. Acceptance requires exact batch/job identity, unique packet mapping,
  child validation, bounded payloads, review/workflow status agreement, digest chaining, and no
  cross-job promotion. Reviewer commitments are unauthenticated caller inputs; this workbench
  cannot authorize or publish release objects.
- F21 contract: the local release steward supplies one ready P11-F13 workflow, its completed
  P11-F17 review packet, a signer key identifier, a public Ed25519 key, a detached signature, and
  a caller-issued challenge digest. The verifier binds the signature to canonical bytes covering
  the manifest, workflow, review packet, release gate, key identifier, public-key digest, and
  challenge before emitting a verification record. Acceptance requires exact child identity/digest
  bindings, bounded and canonical inputs, cryptographic verification, and explicit false values for
  signer identity authentication, key authority, replay prevention, release authorization, and
  publication. The verification record retains the validated workflow and review packet so a
  downstream consumer can recompute every bound digest after transport. It depends on
  P11-F13/F17/F20. It does not manage private keys, establish trust in the caller's public key,
  enforce challenge uniqueness, authorize release, or publish.
- F22 contract: the release-key administrator supplies the F21 signature-verification record, a
  bounded release trust policy, its detached Ed25519 signature, and the policy authority key ID,
  public key, and expected key digest loaded from local operator configuration. Each sorted policy
  grant binds one exact signer key and research identity to the P11 release-approval purpose, a
  validity interval, and optional revocation time. The evaluator revalidates F21, checks the local
  root pin, verifies the signed policy, and evaluates the matching grant at an explicit timestamp.
  Acceptance requires canonical bounded policy bytes, unique and sorted grants, exact key/scope/
  purpose matching, time and revocation checks, proof inputs retained for independent validation,
  and a decision digest. The root pin remains caller-configured; signer trust is not release
  authorization and challenge replay remains unchecked. It does not manage private keys or publish.
- Next wave: the other 12 unimplemented slots. Assign each slot a typed contract, consumer,
  dependency edge, and acceptance gate before adding its implementation.

### P12 — `p12_federated_benchmarking`

- Consumer: consortium administrator and independent evaluator.
- Product contract: aggregate-only benchmark design, power, mechanism transport, quorum, privacy,
  localization, adaptive campaigns, and governance.
- Primary artifacts: `FederationBenchmark`, transport plans, benchmark campaigns, and governance
  verdicts.
- Downstream edges: P01 surveillance calibration, P03 QC transport, P05 mechanism transport, P10
  replication, and P11 release.
- Promotion gate: no raw-data movement, signed capability/policy manifests, quorum, outlier and
  leave-one-site-out analysis, and independently reproducible aggregate results.
- Current implementation: 10/32 slots registered in the research feature catalog: F01 consensus,
  F02 power, F10 benchmark campaigns, F12 site planning, F20 mechanism transport, F24 operating
  cycle, F26 adaptive site campaigns, F28 transport campaigns, F29 replication transport, and F30
  federated interpretation. Adaptive site selection is already delivered by F12 and F26; do not
  count it again as future work. Benchmark-world versioning and a cross-consortium negative-result
  registry remain candidate themes only; assign either to a slot after checking its source contract.

## Change control

Every new feature must first be assigned a folder, stable `GAF-GLIOMA-P##-F##` slot, consumer,
typed contract, dependency edges, and acceptance gate in this plan. Then it may add code under the
owning folder, exports, MCP/CLI/API surfaces, tests, and documentation. The organization validator,
catalogue validator, full crate tests, protocol tests, and replay/negative-path tests are release
gates. A feature is not counted for coverage merely because a file or hypothesis exists.
