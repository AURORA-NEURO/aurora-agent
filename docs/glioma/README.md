# Glioma autonomous research engine

This directory is the source-level organization for the glioma product. It is intentionally
separate from the sibling `aurora-feature-atlas`: the atlas describes the full workspace portfolio;
this directory describes the executable glioma vertical and its ownership boundaries.

The engine is a preclinical research system. It can coordinate literature and local artifact
providers, multimodal quality control, molecular and mechanism analysis, experiment design,
protocol simulation, computation, replication, and research-object release. It cannot diagnose,
prognose, recommend treatment, triage care, enroll people, or accept human-subject/clinical-source
data.

## Folder map

```text
crates/research/src/glioma_engine.rs       cross-program plan, execution, retries, checkpoints
crates/research/src/glioma/
  mod.rs                                   public program API and ownership boundary
  catalog.rs                               12 programs × 32 feature slots = 384 product features
  evidence.rs                              P01 evidence qualification
  programs/p01_evidence_surveillance/evidence_cluster.rs
                                             P01 claim-scope evidence clustering, exact-artifact deduplication, and source-independence scoring
  programs/p01_evidence_surveillance/novelty_adjudication.rs
                                             P01 explicit baseline-vs-candidate novelty, replication, scope-extension, duplicate, and contradiction adjudication
  programs/p01_evidence_surveillance/evidence_stream.rs
                                             P01 prospective idempotent evidence stream snapshots with late-event handling, trend summaries, and negative frontiers
  programs/p01_evidence_surveillance/prospective_triage.rs
                                             P01 researcher-capacity-aware prospective evidence triage with contradiction, coverage, staleness, and budget routing
  programs/p01_evidence_surveillance/researcher_workbench.rs
                                             P01 local researcher evidence workbench with typed query filters, explainable ranking, facets, and omission handling
  programs/p01_evidence_surveillance/multimodal_workbench.rs
                                             P01 cross-study multimodal evidence panels with coverage, disagreement, representative selection, and gap routing
  programs/p01_evidence_surveillance/verification_gate.rs
                                             P01 local evidence verification gate for support, source independence, coverage, freshness, contradictions, and negative review
  programs/p01_evidence_surveillance/outcome_reconciliation.rs
                                             P01 aggregate-only multi-site outcome reconciliation with quorum, modality/model coverage, heterogeneity, influence, and routing gates
  programs/p01_evidence_surveillance/evidence_knowledge_bridge.rs
                                             P01 verification-to-P02 typed-knowledge handoff with claim alignment, omission accounting, and promotion gates
  programs/p01_evidence_surveillance/long_horizon_calibration.rs
                                             P01 epoch-window calibration, reliability drift, independent-group coverage, and review/acquisition routing
  programs/p01_evidence_surveillance/federated_outcome_transport.rs
                                             P01 aggregate-only outcome export with attestation, revocation, freshness, locality, and independent-quorum gates
  programs/p01_evidence_surveillance/federated_operating_cycle.rs
                                             P01 ranked autonomous federation cycle from transport, calibration, reconciliation, omissions, and negative-result signals
  programs/p01_evidence_surveillance/federated_batch_scheduler.rs
                                             P01 prospective high-throughput cycle scheduler with fairness, route quotas, dependency prefixes, and explicit capacity omissions
  programs/p01_evidence_surveillance/continual_promotion.rs
                                             P01 continual promotion/rollback control over replayed cycle outcomes, temporal windows, drift, negatives, and contradictions
  programs/p01_evidence_surveillance/federated_acquisition_policy.rs
                                             P01 consortium-aware site assignment with independence, quorum, budget, privacy, and local-raw-data gates
  programs/p01_evidence_surveillance/evidence_frontier_join.rs
                                             P01 claim-scope frontier join routing support, negatives, contradictions, and sparse evidence to executable next actions
  programs/p01_evidence_surveillance/multimodal_gap_router.rs
                                             P01 smallest cross-modality/model acquisition set for closing unresolved evidence context
  programs/p01_evidence_surveillance/acquisition_feedback.rs
                                             P01 idempotent site-local outcome assimilation into typed positive/negative frontier evidence
  programs/p01_evidence_surveillance/federated_execution_handoff.rs
                                             P01 approval-gated local adapter handoff with bounded autonomy, idempotency, and honest compensation
  programs/p02_evidence_knowledge/knowledge_graph.rs
                                             P02 scoped claim graph and support/contradiction synthesis
  programs/p02_evidence_knowledge/knowledge_drift.rs
                                             P02 prospective typed-knowledge snapshot drift and bounded downstream replanning
  programs/p02_evidence_knowledge/federated_knowledge.rs
                                             P02 aggregate-only multi-site typed-claim consensus with disagreement and influence gates
  programs/p02_evidence_knowledge/closure.rs
                                             P02 claim-to-evidence closure with modality/model/artifact coverage gates
  programs/p02_evidence_knowledge/study_alignment.rs
                                             P02 explicit multi-study claim alignment with pooled support and influence diagnostics
  programs/p02_evidence_knowledge/prospective_monitor.rs
                                             P02 sequential high-throughput claim drift, change-point, persistence, and multiplicity monitoring
  programs/p02_evidence_knowledge/federated_continual.rs
                                             P02 robust aggregate-only federated continual consensus with quorum and influence gates
  programs/p02_evidence_knowledge/continual_agent.rs
                                             P02 federated-continual autonomous action ranking with budget/dependency/autonomy gates
  programs/p02_evidence_knowledge/workflow_compile.rs
                                             P02 local dependency-wave workflow compiler with checkpoints, retries, and compensation metadata
  programs/p02_evidence_knowledge/multimodal_workflow.rs
                                             P02 adaptive multimodal workflow synchronization with study readiness, missingness barriers, and branch selection
  programs/p02_evidence_knowledge/workflow_admission.rs
                                             P02 execution-facing workflow admission with typed routes, local-data boundaries, and stop conditions
  programs/p02_evidence_knowledge/workflow_recovery.rs
                                             P02 checkpoint-aware recovery and resume planning with safe replay, bounded retry, and operator holds
  programs/p02_evidence_knowledge/action_outcome_assimilation.rs
                                             P02 idempotent action-outcome assimilation with explicit conflicts, failure retention, and evidence reconciliation
  programs/p02_evidence_knowledge/claim_experiment_closure.rs
                                             P02 claim-to-experiment closure with coverage scoring, negative-result retention, and next-action routing
  programs/p02_evidence_knowledge/claim_evidence_reconciliation.rs
                                             P02 belief promotion, retention, downgrade, and review gate after experimental closure
  programs/p02_evidence_knowledge/closed_loop_frontier.rs
                                             P02 deterministic budget-bounded promotion of reconciled claims into next research actions
  programs/p02_evidence_knowledge/prospective_belief_calibration.rs
                                             P02 prospective Brier/calibration scoring with omission-aware confidence updates
  programs/p02_evidence_knowledge/frontier_campaign.rs
                                             P02 campaign-level batching of frontier actions into budgeted, review-gated execution rounds
  programs/p02_evidence_knowledge/knowledge_protocol_gateway.rs
                                             P02 typed-knowledge capability negotiation with raw-data and clinical-boundary enforcement
  programs/p02_evidence_knowledge/multimodal_protocol_gateway.rs
                                             P02 multimodal typed-knowledge negotiation with explicit degraded modality branches
  programs/p02_evidence_knowledge/composition.rs
                                             P02 explicit relation graph composition with path bottlenecks and contradiction gates
  programs/p02_evidence_knowledge/belief_revision.rs
                                             P02 explicit-conflict maximal-consistency revision with rival-claim frontier
  programs/p02_evidence_knowledge/claim_frontier.rs
                                             P02 uncertainty/coverage/contradiction frontier prioritization
  programs/p02_evidence_knowledge/gap_compiler.rs
                                             P02 typed-knowledge frontier to P01 acquisition-candidate compiler
  programs/p02_evidence_knowledge/action_compiler.rs
                                             P02 typed-knowledge frontier to dependency-closed research-action compiler
  programs/p02_evidence_knowledge/action_bridge.rs
                                             P02 dependency-closed knowledge actions to local glioma selector candidates
  programs/p02_evidence_knowledge/selection_cycle.rs
                                             P02 knowledge-action bridge plus dependency-aware autonomous next-batch selection
  programs/p02_evidence_knowledge/dispatch.rs
                                             P02 selected knowledge-action execution with bounded retries, local evidence binding, and frontier recompilation
  programs/p02_evidence_knowledge/autonomous_cycle.rs
                                             P02-to-P01 autonomous gap compilation, portfolio planning, and local execution cycle
  programs/p02_evidence_knowledge/operating_cycle.rs
                                             P02 typed-knowledge synthesis, conflict revision, frontier ranking, and P01 handoff cycle
  programs/p03_multimodal_ingestion_qc/readiness_gate.rs
                                             P03 executed ingestion/QC to downstream research-surface admission
  programs/p03_multimodal_ingestion_qc/ingestion_manifest.rs
                                             P03 local, schema/version, duplicate, and modality/model admission before harmonization
  programs/p04_decision_context/context_replay.rs
                                             P04 epoch-aware decision-context replay with promotion, retirement, negative, and unresolved action partitions
  programs/p04_decision_context/decision_context_artifact.rs
                                             P04 portable typed decision-context artifact with consumer compatibility, action/effect metadata, semantic-loss, and replay guarantees
  programs/p04_decision_context/multi_study_context_artifact.rs
                                             P04 independent-study context alignment with quality/group quorum, typed conflicts, and namespaced negative/unknown partitions
  programs/p04_decision_context/multi_study_workflow.rs
                                             P04 dependency-closed multi-study action planning with local resource allocation, replication quorum, autonomy gates, and deterministic waves
  programs/p04_decision_context/federated_decision_context.rs
                                             P04 aggregate-only continual decision frontier with independent-site quorum, heterogeneity, influence, and negative-result gates
  programs/p04_decision_context/branch_evidence.rs
                                             P04 branch-outcome assimilation with contradiction, blocked/unobserved preservation, and deterministic next-frontier ranking
  programs/p05_mechanism_exploration/evidence_assimilation.rs
                                             P05 recency-weighted, contradiction-preserving mechanism evidence assimilation across study epochs
  programs/p05_mechanism_exploration/closed_loop.rs
                                             P05 assimilated mechanism evidence to bounded contradiction/uncertainty-aware next-action frontier
  programs/p05_mechanism_exploration/multi_fidelity_control.rs
                                             P05 transport-debt-aware model-system escalation controller with bounded approval and risk gates
  programs/p05_mechanism_exploration/feedback_replan.rs
                                             P05 observation-driven contradiction/uncertainty-aware mechanism frontier replanning
  programs/p05_mechanism_exploration/mechanism_workflow.rs
                                             P05 dependency-closed local mechanism workflow compiler with deterministic waves and policy gates
  programs/p05_mechanism_exploration/multi_study_workflow.rs
                                             P05 aggregate-only multimodal multi-study mechanism portfolio with replication and transport gates
  programs/p05_mechanism_exploration/prospective_controller.rs
                                             P05 rolling high-throughput mechanism admission with drift, retry, resource, queue, and budget gates
  programs/p05_mechanism_exploration/workflow_assurance.rs
                                             P05 local single-study mechanism workflow assurance with dependency replay, adversarial gates, and safe admission
  programs/p05_mechanism_exploration/fidelity_bridge.rs
                                             P05 cross-model prediction/observation residual transportability and low-fidelity frontier ranking
  programs/p05_mechanism_exploration/robustness_stress.rs
                                             P05 bounded adversarial stress surface for rank reversal, omission fragility, and mechanism stability
  programs/p03_multimodal_ingestion_qc/operating_cycle.rs
                                             P03 ingestion/QC, surface admission, and operator-handoff cycle
  programs/p07_protocol_simulation/scientific_frontier.rs
                                             P07 P02/P03-aware scientific frontier admission and next-batch selection
  programs/p07_protocol_simulation/frontier_execution.rs
                                             P07 admission-bound execution of only the scientifically runnable frontier batch
  programs/p04_decision_context/context_compiler.rs
                                             P04 evidence-gap to typed next-action compilation
  programs/p04_decision_context/admission_gate.rs
                                             P04 evidence, freshness, contradiction, effect, dependency, approval, preflight, and budget admission gate
  programs/p04_decision_context/value_optimizer.rs
                                             P04 bounded value-of-information portfolio beam search with uncertainty, contradiction, and diversity utility
  programs/p04_decision_context/value_calibration.rs
                                             P04 outcome-calibrated value forecasts with shrinkage, confidence, conflict, and negative-result handling
  programs/p04_decision_context/adaptive_controller.rs
                                             P04 calibrated exploitation/exploration controller with dependency-safe adaptive portfolio selection
  programs/p04_decision_context/decision_loop_governor.rs
                                             P04 sequential stopping and continuation policy from progress, failure, contradiction, negative-result, and budget signals
  programs/p04_decision_context/action_graph.rs
                                             P04 claim-path to dependency-closed action DAG and parallel waves
  programs/p04_decision_context/omission_certificate.rs
                                             P04 claim/modality/model closure certificate with explicit omissions and next actions
  programs/p04_decision_context/branch_planner.rs
                                             P04 scenario-aware branch-and-bound portfolio planning with Pareto frontier and uncertainty gates
  programs/p04_decision_context/branch_campaign.rs
                                             P04 bounded robust-branch execution, forecast-vs-observation drift scoring, and Pareto failover
  programs/p04_decision_context/action_bridge.rs
                                             P04 compiler-to-executable action portfolio bridge
  programs/p04_decision_context/campaign.rs
                                             P04 bounded question-to-action campaign with evidence-driven replanning
  programs/p04_decision_context/decision_cycle.rs
                                             P04 knowledge-to-context-to-DAG-to-robust-branch-to-campaign operating cycle
  programs/p04_decision_context/adaptive_branch_campaign.rs
                                             P04 evidence-returning branch execution with knowledge/context recompilation and frontier replanning
  programs/p05_mechanism_exploration/discrimination.rs
                                             P05 residual-likelihood mechanism discrimination and next-assay information gain
  programs/p01_evidence_surveillance/contradiction_cut.rs
                                             P01 weighted minimum-evidence cut for contradictory claims and replication routing
  programs/p05_mechanism_exploration/bayesian_update.rs
                                             P05 deterministic posterior update from typed local observations with coverage and contradiction gates
  programs/p05_mechanism_exploration/calibration.rs
                                             P05 prequential mechanism-probability calibration, reliability bins, and promotion gates
  programs/p05_mechanism_exploration/graph_propagation.rs
                                             P05 signed activation/inhibition mechanism-network propagation with convergence gates
  programs/p05_mechanism_exploration/counterfactual.rs
                                             P05 signed mechanism perturbation and downstream contrast
  programs/p05_mechanism_exploration/ensemble_counterfactual.rs
                                             P05 model-averaged counterfactual targets and agreement envelope
  programs/p05_mechanism_exploration/robust_portfolio.rs
                                             P05 lower-tail robust intervention portfolio optimizer across model ensembles
  programs/p05_mechanism_exploration/action_planner.rs
                                             P05 compiler from discriminator information gain to executable assay actions
  programs/p05_mechanism_exploration/discrimination_campaign.rs
                                             P05 bounded mechanism-discrimination campaign with measurement-driven replanning
  programs/p05_mechanism_exploration/clonal_evolution.rs
                                             P05 marker-aware preclinical clonal-evolution graph inference with explicit ambiguity
  programs/p05_mechanism_exploration/mechanism_dynamics.rs
                                             P05 signed delayed-feedback mechanism dynamics with intervention ranking, stability, oscillation, divergence, and sensitivity analysis
  programs/p05_mechanism_exploration/state_filter.rs
                                             P05 longitudinal finite-state mechanism filtering with transition priors, multimodal residuals, coverage uncertainty, and change points
  programs/p05_mechanism_exploration/state_smoother.rs
                                             P05 fixed-interval forward-backward smoothing with retrospective transition support, coverage, entropy, and negative features
  programs/p05_mechanism_exploration/consensus.rs
                                             P05 source-weighted consensus across imaging, pathway, clonal, and computational evidence with conflict and leave-one-source-out sensitivity
  programs/p05_mechanism_exploration/identifiability.rs
                                             P05 pairwise mechanism-identifiability frontier with quality/risk/budget-gated feature selection
  programs/p05_mechanism_exploration/invariance.rs
                                             P05 cross-model mechanistic invariance frontier with transport-stability and bounded panel selection
  programs/p06_experiment_design/adaptive_panel.rs
                                             P06 mechanism-aware multi-assay panel selection with Gini information gain, correlation-group diversity, risk, feasibility, and budget gates
    programs/p06_experiment_design/replication_plan.rs
                                             P06 multi-site replication topology with pooled effects, heterogeneity, power proxy, leave-one-site-out sensitivity, and budget/risk allocation
    programs/p06_experiment_design/replication_continuation.rs
                                             P06 observation-driven replication-wave continuation with quality, stopping, and negative-result gates
  programs/p06_experiment_design/replication_protocol.rs
                                             P06-to-P07 compiler for site setup, arm, QC, and deterministic protocol preflight
  programs/p06_experiment_design/mechanism_validation.rs
                                             P05 robust intervention portfolio to P06 power-aware sequential validation compiler
  programs/p06_experiment_design/mechanism_validation_protocol.rs
                                             P06 validation decisions to deterministic P07 local protocol preflight compiler
  programs/p06_experiment_design/validation_batch_assessment.rs
                                             P06 measured validation batch pooling and next-look power/stopping reassessment
  programs/p06_experiment_design/validation_campaign.rs
                                             P05→P06→P07 closed-loop validation campaign with observation-driven replanning
  programs/p10_interpretation_replication/validation_replication_gate.rs
                                             P10 efficacy-gated handoff from local validation to independent-site replication planning and protocol preflight
  programs/p10_interpretation_replication/validation_replication_campaign.rs
                                             P10 guarded execution handoff into site replication, meta-analysis, transportability, and next-action control
  programs/p12_federated_benchmarking/replication_transport.rs
                                             P12 replication-to-federation bridge that admits only validated aggregate site summaries into mechanism transport
  programs/p10_interpretation_replication/replication_closure_frontier.rs
                                             P10 ranked scientific closure frontier after independent-site replication with explicit negative and qualified holds
  programs/p10_interpretation_replication/replication_closure_execution.rs
                                             P10 selected closure-frontier execution through a bounded replication campaign worker seam
  programs/p10_interpretation_replication/replication_closure_campaign.rs
                                             P10 multi-round closure campaign with global budget, terminal stops, and replayable progress
  programs/p10_interpretation_replication/closure_interpretation.rs
                                             P10 closure-campaign replication summaries into cross-family uncertainty-aware interpretation
  programs/p12_federated_benchmarking/federated_interpretation.rs
                                             P12 aggregate-only consortium consensus aligned with local closure interpretation
  programs/p07_protocol_simulation/mechanism_validation_execution.rs
                                             P07 compiled validation protocol execution through a bounded institution-local worker seam
  programs/p05_mechanism_exploration/operating_cycle.rs
                                             P05 mechanism-discrimination campaign to typed next-assay operating cycle
  programs/p05_mechanism_exploration/calibrated_campaign.rs
                                             P05 calibration-gated adaptive mechanism policy with trust-discounted effects and exploration debt
  programs/p07_protocol_simulation/mechanism_autopilot.rs
                                             P07 graph/pathway-gated mechanism execution loop with bounded replanning and outcome retirement
  programs/p07_protocol_simulation/program_scheduler.rs
                                             P07 high-throughput multi-intent scheduler with scientific utility, fairness debt, and resource-capacity admission
  programs/p07_protocol_simulation/mechanism_discovery_engine.rs
                                             P07 autonomous multimodal-to-dynamics-to-robust-intervention mechanism discovery and gated assay execution
  programs/p12_federated_benchmarking/transport_campaign.rs
                                             P12 aggregate-only mechanism transport campaign with heterogeneity-aware site follow-up and recomputation
  programs/p06_experiment_design/clonal_panel.rs
                                             P06 clone-aware perturbation/readout panel selection under cost and branch-coverage gates
  programs/p06_experiment_design/contrast_design.rs
                                             P06 balanced factorial contrast-panel compiler with interaction and budget gates
  programs/p06_experiment_design/adaptive_dose_surface.rs
                                             P06 uncertainty-aware adaptive combination dose-surface acquisition planning
  programs/p06_experiment_design/sequential_design.rs
                                             P06 sequential Bayesian interim stopping and bounded next-round allocation with success/futility gates
  programs/p06_experiment_design/power_reestimation.rs
                                             P06 variance-aware adaptive power re-estimation with interim alpha spending and explicit boundaries
  programs/p06_experiment_design/sequential_campaign.rs
                                             P06 autonomous sequential campaign execution with local aggregate batches and posterior replanning
  programs/p06_experiment_design/frontier_controller.rs
                                             P06 multi-objective frontier controller for information gain, power, diversity, fidelity, risk, cost, and bounded replanning
  programs/p06_experiment_design/operating_cycle.rs
                                             P06 plan-to-local-execution-to-replan experiment operating cycle
  programs/p10_interpretation_replication/claim_adjudication.rs
                                             P10 four-gate causal claim adjudication with confounding, replication, heterogeneity, and next-evidence actions
  programs/p10_interpretation_replication/clone_outcomes.rs
                                             P10 replicate-level clone-panel outcome adjudication with explicit null/contradictory evidence
  programs/p10_interpretation_replication/dynamic_policy.rs
                                             P10 longitudinal off-policy evaluation for competing preclinical experiment workflows
  programs/p01_evidence_surveillance/surveillance.rs
                                             P01 snapshot delta surveillance and prioritized evidence review actions
  programs/p01_evidence_surveillance/temporal_shift.rs
                                             P01 prospective weighted baseline/recent shift and reversal detection with explicit under-observation and heterogeneity
  programs/p01_evidence_surveillance/federated_shift.rs
                                             P01 aggregate-only multi-site evidence-shift consensus with heterogeneity and leave-site-out influence gates
  programs/p01_evidence_surveillance/priority.rs
                                             P01 recency/state/coverage action queue for the next autonomous cycle
  programs/p01_evidence_surveillance/acquisition.rs
                                             P01 dependency-closed, source-diverse evidence-acquisition portfolio optimizer
  programs/p01_evidence_surveillance/acquisition_campaign.rs
                                             P01 dependency-safe acquisition execution with retries, budget, and honest outcomes
  programs/p01_evidence_surveillance/campaign.rs
                                             P01 bounded autonomous evidence refresh and round-by-round surveillance replanning
  programs/p01_evidence_surveillance/calibration.rs
                                             P01 quality-weighted isotonic source calibration and review-frontier generation
  programs/p01_evidence_surveillance/triangulation.rs
                                             P01 cross-family claim triangulation with contradiction and source-dominance gates
  programs/p01_evidence_surveillance/operating_cycle.rs
                                             P01 intent-to-evidence portfolio, local acquisition, and operator-handoff cycle
  programs/p02_evidence_knowledge/campaign.rs
                                             P02 bounded claim-resolution campaign with frontier actions and knowledge recompilation
  programs/p03_multimodal_ingestion_qc/campaign.rs
                                             P03 bounded metadata-only ingestion/QC campaign with defect-aware replanning
  programs/p08_instrument_robotics/calibration.rs
                                             P08 robust control calibration and Theil-Sen instrument drift detection
  programs/p08_instrument_robotics/signal_extraction.rs
                                             P08 local median-baseline signal extraction with quality, drift, noise, and peak gates
  programs/p08_instrument_robotics/batch_stability.rs
                                             P08 prospective cross-run endpoint stability with coverage, dispersion, noise, peak, and drift gates
  programs/p08_instrument_robotics/federated_consensus.rs
                                             P08 aggregate-only cross-site instrument endpoint consensus with privacy and influence gates
  programs/p06_experiment_design/adaptive_allocation.rs
  programs/p06_experiment_design/adaptive_allocation_campaign.rs
                                             P06 bounded posterior-aware replicate-allocation campaign with local batch execution and replanning
  programs/p06_experiment_design/information_design.rs
  programs/p06_experiment_design/adaptive_information_campaign.rs
                                             P06 conservative Beta-posterior sequential assay allocation and budgeted exploration
  multimodal.rs                            P03 harmonization and QC
  mechanism.rs                             P05 competing mechanism portfolio
  experiment.rs                             P06 fixed-point power and allocation design
  analysis.rs                               P10 effect, uncertainty, permutation analysis
  replication.rs                            P10 cross-site robustness and null results
  release.rs                                P11 portable research-object preparation
  programs/
    p01_evidence_surveillance/              program owner and route
    p02_evidence_knowledge/
    p03_multimodal_ingestion_qc/
    p04_decision_context/
    p05_mechanism_exploration/
    p06_experiment_design/
    p07_protocol_simulation/
    p08_instrument_robotics/
    p09_reproducible_computation/
    p10_interpretation_replication/
                                             P10 longitudinal, causal contrast, and replication analysis
    p11_research_object_release/
    p11_research_object_release/release_gate.rs
                                             P11 replay-, provenance-, and review-aware release gate for accountable signing
    p11_research_object_release/multimodal_bundle.rs
                                             P11 modality-coverage, semantic-loss, provenance-closure, and cross-modal alignment compiler
    p11_research_object_release/migration.rs
                                             P11 schema migration planner with lossless rewrites, explicit recomputation, and fail-closed compatibility checks
    p11_research_object_release/dependency_closure.rs
                                             P11 transitive artifact/program closure analysis with cycle, orphan, depth, and coverage gates
    p11_research_object_release/operating_cycle.rs
                                             P11 manifest replay, release gating, and accountable operator handoff
    p12_federated_benchmarking/
    p12_federated_benchmarking/mechanism_transport.rs
                                             P12 aggregate-only cross-model mechanism transport and fragility analysis
    p12_federated_benchmarking/site_planner.rs
                                             P12 conservative influence-aware consortium expansion and site portfolio planning
    p12_federated_benchmarking/adaptive_campaign.rs
                                             P12 planner-to-campaign bridge preserving projected versus observed federated outcomes
    p12_federated_benchmarking/operating_cycle.rs
                                             P12 aggregate boundary, consensus, campaign, and governance handoff
  workflow.rs                               P07 adaptive campaign planner and guarded execution
    p07_protocol_simulation/simulator.rs    P07 deterministic resource-constrained scheduling
    p07_protocol_simulation/scenario_ensemble.rs
                                             P07 probability-weighted robustness simulation across timing, capacity, risk, and approval perturbations
    p07_protocol_simulation/execution.rs   P07 guarded local protocol execution with retries
    p07_protocol_simulation/compensation.rs
                                             P07 contract-preserving recovery planning for failed, partial, and skipped protocol tasks
    p07_protocol_simulation/branch_optimizer.rs
                                             P07 deterministic beam search over information/feasibility/risk/time/cost protocol branches
    p07_protocol_simulation/autonomous_protocol.rs
                                             P07 bounded branch-select/execute/compensate autonomous protocol controller
    p07_protocol_simulation/evidence_surface.rs
                                             P07 robust endpoint evidence compilation with quality, uncertainty, negative, and contradiction gates
    p07_protocol_simulation/multistudy_fusion.rs
                                             P07 cross-study, model-system, and modality evidence fusion with heterogeneity and contradiction routing
    p07_protocol_simulation/transport_gate.rs
                                             P07 target-model transport gate with site/model support, information, heterogeneity, and negative-result preservation
    p07_protocol_simulation/action_execution.rs
                                             P07 dependency-safe action-portfolio execution
    p07_protocol_simulation/autonomous_campaign.rs
                                             P07 observation-driven campaign replanning over local actions
    p07_protocol_simulation/research_autopilot.rs
                                             P07 context-to-selection-to-execution autonomous cycle
    p07_protocol_simulation/mechanism_campaign.rs
                                             P07 multimodal graph-to-pathway-to-action mechanism campaign
    p07_protocol_simulation/evidence_campaign.rs
                                             P07 execution bridge from evidence-priority queue to local adapters
    p07_protocol_simulation/active_learning_campaign.rs
                                             P07 bounded autonomous assay execution and observation-driven replanning
    p07_protocol_simulation/robust_active_learning_campaign.rs
                                             P07 ensemble-guided autonomous rounds with lower-tail safety gates
    p07_protocol_simulation/mission.rs
                                             P07 science-aware cross-program mission control with stage/model/modality frontier adaptation
    p07_protocol_simulation/director.rs
                                             P07 high-level focus-aware research director compiling intent into dependency-closed executable batches
    p07_protocol_simulation/autonomous_engine.rs
                                             P07 end-to-end multi-cycle autonomous research engine with artifact-driven replanning
  p07_protocol_simulation/program_cycle.rs
                                             P07 stage-gated autonomous program control and operator handoff
    p07_protocol_simulation/mission_recovery.rs
                                             P07 failed-frontier recovery with dependency-cone closure and alternate mission dispatch
    p07_protocol_simulation/intent_mission.rs
                                             P07 intent-to-stage-action compilation and autonomous mission execution
    p07_protocol_simulation/multimodal_mission.rs
                                             P07 bounded modality/model-system portfolio expansion and coverage-aware mission execution
    p07_protocol_simulation/adaptive_scheduler.rs
                                             P07 outcome-aware dependency scheduler with beam search, risk budgets, and preserved negative evidence
    p07_protocol_simulation/evidence_gate.rs
                                             P07 evidence-gated director admission from P01 cross-family triangulation
    p07_protocol_simulation/clone_continuation.rs
                                             P07 clone-outcome-driven continuation planning with dependency closure and policy gates
    p07_protocol_simulation/clone_campaign.rs
                                             P07 autonomous evolution-to-perturbation-to-replicate-outcome clone campaign
    p08_instrument_robotics/preflight.rs   P08 typed instrument/robotics interlock planning
    p08_instrument_robotics/execution.rs   P08 guarded execution with live rechecks and emergency stop
    p08_instrument_robotics/recovery.rs    P08 deterministic recovery planning for partial, failed, blocked, unresolved, and negative runs
    p08_instrument_robotics/campaign.rs   P08 ordered multi-run instrument campaign with fail-closed safety halts
    p08_instrument_robotics/fleet_scheduler.rs
                                             P08 multi-instrument dependency scheduler with calibration, operator, deadline, utilization, and risk gates
    p08_instrument_robotics/fleet_execution.rs
                                             P08 schedule-bound fleet execution with dependency-safe guarded gateway handoff
    p08_instrument_robotics/assay_adjudication.rs
                                             P08 typed assay/QC adjudication that separates hardware completion from biological evidence
    p08_instrument_robotics/operating_cycle.rs
                                             P08 fail-closed preflight barrier, instrument campaign execution, and operator handoff
    p08_instrument_robotics/adaptive_campaign.rs
                                             P08 information-per-cost, endpoint-diverse instrument portfolio selection with dependency-closed guarded execution
    p08_instrument_robotics/science_loop.rs
                                             P08 governed instrument-to-science loop with assay evidence adjudication and next research actions
    p08_instrument_robotics/research_frontier.rs
                                             P08 status-aware handoff from qualified, negative, and unresolved assay outcomes into executable computation, replication, and falsification missions
    p08_instrument_robotics/signal_extraction.rs
                                             P08 bounded value-only endpoint extraction for local instrument traces before assay adjudication
    p08_instrument_robotics/batch_stability.rs
                                             P08 high-throughput reproducibility gate for extracted endpoints before autonomous promotion
    p08_instrument_robotics/multichannel_concordance.rs
                                             P08 bounded integer-lag multichannel alignment with fixed-point concordance and residual gates
    p08_instrument_robotics/federated_consensus.rs
                                             P08 aggregate-only federation of stable endpoints with heterogeneity and leave-site-out sensitivity
    p09_reproducible_computation/robustness.rs
                                             P09 leave-one-batch/row-out robustness battery
    p09_reproducible_computation/execution.rs
                                             P09 replayable multimodal computation DAG execution
    p09_reproducible_computation/reproducibility.rs
                                             P09 repeated-run reproducibility gate for deterministic digests, numerical drift, runtime drift, and task coverage
    p09_reproducible_computation/lineage.rs
                                             P09 artifact-lineage join and dependency-safe recomputation frontier for failed, stale, and negative computation nodes
    p09_reproducible_computation/planning.rs
                                             P09 budgeted computation-portfolio planning with prerequisite closure
    p09_reproducible_computation/portfolio_execution.rs
                                             P09 autonomous portfolio-to-computation execution bridge
    p09_reproducible_computation/campaign.rs
                                             P09 bounded multi-round computation campaign with typed replanning
    p09_reproducible_computation/workflow.rs
                                             P09 intent-to-DAG compiler for modality-aware autonomous computation
    p09_reproducible_computation/placement.rs
                                             P09 locality-aware worker placement with replay-valid cache and transfer gates
    p09_reproducible_computation/operating_cycle.rs
                                             P09 intent-to-DAG compilation, resource gating, local computation campaign, and operator handoff
    p09_reproducible_computation/recovery_campaign.rs
                                             P09 failed-frontier DAG closure, selective cache invalidation, and fresh-identity recovery execution
    p09_reproducible_computation/robustness_guided.rs
                                             P09 robustness-debt-driven re-analysis frontier with typed omission targeting and dependency-safe execution
    p10_interpretation_replication/trajectory.rs
                                             P10 longitudinal per-unit trajectory analysis
    p10_interpretation_replication/transportability.rs
                                             P10 model-system transportability and portability-gap analysis
    p10_interpretation_replication/campaign.rs
                                             P10 autonomous replication/interpretation campaign and next-action executor
    p10_interpretation_replication/state_transition.rs
                                             P10 discrete-state transition matrices and treatment contrasts
  p10_interpretation_replication/causal_contrast.rs
                                             P10 exact pre/post difference-in-differences analysis
  p10_interpretation_replication/mediation.rs
                                             P10 mediator/direct/indirect effect decomposition with influence bounds
  p10_interpretation_replication/causal_adjustment.rs
                                             P10 stratified overlap-adjusted effect and leave-one-stratum influence
  p10_interpretation_replication/sensitivity.rs
                                             P10 hidden-confounding sensitivity sweep and causal tipping-point bounds
    p10_interpretation_replication/meta_analysis.rs
                                             P10 inverse-uncertainty replication meta-analysis and influence bounds
    p10_interpretation_replication/synthesis.rs
                                             P10 cross-family interpretation gate with contradiction and leave-one-family-out stability
    p10_interpretation_replication/adaptive_frontier.rs
                                             P10 outcome-conditioned next-action frontier for contradiction, replication, evidence gaps, and negative results
    p10_interpretation_replication/adaptive_execution.rs
                                             P10 guarded execution of selected interpretation frontiers with explicit unresolved holds and typed action outcomes
    p10_interpretation_replication/adaptive_campaign.rs
                                             P10 bounded synthesis-to-frontier-to-execution loop with caller-owned evidence replanning
    p10_interpretation_replication/operating_cycle.rs
                                             P10 cross-family interpretation gate to adaptive research frontier and operator handoff
    p12_federated_benchmarking/consensus.rs
                                             P12 aggregate-only multi-site benchmark consensus with robust pooling and influence bounds
    p12_federated_benchmarking/power.rs
                                             P12 aggregate-only benchmark sufficiency gate with information, conservative power, heterogeneity, and influence analysis
    p12_federated_benchmarking/campaign.rs
                                             P12 autonomous aggregate-only benchmark follow-up campaign with deterministic replanning
    p12_federated_benchmarking/operating_cycle.rs
                                             P12 aggregate-only consensus preflight, follow-up execution, and governance handoff
    p11_research_object_release/replay.rs
                                             P11 dependency-aware reproducibility replay campaign and release-readiness gate
    p06_experiment_design/dose_response.rs   P06 monotone dose-response curve analysis
    p06_experiment_design/synergy.rs         P06 Bliss combination-response analysis
    p06_experiment_design/campaign.rs       P06 mechanism-aware closed-loop assay campaign controller and executor seam
    p06_experiment_design/information_design.rs P06 integer Bayesian assay selection by expected mechanism-information reduction
    p06_experiment_design/robust_design.rs P06 maximin scenario-aware replicate allocation with diminishing utility and explicit risk/feasibility gates
    p06_experiment_design/blocked_randomization.rs P06 confounding-aware blocked randomization planner with nuisance-stratum balance, variance-aware information gain, and explicit capacity/risk gates
    p06_experiment_design/power_stress_surface.rs P06 prospective power-stress surface across effect, variance, attrition, risk, and budget worlds
    p06_experiment_design/carryover_sequence.rs P06 directed carryover-aware assay sequence design with transition penalties and bounded information utility
    p06_experiment_design/adaptive_information_campaign.rs P06 closed-loop posterior updating and re-planning through a caller-owned assay executor
    p06_experiment_design/adaptive_allocation_campaign.rs P06 autonomous Beta-posterior replicate allocation with aggregate batch validation and posterior replanning
    p06_experiment_design/multi_fidelity.rs P06 cost-aware multi-fidelity surrogate optimization across screening, mechanistic, and validation models
    p06_experiment_design/active_learning.rs P06 uncertainty-aware kernel active learning for next-assay selection
    p06_experiment_design/robust_active_learning.rs
                                             P06 model-ensemble lower-tail active learning under disagreement
    p06_experiment_design/multi_fidelity_campaign.rs
                                             P06 closed-loop screening-to-mechanistic/validation campaign with transfer calibration gates
    p03_multimodal_ingestion_qc/concordance.rs
                                             P03 feature-level modality concordance analysis
  p03_multimodal_ingestion_qc/consensus.rs
                                             P03 deterministic multimodal sample consensus clustering
  p03_multimodal_ingestion_qc/harmonization.rs
                                             P03 robust per-modality batch harmonization with explicit correction gates
  p03_multimodal_ingestion_qc/latent_factors.rs
                                             P03 robust complete-case multimodal latent-state factorization with convergence and reconstruction gates
  p03_multimodal_ingestion_qc/graph_fusion.rs
                                             P03 reliability-weighted multimodal sample graph fusion with dropout, contradiction, and bounded diffusion gates
  p03_multimodal_ingestion_qc/dropout_stress.rs
                                             P03 modality-dropout stress analysis with no-imputation stability, contradiction, and acquisition routing
  p03_multimodal_ingestion_qc/missingness_audit.rs
                                             P03 sample-by-modality missingness topology, correlated dropout detection, and deterministic reacquisition planning
  p03_multimodal_ingestion_qc/reliability_calibration.rs
                                             P03 replicate-aware modality reliability, leave-one-out instability, and QC admission gating
  p03_multimodal_ingestion_qc/modality_portfolio.rs
                                             P03 bounded reliability- and budget-aware endpoint modality portfolio selection with explicit coverage debt
  p03_multimodal_ingestion_qc/drift_surveillance.rs
                                             P03 continuous modality/metric QC drift surveillance with recalibration gates and ordered follow-up
  p03_multimodal_ingestion_qc/evidence_fusion.rs
                                             P03 reliability/uncertainty-weighted endpoint evidence fusion with contradiction and missing-modality gates
  p03_multimodal_ingestion_qc/sensitivity.rs
                                             P03 leave-one-modality-out and bounded perturbation endpoint fragility analysis with autonomous reacquisition routing
  p03_multimodal_ingestion_qc/decision_gate.rs
                                             P03 uncertainty-aware endpoint threshold gate with explicit autonomous research handoff and negative-result routing
  p03_multimodal_ingestion_qc/contradiction_adjudication.rs
                                             P03 pairwise multimodal contradiction adjudication with trust asymmetry, rival-evidence retention, and orthogonal-resolution routing
  p03_multimodal_ingestion_qc/prospective_quality.rs
                                             P03 prospective modality-quality forecasting with preventive preflight and reacquisition scheduling
  p03_multimodal_ingestion_qc/quality_scheduler.rs
                                             P03 budget-, duration-, deadline-, and forecast-risk-aware modality acquisition scheduling
  p03_multimodal_ingestion_qc/quality_execution.rs
                                             P03 approval-bound execution of the selected quality schedule with retries and quality-floor gates
  p03_multimodal_ingestion_qc/quality_adaptive_campaign.rs
                                             P03 closed-loop quality campaign that assimilates QC outcomes and replans bounded acquisition rounds
  p03_multimodal_ingestion_qc/quality_transport.rs
                                             P03 cross-study/model QC-policy transport calibration with target-local confirmation gates
  p03_multimodal_ingestion_qc/quality_root_cause.rs
                                             P03 measurement-process QC incident attribution with contradiction, missing-evidence, and remediation gates
  p03_multimodal_ingestion_qc/quality_remediation.rs
                                             P03 budgeted approval-aware remediation planning from QC causes to local recovery actions
  p03_multimodal_ingestion_qc/quality_recovery.rs
                                             P03 paired baseline/post-remediation recovery verification with proceed, iterate, and escalate gates
  p03_multimodal_ingestion_qc/spatial_niche.rs
                                             P03 spatial neighbourhood graph, same-lineage niche components, and cross-lineage enrichment
  p03_multimodal_ingestion_qc/spatial_communication.rs
                                             P03 spatial ligand-receptor communication enrichment against lineage-marginal null
  p03_multimodal_ingestion_qc/spatial_propagation.rs
                                             P03 lineage-aware integer spatial-state diffusion and hotspot prioritisation
  p03_multimodal_ingestion_qc/spatial_registration.rs
                                             P03 robust cross-sample lineage-landmark registration with residual and coverage gates
  p03_multimodal_ingestion_qc/temporal_fusion.rs
                                             P03 longitudinal multimodal state-transition inference with explicit missing-timepoint and modality gates
  p03_multimodal_ingestion_qc/temporal_spatial_alignment.rs
                                             P03 declared temporal-to-spatial state alignment with coverage, gap, and follow-up gates
    p05_mechanism_exploration/pathway_activity.rs
                                             P05 signed pathway activity inference with cross-modal confidence and bottleneck gates
    p05_mechanism_exploration/adaptive_policy.rs
                                             P05 finite-horizon model-uncertainty policy with Gini information gain and local execution loop
    p05_mechanism_exploration/intervention_value.rs
                                             P05 posterior-weighted perturbation value engine with uncertainty, risk, cost, feasibility, and redundancy gates
```

