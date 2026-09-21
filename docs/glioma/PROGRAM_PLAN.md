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
- Current implementation: 18/32 slots. Next wave: federated continual context promotion,
  richer outcome assimilation, and decision-context replay across study epochs.

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
- Current implementation: 16/32 slots. Next wave: failure recovery, queue-aware fleet routing,
  and instrument-to-analysis provenance binding.

### P09 — `p09_reproducible_computation`

- Consumer: computational scientist and workflow runtime.
- Product contract: checkpointed multimodal DAGs, resource placement, replay, recovery,
  robustness-guided computation, and interpretation-frontier compilation.
- Primary artifacts: `ComputationRun`, replay tapes, robustness suites, and computation portfolios.
- Downstream edges: P10 interpretation, P11 release, and P12 federated benchmark aggregation.
- Promotion gate: byte-stable canonicalization, resource termination, crash/retry recovery,
  negative-result retention, and independent replay.
- Current implementation: 13/32 slots. Next wave: placement optimization, artifact lineage joins,
  and adaptive robustness-guided recomputation.

### P10 — `p10_interpretation_replication`

- Consumer: methods reviewer and replication scientist.
- Product contract: uncertainty-aware causal interpretation, transportability, mediation,
  dynamic policies, contradiction adjudication, replication closure, and adaptive frontier work.
- Primary artifacts: `AnalysisReplicationRecord`, causal claim adjudications, and replication
  closure campaigns.
- Downstream edges: P06 follow-up design, P11 release verdicts, and P12 consortium comparisons.
- Promotion gate: estimand clarity, uncertainty, sensitivity, rival explanations, independent
  reproduction, and explicit null/negative outcomes.
- Current implementation: 26/32 slots. Next wave: longitudinal replication transport and
  prospective contradiction resolution.

### P11 — `p11_research_object_release`

- Consumer: reproducibility steward and public research commons.
- Product contract: portable research-object assembly, replay campaigns, release gates, limitations,
  provenance, and immutable release lifecycle.
- Primary artifacts: `SignedResearchObject`, release manifests, replay reports, and release gates.
- Downstream edges: P12 federation and every upstream program's publication handoff.
- Promotion gate: complete provenance, methods/limitations, replay evidence, policy-compliant
  localization, signed checksums, and negative-result disclosure.
- Current implementation: 7/32 slots. Next wave: multimodal object packaging, migration checks,
  dependency closure, and long-horizon archival replay.

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
- Current implementation: 10/32 slots. Next wave: benchmark world versioning, adaptive site
  selection, and cross-consortium negative-result registries.

## Change control

Every new feature must first be assigned a folder, stable `GAF-GLIOMA-P##-F##` slot, consumer,
typed contract, dependency edges, and acceptance gate in this plan. Then it may add code under the
owning folder, exports, MCP/CLI/API surfaces, tests, and documentation. The organization validator,
catalogue validator, full crate tests, protocol tests, and replay/negative-path tests are release
gates. A feature is not counted for coverage merely because a file or hypothesis exists.
