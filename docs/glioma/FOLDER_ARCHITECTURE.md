# Glioma research engine: folder architecture

This file is generated from `PROGRAM_PLAN.md`, `organization.json`, and the live Rust module tree. It is an ownership and integration map, not evidence that every listed algorithm is biologically validated.

- Programs: 12
- Stable feature slots: 384
- Implemented feature IDs: 384
- Planned feature IDs: 0
- Feature modules: 384
- Workflow/composition modules: 23
- Module-local Rust test annotations: 1473 (a count is not a measure of assertion quality or scientific validation)

## Research workflow and ownership flow

`intent → P01 evidence → P02 typed claims → P03 multimodal readiness → P04 decision context → P05 mechanisms → P06 experiments → P07 protocol simulation → P08 local instrument gateway → P09 computation → P10 interpretation/replication → P11 research object → P12 aggregate federation`

A program folder owns its algorithms and public module exports. Cross-program composition remains explicit in the modules listed below; the program-level edges and release gates remain specified in `PROGRAM_PLAN.md`. Raw experimental payloads remain institution-local by default. Human-subject and clinical-source data, diagnosis, treatment, triage, and enrollment remain out of scope.

## Program folders

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
- Refresh campaigns now choose each round with a deterministic coverage-aware batch selector:
  newly represented modalities and model systems receive bounded utility bonuses, so a high-
  priority duplicate cannot consume the whole refresh budget while orthogonal glioma evidence is
  still stale, contradictory, or unknown. The selected order remains replay-bound and every
  returned record is still validated locally before replacing its evidence ID.
- The refresh campaign now recompiles a complete local surveillance frontier for planning after
  each round, while retaining the caller's per-round execution cap. Already-refreshed or failed
  top actions cannot strand lower-priority evidence behind the public cap; the final artifact
  exposes the canonical deferred frontier as `GliomaEvidenceRefreshCampaign1@3`.
- P01-F02 `novelty_radar` now emits `priority_milli` and ranks by the full acquisition score rather
  than raw novelty alone. Freshness, source quality, citation signal, domain gaps, and near-duplicate
  suppression therefore influence the queue as declared; `GliomaEvidenceNoveltyRadar1@2` keeps the
  ranked order deterministic without treating novelty as validity or causal support.
- P01 contradiction cuts now use a bounded portfolio beam over signed evidence edges. Complete
  edge-cover closure dominates the objective, with severity, independent groups, source-family
  diversity, confidence, and cost as deterministic refinements; an isolated disagreement selects
  the contradiction anchor on ties, while multi-claim cuts preserve complementary support anchors.
  Partial vertex covers remain explicitly unresolved so the autonomous loop schedules the missing
  support/contradiction audit instead of manufacturing closure; budget-blocked and replication
  work remain first-class routes (`GliomaEvidenceContradictionCut1@2`).
- P01-F09 `surveillance` now preserves the complete priority-ranked change-action frontier under
  a caller action cap. Selected actions remain executable in priority order, while every omitted
  review, contradiction, refresh, or scope-reassessment action is canonicalized in a disjoint
  deferred partition with explicit uncertainty (`GliomaEvidenceSurveillance1@2`).
- Current implementation: 32/32 slots. Maintain with independent federation reproductions,
  continual benchmark promotion, and provider-backed execution evidence.

Folder inventory: 31 source modules; 32/32 feature slots implemented. The program folder directly owns 31 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`acquisition`](crates/research/src/glioma/programs/p01_evidence_surveillance/acquisition.rs) | GAF-GLIOMA-P01-F22 | Budgeted evidence-acquisition portfolio planning for autonomous glioma research. | 3 | — |
| [`acquisition_campaign`](crates/research/src/glioma/programs/p01_evidence_surveillance/acquisition_campaign.rs) | GAF-GLIOMA-P01-F23 | Execute a selected evidence-acquisition portfolio through a caller-owned local adapter. | 3 | `acquisition` |
| [`acquisition_feedback`](crates/research/src/glioma/programs/p01_evidence_surveillance/acquisition_feedback.rs) | GAF-GLIOMA-P01-F15 | Idempotent assimilation of site-local acquisition outcomes into P01 evidence state. | 3 | `federated_acquisition_policy` |
| [`calibration`](crates/research/src/glioma/programs/p01_evidence_surveillance/calibration.rs) | GAF-GLIOMA-P01-F11 | Source-family calibration for autonomous preclinical glioma evidence surveillance. | 3 | — |
| [`campaign`](crates/research/src/glioma/programs/p01_evidence_surveillance/campaign.rs) | GAF-GLIOMA-P01-F20 | Beam-selected autonomous evidence-refresh campaigns for preclinical glioma research. | 5 | `surveillance` |
| [`continual_promotion`](crates/research/src/glioma/programs/p01_evidence_surveillance/continual_promotion.rs) | GAF-GLIOMA-P01-F32 | Continual promotion and rollback control for the federated glioma research commons. | 4 | `federated_batch_scheduler` |
| [`contradiction_cut`](crates/research/src/glioma/programs/p01_evidence_surveillance/contradiction_cut.rs) | GAF-GLIOMA-P01-F08 | Beam-selected minimum-evidence-cut planning for contradictory preclinical glioma surveillance. | 5 | — |
| [`evidence_cluster`](crates/research/src/glioma/programs/p01_evidence_surveillance/evidence_cluster.rs) | GAF-GLIOMA-P01-F05 | Evidence-cluster indexing for local preclinical glioma surveillance. | 3 | — |
| [`evidence_frontier_join`](crates/research/src/glioma/programs/p01_evidence_surveillance/evidence_frontier_join.rs) | GAF-GLIOMA-P01-F13 | Join qualified glioma evidence into an actionable scientific frontier. | 3 | — |
| [`evidence_knowledge_bridge`](crates/research/src/glioma/programs/p01_evidence_surveillance/evidence_knowledge_bridge.rs) | GAF-GLIOMA-P01-F27 | Verified evidence-to-knowledge handoff for the autonomous preclinical glioma engine. | 4 | `p02_evidence_knowledge::knowledge_graph`, `verification_gate` |
| [`evidence_stream`](crates/research/src/glioma/programs/p01_evidence_surveillance/evidence_stream.rs) | GAF-GLIOMA-P01-F07 | Prospective high-throughput evidence stream snapshots for glioma research. | 3 | — |
| [`federated_acquisition_policy`](crates/research/src/glioma/programs/p01_evidence_surveillance/federated_acquisition_policy.rs) | GAF-GLIOMA-P01-F12 | Consortium-aware acquisition policy for unresolved preclinical glioma evidence. | 3 | — |
| [`federated_batch_scheduler`](crates/research/src/glioma/programs/p01_evidence_surveillance/federated_batch_scheduler.rs) | GAF-GLIOMA-P01-F31 | Prospective high-throughput scheduling for federated glioma evidence cycles. | 4 | `federated_operating_cycle` |
| [`federated_execution_handoff`](crates/research/src/glioma/programs/p01_evidence_surveillance/federated_execution_handoff.rs) | GAF-GLIOMA-P01-F16 | Policy-bounded handoff of glioma acquisition actions to institution-local adapters. | 3 | `federated_acquisition_policy` |
| [`federated_operating_cycle`](crates/research/src/glioma/programs/p01_evidence_surveillance/federated_operating_cycle.rs) | GAF-GLIOMA-P01-F30 | Autonomous operating-cycle compiler for federated glioma evidence. | 4 | `federated_outcome_transport`, `long_horizon_calibration`, `outcome_reconciliation` |
| [`federated_outcome_transport`](crates/research/src/glioma/programs/p01_evidence_surveillance/federated_outcome_transport.rs) | GAF-GLIOMA-P01-F29 | Policy-gated transport of aggregate glioma outcomes between research sites. | 4 | — |
| [`federated_shift`](crates/research/src/glioma/programs/p01_evidence_surveillance/federated_shift.rs) | GAF-GLIOMA-P01-F04 | Aggregate-only federated consensus over prospective preclinical glioma evidence shifts. | 2 | — |
| [`long_horizon_calibration`](crates/research/src/glioma/programs/p01_evidence_surveillance/long_horizon_calibration.rs) | GAF-GLIOMA-P01-F28 | Prospective long-horizon calibration and drift control for glioma evidence surveillance. | 4 | — |
| [`multimodal_gap_router`](crates/research/src/glioma/programs/p01_evidence_surveillance/multimodal_gap_router.rs) | GAF-GLIOMA-P01-F14 | Cross-modality gap routing for the glioma evidence frontier. | 3 | `evidence_frontier_join` |
| [`multimodal_workbench`](crates/research/src/glioma/programs/p01_evidence_surveillance/multimodal_workbench.rs) | GAF-GLIOMA-P01-F18 | Multimodal, multi-study researcher workbench for preclinical glioma evidence. | 5 | — |
| [`novelty_adjudication`](crates/research/src/glioma/programs/p01_evidence_surveillance/novelty_adjudication.rs) | GAF-GLIOMA-P01-F06 | Claim-level novelty adjudication for preclinical glioma evidence. | 3 | — |
| [`novelty_radar`](crates/research/src/glioma/programs/p01_evidence_surveillance/novelty_radar.rs) | GAF-GLIOMA-P01-F02 | Deterministic novelty radar for continuous preclinical glioma evidence surveillance. | 2 | — |
| [`operating_cycle`](crates/research/src/glioma/programs/p01_evidence_surveillance/operating_cycle.rs) | GAF-GLIOMA-P01-F24 | Intent-to-evidence operating cycle for preclinical glioma research. | 1 | `acquisition`, `acquisition_campaign` |
| [`outcome_reconciliation`](crates/research/src/glioma/programs/p01_evidence_surveillance/outcome_reconciliation.rs) | GAF-GLIOMA-P01-F26 | Aggregate-only multi-site outcome reconciliation for preclinical glioma research. | 5 | — |
| [`priority`](crates/research/src/glioma/programs/p01_evidence_surveillance/priority.rs) | GAF-GLIOMA-P01-F10 | Continuous evidence-priority scheduling for preclinical glioma research. | 4 | `evidence` |
| [`prospective_triage`](crates/research/src/glioma/programs/p01_evidence_surveillance/prospective_triage.rs) | GAF-GLIOMA-P01-F19 | Prospective evidence-triage workbench for high-throughput preclinical glioma surveillance. | 3 | `evidence_stream` |
| [`researcher_workbench`](crates/research/src/glioma/programs/p01_evidence_surveillance/researcher_workbench.rs) | GAF-GLIOMA-P01-F17 | Local single-study researcher workbench for evidence exploration. | 4 | — |
| [`surveillance`](crates/research/src/glioma/programs/p01_evidence_surveillance/surveillance.rs) | GAF-GLIOMA-P01-F09 | Continuous evidence-surveillance delta detection for preclinical glioma programs. | 3 | — |
| [`temporal_shift`](crates/research/src/glioma/programs/p01_evidence_surveillance/temporal_shift.rs) | GAF-GLIOMA-P01-F03 | Prospective temporal-shift detection for preclinical glioma evidence streams. | 2 | — |
| [`triangulation`](crates/research/src/glioma/programs/p01_evidence_surveillance/triangulation.rs) | GAF-GLIOMA-P01-F21 | Cross-family evidence triangulation for preclinical glioma claims. | 4 | `evidence` |
| [`verification_gate`](crates/research/src/glioma/programs/p01_evidence_surveillance/verification_gate.rs) | GAF-GLIOMA-P01-F25 | Local preclinical evidence verification gate for the glioma research engine. | 6 | — |

Shared public feature modules owned outside this folder:

| Feature ID | Source module | Purpose | Direct test annotations |
|---|---|---|---:|
| `GAF-GLIOMA-P01-F01` | [`evidence`](crates/research/src/glioma/evidence.rs) | Evidence surveillance and qualification for preclinical glioma programs. | 2 |

---

### P02 — `p02_evidence_knowledge`


- Consumer: knowledge engineer and autonomous workflow planner.
- Product contract: typed claim graphs, consistency/closure, study alignment, drift, federated
  continual consensus, action planning, multimodal synchronization, and workflow admission.
- Primary artifacts: `TypedKnowledge`, `FederatedContinualKnowledge`, `LocalResearchWorkflow`,
  `MultimodalKnowledgeWorkflow`, and `ResearchWorkflowAdmission`.
- Downstream edges: every program; P02 is the typed handoff between evidence and execution.
- Promotion gate: explicit support/contradiction/unknown states, dependency-closed plans,
  preclinical boundary, deterministic branch selection, and honest omission reporting.
- Knowledge-frontier batches now use a bounded, deterministic action-family novelty bonus: near-tied
  claims are spread across closure, contradiction, uncertainty, negative revalidation, and supported
  validation work while materially stronger claims retain priority.
- The closed-loop frontier now searches a bounded portfolio over the full eligible candidate set
  rather than truncating top-ranked rows before applying budget. Utility-per-cost and action-family
  diversity favor complementary glioma evidence work, while every non-selected candidate remains
  explicitly deferred in `GliomaClosedLoopFrontier1@2` for replay, budget expansion, or review.
- Knowledge-resolution campaigns now execute from the frontier's complete ranked claim set rather
  than only its capped selected subset. The configured maximum remains the per-round execution
  bound, but deferred claims are promoted in later rounds after completed or failed claims are
  removed, preserving the evidence-to-knowledge loop under bounded budgets.
- Current implementation: 32/32 slots. Maintain with long-horizon calibration transport,
  campaign outcome assimilation, and independently reproducible protocol benchmarks.

Folder inventory: 32 source modules; 32/32 feature slots implemented. The program folder directly owns 32 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`action_bridge`](crates/research/src/glioma/programs/p02_evidence_knowledge/action_bridge.rs) | GAF-GLIOMA-P02-F17 | Bridge validated P02 knowledge actions into the glioma action selector. | 1 | `action_compiler`, `claim_frontier` |
| [`action_compiler`](crates/research/src/glioma/programs/p02_evidence_knowledge/action_compiler.rs) | GAF-GLIOMA-P02-F16 | Evidence-frontier to research-action compilation for preclinical glioma. | 2 | `claim_frontier`, `knowledge_graph` |
| [`action_outcome_assimilation`](crates/research/src/glioma/programs/p02_evidence_knowledge/action_outcome_assimilation.rs) | GAF-GLIOMA-P02-F27 | Idempotent assimilation of P02 knowledge-action outcomes across retries and restarts. | 3 | `dispatch` |
| [`autonomous_cycle`](crates/research/src/glioma/programs/p02_evidence_knowledge/autonomous_cycle.rs) | GAF-GLIOMA-P02-F24 | Execute a bounded autonomous evidence-gap cycle for preclinical glioma research. | 2 | `claim_frontier`, `gap_compiler`, `knowledge_graph` |
| [`belief_revision`](crates/research/src/glioma/programs/p02_evidence_knowledge/belief_revision.rs) | GAF-GLIOMA-P02-F11 | Contradiction-aware belief revision for the preclinical glioma knowledge graph. | 3 | `knowledge_graph` |
| [`campaign`](crates/research/src/glioma/programs/p02_evidence_knowledge/campaign.rs) | GAF-GLIOMA-P02-F20 | Bounded autonomous claim-resolution campaigns for preclinical glioma research. | 4 | `claim_frontier`, `knowledge_graph` |
| [`claim_evidence_reconciliation`](crates/research/src/glioma/programs/p02_evidence_knowledge/claim_evidence_reconciliation.rs) | GAF-GLIOMA-P02-F29 | Belief-state reconciliation after a claim-to-experiment closure. | 3 | `claim_experiment_closure`, `knowledge_graph` |
| [`claim_experiment_closure`](crates/research/src/glioma/programs/p02_evidence_knowledge/claim_experiment_closure.rs) | GAF-GLIOMA-P02-F28 | Claim-to-experiment closure for autonomous preclinical glioma research. | 3 | `action_outcome_assimilation`, `dispatch`, `knowledge_graph` |
| [`claim_frontier`](crates/research/src/glioma/programs/p02_evidence_knowledge/claim_frontier.rs) | GAF-GLIOMA-P02-F09 | Beam-selected scientific claim-frontier prioritization for the autonomous glioma workflow. | 5 | `knowledge_graph` |
| [`closed_loop_frontier`](crates/research/src/glioma/programs/p02_evidence_knowledge/closed_loop_frontier.rs) | GAF-GLIOMA-P02-F30 | Closed-loop promotion from reconciled claim evidence to the next research frontier. | 4 | `claim_evidence_reconciliation` |
| [`closure`](crates/research/src/glioma/programs/p02_evidence_knowledge/closure.rs) | GAF-GLIOMA-P02-F05 | Typed evidence-closure analysis for autonomous preclinical glioma knowledge. | 2 | `knowledge_graph` |
| [`composition`](crates/research/src/glioma/programs/p02_evidence_knowledge/composition.rs) | GAF-GLIOMA-P02-F10 | Evidence-network composition for autonomous preclinical glioma research. | 3 | `knowledge_graph` |
| [`consistency`](crates/research/src/glioma/programs/p02_evidence_knowledge/consistency.rs) | GAF-GLIOMA-P02-F02 | Contradiction-aware consistency closure for typed preclinical glioma knowledge. | 1 | `composition`, `knowledge_graph` |
| [`continual_agent`](crates/research/src/glioma/programs/p02_evidence_knowledge/continual_agent.rs) | GAF-GLIOMA-P02-F12 | Autonomous planning over federated continual glioma knowledge. | 4 | `federated_continual` |
| [`dispatch`](crates/research/src/glioma/programs/p02_evidence_knowledge/dispatch.rs) | GAF-GLIOMA-P02-F19 | Execute a selected, dependency-closed knowledge-action batch. | 2 | `action_compiler`, `claim_frontier`, `knowledge_graph`, `selection_cycle` |
| [`federated_continual`](crates/research/src/glioma/programs/p02_evidence_knowledge/federated_continual.rs) | GAF-GLIOMA-P02-F08 | Robust federated continual knowledge fusion for preclinical glioma research. | 3 | `knowledge_graph` |
| [`federated_knowledge`](crates/research/src/glioma/programs/p02_evidence_knowledge/federated_knowledge.rs) | GAF-GLIOMA-P02-F04 | Aggregate-only federated typed-knowledge consensus for preclinical glioma research. | 2 | `knowledge_graph` |
| [`frontier_campaign`](crates/research/src/glioma/programs/p02_evidence_knowledge/frontier_campaign.rs) | GAF-GLIOMA-P02-F32 | Campaign-level scheduling for the reconciled glioma research frontier. | 3 | `closed_loop_frontier` |
| [`gap_compiler`](crates/research/src/glioma/programs/p02_evidence_knowledge/gap_compiler.rs) | GAF-GLIOMA-P02-F23 | Compile typed knowledge gaps into executable P01 acquisition candidates. | 3 | `claim_frontier`, `knowledge_graph` |
| [`knowledge_drift`](crates/research/src/glioma/programs/p02_evidence_knowledge/knowledge_drift.rs) | GAF-GLIOMA-P02-F03 | Prospective typed-knowledge drift detection for autonomous preclinical glioma research. | 2 | `knowledge_graph` |
| [`knowledge_graph`](crates/research/src/glioma/programs/p02_evidence_knowledge/knowledge_graph.rs) | GAF-GLIOMA-P02-F01 | Deterministic evidence-to-typed-knowledge compilation for preclinical glioma research. | 3 | — |
| [`knowledge_protocol_gateway`](crates/research/src/glioma/programs/p02_evidence_knowledge/knowledge_protocol_gateway.rs) | GAF-GLIOMA-P02-F21 | Typed-knowledge interoperability gateway for local and federated glioma research. | 3 | — |
| [`multimodal_protocol_gateway`](crates/research/src/glioma/programs/p02_evidence_knowledge/multimodal_protocol_gateway.rs) | GAF-GLIOMA-P02-F22 | Multimodal typed-knowledge interoperability for preclinical glioma studies. | 3 | — |
| [`multimodal_workflow`](crates/research/src/glioma/programs/p02_evidence_knowledge/multimodal_workflow.rs) | GAF-GLIOMA-P02-F14 | Adaptive synchronization for multimodal, multi-study glioma workflows. | 3 | `workflow_compile` |
| [`operating_cycle`](crates/research/src/glioma/programs/p02_evidence_knowledge/operating_cycle.rs) | GAF-GLIOMA-P02-F25 | Closed-loop typed-knowledge synthesis for autonomous preclinical glioma research. | 1 | `belief_revision`, `claim_frontier`, `composition`, `gap_compiler`, `knowledge_graph` |
| [`prospective_belief_calibration`](crates/research/src/glioma/programs/p02_evidence_knowledge/prospective_belief_calibration.rs) | GAF-GLIOMA-P02-F31 | Prospective calibration of glioma claim forecasts against observed preclinical outcomes. | 3 | — |
| [`prospective_monitor`](crates/research/src/glioma/programs/p02_evidence_knowledge/prospective_monitor.rs) | GAF-GLIOMA-P02-F07 | Prospective high-throughput monitoring of typed glioma knowledge. | 3 | `knowledge_graph` |
| [`selection_cycle`](crates/research/src/glioma/programs/p02_evidence_knowledge/selection_cycle.rs) | GAF-GLIOMA-P02-F18 | Knowledge-frontier selection cycle for the autonomous glioma engine. | 2 | `action_bridge`, `action_compiler` |
| [`study_alignment`](crates/research/src/glioma/programs/p02_evidence_knowledge/study_alignment.rs) | GAF-GLIOMA-P02-F06 | Explicit multi-study typed-knowledge alignment for preclinical glioma research. | 3 | `knowledge_graph` |
| [`workflow_admission`](crates/research/src/glioma/programs/p02_evidence_knowledge/workflow_admission.rs) | GAF-GLIOMA-P02-F15 | Admission control from multimodal readiness into bounded downstream glioma routes. | 3 | `multimodal_workflow`, `workflow_compile` |
| [`workflow_compile`](crates/research/src/glioma/programs/p02_evidence_knowledge/workflow_compile.rs) | GAF-GLIOMA-P02-F13 | Local workflow orchestration for federated-continual research actions. | 2 | `continual_agent` |
| [`workflow_recovery`](crates/research/src/glioma/programs/p02_evidence_knowledge/workflow_recovery.rs) | GAF-GLIOMA-P02-F26 | Checkpoint-aware recovery and resume planning for P02 knowledge workflows. | 3 | `workflow_compile` |

---

### P03 — `p03_multimodal_ingestion_qc`


- Consumer: data steward, computational scientist, and modality operator.
- Product contract: ingestion, harmonization, concordance, missingness, reliability, drift,
  spatial/temporal fusion, quality remediation, and local microscopy morphodynamics across
  preclinical glioma modalities.
- Primary artifacts: `MultimodalQcReport`, harmonized vectors, quality schedules, and recovery
  campaigns; biological-unit-held-out state posteriors and outcome likelihoods from segmented
  murine glioma cell tracks. The morphodynamic artifact is schema `GliomaMicroscopyMorphodynamics1@2`
  and carries explicit minimum top-posterior and top-vs-runner-up margin gates: ambiguous fields
  retain their calibrated posterior for researcher review but are abstained from downstream
  mechanism selection and instrument authorization.
- Downstream edges: P02 readiness, P05 mechanism state, P06 design, P08 adaptive microscopy, P09
  computation, and P10 interpretation.
- Promotion gate: modality-level quality provenance, dropout stress, cross-study alignment, and
  no complete-case shortcut when missingness is informative.
- Ingestion campaigns retain the complete eligible action frontier and select each bounded round
  with deterministic priority plus new modality, model-system, action-family, and target bonuses;
  this keeps missing coverage visible beside repeated repairs without fabricating observations.
- Current implementation: 32/32 slots. The temporal-fusion sub-capability derives robust speed,
  persistence, invasion-axis alignment, and shape summaries; validates by holding out biological
  units; reports calibration and negative evidence; and passes only qualified, in-domain,
  image-quality-gated field estimates to the one-capture-at-a-time P08 loop. Segmented tracks and
  analysis artifacts remain local; labels are externally supplied, motion is 2D, and this is not a
  causal or clinical classifier. Maintain with additional modality adapters and external benchmark
  worlds.

Folder inventory: 33 source modules; 32/32 feature slots implemented. The program folder directly owns 32 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`campaign`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/campaign.rs) | GAF-GLIOMA-P03-F20 | Bounded beam-selected autonomous multimodal-ingestion and QC campaigns for preclinical glioma research. | 6 | — |
| [`concordance`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/concordance.rs) | GAF-GLIOMA-P03-F02 | Feature-level concordance analysis for preclinical glioma modalities. | 3 | — |
| [`consensus`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/consensus.rs) | GAF-GLIOMA-P03-F03 | Deterministic multimodal consensus clustering for preclinical glioma samples. | 2 | `concordance` |
| [`contradiction_adjudication`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/contradiction_adjudication.rs) | GAF-GLIOMA-P03-F19 | Explicit multimodal contradiction adjudication for autonomous preclinical glioma research. | 3 | `evidence_fusion` |
| [`decision_gate`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/decision_gate.rs) | GAF-GLIOMA-P03-F18 | Uncertainty-aware endpoint decision gating for autonomous preclinical glioma workflows. | 3 | `evidence_fusion` |
| [`drift_surveillance`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/drift_surveillance.rs) | GAF-GLIOMA-P03-F08 | Continuous modality/metric drift surveillance for preclinical glioma QC. | 2 | — |
| [`dropout_stress`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/dropout_stress.rs) | GAF-GLIOMA-P03-F04 | Modality-dropout stress analysis for preclinical glioma interpretation. | 2 | — |
| [`evidence_fusion`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/evidence_fusion.rs) | GAF-GLIOMA-P03-F09 | Reliability- and uncertainty-aware endpoint evidence fusion for preclinical glioma research. | 2 | — |
| [`graph_fusion`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/graph_fusion.rs) | GAF-GLIOMA-P03-F15 | Reliability-weighted multimodal graph fusion for preclinical glioma studies. | 2 | `concordance` |
| [`harmonization`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/harmonization.rs) | GAF-GLIOMA-P03-F10 | Robust batch-effect harmonization for preclinical glioma modality vectors. | 3 | `concordance` |
| [`ingestion_manifest`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/ingestion_manifest.rs) | GAF-GLIOMA-P03-F01 | Typed multimodal ingestion manifest and pre-QC admission for preclinical glioma studies. | 3 | — |
| [`latent_factors`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/latent_factors.rs) | GAF-GLIOMA-P03-F11 | Deterministic multimodal latent-state factorization for preclinical glioma studies. | 3 | `concordance` |
| [`microscopy_morphodynamics`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/microscopy_morphodynamics.rs) | alias: `super::temporal_fusion::FEATURE_ID` | Local, replicate-held-out glioma cell morphodynamics for adaptive microscopy. | 5 | `concordance`, `temporal_fusion` |
| [`missingness_audit`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/missingness_audit.rs) | GAF-GLIOMA-P03-F05 | Missingness-mechanism audit for preclinical glioma multimodal studies. | 2 | — |
| [`modality_portfolio`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/modality_portfolio.rs) | GAF-GLIOMA-P03-F07 | Reliability- and budget-aware multimodal portfolio selection for preclinical glioma research. | 2 | — |
| [`operating_cycle`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/operating_cycle.rs) | GAF-GLIOMA-P03-F24 | Multimodal ingestion-to-readiness operating cycle for preclinical glioma research. | 1 | `campaign`, `readiness_gate` |
| [`prospective_quality`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/prospective_quality.rs) | GAF-GLIOMA-P03-F25 | Prospective multimodal quality forecasting for autonomous preclinical glioma workflows. | 3 | — |
| [`quality_adaptive_campaign`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/quality_adaptive_campaign.rs) | GAF-GLIOMA-P03-F28 | Closed-loop adaptive quality campaigns for preclinical glioma acquisition. | 3 | `quality_execution`, `quality_scheduler` |
| [`quality_execution`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/quality_execution.rs) | GAF-GLIOMA-P03-F27 | Schedule-bound execution for quality-aware preclinical glioma acquisition plans. | 3 | `quality_scheduler` |
| [`quality_recovery`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/quality_recovery.rs) | GAF-GLIOMA-P03-F32 | Conservative verification of multimodal QC recovery after a remediation action. | 3 | — |
| [`quality_remediation`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/quality_remediation.rs) | GAF-GLIOMA-P03-F31 | Budgeted remediation planning for multimodal glioma QC incidents. | 3 | `quality_root_cause` |
| [`quality_root_cause`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/quality_root_cause.rs) | GAF-GLIOMA-P03-F30 | Multimodal QC incident root-cause attribution for preclinical glioma research. | 3 | — |
| [`quality_scheduler`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/quality_scheduler.rs) | GAF-GLIOMA-P03-F26 | Quality-risk-aware acquisition scheduling for autonomous preclinical glioma workflows. | 3 | — |
| [`quality_transport`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/quality_transport.rs) | GAF-GLIOMA-P03-F29 | Cross-study quality-policy transport calibration for preclinical glioma workflows. | 3 | — |
| [`readiness_gate`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/readiness_gate.rs) | GAF-GLIOMA-P03-F23 | Multimodal research-readiness admission after an ingestion/QC campaign. | 2 | `campaign` |
| [`reliability_calibration`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/reliability_calibration.rs) | GAF-GLIOMA-P03-F06 | Replicate-aware modality reliability calibration for preclinical glioma studies. | 2 | — |
| [`sensitivity`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/sensitivity.rs) | GAF-GLIOMA-P03-F17 | Endpoint-sensitivity analysis for autonomous preclinical glioma research. | 3 | `evidence_fusion` |
| [`spatial_communication`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/spatial_communication.rs) | GAF-GLIOMA-P03-F13 | Spatial ligand–receptor communication inference for preclinical glioma tissue models. | 3 | — |
| [`spatial_niche`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/spatial_niche.rs) | GAF-GLIOMA-P03-F12 | Spatial niche graph analysis for preclinical glioma research. | 4 | — |
| [`spatial_propagation`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/spatial_propagation.rs) | GAF-GLIOMA-P03-F14 | Spatial state propagation for preclinical glioma research. | 3 | — |
| [`spatial_registration`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/spatial_registration.rs) | GAF-GLIOMA-P03-F22 | Robust landmark registration for multi-sample preclinical glioma spatial assays. | 3 | — |
| [`temporal_fusion`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/temporal_fusion.rs) | GAF-GLIOMA-P03-F21 | Longitudinal multimodal state-transition inference for preclinical glioma studies. | 6 | `concordance` |
| [`temporal_spatial_alignment`](crates/research/src/glioma/programs/p03_multimodal_ingestion_qc/temporal_spatial_alignment.rs) | GAF-GLIOMA-P03-F16 | Temporal-spatial state alignment for preclinical glioma research. | 2 | `spatial_niche`, `spatial_propagation`, `spatial_registration`, `temporal_fusion` |