`docs/glioma/organization.json` is the machine-readable version of this map. The runtime
`generate_feature_catalog()` function and that file must agree on program ids, folders, and the
8-archetype × 4-scale expansion.

Run `python tools/validate_glioma_organization.py --json` before adding a capability. The validator
checks that every P01–P12 source folder has its `mod.rs` ownership boundary, that the organization
cardinality remains 12 × 32 = 384, and that the implementation manifest contains stable feature
ids assigned to the correct program. Its report is the folder-by-folder handoff between the
portfolio plan and executable code; it does not promote a planned slot to implemented status.

## Program order

| Program | Product owner | Engine stages | Observable product result |
| --- | --- | --- | --- |
| P01 Evidence surveillance | evidence curator | evidence surveillance | snapshot deltas, deterministic novelty radar, recency/state/coverage action queues, dependency-closed evidence-acquisition portfolios, source calibration, cross-family claim triangulation, local and multimodal researcher evidence workbenches, evidence verification gates, researcher-capacity-aware prospective triage, aggregate-only federation transport, ranked autonomous federation cycles, high-throughput batch scheduling, continual promotion/rollback control, review/revalidation actions, autonomous intent-to-evidence execution cycles, and stale/unknown/contradictory coverage |
| P02 Evidence-to-typed-knowledge | knowledge engineer | evidence compilation | scoped claims, contradiction-aware consistency closure, explicit multi-study alignment and influence diagnostics, prospective change-point monitoring with multiplicity control, robust aggregate-only federated continual consensus, autonomous evidence-to-action ranking with budget/dependency/autonomy gates, local dependency-wave workflow compilation with checkpoint/compensation planning, maximal-consistency portfolios, ranked rival frontiers, typed frontier-to-acquisition candidate compilation, dependency-closed validation/replication action compilation, autonomous P02-to-P01 gap cycles, a complete knowledge-synthesis operating cycle, and competing explanations bound to source artifacts |
| P03 Multimodal ingestion and QC | data steward | multimodal ingestion/QC | comparable cells, robust batch harmonization, feature-level concordance, consensus clusters, spatial niches, ligand-receptor communication, cross-sample registration, spatial-state diffusion, explicit defects, downstream research-surface admission, and an executable QC-to-handoff operating cycle |
| P04 Question-to-decision context | principal investigator | intent normalization, context compilation | bounded decision context, portable and multi-study typed context artifacts, dependency-closed replicated task waves, local resource allocation, autonomy/approval gates, scenario-aware Pareto workflow branches, branch execution with forecast-drift failover, aggregate-only federated branch consensus, evidence-returning adaptive replanning, full operating-cycle execution, selected action batches, and unresolved omissions |
| P05 Mechanism exploration | mechanism scientist | molecular landscape, mechanism exploration | residual-fit competing mechanisms, pairwise identifiability analysis with quality/risk/budget-gated feature selection, cross-model mechanistic invariance and transport-stable panel selection, calibrated trust-discounted posterior action selection, posterior-weighted next-assay information gain, signed mechanism-network propagation, delayed-feedback mechanism dynamics, model-averaged counterfactuals, robust lower-tail intervention portfolios, discriminating campaigns, observation-driven feedback replanning, dependency-closed workflow compilation, local single-study workflow assurance, multimodal multi-study portfolio compilation, prospective high-throughput control, and an end-to-end next-assay operating cycle |
| P06 Power-aware experiment design | experimentalist | experiment design | falsifiable allocation, power, blocking, dose-response, adaptive replicate allocation, sequential Bayesian success/futility stopping, local sequential campaign execution, uncertainty-aware dose-surface acquisition, mechanism-aware closed-loop campaign rounds, an end-to-end plan/execute/replan cycle, combination-synergy fitting, and null-result plan |
| P07 Protocol simulation | lab operations lead | protocol simulation, adaptive workflow planning | critical-path scheduling, outcome-aware dependency scheduling, intent-to-stage-action compilation, bounded modality/model-system portfolio expansion, evidence-gated director admission, evidence-priority execution cycles, context-to-action execution, multimodal mechanism campaigns, evolution-aware clone campaigns, stage-gated autonomous program control, failed-frontier recovery with alternate dependency-safe missions, P02/P03-aware scientific frontier admission, utilization, deterministic next batches, and repair/abstain routing before physical effects |
| P08 Instrument and robotics preflight | instrument operator | instrument preflight | robust control calibration, local median-baseline signal extraction, prospective cross-run endpoint stability, aggregate-only cross-site consensus, drift/noise/peak quality gates, information-per-cost endpoint-diverse campaign selection, multi-instrument dependency scheduling, schedule-bound fleet execution, signed interlocked planning, guarded execution, and fail-closed multi-run campaigns |
| P09 Reproducible computation | computational scientist | computational execution | checkpointed/replayable computation, intent-to-DAG compilation, locality-aware worker placement, budgeted portfolio execution, robustness-debt-driven re-analysis, selective failed-frontier recovery, omission-stress robustness suite, and computation-to-interpretation/replication routing |
| P10 Causal interpretation and replication | methods reviewer | statistical interpretation, replication/robustness | uncertainty-aware endpoint, longitudinal, stratified causal, dynamic-policy, causal-contrast, meta-analytic, cross-site verdicts, guarded adaptive-frontier execution, bounded resynthesis campaigns, and computation-evidence adjudication |
| P11 Research-object release | reproducibility steward | research-object release | portable manifest with limitations and negative evidence, dependency-aware replay, accountable release gating, and operator handoff |
| P12 Federated benchmarking | consortium administrator | federation benchmarking | aggregate-only cross-site benchmark consensus, influence-aware site portfolio planning, planner-to-campaign adaptive execution, robust pooling, heterogeneity, and site-influence analysis |

