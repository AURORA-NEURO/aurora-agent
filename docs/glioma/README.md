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
  programs/p02_evidence_knowledge/knowledge_graph.rs
                                             P02 scoped claim graph and support/contradiction synthesis
  programs/p02_evidence_knowledge/composition.rs
                                             P02 explicit relation graph composition with path bottlenecks and contradiction gates
  programs/p02_evidence_knowledge/claim_frontier.rs
                                             P02 uncertainty/coverage/contradiction frontier prioritization
  programs/p04_decision_context/context_compiler.rs
                                             P04 evidence-gap to typed next-action compilation
  programs/p04_decision_context/action_graph.rs
                                             P04 claim-path to dependency-closed action DAG and parallel waves
  programs/p04_decision_context/action_bridge.rs
                                             P04 compiler-to-executable action portfolio bridge
  programs/p04_decision_context/campaign.rs
                                             P04 bounded question-to-action campaign with evidence-driven replanning
  programs/p05_mechanism_exploration/discrimination.rs
                                             P05 residual-likelihood mechanism discrimination and next-assay information gain
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
  programs/p06_experiment_design/clonal_panel.rs
                                             P06 clone-aware perturbation/readout panel selection under cost and branch-coverage gates
  programs/p06_experiment_design/contrast_design.rs
                                             P06 balanced factorial contrast-panel compiler with interaction and budget gates
  programs/p06_experiment_design/adaptive_dose_surface.rs
                                             P06 uncertainty-aware adaptive combination dose-surface acquisition planning
  programs/p10_interpretation_replication/clone_outcomes.rs
                                             P10 replicate-level clone-panel outcome adjudication with explicit null/contradictory evidence
  programs/p01_evidence_surveillance/surveillance.rs
                                             P01 snapshot delta surveillance and prioritized evidence review actions
  programs/p01_evidence_surveillance/priority.rs
                                             P01 recency/state/coverage action queue for the next autonomous cycle
  programs/p01_evidence_surveillance/campaign.rs
                                             P01 bounded autonomous evidence refresh and round-by-round surveillance replanning
  programs/p01_evidence_surveillance/triangulation.rs
                                             P01 cross-family claim triangulation with contradiction and source-dominance gates
  programs/p02_evidence_knowledge/campaign.rs
                                             P02 bounded claim-resolution campaign with frontier actions and knowledge recompilation
  programs/p03_multimodal_ingestion_qc/campaign.rs
                                             P03 bounded metadata-only ingestion/QC campaign with defect-aware replanning
  programs/p08_instrument_robotics/calibration.rs
                                             P08 robust control calibration and Theil-Sen instrument drift detection
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
    p12_federated_benchmarking/
    p12_federated_benchmarking/mechanism_transport.rs
                                             P12 aggregate-only cross-model mechanism transport and fragility analysis
    p12_federated_benchmarking/site_planner.rs
                                             P12 conservative influence-aware consortium expansion and site portfolio planning
  workflow.rs                               P07 adaptive campaign planner and guarded execution
    p07_protocol_simulation/simulator.rs    P07 deterministic resource-constrained scheduling
    p07_protocol_simulation/execution.rs   P07 guarded local protocol execution with retries
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
    p07_protocol_simulation/evidence_gate.rs
                                             P07 evidence-gated director admission from P01 cross-family triangulation
    p07_protocol_simulation/clone_continuation.rs
                                             P07 clone-outcome-driven continuation planning with dependency closure and policy gates
    p08_instrument_robotics/preflight.rs   P08 typed instrument/robotics interlock planning
    p08_instrument_robotics/execution.rs   P08 guarded execution with live rechecks and emergency stop
    p08_instrument_robotics/campaign.rs   P08 ordered multi-run instrument campaign with fail-closed safety halts
    p08_instrument_robotics/assay_adjudication.rs
                                             P08 typed assay/QC adjudication that separates hardware completion from biological evidence
    p09_reproducible_computation/robustness.rs
                                             P09 leave-one-batch/row-out robustness battery
    p09_reproducible_computation/execution.rs
                                             P09 replayable multimodal computation DAG execution
    p09_reproducible_computation/planning.rs
                                             P09 budgeted computation-portfolio planning with prerequisite closure
    p09_reproducible_computation/portfolio_execution.rs
                                             P09 autonomous portfolio-to-computation execution bridge
    p09_reproducible_computation/campaign.rs
                                             P09 bounded multi-round computation campaign with typed replanning
    p09_reproducible_computation/workflow.rs
                                             P09 intent-to-DAG compiler for modality-aware autonomous computation
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
    p12_federated_benchmarking/consensus.rs
                                             P12 aggregate-only multi-site benchmark consensus with robust pooling and influence bounds
    p12_federated_benchmarking/campaign.rs
                                             P12 autonomous aggregate-only benchmark follow-up campaign with deterministic replanning
    p11_research_object_release/replay.rs
                                             P11 dependency-aware reproducibility replay campaign and release-readiness gate
    p06_experiment_design/dose_response.rs   P06 monotone dose-response curve analysis
    p06_experiment_design/synergy.rs         P06 Bliss combination-response analysis
    p06_experiment_design/campaign.rs       P06 mechanism-aware closed-loop assay campaign controller and executor seam
    p06_experiment_design/information_design.rs P06 integer Bayesian assay selection by expected mechanism-information reduction
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
    p05_mechanism_exploration/pathway_activity.rs
                                             P05 signed pathway activity inference with cross-modal confidence and bottleneck gates
    p05_mechanism_exploration/adaptive_policy.rs
                                             P05 finite-horizon model-uncertainty policy with Gini information gain and local execution loop
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
| P01 Evidence surveillance | evidence curator | evidence surveillance | snapshot deltas, recency/state/coverage action queues, cross-family claim triangulation, review/revalidation actions, and stale/unknown/contradictory coverage |
| P02 Evidence-to-typed-knowledge | knowledge engineer | evidence compilation | scoped claims, ranked uncertainty frontiers, and competing explanations bound to source artifacts |
| P03 Multimodal ingestion and QC | data steward | multimodal ingestion/QC | comparable cells, robust batch harmonization, feature-level concordance, consensus clusters, spatial niches, ligand-receptor communication, cross-sample registration, spatial-state diffusion, and explicit defects |
| P04 Question-to-decision context | principal investigator | intent normalization, context compilation | bounded decision context, selected executable action batches, and unresolved omissions |
| P05 Mechanism exploration | mechanism scientist | molecular landscape, mechanism exploration | residual-fit competing mechanisms, posterior-weighted next-assay information gain, signed mechanism-network propagation, model-averaged counterfactuals, robust lower-tail intervention portfolios, and discriminating actions |
| P06 Power-aware experiment design | experimentalist | experiment design | falsifiable allocation, power, blocking, dose-response, adaptive replicate allocation, uncertainty-aware dose-surface acquisition, mechanism-aware closed-loop campaign rounds, combination-synergy fitting, and null-result plan |
| P07 Protocol simulation | lab operations lead | protocol simulation, adaptive workflow planning | critical-path scheduling, evidence-gated director admission, evidence-priority execution cycles, context-to-action execution, multimodal mechanism campaigns, utilization, deterministic next batches, and repair/abstain routing before physical effects |
| P08 Instrument and robotics preflight | instrument operator | instrument preflight | robust control calibration, drift detection, signed interlocked planning, guarded execution, and fail-closed multi-run campaigns |
| P09 Reproducible computation | computational scientist | computational execution | checkpointed/replayable computation, intent-to-DAG compilation, budgeted portfolio execution, and omission-stress robustness suite |
| P10 Causal interpretation and replication | methods reviewer | statistical interpretation, replication/robustness | uncertainty-aware endpoint, longitudinal, stratified causal, causal-contrast, meta-analytic, and cross-site verdicts |
| P11 Research-object release | reproducibility steward | research-object release | portable manifest with limitations and negative evidence |
| P12 Federated benchmarking | consortium administrator | federation benchmarking | aggregate-only cross-site benchmark consensus, influence-aware site portfolio planning, robust pooling, heterogeneity, and site-influence analysis |

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
omission, retry, and budget outcomes. P07 now also has an
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
decision-maker. P09 now includes a bounded robustness suite (`assess_glioma_robustness`) that
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
P04 now includes decision-context compilation (`compile_decision_context`) that converts those
gaps into typed A1 candidates for coverage closure, contradiction replication, negative-result
falsification, evidence resolution, or mechanism validation; the existing action selector then
applies budget and policy gates before any provider dispatch.
The decision-action graph (`compile_decision_action_graph`) joins that context with P02 composed
claim paths. It adds explicit prerequisite edges, computes deterministic topological order and
parallel waves, reports critical-path and total cost, and keeps missing claims, unresolved paths,
negative evidence, and budget blocks visible for the autonomous engine.
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

P05 now also exposes deterministic mechanism calibration
(`calibrate_glioma_mechanisms`). It scores competing mechanism probabilities against typed local
observations with fixed reliability bins, Brier loss, sharpness, and a final-round prequential
holdout. Underpowered mechanisms, high-uncertainty observations, discordant negative evidence, and
calibration/Brier gate failures remain explicit; calibration never refits a model or becomes a
causal claim, and the MCP route never executes an assay or moves raw data.

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
P09 now also includes replayable computation execution (`execute_glioma_computation`). It schedules
typed multimodal DAGs in stable topological order, reuses only replay-keyed local cache artifacts,
enforces cost budgets, retries transient worker failures, and preserves negative, partial, failed,
and skipped tasks. The dry-run worker emits synthetic artifacts; production containers, GPUs, and
schedulers remain behind a caller-owned executor.
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