Shared public feature modules owned outside this folder:

| Feature ID | Source module | Purpose | Direct test annotations |
|---|---|---|---:|
| `None` | [`multimodal`](crates/research/src/glioma/multimodal.rs) | Multimodal ingestion and quality control for glioma research objects. | 2 |

---

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
- Context compilation now retains the complete typed action frontier and uses a bounded,
  deterministic portfolio bonus for distinct action families, modalities, and model systems when
  the action cap is tight; deferred actions and omission reasons remain explicit.
- P04-F31 — `adaptive_context_scheduler` is the autonomous refresh controller. It ranks stale,
  contradictory, newly updated, and starved decision contexts across fairness groups under hard
  action and compute budgets, but admits only contexts with an actual refresh trigger. Fresh,
  noncritical contexts are retained as typed `not_due` deferrals so they do not consume refresh
  capacity or starve invalidated contexts. Critical budget deferrals become `budget_blocked`, while
  every other omission carries an action-limit, budget, or `not_due` reason; no deferred context is
  treated as current. The `GliomaContextRefreshSchedule1@2` schedule is deterministic and
  digest-bound, but dispatch remains with the caller-owned local context compiler.
- P04-F17 — `uncertainty_branch_explorer` is the researcher-facing comparison surface for
  competing decision branches. It joins the immutable context and scenario-aware portfolio plan
  with optional observed branch evidence, computes claim coverage, model-system/modalities,
  expected information gain, scenario disagreement, cost, failure risk, and a transparent
  weighted ranking. Forecast-only, evidence-limited, confirmed, contradicted, blocked, and
  unresolved states remain distinct; omitted claims and negative evidence remain visible. Notes
  are context-digest-bound annotations and the output is replay-stable. It is a decision-support
  capability for preclinical research, not a clinical recommendation or autonomous dispatch.
- P04-F18 — `cross_study_context_diff` is the field-level transportability gate before contexts
  are pooled. It compares model, assay, material, timing, environment, intervention, and
  missingness fields; applies only declared value/unit aliases; and distinguishes measured value
  differences from missing, unmeasured, unit, domain, and context-version mismatches. Pair-level
  transport warnings and modality acquisition gaps remain explicit for P06/P10 follow-up.
- P04-F26 — `cross_study_context_invariance_test` executes a typed rule across independent
  studies, applies declared nuisance-field perturbations, and retains leave-one-study-out flips,
  counterexamples, missing measurements, and unresolved group-floor states. It is a transport
  diagnostic for preclinical research, never a clinical or biological conclusion.
- P04-F29 — `snapshot_store` is the local immutable recovery substrate for long-running agents. It
  verifies each context digest and event lineage, rejects corrupt or unanchored parent chains,
  retains protected snapshots and all ancestors needed for recovery, and emits deterministic
  retention/restore omissions. The index is metadata-only at the MCP boundary; institution-local
  storage retains the context payload and no snapshot is treated as biological evidence.
- P04-F30 — `partition_checkpoint` reconciles typed site-local context deltas after a network
  partition. It acknowledges identical retries, classifies stale/future/anchored contributions,
  computes field-level convergence from content digests, and preserves conflicts instead of
  applying last-writer-wins. Partitioned or non-consensus checkpoints cannot be promoted, and the
  MCP surface returns metadata only while site payloads remain local.
- P04-F32 — `access_governor` is the field-level federation control plane. It binds a request to a
  declared preclinical purpose, verified membership/policy/scope digests, approval, expiry, and a
  revocation set; it emits allow, redact, deny, approval-required, or revoked decisions without
  widening scope or returning context values. Protected or non-local requests fail closed.
- Current implementation: 32/32 slots. Maintain with federated continual context promotion,
  richer outcome assimilation, decision-context replay across study epochs, and independent
  multi-site context reproductions.

P04-F08 `federated_decision_capsule` is the site-to-consortium context boundary. It packages a
question scope, content-addressed claims, evidence-coverage identifiers, omissions, uncertainty,
and downstream action identifiers while retaining locality and explicit expiry. Import rejects
stale, revoked, tampered, protected-payload, or over-scoped capsules; action authority is an
intersection with the receiving policy, never an expansion. The resulting capsule is a
simulation-only handoff to the existing P04/P05/P06 decision routes and never a clinical decision.

P04-F19 `decision_budget_dashboard` is the prospective resource-control surface for autonomous
glioma campaigns. It reconciles completed and running event consumption with branch forecasts over
assay, compute, time, and review resources; emits warning, approval-required, and hard-stop states;
and proposes deterministic, approval-bound reallocations when a high-priority branch exceeds its
current envelope. Forecast confidence and negative accounting evidence remain visible, and no
proposal authorizes spending or dispatches an experiment.

P04-F21 `decision_context_query_api` is the bounded read surface for autonomous glioma research
agents and workbenches. It queries typed local context records by schema-bound scope and field,
requires a digest-verified capability with expiry, revocation, and result budgets, and paginates
with content-addressed cursors. Omitted, uncertain, unavailable, and negative states remain
explicit; the route returns metadata and provenance digests only and never exports raw evidence or
makes a clinical decision.

P04-F23 `decision_event_update_api` is the live context update surface for prospective glioma
campaigns. It accepts content-addressed evidence, QC, resource, and action-outcome events against
an immutable context anchor, orders them deterministically, makes retries idempotent, and emits a
new digest-bound epoch per applied event. Contradictions, failed quality checks, exhausted
resources, blocked actions, and negative results invalidate executable branches into explicit
deferred/negative partitions without deleting history; stale anchors and conflicting retries fail
closed.

Folder inventory: 32 source modules; 32/32 feature slots implemented. The program folder directly owns 32 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`access_governor`](crates/research/src/glioma/programs/p04_decision_context/access_governor.rs) | GAF-GLIOMA-P04-F32 | Purpose-bound, revocable access control for federated glioma decision context. | 5 | `partition_checkpoint` |
| [`action_bridge`](crates/research/src/glioma/programs/p04_decision_context/action_bridge.rs) | GAF-GLIOMA-P04-F09 | Evidence-to-action portfolio selection for the autonomous glioma workflow. | 3 | `context_compiler` |
| [`action_graph`](crates/research/src/glioma/programs/p04_decision_context/action_graph.rs) | GAF-GLIOMA-P04-F10 | Dependency-closed decision-action graph compilation for preclinical glioma research. | 4 | `context_compiler`, `p02_evidence_knowledge::composition`, `p02_evidence_knowledge::knowledge_graph` |
| [`adaptive_branch_campaign`](crates/research/src/glioma/programs/p04_decision_context/adaptive_branch_campaign.rs) | GAF-GLIOMA-P04-F28 | Adaptive scenario-branch campaign for preclinical glioma research. | 2 | `branch_campaign`, `branch_planner`, `campaign`, `context_compiler` |
| [`adaptive_context_scheduler`](crates/research/src/glioma/programs/p04_decision_context/adaptive_context_scheduler.rs) | GAF-GLIOMA-P04-F31 | Adaptive refresh scheduling for the autonomous glioma research director. | 5 | — |
| [`adaptive_controller`](crates/research/src/glioma/programs/p04_decision_context/adaptive_controller.rs) | GAF-GLIOMA-P04-F15 | Adaptive exploitation/exploration control for the glioma decision portfolio. | 3 | `value_calibration`, `value_optimizer` |
| [`admission_gate`](crates/research/src/glioma/programs/p04_decision_context/admission_gate.rs) | GAF-GLIOMA-P04-F12 | Evidence- and authority-gated admission of generated glioma research actions. | 3 | — |
| [`branch_campaign`](crates/research/src/glioma/programs/p04_decision_context/branch_campaign.rs) | GAF-GLIOMA-P04-F27 | Bounded execution and failover for scenario-aware glioma decision branches. | 1 | `branch_planner`, `campaign`, `context_compiler` |
| [`branch_evidence`](crates/research/src/glioma/programs/p04_decision_context/branch_evidence.rs) | GAF-GLIOMA-P04-F03 | Branch-evidence assimilation for autonomous preclinical glioma decision contexts. | 2 | `branch_planner`, `context_compiler` |
| [`branch_planner`](crates/research/src/glioma/programs/p04_decision_context/branch_planner.rs) | GAF-GLIOMA-P04-F22 | Robust branch planning for autonomous preclinical glioma research. | 3 | `context_compiler`, `p02_evidence_knowledge::knowledge_graph` |
| [`campaign`](crates/research/src/glioma/programs/p04_decision_context/campaign.rs) | GAF-GLIOMA-P04-F20 | Bounded autonomous question-to-action campaigns for preclinical glioma research. | 4 | `action_bridge`, `context_compiler` |
| [`context_compiler`](crates/research/src/glioma/programs/p04_decision_context/context_compiler.rs) | GAF-GLIOMA-P04-F01 | Evidence-gap to next-action compilation for the autonomous glioma workflow. | 3 | — |
| [`context_replay`](crates/research/src/glioma/programs/p04_decision_context/context_replay.rs) | GAF-GLIOMA-P04-F02 | Epoch-aware decision-context replay for autonomous preclinical glioma research. | 2 | `context_compiler` |
| [`cross_study_context_diff`](crates/research/src/glioma/programs/p04_decision_context/cross_study_context_diff.rs) | GAF-GLIOMA-P04-F18 | Field-level cross-study context comparison for `GAF-GLIOMA-P04-F18`. | 5 | — |
| [`cross_study_context_invariance_test`](crates/research/src/glioma/programs/p04_decision_context/cross_study_context_invariance_test.rs) | GAF-GLIOMA-P04-F26 | Cross-study context invariance testing for `GAF-GLIOMA-P04-F26`. | 5 | `cross_study_context_diff` |
| [`decision_budget_dashboard`](crates/research/src/glioma/programs/p04_decision_context/decision_budget_dashboard.rs) | GAF-GLIOMA-P04-F19 | Prospective research-budget accounting and rebalancing for autonomous glioma programs. | 5 | — |
| [`decision_context_artifact`](crates/research/src/glioma/programs/p04_decision_context/decision_context_artifact.rs) | GAF-GLIOMA-P04-F05 | Portable typed decision-context artifact for preclinical glioma workflows. | 4 | `context_compiler` |
| [`decision_context_query_api`](crates/research/src/glioma/programs/p04_decision_context/decision_context_query_api.rs) | GAF-GLIOMA-P04-F21 | Typed, bounded, omission-aware queries over local glioma decision contexts. | 5 | — |
| [`decision_cycle`](crates/research/src/glioma/programs/p04_decision_context/decision_cycle.rs) | GAF-GLIOMA-P04-F24 | End-to-end decision-context orchestration for preclinical glioma research. | 2 | `action_bridge`, `action_graph`, `branch_planner`, `campaign`, `context_compiler` |
| [`decision_event_update_api`](crates/research/src/glioma/programs/p04_decision_context/decision_event_update_api.rs) | GAF-GLIOMA-P04-F23 | Event-sourced updates for a live preclinical glioma decision context. | 5 | `context_compiler` |
| [`decision_loop_governor`](crates/research/src/glioma/programs/p04_decision_context/decision_loop_governor.rs) | GAF-GLIOMA-P04-F16 | Sequential stopping and continuation policy for autonomous glioma research loops. | 3 | — |
| [`federated_decision_capsule`](crates/research/src/glioma/programs/p04_decision_context/federated_decision_capsule.rs) | GAF-GLIOMA-P04-F08 | Signed, bounded decision-context capsules for federated preclinical glioma research. | 5 | — |
| [`federated_decision_context`](crates/research/src/glioma/programs/p04_decision_context/federated_decision_context.rs) | GAF-GLIOMA-P04-F04 | Federated continual decision-context aggregation for preclinical glioma research. | 5 | — |
| [`mission_bridge`](crates/research/src/glioma/programs/p04_decision_context/mission_bridge.rs) | GAF-GLIOMA-P04-F25 | Decision-context to autonomous-mission execution bridge. | 2 | — |
| [`multi_study_context_artifact`](crates/research/src/glioma/programs/p04_decision_context/multi_study_context_artifact.rs) | GAF-GLIOMA-P04-F06 | Multi-study typed decision-context artifact for preclinical glioma research. | 4 | `decision_context_artifact` |
| [`multi_study_workflow`](crates/research/src/glioma/programs/p04_decision_context/multi_study_workflow.rs) | GAF-GLIOMA-P04-F07 | Dependency-safe multi-study workflow planning for preclinical glioma research. | 9 | `decision_context_artifact`, `multi_study_context_artifact` |
| [`omission_certificate`](crates/research/src/glioma/programs/p04_decision_context/omission_certificate.rs) | GAF-GLIOMA-P04-F11 | Deterministic omission certification for question-to-decision contexts. | 3 | `action_graph`, `context_compiler` |
| [`partition_checkpoint`](crates/research/src/glioma/programs/p04_decision_context/partition_checkpoint.rs) | GAF-GLIOMA-P04-F30 | Partition-resilient multi-study decision-context checkpoint reconciliation. | 5 | `context_compiler` |
| [`snapshot_store`](crates/research/src/glioma/programs/p04_decision_context/snapshot_store.rs) | GAF-GLIOMA-P04-F29 | Local immutable decision-context snapshot indexing for long-running glioma programs. | 4 | `context_compiler` |
| [`uncertainty_branch_explorer`](crates/research/src/glioma/programs/p04_decision_context/uncertainty_branch_explorer.rs) | GAF-GLIOMA-P04-F17 | Uncertainty-aware comparison of competing preclinical glioma research branches. | 4 | `branch_evidence`, `branch_planner`, `context_compiler` |
| [`value_calibration`](crates/research/src/glioma/programs/p04_decision_context/value_calibration.rs) | GAF-GLIOMA-P04-F14 | Outcome calibration for the glioma value-of-information planner. | 3 | `value_optimizer` |
| [`value_optimizer`](crates/research/src/glioma/programs/p04_decision_context/value_optimizer.rs) | GAF-GLIOMA-P04-F13 | Bounded value-of-information portfolio optimization for preclinical glioma decisions. | 3 | — |

---

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
- P05-F02 identifiability now uses a bounded portfolio beam over quality/risk-gated features. It
  optimizes resolved mechanism-pair coverage and separation under budget/cardinality, allowing
  complementary discriminators to beat a single expensive hub while preserving unresolved pairs
  as negative evidence in `GliomaMechanismIdentifiability1@2`.
- P05-F03 invariance now uses a bounded set-cover beam over transport-stable signatures. It scores
  complete cross-model pair coverage before separation value and cost, preserving complementary
  model-system signatures and unresolved transport pairs in `GliomaMechanismInvariance1@2`.
- P05-F27 `robust_portfolio` computes prior-weighted expected, lower-tail, and worst-case effects
  across the declared model ensemble, then uses a bounded deterministic beam/knapsack packer over
  redundancy groups. This preserves low-cost combinations long enough to beat a single
  high-rate perturbation when absolute robust value is greater; model disagreement, risk, and
  budget deferrals remain explicit.
- P05-F29 `pathway_activity` now aggregates repeated sample-lineage observations with a
  reliability-weighted median and mean reliability rather than selecting the single most reliable
  row. Large lineage spread is retained as stable disagreement uncertainty and negative evidence,
  so a resistant subpopulation remains visible while pathway direction stays bounded by the
  observed-node, modality, and confidence gates. The artifact is now
  `GliomaPathwayActivity1@2`: declared signed edges are evaluated against observed node signs,
  weighted by edge confidence, and retained as canonical conflict IDs. A pathway with missing
  edge evidence or contradictory interactions is unresolved rather than being released from node
  marginal averages alone; the edge floor and release policy are replay-bound.
- Mechanism-action plans now use a bounded mechanism-set novelty bonus: near-tied discriminator
  assays that probe previously unrepresented mechanism members can displace redundant pairs, while
  materially stronger information-per-cost actions retain priority and every selection is replayable.
  The compiler now also enforces a declared total assay-cost envelope while searching the portfolio,
  records planned versus reserved cost in `GliomaMechanismActionPlan1@3`, and rejects an expensive
  shortcut when complementary lower-cost discriminators cover more mechanism space within budget.
- P05-F30 `clonal_evolution` now scores candidate parentage with confidence-weighted marker overlap
  and a bounded present/absent conflict penalty. Low-confidence overlap cannot clear the declared
  parent-support floor, while surviving conflict edges retain stable marker-state uncertainty for
  follow-up assays; the graph remains an ambiguity-preserving preclinical lineage artifact rather
  than a certain phylogeny.
- P05-F17 `state_smoother` now uses a lower-median emission consensus across the features observed
  at each timepoint. A model that contradicts a measured feature cannot regain a confident emission
  by averaging a separate strong match; feature-level negative evidence and the existing entropy,
  coverage, transition-support, and change-point gates remain explicit.
- P05-F23 `closed_loop` now performs bounded beam/knapsack selection over eligible mechanism
  actions. Portfolio utility includes deterministic diminishing returns for repeated mechanism
  targets, allowing complementary low-cost discriminators to beat a single expensive assay while
  retaining exact deferred/blocked partitions for the next autonomous round.
- The P05 temporal/multimodal fusion composition now joins those mechanism predictions to local
  source-keyed observations across modality and timepoint. It weights residual agreement by QC,
  keeps source independence and coverage visible, separates contradiction from missingness, and
  emits a posterior-ranked `TemporalAcquisitionCandidate` frontier for unmeasured predictions.
  The `glioma_temporal_multimodal_mechanism_fusion` MCP route is analysis-only: artifacts remain
  local, no causal or clinical conclusion is emitted, and contradictory evidence yields an
  unresolved disposition rather than a forced winner.
- Current implementation: 32/32 slots. Maintain with federated continual mechanism assurance,
  adversarial replay, cross-site safety promotion, and new evidence-driven extensions.

Folder inventory: 34 source modules; 32/32 feature slots implemented. The program folder directly owns 31 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`action_planner`](crates/research/src/glioma/programs/p05_mechanism_exploration/action_planner.rs) | GAF-GLIOMA-P05-F20 | Compile mechanism-discrimination results into beam-selected executable glioma research actions. | 5 | `discrimination`, `p07_protocol_simulation` |
| [`adaptive_policy`](crates/research/src/glioma/programs/p05_mechanism_exploration/adaptive_policy.rs) | GAF-GLIOMA-P05-F31 | Observation-driven adaptive mechanism policies for preclinical glioma research. | 3 | — |
| [`bayesian_update`](crates/research/src/glioma/programs/p05_mechanism_exploration/bayesian_update.rs) | GAF-GLIOMA-P05-F08 | Deterministic Bayesian-style mechanism updating for preclinical glioma research. | 3 | `discrimination` |
| [`calibrated_campaign`](crates/research/src/glioma/programs/p05_mechanism_exploration/calibrated_campaign.rs) | GAF-GLIOMA-P05-F28 | Calibration-aware adaptive mechanism campaign for preclinical glioma research. | 2 | `adaptive_policy`, `calibration` |
| [`calibration`](crates/research/src/glioma/programs/p05_mechanism_exploration/calibration.rs) | GAF-GLIOMA-P05-F21 | Preclinical mechanism-probability calibration and promotion gates. | 3 | — |
| [`clonal_evolution`](crates/research/src/glioma/programs/p05_mechanism_exploration/clonal_evolution.rs) | GAF-GLIOMA-P05-F30 | Bounded clonal-evolution graph inference for preclinical glioma models. | 8 | — |
| [`closed_loop`](crates/research/src/glioma/programs/p05_mechanism_exploration/closed_loop.rs) | GAF-GLIOMA-P05-F23 | Closed-loop mechanism evidence control for preclinical glioma research. | 3 | `bayesian_update`, `discrimination`, `evidence_assimilation` |
| [`consensus`](crates/research/src/glioma/programs/p05_mechanism_exploration/consensus.rs) | GAF-GLIOMA-P05-F22 | Independent-evidence consensus for preclinical glioma mechanism research. | 3 | — |
| [`counterfactual`](crates/research/src/glioma/programs/p05_mechanism_exploration/counterfactual.rs) | GAF-GLIOMA-P05-F18 | Counterfactual mechanism perturbation for preclinical glioma research. | 3 | `graph_propagation` |
| [`discrimination`](crates/research/src/glioma/programs/p05_mechanism_exploration/discrimination.rs) | GAF-GLIOMA-P05-F09 | Mechanism discrimination and next-assay information gain for preclinical glioma research. | 2 | — |
| [`discrimination_campaign`](crates/research/src/glioma/programs/p05_mechanism_exploration/discrimination_campaign.rs) | GAF-GLIOMA-P05-F32 | Autonomous mechanism-discrimination campaigns for preclinical glioma research. | 3 | `discrimination` |
| [`ensemble_counterfactual`](crates/research/src/glioma/programs/p05_mechanism_exploration/ensemble_counterfactual.rs) | GAF-GLIOMA-P05-F19 | Model-averaged mechanism counterfactuals for preclinical glioma research. | 2 | `counterfactual`, `graph_propagation` |
| [`evidence_assimilation`](crates/research/src/glioma/programs/p05_mechanism_exploration/evidence_assimilation.rs) | GAF-GLIOMA-P05-F05 | Closed-loop mechanism evidence assimilation across preclinical glioma study epochs. | 2 | `bayesian_update`, `discrimination` |
| [`feedback_replan`](crates/research/src/glioma/programs/p05_mechanism_exploration/feedback_replan.rs) | GAF-GLIOMA-P05-F12 | Observation-driven mechanism frontier replanning for preclinical glioma research. | 2 | `closed_loop`, `evidence_assimilation` |
| [`fidelity_bridge`](crates/research/src/glioma/programs/p05_mechanism_exploration/fidelity_bridge.rs) | GAF-GLIOMA-P05-F06 | Cross-model fidelity bridging for preclinical glioma mechanism research. | 2 | — |
| [`graph_propagation`](crates/research/src/glioma/programs/p05_mechanism_exploration/graph_propagation.rs) | GAF-GLIOMA-P05-F10 | Signed mechanism-network propagation for preclinical glioma research. | 4 | — |
| [`identifiability`](crates/research/src/glioma/programs/p05_mechanism_exploration/identifiability.rs) | GAF-GLIOMA-P05-F02 | Mechanism identifiability frontier for preclinical glioma research. | 3 | — |
| [`instrument_campaign`](crates/research/src/glioma/programs/p05_mechanism_exploration/instrument_campaign.rs) | alias: `super::discrimination_campaign::FEATURE_ID` | Instrument-backed execution for glioma mechanism-discrimination campaigns. | 3 | `discrimination`, `discrimination_campaign`, `p08_instrument_robotics::calibration`, `p08_instrument_robotics::execution`, `p08_instrument_robotics::preflight` |
| [`intervention_value`](crates/research/src/glioma/programs/p05_mechanism_exploration/intervention_value.rs) | GAF-GLIOMA-P05-F04 | Posterior-weighted intervention value for preclinical glioma mechanism research. | 3 | — |
| [`invariance`](crates/research/src/glioma/programs/p05_mechanism_exploration/invariance.rs) | GAF-GLIOMA-P05-F03 | Cross-model mechanistic invariance frontier for preclinical glioma research. | 3 | — |
| [`mechanism_dynamics`](crates/research/src/glioma/programs/p05_mechanism_exploration/mechanism_dynamics.rs) | GAF-GLIOMA-P05-F26 | Bounded dynamical mechanism simulation for preclinical glioma research. | 3 | — |
| [`mechanism_workflow`](crates/research/src/glioma/programs/p05_mechanism_exploration/mechanism_workflow.rs) | GAF-GLIOMA-P05-F13 | Dependency-safe mechanism workflow compilation for preclinical glioma research. | 2 | `evidence_assimilation`, `feedback_replan` |
| [`morphodynamic_reducer`](crates/research/src/glioma/programs/p05_mechanism_exploration/morphodynamic_reducer.rs) | workflow composition | Built-in P03-to-P05 reduction for preclinical glioma invasion microscopy. | 2 | `discrimination`, `instrument_campaign`, `p03_multimodal_ingestion_qc::microscopy_morphodynamics`, `p08_instrument_robotics::execution` |
| [`multi_fidelity_control`](crates/research/src/glioma/programs/p05_mechanism_exploration/multi_fidelity_control.rs) | GAF-GLIOMA-P05-F11 | Multi-fidelity mechanism control for preclinical glioma research. | 2 | `bayesian_update`, `discrimination`, `evidence_assimilation`, `fidelity_bridge` |
| [`multi_study_workflow`](crates/research/src/glioma/programs/p05_mechanism_exploration/multi_study_workflow.rs) | GAF-GLIOMA-P05-F14 | Multimodal, multi-study mechanism workflow compilation for preclinical glioma research. | 2 | `mechanism_workflow` |
| [`operating_cycle`](crates/research/src/glioma/programs/p05_mechanism_exploration/operating_cycle.rs) | GAF-GLIOMA-P05-F24 | End-to-end mechanism-exploration orchestration for preclinical glioma research. | 3 | `action_planner`, `discrimination`, `discrimination_campaign` |
| [`pathway_activity`](crates/research/src/glioma/programs/p05_mechanism_exploration/pathway_activity.rs) | GAF-GLIOMA-P05-F29 | Signed pathway-activity inference for preclinical glioma research. | 4 | — |
| [`prospective_controller`](crates/research/src/glioma/programs/p05_mechanism_exploration/prospective_controller.rs) | GAF-GLIOMA-P05-F15 | Prospective high-throughput mechanism workflow control for preclinical glioma research. | 2 | `multi_study_workflow` |
| [`robust_portfolio`](crates/research/src/glioma/programs/p05_mechanism_exploration/robust_portfolio.rs) | GAF-GLIOMA-P05-F27 | Robust intervention portfolio optimisation with bounded beam/knapsack search for preclinical glioma research. | 3 | `counterfactual`, `ensemble_counterfactual` |
| [`robustness_stress`](crates/research/src/glioma/programs/p05_mechanism_exploration/robustness_stress.rs) | GAF-GLIOMA-P05-F07 | Adversarial robustness stress surfaces for preclinical glioma mechanisms. | 2 | — |
| [`state_filter`](crates/research/src/glioma/programs/p05_mechanism_exploration/state_filter.rs) | GAF-GLIOMA-P05-F16 | Longitudinal finite-state mechanism filtering for preclinical glioma research. | 3 | — |
| [`state_smoother`](crates/research/src/glioma/programs/p05_mechanism_exploration/state_smoother.rs) | GAF-GLIOMA-P05-F17 | Fixed-interval mechanism-state smoothing for preclinical glioma research. | 4 | `state_filter` |
| [`temporal_multimodal_fusion`](crates/research/src/glioma/programs/p05_mechanism_exploration/temporal_multimodal_fusion.rs) | workflow composition | Temporal and multimodal mechanism fusion for preclinical glioma research. | 2 | — |
| [`workflow_assurance`](crates/research/src/glioma/programs/p05_mechanism_exploration/workflow_assurance.rs) | GAF-GLIOMA-P05-F25 | Local single-study verification and safety assurance for glioma mechanism workflows. | 3 | `mechanism_workflow` |

