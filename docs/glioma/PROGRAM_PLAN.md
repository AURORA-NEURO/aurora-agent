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

## Change control

Every new feature must first be assigned a folder, stable `GAF-GLIOMA-P##-F##` slot, consumer,
typed contract, dependency edges, and acceptance gate in this plan. Then it may add code under the
owning folder, exports, MCP/CLI/API surfaces, tests, and documentation. The organization validator,
catalogue validator, full crate tests, protocol tests, and replay/negative-path tests are release
gates. A feature is not counted for coverage merely because a file or hypothesis exists.