## Feature expansion

Every program expands the same product surface into 32 independently demonstrable capabilities:

| Archetype | Local single-study | Multimodal multi-study | Prospective high-throughput | Federated continual |
| --- | --- | --- | --- | --- |
| Scientific algorithm | F01 | F02 | F03 | F04 |
| Typed data primitive | F05 | F06 | F07 | F08 |
| Agent automation | F09 | F10 | F11 | F12 |
| Workflow orchestration | F13 | F14 | F15 | F16 |
| Researcher interaction | F17 | F18 | F19 | F20 |
| API/protocol integration | F21 | F22 | F23 | F24 |
| Verification/safety system | F25 | F26 | F27 | F28 |
| Operations/federation capability | F29 | F30 | F31 | F32 |

The generated feature text names a consumer, behavior, artifact, surface, and acceptance gate.
It is a product capability route, not a hypothesis or a to-do item. Stable ids use
`GAF-GLIOMA-P##-F##` so they cannot be mistaken for blueprint coverage ids.

## Build order

1. Keep the folder map and catalog valid before adding a new program implementation.
2. Implement the typed contract in its owning program module and add a deterministic digest.
3. Add negative, missing, contradictory, boundary, and replay tests before connecting a provider.
4. Expose the capability through MCP only after the Rust contract is validated.
5. Connect local providers through `GliomaStageExecutor`; this crate never opens a socket or
   touches an instrument itself.
