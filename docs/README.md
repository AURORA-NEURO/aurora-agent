# Documentation map

This index groups the reference by the work a reader is trying to do. The links cover every
top-level Markdown reference in this directory; the `glioma/` program references are indexed
separately. The [repository README](../README.md) remains the entry point for installation and
commands.

## Start with the project and architecture

- [Architecture and crate ownership](ARCHITECTURE.md)
- [Language strategy ADR](ADR-001-language-strategy.md)
- [Blueprint coverage method and current coverage](COVERAGE.md)
- [Uncovered work and its limits](BACKLOG.md)
- [Measured findings](FINDINGS.md)
- [Project modeling](PROJECT_MODELING.md)
- [Generalizing beyond biological domains](GENERALIZATION.md)
- [BioAtlas surface audit](ATLAS_SURFACE_AUDIT.md)

## Build and integrate the autonomous agent

- [Autonomous brain runtime](AUTONOMOUS_BRAIN.md) — routing, execution, learning, memory, goals,
  evidence, persistence, recovery, and deployment boundaries.
- [Execution policy](AUTONOMOUS_EXECUTION_POLICY.md)
- [Provider streaming](AUTONOMOUS_STREAMING.md)
- [Autopilot](AUTOPILOT.md)
- [Python SDK](PYTHON_SDK.md)
- [TypeScript SDK](TYPESCRIPT_SDK.md)
- [Autonomous GitHub Action](GITHUB_ACTION.md)
- [HTTP API](HTTP_API.md)
- [Workflow execution binding](WORKFLOW_EXECUTION_BINDING.md)
- [Execution provenance](EXECUTION_PROVENANCE.md)
- [Capability dashboard](CAPABILITY_DASHBOARD.md)
- [Issue repair](ISSUE_REPAIR.md)
- [Neurosurgical research agent](NEUROSURGICAL_AGENT.md)

## Evaluate, compare, and adapt systems

- [Baseline comparison](BASELINE_COMPARISON.md)
- [Benchmark compilation](BENCHMARK_COMPILE.md)
- [Counterfactual benchmark checks](BENCHMARK_COUNTERFACTUAL_CHECK.md)
- [Benchmark decision audit](BENCHMARK_DECISION_AUDIT.md)
- [Benchmark integrity audit](BENCHMARK_INTEGRITY_AUDIT.md)
- [Benchmark oracle review](BENCHMARK_ORACLE_REVIEW.md)
- [Decision quotient](DECISION_QUOTIENT.md)
- [Discriminating comparisons](DISCRIMINATING_COMPARISON.md)
- [Epistemic adaptive acquisition](EPISTEMIC_ADAPTIVE_ACQUISITION.md)
- [Epistemic adaptive execution](EPISTEMIC_ADAPTIVE_EXECUTION.md)
- [Epistemic context audit](EPISTEMIC_CONTEXT_AUDIT.md)
- [Epistemic cost vectors](EPISTEMIC_COST_VECTORS.md)
- [Epistemic selection audit](EPISTEMIC_SELECTION_AUDIT.md)
- [FIBER adaptive acquisition](FIBER_ADAPTIVE_ACQUISITION.md)
- [FIBER rate-distortion analysis](FIBER_RATE_DISTORTION.md)
- [Evaluation reproduction checks](EVALUATION_REPRODUCTION_CHECK.md)
- [Evaluation trajectory checks](EVALUATION_TRAJECTORY_CHECK.md)
- [Evaluation worldline audit](EVALUATION_WORLDLINE_AUDIT.md)
- [Lab branch audit](LAB_BRANCH_AUDIT.md)
- [Lab evolution audit](LAB_EVOLUTION_AUDIT.md)
- [Lab holdout audit](LAB_HOLDOUT_AUDIT.md)
- [Lab Pareto audit](LAB_PARETO_AUDIT.md)
- [Lab search-space audit](LAB_SPACE_AUDIT.md)
- [Oracle combination](ORACLE_COMBINE.md)
- [Posterior gate](POSTERIOR_GATE.md)
- [Routing lab run](ROUTING_LAB_RUN.md)

## Verify evidence, claims, and biomedical evaluations

