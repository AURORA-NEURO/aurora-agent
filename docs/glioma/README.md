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
  programs/p04_decision_context/context_compiler.rs
                                             P04 evidence-gap to typed next-action compilation
  programs/p05_mechanism_exploration/discrimination.rs
                                             P05 residual-likelihood mechanism discrimination and next-assay information gain
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
  programs/p01_evidence_surveillance/surveillance.rs
                                             P01 snapshot delta surveillance and prioritized evidence review actions
  programs/p08_instrument_robotics/calibration.rs
                                             P08 robust control calibration and Theil-Sen instrument drift detection
  programs/p06_experiment_design/adaptive_allocation.rs
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
    p12_federated_benchmarking/
  workflow.rs                               P07 adaptive campaign planner and guarded execution
    p07_protocol_simulation/simulator.rs    P07 deterministic resource-constrained scheduling
    p07_protocol_simulation/execution.rs   P07 guarded local protocol execution with retries
    p07_protocol_simulation/action_execution.rs
                                             P07 dependency-safe action-portfolio execution
    p07_protocol_simulation/autonomous_campaign.rs
                                             P07 observation-driven campaign replanning over local actions
    p08_instrument_robotics/preflight.rs   P08 typed instrument/robotics interlock planning
    p09_reproducible_computation/robustness.rs
                                             P09 leave-one-batch/row-out robustness battery
    p09_reproducible_computation/execution.rs
                                             P09 replayable multimodal computation DAG execution
    p10_interpretation_replication/trajectory.rs
                                             P10 longitudinal per-unit trajectory analysis
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
    p12_federated_benchmarking/consensus.rs
                                             P12 aggregate-only multi-site benchmark consensus with robust pooling and influence bounds
    p06_experiment_design/dose_response.rs   P06 monotone dose-response curve analysis
    p06_experiment_design/synergy.rs         P06 Bliss combination-response analysis
    p06_experiment_design/campaign.rs       P06 mechanism-aware closed-loop assay campaign controller and executor seam
    p06_experiment_design/information_design.rs P06 integer Bayesian assay selection by expected mechanism-information reduction
    p06_experiment_design/adaptive_information_campaign.rs P06 closed-loop posterior updating and re-planning through a caller-owned assay executor
    p03_multimodal_ingestion_qc/concordance.rs
                                             P03 feature-level modality concordance analysis
  p03_multimodal_ingestion_qc/consensus.rs
                                             P03 deterministic multimodal sample consensus clustering
  p03_multimodal_ingestion_qc/harmonization.rs
                                             P03 robust per-modality batch harmonization with explicit correction gates
  p03_multimodal_ingestion_qc/latent_factors.rs
                                             P03 robust complete-case multimodal latent-state factorization with convergence and reconstruction gates
  p03_multimodal_ingestion_qc/spatial_niche.rs
                                             P03 spatial neighbourhood graph, same-lineage niche components, and cross-lineage enrichment
  p03_multimodal_ingestion_qc/spatial_communication.rs
                                             P03 spatial ligand-receptor communication enrichment against lineage-marginal null
  p03_multimodal_ingestion_qc/spatial_propagation.rs
                                             P03 lineage-aware integer spatial-state diffusion and hotspot prioritisation
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
| P01 Evidence surveillance | evidence curator | evidence surveillance | snapshot deltas, prioritized review/revalidation actions, and stale/unknown/contradictory coverage |
| P02 Evidence-to-typed-knowledge | knowledge engineer | evidence compilation | scoped claims and competing explanations bound to source artifacts |
| P03 Multimodal ingestion and QC | data steward | multimodal ingestion/QC | comparable cells, robust batch harmonization, feature-level concordance, consensus clusters, spatial niches, ligand-receptor communication, spatial-state diffusion, and explicit defects |
| P04 Question-to-decision context | principal investigator | intent normalization, context compilation | bounded decision context and unresolved omissions |
| P05 Mechanism exploration | mechanism scientist | molecular landscape, mechanism exploration | residual-fit competing mechanisms, posterior-weighted next-assay information gain, signed mechanism-network propagation, model-averaged counterfactuals, robust lower-tail intervention portfolios, and discriminating actions |
| P06 Power-aware experiment design | experimentalist | experiment design | falsifiable allocation, power, blocking, dose-response, adaptive replicate allocation, mechanism-aware closed-loop campaign rounds, combination-synergy fitting, and null-result plan |
| P07 Protocol simulation | lab operations lead | protocol simulation, adaptive workflow planning | critical-path resource scheduling, utilization, deterministic next batches, and repair/abstain routing before physical effects |
| P08 Instrument and robotics preflight | instrument operator | instrument preflight | robust control calibration, drift detection, and signed interlocked action planning |
| P09 Reproducible computation | computational scientist | computational execution | checkpointed/replayable computation plus omission-stress robustness suite |
| P10 Causal interpretation and replication | methods reviewer | statistical interpretation, replication/robustness | uncertainty-aware endpoint, longitudinal, stratified causal, causal-contrast, meta-analytic, and cross-site verdicts |
| P11 Research-object release | reproducibility steward | research-object release | portable manifest with limitations and negative evidence |
| P12 Federated benchmarking | consortium administrator | federation benchmarking | aggregate-only cross-site benchmark consensus with robust pooling, heterogeneity, and site-influence analysis |

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