Shared public feature modules owned outside this folder:

| Feature ID | Source module | Purpose | Direct test annotations |
|---|---|---|---:|
| `GAF-GLIOMA-P05-F01` | [`mechanism`](crates/research/src/glioma/mechanism.rs) | Competing-mechanism exploration for glioma research programs. | 2 |

---

### P06 — `p06_experiment_design`


- Consumer: experimentalist and assay allocation agent.
- Product contract: power-aware, blocked, sequential, adaptive, multi-fidelity, dose/synergy,
  clone-panel, replication, and mechanism-validation design.
- Primary artifacts: `ExecutableExperimentDesign`, allocation campaigns, power-stress surfaces,
  and validation protocols.
- Downstream edges: P07 simulation, P08 instrument preflight, P09 computation, and P10 replication.
- Promotion gate: estimand, randomization, controls, power uncertainty, null-result path, and
  resource/budget constraints are explicit before execution.
- P06-F29 `mechanism_validation` now exposes a canonical selected action order plus a disjoint
  deferred action order when the 512-action safety bound is reached. Power decisions, missing-arm
  remediation, missing observations, portfolio holds, and negative-result preservation therefore
  remain resumable rather than silently disappearing at the validation boundary; schema is
  `GliomaMechanismValidationPlan1@2`.
- P06-F17 `sequential_design` now exposes eligible assay arms omitted by the bounded arm-selection
  cap as a canonical `deferred_order`, disjoint from selected, risk-blocked, and budget-blocked
  partitions (`GliomaSequentialDesign1@2`). This keeps posterior exploration candidates routable
  to later interim rounds instead of conflating omission with scientific futility.
- The multi-objective frontier controller now performs posterior-aware sequential batch selection:
  after ranking information, power, risk, feasibility, fidelity, and replication value, each next
  assay receives a declared outcome-profile redundancy penalty against already selected assays.
  This favors complementary mechanism discrimination within a batch instead of spending the whole
  budget on assays with nearly identical likelihood signatures; the selected order and planner
  digest retain the exact decision path for replay. Maintain with assay-specific design plugins,
  prospective re-estimation benchmarks, and held-out batch-diversity evaluation.
- The heterogeneity-aware experiment portfolio planner now compiles investigator-declared assay
  candidates into a bounded, replication-reserved portfolio across model systems and biological
  strata. A deterministic beam jointly chooses replicate counts, cost, independence groups, risk
  ceilings, model-system diversity, and information utility; unavailable, unsafe, budget-deferred,
  underpowered, and partial arms remain explicit. The output includes declared/low/stress
  heterogeneity power surfaces, protected replication capacity, negative evidence, a content hash,
  and a non-dispatching boundary for P07/P08 handoff. It is a planning feature, not an assay
  executor or a clinical decision mechanism.
- Current implementation: 32/32 slots. Maintain with assay-specific design plugins and prospective
  re-estimation benchmarks.

P06-F25's adaptive multi-assay panel compiler now uses a bounded sequential-belief portfolio beam.
Each candidate updates a deterministic, probability-mass-bounded posterior branch set before the
next candidate is scored, so conditional information gain—not repeated single-assay gain—is used
for panel utility. Diversity, feasibility, risk, cost, and hard budget gates remain explicit;
`GliomaAdaptivePanelDesign1@2` records the conditional per-assay scores, final expected Gini, and
an uncertainty marker whenever the requested panel depth exceeds the bounded search horizon.
The compiler remains a planning capability: it never fabricates assay outcomes or dispatches an
instrument.

- P06-F13 `robust_active_learning` now uses deterministic inverse-uncertainty weighted median
  centers for repeated candidate observations. A single precise outlier cannot move the ensemble
  center, while the complete replicate spread remains a contradiction hold and unresolved output;
  the lower-tail, model-disagreement, and complementary-batch gates remain unchanged.
- P06-F02 `dose_response` now estimates each dose from equal-weight technical-batch means before
  isotonic fitting. The typed curve exposes batch count separately from observation count, making
  over-sampled processing runs unable to dominate a preclinical dose-response claim.
- P06-F03 `synergy` now equal-weights technical-batch means for vehicle, single-agent, and
  combination cells before Bliss calculation. Batch support is explicit in every cell, preventing
  an over-sampled processing run from manufacturing a synergy claim.
- P06-F10 `adaptive_allocation` now uses a bounded deterministic beam over arm inclusion and
  replicate counts under the declared budget and arm-count limits. This prevents a single expensive
  high-utility arm from consuming the entire batch when a complementary pair of lower-cost arms
  provides greater total posterior utility; budget blocks, partial allocations, and replay-stable
  tie breaks remain explicit and the output remains a non-dispatching planning artifact.
- P06-F04 `power_reestimation` now uses a bounded deterministic interim-allocation beam over
  partial replicate counts. Integer alpha spending and look-adjusted stopping boundaries are
  unchanged, while cost-aware information utility can preserve complementary low-cost follow-up
  arms instead of allowing one expensive arm to consume the declared budget; partial batches and
  budget blocks remain explicit in the plan and digest.
- P06-F12 `active_learning` now uses a bounded deterministic portfolio beam over eligible
  candidates. Joint acquisition utility is optimized under cost, selection, and one-per-redundancy-
  group constraints; risk blocks, replicate ceilings, uncertainty holds, budget blocks, and
  redundancy/selection deferrals remain explicit rather than being conflated with low score.
  The same feature now exposes `evaluate_glioma_active_learning`, which replays the policy against
  caller-supplied held-out candidate utilities and compares it with acquisition-greedy and fixed-
  coverage baselines plus a bounded oracle beam. It reports cost-adjusted selection utility and
  regret without feeding held-out truth into planning; the oracle is an evaluation upper bound,
  not a deployable predictor or biological claim.
- P06-F30 `clonal_panel` now uses a bounded sequential portfolio beam over clone-aware perturbation
  and readout candidates. It evaluates joint exact branch coverage, uncertainty-only matches,
  effect, cost, budget, and cardinality before selecting a panel, so a high-ratio single candidate
  cannot consume the budget when complementary branch assays provide stronger coverage. The
  `GliomaClonePerturbationPanel1@2` output preserves conditional gains, deferred candidates,
  uncovered branches, and bounded search uncertainty; it remains a planning artifact and never
  dispatches a perturbation.
- P06-F11 `campaign` now selects each same-round assay batch with a bounded complementarity beam.
  Predictive separation between candidate mechanism signatures is rewarded alongside information,
  effect, feasibility, risk, cost, and replicate ceilings, preventing a tied high-scoring duplicate
  assay from displacing an orthogonal assay. Posterior updates still occur only from returned local
  observations; `GliomaClosedLoopCampaign1@2` and
  `GliomaClosedLoopCampaignExecution1@2` preserve the replayable batch decision and execution
  boundary.

#### Implemented P06-F18 posterior-predictive assay acquisition

Extend `programs/p06_experiment_design/information_design.rs` with an explicit, selectable
posterior-predictive-diameter acquisition objective for finite, investigator-declared glioma
mechanism models. For each pair of mechanisms, compute their average total-variation distance over
the declared assay panel's categorical outcomes. For each candidate assay and each
non-zero prior-predictive outcome, update the finite posterior exactly in fixed-point arithmetic,
measure the remaining weighted pairwise predictive disagreement, and rank by its expected reduction.
The existing mechanism-Gini reduction remains available as a separate acquisition objective and
diagnostic; it must not be renamed as PDBAL or presented as equivalent to the published method.

- Inputs: positive finite mechanism priors; a finite declared panel of assay outcome distributions that each
  close to 1,000 milli per mechanism; explicit utility/risk/cost constraints; and a named acquisition
  objective.
- Outputs: both mechanism-Gini and panel predictive-diameter scores for every action; the objective
  actually used for ranking; deterministic selected/deferred actions; and the panel definition bound
  into the plan digest. Milli-unit truncation is declared on affected action scores and in plan
  uncertainty. The existing `InformationDesignRequest` and
  `plan_glioma_information_design` remain the mechanism-Gini-compatible entry point; new workflows
  opt in through `plan_glioma_information_design_with_objective`. The MCP `glioma_information_design`
  tool exposes the same choice as an optional `acquisition_objective` field, defaulting to Gini for
  existing callers.
- Migration: request fields and the original function contract are retained. Output schema advances
  from `GliomaInformationDesign1@1` to `@2` because score records now include predictive-diameter
  diagnostics and the canonical panel digest. Consumers that need the old ranking retain it by
  using the original function; consumers must version-gate the expanded output. The adaptive
  information campaign publishes schema `@2` and uses panel predictive diameter while retaining
  its existing minimum-gain request field as the threshold for the selected objective.
- Boundaries: this is a one-step finite categorical design score, not a continuous/noisy batch
  optimizer, causal-effect estimate, inferred assay response, or claim of superior scientific
  discovery. Null and impossible predictive outcomes remain distinct; fixed-point truncation is
  surfaced; complexity exhaustion is an explicit error, never a zero score.
- The adaptive campaign now fails closed when a returned categorical outcome has zero likelihood
  under every declared mechanism. Milli-unit smoothing may preserve a mechanism with a zero
  likelihood under one model, but it cannot manufacture an in-model posterior for an out-of-model
  outcome; the caller receives a typed recalibration error instead.
- Implementation validation: hand-computed asymmetric-panel scores, invariance to action/mechanism
  ordering, impossible-outcome handling, risk-ceiling enforcement, typed work-budget failure, and
  compatibility through the unchanged Gini entry point pass unit tests. The adaptive information
  campaign uses the predictive-diameter objective and its focused planner/executor tests pass.
- Remaining scientific acceptance: compare against fixed coverage, random feasible selection,
  mechanism-Gini, and appropriate posterior-predictive methods on source-unit-level held-out glioma
  model systems, then prospectively replicate. No comparative biological-performance claim is made.
  See Tosh et
  al., *Targeted active learning for probabilistic models* (2022), and Tosh et al., *A Bayesian active
  learning platform for scalable combination drug screens*, Nature Communications 16, 156 (2025).

#### P06-F19 P06 × P07 × P08 state-plasticity instrument vertical

The next engine increment is an executable preclinical glioma cell-state-plasticity campaign,
owned by `programs/p06_experiment_design/state_plasticity_instrument_campaign.rs`. It composes,
rather than duplicates, the existing P10 longitudinal transition analysis, P06 state-stratified
adaptive assay selector, P07 protocol feasibility simulation, and P08 instrument preflight and
execution.

- Inputs: one declared glioma model system; longitudinal specimen-level state observations; explicit
  control and perturbation arms; investigator-declared state priors and eligible single-use assay
  candidates; P07/P08 route for each candidate; local instrument gateway and assay-outcome
  interpreter.
- Loop: analyze transition rows at the independent source-unit level; derive visible state sampling
  priorities from support gaps, posterior uncertainty, and between-unit heterogeneity; screen each
  assay protocol with P07; let P06 choose the next eligible assay; revalidate and dispatch through
  P08; interpret only returned local artifacts; update only that state's posterior; and repeat until
  a typed stop condition or budget boundary.
- Outputs: transition analysis, state-priority decomposition, route readiness/block reasons,
  selected and completed assay rounds, retained null/negative or out-of-model outcomes, campaign
  stop reason, and an operator-readable next action. Partial work stays visible if a later route
  fails.
- Boundaries: no treatment or clinical recommendation; no causal interpretation from transition
  association; no inference from simulation-only artifacts; no synthesized negative result for a
  missing/failed readout; no physical execution without P08 authorization and interlocks; raw data
  remains institution-local.
- Algorithm status: state priorities are a transparent descriptive design heuristic, not a claim of
  optimal acquisition. The adaptive assay selector must be compared against fixed coverage, random
  feasible selection, conventional expected-information-gain selection, and posterior-predictive
  batch design (including PDBAL where its model assumptions fit). Any superiority claim requires
  held-out glioma model systems, source-unit-level splits, cost-adjusted discovery metrics, and
  prospective replication. See Tosh et al., *A Bayesian active learning platform for scalable
  combination drug screens*, Nature Communications 16, 156 (2025),
  https://doi.org/10.1038/s41467-024-55287-7.
- Acceptance: a deterministic multi-state fixture must prove exact candidate/route matching,
  minimum stratum coverage, P07 rejection before dispatch, P08 approval and interlock enforcement,
  per-state-only posterior updates, honest partial-stop behavior, and preservation of null and
  negative results. This is a P06-F19 vertical extension, not a new feature-slot claim.

#### P06-F19 extension: P10-F04 response-guided assay allocation

The response-guided entry point consumes a previously qualified P10-F02 propagation analysis and
an investigator-specified baseline state mixture, computes its P10-F04 net-yield/state-composition
decomposition, and uses each source state's outgoing response-follow-up needs to adjust that state's
P06 assay allocation. Component-validation focus contributes 1,000 attention points, independent
replication contributes 750, identifiability resolution contributes 1,000, and a resolved
within-margin component contributes zero; the source-state score is the mean across its destination
states. That score is combined with the existing P10 bootstrap and assay-batch follow-up attention
before exact positive-weight normalization. The run retains the full F04 decomposition and the
per-state response-attention audit, including an explicit absent value when this pathway was not
requested. It does not infer a baseline mixture or convert composition into individual-cell
switching.

The resulting P06 campaign still selects only declared assays. Every selected candidate must pass
P07 feasibility before P08 revalidates authorization, instrument state, and interlocks. A blocked
route remains a valid blocked campaign and causes no dispatch. Evaluation must include a
source-specific composition shift with an unaffected source as a discriminating allocation test,
an unresolved F04 component that elevates replication/identifiability follow-up without claiming a
positive result, baseline-mixture permutation and invalid-mixture refusal, P07 rejection before
dispatch, and P08 revocation/interlock failure. This is a P06-F19/P10-F04 composition and execution
edge, not an additional feature slot.

#### P06-F19 extension: lineage-propagation-guided assay selection

Extend the existing instrument campaign with the P10-F02 finite-interval lineage propagation
analysis. The workflow accepts local barcode-by-state count snapshots and their capture fractions,
fits P10-F02 before any P07/P08 operation, and uses the source-state-specific bootstrap uncertainty
in descendant yield and destination composition together with exact leave-one-assay-batch-out
refits. A fragile or unevaluable technical-batch sensitivity raises that state's next-assay
follow-up attention without changing the primary estimate. Within those priorities, P06 ranks the
*assay candidates themselves* by expected
posterior variance reduction in paired treatment-minus-control lineage-operator contrasts per
assay cost. Each candidate's calibrated categorical outcome likelihood is evaluated on every
paired independent-unit bootstrap draw; outcome posteriors are computed by deterministic integer
reweighting, so the acquisition step does not need a nested sampler. The P06 state priority is a
research allocation preference, while the candidate score estimates how much that specific assay
can resolve the declared propagation contrast: these are separate terms and neither substitutes
for the other.

The selector reports predicted outcome probabilities, prior and expected residual contrast
variance, both conditional and feasibility-adjusted expected reduction per cost, calibration-model
identity, and the minimum predicted effective-sample fraction across plausible outcomes. The
campaign ranks by expected realized information, multiplying the conditional reduction by the
candidate's declared probability of yielding an interpretable result; this prevents a low-success
assay from outranking a more reliable assay solely on its best-case information. P06 feasibility
values must be prospectively calibrated against completed local assays before this adjustment is
treated as empirically reliable. The scalar feasibility adjustment assumes assay success is
independent of the latent propagation draw conditional on the supplied response model. If failure
depends on state, operator, or assay outcome, success/failure must become an explicit calibrated
observation rather than a scalar discount. Candidates whose outcome model would leave less than 10% of
the bootstrap ensemble effective are withheld. After a measured result, the same likelihood
updates the run-local bootstrap weights before the next acquisition decision. A zero-probability
outcome or depleted effective sample size preserves the measured assay as evidence, blocks further
automated selection, and requests model recalibration or more independent-unit support. Callers
resuming a run must provide the aligned particle weights returned by the preceding run; an empty
weight vector explicitly means a uniform initial P10 bootstrap posterior.

That weight-only resume applies to the conditional-independence mode. Joint-response mode instead
replays the ordered assay history and requires any supplied posterior to match the replay exactly.

The selector also accepts an explicit destination-state estimand, with per-state 0..=1,000 weights
bound to the exact P10 state order. Weights define a normalized linear composite of the
treatment-minus-control destination effects; uncertainty is computed from that composite on each
paired bootstrap draw, retaining cross-destination covariance. This lets an investigator ask a
targeted glioma question such as whether a declared perturbation changes descendant composition
toward a mesenchymal-like state; zero-weight destinations do not contribute to the objective.
Existing callers retain the equal-weight average over all destinations. The live lineage-guided
campaign exposes this as an opt-in target-specific entrypoint and binds the target into the
replayable result.

An optional first-order joint-response mode uses a calibrated joint outcome distribution for every
unordered pair of single-use assay candidates, indexed by the same P10 bootstrap draws. The paired
calibrator accepts one record per independent local preclinical unit, fits a bounded joint
quantile-bin model, and evaluates it with leave-one-unit-out multiclass Brier loss against the
product of separately calibrated marginal models. Dependence is retained only when it beats that
conditional-independence baseline by the configured skill threshold. Iterative proportional fitting
followed by deterministic bipartite controlled rounding projects each joint table to the exact
single-assay marginals at 1,000-milli precision. Outside paired-data support, or when dependence
does not pass validation, the model uses the calibrated independent product instead of extrapolating
shared noise. The evidence digest binds the paired units and both assay contracts.

After the first assay, the next P06 score and posterior update use
`P(next outcome | P10 draw, previous assay, previous outcome)`, derived from the calibrated pair
distribution; they no longer multiply two marginal likelihoods as if shared measurement noise were
independent. This is explicitly a first-order Markov observation model, not an unrestricted
higher-order joint model. It is bounded to 64 actions and 32 outcome bins per action. A
joint-response resume requires ordered prior action/outcome history and verifies that replayed
history reproduces the supplied particle posterior.

The executable P06 campaign now selects an assay with a bounded two-assay adaptive rollout rather
than ranking every assay solely by immediate variance reduction. For each eligible first assay, it
enumerates the calibrated outcome bins, reweights the exact finite P10 particle posterior for each
branch, and scores the best affordable second assay under that branch's conditional response model.
It ranks the first choice by feasibility-adjusted expected treatment-minus-control operator
variance reduction over expected total assay cost when both budget and remaining campaign rounds
permit the second measurement; the first assay's cost remains charged even when
it produces no interpretable result, while an uninformative or low-effective-sample branch receives
no imagined follow-up value. In paired mode, a candidate pair without calibrated dependence support
receives no second-step credit. The horizon is exactly two assays, restricted to at most 64 eligible
single-use candidates and a 50,000,000 particle/state-operation estimate; exceeding those bounds
returns a typed request error before instrument dispatch instead of silently reverting to greedy
selection. The per-round result exposes whether the chosen priority used a one- or two-assay horizon.
After every stratum satisfies its declared minimum-coverage floor, the second assay may come from a
different glioma state: the P10 bootstrap draws are joint across transition-matrix columns, so the
first state's observed outcome can change the value of a follow-up assay in another state. During
coverage-floor enforcement, scoring remains one-step and the campaign may only dispatch from the
least-covered strata; speculative lookahead cannot bypass that rule. This global two-assay search is
still bounded by the same candidate and work limits. Its reported variance-reduction fraction uses
the equal-source-weight sum of target-weighted uncertainty across the full P10 transition operator,
so first- and second-assay gains from different source states share one denominator.
This finite-horizon score is a calibrated research-design heuristic, not a generally optimal policy
or a guarantee of improved scientific yield; prospective preclinical comparison remains required.

The acquisition-plan output remains `GliomaLineageAssayAcquisition1@5`; the state-stratified
campaign advances to `GliomaStateStratifiedCampaign1@4`, the simulation-gated campaign to
`GliomaSimulationGatedAssayCampaign1@9`, and the enclosing state-plasticity instrument output to
`GliomaStatePlasticityInstrumentCampaign1@12`. The latter version bumps record the newly global
cross-state rollout semantics. Older readers should reject or explicitly migrate these payloads
rather than silently interpreting the changed covariance-aware estimand, response-dependence, or
adaptive-selection semantics. The acquisition plan continues to expose each candidate's immediate
one-step score as a diagnostic; the executed two-assay priority and its horizon are recorded on the
selected campaign round.

- Matching gate: P10-F02 and P10-F14 must agree on preclinical model, control/treatment arms, and
  declared state order. P10-F02 must be qualified (full-rank source mixtures, converged interior
  fit, and accepted held-out forecasts). A mismatch, unresolved fit, prediction failure, or boundary
  estimate returns a typed refusal before the instrument gateway is called; the workflow never
  turns an unvalidated propagation estimate into an autonomous physical assay choice.
- Validation: synthetic operators with known treatment-control contrasts must rank a calibrated
  informative assay above an uninformative assay; cost must alter the ordering; posterior updates
  must be Bayesian, deterministic, and refusal-safe; low effective-sample outcomes must be reported
  unresolved without being mislabeled as impossible; candidate/input permutation must replay
  identically; a correlated duplicate assay must not create spurious posterior concentration; joint
  marginals, action history, and resumed posterior must cross-validate exactly; invalid/rank-deficient
  propagation must result in zero gateway calls; selected assays must still pass P07 simulation and
  P08 approval/interlock; adaptive rollout must prefer complementary over redundantly paired
  evidence across same-state and cross-state follow-ups, honor second-assay budget limits and
  least-covered-stratum constraints, and refuse above its bounded work budget. This is a P06-F19
  composition extension and creates no new feature slot.
- Scientific boundary: the acquisition objective is conditional on caller-provided calibration
  likelihoods and P10 bootstrap model adequacy; it is not a claim that calibrated outcome models or
  the P10 fit are biologically correct. The target is finite-interval descendant propagation,
  not direct cell-switch probabilities or clinical benefit. The finite-particle expected
  variance-reduction approach follows the sample-reweighting family of targeted Bayesian design
  (Vanlier et al., *A Bayesian approach to targeted experiment design*, Bioinformatics 28(8):
  1136–1142, 2012, doi:10.1093/bioinformatics/bts092); effective sample size is monitored because
  particle-weight degeneracy can make reweighted posteriors unreliable. This categorical finite-
  ensemble implementation is not a reproduction of that paper's continuous measurement model.

#### P06-F19 extension: empirical lineage-assay response calibration

The empirical calibration entry point turns preclinical observations into the per-bootstrap-draw
categorical likelihoods consumed above; callers no longer need to hand-author probabilities. Each
single-assay row represents one independent experimental unit and carries its measured outcome plus
the P10-compatible treatment-minus-control coefficient for one declared source→destination
glioma-state transition. Cell or barcode replicates remain nested within that unit. The calibrator
fits a bounded quantile-bin outcome model with a symmetric Dirichlet pseudocount and scores it using
true leave-one-independent-unit-out multiclass Brier loss against a prevalence-only baseline. It
requires at least 12 independent units, three units per marginal outcome, and supports up to 16
outcome classes and 16 bins.

Only a model that improves held-out Brier loss by the configured minimum relative margin is allowed
to express outcome likelihoods that vary across P10 draws. Otherwise, the generated model is
prevalence-only and has zero lineage-acquisition value. Draws outside the empirical calibration
contrast range also receive prevalence-only likelihoods rather than extrapolated tail probabilities.
Calibration outputs report held-out/model-versus-baseline scores, skill, outcome support, bin count,
and out-of-support draw count. Unit identity is unique and input-order permutation must reproduce
the same content digest.

The paired calibrator accepts two measured outcomes from the same unit on each row, producing the
full Cartesian joint outcome table. It independently cross-validates joint predictions against
the product of the single-assay models on the same held-out units. The marginal skill threshold and
the additional dependence-skill threshold are separate controls. Its output includes both
single-assay models and the exact-marginal joint model; this lets P06 consume a complete pair set
without hand-authored joint probabilities. Pair calibration still requires independent unit
records, versions the paired evidence digest, and reports minimum support for each joint category.
The resulting run schema is `GliomaLineageJointAssayResponseCalibration1@1`.

- Acceptance: synthetic monotone and null-skill datasets must respectively produce a predictive
  model and a prevalence-only fallback; leave-one-unit-out scoring must beat the baseline only when
  its predictive skill meets the threshold; duplicated units, unsupported outcomes, unresolved P10
  fits, and insufficient class counts must be refused; likelihoods must normalize for every draw;
  out-of-support particles must not receive edge-bin extrapolations; candidate-order and source-row
  permutations must be digest-stable; joint models must beat conditional independence on held-out
  units to retain dependence, preserve both single-assay marginals exactly, fall back below the
  dependence threshold, and drive the sequential P06→P07→P08→P10 campaign test. This is a P06-F19
  composition extension and creates no new feature slot.
- Scientific boundary: calibration is conditional on the validity of the unit-level contrast values,
  calibration assay identity, and transportability of the independent calibration units to the
  current preclinical workflow. It does not validate assay biology, establish causality, infer
  individual-cell switching, or justify clinical decisions. Probability evaluation follows
  multiclass calibration and proper-scoring-rule practice (Johansson et al., *Calibrating
  Multi-class Models*, Proceedings of the 10th Symposium on Conformal and Probabilistic Prediction
  and Applications, PMLR 152, 2021).