- [Bioevaluation acquisition audit](BIOEVAL_ACQUISITION_AUDIT.md)
- [Bioevaluation boundary audit](BIOEVAL_BOUNDARY_AUDIT.md)
- [Bioevaluation burden audit](BIOEVAL_BURDEN_AUDIT.md)
- [Bioevaluation design audit](BIOEVAL_DESIGN_AUDIT.md)
- [Bioevaluation estimand audit](BIOEVAL_ESTIMAND_AUDIT.md)
- [Bioevaluation evaluator audit](BIOEVAL_EVALUATOR_AUDIT.md)
- [Bioevaluation grounding audit](BIOEVAL_GROUNDING_AUDIT.md)
- [Bioevaluation mesh audit](BIOEVAL_MESH_AUDIT.md)
- [Bioevaluation metamorphic audit](BIOEVAL_METAMORPHIC_AUDIT.md)
- [Bioevaluation plane audit](BIOEVAL_PLANE_AUDIT.md)
- [Bioevaluation reference audit](BIOEVAL_REFERENCE_AUDIT.md)
- [Bioevaluation reveal audit](BIOEVAL_REVEAL_AUDIT.md)
- [Bioevaluation waiver audit](BIOEVAL_WAIVER_AUDIT.md)
- [Literature binding check](LITERATURE_BIND_CHECK.md)
- [Modality comparability check](MODALITY_COMPARABILITY_CHECK.md)
- [Modality support check](MODALITY_SUPPORT_CHECK.md)
- [Modality transport check](MODALITY_TRANSPORT_CHECK.md)
- [Obligation gate check](OBLIGATION_GATE_CHECK.md)
- [Research workflow](RESEARCH.md)
- [Research figures](FIGURES.md)

## Inspect oncology and worldline evidence

- [Oncology boundary check](ONCO_BOUNDARY_CHECK.md)
- [Oncology classification check](ONCO_CLASSIFICATION_CHECK.md)
- [Oncology outcome analysis](ONCO_OUTCOME_ANALYZE.md)
- [Oncology response assessment](ONCO_RESPONSE_ASSESS.md)
- [Oncology worldline view](ONCO_WORLDLINE_VIEW.md)
- [OncoWorlds clonal evidence](ONCOWORLDS_CLONAL_EVIDENCE.md)
- [OncoWorlds clonal-history check](ONCOWORLDS_CLONAL_HISTORY_CHECK.md)
- [OncoWorlds entity worlds](ONCOWORLDS_ENTITY_WORLDS.md)
- [OncoWorlds identity join](ONCOWORLDS_IDENTITY_JOIN.md)
- [OncoWorlds methylation](ONCOWORLDS_METHYLATION.md)
- [OncoWorlds model transport](ONCOWORLDS_MODEL_TRANSPORT.md)
- [OncoWorlds radiogenomic check](ONCOWORLDS_RADIOGENOMIC_CHECK.md)
- [OncoWorlds shift and equity](ONCOWORLDS_SHIFT_EQUITY.md)
- [Glioma program overview](glioma/README.md)
- [Glioma program plan](glioma/PROGRAM_PLAN.md)

## Operate, release, and secure deployments

- [Bundle signatures](BUNDLE_SIGNATURES.md)
- [CI evidence](CI_EVIDENCE.md)
- [Engineering manifest audit](ENGINEERING_MANIFEST_AUDIT.md)
- [Engineering execution plan](ENGINEERING_EXECUTION_PLAN.md)
- [Operational readiness audit](OPERATIONAL_READINESS_AUDIT.md)
- [OCI sandbox](OCI_SANDBOX.md)
- [Pack coverage audit](PACK_COVERAGE_AUDIT.md)
- [Pack release audit](PACK_RELEASE_AUDIT.md)
- [Receipt audit](RECEIPTS_AUDIT.md)
- [Release pipeline audit](RELEASE_PIPELINE_AUDIT.md)
- [Runtime execution simulation](RUNTIME_EXECUTION_SIMULATE.md)
- [Runtime tape verification](RUNTIME_TAPE_VERIFY.md)
- [Sandbox admission audit](SANDBOX_ADMISSION_AUDIT.md)
- [Sandbox runtime simulation](SANDBOX_RUNTIME_SIMULATION.md)
- [Security and privacy audit](SECURITY_PRIVACY_AUDIT.md)
- [Security program audit](SECURITY_PROGRAM_AUDIT.md)
- [OTLP trace ingestion](OTLP_TRACE_INGEST.md)