6. Promote a program only when its independent baseline, reproducibility, and preclinical safety
   gates are measured.

The first tranche is implemented in P01, P02, P03, P05, P06, P08, P10, P11, and P12. P04 now also
bridges compiled evidence gaps into the dependency-aware action selector, so a researcher can hand
the returned `selected_order` directly to the local portfolio executor. P04 also includes a bounded
question-to-action campaign that dispatches claim-scoped local actions, recompiles typed knowledge
and decision context from returned evidence, and retains negative, contradictory, unresolved,
omission, retry, and budget outcomes. The P04 branch campaign now executes the selected
scenario-robust portfolio through the same claim-scoped local executor, scores observed evidence
against the branch forecast, and fails over to the next Pareto branch on forecast drift, action
failure, or budget exhaustion. It preserves supported, negative, contradictory, unknown, stale,
and unmeasured evidence as typed outcomes; no drift is converted into a confident conclusion.
The adaptive decision-branch campaign closes the loop across those layers: after each local branch
returns evidence, it recompiles typed knowledge and decision context, removes completed actions,
replans the robust frontier, and continues only while bounded budget and progress gates hold. No
scenario forecast is promoted to an observation; no-progress, executor failure, budget exhaustion,
and unresolved branch frontiers remain explicit terminal states for the researcher.
The P04 admission gate (`admit_glioma_decision_actions`) is the execution boundary after context
compilation: it evaluates every generated action against evidence strength, freshness, modality
coverage, contradiction, reproducibility, dependency closure, effect permissions, autonomy
approval, signed preflight, and budget. It emits admitted, approval-required, blocked, and denied
partitions with stable reason codes, so autonomous research can advance only through typed actions
that are currently supportable and authorized.
The value optimizer (`optimize_glioma_decision_value`) searches bounded combinations of typed
actions instead of taking a greedy single row. It scores information gain, uncertainty reduction,
contradiction resolution, reproducibility, failure risk, and modality/model/diversity-group
coverage, returning a selected portfolio plus deterministic alternatives and blocked/deferred
reasons for the researcher or admission gate.
The value calibration surface (`calibrate_glioma_decision_value`) learns only from typed outcomes
of completed local runs. It applies bounded shrinkage to prior utility, reports weighted forecast
error and confidence, marks prior-only or conflicted candidates as uncertain, and preserves failed
outcomes for review before a learned ranking is sent back to the optimizer. Calibration never
mutates historical observations and never upgrades a planning score into biological evidence.
The adaptive controller (`execute_glioma_adaptive_decision_controller`) consumes that calibration
directly. It gives prior-only candidates a bounded exploration bonus, applies explicit penalties to
forecast conflicts, searches dependency-closed combinations with a deterministic beam, and keeps
budget-blocked, deferred, negative, and uncertain actions visible before admission. This is the
P04 learning-to-action seam used by the autonomous research loop; it still produces planning
utility only and cannot dispatch an assay or instrument by itself.
The loop governor (`govern_glioma_decision_loop`) controls continuation after local rounds. It
combines information gain and uncertainty reduction into a bounded net-progress score, penalizes
failures, contradictions, and negative outcomes, and stops for budget exhaustion, repeated
no-progress, failure limits, incomplete closure, or human review. A qualified round is sent to
independent validation rather than being treated as a clinical or biological conclusion.
The federated decision-context engine (`aggregate_glioma_federated_decision_context`) combines
site-local branch plans without moving raw data. It applies independent-group quorum, site quality,
branch support, heterogeneity, and leave-one-site-out influence gates, ranking only robust branches
for promotion. Denied, underpowered, negative, contradicted, failed, and unknown site outcomes are
retained with deterministic omissions and a route back to the decision cycle or researcher review.
The typed context artifact (`materialize_glioma_decision_context_artifact`) packages the same
decision state for local agents, workbenches, Rust/Python/TypeScript SDKs, and MCP clients. It
retains candidate dependencies, autonomy tiers, effects, deferred actions, omissions, negatives,
uncertainty, consumer compatibility, and semantic-loss declarations, while keeping the source
context content-addressed and the preclinical boundary explicit.
The multi-study context artifact (`align_glioma_multi_study_context_artifacts`) aligns those
portable contracts across independent studies without moving raw evidence. It filters denied or
low-quality studies, requires independent-group support, retains typed action conflicts as
unresolved, and namespaces each study's negative and unknown partitions before producing a
deterministic shared frontier.
The multi-study workflow planner (`plan_glioma_multi_study_workflow`) then closes selected
frontier actions over prerequisites, allocates them only to policy-approved institution-local
budgets, and schedules deterministic waves. It refuses to schedule a partial independent-group
replication quorum, reserves compute/material/instrument capacity, and marks physical, material,
external-data, federation, or elevated-autonomy effects as requiring approval/preflight. Its output
is still a plan; dispatch remains a separate policy- and operator-gated step.
P07 now also has an
adaptive campaign planner (`plan_glioma_workflow`) and a guarded full-program executor that
chooses deterministic next batches, closes over dependencies, and routes unresolved evidence,
QC defects, contradictory mechanisms, underpowered designs, budget exhaustion, and approval gaps
into explicit hold/abstain branches. Checkpoint output digests are bound into the workflow plan so
a resumed campaign cannot silently swap a local evidence, QC, mechanism, or design object. P04
retains an explicit ownership folder and catalog route. P08 now includes robust instrument-control
calibration and Theil–Sen drift detection, while P12 includes aggregate-only federated benchmark
consensus with heterogeneity and leave-site-out influence bounds. P01 now also compiles a deterministic
evidence-priority queue from the current local snapshot: stale, contradictory, unknown, negative,
coverage-deficient, and supported records become explicit refresh, resolution, measurement,
revalidation, coverage, or replication actions for the next P04/P07 cycle. The queue is bounded,
content-addressed, and keeps negative/uncertain records visible; it never fetches sources or
promotes a claim. P01 also triangulates a claim across independent source families and local
artifacts (`triangulate_glioma_evidence`), reporting support, contradiction, negative evidence,
unknown coverage, source-family diversity, and leave-one-artifact sensitivity. A claim is only
qualified when its support clears the declared floors without contradiction, incompleteness, or
source dominance; otherwise the engine emits a partial, negative, or unresolved state with a
specific next action. This is a scientific synthesis gate, not a literature fetcher or clinical
decision-maker.
P01 also calibrates source-family scores (`calibrate_glioma_evidence`) against resolved local
outcomes using quality-weighted isotonic regression. Non-monotonic score/outcome relationships are
pooled deterministically, unknown/stale/unmeasured outcomes do not count as resolved support, and
under-observed or high-error families become explicit review actions with calibrated reliability
and uncertainty for P02/P04/P07 planning.
P01 now also compiles a budgeted evidence-acquisition portfolio (`plan_glioma_evidence_acquisition`)
for the autonomous engine. It closes candidate dependencies before selection, rewards independent
source families and cross-modality/model coverage, penalizes expected failure and cost, and emits
expected versus worst-case value. Human-data, non-local payloads, privacy-ceiling violations, and
independence overuse are blocked before dispatch; deferred work, missing coverage, negative policy
evidence, and the candidate frontier remain explicit. The planner is intentionally acquisition
agnostic: a separate approved adapter must perform retrieval, assay setup, or replication, and no
successful acquisition is claimed by the planning route.
P01 now also exposes a deterministic novelty radar (`glioma_evidence_novelty_radar`). It compares
normalized preclinical source metadata against a known claim/domain corpus, suppresses near
duplicates, and scores novelty separately from freshness, quality, citation signal, and domain
gaps. The result is an acquisition/review/deprioritization queue for P02 knowledge compilation;
low-quality records, stale snapshots, and no-novel-evidence outcomes remain explicit, and the
radar never treats novelty as validity, causality, or clinical evidence.
P01 now also detects prospective evidence trajectories (`glioma_evidence_temporal_shift`). It
compares weighted baseline and recent windows for each typed claim, uses replicate/quality/
inverse-uncertainty weighting, detects emergent signals and reversals, exposes temporal
heterogeneity, and refuses to promote under-observed windows. The output is a bounded re-review
or replanning action for the autonomous engine; it does not infer an unmeasured mechanism or
make a clinical decision.
P01 now also compares those trajectories across institutions (`glioma_federated_evidence_shift`).
It pools only typed baseline/recent summaries, measures direction consensus, heterogeneity, and
leave-one-site-out influence, and distinguishes a consortium-wide shift from a site-specific
effect. Quorum, privacy, preclinical-only, and influence gates are explicit; raw sources remain
local and a qualified shift routes to knowledge refresh, mechanism review, or replication planning.
The execution bridge (`execute_glioma_evidence_acquisition_campaign`) consumes only that
content-addressed plan, orders prerequisites before dependants, retries transient adapter faults,
and records completed, negative, partial, unknown, failed, budget-blocked, and dependency-blocked
work. MCP uses an explicit synthetic adapter; institution-local executors are the only path to
real retrieval, assay, simulation, or replication effects.
The multimodal researcher workbench (`glioma_multimodal_researcher_workbench`) turns local typed
records into cross-study panels keyed by an explicit claim/scope contract. It selects bounded
representatives while preserving study, modality, model, freshness, quality, negative, and
contradiction coverage, reports a deterministic coverage matrix, and routes under-covered panels
to the multimodal gap compiler. It never pools raw measurements or upgrades a panel into a causal
or clinical conclusion.
The evidence verification gate (`glioma_evidence_verification_gate`) is the promotion boundary
before P02 knowledge compilation. It requires configurable support counts, independent source
families, multimodal/model coverage, quality/reproducibility floors, freshness, and explicit
negative/contradiction policy. Failed gates produce typed blocking findings and remediation routes;
they cannot be converted into confidence by the autonomous engine.
The multi-site outcome reconciler (`glioma_multisite_outcome_reconciliation`) is the consortium
boundary after local study execution. It accepts only typed aggregate site summaries, equalizes
site influence, computes support/negative/contradiction fractions, heterogeneity, leave-one-site-
out influence, quorum, and modality/model coverage, then routes consistent support, preserved
negative results, contradictions, replication work, or missing coverage. Raw measurements remain
site-local, and any eligible contradiction is routed for resolution instead of being diluted by
majority support.
The evidence-to-knowledge bridge (`glioma_evidence_knowledge_bridge`) is the explicit P01→P02
promotion boundary. It requires matching verification and typed-knowledge objectives, binds both
content digests, aligns every claim's evidence identifiers, and emits claim-level admission,
conditional, negative-preservation, contradiction, coverage, or hold decisions. Evidence that is
stale, omitted, or absent from the verification surface remains an omission and cannot be promoted
because a knowledge compiler happened to produce a claim for it.
The long-horizon calibrator (`glioma_long_horizon_evidence_calibration`) extends source-family
calibration into explicit retrospective epochs. It computes quality-weighted calibration error,
Brier error, reliability, independent-group coverage, and recent-versus-baseline drift for every
window—including empty and underpowered windows. Degrading families route to recalibration review,
volatile families remain visible, and missing windows route to bounded acquisition rather than
being interpolated into a confident trend.
The federated outcome transport (`glioma_federated_outcome_transport`) is the production boundary
for moving consortium summaries after local execution. It checks claim/scope identity, purpose,
capability version, content attestations, revocation, freshness, quality, reproducibility, raw-data
locality, de-identification, and independent-site quorum. It exports only typed aggregate summaries;
negative, contradictory, unknown, stale, and under-quorum outcomes remain explicit and route back to
acquisition, triage, reconciliation, or verification rather than being promoted as biology.
The federated operating cycle (`glioma_federated_evidence_operating_cycle`) is the next-action
director over those reports. It ranks bounded acquisition, omission verification, calibration
refresh, contradiction resolution, replication, negative-result preservation, and typed-knowledge
handoff actions; carries prerequisite and route metadata; and exposes a deterministic action budget
with omitted work rather than silently dropping it. The planner is advisory (A0) and cannot execute
assays, move raw data, or promote a biological claim on its own.
P09 now includes a bounded robustness suite (`assess_glioma_robustness`) that
recomputes the declared effect under leave-one-batch-out and optional leave-one-row-out omissions;
unresolved subsets, fragile effects, and null results remain explicit. Provider-specific execution
for the remaining programs remains subsequent build work rather than being implied as complete.
P06 now also includes a weighted monotone dose-response analyzer (`analyze_glioma_dose_response`)
with transparent raw/fitted means, residual noise, monotonicity violations, and half-maximal-dose
interpolation only on the declared preclinical grid.
P03 now includes feature-level multimodal concordance (`analyze_multimodal_concordance`) with
shared-feature alignment, fixed-point correlation, and explicit contradictory or unresolved pairs.
It also includes deterministic multimodal consensus clustering (`analyze_multimodal_consensus`),
which forms per-lineage median profiles and bounded k-medoids clusters while preserving missing
modalities, disconnected sample pairs, distance failures, and unresolved samples.
P03 also includes robust batch harmonization (`harmonize_glioma_multimodal_batches`) that
median-centers each modality against a declared reference batch, emits corrected vectors and
residual-spread diagnostics, and refuses to impute missing features or hide oversized corrections.
P03 now also includes deterministic latent-state factorization (`analyze_glioma_latent_factors`)
that robustly median/MAD-scales complete-case modality columns, extracts bounded fixed-point power
components, and gates on explained variance, reconstruction error, convergence, and explicit
missing modality/feature coverage without imputation.
P03 now also includes spatial niche graph analysis (`analyze_glioma_spatial_niches`) that builds
same-lineage connected components from bounded spatial neighborhoods, summarizes local state and
boundary structure, and tests cross-lineage edges against a random-mixing expected-edge null model.
Sparse neighborhoods, isolated cells, undersized components, and absent interactions remain
explicit partial or unresolved outcomes; no spatial payload is moved or imputed.
P03 now also includes spatial ligand-receptor communication analysis
(`analyze_glioma_spatial_communication`) that builds deterministic sender/receiver neighborhoods,
aggregates declared ligand and receptor scores, and compares observed signal against a
lineage-marginal random-mixing null. Missing feature coverage, sparse support, zero expected
signal, and non-enrichment remain explicit; this is a local association screen rather than a
causal signalling or clinical inference.
P03 now also includes spatial-state propagation (`analyze_glioma_spatial_state_propagation`) that
builds same-sample neighborhood edges and runs a bounded integer diffusion with self-retention,
lineage-aware coupling, convergence checks, and hotspot ranking. It never diffuses across samples,
imputes isolated cells, or presents a spatial simulation as biological proof.
P03 now also includes cross-sample spatial registration (`register_glioma_spatial_samples`). It
estimates a robust translation/isotropic-scale transform from repeated lineage landmarks, emits
aligned local cell coordinates and landmark residuals, and gates samples on shared-lineage coverage,
residual spread, and missing landmarks. Rotation/shear not modeled, unresolved samples, and
unregistered cells remain explicit, so downstream spatial niches can compare only the registered
subset rather than silently mixing coordinate frames.
P03 now also includes multimodal graph fusion (`analyze_glioma_multimodal_graph_fusion`) that
retains modality-specific sample neighbours, fuses them with reliability-weighted consensus, and
runs bounded diffusion over observed edges. Modality dropout, sparse shared features, contradictory
cross-modal scores, and the all-modalities release gate remain explicit negative evidence rather than
being silently imputed or converted into a confident state.
P03 now also includes multimodal endpoint sensitivity analysis
(`analyze_glioma_multimodal_sensitivity`). It computes leave-one-modality-out influence and
bounded low/high perturbation ranges, detects sign-changing endpoint decisions, and ranks
reacquisition or orthogonal-replication actions for the autonomous research engine. A robust
endpoint can advance to mechanism planning; a fragile or under-covered endpoint remains
conditional or blocked, without imputation or assay dispatch.
P03 also includes an uncertainty-aware multimodal decision gate
(`analyze_glioma_multimodal_decision_gate`). It estimates a reliability-weighted endpoint
interval, compares that interval with an investigator-declared preclinical research threshold,
and routes supported, negative, indeterminate, and blocked states to the next autonomous stage.
Contradictory modalities, wide intervals, low confidence, and missing required evidence remain
explicit handoff conditions; the gate is never a diagnostic, treatment, or clinical decision.
P03 now also includes multimodal contradiction adjudication
(`adjudicate_glioma_multimodal_contradictions`). It classifies pairwise agreement, sign reversal,
magnitude conflict, quality asymmetry, and unresolved gates; ranks measurement trust without
deleting rival evidence; and emits deterministic reacquisition or orthogonal-assay actions for
the autonomous engine. Contradictory preclinical measurements remain research uncertainty rather
than becoming a confident biological or clinical conclusion.
P03 now also includes prospective multimodal quality forecasting
(`forecast_glioma_multimodal_quality`). It estimates robust quality trends over ordered QC
epochs, forecasts the next acquisition horizon, and ranks modality preflight/reacquisition risk
before an autonomous endpoint workflow is scheduled. Missing history remains blocked, quality
failure remains explicit, and the forecast never predicts biology or invents an assay result.
The quality-risk-aware scheduler (`plan_glioma_multimodal_quality_schedule`) turns that forecast
into a bounded, deterministic acquisition order under local budget, duration, deadline, and
required-modality constraints. It emits alternatives, uncovered-required gates, explicit risk
reduction, and approval actions while remaining simulation-only; it does not dispatch an assay or
silently substitute a missing modality.
The schedule-bound executor (`execute_glioma_multimodal_quality_schedule`) is the workflow bridge
from planning to local action. It requires a content-bound, unrevoked study approval, executes
only the selected modality order through an institution-owned executor seam, retries declared
transient failures, preserves below-floor QC and blocked work, and refuses downstream continuation
when required quality gates fail. MCP uses a deterministic dry-run executor; no hardware, raw
payload, or clinical decision is produced by the route.
The adaptive campaign (`execute_glioma_multimodal_quality_adaptive_campaign`) closes the loop
without becoming an unconstrained self-modifying agent: each round has a bounded budget and
approval window, only typed QC observations update risk, satisfied modalities are removed from the
pending frontier, and required failures or budget exhaustion stop the campaign with explicit
negative evidence. This is the P03 handoff into readiness and endpoint analysis.
Cross-study QC-policy transport (`calibrate_glioma_multimodal_quality_transport`) compares
source and target study summaries per modality, scoring quality/coverage gaps, target alignment,
and drift. Missing target calibration blocks transfer; weak alignment or drift makes it
conditional, and only qualified modalities can be used as target-local confirmation candidates.
QC incident root-cause attribution (`attribute_glioma_multimodal_quality_root_cause`) ranks
instrument, batch, preparation, alignment, transport, connector, and unknown process causes from
typed local signals. It preserves contradictory and low-reliability evidence, blocks attribution
when required modalities are unobserved, and emits remediation actions for the adaptive campaign;
it never treats a measurement-process cause as a biological or clinical conclusion.
QC remediation planning (`plan_glioma_multimodal_quality_remediation`) converts qualified or
competing process causes into deterministic, budgeted action sequences. It ranks re-harmonization,
re-preparation, alignment, transport, connector quarantine, instrument inspection, and orthogonal
QC actions by expected recovery versus cost, duration, and risk; approval requirements, unavailable
capabilities, resource bounds, and rejected actions remain explicit for the execution campaign.
Paired recovery verification (`verify_glioma_multimodal_quality_recovery`) closes the loop by
conservatively aggregating baseline and post-remediation QC observations, requiring target floors,
reliability, sample coverage, and multi-metric improvement, then routing each modality to proceed,
iterate, escalate, or remain blocked. Missing paired evidence and failed floors prevent downstream
analysis admission rather than being converted into a confident scientific conclusion.
The feature transfers no raw data and never treats QC-policy portability as biological validity.
P05 now also includes signed pathway activity inference (`analyze_glioma_pathway_activity`) that
maps declared modality-specific molecular nodes to reliability-weighted pathway activity, compares
cross-modal direction, and ranks mechanism priorities. Missing nodes, low-confidence bottlenecks,
and modality disagreement remain explicit so downstream experiment selection receives actionable
coverage gates rather than an invented pathway state.
P02 now includes typed-knowledge compilation (`compile_typed_knowledge`) that coalesces scoped
claims, ranks support against contradiction, preserves negative/unknown evidence, and exposes
missing modality/model coverage for the next workflow action.
The claim frontier (`prioritize_knowledge_frontier`) then scores coverage debt, contradiction,
unresolved evidence, support, and workflow leverage to choose which claims should drive the next
P04/P07 cycle; it returns explicit action modes and never upgrades a claim's evidence state.
Knowledge composition (`compose_knowledge_graph`) adds the missing network layer: it traverses
only caller-declared supports and prerequisites, computes weakest-link path strength, detects
contradiction edges, identifies connected claim components and bottleneck claims, and returns
replay-stable paths for P04/P05. It is an evidence-network planning capability, not causal
identification or clinical guidance.
Belief revision (`revise_glioma_beliefs`) adds an explicit-conflict maximal-consistency layer. It
accepts only typed conflict edges, retains the strongest compatible claim portfolio with bounded
beam search, and preserves rival claims, negative claims, unresolved coverage, conflict evidence,
and a ranked frontier for the next autonomous cycle. It never infers contradiction from wording or
turns a research claim into a clinical decision.
The consistency closure (`compile_glioma_knowledge_consistency`) adds a deterministic score-aware
pre-dispatch gate over the typed graph. Explicit support and prerequisite edges increase usable
claim score, contradiction edges impose bounded penalties, negative claims are excluded without
deletion, and contested rivals remain visible with adjudication actions. Claims that cannot clear
the support closure emit evidence-acquisition actions for P01/P02 rather than being silently
promoted into a downstream workflow.
Knowledge drift detection (`glioma_knowledge_drift`) now compares two validated typed-knowledge
snapshots after each evidence cycle. It classifies claim additions, removals, strengthening,
weakening, contradiction, resolution, and stability, then emits priority-gated actions for
consistency closure, gap compilation, or mechanism replanning. The transition is bound to both
snapshot digests, preserves negative and unresolved states, and never treats confidence drift as
causal or clinical evidence.
Multi-study typed-knowledge alignment (`glioma_multi_study_knowledge`) creates a deterministic
cross-study claim table from explicit study-owned bindings. It pools support and confidence,
reports agreement and leave-one-study-out influence, exposes modality/model coverage debt, and
preserves semantic-loss, negative, contested, unresolved, and unbound claims. Equivalence is
never inferred from prose, so the table is safe to route into consistency closure and mechanism
planning without allowing a dominant study to silently define the portfolio.
Prospective knowledge monitoring (`glioma_prospective_knowledge_monitor`) consumes an ordered
stream of already-typed claim observations for high-throughput programs. It estimates claim-local
baselines, computes bounded cumulative drift and change points, applies persistence and
multiplicity gates, and routes review, acquisition, or quarantine actions. A single noisy event
cannot promote a claim; negative, contradictory, unresolved, and missing-coverage states remain
first-class outputs for the next autonomous cycle.
Federated continual knowledge (`glioma_federated_continual_knowledge`) fuses aggregate-only
observations across sites and release epochs with robust medians, quorum floors, outlier-site
diagnostics, and leave-one-site-out influence. Conflicting semantic digests fail closed, export
and preclinical-only policies are enforced before analysis, and negative or contested epochs are
kept visible instead of being averaged into a false consensus.
The federated continual agent (`glioma_federated_continual_agent`) turns that state into a bounded
research-action frontier. It scores expected information against coverage debt, contradiction,
trend, confidence, and site influence, then applies dependency, budget, autonomy, physical-effect,
approval, and quarantine gates. It returns a typed plan for the local dispatcher; it does not
silently execute a physical or cross-institution effect.
The local workflow compiler (`glioma_local_research_workflow`) turns selected agent actions into
deterministic dependency waves. It emits checkpoints, retry limits, expected artifacts,
compensation kinds, critical-path length, and omitted/approval states so a local dispatcher can
execute a replayable bounded workflow without silently admitting an incomplete frontier.
Federated typed-knowledge consensus (`glioma_federated_knowledge`) now compares independent
site summaries without exporting source text. It pools support, contradiction, confidence, and
disposition mass, reports site-specific disagreement and leave-one-site-out influence, and emits
bounded promotion, adjudication, or negative-result actions. A consensus summary is still a
governed research state—not a causal conclusion—and raw data stays at the originating institution.
Typed evidence closure (`glioma_knowledge_closure`) is the action-readiness gate after compilation:
it reconciles every claim to caller-supplied evidence identifiers, measures independent-artifact,
modality, and model-system coverage, and keeps missing references, negative evidence, uncertainty,
and orphan artifacts explicit. Only claims that clear the declared closure floors can qualify for
bounded downstream planning; the route never retrieves sources, moves raw data, executes an
instrument, or makes a clinical decision.
P04 now includes decision-context compilation (`compile_decision_context`) that converts those
gaps into typed A1 candidates for coverage closure, contradiction replication, negative-result
falsification, evidence resolution, or mechanism validation; the existing action selector then
applies budget and policy gates before any provider dispatch.
The decision-action graph (`compile_decision_action_graph`) joins that context with P02 composed
claim paths. It adds explicit prerequisite edges, computes deterministic topological order and
parallel waves, reports critical-path and total cost, and keeps missing claims, unresolved paths,
negative evidence, and budget blocks visible for the autonomous engine.
The decision-mission bridge (`execute_glioma_decision_mission`) is the executable handoff from
that P04 graph into the P07 science-aware mission controller. It carries the context and graph
digests into adaptive rounds, preserves dependency closure and explicit partial-graph opt-in, and
accepts a canonical completed-action frontier for deterministic resumption. It returns typed mission outcomes rather than a receipt-only acknowledgement. The MCP worker remains
simulation-only; a production institution supplies the local executor that owns assays, analyses,
or instrument gateways.
P05 now also includes signed mechanism-network propagation (`propagate_glioma_mechanism_graph`)
that combines direct support/contradiction with activating or inhibiting evidence edges using
bounded damped fixed-point diffusion. Low-confidence edges, disconnected nodes, contradiction,
and non-convergence remain visible instead of becoming false mechanistic certainty.
P05 now also includes robust intervention portfolio planning
(`plan_glioma_robust_intervention_portfolio`) that evaluates each signed perturbation across a
declared model ensemble, computes prior-weighted expected and lower-tail effects, and selects
non-redundant candidates under worst-case effect, model agreement, feasibility, risk, and budget
gates. The output is a ranked assay portfolio with explicit exclusions and no biological dispatch
seam.
P05 now also includes mechanism-action compilation (`compile_mechanism_action_plan`). It converts
 residual-likelihood information-gain assays into typed A1 local candidates ranked by information
 per cost, feasibility, measurement uncertainty, and mechanism-unlock value. The resulting plan
 feeds the autonomous campaign controller; it never turns a mechanism score into an observation
 or a clinical recommendation.