The first tranche is implemented in P01, P02, P03, P05, P06, P08, P10, P11, and P12. P07 now also has an
adaptive campaign planner (`plan_glioma_workflow`) and a guarded full-program executor that
chooses deterministic next batches, closes over dependencies, and routes unresolved evidence,
QC defects, contradictory mechanisms, underpowered designs, budget exhaustion, and approval gaps
into explicit hold/abstain branches. Checkpoint output digests are bound into the workflow plan so
a resumed campaign cannot silently swap a local evidence, QC, mechanism, or design object. P04
retains an explicit ownership folder and catalog route. P08 now includes robust instrument-control
calibration and Theil–Sen drift detection, while P12 includes aggregate-only federated benchmark
consensus with heterogeneity and leave-site-out influence bounds. P09 now includes a bounded robustness suite (`assess_glioma_robustness`) that
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
P02 now includes typed-knowledge compilation (`compile_typed_knowledge`) that coalesces scoped
claims, ranks support against contradiction, preserves negative/unknown evidence, and exposes
missing modality/model coverage for the next workflow action.
P04 now includes decision-context compilation (`compile_decision_context`) that converts those
gaps into typed A1 candidates for coverage closure, contradiction replication, negative-result
falsification, evidence resolution, or mechanism validation; the existing action selector then
applies budget and policy gates before any provider dispatch.
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
P06 now includes combination-response analysis (`analyze_glioma_combination_synergy`) with
vehicle/single-agent control requirements, integer Bliss expectations, residual noise, synergy,
antagonism, and explicit unresolved cells for missing controls or replicates.
P06 also includes sequential assay allocation (`allocate_glioma_assays`) using Beta posteriors,
conservative Cantelli target-effect bounds, uncertainty exploration, replicate floors, risk
ceilings, and a hard next-batch budget.
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
P08 now also includes deterministic instrument preflight (`preflight_glioma_instrument`). It combines
qualified calibration, live interlock telemetry, typed operation parameters, operator authorization,
serialized scheduling, and risk/duration budgets into a dispatch-permitted or fail-closed plan. The
MCP route only emits the plan; a local gateway must re-verify authorization before any hardware
effect, and missing telemetry remains unresolved rather than imputed.
P09 now also includes replayable computation execution (`execute_glioma_computation`). It schedules
typed multimodal DAGs in stable topological order, reuses only replay-keyed local cache artifacts,
enforces cost budgets, retries transient worker failures, and preserves negative, partial, failed,
and skipped tasks. The dry-run worker emits synthetic artifacts; production containers, GPUs, and
schedulers remain behind a caller-owned executor.
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