Folder inventory: 40 source modules; 32/32 feature slots implemented. The program folder directly owns 31 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`active_learning`](crates/research/src/glioma/programs/p06_experiment_design/active_learning.rs) | GAF-GLIOMA-P06-F12 | Mechanism-aware active learning for bounded preclinical glioma assays. | 7 | — |
| [`adaptive_allocation`](crates/research/src/glioma/programs/p06_experiment_design/adaptive_allocation.rs) | GAF-GLIOMA-P06-F10 | Sequential Bayesian assay allocation for preclinical glioma experiments. | 4 | — |
| [`adaptive_allocation_campaign`](crates/research/src/glioma/programs/p06_experiment_design/adaptive_allocation_campaign.rs) | GAF-GLIOMA-P06-F22 | Autonomous adaptive-allocation campaigns for preclinical glioma experiments. | 3 | `adaptive_allocation` |
| [`adaptive_dose_surface`](crates/research/src/glioma/programs/p06_experiment_design/adaptive_dose_surface.rs) | GAF-GLIOMA-P06-F31 | Uncertainty-aware adaptive dose-surface planning for preclinical glioma assays. | 3 | `synergy` |
| [`adaptive_information_campaign`](crates/research/src/glioma/programs/p06_experiment_design/adaptive_information_campaign.rs) | GAF-GLIOMA-P06-F19 | Adaptive information-design campaigns for preclinical glioma research. | 5 | `information_design` |
| [`adaptive_panel`](crates/research/src/glioma/programs/p06_experiment_design/adaptive_panel.rs) | GAF-GLIOMA-P06-F25 | Mechanism-aware adaptive assay-panel design for preclinical glioma research. | 4 | — |
| [`blocked_randomization`](crates/research/src/glioma/programs/p06_experiment_design/blocked_randomization.rs) | GAF-GLIOMA-P06-F09 | Confounding-aware blocked randomization for preclinical glioma experiments. | 2 | — |
| [`campaign`](crates/research/src/glioma/programs/p06_experiment_design/campaign.rs) | GAF-GLIOMA-P06-F11 | Mechanism-aware closed-loop campaign planning for preclinical glioma research. | 7 | — |
| [`carryover_sequence`](crates/research/src/glioma/programs/p06_experiment_design/carryover_sequence.rs) | GAF-GLIOMA-P06-F15 | Carryover-aware assay sequence design for preclinical glioma workflows. | 2 | — |
| [`clonal_panel`](crates/research/src/glioma/programs/p06_experiment_design/clonal_panel.rs) | GAF-GLIOMA-P06-F30 | Clone-aware preclinical perturbation-panel design. | 5 | — |
| [`contrast_design`](crates/research/src/glioma/programs/p06_experiment_design/contrast_design.rs) | GAF-GLIOMA-P06-F24 | Deterministic multi-factor contrast-panel design for preclinical glioma studies. | 3 | — |
| [`dose_response`](crates/research/src/glioma/programs/p06_experiment_design/dose_response.rs) | GAF-GLIOMA-P06-F02 | Fixed-point dose-response analysis for preclinical glioma experiments. | 5 | — |
| [`frontier_controller`](crates/research/src/glioma/programs/p06_experiment_design/frontier_controller.rs) | GAF-GLIOMA-P06-F08 | Closed-loop multi-objective experiment frontier control for preclinical glioma research. | 5 | — |
| [`heterogeneity_aware_portfolio`](crates/research/src/glioma/programs/p06_experiment_design/heterogeneity_aware_portfolio.rs) | workflow composition | Heterogeneity-aware experiment portfolio allocation for preclinical glioma research. | 3 | — |
| [`information_design`](crates/research/src/glioma/programs/p06_experiment_design/information_design.rs) | GAF-GLIOMA-P06-F18 | Integer-only Bayesian information design with beam-selected assay panels for preclinical glioma assays. | 7 | — |
| [`lineage_acquisition_design`](crates/research/src/glioma/programs/p06_experiment_design/lineage_acquisition_design.rs) | alias: `super::adaptive_information_campaign::FEATURE_ID` | Candidate-specific expected reduction in uncertainty of glioma lineage-propagation operators. | 10 | `adaptive_information_campaign`, `p10_interpretation_replication::lineage_propagation`, `state_stratified_campaign` |
| [`lineage_guided_campaign`](crates/research/src/glioma/programs/p06_experiment_design/lineage_guided_campaign.rs) | workflow composition | P10 lineage analysis driving P06 state-specific adaptive assay execution. | 4 | `adaptive_information_campaign`, `p08_instrument_robotics`, `p10_interpretation_replication::lineage_dynamics`, `simulation_gated_campaign`, `state_stratified_campaign` |
| [`lineage_response_calibration`](crates/research/src/glioma/programs/p06_experiment_design/lineage_response_calibration.rs) | alias: `super::lineage_acquisition_design::FEATURE_ID` | Cross-validated calibration of glioma assay outcomes against lineage-propagation contrasts. | 2 | `lineage_acquisition_design`, `p10_interpretation_replication::lineage_propagation`, `state_stratified_campaign` |
| [`mechanism_validation`](crates/research/src/glioma/programs/p06_experiment_design/mechanism_validation.rs) | GAF-GLIOMA-P06-F29 | Mechanism-to-validation compiler for preclinical glioma research. | 2 | `p05_mechanism_exploration::counterfactual`, `p05_mechanism_exploration::ensemble_counterfactual`, `p05_mechanism_exploration::graph_propagation`, `p05_mechanism_exploration::robust_portfolio`, `power_reestimation` |
| [`mechanism_validation_protocol`](crates/research/src/glioma/programs/p06_experiment_design/mechanism_validation_protocol.rs) | GAF-GLIOMA-P06-F16 | Compile a mechanism-validation decision into a deterministic local protocol. | 2 | `mechanism_validation`, `p05_mechanism_exploration::counterfactual`, `p05_mechanism_exploration::ensemble_counterfactual`, `p05_mechanism_exploration::graph_propagation`, `p05_mechanism_exploration::robust_portfolio`, `power_reestimation` |
| [`multi_fidelity`](crates/research/src/glioma/programs/p06_experiment_design/multi_fidelity.rs) | GAF-GLIOMA-P06-F20 | Cost-aware multi-fidelity optimization for preclinical glioma experiments. | 3 | — |
| [`multi_fidelity_campaign`](crates/research/src/glioma/programs/p06_experiment_design/multi_fidelity_campaign.rs) | GAF-GLIOMA-P06-F21 | Closed-loop multi-fidelity intervention campaigns for preclinical glioma research. | 3 | `multi_fidelity` |
| [`operating_cycle`](crates/research/src/glioma/programs/p06_experiment_design/operating_cycle.rs) | GAF-GLIOMA-P06-F23 | End-to-end experiment-design orchestration for preclinical glioma research. | 2 | `campaign` |
| [`posterior_batch`](crates/research/src/glioma/programs/p06_experiment_design/posterior_batch.rs) | workflow composition | Batch-conditional selection over externally supplied glioma posterior draws. | 7 | `active_learning` |
| [`power_reestimation`](crates/research/src/glioma/programs/p06_experiment_design/power_reestimation.rs) | GAF-GLIOMA-P06-F04 | Adaptive power re-estimation and group-sequential boundaries for preclinical glioma assays. | 5 | — |
| [`power_stress_surface`](crates/research/src/glioma/programs/p06_experiment_design/power_stress_surface.rs) | GAF-GLIOMA-P06-F14 | Scenario stress surface for preclinical glioma power planning. | 2 | — |
| [`replication_continuation`](crates/research/src/glioma/programs/p06_experiment_design/replication_continuation.rs) | GAF-GLIOMA-P06-F27 | Observation-driven continuation control for multi-site preclinical glioma replication. | 2 | `replication_plan` |
| [`replication_plan`](crates/research/src/glioma/programs/p06_experiment_design/replication_plan.rs) | GAF-GLIOMA-P06-F26 | Multi-site replication topology planning for preclinical glioma experiments. | 3 | — |
| [`replication_protocol`](crates/research/src/glioma/programs/p06_experiment_design/replication_protocol.rs) | GAF-GLIOMA-P06-F28 | Compile a replication continuation decision into an executable local protocol. | 2 | `replication_continuation`, `replication_plan` |
| [`robust_active_learning`](crates/research/src/glioma/programs/p06_experiment_design/robust_active_learning.rs) | GAF-GLIOMA-P06-F13 | Robust, model-ensemble active learning with bounded portfolio selection for preclinical glioma assays. | 5 | `active_learning` |
| [`robust_design`](crates/research/src/glioma/programs/p06_experiment_design/robust_design.rs) | GAF-GLIOMA-P06-F07 | Robust scenario-aware allocation for preclinical glioma experiment design. | 2 | — |
| [`sequential_campaign`](crates/research/src/glioma/programs/p06_experiment_design/sequential_campaign.rs) | GAF-GLIOMA-P06-F32 | Closed-loop execution for sequential preclinical glioma experiment design. | 3 | `sequential_design` |
| [`sequential_design`](crates/research/src/glioma/programs/p06_experiment_design/sequential_design.rs) | GAF-GLIOMA-P06-F17 | Sequential Bayesian stopping and allocation for preclinical glioma assays. | 6 | — |
| [`simulation_gated_campaign`](crates/research/src/glioma/programs/p06_experiment_design/simulation_gated_campaign.rs) | workflow composition | Adaptive glioma assay campaigns connected to P07 protocol simulation and P08 execution. | 8 | `adaptive_information_campaign`, `information_design`, `lineage_acquisition_design`, `lineage_response_calibration`, `p08_instrument_robotics::calibration`, `p08_instrument_robotics::execution`, `p08_instrument_robotics::preflight`, `p08_instrument_robotics::protocol_binding`, `p10_interpretation_replication::lineage_propagation`, `p10_interpretation_replication::lineage_response_decomposition`, `p10_interpretation_replication::state_transition`, `state_stratified_campaign` |
| [`state_plasticity_instrument_campaign`](crates/research/src/glioma/programs/p06_experiment_design/state_plasticity_instrument_campaign.rs) | alias: `super::adaptive_information_campaign::FEATURE_ID` | Closed-loop cell-state plasticity campaigns connecting glioma analysis to real assay execution. | 6 | `adaptive_information_campaign`, `lineage_acquisition_design`, `p10_interpretation_replication::lineage_propagation`, `p10_interpretation_replication::lineage_response_decomposition`, `p10_interpretation_replication::state_transition`, `simulation_gated_campaign`, `state_stratified_campaign`, `transition_guided_campaign` |
| [`state_stratified_campaign`](crates/research/src/glioma/programs/p06_experiment_design/state_stratified_campaign.rs) | workflow composition | State- and clone-stratified adaptive assay campaigns for preclinical glioma research. | 8 | `adaptive_information_campaign`, `information_design` |
| [`synergy`](crates/research/src/glioma/programs/p06_experiment_design/synergy.rs) | GAF-GLIOMA-P06-F03 | Fixed-point combination-response and Bliss synergy analysis for preclinical glioma assays. | 3 | — |
| [`transition_guided_campaign`](crates/research/src/glioma/programs/p06_experiment_design/transition_guided_campaign.rs) | alias: `super::adaptive_information_campaign::FEATURE_ID` | Transition-uncertainty-guided adaptive assay selection for preclinical glioma studies. | 7 | `adaptive_information_campaign`, `p10_interpretation_replication::state_transition`, `state_stratified_campaign` |
| [`validation_batch_assessment`](crates/research/src/glioma/programs/p06_experiment_design/validation_batch_assessment.rs) | GAF-GLIOMA-P06-F05 | Close the loop from an executed validation batch to the next power-aware decision. | 2 | `p07_protocol_simulation`, `p07_protocol_simulation::execution`, `p07_protocol_simulation::mechanism_validation_execution`, `p07_protocol_simulation::simulator`, `power_reestimation` |
| [`validation_campaign`](crates/research/src/glioma/programs/p06_experiment_design/validation_campaign.rs) | GAF-GLIOMA-P06-F06 | Closed-loop orchestration for glioma mechanism validation. | 1 | `mechanism_validation`, `mechanism_validation_protocol`, `p05_mechanism_exploration::counterfactual`, `p05_mechanism_exploration::ensemble_counterfactual`, `p05_mechanism_exploration::graph_propagation`, `p05_mechanism_exploration::robust_portfolio`, `p07_protocol_simulation`, `p07_protocol_simulation::execution`, `p07_protocol_simulation::mechanism_validation_execution`, `p07_protocol_simulation::simulator`, `power_reestimation`, `validation_batch_assessment` |

Shared public feature modules owned outside this folder:

| Feature ID | Source module | Purpose | Direct test annotations |
|---|---|---|---:|
| `GAF-GLIOMA-P06-F01` | [`experiment`](crates/research/src/glioma/experiment.rs) | Power-aware preclinical experiment design. | 2 |

---

### P07 — `p07_protocol_simulation`


- Consumer: lab operations lead and research director.
- Product contract: resource-feasible protocol simulation, branch optimization, compensation,
  autonomous campaign scheduling, claim-prioritized evidence-to-action selection, and multi-program
  mission execution planning.
- Primary artifacts: `ProtocolSimulationReport`, branch plans, campaign state, and mission plans.
- Downstream edges: P08 signed instrument preflight, P09 computation placement, P11 release, and
  P12 benchmark runs.
- Promotion gate: no physical effect before simulation, interlocks, compensation, resource budget,
  and deterministic replay pass.
- Current implementation: 32/32 slots. Maintain with queue contention, fault injection, and
  long-horizon campaign worlds.

P07-F11 admits a stage checkpoint only from a complete typed result (`Completed`) or a complete,
explicit negative scientific result (`Negative`). A `Partial` artifact is retained in that cycle's
execution outcome and uncertainty, but cannot satisfy a prerequisite or enter downstream worker
context. The engine reports current unresolved/deferred action IDs separately from completed stage
checkpoints; successful actions from earlier cycles cannot remain listed as current work. This keeps
an honestly measured negative result usable while preventing an incomplete output from unlocking
later glioma workflows. The engine also performs deterministic outcome-aware reweighting between
cycles: checkpoint progress increases downstream-unlock and feasibility value, while uncertainty,
negative results, or stagnation increase information, reproducibility, and conservative feasibility
value. The adaptation trace and final weights are content-addressed with the run so a consortium
can replay why the next glioma action batch changed.
In addition, every returned action outcome updates exact-action and stage/modality/model cohort
posteriors. The next director invocation applies a bounded empirical-Bayes utility adjustment,
records the adjusted action IDs and rationale, and leaves dependencies, effects, autonomy, and
artifact locality unchanged. Failed, partial, skipped, and negative outcomes therefore change
what is investigated next without being converted into biological conclusions or silently erased.
Within each bounded beam portfolio, a deterministic stage-diversity bonus breaks near-ties across
complementary protocol stages while preserving dependency safety and materially stronger expected
gain. The selected stage mix and all deferred alternatives remain digest-bound for replay.
The selector's diversity context now includes completed candidate metadata when it is available in
the current candidate registry. Completed actions remain in the blocked history, while their
modality/model and stage counts lower marginal utility for redundant follow-up; this makes resumed
missions history-aware without exporting payloads or changing dependency and autonomy gates.
When a provider returns the same partial frontier again after that adaptive reweighting, the engine
uses a deterministic stagnation brake: it stops with `NoProgress`/`NoRunnableActions` before burning
the remaining cycle budget, while retaining the partial result, uncertainty, and resume checkpoints.
This makes a failed or under-specified local adapter recoverable without allowing an autonomous
worker to retry an unchanged action indefinitely. The stop is a workflow state, not a scientific
negative claim; a caller can resume after supplying a repaired executor or new typed artifact.
Continuation also carries the run's value-only exact-action and cohort outcome summaries into the
next bounded budget, so a repaired executor benefits from prior failure/negative evidence without
re-ingesting local payloads or pretending that a prior partial result was successful.
It also carries the run's final adaptive selection weights, preventing a continuation from
silently resetting the learned policy and replaying the same weak frontier after a negative or
uncertain cycle.
P07-F23 `active_learning_campaign` now charges every candidate invocation before dispatch,
including retry and terminal-failure attempts. A retry that cannot fit the remaining assay budget
is not sent to the executor; its failed candidate and explicit budget uncertainty remain in the
round, and the versioned output is `GliomaActiveLearningCampaign1@2` so downstream planners cannot
silently interpret legacy success-only accounting as measured spend.
P07-F24 `robust_active_learning_campaign` applies the same invocation-level accounting to
model-disagreement-aware candidate rounds. Resource exhaustion is uncertainty, never negative
scientific evidence; an unfundable retry is withheld before dispatch, and the versioned output
`GliomaRobustActiveLearningCampaign1@2` preserves exact spend and retry semantics for continuation.
The mission frontier now also performs a deterministic transitive dependency walk over the active
candidate graph. Actions receive bounded leverage for the downstream assays and analyses they
unlock, while follow-ups connected to an action that returned uncertainty receive targeted
information and reproducibility priority. This prevents a shallow high-score action from starving
the branch that can actually close an unresolved glioma mechanism, without changing permissions,
autonomy tiers, or prerequisite gates.
The mechanism-specific autopilot now consumes the action portfolio's measured invocation spend,
including retry attempts, so a transient local worker failure cannot create unreported budget
headroom or cause a resumed mechanism campaign to exceed its declared envelope.
P07 action-portfolio execution now charges each actual worker invocation against the selected
portfolio budget, including retry attempts, and exposes `budget_spent_units` in
`GliomaActionPortfolioExecution1@2`. If a retry would exceed the remaining envelope, it is not
launched: the action is failed with explicit budget uncertainty, downstream actions are skipped,
and the autonomous engine stops as budget-exhausted. Engine-cycle spend is derived from returned
attempt counts rather than the nominal selection, so skipped work is not billed and a continuation
can use the truthful remaining budget after a transient worker failure.
The cross-program mission controller applies the same invariant at its round boundary: each
`GliomaMissionRound.cost_units` must equal the embedded portfolio's measured
`budget_spent_units`, including retries and excluding skipped actions. Validation rejects a stale
or hand-edited round that reports nominal spend, keeping resumable glioma missions from inventing
budget headroom during recovery.

The engine now has an evaluation-only policy harness alongside execution. It compiles the exact
P07 action frontier, holds the planner blind to caller-supplied held-out utilities, and compares
the AURORA beam policy with score-greedy and stage-coverage baselines plus a bounded
dependency-aware oracle. Utility, cost, regret, plan digest, negative held-out outcomes, and
uncertainty are replayable; this gives a research lead a measurable way to improve autonomous
glioma workflow selection before enabling an institution-local worker, without mistaking a policy
benchmark for a biological observation.
The same harness now accepts a bounded, canonically ordered set of named held-out scenarios. It
returns per-scenario evaluation and truth digests plus policy mean, lower-quartile, worst-case,
regret-to-oracle, and selection-stability metrics. Scenario stress results remain evaluation-only:
they do not enter planning, are not biological evidence, and explicitly disclose that set overlap
is not biological reproducibility. Negative mean or worst-case utility is retained as first-class
evidence for policy revision rather than hidden by an aggregate score.
The trace evaluator now replays the actual bounded engine loop—not just a static action score—over
named synthetic provider-outcome worlds. It compares all seven closed focus policies on completed
stage progress, worst-case progress, qualification rate, budget spent, negative-result retention,
and failed/skipped-work burden. Missing action outcomes fail closed, and every run digest remains
available for deterministic replay; these worlds are evaluation inputs, never biological evidence.
The director also accepts an `Adaptive` focus. It inspects only the currently dependency-closed,
ready stage frontier and deterministically resolves that frontier to evidence, mechanism,
experiment, computation, replication, or full-program work. The resolved focus is emitted in the
director and engine artifacts, while a `director:auto-focus:*` uncertainty marker records that the
choice was made from readiness rather than asserted biological evidence. This removes a manual
research-strategy knob without allowing the director to invent a stage, bypass a prerequisite, or
hide an unresolved branch.
When several branches are simultaneously ready, the resolver scores each target with the caller's
selection weights plus a bounded downstream-unlock bonus and excludes already completed stages;
the engine therefore prefers the branch with the greatest declared scientific leverage rather than
using a permanent mechanism-first ordering.
When a resumed run carries value-only negative, partial, or failed outcomes for an upstream
scientific arm, adaptive focus also promotes a dependency-closed replication branch; evidence
pressure instead promotes evidence repair. This is bounded outcome pressure over typed stage IDs,
not a biological conclusion: the original branch remains available for falsification and all
payloads stay in the institution-local artifact store.
P07 also compiles `GliomaStageWorkerRoute1@1` from bounded institution-local worker profiles.
Every stage in the 14-stage graph receives an explicit route: selected, not-ready, or blocked for
missing capability. Selection requires declared stage kind, output schema, autonomy, locality,
availability, modality, model-system, and optional determinism compatibility; priority, deterministic
preference, and worker-ID ties are stable. `GliomaStageExecutorRegistry` dispatches only through
those admitted typed workers and fails closed when a route or worker is absent. The MCP
`glioma_stage_worker_routes_compile` operation exposes this admission plan without invoking a
worker, moving data, or authorizing an assay; real execution remains with the caller-owned local
worker and P08/P09 safety gates.
The route is now executable rather than advisory: `execute_glioma_autonomous_research_engine_with_stage_workers`
binds the plan digest, wraps the registry through the typed stage/action adapter, and returns a
route-plus-engine artifact with the same checkpoints, retry accounting, negative evidence, and
budget stops used by the autonomous loop. The MCP rehearsal
`glioma_autonomous_research_engine_stage_execute` exercises that path with deterministic
schema-correct synthetic workers; it is explicitly simulation-only, while institution hosts may
replace those workers through the Rust trait seam.
The next composition gate is `execute_glioma_evidence_gated_stage_engine`. It joins the P01
cross-family triangulation digest to the same routed P07 engine: unresolved, partial, negative,
contradictory, or insufficient evidence returns a typed evidence hold before route compilation;
ready stages without an eligible local capability return a route hold; only qualified evidence
with complete deterministic stage coverage can invoke a worker. The output carries the evidence
partition, route plan, engine checkpoints, negative results, uncertainty, next actions, and a
single replay digest. `glioma_evidence_gated_stage_engine_execute` is the simulation-only MCP
rehearsal; production hosts inject institution-local workers through the typed Rust API.
The unified `execute_glioma_autonomous_research_workflow_dry_run` seam now composes that
evidence/admission gate with the existing P08 instrument-preflight, P09 reproducible-computation,
P10 interpretation/replication, P11 research-object release, and P12 federated-benchmark operating
cycles. A single typed workflow request can carry the domain cycle payloads; omitted payloads use
deterministic synthetic workers and remain marked simulation-only. The MCP
`glioma_autonomous_research_workflow_execute` surface returns the complete staged execution while
preserving negative, partial, contradictory, unresolved, held, blocked, locality, retry, and
budget outcomes. It never contacts instruments, moves raw data, publishes/signs objects, exports
federation data, or makes a clinical decision.

The `heterogeneity_portfolio_mission` bridge closes the P06-to-P07 handoff for autonomous glioma
research. It binds each selected biological portfolio arm to a typed `GliomaActionCandidate`, checks
that dependencies are either selected or already completed, and then runs the P07 adaptive beam
over authority, risk, budget, outcome history, and dependency closure. Missing bindings, unresolved
dependencies, underpowered portfolios, and blocked arms become replayable route holds instead of
being silently dropped. A ready or partial mission is still a plan: evidence admission, protocol
simulation, instrument preflight, and local execution remain separate downstream gates.

Folder inventory: 42 source modules; 32/32 feature slots implemented. The program folder directly owns 31 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`action_execution`](crates/research/src/glioma/programs/p07_protocol_simulation/action_execution.rs) | GAF-GLIOMA-P07-F19 | Execution of a beam-selected preclinical glioma action portfolio. | 5 | — |
| [`active_learning_campaign`](crates/research/src/glioma/programs/p07_protocol_simulation/active_learning_campaign.rs) | GAF-GLIOMA-P07-F23 | Autonomous active-learning campaign execution for preclinical glioma research. | 5 | `p06_experiment_design::active_learning` |
| [`adaptive_scheduler`](crates/research/src/glioma/programs/p07_protocol_simulation/adaptive_scheduler.rs) | GAF-GLIOMA-P07-F12 | Evidence-aware, dependency-closed scheduling for autonomous preclinical glioma programs. | 5 | — |
| [`adaptive_scientific_mission`](crates/research/src/glioma/programs/p07_protocol_simulation/adaptive_scientific_mission.rs) | alias: `super::mission::FEATURE_ID` | Replanned, evidence-driven execution for preclinical glioma research missions. | 2 | `action_execution`, `frontier_execution`, `mission`, `scientific_frontier` |
| [`autonomous_campaign`](crates/research/src/glioma/programs/p07_protocol_simulation/autonomous_campaign.rs) | GAF-GLIOMA-P07-F20 | Autonomous, closed-loop execution of a preclinical glioma research campaign. | 3 | `action_execution` |
| [`autonomous_engine`](crates/research/src/glioma/programs/p07_protocol_simulation/autonomous_engine.rs) | GAF-GLIOMA-P07-F11 | End-to-end autonomous research engine for preclinical glioma programs. | 11 | `action_execution`, `director` |
| [`autonomous_protocol`](crates/research/src/glioma/programs/p07_protocol_simulation/autonomous_protocol.rs) | GAF-GLIOMA-P07-F09 | Bounded autonomous control for one preclinical glioma protocol mission. | 3 | `branch_optimizer`, `compensation`, `execution`, `simulator` |
| [`autonomous_workflow`](crates/research/src/glioma/programs/p07_protocol_simulation/autonomous_workflow.rs) | workflow composition | Unified autonomous glioma workflow composition. | 0 | `evidence_gated_stage_execution` |
| [`branch_optimizer`](crates/research/src/glioma/programs/p07_protocol_simulation/branch_optimizer.rs) | GAF-GLIOMA-P07-F04 | Deterministic beam-search selection of resource-feasible protocol branches. | 3 | `simulator` |
| [`clone_campaign`](crates/research/src/glioma/programs/p07_protocol_simulation/clone_campaign.rs) | GAF-GLIOMA-P07-F18 | Adaptive evolutionary clone campaign for preclinical glioma research. | 2 | `clone_continuation` |
| [`clone_continuation`](crates/research/src/glioma/programs/p07_protocol_simulation/clone_continuation.rs) | GAF-GLIOMA-P07-F28 | Clone-outcome-driven continuation planning for the autonomous preclinical workflow. | 4 | — |
| [`compensation`](crates/research/src/glioma/programs/p07_protocol_simulation/compensation.rs) | GAF-GLIOMA-P07-F03 | Dependency-aware compensation planning after a preclinical protocol run. | 3 | `execution`, `simulator` |
| [`cross_model_replication_mission`](crates/research/src/glioma/programs/p07_protocol_simulation/cross_model_replication_mission.rs) | workflow composition | P10-to-P07 autonomous cross-model replication mission. | 3 | `action_execution`, `adaptive_scheduler`, `p10_interpretation_replication::cross_model_claim_envelope`, `p10_interpretation_replication::cross_model_replication_frontier` |
| [`director`](crates/research/src/glioma/programs/p07_protocol_simulation/director.rs) | GAF-GLIOMA-P07-F32 | High-level autonomous direction for preclinical glioma research. | 5 | `action_execution` |
| [`engine_evaluation`](crates/research/src/glioma/programs/p07_protocol_simulation/engine_evaluation.rs) | alias: `super::autonomous_engine::FEATURE_ID` | Held-out evaluation for the autonomous glioma research engine. | 4 | `action_execution`, `autonomous_engine`, `director` |
| [`evidence_campaign`](crates/research/src/glioma/programs/p07_protocol_simulation/evidence_campaign.rs) | GAF-GLIOMA-P07-F22 | Execute the bounded work selected by the P01 evidence-priority queue. | 4 | `action_execution` |
| [`evidence_gate`](crates/research/src/glioma/programs/p07_protocol_simulation/evidence_gate.rs) | GAF-GLIOMA-P07-F26 | Evidence-gated orchestration for autonomous preclinical glioma research. | 2 | `action_execution`, `director` |
| [`evidence_gated_stage_execution`](crates/research/src/glioma/programs/p07_protocol_simulation/evidence_gated_stage_execution.rs) | workflow composition | Evidence-admitted autonomous stage execution for preclinical glioma research. | 2 | `autonomous_engine`, `director`, `stage_worker_registry` |
| [`evidence_surface`](crates/research/src/glioma/programs/p07_protocol_simulation/evidence_surface.rs) | GAF-GLIOMA-P07-F05 | Quality- and uncertainty-aware evidence compilation from a local protocol run. | 3 | `execution`, `simulator` |
| [`execution`](crates/research/src/glioma/programs/p07_protocol_simulation/execution.rs) | GAF-GLIOMA-P07-F10 | Guarded execution of a feasible preclinical glioma protocol. | 3 | `simulator` |
| [`frontier_execution`](crates/research/src/glioma/programs/p07_protocol_simulation/frontier_execution.rs) | GAF-GLIOMA-P07-F08 | Execute an admitted scientific-frontier batch for preclinical glioma research. | 4 | `action_execution`, `scientific_frontier` |
| [`heterogeneity_portfolio_mission`](crates/research/src/glioma/programs/p07_protocol_simulation/heterogeneity_portfolio_mission.rs) | workflow composition | P06-to-P07 mission compilation for autonomous glioma research. | 2 | `adaptive_scheduler` |
| [`intent_mission`](crates/research/src/glioma/programs/p07_protocol_simulation/intent_mission.rs) | GAF-GLIOMA-P07-F13 | Intent-to-mission compilation for autonomous preclinical glioma research. | 2 | `action_execution`, `mission`, `mission_recovery` |
| [`mechanism_autopilot`](crates/research/src/glioma/programs/p07_protocol_simulation/mechanism_autopilot.rs) | GAF-GLIOMA-P07-F30 | Closed-loop multimodal mechanism autopilot for preclinical glioma research. | 7 | `action_execution`, `mechanism_campaign`, `p03_multimodal_ingestion_qc`, `p05_mechanism_exploration` |
| [`mechanism_campaign`](crates/research/src/glioma/programs/p07_protocol_simulation/mechanism_campaign.rs) | GAF-GLIOMA-P07-F29 | End-to-end multimodal mechanism campaign for preclinical glioma research. | 2 | `action_execution`, `p03_multimodal_ingestion_qc`, `p05_mechanism_exploration` |
| [`mechanism_discovery_engine`](crates/research/src/glioma/programs/p07_protocol_simulation/mechanism_discovery_engine.rs) | GAF-GLIOMA-P07-F16 | Autonomous mechanism-discovery engine for preclinical glioma research. | 3 | `action_execution`, `mechanism_campaign`, `p03_multimodal_ingestion_qc`, `p05_mechanism_exploration` |
| [`mechanism_validation_execution`](crates/research/src/glioma/programs/p07_protocol_simulation/mechanism_validation_execution.rs) | workflow composition | Execute a preflighted glioma mechanism-validation protocol through a local worker. | 2 | `execution`, `p06_experiment_design`, `simulator` |
| [`mission`](crates/research/src/glioma/programs/p07_protocol_simulation/mission.rs) | GAF-GLIOMA-P07-F31 | Science-aware autonomous mission control for preclinical glioma research. | 9 | `action_execution` |
| [`mission_recovery`](crates/research/src/glioma/programs/p07_protocol_simulation/mission_recovery.rs) | GAF-GLIOMA-P07-F27 | Failure-aware recovery for autonomous preclinical glioma missions. | 2 | `action_execution`, `mission` |
| [`multimodal_mission`](crates/research/src/glioma/programs/p07_protocol_simulation/multimodal_mission.rs) | GAF-GLIOMA-P07-F14 | Multimodal/model-system portfolio expansion for autonomous glioma missions. | 2 | `action_execution`, `intent_mission`, `mission`, `mission_recovery` |
| [`multistudy_fusion`](crates/research/src/glioma/programs/p07_protocol_simulation/multistudy_fusion.rs) | GAF-GLIOMA-P07-F06 | Cross-study fusion of typed glioma protocol evidence surfaces. | 3 | `evidence_surface` |
| [`posterior_batch_campaign`](crates/research/src/glioma/programs/p07_protocol_simulation/posterior_batch_campaign.rs) | alias: `super::active_learning_campaign::FEATURE_ID` | Closed-loop posterior-guided assay execution for preclinical glioma research. | 2 | `active_learning_campaign`, `p06_experiment_design` |
| [`program_cycle`](crates/research/src/glioma/programs/p07_protocol_simulation/program_cycle.rs) | GAF-GLIOMA-P07-F17 | Program-level control for autonomous preclinical glioma research. | 2 | `action_execution`, `autonomous_engine`, `director` |
| [`program_scheduler`](crates/research/src/glioma/programs/p07_protocol_simulation/program_scheduler.rs) | GAF-GLIOMA-P07-F15 | High-throughput autonomous scheduling for preclinical glioma research programs. | 4 | `action_execution`, `director` |
| [`research_autopilot`](crates/research/src/glioma/programs/p07_protocol_simulation/research_autopilot.rs) | GAF-GLIOMA-P07-F21 | Evidence-to-execution autopilot for local preclinical glioma research. | 3 | `action_execution` |
| [`robust_active_learning_campaign`](crates/research/src/glioma/programs/p07_protocol_simulation/robust_active_learning_campaign.rs) | GAF-GLIOMA-P07-F24 | Autonomous execution loop for robust ensemble active learning. | 4 | `p06_experiment_design::active_learning`, `p06_experiment_design::robust_active_learning` |
| [`scenario_ensemble`](crates/research/src/glioma/programs/p07_protocol_simulation/scenario_ensemble.rs) | workflow composition | Robust scenario-ensemble simulation for autonomous preclinical glioma workflows. | 2 | `simulator` |
| [`scientific_frontier`](crates/research/src/glioma/programs/p07_protocol_simulation/scientific_frontier.rs) | GAF-GLIOMA-P07-F25 | Scientific frontier orchestration for autonomous preclinical glioma research. | 3 | — |
| [`simulator`](crates/research/src/glioma/programs/p07_protocol_simulation/simulator.rs) | GAF-GLIOMA-P07-F02 | Deterministic resource-constrained protocol simulation for preclinical glioma campaigns. | 4 | — |
| [`stage_executor_adapter`](crates/research/src/glioma/programs/p07_protocol_simulation/stage_executor_adapter.rs) | workflow composition | Bridge adaptive P07 actions to the typed glioma stage execution contract. | 4 | `action_execution`, `director` |
| [`stage_worker_registry`](crates/research/src/glioma/programs/p07_protocol_simulation/stage_worker_registry.rs) | workflow composition | Capability-aware routing for institution-local glioma stage workers. | 4 | `autonomous_engine`, `director`, `stage_executor_adapter` |
| [`transport_gate`](crates/research/src/glioma/programs/p07_protocol_simulation/transport_gate.rs) | GAF-GLIOMA-P07-F07 | Evidence-to-workflow transport gate for preclinical glioma research. | 2 | `evidence_surface`, `multistudy_fusion` |