The local workflow assurance gate (`glioma_mechanism_workflow_assure`) is the pre-dispatch
verification boundary for that controller. It replays dependency closure, checks route and local
artifact allowlists, evaluates stale, missing, unresolved, contradictory, and low-quality local
evidence, and enforces risk, approval, compensation, retry, action-count, and budget limits.
Failed prerequisites propagate to downstream actions, while admitted actions remain a typed local
execution plan; no assay, instrument, network, or clinical effect is performed by the gate.
P10 now includes an exact bounded causal contrast (`analyze_glioma_causal_contrast`) using
pre/post unit changes, treatment-label permutations, and leave-one-unit bounds; null, non-significant,
or underpowered effects remain explicit rather than being promoted into mechanism claims.
P10 now also includes discrete-state longitudinal transition analysis
(`analyze_glioma_state_transitions`) for investigator-declared glioma phenotypic states. It builds
deterministic per-arm transition matrices from consecutive within-unit observations, contrasts
treatment with control, and preserves irregular sampling, absent transitions, null effects, negative
evidence, and support-floor failures as explicit outcomes; state order is descriptive and never a
clinical severity scale.
P10 also includes fixed-point replication meta-analysis (`analyze_replication_meta_analysis`) with
fixed and random-effects inverse-uncertainty pooling, estimated between-study variance,
Cochran/I² heterogeneity, leave-one-study-out influence, and explicit negative or unresolved
outcomes for contradiction, underpowered sites, weak signal, and unstable pools.
P10 now also includes the autonomous replication campaign (`execute_glioma_replication_campaign`).
It composes site-level replication, fixed/random-effects pooling, and model-system transportability;
scores missing coverage, heterogeneity, influential studies, and transport gaps; dispatches only
typed bounded local actions; and incorporates returned study artifacts into the next analysis round.
Simulation observations are never promoted into biological evidence, and qualification, negative,
partial, failed, budget, transport, and no-progress stops remain explicit.
The cross-family interpretation gate (`synthesize_glioma_interpretation`) then combines typed
causal, trajectory, sensitivity, meta-analytic, transportability, and replication summaries. It
weights only quality-qualified local artifacts, preserves negative and unresolved evidence, exposes
cross-family contradiction, and requires leave-one-family-out stability plus declared replication
floors before returning a qualified interpretation. It is an autonomous research conclusion gate,
not a clinical decision or an assay dispatcher.
P10 now also exposes guarded adaptive-frontier execution (`execute_glioma_adaptive_frontier`). It
recompiles the frontier before dispatch, verifies that the action selector cannot drift, and sends
the selected replication, stability, evidence-gap, model-transfer, or mechanism-discrimination
actions through the dependency-safe local action executor. Unresolved synthesis remains held unless
the caller explicitly permits bounded local dispatch; approval, effect, artifact, retry, negative,
partial, failed, and blocked outcomes feed the next synthesis round rather than being hidden.
P10 now also exposes the bounded adaptive interpretation campaign
(`execute_glioma_adaptive_interpretation_campaign`). It repeats synthesis, frontier selection,
and local execution under hard round/action/budget limits, carries completed and negative actions
forward, and stops on qualification, negative evidence, unresolved holds, executor failure, budget,
or planner no-progress. The dry-run planner refuses to turn synthetic action artifacts into a new
claim; institution-local planners can return a validated evidence request for the next round.
P06 now includes combination-response analysis (`analyze_glioma_combination_synergy`) with
vehicle/single-agent control requirements, integer Bliss expectations, residual noise, synergy,
antagonism, and explicit unresolved cells for missing controls or replicates.
P06 also includes sequential assay allocation (`allocate_glioma_assays`) using Beta posteriors,
conservative Cantelli target-effect bounds, uncertainty exploration, replicate floors, risk
ceilings, and a hard next-batch budget.
P06 now also exposes `execute_glioma_adaptive_allocation_campaign`: a bounded controller that
selects one arm, requests an exact aggregate replicate batch from a caller-owned local adapter,
validates the returned artifact and success/failure count, updates the posterior, and replans. It
keeps underpowered, negative, risk-blocked, budget, retry, no-progress, and executor-failure
states explicit; the bundled MCP executor is simulation-only and never represents biological
evidence.
The engine-level action selector (`select_glioma_actions`) now uses a bounded deterministic beam
search over executable action portfolios. It preserves multiple partial plans, discounts repeated
modality/model pairs, and can select a prerequisite-plus-downstream bundle that has greater total
research value than a locally attractive isolated action; dependency, approval, instrument, and
federation gates remain fail-closed.
P06 also includes a mechanism-aware closed-loop campaign controller
(`plan_glioma_closed_loop_campaign`) and round-by-round executor
(`execute_glioma_closed_loop_campaign`). They reweight competing mechanisms from local
observations, score typed assays by expected mechanism information and effect, and emit bounded
sequential rounds under feasibility, cost, risk, and replicate ceilings. The caller-owned executor
runs each local batch and the engine replans from the observations it actually returns; malformed
or missing observations stop the loop, while posterior convergence, no-information,
negative-result, and budget stops remain explicit.
P07 now also includes guarded protocol execution (`execute_glioma_protocol`) behind a caller-owned
local executor. A feasible simulation is required before any task is admitted; dependency order,
typed output artifacts, bounded retries, partial results, failed tasks, and skipped dependents are
recorded explicitly. The executor seam can target a local simulator, compute worker, robotics
gateway, or institution-approved instrument service without the research crate opening a socket or
making a clinical decision.
P07 now also includes action-portfolio execution (`execute_glioma_action_portfolio`) for the
beam-selected autonomous batch. It runs assays, analyses, simulations, or approved gateway
actions in dependency order through a caller-owned executor, retries only declared transient
failures, requires local typed artifacts when configured, and stops with explicit failed, partial,
negative, or skipped outcomes instead of pretending the portfolio completed.
P07 now also includes an observation-driven autonomous campaign controller
(`execute_glioma_autonomous_campaign`). It keeps a bounded typed action registry, asks a local
planner for new assays or analyses after each returned round, spends a hard research budget, and
replans only from observed executor results. Failed or partial effects stop the campaign; negative
results remain first-class evidence and can satisfy downstream dependencies. The MCP route uses
the deterministic dry-run planner/worker, while institution-local planners and execution gateways
can implement the two explicit Rust seams for real preclinical workflows.
The context-to-action autopilot (`execute_glioma_research_autopilot`) closes the common single-cycle
path: it consumes the P04 context, selects the next dependency-safe batch, executes it through the
same local worker seam, and returns the exact ids to recompile after new artifacts arrive. A hold
with no runnable action produces no synthetic execution result.
The evidence campaign bridge (`execute_glioma_evidence_campaign`) closes the preceding P01-to-P07
handoff: it accepts the content-addressed priority plan, admits only typed local action adapters
for selected evidence work, computes dependency closure, executes through the same portfolio
executor, and reports missing adapters, policy blocks, partial effects, negative outcomes, and
requeue instructions as first-class states.
The evolution-aware clone campaign (`execute_glioma_adaptive_clone_campaign`) now closes the
P05→P06→P10→P07 loop in one bounded workflow: it infers an ambiguity-preserving clonal graph,
selects a branch-covering perturbation panel, executes only through a caller-owned local worker,
accumulates replicate observations, adjudicates supported/null/negative/contradictory cells, and
routes unresolved branches into dependency-closed continuation actions. Qualified and negative
stops are explicit, missing cells are never imputed, and the MCP worker is simulation-only.
P07 now also includes the multimodal mechanism campaign
(`execute_glioma_multimodal_mechanism_campaign`). It runs graph fusion and signed pathway activity
before invoking the dependency-aware action selector, so a local research engine can move from
observed multimodal evidence to a bounded next assay/analysis portfolio in one replayable cycle.
Unresolved graph coverage, pathway bottlenecks, contradictory modalities, and empty safe portfolios
remain explicit holds; the returned actions are plans until the existing policy and execution gates
admit them.
The companion execution bridge
(`execute_glioma_multimodal_mechanism_campaign_with_executor`) now passes a qualified selection
through the existing action-portfolio executor. It preserves dependency ordering, bounded retries,
local-artifact requirements, negative outcomes, and partial/blocked states; the MCP surface uses a
deterministic dry-run worker while institution-local deployments provide the production executor.
The science-aware mission controller (`execute_glioma_autonomous_research_mission`) sits above
these individual campaigns. It maintains a deterministic frontier state across evidence,
mechanism, assay, computation, and replication candidates; rewards uncovered stages and
model/modality diversity; increases priority for actions that resolve negative dependencies; and
replans only after typed local results arrive. Qualification requires explicit stage coverage,
information-gain, model/modality, and uncertainty gates. Negative results remain first-class
findings, while failed or partial effects terminate the mission. This is the cross-program
autonomous engine loop a researcher can run locally without turning a score or dry-run into a
biological conclusion.
The research director (`execute_glioma_research_director`) is the high-level entry point above
that loop. It compiles a bounded `GliomaResearchIntent` into the closed fourteen-stage graph,
binds typed local checkpoint artifacts, computes a focus-aware utility profile (evidence, mechanism,
experiment, computation, or replication), closes dependencies to a runnable frontier, and executes
one beam-selected batch through the existing action worker. Missing inputs, policy holds, synthetic
dry-run outcomes, negative evidence, and the exact next checkpoint are returned for the next cycle;
the director never skips an upstream gate or calls a clinical workflow.
The end-to-end engine (`execute_glioma_autonomous_research_engine`) closes that loop across the
full preclinical graph. It repeatedly compiles the intent, executes a bounded local batch, promotes
only returned typed artifacts into stage checkpoints, and replans downstream evidence, mechanism,
experiment, computation, replication, release, and federation work. Budget exhaustion, negative or
partial outcomes, policy holds, executor failures, and no-progress states stop honestly; the MCP
surface is a deterministic dry-run rehearsal while institution-local executors own real effects.
The adaptive workflow scheduler (`plan_glioma_adaptive_workflow`) adds a science-aware scheduling
layer for long-running programs. It updates conservative action utility from qualified, negative,
inconclusive, failed, and blocked observations; uses deterministic beam search to choose
dependency-closed portfolios; and enforces cost, risk, authority, instrument, federation, and
action-count budgets. Negative results remain in the plan as information, while blocked and
deferred actions carry explicit reasons so a later engine cycle can replan without inventing
evidence or silently escalating autonomy.
The evidence-gated director (`execute_glioma_evidence_gated_research`) adds the scientific
admission boundary before that execution loop: it consumes P01's independent source-family and
artifact triangulation, holds partial/negative/contradictory/incomplete/source-dominant claims with
their exact next evidence actions, and admits the dependency-safe director only when the configured
qualified-claim and global-qualification gates pass. This prevents a structurally runnable workflow
from becoming an autonomous scientific action merely because its inputs are present.
P06 also exposes the closed-loop multi-fidelity campaign
(`execute_glioma_multi_fidelity_campaign`). It repeatedly executes the optimizer's selected
screening, mechanistic, or validation conditions through a caller-owned local worker, recalibrates
paired-fidelity bias and reliability from the observations that actually return, and keeps prior,
transfer, neighbourhood, and observed estimates distinct. Higher-fidelity candidates remain
blocked until their declared lower-fidelity support is qualified; retries, duplicate replicate
keys, missing artifacts, budget exhaustion, and worker failure remain explicit instead of being
converted into a validation claim.
P05 now also exposes the adaptive mechanism policy and campaign
(`plan_glioma_adaptive_mechanism_policy`, `execute_glioma_adaptive_mechanism_campaign`). It updates
an integer model posterior from returned local observations, computes outcome-bucket Gini
information gain plus lower-tail effect robustness, and beam-selects a finite-horizon assay
sequence under feasibility, risk, redundancy, and budget gates. Each campaign round replans from
the typed observation that actually returned; posterior concentration is never treated as causal
identification, and the MCP worker is synthetic-only.
P05 also exposes a direct mechanism-discrimination campaign
(`execute_glioma_mechanism_discrimination_campaign`). It ranks competing hypotheses against local
feature observations, dispatches the highest-information discriminator through a caller-owned
adapter, replaces or appends the returned feature, and recomputes residual fit and posterior
separation after every round. Missing features, diffuse mechanisms, negative evidence, retries,
budget exhaustion, and no-progress are explicit; the dry-run route never represents a biological
measurement.

P05 now also exposes a mechanism-identifiability frontier
(`glioma_mechanism_identifiability`). It computes pairwise separation from declared preclinical
mechanism predictions, makes observationally indistinguishable pairs explicit, and greedily selects
the highest-value feature set under quality, risk, budget, and cardinality gates. Low-quality or
high-risk features remain blocked rather than silently entering the frontier, while unresolved
pairs become negative evidence and route to the next assay or model refinement. The result is a
deterministic planning artifact for the autonomous engine; it never treats predicted separation as
measured biology, executes an assay, or makes a clinical decision.

P05 now also exposes a cross-model mechanism-invariance frontier
(`glioma_mechanism_invariance`). It evaluates signed mechanism predictions across weighted
preclinical contexts, combining per-mechanism direction consistency, pairwise separation, and
transport stability. A greedy panel selector admits only signatures that clear the invariance and
separation floors while respecting quality, risk, cost, budget, and panel-size gates. Contextual
reversals, missing coverage, and unresolved mechanism pairs become explicit negative evidence for
the autonomous engine to route into model refinement or independent replication; predicted
invariance is never treated as measured biology or a clinical conclusion.

P05 now also exposes a mechanism intervention-value engine
(`glioma_mechanism_intervention_value`). It scores candidate perturbation assays by posterior-
weighted pairwise separation between competing glioma mechanisms, subtracts declared prediction
uncertainty, and applies feasibility, cost, risk, budget, disagreement, and redundancy-group
gates before selecting a bounded next-assay portfolio. Candidates that cannot distinguish the
current mechanism frontier remain deferred or negative evidence; the route is research planning
only and never dispatches biology or makes a clinical decision.