Shared public feature modules owned outside this folder:

| Feature ID | Source module | Purpose | Direct test annotations |
|---|---|---|---:|
| `GAF-GLIOMA-P07-F01` | [`workflow`](crates/research/src/glioma/workflow.rs) | Adaptive workflow planning for autonomous preclinical glioma research. | 5 |

---

### P08 — `p08_instrument_robotics`


- Consumer: instrument operator and institution-local gateway.
- Product contract: calibration, signal extraction, batch stability, multichannel concordance,
  fleet scheduling, signed preflight, human authorization, and emergency-stop handling.
- Primary artifacts: `InstrumentPreflight`, instrument plans, fleet campaigns, and assay evidence.
- Downstream edges: P03 QC, P07 protocol state, P09 computation, and P11 research-object release.
- Promotion gate: A3 physical execution requires signed preflight, interlocks, revocation checks,
  local-only raw data, and honest partial-execution compensation.
- Preflight plans now bind a canonical digest of every admitted action, including operation,
  timing, parameters, output schema, model system, and risk. The execution gateway recomputes the
  manifest before dispatch and refuses mutated actions even when IDs, instrument scope, and model
  system still match; the plan schema is versioned so this is an explicit compatibility boundary.
- Current implementation: 32/32 slots. The adaptive microscopy loop computes fixed-point
  expected information gain from a local glioma-state posterior and calibrated image-outcome model,
  enforces invasion-front/state coverage before resampling, estimates conservative instrument-dose
  cost from typed illumination settings, and executes one capture through existing authorization
  and live-interlock checks. Its research-workflow entry point now derives state posteriors from P03
  segmented-track morphodynamics, requires biological-unit-held-out qualification, blocks
  low-quality/out-of-domain/ambiguous fields, and refuses to execute without caller-owned
  preflight grants. Posterior confidence and margin thresholds are explicit in the P03 request
  and bound into the versioned analysis digest, so an autonomous loop cannot silently turn a
  near-tie into an action. A quality/information-qualified field on the coverage frontier that
  exceeds the remaining dose is reported as `dose_budget_exhausted`, not as missing biological
  coverage, so the autonomous loop can request budget review without sampling an over-covered
  stratum.
  Human-origin material is excluded. Maintain with prospective comparison against fixed-grid,
  equal-dose sampling across independent non-human preclinical model systems.
- The adaptive instrument campaign now uses a bounded deterministic beam over complete
  dependency-closed portfolios, not only the downstream root. Prerequisites contribute their
  information, endpoint coverage, duration, and risk to the portfolio objective, while endpoint
  diversity remains primary and a smaller instrument-novelty term spreads near-equal work across
  a trusted fleet. The selector compares alternative whole portfolios before applying explicit
  information, endpoint, cost, risk, and dependency gates, and ranks portfolios that satisfy the
  scientific information/endpoint floors ahead of higher-scoring but gate-infeasible portfolios.
  Its versioned output also exposes a complete `deferred_order` complement so bounded execution
  can be resumed when capacity, approvals, or endpoint gaps change without losing candidates.
- P08-F25 `research_frontier` now uses a bounded portfolio beam over adjudicated instrument
  outcomes. Evidence-state completeness (qualified, negative, unresolved), independent source-run
  coverage, and frontier utility are optimized jointly; `GliomaInstrumentResearchFrontier1@2`
  preserves null and unresolved routes before handing the selected candidates to P07, and never
  promotes hardware completion into biological evidence.
- P08-F30 `fleet_health_monitor` is an aggregate-only instrument reliability algorithm. It compares
  baseline and recent throughput/QC/calibration windows, detects consecutive downtime clusters,
  calibrates confidence from effective observations, and emits healthy/watch/investigate/blocked
  partitions plus bounded site-local investigation tasks. Site identity can be masked and the
  monitor never exports raw traces, dispatches hardware, or turns a single noisy metric into a
  hard stop.
- P08-F31 `acquisition_capacity_controller` is the prospective high-throughput allocator. It uses
  a bounded allocation beam rather than one-unit greedy fill, jointly scoring minimum-demand
  closure, minimum fairness, weighted target coverage, priority, deadline urgency, resource cost,
  and global/per-resource budgets. Completion horizon, weighted Jain fairness, disabled resources,
  approval gaps, starvation deferrals, and search-bound uncertainty are explicit; the plan remains
  preflight-bound and cannot dispatch hardware (`GliomaAcquisitionCapacityPlan1@2`).
- P08-F28 `cross_site_protocol_conformance` compares a digest-bound reference protocol with
  site-local semantic realizations before any federated pooling. Ordered step identity, semantic
  role, units, numeric tolerance, required capabilities, calibration class/freshness, and version
  identity are checked independently; bounded adaptations may be admitted only under policy, while
  stale or blocked sites remain excluded with named negative evidence.
- P08-F29 `maintenance_window_manager` searches the earliest reservation-free service interval for
  each instrument before calibration expiry. Overdue service, low health, disabled devices, and
  fleets with no safe gap are explicit locks; the plan feeds capacity and fleet scheduling but
  never mutates bookings or contacts hardware.
- P08-F27 `assay_provenance_integrity_audit` is the prospective admission barrier between local
  acquisition and downstream glioma analysis. It separately checks sample lineage and scope,
  approved versus observed protocol, calibration expiry, operator authority, clock skew, lifecycle
  completion, and artifact-chain continuity. Only a fully verified run enters P03/P09 analysis or
  verified research-object release; warnings, blocks, unresolved lifecycle state, and negative
  evidence remain explicit and digest-bound.
- P08-F19 `high_throughput_acquisition_console` is the read-only operations surface for autonomous
  screening campaigns. It projects assignment finish times and deadline risk, counts preflight and
  calibration blockers, identifies stale device telemetry and saturated operator capacity, and
  ranks approved ready demand with a deterministic priority/urgency/fairness score. Proposals are
  advisory only: they cannot rewrite signed plans, bypass approval, or dispatch an instrument. The
  versioned snapshot now returns the complete ready-demand candidate order plus a canonical
  `deferred_reorder_order` complement whenever the proposal cap truncates the advisory queue, so
  later capacity rounds can resume without silently dropping work.
- P08-F17 `instrument_operator_approval_console` is the physical-effect authorization boundary.
  A single-use approval is bound to the exact plan digest, device, opaque sample scope, operator
  authority, expiry, interlock observations, uncertainty budget, and emergency-stop path. Revoked,
  expired, consumed, changed, failed, stale, or unmeasured conditions never become dispatchable;
  the MCP surface only returns a simulation-safe approval decision for the local gateway.
- P08-F08 `federated_device_capability_manifest` publishes the scheduler-facing device contract.
  Capability claims, protocol versions, availability windows, calibration validity, policy digest,
  revocation, expiry, and explicit secret/raw-identifier exclusion proofs are content-bound before
  a site can be considered schedulable. The federated payload is metadata-only; stale, unavailable,
  revoked, tampered, or locality-unsafe declarations remain visible as negative operational state.
- P08-F32 `federated_instrument_operations` exchanges the consortium scheduling aggregate. Each
  site summary is signature-checked and filtered by membership revocation, freshness, privacy floor,
  locality/credential exclusion, and exchange policy before service and available capacity are
  summed. Site-level capacity is not exported, reconstruction-risk is reported, and a quorum is
  required before the aggregate can inform a federated campaign.
- P08-F09 `phase_resolved_invasion_schedule` adds a separate experiment-design capability: it
  ranks already device-validated time-lapse profiles using conservative reporter-phase dwell times,
  independently measured motility persistence/localization error, reporter/tracking quality, and
  calibrated total-dose limits. A macro-average/worst-phase fixed-point objective avoids letting a
  common long phase conceal a missed short phase. This returns a schedule proposal—not biological
  evidence or an autonomous hardware command—and routes the selected profile through the existing
  P08 protocol compiler and signed preflight. The question is grounded in conflicting preclinical
  observations: some organoid invasion assays report low proliferation markers in invasive cells,
  while cell-cycle-resolved time-lapse work reports highly motile G2/M subpopulations. That conflict
  is a reason to measure phase and movement together, not to assume a “go-and-grow” mechanism.
  Its action-plan compiler expands the selected, version-pinned cadence into bounded P08 image
  actions; each remains operator-gated and must pass the ordinary protocol binding, qualified
  calibration, live interlocks, and guarded gateway path. A 48-world held-out synthetic trajectory
  suite tests phase coverage and motility recovery against a fixed-cadence baseline at equal total
  dose; passing this software benchmark is not prospective biological validation.
  References: da Silva et al., *Spontaneous Glioblastoma Spheroid Infiltration of Early-Stage
  Cerebral Organoids Models Brain Tumor Invasion* (2018),
  https://journals.sagepub.com/doi/10.1177/2472555218764623; Akhunbay-Fudge et al., *Glioblastoma
  invasion into different organoid hosts reveals cell-intrinsic and proliferative migratory
  programs* (2026), https://pmc.ncbi.nlm.nih.gov/articles/PMC13059114/. The score is a sampling
  proxy; release requires comparison to fixed-cadence and equal-dose baselines on held-out
  non-human preclinical trajectories, including reporter dropout, unequal phase durations, and
  phototoxicity/track-quality shifts. No novelty claim is considered proven by these references.

P08-F07 `assay_run_schema` is the typed handoff between an admitted local run and downstream
ingestion/computation. It binds an opaque non-human sample token, approved protocol digest,
qualified calibration, exact preflight digest, acquisition cadence, channel schemas, and local
storage policy into a content-addressed `AssayRunSpec`. `record_glioma_assay_run_result` requires
one explicit state per channel and emits `Completed`, `Negative`, `Partial`, `Blocked`, or
`Unresolved` without imputing missing measurements. P03/P09 consumers can therefore distinguish
measured image artifacts from transport-only, failed, dropped, or unresolved channels. Human-origin
material, direct identifiers, non-local raw data, model mismatch, stale calibration, and incomplete
preflight are rejected before dispatch. The contract is Rust/serde-compatible and preserves
simulation-only execution as non-biological evidence.

P08-F13 `instrument_protocol_compiler` turns a typed, version-pinned assay protocol into an
ordered `InstrumentAction` plan before preflight or gateway binding. It matches each requested
operation/output schema against the local device capability manifest, performs exact fixed-point
dimensional conversion (for example seconds to milliseconds and milliliters to microliters),
checks device ranges and required parameters, and emits expected local artifacts, a proof-linked
preflight checklist, and explicit compensation actions. Unsupported commands, dimensional
mismatches, overlapping schedules, and risk/duration-policy violations remain visible in an
unsupported-step report and force `Unsupported` or `Blocked`; they never disappear or become
dispatchable. Golden, unit-mismatch, unsupported-command, and budget fixtures establish
deterministic compilation and fail-closed behavior. The compiled plan still requires P08-F02
calibration, P08 preflight, authorization, protocol binding, interlocks, and the institution-local
gateway before physical execution.

P08-F14 `synchronized_multimodal_acquisition` executes the useful shared-timeline portion of a
multimodal glioma assay. It accepts an investigator-declared set of imaging, molecular, and
functional captures, checks that each instrument has a validity-bounded clock calibration, converts
global assay ticks into device ticks with integer drift correction, and rejects overlapping or
over-budget captures before dispatch. A caller-owned institution gateway executes captures one at a
time; every result must carry a local artifact, observed timing, and a quality value. The engine
computes corrected global starts and bundle skew, preserves failed/negative/unresolved/blocked
modalities, and emits a bundle artifact only when all required modalities pass their quality and
skew gates. A failed device can therefore never make a multimodal run appear complete. Dry-run
execution is explicitly simulation-only and cannot be interpreted as biological evidence. The
acceptance suite covers deterministic ordering, calibrated clock conversion, late-device skew,
required-device failure, and instrument overlap blocking. Raw signals and sample payloads remain
institution-local; this is an acquisition/orchestration capability, not a clinical or biological
classifier.
P08-F14 admission now uses a bounded interval/risk portfolio beam rather than first-fit timeline
greediness. Required channels dominate optional channels when they conflict on one instrument;
among equally complete required portfolios, the planner maximizes admitted channel coverage, then
prefers lower total risk with replay-stable ties. Every displaced channel remains blocked and is
reported in the uncertainty partition, so an optional early capture cannot silently starve a
required later modality.
The autonomous-stage bridge now lets the P07 engine invoke this P08 operating cycle as its typed
`instrument-preflight` worker. It runs the existing preflight barrier, authorization and revocation
checks, live-interlock recheck, bounded campaign, retry accounting, and emergency-stop path; the
returned stage artifact embeds the operating-cycle outcome while explicitly setting
`biological_evidence_promoted=false` and requiring assay-evidence adjudication. A dry-run
constructor is available for sandbox/MCP rehearsal, while production hosts inject their own
institution-owned `InstrumentExecutor` through the same stage-worker trait.

Folder inventory: 33 source modules; 32/32 feature slots implemented. The program folder directly owns 32 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`acquisition_capacity_controller`](crates/research/src/glioma/programs/p08_instrument_robotics/acquisition_capacity_controller.rs) | GAF-GLIOMA-P08-F31 | Prospective acquisition-capacity allocation for high-throughput preclinical glioma studies. | 4 | — |
| [`adaptive_campaign`](crates/research/src/glioma/programs/p08_instrument_robotics/adaptive_campaign.rs) | GAF-GLIOMA-P08-F15 | Beam-selected information-aware instrument campaign selection for preclinical glioma research. | 5 | `campaign`, `execution` |
| [`adaptive_microscopy`](crates/research/src/glioma/programs/p08_instrument_robotics/adaptive_microscopy.rs) | GAF-GLIOMA-P08-F26 | Dose-aware, uncertainty-driven adaptive microscopy for preclinical glioma models. | 6 | `calibration`, `execution`, `p03_multimodal_ingestion_qc::microscopy_morphodynamics`, `preflight` |
| [`assay_adjudication`](crates/research/src/glioma/programs/p08_instrument_robotics/assay_adjudication.rs) | GAF-GLIOMA-P08-F20 | Evidence adjudication after guarded preclinical instrument execution. | 3 | `execution` |
| [`assay_provenance_integrity_audit`](crates/research/src/glioma/programs/p08_instrument_robotics/assay_provenance_integrity_audit.rs) | GAF-GLIOMA-P08-F27 | Prospective assay provenance integrity auditing for preclinical glioma workflows. | 5 | — |
| [`assay_run_schema`](crates/research/src/glioma/programs/p08_instrument_robotics/assay_run_schema.rs) | GAF-GLIOMA-P08-F07 | Typed assay-run and instrument-result contracts for preclinical glioma workflows. | 4 | `calibration`, `execution`, `preflight` |
| [`autonomous_stage_bridge`](crates/research/src/glioma/programs/p08_instrument_robotics/autonomous_stage_bridge.rs) | workflow composition | P07 autonomous-engine bridge for the governed P08 instrument operating cycle. | 1 | `calibration`, `campaign`, `execution`, `operating_cycle`, `preflight` |
| [`batch_stability`](crates/research/src/glioma/programs/p08_instrument_robotics/batch_stability.rs) | GAF-GLIOMA-P08-F03 | High-throughput cross-run endpoint stability for preclinical glioma instruments. | 2 | `signal_extraction` |
| [`calibration`](crates/research/src/glioma/programs/p08_instrument_robotics/calibration.rs) | GAF-GLIOMA-P08-F02 | Robust instrument-control calibration and drift detection for preclinical glioma workflows. | 3 | — |
| [`campaign`](crates/research/src/glioma/programs/p08_instrument_robotics/campaign.rs) | GAF-GLIOMA-P08-F12 | Safety-aware multi-run instrument campaigns for preclinical glioma workflows. | 2 | `execution` |
| [`cross_site_protocol_conformance`](crates/research/src/glioma/programs/p08_instrument_robotics/cross_site_protocol_conformance.rs) | GAF-GLIOMA-P08-F28 | Cross-site protocol conformance for federated preclinical glioma acquisition. | 4 | — |
| [`execution`](crates/research/src/glioma/programs/p08_instrument_robotics/execution.rs) | GAF-GLIOMA-P08-F11 | Guarded execution of an admitted preclinical glioma instrument plan. | 5 | `preflight` |
| [`federated_consensus`](crates/research/src/glioma/programs/p08_instrument_robotics/federated_consensus.rs) | GAF-GLIOMA-P08-F04 | Federated aggregate-only instrument endpoint consensus for preclinical glioma research. | 2 | — |
| [`federated_device_capability_manifest`](crates/research/src/glioma/programs/p08_instrument_robotics/federated_device_capability_manifest.rs) | GAF-GLIOMA-P08-F08 | Federated device capability manifests for preclinical glioma research. | 5 | — |
| [`federated_instrument_operations`](crates/research/src/glioma/programs/p08_instrument_robotics/federated_instrument_operations.rs) | GAF-GLIOMA-P08-F32 | Aggregate-only federated instrument operations for preclinical glioma research. | 5 | — |
| [`fleet_execution`](crates/research/src/glioma/programs/p08_instrument_robotics/fleet_execution.rs) | GAF-GLIOMA-P08-F21 | Schedule-aware execution of a preclinical glioma instrument fleet. | 2 | `execution`, `fleet_scheduler` |
| [`fleet_health_monitor`](crates/research/src/glioma/programs/p08_instrument_robotics/fleet_health_monitor.rs) | GAF-GLIOMA-P08-F30 | Robust multi-instrument health monitoring for preclinical glioma acquisition. | 4 | — |
| [`fleet_scheduler`](crates/research/src/glioma/programs/p08_instrument_robotics/fleet_scheduler.rs) | GAF-GLIOMA-P08-F18 | Deterministic fleet scheduling for preclinical glioma instrument workflows. | 5 | `preflight` |
| [`high_throughput_acquisition_console`](crates/research/src/glioma/programs/p08_instrument_robotics/high_throughput_acquisition_console.rs) | GAF-GLIOMA-P08-F19 | Read-only high-throughput acquisition operations for preclinical glioma campaigns. | 5 | — |
| [`instrument_operator_approval_console`](crates/research/src/glioma/programs/p08_instrument_robotics/instrument_operator_approval_console.rs) | GAF-GLIOMA-P08-F17 | Single-use human authorization for preclinical glioma instrument actions. | 4 | — |
| [`instrument_protocol_compiler`](crates/research/src/glioma/programs/p08_instrument_robotics/instrument_protocol_compiler.rs) | GAF-GLIOMA-P08-F13 | Compile validated preclinical assay protocols into typed instrument actions. | 4 | `assay_run_schema`, `preflight`, `protocol_binding` |
| [`maintenance_window_manager`](crates/research/src/glioma/programs/p08_instrument_robotics/maintenance_window_manager.rs) | GAF-GLIOMA-P08-F29 | Deterministic maintenance-window planning for preclinical glioma instrument fleets. | 4 | — |
| [`multichannel_concordance`](crates/research/src/glioma/programs/p08_instrument_robotics/multichannel_concordance.rs) | GAF-GLIOMA-P08-F05 | Local multichannel temporal alignment and concordance for preclinical glioma instruments. | 2 | — |
| [`operating_cycle`](crates/research/src/glioma/programs/p08_instrument_robotics/operating_cycle.rs) | GAF-GLIOMA-P08-F24 | Preflight-barrier and campaign orchestration for local glioma instruments. | 1 | `campaign`, `execution`, `preflight` |
| [`phase_resolved_invasion_schedule`](crates/research/src/glioma/programs/p08_instrument_robotics/phase_resolved_invasion_schedule.rs) | GAF-GLIOMA-P08-F09 | Designs time-lapse sampling for phase-resolved preclinical glioma invasion assays. | 9 | `calibration`, `execution`, `preflight` |
| [`preflight`](crates/research/src/glioma/programs/p08_instrument_robotics/preflight.rs) | GAF-GLIOMA-P08-F10 | Deterministic instrument and robotics preflight for preclinical glioma protocols. | 5 | `calibration` |
| [`protocol_binding`](crates/research/src/glioma/programs/p08_instrument_robotics/protocol_binding.rs) | GAF-GLIOMA-P08-F22 | Bind approved glioma instrument actions to a version-pinned local device protocol profile. | 5 | `execution`, `preflight` |
| [`recovery`](crates/research/src/glioma/programs/p08_instrument_robotics/recovery.rs) | GAF-GLIOMA-P08-F06 | Deterministic recovery planning for institution-local glioma instrument runs. | 2 | `execution` |
| [`research_frontier`](crates/research/src/glioma/programs/p08_instrument_robotics/research_frontier.rs) | GAF-GLIOMA-P08-F25 | Status-aware instrument-result to autonomous-research-frontier compilation. | 3 | `assay_adjudication`, `science_loop` |
| [`science_loop`](crates/research/src/glioma/programs/p08_instrument_robotics/science_loop.rs) | GAF-GLIOMA-P08-F16 | Governed instrument-to-science loop for preclinical glioma assays. | 2 | `assay_adjudication`, `execution`, `operating_cycle` |
| [`signal_extraction`](crates/research/src/glioma/programs/p08_instrument_robotics/signal_extraction.rs) | GAF-GLIOMA-P08-F01 | Robust local instrument-signal extraction for preclinical glioma workflows. | 3 | — |
| [`simulated_protocol_workflow`](crates/research/src/glioma/programs/p08_instrument_robotics/simulated_protocol_workflow.rs) | GAF-GLIOMA-P08-F23 | P07-simulation-gated execution of one instrument-backed glioma protocol slice (`GAF-GLIOMA-P08-F23`), not an end-to-end executor for every P07 task. | 6 | `calibration`, `execution`, `preflight`, `protocol_binding` |
| [`synchronized_multimodal_acquisition`](crates/research/src/glioma/programs/p08_instrument_robotics/synchronized_multimodal_acquisition.rs) | GAF-GLIOMA-P08-F14 | Synchronized multimodal acquisition for preclinical glioma assays. | 5 | — |

---

### P09 — `p09_reproducible_computation`


- Consumer: computational scientist and workflow runtime.
- Product contract: checkpointed multimodal DAGs, resource placement, replay, recovery,
  robustness-guided computation, and interpretation-frontier compilation.
- Primary artifacts: `ComputationRun`, replay tapes, robustness suites, and computation portfolios.
- Downstream edges: P10 interpretation, P11 release, and P12 federated benchmark aggregation.
- Promotion gate: byte-stable canonicalization, resource termination, crash/retry recovery,
  negative-result retention, and independent replay.
- P09-F05 — `workflow_execution_manifest` wraps the computation DAG with pinned tools, typed task
  contracts, local resource bindings, retry policy, expected outputs, and an explicit unsupported-
  step list. It rejects undeclared effects, non-local resources, unpinned tools, and graph coverage
  gaps before a scheduler sees the plan. Resource-shortfall and unsupported states remain blocked,
  while a ready manifest can be handed to the existing caller-owned P09 executor.
- P09-F08 — `partial_result_semantics` is the computation-to-interpretation missingness contract.
  It requires explicit field observations and separates measured values, measured nulls, censored,
  interrupted, failed, unavailable, redacted, and invalid states. It derives conservative gates for
  descriptive summaries, model fitting, mechanism inference, and publication, so budget-censored
  or missing fields cannot become measured zeroes. The bundle is replay-bound and carries negative
  evidence and a recovery/review handoff.