P05 now also exposes deterministic mechanism calibration
(`calibrate_glioma_mechanisms`). It scores competing mechanism probabilities against typed local
observations with fixed reliability bins, Brier loss, sharpness, and a final-round prequential
holdout. Underpowered mechanisms, high-uncertainty observations, discordant negative evidence, and
calibration/Brier gate failures remain explicit; calibration never refits a model or becomes a
causal claim, and the MCP route never executes an assay or moves raw data.
The calibration-aware campaign (`execute_glioma_calibrated_mechanism_campaign`) now joins that
report to adaptive policy selection. It discounts expected effects by model-specific calibration
trust, gives an explicit exploration bonus to actions that can repair calibration debt, and blocks
qualification when held-out coverage or Brier/calibration thresholds fail. Each returned local
observation triggers a fresh policy/trust recomputation; the dry-run route remains synthetic-only.
The longitudinal state filter (`filter_glioma_mechanism_states`) complements those static and
forward models with a transition-aware posterior over ordered local timepoints. It combines
multimodal residual compatibility with process and measurement uncertainty, reports coverage and
entropy proxies, preserves feature-level negative evidence, and marks dominant-state changes as
change points for the next discriminating assay or counterfactual plan.

P06 now also exposes a multi-factor contrast-panel compiler
(`design_glioma_contrast_panel`). It expands declared preclinical factors into balanced factorial
conditions, names each main-effect estimand, checks required interaction coverage, and reports
replicate/adequacy/budget gates. Its adequacy score is deliberately a bounded design proxy rather
than formal power; variance, batch, and site calibration remain downstream obligations. The MCP
route only compiles the panel and never randomizes material or executes an assay.
The adaptive dose-surface planner (`plan_adaptive_glioma_dose_surface`) chooses the next local
combination cells from typed preclinical observations. It uses inverse-distance response-surface
interpolation, replicate-debt and residual-noise estimates, an upper-confidence acquisition score,
hard total-dose and uncertainty ceilings, and a diversity constraint so a batch does not collapse
onto one neighborhood. Sparse neighborhoods, missing controls, under-replicated cells, budget
shortfalls, and already-complete cells remain explicit; the result is a validation plan, never a
clinical dose recommendation or an observed biological effect.
P12 now also exposes an autonomous federated benchmark campaign
(`execute_federated_benchmark_campaign`). It ranks aggregate-only follow-up actions from the
current pooled effect, heterogeneity, replicate floor, and leave-one-site-out influence, asks a
local executor for new site aggregates, and recomputes consensus after each bounded round. Site
raw traces stay local; duplicate identities, unbound results, budget exhaustion, negative evidence,
and unresolved heterogeneity remain explicit campaign outcomes.
P12 also exposes a conservative site-portfolio planner (`plan_federated_benchmark_sites`). It
uses bounded beam search over candidate independent studies, pessimistically discounts expected
scores by declared uncertainty, and replays the real consensus analyzer for every projected
portfolio. Budget, privacy risk, replicate floors, heterogeneity, spread, and leave-one-site-out
influence are optimized together; a ready plan is still a scenario requiring future validation,
never a fabricated benchmark observation.
The adaptive federated campaign (`execute_federated_benchmark_adaptive_campaign`) closes that
planner-to-execution gap. It validates aggregate-only site and artifact boundaries, converts the
selected portfolio into typed follow-up actions, runs the bounded campaign through a local executor
seam, and reports projected consensus separately from observed consensus. Qualified, negative,
heterogeneous, partial, blocked, and no-admissible-plan outcomes remain explicit, so a favorable
projection can never masquerade as a measured consortium result.
P12 now also exposes federated benchmark power sufficiency
(`glioma_federated_benchmark_power`). It combines inverse-uncertainty site information,
replicate floors, pooled signal-to-noise, a conservative fixed-point power proxy, between-site
heterogeneity, and leave-one-site-out influence. Underpowered, binding-mismatched, heterogeneous,
and influential site sets remain evidence gaps for site expansion or replication; the analyzer
never moves raw data or turns benchmark sufficiency into a clinical decision.
The replication-to-federation bridge (`execute_validation_replication_transport`) closes the next
workflow boundary. It accepts only a validated independent-site replication run, converts its
non-origin study summaries into aggregate mechanism sites, and invokes the bounded P12 transport
campaign. Each promoted study must carry an explicit bounded aggregate QC score; the bridge never
assigns a passing quality value merely because a replication request exists. Held, blocked,
negative, heterogeneous, unresolved, and budget-limited states remain typed; raw traces, human
data, specimen data, and instrument effects never cross the federation boundary. This is an
executable research handoff, not a passive receipt or an automatic scientific claim.
P10 also exposes a replication-closure frontier (`plan_glioma_replication_closure_frontier`). It
turns a typed independent-site result into a deterministic, budget/risk-constrained ranking of
site extension, heterogeneity reconciliation, target-model acquisition, influential-study stress
testing, negative-result confirmation, and methods review. Qualified and negative outcomes are
holds with operator actions rather than automatic permission to spend or claims of efficacy; each
selected route remains an institution-owned workflow with its own execution gate.
The closure execution seam (`execute_glioma_replication_closure`) now consumes that frontier and
dispatches only explicitly selected candidates whose route names the guarded replication campaign.
It validates objective/model parity, preserves frontier and campaign content digests, and returns
typed qualified, negative, partial, unresolved, or blocked outcomes. A held or unrunnable frontier
never reaches the executor; the MCP worker is deterministic and simulation-only, while institution
gateways retain all physical, protected-data, federation, and release authority.
The multi-round closure campaign (`execute_glioma_replication_closure_campaign`) composes a
caller-declared sequence of those guarded frontiers into one autonomous research loop. It reserves
a global budget before dispatch, rejects objective/model drift and duplicate actions, preserves
held and partial rounds, and stops on qualified, negative, unresolved, blocked, or exhausted
states. This is workflow execution over typed scientific decisions, not a receipt-only transport
layer; every round still requires the same local executor and preclinical boundary.
The closure interpretation bridge (`interpret_glioma_replication_closure`) converts only observed
campaign rounds into the existing replication evidence family, then reruns cross-family synthesis
with replication-required, quality, disagreement, and leave-one-out gates. Held frontiers produce
no evidence; negative, partial, unresolved, and contradictory rounds remain visible in the output
and drive the next scientific action. This gives the autonomous engine a usable conclusion surface
without allowing a workflow receipt, synthetic artifact, or clinical inference to masquerade as
research evidence.
The federated interpretation gate (`interpret_glioma_federated_closure`) joins that local P10
conclusion with independent aggregate benchmark consensus. Qualification requires alignment;
heterogeneous, negative, underpowered, and model-discordant consortium outcomes remain explicit
next-work. Raw traces stay institution-local, making this a scientific generalization gate rather
than an export or receipt mechanism.
P11 now adds a dependency-aware reproducibility replay campaign
(`execute_glioma_replay_campaign`). It schedules declared program replays, compares exact artifact
hashes, blocks downstream tasks after mismatch or unavailable outputs, and only emits a
reproducible release-readiness state when the manifest, required coverage, and replay gates all
clear. Non-deterministic tasks remain unavailable rather than being fabricated as successful.
P08 now also includes deterministic instrument preflight (`preflight_glioma_instrument`). It combines
qualified calibration, live interlock telemetry, typed operation parameters, operator authorization,
serialized scheduling, and risk/duration budgets into a dispatch-permitted or fail-closed plan. The
MCP route only emits the plan; a local gateway must re-verify authorization before any hardware
effect, and missing telemetry remains unresolved rather than imputed.
P08 also exposes bounded local signal extraction (`glioma_instrument_signal_extract`). It converts
value-only instrument points into replay-stable endpoint candidates using local median baselines,
robust residual noise, quality and drift gates, and spacing-constrained peak selection. Rejected
points, drifting channels, no-signal channels, and raw-trace locality remain explicit; extraction
does not become assay evidence until the existing QC adjudicator accepts it.
P08 also exposes prospective batch stability (`glioma_instrument_batch_stability`). It aligns
value-only extraction summaries across repeated instrument runs and gates endpoint promotion on
coverage, robust amplitude dispersion, noise, peak support, and first-to-last run drift. An
unstable or under-covered channel remains negative evidence for recalibration, replication, or
mechanism follow-up rather than being silently promoted by the autonomous engine.
P08 now also exposes multichannel temporal concordance
(`glioma_instrument_multichannel_concordance`). It searches a bounded integer lag against a
declared reference channel, computes fixed-point Pearson correlation and median absolute residual,
and preserves low-overlap, weak/inverse-correlation, low-quality, and high-residual channels as
explicit blocked evidence. Only bounded local summaries continue to signal extraction, batch
stability, and assay adjudication; the route never controls hardware or makes a clinical decision.
P08 also exposes aggregate-only federated instrument consensus
(`glioma_federated_instrument_consensus`). It inverse-uncertainty-weights permitted site summaries
and gates promotion on privacy counts, endpoint coverage, heterogeneity, leave-one-site-out shift,
and maximum site influence. The route lets the autonomous engine compare instrument-derived
endpoints across institutions without moving raw traces, specimens, human data, or hardware
effects; blocked endpoints remain explicit federation evidence gaps.
P08 now also includes guarded plan execution (`execute_glioma_instrument_plan`). A caller-owned
gateway is rechecked for authorization and live interlocks before every operation; transient
failures are bounded by retries, partial effects and negative outcomes halt the plan, and an
emergency stop is requested before remaining operations are skipped. The MCP adapter is synthetic
only; no hardware or biological effect occurs without an institution-owned `InstrumentExecutor`.
P08 now also exposes an ordered instrument campaign (`execute_glioma_instrument_campaign`). It
composes admitted execution requests into a durable run-level queue, preserves each run's
preflight/interlock/authorization digest, and partitions completed, negative, partial, failed,
blocked, and unresolved runs. Any unsafe or unresolved effect halts the remaining queue, while
the MCP adapter remains a deterministic synthetic gateway.
P08 now also exposes assay adjudication (`adjudicate_glioma_assay_evidence`), the scientific bridge
after execution: each local, de-identified assay summary is checked against the run action,
quality floor, uncertainty ceiling, replicate floor, effect threshold, and optional negative-control
gate. Hardware completion never qualifies biology by itself; qualified, negative, and unresolved
actions remain partitioned with explicit next actions for missing observations, repeat work, or
instrument recalibration. The MCP route only consumes value summaries and never moves raw data,
executes hardware, or makes a clinical decision.
P08 now also exposes adaptive instrument campaign selection (`execute_glioma_adaptive_instrument_campaign`). It
scores already-preflighted assay candidates by expected information, frontier novelty,
reproducibility, instrument time, and physical risk, then chooses a dependency-closed,
endpoint-diverse subset under explicit budgets before entering the guarded campaign executor.
Information and endpoint floors remain no-feasible-plan holds; instrument completion is never
promoted to biological evidence. The MCP route uses only synthetic local artifacts.
P08 also exposes schedule-bound fleet execution (`glioma_instrument_fleet_execute`). It
consumes the validated multi-instrument schedule, requires one admitted preflight plan per
assigned task, executes dependency-safe work through the guarded gateway, and records exact
instrument/time bindings. Schedule blocks, dependency blocks, negative results, partial effects,
unresolved telemetry, failed runs, bounded retries, and emergency-stop state remain first-class;
the MCP route stays synthetic and cannot dispatch hardware.
The instrument research frontier (`compile_glioma_instrument_research_frontier` and
`execute_glioma_instrument_research_frontier`) now closes the downstream autonomy gap. It scores
adjudicated assay records by information, QC, replicate support, uncertainty, cost, and status;
routes qualified results to computation, unresolved results to replication, and negative results
to falsification; then executes the bounded candidates through the P07 mission controller. No
hardware completion is promoted to evidence, and a frontier with no routable scientific action is
held explicitly.
P09 now also includes replayable computation execution (`execute_glioma_computation`). It schedules
typed multimodal DAGs in stable topological order, reuses only replay-keyed local cache artifacts,
enforces cost budgets, retries transient worker failures, and preserves negative, partial, failed,
and skipped tasks. The dry-run worker emits synthetic artifacts; production containers, GPUs, and
schedulers remain behind a caller-owned executor.
P09 now also exposes repeated-run computation reproducibility
(`glioma_computation_reproducibility`). It compares institution-local replay summaries by shared
replay identity, gates deterministic tasks on byte-identical output digests, and gates numerical
tasks on explicit effect and runtime drift thresholds. Partial, failed, undercovered, drifted,
and high-uncertainty tasks remain negative or unresolved evidence for the autonomous engine rather
than being promoted because a worker returned successfully; only typed summaries cross the route.
P09 now also includes computation-portfolio planning (`glioma_computation_portfolio_plan`). It
scores declared multimodal analyses by information gain, uncertainty reduction, coverage debt, and
resource penalties; closes prerequisite DAGs in deterministic order; and hands the existing
computation executor an execution-ready task order under cost, duration, task-count, modality, and
determinism gates. Missing or cyclic dependencies, required work that cannot fit, deferred analyses,
non-deterministic tasks, and unmet modality coverage remain explicit rather than being silently
discarded. The planner never runs external code or moves raw data; institution-local workers own
the actual computation effects.
P09 also exposes portfolio execution (`glioma_computation_portfolio_execute`), which passes the
selected prerequisite-closed DAG directly into the existing computation worker under the same
replay identity, retry/cache policy, local-artifact requirement, and resource budget. It is the
autonomous computation loop: selection, closure, execution, and result classification stay bound
together, while failed, partial, blocked, unresolved, deferred, and negative work cannot be
silently promoted into a completed research conclusion.
P09 now also includes the computation campaign controller
(`execute_glioma_computation_campaign`). It keeps a typed candidate registry across rounds,
re-plans only from returned task results, and consumes hard cost and duration budgets while
preserving replay identity, deterministic-task policy, dependency closure, cache/artifact gates,
negative results, and failed or skipped work. The MCP route uses a static dry-run planner and
worker; institution-local deployments can replace both seams to drive high-throughput multi-omics
and imaging analysis without moving raw data into the research crate.
P09 now also exposes the intent-to-workflow compiler
(`glioma_computation_workflow_execute`). A researcher declares a preclinical model, local input
artifact identifiers, modalities, and terminal analyses; the compiler expands every prerequisite
from ingest through integration, model fitting, validation, and export into a deterministic DAG.
It reports estimated cost/time shortfalls before execution and then reuses the same autonomous
campaign gates, so a high-level glioma question can become an executable local computation plan
without hand-authoring task plumbing or fabricating evidence.
P09 now also exposes robustness-guided computation (`glioma_robustness_guided_computation_execute`).
It converts leave-one-batch/row-out instability, negative cases, direction reversals, and
unresolved omissions into explicit case-targeted priority signals, reweights only the declared
typed candidates, and executes the resulting prerequisite-closed DAG under the existing resource,
determinism, replay, cache, and artifact gates. Stable robustness may hold without dispatch;
fragility and no-feasible-plan states remain visible instead of being promoted to a conclusion.
P09 now also exposes deterministic computation placement (`glioma_computation_placement`). It
maps the compiled DAG onto compatible institution-local workers, accounts for artifact locality,
transfer cost, availability windows, critical-path timing, and utilization, and reuses only
replay-valid schema-compatible local cache artifacts. Completed, cached, budget-blocked, and
worker-incompatible tasks remain explicit. The MCP route returns a pre-dispatch handoff with
preflight required; it never executes code, moves payloads, or dispatches a worker.
P09 now also exposes the computation interpretation frontier
(`glioma_computation_interpretation_frontier_compile` and
`glioma_computation_interpretation_frontier_execute`). Completed or cached computation tasks
become typed interpretation candidates, negative tasks become replication/falsification work,
and partial, failed, or skipped tasks become bounded recovery work. The bridge carries replay
identity, baseline comparison, uncertainty, and negative-result evaluation obligations into the
P07 autonomous mission controller, so computation output is never silently promoted to a
biological conclusion. Empty or held frontiers remain explicit and the MCP route stays local,
deterministic, and preclinical-only.
P10 now also exposes computation interpretation evidence adjudication
(`glioma_computation_interpretation_evidence_gate`). It binds value-only summaries to the exact
P09 routed actions, refuses observations for actions that did not complete, keeps computation and
replication evidence in separate synthesis families, and applies quality, independent-group,
negative-result, uncertainty, and replication floors before producing an interpretation state.
Missing summaries, replay drift, negative outcomes, and unresolved actions become explicit next
actions instead of being collapsed into a confident claim.
P05 now also includes counterfactual mechanism simulation (`simulate_glioma_counterfactual`). It
compares baseline and signed node perturbation fixed points over activating/inhibiting networks,
rank-orders downstream changes, and exposes low-confidence edges and non-convergence as unresolved.
The result is an assay-prioritization simulation, not a causal estimate or clinical recommendation.
The ensemble extension (`simulate_glioma_counterfactual_ensemble`) runs the same intervention
across independently declared mechanism graphs, weights effects by explicit model priors, and
withholds target direction below a model-agreement floor. Each underlying simulation remains
inspectable so disagreement is actionable rather than averaged away.
P10 also includes stratified causal adjustment (`analyze_stratified_causal_adjustment`) that
collapses repeated measurements to units, requires positivity within confounder strata, computes
pooled weighted contrasts, and exposes leave-one-stratum influence and missing coverage.
P10 also includes causal sensitivity bounds (`analyze_causal_sensitivity`) that sweep a declared
normalized hidden-confounder budget, expose worst-case threshold/sign intervals and exact tipping
strength, and report leave-one-unit-out instability before a mechanism claim can be released.
P10 also includes causal mediation analysis (`analyze_glioma_mediation`) for preclinical
interventions. It estimates mediator, total, direct, and indirect effects with integer covariance,
propagates measurement uncertainty into signal-to-noise, and runs leave-one-unit-out influence
bounds. Underpowered arms, zero mediator variance, null effects, and fragile decompositions remain
explicit rather than being promoted into mechanistic or clinical conclusions.