- P09-F07 — `artifact_lineage_index` verifies content-addressed derivation edges and computes
  deterministic output-to-root proofs. Exact, transformed, sampled, and semantic-loss relations
  remain distinct; tampered digests, unauthorized nodes, cycles, missing parents, and orphan
  artifacts become explicit gaps rather than being hidden by a successful task status.
- P09-F17 — `computation_run_inspector` is the researcher-facing run observability contract. It
  reconciles immutable task outcomes with ordered telemetry, resource totals, stale windows,
  lineage status, and partial-result limitations, then emits a deterministic recovery issue bundle.
  Missing terminal events and disposition mismatches remain unresolved instead of being shown as
  success.
- P09-F19 — `high_throughput_compute_timeline` turns those run inspections into a campaign-level
  operations product. It separates successful, failed, incomplete, and unresolved task volume;
  ranks queue latency, compute duration, retry burden, resource saturation, and stale telemetry;
  retains scientific scope and run links; and evaluates throughput forecasts only against an
  explicit held-out run partition. Failed work cannot inflate capacity, and an uncalibrated
  forecast remains a partial state rather than an operational claim.
- P09-F26 — `interpretation_frontier` now retains the complete computation-derived action universe
  and uses a bounded stage-diverse utility/cost beam for the executable subset. Deferred actions
  remain canonical, routable, and visible alongside replay, negative-result, and recovery gates in
  `GliomaComputationInterpretationFrontier1@2`; no task disappears before P07 admission.
- The campaign controller now carries value-only `ComputationOutcomeSummary` posteriors into every
  planner round and into selective recovery. Completed, cached, negative, partial, failed, and
  skipped task outcomes remain separately countable with retry burden and an explicit smoothed
  success estimate; raw artifacts never leave the institution-local executor. This lets an
  autonomous planner prioritize validation or robustness work from observed computation behavior
  without turning a failed or missing result into a success claim.
- P09-F11 — `computation_portfolio_planner` now admits required analyses first, then uses a
  deterministic bounded beam over complete prerequisite closures. Whole portfolios are compared
  by information, uncertainty reduction, coverage, modality and redundancy diversity, cost, and
  duration rather than by a single local score. This makes a tight compute envelope prefer
  genuinely complementary glioma evidence while preserving typed DAG closure, resource gates,
  deterministic replay, and explicit deferral of lower-coverage work. Signed utility arithmetic
  clamps negative optional-task utility before portfolio scoring, so a resource-heavy low-value
  analysis cannot win through signed-to-unsigned conversion.
- P09-F10 — `execution` charges the declared estimated cost and duration for every worker
  invocation, including retry attempts, while replay-cache hits remain zero-cost. A retry is
  admitted only when its full envelope still fits the remaining budget; otherwise the task is
  recorded as failed with an explicit budget-stop uncertainty and dependents are skipped. This
  prevents transient failures from silently exceeding compute limits or making partial runs look
  cheaper than the work actually attempted. Retry counts represent dispatched retries only, and
  `GliomaComputationExecution1@2` makes that accounting contract explicit. Structured provider
  failures returned through the typed result channel are terminal immediately, with a task-failed
  stop reason rather than being misclassified as a dependency-only block.
- The autonomous-stage bridge composes P09-F24 with P07's routed engine. A dedicated
  `computational-execution` worker now invokes workflow compilation, the declared resource gate,
  the replay-keyed computation campaign, bounded retries, and the caller-owned local executor.
  Its `GliomaComputationRun1@1` stage artifact records the full operating-cycle outcome while
  explicitly setting `biological_evidence_promoted=false`; completed computation remains subject
  to statistical-interpretation and reproducibility adjudication. The MCP rehearsal uses only the
  deterministic local executor, while institution hosts can inject a governed executor through
  `GliomaComputationExecutor`.
- The `computation_placement_stress_evaluate` composition evaluates that placement policy under
  worker loss, transfer-cost inflation, and contracted compute windows. It replays the exact typed
  DAG and compares the proposed multi-worker schedule with a constrained fastest-single-worker
  baseline, reporting assigned coverage, makespan, transfer cost, non-degradation evidence, and
  scenario-level uncertainty. This gives the autonomous engine an operational promotion gate
  before local execution without treating scheduling performance as biological evidence.
- Current implementation: 32/32 slots. The phase-resolved imaging handoff now consumes only
  completed, local, de-identified P08 capture artifacts and compiles them into the standard P09
  ingest-to-validate/export DAG. It preserves negative, partial, and unresolved acquisition as
  blocked handoff states; dry-run captures are explicitly simulation-only and cannot be read as
  biological measurements. Maintain with placement optimization, artifact lineage joins, and
  adaptive robustness-guided recomputation.

P09-F06 `execution_environment_lock` is the autonomous computation admission layer. It converts
workflow identity, an institution architecture profile, and signed trusted package metadata into
an exact content-addressed environment/build identity. OS, compiler, libraries, models, runtimes,
and accelerators carry resolved versions, source/build digests, ABI, availability, and portability
limits. Mutable or compromised sources block before dispatch; unavailable, incompatible, untrusted,
and non-portable dependencies remain explicit. Permutation, compromise, version, architecture,
optional-dependency, and human/clinical-boundary tests are included.

P09-F09 `environment_resolution_agent` closes the environment recovery loop. It evaluates
trusted, signed, content-addressed dependency and hardware candidates against the existing lock
request, refuses poisoned or mutable sources, enforces change/cost budgets, and preserves pinned
scientific versions and sources unless an explicit approval is present. A qualified proposal
contains the exact resulting lock; rejected candidates, unresolved conflicts, and approval debt
remain visible for an operator or the next autonomous cycle. Candidate-order, missing-dependency,
silent-upgrade, poisoned-source, and deterministic-proposal tests are included.

P09-F21 `reproducible_task_api` is the typed handoff between autonomous planning and an
institution-local computation worker. It binds a task's replay identity, canonical input schemas,
authorized local artifacts, qualified environment lock, expiring policy grant, idempotency key,
and cost/duration budget into a replayable execution handle. Duplicate submissions reuse the exact
handle; conflicting key reuse, unauthorized artifacts, expired grants, and over-budget work fail
before dispatch. The exchange returns explicit not-started/partial/failure state and never moves
raw data or executes a worker in the core.

P09-F16 `federated_workflow_template_exchange` is the consortium-facing workflow product.
It packages a deterministic, local-only task/schema/effect contract with a qualified environment
identity and a held-out validation card, then admits only signed aggregate site attestations under
an expiry-bounded sharing policy. Two independent conformant sites can publish a portability claim
and explicit revalidation-required adaptations; failed, revoked, missing, or stale evidence remains
visible and blocks portability. Inputs, credentials, and raw outputs never leave their institution.

P09-F22 `artifact_registry_connector` is the data-plane admission boundary for autonomous
computation. It resolves handles by exact content hash, schema, license, approved registry source,
signed metadata, availability, freshness, locality, and an expiring site grant. Corrupt, stale,
ambiguous, unauthorized, protected-human, direct-identifier, and clinical-decision candidates are
denied or left unresolved; the connector returns a verified local handle and never moves raw bytes.

P09-F28 `federated_replay_conformance` is the cross-site release gate for a reusable glioma
workflow. It verifies signed aggregate replay metrics against an exact workflow digest and
versioned absolute/relative tolerances, retains per-metric deviations, and emits missing, stale,
revoked, tampered, rejected, and out-of-tolerance site states. A portability claim is possible only
when every required site passes; absent or failed evidence is never imputed as success.

P09-F29 `local_compute_cache_governor` turns cache reuse into a reproducibility-controlled
product capability. It binds every hit to input, code, environment, policy, semantic-version, and
output-schema identities, invalidates entries after dependency or policy changes, and applies
deterministic retention and size/entry quotas. Only unpinned local intermediates may be evicted;
pinned research inputs can block admission. The decision returns reusable handles, invalidations,
eviction records, negative evidence, and uncertainty without accessing raw bytes.

P09-F30 `multistudy_cache_partition` extends cache safety to consortium-scale studies. It
partitions reuse by requesting study and de-identification scope, permits explicitly public
reference assets to cross studies, rejects protected or restricted cross-study reads, and
invalidates entries when cache-key or policy identity changes. Boundary flags for human data,
direct identifiers, and clinical decisions fail closed before any handle is returned.

P09-F31 `high_throughput_compute_capacity` is the queue-to-capacity control surface for
autonomous preclinical glioma computation. It orders admitted jobs with deterministic fairness
across workflow groups, calibrates duration and success intervals from local runtime observations,
reserves bounded concurrency/resource/memory/accelerator/budget capacity, and keeps stale
telemetry, saturation, age expiry, deferral, and required-job blocking visible. It emits a
simulation-only `ComputeCapacityPlan`; local schedulers may consume that plan, but the MCP route
does not execute code or move protected data.

P09-F32 `federated_compute_cost_exchange` is the consortium placement surface for reproducible
glioma computation. It exchanges signed aggregate capacity classes, cost intervals, capability
and localization scopes, freshness, and reconstruction-risk evidence while keeping credentials,
raw inputs, and outputs at the originating site. Revoked, stale, expired, incompatible, or
policy-over-budget sites are excluded explicitly; missing summaries remain unresolved and never
become implied capacity. The resulting `FederatedComputeCapacityEnvelope` is a ranked,
simulation-only handoff to a caller-owned local scheduler.

P09-F23 `computation_event_stream` gives the autonomous engine a replayable execution
telemetry surface. It orders task, resource, QC, and recovery events by durable sequence,
deduplicates exact retries, emits explicit recovery gaps instead of inferring completion from
missing telemetry, and preserves event identity when payloads are redacted by local policy.
Cursor-bound pages are deterministic across permutations and resumable after network loss;
the route remains institution-local and never performs raw-data movement or clinical decisions.

P09-F04 `federated_replay_discrepancy_scan` is the consortium computational-science lead's
cross-site replay-localization capability. It compares signed permitted aggregate attestations
without moving protected inputs, separates workflow, data-version, environment, dependency,
numeric-kernel, seed, output, and multi-factor divergence, and emits bounded non-dispatchable
site-local diagnostic tasks. Missing or stale summaries remain unresolved; raw inputs and clinical
decisions stay out of scope. Deterministic permutation, injected environment/seed differences,
missing-field, signature, and raw/clinical-boundary tests are included.

P09-F18 `cross_study_comparator` is the computational scientist's cross-study alignment and
variation diagnostic. It accepts explicit feature semantics, units, pipeline/version/normalization
profiles, local artifact handles, and independent experimental-group identities. It refuses hidden
unit or semantic coercion, preserves missing and censored observations as exclusions, gives each
independence group equal weight while using bounded inverse-uncertainty weighting within each group,
and reports pooled fixed-point means, uncertainty, total range/MAD, within-pipeline variation,
between-pipeline variation, and leave-one-group-out shifts. A feature
with dominant pipeline variation is labeled `processing_shift`; residual disagreement is retained
as `biological_variation` rather than averaged away; incomplete support remains `insufficient` or
`non_comparable`. The result is a typed input for P10 interpretation/replication and P06 follow-up
selection, not a causal claim and not a clinical prediction. Deterministic permutation, unit/schema
shift, processing-shift, and repeated-independence-group tests are included.

Folder inventory: 34 source modules; 32/32 feature slots implemented. The program folder directly owns 32 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`artifact_lineage_index`](crates/research/src/glioma/programs/p09_reproducible_computation/artifact_lineage_index.rs) | GAF-GLIOMA-P09-F07 | Semantic-loss-aware computation-artifact lineage indexing for preclinical glioma research. | 4 | `execution` |
| [`artifact_registry_connector`](crates/research/src/glioma/programs/p09_reproducible_computation/artifact_registry_connector.rs) | GAF-GLIOMA-P09-F22 | Policy-bounded local scientific artifact-registry resolution for glioma workflows. | 4 | — |
| [`autonomous_stage_bridge`](crates/research/src/glioma/programs/p09_reproducible_computation/autonomous_stage_bridge.rs) | workflow composition | P07 autonomous-engine bridge for the governed P09 computation operating cycle. | 1 | `campaign`, `execution`, `operating_cycle` |
| [`campaign`](crates/research/src/glioma/programs/p09_reproducible_computation/campaign.rs) | GAF-GLIOMA-P09-F13 | Bounded autonomous computation campaigns with recoverable partial rounds for preclinical glioma research. | 3 | `execution`, `planning`, `portfolio_execution` |
| [`computation_event_stream`](crates/research/src/glioma/programs/p09_reproducible_computation/computation_event_stream.rs) | GAF-GLIOMA-P09-F23 | Replayable event batches for autonomous preclinical glioma computation. | 5 | — |
| [`computation_run_inspector`](crates/research/src/glioma/programs/p09_reproducible_computation/computation_run_inspector.rs) | GAF-GLIOMA-P09-F17 | Researcher-facing inspection of long-running preclinical glioma computations. | 4 | `artifact_lineage_index`, `execution`, `partial_result_semantics` |
| [`cross_study_comparator`](crates/research/src/glioma/programs/p09_reproducible_computation/cross_study_comparator.rs) | GAF-GLIOMA-P09-F18 | Cross-study computational result comparison for preclinical glioma research. | 5 | — |
| [`environment_resolution_agent`](crates/research/src/glioma/programs/p09_reproducible_computation/environment_resolution_agent.rs) | GAF-GLIOMA-P09-F09 | Constrained environment-resolution proposals for autonomous glioma computation. | 4 | `execution_environment_lock` |
| [`execution`](crates/research/src/glioma/programs/p09_reproducible_computation/execution.rs) | GAF-GLIOMA-P09-F10 | Replayable execution of multimodal glioma computation graphs. | 7 | — |
| [`execution_environment_lock`](crates/research/src/glioma/programs/p09_reproducible_computation/execution_environment_lock.rs) | GAF-GLIOMA-P09-F06 | Content-addressed compute-environment locking for autonomous preclinical glioma workflows. | 5 | — |
| [`federated_compute_cost_exchange`](crates/research/src/glioma/programs/p09_reproducible_computation/federated_compute_cost_exchange.rs) | GAF-GLIOMA-P09-F32 | Federated compute capacity and cost exchange for preclinical glioma research. | 5 | — |
| [`federated_replay_conformance`](crates/research/src/glioma/programs/p09_reproducible_computation/federated_replay_conformance.rs) | GAF-GLIOMA-P09-F28 | Federated replay conformance verification for preclinical glioma workflows. | 4 | — |
| [`federated_replay_discrepancy_scan`](crates/research/src/glioma/programs/p09_reproducible_computation/federated_replay_discrepancy_scan.rs) | GAF-GLIOMA-P09-F04 | Federated replay discrepancy localization for preclinical glioma computation. | 5 | — |
| [`federated_workflow_template_exchange`](crates/research/src/glioma/programs/p09_reproducible_computation/federated_workflow_template_exchange.rs) | GAF-GLIOMA-P09-F16 | Federated validated-workflow template exchange for preclinical glioma research. | 4 | `execution_environment_lock` |
| [`high_throughput_compute_capacity`](crates/research/src/glioma/programs/p09_reproducible_computation/high_throughput_compute_capacity.rs) | GAF-GLIOMA-P09-F31 | High-throughput capacity planning for autonomous preclinical glioma computation. | 5 | — |
| [`high_throughput_compute_timeline`](crates/research/src/glioma/programs/p09_reproducible_computation/high_throughput_compute_timeline.rs) | GAF-GLIOMA-P09-F19 | Campaign-level throughput and bottleneck intelligence for `GAF-GLIOMA-P09-F19`. | 5 | `computation_run_inspector`, `execution` |
| [`interpretation_frontier`](crates/research/src/glioma/programs/p09_reproducible_computation/interpretation_frontier.rs) | GAF-GLIOMA-P09-F26 | Replayable computation to interpretation/replication frontier. | 2 | `campaign` |
| [`lineage`](crates/research/src/glioma/programs/p09_reproducible_computation/lineage.rs) | GAF-GLIOMA-P09-F03 | Artifact-lineage joins and adaptive recomputation frontiers for glioma computation runs. | 2 | `execution` |
| [`local_compute_cache_governor`](crates/research/src/glioma/programs/p09_reproducible_computation/local_compute_cache_governor.rs) | GAF-GLIOMA-P09-F29 | Deterministic local computation-cache governance for preclinical glioma workflows. | 5 | — |
| [`multistudy_cache_partition`](crates/research/src/glioma/programs/p09_reproducible_computation/multistudy_cache_partition.rs) | GAF-GLIOMA-P09-F30 | Study-aware cache partitioning for preclinical glioma computation. | 5 | — |
| [`operating_cycle`](crates/research/src/glioma/programs/p09_reproducible_computation/operating_cycle.rs) | GAF-GLIOMA-P09-F24 | Intent-to-computation operating cycle for autonomous preclinical glioma research. | 2 | `campaign`, `execution`, `workflow` |
| [`partial_result_semantics`](crates/research/src/glioma/programs/p09_reproducible_computation/partial_result_semantics.rs) | GAF-GLIOMA-P09-F08 | Typed partial-result semantics for reproducible preclinical glioma computation. | 4 | `execution` |
| [`phase_resolved_imaging_handoff`](crates/research/src/glioma/programs/p09_reproducible_computation/phase_resolved_imaging_handoff.rs) | GAF-GLIOMA-P09-F27 | Compile completed phase-resolved glioma image captures into a reproducible P09 workflow. | 4 | `execution`, `p08_instrument_robotics`, `p08_instrument_robotics::execution`, `p08_instrument_robotics::phase_resolved_invasion_schedule`, `p08_instrument_robotics::preflight`, `workflow` |
| [`placement`](crates/research/src/glioma/programs/p09_reproducible_computation/placement.rs) | GAF-GLIOMA-P09-F20 | Deterministic worker placement for reproducible preclinical glioma computation. | 3 | `execution` |
| [`placement_stress_evaluation`](crates/research/src/glioma/programs/p09_reproducible_computation/placement_stress_evaluation.rs) | workflow composition | Stress evaluation for the reproducible glioma computation placement planner. | 2 | `execution`, `placement` |
| [`planning`](crates/research/src/glioma/programs/p09_reproducible_computation/planning.rs) | GAF-GLIOMA-P09-F11 | Beam-selected portfolio planning for reproducible glioma computation DAGs. | 5 | `execution` |
| [`portfolio_execution`](crates/research/src/glioma/programs/p09_reproducible_computation/portfolio_execution.rs) | GAF-GLIOMA-P09-F12 | Autonomous bridge from computation-portfolio selection to local execution. | 2 | `execution`, `planning` |
| [`recovery_campaign`](crates/research/src/glioma/programs/p09_reproducible_computation/recovery_campaign.rs) | GAF-GLIOMA-P09-F25 | Adaptive recovery for failed and partial preclinical glioma computation campaigns. | 1 | `campaign`, `execution`, `planning` |
| [`reproducibility`](crates/research/src/glioma/programs/p09_reproducible_computation/reproducibility.rs) | GAF-GLIOMA-P09-F02 | Repeated-run reproducibility analysis for preclinical glioma computation. | 2 | — |
| [`reproducible_task_api`](crates/research/src/glioma/programs/p09_reproducible_computation/reproducible_task_api.rs) | GAF-GLIOMA-P09-F21 | Versioned, idempotent task handoff for autonomous preclinical glioma computation. | 4 | `execution_environment_lock` |
| [`robustness`](crates/research/src/glioma/programs/p09_reproducible_computation/robustness.rs) | GAF-GLIOMA-P09-F01 | Deterministic robustness analysis for preclinical glioma outcomes. | 3 | — |
| [`robustness_guided`](crates/research/src/glioma/programs/p09_reproducible_computation/robustness_guided.rs) | GAF-GLIOMA-P09-F15 | Robustness-guided computation frontier for preclinical glioma studies. | 2 | `execution`, `planning`, `portfolio_execution`, `robustness` |
| [`workflow`](crates/research/src/glioma/programs/p09_reproducible_computation/workflow.rs) | GAF-GLIOMA-P09-F14 | Intent-to-DAG compilation for autonomous preclinical glioma computation. | 3 | `campaign`, `execution`, `planning` |
| [`workflow_execution_manifest`](crates/research/src/glioma/programs/p09_reproducible_computation/workflow_execution_manifest.rs) | GAF-GLIOMA-P09-F05 | Dependency-closed execution manifest for reproducible glioma computation. | 4 | `workflow` |

---

### P10 — `p10_interpretation_replication`


- Consumer: methods reviewer and replication scientist.
- Product contract: uncertainty-aware causal interpretation, transportability, mediation,
  dynamic policies, contradiction adjudication, replication closure, and adaptive frontier work.
- Stratified adjustment must retain technical-batch leave-one-out shifts alongside stratum
  influence. A contrast dominated by one assay batch routes to replication; if a fold drops below
  the primary replicate floor, the result says so explicitly rather than silently discarding it.
- P10-F11 `causal_adjustment` now emits `GliomaStratifiedCausalAdjustment1@2` with a declared
  bounded unmeasured-confounding sensitivity interval. A nominal effect that survives observed
  strata and batch checks but falls below the practical-effect floor under that hidden-bias budget
  remains unresolved and routes replication/measurement follow-up; the bound is not treated as an
  imputed correction or a causal claim.
- Mediation estimators must center mediator/outcome covariance within treatment arms and use
  arm-specific residual baselines; pooled between-arm separation is not allowed to create an
  indirect-effect claim. Leave-one-unit influence, declared uncertainty, and null mediation remain
  release gates for the preclinical mechanism workflow.
- Primary artifacts: `AnalysisReplicationRecord`, causal claim adjudications, and replication
  closure campaigns.
- The autonomous graph now has executable P10 stage workers in
  `autonomous_stage_bridge.rs`. A statistical-interpretation worker runs the existing
  cross-family synthesis and adaptive-frontier algorithm, while a replication-robustness worker
  runs the bounded multi-round replication/meta-analysis/transportability campaign. Both workers
  are reachable through the evidence-gated P07 engine and MCP dry-run tools, preserve negative,
  unresolved, heterogeneous, retry, and budget outcomes, and emit local typed artifacts with
  `biological_evidence_promoted=false`. Interpretation must still clear independent replication;
  replication must still clear research-object release and federated review. No stage output is a
  clinical decision or an instrument authorization.
- P10-F02 — `lineage_propagation` (early implementation; scientific validation remains open): a
  local, lineage-resolved finite-interval
  state propagation and clonal-growth model. Inputs are barcode-by-state cell-count snapshots
from preclinical experiments, with known capture fractions, fixed observation spacing, and
independent experimental-unit identifiers. The product fits a nonnegative discrete-time
propagation operator per arm, approximately equal-weights independent units in the fitting
objective, reports its effective descendant yield and destination-state composition, quantifies
between-arm operator differences with experimental-unit-clustered resampling, and validates
one-step predictions on the final held-out interval. Each observation
  carries a technical assay/processing-batch identifier distinct from its biological experimental
  unit. The model performs bounded leave-one-assay-batch-out refits of treatment-minus-control
  effects, reports fragile, partial, and unresolved sensitivity states, and sends state-specific
  batch uncertainty into P06 follow-up prioritization. A failed or underpowered omission fold is
  reported as unevaluable, never treated as stable. It must not call
  operator entries direct cell-switch probabilities or continuous-time transition rates. Missing
  lineage/timepoint records are excluded and counted, never imputed as extinction; only explicit
  zero-count observations encode non-detection. Rank-deficient state mixtures, boundary fits, or
  failed held-out prediction keep mechanistic interpretation unresolved. This extends AURORA's
  descriptive P10-F14 state-transition analysis into a productized growth-aware model; the
  underlying state-transition/growth model family has published glioblastoma prior art, so the
  differentiator is the unit-aware validation, identifiability disclosure, and workflow integration,
  not an unverified claim of a new mathematical family.
#### Implemented P10-F04 cross-model claim-envelope analysis

`GAF-GLIOMA-P10-F04` is owned by `programs/p10_interpretation_replication/cross_model_claim_envelope.rs`.
It gives the autonomous research engine a conservative claim boundary when independent studies
agree imperfectly across organoid, animal, in-silico, or other preclinical model systems. The
consumer is the interpretation/replication planner, which needs to know whether a mechanistic
claim is transportable enough to schedule replication or must remain model-dependent.

- Inputs are named study-level effect intervals, independent-group identities, quality and
  uncertainty metadata, and local artifact references. Raw measurements remain behind the local
  artifact boundary; human data and clinical decisions are out of scope.
- The algorithm averages estimates within each represented model system, gives represented
  systems equal weight, expands intervals by an investigator-declared hidden-bias budget, and
  reports the pooled interval, practical-effect coverage, sign stability, between-system range,
  and leave-one-study-out shifts.
- `qualified` requires the declared system/study floors, a practically meaningful sign-stable
  envelope, and the heterogeneity gate. `model_dependent`, `negative`, `partial`, and
  `unresolved` remain first-class scientific outcomes. Omitted estimates and typed next actions
  identify the exact replication or coverage work needed next.
- Acceptance includes deterministic ordering and digest validation, under-covered-system
  preservation, equal-system weighting, hidden-bias stress, and an MCP analysis-only route. The
  result cannot authorize an instrument, export raw data, or become clinical guidance.

#### Implemented autonomous cross-model replication frontier composition

The `glioma_cross_model_replication_frontier` route consumes the validated envelope and compiles
the next bounded research portfolio. It distinguishes independent replication, missing-model
acquisition, heterogeneity resolution, influential-study stress, negative-result confirmation, and
methods audit; scores each by information, expected between-system-range reduction,
reproducibility, feasibility, risk, and cost; and performs deterministic beam selection under
budget, action, risk, model-diversity, and dependency constraints. A selected action is still a
plan, not evidence: the result explicitly hands off to P07 authority and local execution gates,
while `negative_hold`, `partial`, and `no_runnable_actions` remain scientific/operational holds.

#### Implemented P07 cross-model replication mission bridge

The `glioma_cross_model_replication_mission` composition turns that scientific frontier into a
usable autonomous research workflow plan. It re-materializes selected P10 follow-ups as typed
`GliomaActionCandidate` replication actions and submits them to the P07 adaptive beam scheduler,
so dependency closure, completed-action history, budget, risk, autonomy, instrument, and
federation controls are evaluated in the same plan. The mission keeps the frontier digest,
selected/deferred/blocked action partitions, negative evidence, uncertainty, and next operator
action together, making a model-dependent or underpowered claim an explicit hold rather than a
silent promotion. It is planning-only: the returned plan still requires institutional authority
and local execution services before any preclinical computation or assay can run.

The companion `glioma_cross_model_replication_mission_execute` surface closes the safe workbench
loop without pretending that a dry run is science. It replays the same frontier and scheduler,
filters execution to the selected action set, and invokes the existing local action-executor seam
with bounded retries, dependency artifacts, optional workflow scope, and required-artifact checks.
The default MCP path uses the synthetic executor, emits content-addressed local artifacts, and
labels every result as non-biological evidence; an institution can provide its own executor only
through the typed Rust trait and its existing authority/instrument gates.

#### Implemented P10-F03 cross-model lineage-transition transport audit

`GAF-GLIOMA-P10-F03` is owned by `programs/p10_interpretation_replication/lineage_transport.rs`.
It asks whether a preclinical glioma perturbation produces a state-specific lineage-propagation
contrast that is consistent across represented model systems or carried by one system. Consumers
are the replication scientist and P06 assay-allocation planner. This is a preclinical research
transportability diagnostic, not a clinical prediction or treatment recommendation.

- Inputs: separately validated P10-F02 analyses from named independent studies; local,
  de-identified artifact references; a shared objective, state order, arm definitions, and
  observation interval; prespecified practical-effect and between-system limits; minimum system
  and study counts; confidence level; and bounded bootstrap budget.
- Estimand: each source analysis retains its experimental-unit-clustered contrast. Within each
  model-system stratum, independent study estimates receive equal weight; represented systems
  then receive equal weight. This prevents a large number of studies from one cell-line family
  numerically dominating organoid, xenograft, or animal-model strata. Systems are treated as the
  fixed set observed, not a random sample of every possible glioma model.