P06 also includes blocked randomization design (`plan_glioma_blocked_randomization`). It turns
declared nuisance strata into a deterministic allocation matrix, keeps arm counts balanced within
each block, and spends residual capacity using integer variance-aware information gain per unit
cost. Risk- and feasibility-blocked arms, capacity shortfalls, and imbalance uncertainty remain
visible; the planner never randomizes specimens or dispatches a protocol.

The prospective power-stress surface (`plan_glioma_power_stress_surface`) evaluates each candidate
arm across weighted effect, variance, attrition, risk, and budget worlds before protocol admission.
It returns integer replicate requirements, worst-case and weighted power proxies, underpowered
scenarios, and explicit blocked arms so the autonomous engine can reject brittle designs before
compilation. These are planning diagnostics, not validated biological or clinical claims.

The carryover sequence planner (`plan_glioma_carryover_sequence`) treats assay ordering as a
directed transition problem. It selects a bounded information-rich sequence while penalizing
declared carryover from the prior action, risk, feasibility, repeat limits, and cost; it reports
the exact transition penalties and any unfilled positions for researcher review.

P06 also includes multi-fidelity optimization (`plan_glioma_multi_fidelity_optimization`). It
calibrates paired designs across screening, mechanistic, and validation scales, combines transferred
and local neighborhood estimates with uncertainty, and selects a bounded next batch with a
cost/risk-aware beam search. A high-fidelity candidate without qualified lower-fidelity support is
blocked or deferred instead of being treated as an unexplained positive result.

P06 also includes deterministic active learning (`plan_glioma_active_learning`). It combines
same-candidate and nearby-candidate observations with an inverse-distance kernel surrogate, uses
conservative residual uncertainty so contradictory evidence cannot be averaged away, and selects
a diverse next assay batch under budget, risk, cost, replicate, and redundancy limits. The output
is a next-batch plan only; institution-owned executors remain responsible for any physical or
computational effect.

P07 closes that planning loop with `execute_glioma_active_learning_campaign`. The campaign
controller repeatedly invokes the P06 surrogate, sends each admitted candidate to a caller-owned
local executor, validates candidate-bound content-addressed observations, and replans from those
observations. Retry, budget, replicate, redundancy, unresolved-evidence, executor-failure, and
maximum-round gates are persisted in a replayable campaign record; the bundled MCP route uses a
deterministic sandbox executor and cannot contact hardware, move raw data, or make a clinical
decision.

P07 also exposes `execute_glioma_robust_active_learning_campaign`, which carries the ensemble
planner through repeated local assay rounds. It re-evaluates model disagreement after every typed
observation, keeps lower-tail and contradiction holds visible, and stops on explicit budget,
replicate, reliability, unresolved, retry, or executor-failure gates. The result is a resumable
research campaign rather than a one-shot ranking.

P10 adds `analyze_glioma_transportability` for the common preclinical question of whether an
effect learned in one model system is portable to another. It combines declared population
signatures, quality, replicate count, and measurement uncertainty; reports heterogeneity,
transport gap, and leave-one-study-out shifts; and refuses to promote distant, unstable, negative,
or insufficient evidence into a portability claim.

The robust active-learning surface (`plan_glioma_robust_active_learning`) keeps competing
mechanistic and spatial surrogates separate. It shrinks local observations toward reliability- and
prior-weighted predictions, scores lower-tail utility plus expected information, and treats model
disagreement, contradictory replicates, unsupported models, and safety/resource ceilings as
explicit product states. This gives a glioma program lead a defensible assay queue when models
disagree instead of a brittle single-model ranking.
The batch scheduler (`schedule_glioma_federated_evidence_batch`) coordinates many advisory cycles
for prospective high-throughput operation. It validates every cycle digest, rejects blocked or held
cycles, gives each eligible cycle a deterministic opportunity, enforces per-cycle and per-route
quotas, admits only dependency-safe action prefixes, and records every deferred candidate. Capacity
or throughput therefore never becomes a reason to hide scientific uncertainty or to execute a
physical action without its downstream authorization.
The continual promotion controller (`evaluate_glioma_continual_promotion`) closes the P01 loop over
later outcome observations. It evaluates explicit temporal windows, requires replay and independent
groups, compares recent performance with a baseline, and emits promote, continue, rollback, or hold
with a route and rollback flag. Unknown, failed, negative, and contradictory outcomes are retained;
insufficient windows cannot be promoted by absence of evidence.