- Uncertainty: a deterministic, bounded hierarchical bootstrap resamples studies within each
  represented system and paired unit-bootstrap draws within each study. The report includes
  per-system and pooled intervals, between-system effect range, leave-one-system-out shifts,
  direction reversals, included and omitted study IDs, and explicit evidence gaps. Cells,
  barcodes, fields, and technical wells are never treated as independent studies.
- Dispositions: `qualified` requires the declared study/system floors, an interval beyond the
  practical margin, and the prespecified transport-gap criterion; `model_dependent` retains a
  material cross-system reversal or excessive range; `negative` means the pooled interval lies
  inside the practical-effect margin; `partial` means some contrasts or sample floors remain
  incomplete; and `unresolved` means fewer than two comparable systems or no eligible contrast.
  A negative outcome is a scientific result, not an execution failure.
- Acceptance: hand-computed equal-system weighting; repeating studies in one system cannot create
  false multi-system qualification; a reversed system triggers `model_dependent`; rank-unresolved
  P10-F02 inputs remain named omissions; shuffled study order preserves byte-identical output;
  mismatched state order, arms, objective, interval, or artifact boundaries are rejected; and
  excessive work fails explicitly.
- Integration: P06 can use discordant or underrepresented systems as assay-follow-up candidates,
  while P11 can retain negative or unresolved evaluation with the research object. This feature
  does not dispatch instruments or move raw experimental data between institutions.
- Prior-art comparison: comparative glioblastoma studies report model-context differences in cell
  state composition, and perturbation-associated transitions have been tested across patient-
  derived and mouse models. Independent experimental units, not nested cells, remain the inferential
  basis. The claim is deliberately **not** “a new transportability estimator.” The product advance
  is composition of P10-F02 lineage-resolved effects with equal-system study aggregation, bounded
  nested uncertainty, and explicit leave-one-system-out fragility. Biological performance remains
  unknown until prospective held-out-model benchmarking. Sources: Pine et al., *Cancer Discovery*
  (2020), doi:10.1158/2159-8290.CD-20-0057; Caspani et al., *Nature Communications* (2024),
  doi:10.1038/s41467-024-47985-z; Zimmerman et al., *Nature Communications* (2021),
  doi:10.1038/s41467-021-21038-1.

- Downstream edges: P06 follow-up design, P11 release verdicts, and P12 consortium comparisons.

#### Implemented P10-F05 typed independent-site replication protocol

`GAF-GLIOMA-P10-F05` is owned by `programs/p10_interpretation_replication/replication_protocol_schema.rs`.
It compiles a preclinical replication contract with an explicit estimand, assay scope, independent
site capabilities, randomization, masking, stopping rule, prespecified analysis, declared
deviations, and named unknowns. Source-site provenance is retained, but source values cannot be
reused as independent-site identity. Blocking unknowns produce a valid blocked protocol rather
than an executable-looking pass. The artifact is canonical, content-addressed, and replay-stable;
it never dispatches work or makes a clinical decision.

#### Implemented P10-F06 cross-site assay-mapping ledger

`GAF-GLIOMA-P10-F06` is owned by `programs/p10_interpretation_replication/replication_assay_mapping_ledger.rs`.
It compares source and replication variable dictionaries, units, semantic roles, detection limits,
and calibration observations. Exact and calibrated mappings are separately listed from
context-limited, non-equivalent, and unresolved mappings; uncertain measures are never promoted
into a pooled analysis. Every accepted transform carries an evidence digest and bounded residual
check. This is a measurement-comparability contract, not a biological replication result.

#### Implemented P10-F04 lineage-response decomposition

`GAF-GLIOMA-P10-F04` is owned by
`programs/p10_interpretation_replication/lineage_response_decomposition.rs`. It standardizes the
P10-F02 control and perturbation propagation operators to the same investigator-declared
pretreatment state mixture, then decomposes each source-to-destination output contrast into
symmetric net-descendant-yield and destination-composition components. Its consumers are the
preclinical glioma methods reviewer and P06 assay-allocation planner.

- Inputs: a validated P10-F02 analysis and an explicit integer-ppm source-state composition that
  sums to one million, plus a prespecified practical component margin.
- Estimand: expected finite-interval descendants per million cells under the declared starting
  state mixture. The decomposition uses the symmetric identity
  `Δ(yq) = Δy(qₜ+q꜀)/2 + (yₜ+y꜀)Δq/2`; each source/destination pair reports the total, both
  components, paired bootstrap intervals, and fixed-point reconstruction residual.
- Interpretation: net yield combines proliferation and death; destination composition is an
  aggregate lineage-level quantity. Neither component is a direct cell-switch probability, a pure
  growth/viability effect, or a causal mediation estimate. Conclusions depend on the supplied
  baseline mixture.
- Fail-closed behavior: rank-unresolved or prediction-failing P10-F02 analyses do not produce
  qualified component estimates. If a positive-share source state has zero yield in any bootstrap
  draw, the affected component intervals are withheld rather than conditioning on draws that can
  be estimated. Output is explicitly unresolved/partial and routes follow-up to net-yield,
  state-composition, joint validation, replication, or identifiability review.
- Acceptance: pure-yield changes leave the composition component at zero; pure destination
  redistribution leaves the yield component at zero; each pair reconstructs the standardized
  total contrast within fixed-point tolerance; row/order permutations preserve identical output;
  malformed mixtures and invalid source analyses are rejected; and zero-yield draws cannot be
  silently dropped.
- Prior-art comparison: PATH already estimates cell-state heritability/plasticity and transition
  and proliferation dynamics from phylogenetically annotated single-cell lineages, including a
  glioblastoma application; Neftel et al. established lineage-tracing evidence for plasticity among
  glioblastoma cell states. This feature explicitly does **not** claim a new transition model or
  decomposition family. Its product advance is a reproducible, paired-unit-bootstrap decomposition
  directly composed with P10-F02's aggregate barcode-by-state operator and routed into assay design.
  Its biological utility remains unvalidated until prospective held-out model benchmarking.
  Sources: Schiffman et al., *Nature Genetics* 56, 2174–2184 (2024),
  doi:10.1038/s41588-024-01920-6; Neftel et al., *Cell* 178, 835–849.e21 (2019),
  doi:10.1016/j.cell.2019.06.024.

P10-F07 `claim_adjudication` now computes confidence from the disposition of each scientific
gate, not from unconstrained numeric summaries alone. A negative or unresolved causal, sensitivity,
replication, or meta-analysis gate contributes zero or a bounded hold score; a strong downstream
signal cannot average away missing or failed evidence. The output schema is
`GliomaCausalClaimAdjudication1@3`. Selected follow-ups are canonicalized for replay while
priority order remains the execution recommendation; when the caller action cap is smaller than
the generated portfolio, deferred action IDs remain explicit and disjoint rather than being
dropped. Timepoint completion, confounder measurement, independent replication, heterogeneity
resolution, negative-result publication, and bounded preclinical release therefore remain
routable in later cycles.

- Program promotion gate: estimand clarity, unit-clustered uncertainty, full-rank or explicitly unresolved
  design, calibrated held-out prediction, sensitivity to assay batch, capture correction, and regularization,
  independent reproduction, and explicit null/negative outcomes.
- The adaptive interpretation campaign now records value-only per-action outcome posteriors and
  exposes them to history-aware planners. Completed, negative, partial, failed, and skipped
  actions remain distinct with retry burden and a smoothed success estimate, so a glioma planner
  can switch from a failing frontier action to complementary stability, replication, or mechanism
  discrimination work without importing institution-local artifacts or fabricating evidence.
- P10-F20/P10-F26 now close that loop inside the frontier selector: prior action summaries are
  carried into `GliomaAdaptiveResearchFrontier1@2`, and the empirical-Bayes adapter discounts
  information, leverage, feasibility, and safety scores for repeatedly failing branches while
  preserving a bounded novelty bonus. The next batch therefore changes because of observed local
  outcomes, not because a static action table was replayed; the summary is retained in the signed
  content-addressed frontier digest for deterministic resumption.
- P10-F26 now charges each adaptive round from the nested portfolio's measured worker-invocation
  spend, including retries, instead of summing each dispatched action once. A transient
  interpretation-worker failure therefore cannot create false budget headroom for a later round.
- P10-F18 now charges every replication-campaign worker invocation before dispatch, including
  failed and retry attempts. Retry counts are incremented only for an observed retryable failure;
  if the next attempt cannot fit the remaining budget, the action is recorded as failed with an
  explicit budget-exhaustion uncertainty and the campaign returns a replayable partial result.
  Terminal executor failures now seal the attempted round as well, retaining its observations,
  budget delta, and final re-analysis instead of returning a failed campaign with an unaccounted
  spend.
- Current implementation: 32/32 slots including P10-F02 through P10-F06. Maintain with
  longitudinal replication transport and prospective contradiction resolution.

P10-F27 `replication_closure_frontier` now adds deterministic target- and model-system-diversity
bonuses to its bounded cost/risk beam. Near-tied closure portfolios therefore cover distinct
independent-site, heterogeneity, target-model, stress-test, or negative-confirmation routes rather
than repeating one closure target; the versioned frontier is `GliomaReplicationClosureFrontier1@2`.
P10-F28 now translates selected closure targets into an admitted campaign-action-kind set before
dispatch. The campaign selector is filtered by that set, so a high-level frontier approval for
independent-site replication, heterogeneity resolution, target-model acquisition, stress testing,
or negative-result confirmation cannot silently drift into another scientific operation.

Folder inventory: 33 source modules; 32/32 feature slots implemented. The program folder directly owns 30 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`adaptive_campaign`](crates/research/src/glioma/programs/p10_interpretation_replication/adaptive_campaign.rs) | GAF-GLIOMA-P10-F26 | Bounded multi-round adaptive interpretation campaigns. | 4 | `adaptive_execution`, `adaptive_frontier`, `synthesis` |
| [`adaptive_execution`](crates/research/src/glioma/programs/p10_interpretation_replication/adaptive_execution.rs) | GAF-GLIOMA-P10-F25 | Guarded execution of the adaptive interpretation frontier. | 3 | `adaptive_frontier` |
| [`adaptive_frontier`](crates/research/src/glioma/programs/p10_interpretation_replication/adaptive_frontier.rs) | GAF-GLIOMA-P10-F20 | Outcome-conditioned next-step planning for the autonomous preclinical glioma engine. | 6 | — |
| [`autonomous_stage_bridge`](crates/research/src/glioma/programs/p10_interpretation_replication/autonomous_stage_bridge.rs) | workflow composition | P07 autonomous-engine bridges for P10 interpretation and replication stages. | 1 | `campaign`, `operating_cycle` |
| [`campaign`](crates/research/src/glioma/programs/p10_interpretation_replication/campaign.rs) | GAF-GLIOMA-P10-F18 | Beam-selected autonomous replication and interpretation campaigns for preclinical glioma research. | 9 | `meta_analysis`, `transportability` |
| [`causal_adjustment`](crates/research/src/glioma/programs/p10_interpretation_replication/causal_adjustment.rs) | GAF-GLIOMA-P10-F11 | Stratified causal adjustment for preclinical glioma assays. | 6 | — |
| [`causal_contrast`](crates/research/src/glioma/programs/p10_interpretation_replication/causal_contrast.rs) | GAF-GLIOMA-P10-F10 | Exact, bounded difference-in-differences interpretation for preclinical glioma studies. | 2 | `trajectory` |
| [`claim_adjudication`](crates/research/src/glioma/programs/p10_interpretation_replication/claim_adjudication.rs) | GAF-GLIOMA-P10-F07 | Causal-claim adjudication for preclinical glioma research. | 4 | `causal_contrast`, `meta_analysis`, `sensitivity`, `trajectory` |
| [`clone_outcomes`](crates/research/src/glioma/programs/p10_interpretation_replication/clone_outcomes.rs) | GAF-GLIOMA-P10-F21 | Adjudication of clone-aware perturbation-panel outcomes. | 5 | — |
| [`closure_interpretation`](crates/research/src/glioma/programs/p10_interpretation_replication/closure_interpretation.rs) | GAF-GLIOMA-P10-F30 | Convert replication-closure campaign results into uncertainty-aware interpretation. | 1 | `campaign`, `replication_closure_campaign`, `synthesis` |
| [`computation_evidence_gate`](crates/research/src/glioma/programs/p10_interpretation_replication/computation_evidence_gate.rs) | GAF-GLIOMA-P10-F31 | Evidence-aware handoff from the P09 computation frontier into P10 interpretation. | 1 | `synthesis` |
| [`cross_model_claim_envelope`](crates/research/src/glioma/programs/p10_interpretation_replication/cross_model_claim_envelope.rs) | workflow composition | Cross-model, partially identified claim envelopes for preclinical glioma research. | 2 | — |
| [`cross_model_replication_frontier`](crates/research/src/glioma/programs/p10_interpretation_replication/cross_model_replication_frontier.rs) | workflow composition | Cross-model replication frontier for autonomous preclinical glioma research. | 3 | `cross_model_claim_envelope` |
| [`dynamic_policy`](crates/research/src/glioma/programs/p10_interpretation_replication/dynamic_policy.rs) | GAF-GLIOMA-P10-F16 | Deterministic longitudinal policy evaluation for preclinical glioma workflows. | 3 | — |
| [`lineage_dynamics`](crates/research/src/glioma/programs/p10_interpretation_replication/lineage_dynamics.rs) | GAF-GLIOMA-P10-F32 | Replicate-level decomposition of glioma population-state changes across tracked lineages. | 5 | — |
| [`lineage_propagation`](crates/research/src/glioma/programs/p10_interpretation_replication/lineage_propagation.rs) | GAF-GLIOMA-P10-F02 | Fit and validate a lineage-resolved finite-interval glioma cell-state propagation operator. | 8 | — |
| [`lineage_response_decomposition`](crates/research/src/glioma/programs/p10_interpretation_replication/lineage_response_decomposition.rs) | GAF-GLIOMA-P10-F04 | Decompose preclinical glioma lineage-propagation contrasts into net-yield and state-composition components. | 3 | `lineage_propagation` |
| [`lineage_transport`](crates/research/src/glioma/programs/p10_interpretation_replication/lineage_transport.rs) | GAF-GLIOMA-P10-F03 | Compare lineage-resolved glioma perturbation contrasts across preclinical model systems. | 5 | — |
| [`mediation`](crates/research/src/glioma/programs/p10_interpretation_replication/mediation.rs) | GAF-GLIOMA-P10-F15 | Deterministic causal-mediation analysis for preclinical glioma studies. | 4 | — |
| [`meta_analysis`](crates/research/src/glioma/programs/p10_interpretation_replication/meta_analysis.rs) | GAF-GLIOMA-P10-F09 | Deterministic fixed-point meta-analysis for independent preclinical glioma studies. | 5 | — |
| [`operating_cycle`](crates/research/src/glioma/programs/p10_interpretation_replication/operating_cycle.rs) | GAF-GLIOMA-P10-F24 | Interpretation-gate and adaptive-frontier operating cycle for preclinical glioma research. | 1 | `adaptive_frontier`, `synthesis` |
| [`replication_assay_mapping_ledger`](crates/research/src/glioma/programs/p10_interpretation_replication/replication_assay_mapping_ledger.rs) | GAF-GLIOMA-P10-F06 | Cross-site assay equivalence ledger for independent preclinical glioma replication. | 3 | `replication_protocol_schema` |
| [`replication_closure_campaign`](crates/research/src/glioma/programs/p10_interpretation_replication/replication_closure_campaign.rs) | GAF-GLIOMA-P10-F29 | Bounded, multi-round execution of a glioma replication-closure frontier sequence. | 1 | `campaign`, `replication_closure_execution`, `replication_closure_frontier`, `validation_replication_campaign` |
| [`replication_closure_execution`](crates/research/src/glioma/programs/p10_interpretation_replication/replication_closure_execution.rs) | GAF-GLIOMA-P10-F28 | Execute a selected replication-closure frontier for preclinical glioma research. | 1 | `campaign`, `replication_closure_frontier`, `validation_replication_campaign` |
| [`replication_closure_frontier`](crates/research/src/glioma/programs/p10_interpretation_replication/replication_closure_frontier.rs) | GAF-GLIOMA-P10-F27 | Replication-closure frontier with bounded portfolio selection for autonomous preclinical glioma research. | 3 | `campaign`, `validation_replication_campaign`, `validation_replication_gate` |
| [`replication_protocol_schema`](crates/research/src/glioma/programs/p10_interpretation_replication/replication_protocol_schema.rs) | GAF-GLIOMA-P10-F05 | A typed, independent-site replication contract for preclinical glioma work. | 4 | — |
| [`sensitivity`](crates/research/src/glioma/programs/p10_interpretation_replication/sensitivity.rs) | GAF-GLIOMA-P10-F12 | Unmeasured-confounding sensitivity bounds for preclinical glioma effects. | 3 | — |
| [`state_transition`](crates/research/src/glioma/programs/p10_interpretation_replication/state_transition.rs) | GAF-GLIOMA-P10-F14 | Longitudinal discrete-state transition analysis for preclinical glioma models. | 8 | — |
| [`synthesis`](crates/research/src/glioma/programs/p10_interpretation_replication/synthesis.rs) | GAF-GLIOMA-P10-F19 | Cross-family synthesis for autonomous preclinical glioma interpretation. | 5 | — |
| [`trajectory`](crates/research/src/glioma/programs/p10_interpretation_replication/trajectory.rs) | GAF-GLIOMA-P10-F13 | Deterministic longitudinal trajectory analysis for preclinical glioma studies. | 3 | — |
| [`transportability`](crates/research/src/glioma/programs/p10_interpretation_replication/transportability.rs) | GAF-GLIOMA-P10-F17 | Transportability analysis across preclinical glioma model systems. | 3 | — |
| [`validation_replication_campaign`](crates/research/src/glioma/programs/p10_interpretation_replication/validation_replication_campaign.rs) | GAF-GLIOMA-P10-F23 | Execute the independent-site replication campaign after validation admission. | 1 | `campaign`, `validation_replication_gate` |
| [`validation_replication_gate`](crates/research/src/glioma/programs/p10_interpretation_replication/validation_replication_gate.rs) | GAF-GLIOMA-P10-F22 | Evidence-gated handoff from a local glioma validation campaign to independent replication. | 2 | `p06_experiment_design::validation_campaign` |

Shared public feature modules owned outside this folder:

| Feature ID | Source module | Purpose | Direct test annotations |
|---|---|---|---:|
| `GAF-GLIOMA-P10-F01` | [`analysis`](crates/research/src/glioma/analysis.rs) | Uncertainty-aware preclinical outcome analysis. | 2 |
| `GAF-GLIOMA-P10-F08` | [`replication`](crates/research/src/glioma/replication.rs) | Cross-study replication, robustness, and negative-result assessment. | 2 |

---

### P11 — `p11_research_object_release`


- Consumer: reproducibility steward and public research commons.
- Product contract: portable research-object assembly, replay campaigns, release gates, limitations,
  provenance, and immutable release lifecycle.
- Primary artifacts: `SignedResearchObject`, release manifests, replay reports, and release gates.
- Downstream edges: P12 federation and every upstream program's publication handoff.
- Promotion gate: complete provenance, methods/limitations, replay evidence, policy-compliant
  localization, signed checksums, and negative-result disclosure.
- Cross-program handoff: `execute_glioma_engine_release_operating_cycle` accepts only a qualified
  P07 autonomous-engine run. It content-binds the final workflow plan, engine execution digest,
  replay identity, exact checkpoint artifacts, and engine-reported negative evidence before P11
  starts replay or release evaluation. Partial, held, approval-pending, blocked, exhausted, or
  tampered engine runs are rejected as release inputs rather than being upgraded into publication
  candidates.
- Autonomous stage bridge: `glioma_evidence_gated_stage_engine_release_execute` routes a qualified
  P01/P07 frontier into the P11 replay and release gate as an unpublished typed research-object
  candidate. It keeps exact checkpoint hashes, negative evidence, uncertainty, and accountable
  review visible; signing, publication, raw-data movement, and federation remain separate governed
  actions.
- P11-F02 `reproducibility_score` computes explicit data-scope, code, environment, methods,
  artifact, uncertainty, negative-outcome, lineage, and independent-replay dimensions. Missing
  evidence scores zero; blocked manifests, replay mismatch, and missing scientific qualifications
  hard-block the profile. Leave-one-component-out sensitivity identifies which required component
  carries the claim, and the route remains evaluation-only until accountable release review.
- P11-F03 `leakage_audit` traverses the complete release dependency graph before serialization,
  preserving transitive paths while detecting protected payloads, direct identifiers, local-only
  references, embedded secrets, path escapes, missing dependencies, cycles, and depth overflow.
  Critical findings hard-block export; omitted local nodes remain explicit and the audit emits only
  metadata and content digests.
- P11-F04 `qualification_preserver` compares source and release qualification declarations,
  preserving uncertainty, intervals, null outcomes, failed replications, contradictions, omissions,
  limitations, and negative evidence. Weakened or missing qualifications, unbound lineage, and
  unsupported release claims remain explicit blockers; no release claim can exceed its evidence.
- P11-F09 `metadata_normalizer` compiles local release metadata through approved mapping rules,
  controlled vocabularies, and a target schema while retaining exact source-field links and a
  reversible change set. Conflicting values remain unresolved, inferred rules require confirmation,
  required fields fail closed, and protected human/clinical metadata is rejected before release.
- P11-F08 `signed_attestation` binds the manifest, build provenance, release-gate evidence, signer
  scope, independent verification results, and key revocation state into a content-addressed
  attestation. Failed gates, inactive/revoked authorities, missing verification, and post-sign
  mutation are fail-closed; issuing remains an institution-owned cryptographic seam.
- P11-F11 `artifact_integrity_scanner` reduces local streaming observations into a verified or
  quarantined partition. Digest mismatch, truncation, unsupported formats, malformed metadata,
  executable payloads, links, missing members, and byte/memory-budget violations remain explicit
  findings and can never be represented as verified.
- P11-F12 `license_scope_checker` evaluates transitive dependency licenses, audience scope,
  locality, embargo, rights confirmation, and field classification into allow/redact/deny/unresolved
  decisions. Unknown rights never become allowed; raw data, identifiers, secrets, and local-only
  content remain protected before any export plan is accepted.
- P11-F13 `release_bundle_compiler` compiles only supplied, authorized, content-addressed members
  into a deterministic offline replay plan. Missing dependencies and cycles block completion;
  excluded local inputs retain an explicit origin-institution replay boundary and limitations rather
  than being silently fetched or represented as complete.
- P11-F14 `multistudy_release_composer` composes study-scoped comparative metadata while retaining
  model-system, method, provenance, limitation, and assay-mapping lineage. Exact, comparable, and
  non-equivalent measures are partitioned deterministically; missing studies remain unavailable and
  comparable measures require an explicit pooling policy.
- P11-F15 `continuous_release_pipeline` compiles an ordered local event stream into an immutable
  release candidate. It detects stale or non-monotonic versions, missing required programs/artifacts,
  schema and policy regressions, dropped negative evidence, and explicit omissions; semantic diffs
  and accountable-review requirements remain content-addressed before any publication action.
- P11-F16 `federated_release_bundle` joins only policy-approved aggregate contributions from local
  sites. Quorum, freshness, schema/policy identity, uncertainty, heterogeneity, human-data, and
  locality gates are evaluated per site; omissions and localization statements remain in the signed
  object boundary, while raw data and credentials never enter the federated bundle.
- P11-F17 `release_preview_workbench` compiles an exact audience-specific release preview from a
  validated manifest. Canonical section/artifact allow-lists and redactions are applied before
  rendering, prior-version differences are explicit, and protected payloads cannot enter any
  preview path.
- P11-F18 `comparative_release_explorer` exposes only source-linked cells from a validated
  comparative object. Every cell retains study, assay field, digest, model-system, and mapping
  relation; missing/non-equivalent mappings stay unavailable or blocked, and access-bound cache
  keys force eviction when scope or epoch changes.
- P11-F19 `release_queue_console` reconciles a high-throughput candidate ledger with CI and reviewer
  telemetry. It marks stale observations, exposes failed checks and reviewer bottlenecks, preserves
  blocked states, and proposes only gate-preserving reorderings without mutating scientific state.
- P11-F21 `research_object_exchange_api` plans resumable, idempotent chunk exchange from a signed
  version manifest. Audience, locality, grant, signature, size, range, duplicate, cursor, and
  missing-chunk gates remain explicit; incomplete transfers are resumable and never become a
  publication side effect.
- P11-F27 `prospective_replay_fidelity_gate` runs a bounded clean-room task graph against the
  bundle-pinned environment. It compares content hashes, lineage digests, uncertainty, negative
  findings, and numeric metrics with task-local tolerances; unexplained divergence, dependency
  blockage, and resource exhaustion remain non-passing states. A tolerance pass is explicitly
  qualified rather than collapsed into exact reproducibility.
- P11-F10 `replay` charges every replay-worker invocation before dispatch, including retry and
  terminal-failure attempts. If a retry no longer fits the remaining budget, it is not launched;
  the task and budget uncertainty remain in the sealed round, and dependent tasks cannot be
  promoted from a partial replay. Replay spend and retry counts reconcile exactly to round ledgers.
- P11-F22 `archive_migration_adapter` applies only explicit, version-pinned identity or rename
  rules to long-lived research objects. It preserves artifact bytes, provenance, uncertainty, and
  negative evidence while reporting reversible mappings, optional omissions, semantic-loss budgets,
  and rollback boundaries; unknown mandatory fields or undeclared lossy transforms fail closed.
- P11-F25 `release_signature_verifier` verifies a signed release object against caller-provided
  offline trust roots. It independently checks manifest, build, release-gate, policy, canonical
  signature payload, key validity, freshness, and release readiness; tampering blocks, missing
  trust material remains unverifiable, and cryptographic validity never overrides scientific gates.
- P11-F26 `research_object_conformance_suite` evaluates a frozen standards profile against schema
  identity/version, required and forbidden fields, artifact coverage, provenance, uncertainty,
  negative evidence, verified-signature requirements, and declared extensions. Unsupported fields
  block with an explicit migration route; local extensions remain warnings rather than silently
  becoming part of the frozen core contract.
- P11-F28 `federated_release_sharing_gate` evaluates every aggregate field against recipient scope,
  quorum, site membership, revocation, localization, human-data exclusion, raw-data locality, and
  field policy. Share, redact, deny, and unresolved outcomes are independently content-addressed;
  a permissive global policy cannot override a revoked site or denied field.
- P11-F23 `release_event_protocol` replays content-bound candidate, review, publication, correction,
  withdrawal, and supersession events through an ordered lifecycle. Predecessor-chain integrity,
  authority revocation, sequence gaps, conflicting duplicates, and idempotent delivery remain
  explicit, while the protocol performs no publication or raw-data movement.
- P11-F29 `version_retention_governor` plans immutable-version retention and archival transitions
  from age, legal hold, pin, supersession, lineage, and verified-replica evidence. It produces
  deterministic retain/archive/deletion-blocked/restoration-blocked decisions and never mutates
  bytes or silently authorizes destruction.
- P11-F30 `distributed_archive_mirror` compares source and replica digests, availability, freshness,
  repair budgets, and approved regions. It distinguishes synchronized, repair-required, blocked,
  and unresolved versions and emits only bounded repair work; unauthorized or corrupt replicas
  never count toward archive health.
- P11-F31 `release_queue_scheduler` ranks ready candidates by gate state, reviewer readiness,
  fairness credit, deadline, and risk-adjusted priority under explicit compute/reviewer capacity.
  It emits scheduled, deferred, and blocked entries with reasons; no unready candidate can bypass
  the scientific or provenance gate.
- P11-F32 `consortium_publication_steward` reconciles independent site approvals, rejections,
  abstentions, pending responses, signatures, digest identity, quorum, dissent, and correction
  lineage. It preserves every site's authority and cannot convert silence or a revoked approval
  into publication.
- Current implementation: 32/32 slots. Maintain with versioned exchange and
  migration/conformance, archival retention, and consortium publication operations.

Folder inventory: 32 source modules; 32/32 feature slots implemented. The program folder directly owns 31 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`archive_migration_adapter`](crates/research/src/glioma/programs/p11_research_object_release/archive_migration_adapter.rs) | GAF-GLIOMA-P11-F22 | Standards-versioned archive migration adapter for preclinical glioma research objects. | 4 | — |
| [`artifact_integrity_scanner`](crates/research/src/glioma/programs/p11_research_object_release/artifact_integrity_scanner.rs) | GAF-GLIOMA-P11-F11 | Bounded release-artifact integrity scanning for preclinical glioma research objects. | 5 | — |
| [`autonomous_stage_bridge`](crates/research/src/glioma/programs/p11_research_object_release/autonomous_stage_bridge.rs) | workflow composition | P07 autonomous-engine bridge for the governed P11 research-object release cycle. | 1 | `operating_cycle`, `replay` |
| [`comparative_release_explorer`](crates/research/src/glioma/programs/p11_research_object_release/comparative_release_explorer.rs) | GAF-GLIOMA-P11-F18 | Read-only provenance-linked comparative explorer for released preclinical glioma studies. | 4 | `multistudy_release_composer` |
| [`consortium_publication_steward`](crates/research/src/glioma/programs/p11_research_object_release/consortium_publication_steward.rs) | GAF-GLIOMA-P11-F32 | Quorum-aware consortium publication and correction coordination for preclinical glioma objects. | 2 | — |
| [`continuous_release_pipeline`](crates/research/src/glioma/programs/p11_research_object_release/continuous_release_pipeline.rs) | GAF-GLIOMA-P11-F15 | Prospective continuous release-candidate compilation for preclinical glioma programs. | 5 | — |
| [`dependency_closure`](crates/research/src/glioma/programs/p11_research_object_release/dependency_closure.rs) | GAF-GLIOMA-P11-F07 | Transitive dependency-closure analysis for preclinical glioma research objects. | 2 | `multimodal_bundle` |
| [`distributed_archive_mirror`](crates/research/src/glioma/programs/p11_research_object_release/distributed_archive_mirror.rs) | GAF-GLIOMA-P11-F30 | Verified, locality-aware archival mirror assessment for preclinical glioma research objects. | 2 | — |
| [`federated_release_bundle`](crates/research/src/glioma/programs/p11_research_object_release/federated_release_bundle.rs) | GAF-GLIOMA-P11-F16 | Federated aggregate research-object release for preclinical glioma consortia. | 5 | — |
| [`federated_release_sharing_gate`](crates/research/src/glioma/programs/p11_research_object_release/federated_release_sharing_gate.rs) | GAF-GLIOMA-P11-F28 | Field-level federation sharing gate for preclinical glioma research objects. | 3 | `federated_release_bundle` |
| [`leakage_audit`](crates/research/src/glioma/programs/p11_research_object_release/leakage_audit.rs) | GAF-GLIOMA-P11-F03 | Transitive dependency and locality-leakage audit for glioma research-object releases. | 5 | — |
| [`license_scope_checker`](crates/research/src/glioma/programs/p11_research_object_release/license_scope_checker.rs) | GAF-GLIOMA-P11-F12 | Transitive license, locality, embargo, and audience shareability evaluation for releases. | 5 | — |
| [`metadata_normalizer`](crates/research/src/glioma/programs/p11_research_object_release/metadata_normalizer.rs) | GAF-GLIOMA-P11-F09 | Deterministic, reversible metadata normalization for preclinical glioma research objects. | 5 | — |
| [`migration`](crates/research/src/glioma/programs/p11_research_object_release/migration.rs) | GAF-GLIOMA-P11-F06 | Versioned migration checks for multimodal preclinical glioma research objects. | 2 | `multimodal_bundle` |
| [`multimodal_bundle`](crates/research/src/glioma/programs/p11_research_object_release/multimodal_bundle.rs) | GAF-GLIOMA-P11-F05 | Multimodal research-object packaging for preclinical glioma studies. | 2 | — |
| [`multistudy_release_composer`](crates/research/src/glioma/programs/p11_research_object_release/multistudy_release_composer.rs) | GAF-GLIOMA-P11-F14 | Provenance-preserving multi-study comparative release composition for preclinical glioma work. | 5 | — |
| [`operating_cycle`](crates/research/src/glioma/programs/p11_research_object_release/operating_cycle.rs) | GAF-GLIOMA-P11-F24 | Reproducible research-object release operating cycle for preclinical glioma work. | 3 | `release_gate`, `replay` |
| [`prospective_replay_fidelity_gate`](crates/research/src/glioma/programs/p11_research_object_release/prospective_replay_fidelity_gate.rs) | GAF-GLIOMA-P11-F27 | Prospective replay-fidelity gate for preclinical glioma research releases. | 4 | `release_bundle_compiler` |
| [`qualification_preserver`](crates/research/src/glioma/programs/p11_research_object_release/qualification_preserver.rs) | GAF-GLIOMA-P11-F04 | Scientific-qualification preservation audit for glioma research-object releases. | 5 | — |
| [`release_bundle_compiler`](crates/research/src/glioma/programs/p11_research_object_release/release_bundle_compiler.rs) | GAF-GLIOMA-P11-F13 | Dependency-closed offline reproducibility-bundle compilation for preclinical glioma results. | 5 | `license_scope_checker` |
| [`release_event_protocol`](crates/research/src/glioma/programs/p11_research_object_release/release_event_protocol.rs) | GAF-GLIOMA-P11-F23 | Deterministic, idempotent release-lifecycle event protocol for preclinical glioma objects. | 3 | — |
| [`release_gate`](crates/research/src/glioma/programs/p11_research_object_release/release_gate.rs) | GAF-GLIOMA-P11-F20 | Reproducibility-aware release gating for preclinical glioma research objects. | 4 | `replay` |
| [`release_preview_workbench`](crates/research/src/glioma/programs/p11_research_object_release/release_preview_workbench.rs) | GAF-GLIOMA-P11-F17 | Audience-specific research-object release previews. | 4 | — |
| [`release_queue_console`](crates/research/src/glioma/programs/p11_research_object_release/release_queue_console.rs) | GAF-GLIOMA-P11-F19 | High-throughput research-object release queue snapshot and safe reorder proposals. | 4 | — |
| [`release_queue_scheduler`](crates/research/src/glioma/programs/p11_research_object_release/release_queue_scheduler.rs) | GAF-GLIOMA-P11-F31 | Fair, gate-preserving high-throughput scheduling for preclinical glioma releases. | 2 | — |
| [`release_signature_verifier`](crates/research/src/glioma/programs/p11_research_object_release/release_signature_verifier.rs) | GAF-GLIOMA-P11-F25 | Offline signature, provenance, and revocation verification for glioma research objects. | 3 | `release_gate`, `signed_attestation` |
| [`replay`](crates/research/src/glioma/programs/p11_research_object_release/replay.rs) | GAF-GLIOMA-P11-F10 | Autonomous reproducibility replay campaigns for preclinical glioma research. | 7 | — |
| [`reproducibility_score`](crates/research/src/glioma/programs/p11_research_object_release/reproducibility_score.rs) | GAF-GLIOMA-P11-F02 | Reproducibility-completeness scoring for preclinical glioma research objects. | 5 | `replay` |
| [`research_object_conformance_suite`](crates/research/src/glioma/programs/p11_research_object_release/research_object_conformance_suite.rs) | GAF-GLIOMA-P11-F26 | Pinned standards conformance for preclinical glioma research objects. | 3 | `archive_migration_adapter`, `release_signature_verifier` |
| [`research_object_exchange_api`](crates/research/src/glioma/programs/p11_research_object_release/research_object_exchange_api.rs) | GAF-GLIOMA-P11-F21 | Resumable, audience- and locality-gated exchange planning for preclinical glioma objects. | 2 | — |
| [`signed_attestation`](crates/research/src/glioma/programs/p11_research_object_release/signed_attestation.rs) | GAF-GLIOMA-P11-F08 | Fail-closed signed attestation planning for preclinical glioma research-object releases. | 5 | `release_gate` |
| [`version_retention_governor`](crates/research/src/glioma/programs/p11_research_object_release/version_retention_governor.rs) | GAF-GLIOMA-P11-F29 | Immutable-version retention planning for preclinical glioma research objects. | 2 | — |

Shared public feature modules owned outside this folder:

| Feature ID | Source module | Purpose | Direct test annotations |
|---|---|---|---:|
| `GAF-GLIOMA-P11-F01` | [`release`](crates/research/src/glioma/release.rs) | Portable research-object release preparation. | 2 |

---

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
- Autonomous stage bridge: `glioma_evidence_gated_stage_engine_federation_execute` executes the
  P12 consensus and bounded follow-up campaign from the P07 frontier using aggregate-only site
  artifacts. It preserves heterogeneous, negative, partial, unresolved, and blocked outcomes and
  requires consortium governance and independent validation before any transportability claim.
- Current implementation: 32/32 slots. Maintain with benchmark world versioning, adaptive site
  selection, and cross-consortium negative-result registries.

P12-F10's federated benchmark campaign uses a bounded deterministic portfolio beam rather than
greedy top-score selection. It rewards complementary action kinds and target-site coverage under
the aggregate-only budget, so a pair of affordable heterogeneity/coverage actions can beat one
expensive replicate when their combined information is stronger. Each worker invocation is charged
individually, including retries; actions skipped after a budget or executor stop are not billed.
`GliomaFederatedBenchmarkCampaign1@2` exposes the measured round spend and preserves the selected
portfolio, retry count, failed actions, and aggregate consensus for replay.

P12-F28's federated mechanism-transport campaign applies the same execution discipline to
cross-model generalization. A bounded portfolio beam rewards complementary model systems and
population signatures instead of repeatedly selecting the highest single action score; each local
attempt, including a retry, consumes declared budget, while actions skipped after a stop remain
unbilled. `GliomaFederatedMechanismTransportCampaign1@2` therefore reports observed transport,
heterogeneity, failed attempts, and measured spend without turning a projected transport gain into
an observed biological result.

P12-F05 `site_capability_envelope` compiles a site-local capability registry into a bounded
planner-facing envelope. Model systems, assay classes, standards, compute class, review capacity,
confidence, attestation freshness, approval, and local-only status are evaluated independently;
expired or unapproved capabilities become explicit omissions and no raw sample, credential, or
institutional identifier is exported.

P12-F06 `aggregate_phenotype_schema` harmonizes a site-local phenotype dictionary into a typed
aggregate summary. Exact, comparable, non-comparable, unmapped, suppressed, unit-conflicted, and
uncertain concepts remain separate; comparable mappings require an explicit pooling policy and
suppressed values can never be encoded as zero.

P12-F07 `site_provenance_attestation` binds a permitted aggregate contribution to site-local
source-lineage, analysis-version, calibration, environment-lock, policy, freshness, and signer-chain
evidence. Invalid, revoked, stale, protected, non-local, and non-aggregate contributions remain
blocked; only content-addressed metadata leaves the site.

P12-F08 `benchmark_execution_record` compiles an immutable, replayable federated benchmark run.
Benchmark and executor versions, policy scope, replay identity, quorum, admitted site attestations,
aggregate metric inputs, uncertainty digests, and every omitted site reason are content-addressed;
replay cannot silently change inclusion or invent a metric when quorum is not met.

P12-F16 `governance_cycle` compiles the consortium decision state machine across proposal, local site
review, privacy approval, analysis, dissent reconciliation, release, and correction. Authorized
transitions, explicit abstentions and rejections, quorum shortfalls, policy-version invalidation,
and next-review triggers are preserved without treating missing votes as approval.

P12-F03 `small_consortium_bias` is an aggregate-only bias assessment for small federated glioma
benchmarks. It computes the inverse-uncertainty site effect, a weighted robust median, deterministic
delete-one-site jackknife bias, a conservative bias bound, and a site-count sensitivity surface
under explicit extrapolation assumptions. Heterogeneity is now estimated with a bounded,
precision-aware Cochran-Q/I² calculation from permitted site effects rather than a magnitude-scaled
absolute-spread proxy; small differences can therefore remain heterogeneous when site uncertainty
is small, and null-centered disagreement cannot be hidden by a nonzero pooled effect.
underpowered site floors, material heterogeneity, high leave-one-site-out influence, and unstable
site-count assumptions remain negative or unresolved rather than being corrected into a confident
claim. The output is suitable for P10 replication interpretation and P12 benchmark follow-up, but
does not infer hidden site data, move raw records, or make a clinical decision. Permutation,
heterogeneity, underpowered, and corrected-null tests are included.
P12-F04 `heterogeneity_adaptive_benchmark_power` turns that uncertainty into a bounded portfolio
planner for glioma benchmark campaigns. It evaluates aggregate-only site envelopes across site
count and replicate sensitivity surfaces, adjusting information for attrition, modality coverage,
heterogeneity, privacy noise, cost, and claim width. The planner recommends a qualified site and
replicate portfolio when the conservative power proxy clears its gate, otherwise recommends adding
sites, increasing replicates, or narrowing the claim. It never reconstructs per-site records,
dispatches an instrument, or converts a speculative effect into a clinical conclusion.
P12-F11 `aggregate_anomaly_detector` audits permitted site summaries with robust median/MAD,
declared value bounds, temporal drift, protocol-version mismatch, uncertainty, and privacy
suppression checks. Each flagged contribution becomes a deterministic site-local review request;
the detector never infers an unobserved cause, automatically excludes a site, or turns a small,
suppressed consortium into a clean benchmark. Its disposition can block downstream aggregation
when suppression or quorum loss makes the anomaly assessment itself under-covered.
P12-F09 `site_selection_agent` selects eligible glioma research sites from bounded capability
envelopes with a deterministic portfolio beam. Complete representation coverage and the minimum
site floor are optimized jointly with compatibility, capacity, freshness, marginal independence,
and lower-cost ties; hard institution-group fraction gates are applied during expansion rather than
after selection. The `GliomaFederatedSiteSelectionPlan1@3` output preserves the selected portfolio,
per-site marginal scores, and a bounded-depth uncertainty marker for auditability. Capability,
model-system, freshness, capacity, cost, approval, revocation, and privacy failures remain
exclusions or unresolved states with reason codes. It produces a plan for the downstream power and
workflow controllers; it does not invite an institution or infer that capability evidence implies
biological quality.
P12-F15 `continual_benchmark_monitor` compares immutable aggregate benchmark windows in epoch
order, estimating adjacent drift and explicit change points while retaining uncertainty, site
coverage, and epoch gaps. Under-observed windows cannot certify stability; drift produces a
recalibration/rerun signal and never mutates prior snapshots or infers a hidden site cause.
P12-F31 `capacity_planner` forecasts usable site capacity, commitments, privacy-budget headroom,
latency, availability, and quorum resiliency over a bounded horizon. It selects only a minimum
quorum portfolio, reports at-risk sites and demand shortfalls, and keeps predicted capacity
separate from admitted execution. The output is a schedule proposal for the workflow controller,
not an automatic request or dispatch.
P12-F13 `dry_run_coordinator` compiles a synthetic/site-local federated benchmark preflight before
live coordination. It checks binding, schema, fixture availability, approval, locality, declared
failure modes, projected budget, and quorum per site, preserving omissions and negative evidence.
Every result is explicitly simulation-only and non-evidence: it cannot query an institution, move
raw data, create a benchmark job, or imply biological validity.
P12-F14 adds `multisite_benchmark_workflow`, a resumable event-driven coordinator for local
validation, site approval, aggregate query, reconciliation, review, and release. It keeps per-site
stage state and blockers, rejects conflicting duplicate event identities, exposes sequence
partitions and bounded retries, prevents denied approvals from unlocking queries, and removes late
withdrawals from active consensus. Terminal stage results and withdrawals are immutable against
delayed worker failures: stale failures remain explicitly ignored and blocked for audit without
reopening successful work, consuming retry capacity, or rewriting a withdrawn site. It coordinates
preclinical aggregate evidence only; it never dispatches an institution, moves raw data, or makes a
clinical decision.
P12-F19 adds `benchmark_director`, a prospective control-room algorithm for concurrent glioma
benchmarks. It reconciles run demand, quorum, budget, privacy spend, workload, anomaly, freshness,
uncertainty, approvals, and release readiness into one deterministic snapshot while keeping
operational completion separate from scientific success. It emits only bounded, approval-required
reallocation or review proposals; it cannot expand scope, dispatch a site, move raw data, or make
a clinical decision.
P12-F23 adds `benchmark_job_scheduler`, a durable event-stream state machine for resumable
aggregate-only benchmark jobs. It deduplicates identical retries, blocks conflicting idempotency
events, persists site checkpoints, enforces query concurrency, revalidates quorum, and preserves
partitioned, partial, cancelled, blocked, budget-exhausted, privacy-exhausted, and completed states
without treating job completion as scientific success. Event identity validation is applied even to
replayed duplicates, and per-site query counts represent completed aggregate results rather than
dispatch attempts, so quota and reconciliation consumers cannot be inflated by retries or malformed
events. MCP exposes a simulation-only route; local institution adapters retain all execution
authority.
P12-F18 adds `cross_site_evidence_explorer`, an aggregate-only researcher view over harmonized
glioma outcomes. It computes weighted cells with heterogeneity-aware uncertainty while keeping
privacy-suppressed, revoked, non-comparable, and low-confidence strata visible as non-values. The
explorer has an explicit indirect-query protection flag and cannot reconstruct site records or
silently turn missing mappings into zeros.
P12-F27 adds `quorum_admission`, the pre-query gate for aggregate federation. It verifies signer and
policy scope, schema and benchmark/assay/model conformance, freshness, locality, privacy budget,
duplication, and revocation. Correlated sites remain visible but cannot inflate independent quorum;
under-quorum and missing-model states are explicit, content-digested decisions rather than a silent
permission to query.
P12-F17 adds `site_participation`, a local PI/data-steward review workbench that renders the exact
purpose, aggregate fields, model/assay coverage, protocol version, workload, privacy cost, approval,
and withdrawal state. It can mark participation approved only after every declared boundary passes;
query dispatch remains false even for an approved review until the downstream quorum and scheduler
gates run.
P12-F25 adds `contribution_integrity`, a reusable trust gate that partitions aggregate contributions
into verified and rejected sets using signer identity, signature validity, policy scope, benchmark and
schema binding, freshness, revocation, locality, aggregate-only boundaries, and duplicate artifact
identity. Rejections carry stable reasons and a pinned trust-snapshot digest; unverified records cannot
be consumed by downstream statistics.
P12-F32 adds `federation_operations`, an operations-only consortium view of gateway availability,
heartbeat freshness, standards compatibility, maintenance, incidents, queue pressure, revocation, and
failover candidates. It keeps operational completion separate from research evidence, requires local
policy for failover, and exchanges no payloads or credentials.
P12-F21 adds `participant_api`, a versioned protocol seam for capability discovery, proposal review,
aggregate contribution, revocation, and receipt retrieval. Idempotency keys, action-specific content
digests, policy scope, revocation, and local PI/data-steward approval are evaluated before an exchange
can be accepted; duplicate exchanges never create a second operation.
P12-F22 adds `signed_aggregate_api`, the site-local submission boundary for aggregate results. It binds
an aggregate digest to benchmark/schema/policy identity, signer validity, calibration, provenance,
approval, revocation, privacy accounting, locality, and idempotency. Only an accepted submission is
eligible for downstream integrity and quorum gates; no raw result or credential is transported here.

Folder inventory: 33 source modules; 32/32 feature slots implemented. The program folder directly owns 32 feature modules; shared public feature facades live under `crates/research/src/glioma/`.

| Module | Kind / feature slot | Purpose | Direct test annotations | Imports |
|---|---|---|---:|---|
| [`adaptive_campaign`](crates/research/src/glioma/programs/p12_federated_benchmarking/adaptive_campaign.rs) | GAF-GLIOMA-P12-F26 | Adaptive federated benchmark planning and aggregate-only execution. | 2 | `campaign`, `consensus`, `site_planner` |
| [`aggregate_anomaly_detector`](crates/research/src/glioma/programs/p12_federated_benchmarking/aggregate_anomaly_detector.rs) | GAF-GLIOMA-P12-F11 | Aggregate-only anomaly detection for federated preclinical glioma benchmarks. | 5 | — |
| [`aggregate_phenotype_schema`](crates/research/src/glioma/programs/p12_federated_benchmarking/aggregate_phenotype_schema.rs) | GAF-GLIOMA-P12-F06 | Federated aggregate phenotype harmonization for preclinical glioma comparisons. | 5 | — |
| [`autonomous_stage_bridge`](crates/research/src/glioma/programs/p12_federated_benchmarking/autonomous_stage_bridge.rs) | workflow composition | P07 autonomous-engine bridge for the aggregate-only P12 federation cycle. | 1 | `campaign`, `consensus`, `operating_cycle` |
| [`benchmark_director`](crates/research/src/glioma/programs/p12_federated_benchmarking/benchmark_director.rs) | GAF-GLIOMA-P12-F19 | Prospective benchmark director snapshot and bounded reallocation proposals. | 5 | — |
| [`benchmark_execution_record`](crates/research/src/glioma/programs/p12_federated_benchmarking/benchmark_execution_record.rs) | GAF-GLIOMA-P12-F08 | Immutable, replayable federated benchmark execution records for glioma research. | 5 | — |
| [`benchmark_job_scheduler`](crates/research/src/glioma/programs/p12_federated_benchmarking/benchmark_job_scheduler.rs) | GAF-GLIOMA-P12-F23 | Durable, aggregate-only federated benchmark job state machine for glioma research. | 7 | — |
| [`campaign`](crates/research/src/glioma/programs/p12_federated_benchmarking/campaign.rs) | GAF-GLIOMA-P12-F10 | Closed-loop autonomous federated benchmark campaigns for preclinical glioma research. | 7 | `consensus` |
| [`capacity_planner`](crates/research/src/glioma/programs/p12_federated_benchmarking/capacity_planner.rs) | GAF-GLIOMA-P12-F31 | Continual capacity and quorum planning for federated preclinical glioma workflows. | 5 | — |
| [`consensus`](crates/research/src/glioma/programs/p12_federated_benchmarking/consensus.rs) | GAF-GLIOMA-P12-F01 | Robust, aggregate-only consensus for multi-site preclinical glioma benchmarks. | 3 | — |
| [`continual_benchmark_monitor`](crates/research/src/glioma/programs/p12_federated_benchmarking/continual_benchmark_monitor.rs) | GAF-GLIOMA-P12-F15 | Continual drift and calibration monitoring for federated preclinical glioma benchmarks. | 5 | — |
| [`contribution_integrity`](crates/research/src/glioma/programs/p12_federated_benchmarking/contribution_integrity.rs) | GAF-GLIOMA-P12-F25 | Contribution-integrity verification for aggregate-only federated glioma research. | 5 | — |
| [`cross_site_evidence_explorer`](crates/research/src/glioma/programs/p12_federated_benchmarking/cross_site_evidence_explorer.rs) | GAF-GLIOMA-P12-F18 | Aggregate-only cross-site evidence explorer for preclinical glioma benchmarks. | 5 | — |
| [`dry_run_coordinator`](crates/research/src/glioma/programs/p12_federated_benchmarking/dry_run_coordinator.rs) | GAF-GLIOMA-P12-F13 | Synthetic/site-local federated benchmark dry-run coordination. | 5 | — |
| [`federated_interpretation`](crates/research/src/glioma/programs/p12_federated_benchmarking/federated_interpretation.rs) | GAF-GLIOMA-P12-F30 | Federated, aggregate-only interpretation gate for preclinical glioma research. | 1 | `consensus` |
| [`federation_operations`](crates/research/src/glioma/programs/p12_federated_benchmarking/federation_operations.rs) | GAF-GLIOMA-P12-F32 | Consortium federation operations exchange for aggregate-only glioma research. | 5 | — |
| [`governance_cycle`](crates/research/src/glioma/programs/p12_federated_benchmarking/governance_cycle.rs) | GAF-GLIOMA-P12-F16 | Consortium benchmark-governance cycle for autonomous preclinical glioma federation. | 5 | — |
| [`heterogeneity_adaptive_power`](crates/research/src/glioma/programs/p12_federated_benchmarking/heterogeneity_adaptive_power.rs) | GAF-GLIOMA-P12-F04 | Heterogeneity-adaptive, privacy-aware federated benchmark power planning. | 5 | `consensus` |
| [`mechanism_transport`](crates/research/src/glioma/programs/p12_federated_benchmarking/mechanism_transport.rs) | GAF-GLIOMA-P12-F20 | Federated mechanistic transport analysis for preclinical glioma programs. | 4 | — |
| [`multisite_benchmark_workflow`](crates/research/src/glioma/programs/p12_federated_benchmarking/multisite_benchmark_workflow.rs) | GAF-GLIOMA-P12-F14 | Resumable multi-site benchmark workflow orchestration for `GAF-GLIOMA-P12-F14`. | 8 | `consensus` |
| [`operating_cycle`](crates/research/src/glioma/programs/p12_federated_benchmarking/operating_cycle.rs) | GAF-GLIOMA-P12-F24 | Aggregate-only federated benchmark operating cycle for preclinical glioma research. | 1 | `campaign`, `consensus` |
| [`participant_api`](crates/research/src/glioma/programs/p12_federated_benchmarking/participant_api.rs) | GAF-GLIOMA-P12-F21 | Versioned participant exchange for institution-local glioma federation agents. | 5 | — |
| [`power`](crates/research/src/glioma/programs/p12_federated_benchmarking/power.rs) | GAF-GLIOMA-P12-F02 | Aggregate-only federated benchmark power and sufficiency analysis. | 2 | `consensus` |
| [`quorum_admission`](crates/research/src/glioma/programs/p12_federated_benchmarking/quorum_admission.rs) | GAF-GLIOMA-P12-F27 | Conservative quorum admission for aggregate-only federated glioma benchmarks. | 5 | — |
| [`replication_transport`](crates/research/src/glioma/programs/p12_federated_benchmarking/replication_transport.rs) | GAF-GLIOMA-P12-F29 | Replication-to-federation handoff for preclinical glioma research. | 1 | `p10_interpretation_replication::validation_replication_campaign`, `p10_interpretation_replication::validation_replication_gate`, `transport_campaign` |
| [`signed_aggregate_api`](crates/research/src/glioma/programs/p12_federated_benchmarking/signed_aggregate_api.rs) | GAF-GLIOMA-P12-F22 | Idempotent signed aggregate-result submission for the glioma federation. | 5 | — |
| [`site_capability_envelope`](crates/research/src/glioma/programs/p12_federated_benchmarking/site_capability_envelope.rs) | GAF-GLIOMA-P12-F05 | Typed, privacy-preserving research-site capability envelopes for glioma federation planning. | 5 | — |
| [`site_participation`](crates/research/src/glioma/programs/p12_federated_benchmarking/site_participation.rs) | GAF-GLIOMA-P12-F17 | Local site participation review for aggregate-only federated glioma research. | 5 | — |
| [`site_planner`](crates/research/src/glioma/programs/p12_federated_benchmarking/site_planner.rs) | GAF-GLIOMA-P12-F12 | Influence-aware, aggregate-only planning for federated preclinical glioma benchmarks. | 3 | `consensus` |
| [`site_provenance_attestation`](crates/research/src/glioma/programs/p12_federated_benchmarking/site_provenance_attestation.rs) | GAF-GLIOMA-P12-F07 | Site-local provenance attestations for aggregate-only glioma federation. | 5 | — |
| [`site_selection_agent`](crates/research/src/glioma/programs/p12_federated_benchmarking/site_selection_agent.rs) | GAF-GLIOMA-P12-F09 | Explainable, policy-bounded site selection for federated preclinical glioma workflows. | 7 | — |
| [`small_consortium_bias`](crates/research/src/glioma/programs/p12_federated_benchmarking/small_consortium_bias.rs) | GAF-GLIOMA-P12-F03 | Small-consortium bias assessment for aggregate-only preclinical glioma benchmarks. | 5 | `consensus` |
| [`transport_campaign`](crates/research/src/glioma/programs/p12_federated_benchmarking/transport_campaign.rs) | GAF-GLIOMA-P12-F28 | Adaptive federated mechanism-transport campaign for preclinical glioma research. | 4 | `mechanism_transport` |

---

## Reading this map

A feature slot is counted only when a source file declares its stable `FEATURE_ID`; unidentified modules are listed as workflow composition and do not inflate feature coverage. Shared facades are called out separately. Test-annotation counts are inventory signals only: the crate test suite, behavioral assertions, negative-path tests, replay checks, and independent scientific validation are separate gates.

Regenerate and validate with `python tools/validate_glioma_organization.py --write-architecture`, then run the validator without that flag to detect drift.
