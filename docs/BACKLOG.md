# Remaining backlog

287 code-bearing blueprint modules are not yet cited by any crate or design note,
across 25 sections. This is the enumerated form of `docs/COVERAGE.md`'s
percentage: a percentage says how far there is to go, a list says what is actually left.

Regenerate with `tools/backlog.sh`. Programme sections are excluded for the same reason they
are excluded from the coverage denominator — they specify no behaviour.

A module leaves this list when a crate cites it, which is the same weak criterion coverage
uses. This file tracks *attention*, not completeness.


## §23 Agent Interweave Fabric — 23 uncovered

- `23.00` Interweave Overview And Thesis
- `23.01` Layered Protocol Stack
- `23.11` Capability Molecules And Virtual Agents
- `23.14` Effect Types Permissions And Information Flow
- `23.15` Negotiation Bidding And Service Contracts
- `23.17` Shared Memory Blackboard And Replicated State
- `23.24` Protocol Adapters A2A Mcp Otel And Cloudevents
- `23.25` Component Runtime Wasm Wit And Sandbox Composition
- `23.27` Interweave Evaluation And Microbenchmark Generation
- `23.28` Orchestration Learning And Credit Assignment
- `23.29` Security Threat Model And Trust Boundaries
- `23.31` Governance Versioning And Conformance
- `23.33` Reference Interweave Workflows
- `23.35` Implementation Roadmap And Vertical Slices
- `23.36` Open Research Program
- `23.39` Weavebench Packs And Microbenchmark Taxonomy
- `23.40` Novelty Boundary And Ecosystem Positioning
- `23.41` Agent Composition Algebra And Substitution Laws
- `23.42` Goal To Molecule Compiler And Team Synthesis
- `23.44` Distributed Epistemic Crdt And Common Ground
- `23.46` Agent Identity Attested Capability And Contextual Reputation
- `23.47` Human And Organizational Participants
- `23.48` Semantic Lifecycle Compaction And Garbage Collection

## §11 Developer Platform — 20 uncovered

- `11.02` Cli
- `11.03` Cli Specification
- `11.04` Python Sdk
- `11.05` Python Benchmark Authoring Sdk
- `11.06` Typescript Sdk
- `11.07` Rest And Streaming Api
- `11.08` Rest Grpc And Event Apis
- `11.09` Event Stream And Webhooks
- `11.10` Mcp Server
- `11.15` Evaluator Oracle And Mutation Sdk
- `11.16` Environment And Pack Authoring Sdk
- `11.17` Authoring Studio
- `11.18` Authoring Studio And Notebook Workflow
- `11.19` Trace Fork And Diff Viewer
- `11.20` Capability Dashboard And Query
- `11.21` Github Action For Consumer Repositories
- `11.22` Github Action And Ci Integration
- `11.23` Reporting And Exports
- `11.24` Scaffolding Templates And Conformance
- `11.25` Local Development Documentation And Telemetry

## §27 Benchmark Factory And Hub — 20 uncovered

- `27.01` Parent Bioworld Authoring
- `27.02` Observed Real Data Worlds
- `27.03` Semi Synthetic Worlds
- `27.04` Mechanistic And Simulated Worlds
- `27.05` Prospective Escrow And Holdout Vault
- `27.06` Trajectory Mining And Decision Compilation
- `27.07` Biomutator Engine
- `27.08` Semantics Preserving Mutations
- `27.09` Controlled Semantic Mutations
- `27.10` Assay Fault And Preanalytic Mutations
- `27.11` Specimen Lineage And Identity Mutations
- `27.12` Site Batch Platform And Population Mutations
- `27.13` Temporal And Treatment History Mutations
- `27.14` Multimodal Contradiction Programs
- `27.15` Deduplication Diversity And Effective Size
- `27.17` Hub Domain Model
- `27.19` World Pack Oracle And Result Cards
- `27.20` Submissions Challenges And Reproduction
- `27.21` Private And Federated Evaluation
- `27.22` Hub Apis Sdk And Visualization

## §14 Governance And Quality — 18 uncovered

- `14.01` Project Governance
- `14.02` Open Governance And Rfc Process
- `14.03` Roles Ownership And Maintainer Model
- `14.04` Contributor Model And Code Ownership
- `14.05` Rfc Adr And Technical Decision Process
- `14.07` Benchmark Governance And Stewardship
- `14.08` Pack Review And Acceptance
- `14.09` Oracle And Evaluator Review
- `14.10` Metric Score And Statistical Governance
- `14.11` Result Claims And Leaderboard Governance
- `14.13` Benchmark Ethics Fairness And Conflicts
- `14.14` Medical And Neuroscience Boundary
- `14.15` Data Governance Federation And Access
- `14.18` Conflicts Of Interest And Sponsorship
- `14.22` Documentation Information Architecture And Review
- `14.23` Community Conduct Inclusion And Appeals
- `14.24` Sustainability Finance And Public Benefit
- `14.25` Periodic Program Review

## §19 Reference Examples — 18 uncovered

- `19.01` Decision Cell Example
- `19.02` Agent Architecture Ir Example
- `19.03` Benchmark Pack Manifest Example
- `19.04` Oracle Example
- `19.07` Capability Profile Example
- `19.08` Github Action Example
- `19.10` Pack Directory Reference
- `19.11` Duplicate Payment Vertical Slice
- `19.12` Scientific Figure Reproduction Case
- `19.13` Neuro Oncology Research Workflow Case
- `19.14` Adaptive Scheduler Worked Example
- `19.15` Evaluation Conditioned Routing Example
- `19.17` Security Exploit Decision Cell
- `19.18` Private Incident To Public Derivative
- `19.19` Local Cli Session
- `19.20` Federated Registry Flow
- `19.21` Reliable Repair Weave Program
- `19.22` Scientific Reproduction Capability Molecule

## §28 Biology Data And Standards — 17 uncovered

- `28.02` Epigenomics And Methylation
- `28.03` Bulk Transcriptomics
- `28.04` Single Cell And Multiome
- `28.05` Spatial Omics
- `28.06` Proteomics And Proteogenomics
- `28.07` Metabolomics And Flux
- `28.08` Perturbation Crispr And Functional Screens
- `28.09` Protein Structure And Engineering
- `28.10` Drug Discovery Pharmacology And Pk
- `28.11` Microbiome And Metagenomics
- `28.12` Microscopy And High Content Imaging
- `28.14` Digital Pathology
- `28.15` Clinical Research Ehr And Clinicogenomics
- `28.16` Clinical Trials And Real World Evidence
- `28.17` Literature Knowledge Bases And Citations
- `28.18` Model Organisms Cross Species And Preclinical Models
- `28.20` Neuro Oncology Data Atlas And Connectors

## §34 Bioatlas Public Hub And Ecosystem — 17 uncovered

- `34.01` Users Personas And Jobs To Be Done
- `34.02` Information Architecture And Navigation
- `34.03` Bioworld Registry And World Cards
- `34.04` Worldline Timeline And State Explorer
- `34.05` Biodecision Cell Inference Microscope
- `34.06` Fork Compare And Counterfactual Lab
- `34.07` Oracle Evidence And Disagreement Explorer
- `34.08` Biocapability Atlas
- `34.09` Failure Atlas And First Divergence Browser
- `34.10` Value Of Experiment And Active Evaluation Lab
- `34.11` Architecture And Agent Molecule Registry
- `34.12` Data Connector And Research Object Registry
- `34.17` Private Federated And Bring Your Own Data Evaluation
- `34.19` Notebook Ide Mcp And Agent Integrations
- `34.20` Github Actions And Research Ci
- `34.21` No Key Demos And Onboarding
- `34.22` Open Source Community And Star Flywheel

## §12 Data And Infrastructure — 16 uncovered

- `12.01` Data Architecture Overview
- `12.02` Storage Architecture
- `12.03` Relational Catalog Schema
- `12.05` Content Addressed Object Storage
- `12.06` Result Lake And Analytical Model
- `12.07` Search Graph And Vector Projections
- `12.08` Search Cache And Queue
- `12.09` Workflow Queue And Operation Engine
- `12.11` Evaluation Telemetry And Provenance Signals
- `12.12` Observability And Slos
- `12.13` Compute Provider And Kubernetes
- `12.14` Distributed Compute And Placement
- `12.15` Local First Deployment
- `12.16` Cloud And Federated Deployment
- `12.18` Backup Disaster Recovery And Retention
- `12.20` Cost Model And Capacity Plan

## §33 Biocapability Atlas And Metrics — 15 uncovered

- `33.02` Biological Correctness Decomposition
- `33.03` Evidence Grounding Provenance And Claim Support
- `33.05` Information Acquisition And Context Value
- `33.06` Value Of Experiment Assay Selection And Active Discovery
- `33.07` Tissue Sample Time And Resource Efficiency
- `33.08` Temporal Validity And Evidence Firewall Metrics
- `33.09` Cross Modal Consistency And Contradiction Metrics
- `33.10` Causal Identification Intervention And Mechanism Metrics
- `33.11` Site Batch Population And Temporal Generalization
- `33.12` Reproducibility Reexecution And Claim Stability
- `33.13` Translation Spine And Evidence Maturity Metrics
- `33.14` Multi Agent Coordination And Molecule Value
- `33.15` Safety Privacy Dual Use And Boundary Metrics
- `33.16` Cost Latency Energy And Operational Reliability
- `33.17` Matched Counterfactual Architecture Attribution

## §10 Registry And Hub — 13 uncovered

- `10.01` Registry Overview
- `10.03` Registry Entity And Metadata Model
- `10.06` Submission Review And Maintenance
- `10.09` Result Ingestion And Attestation
- `10.10` Catalog Search And Recommendation
- `10.11` Search Discovery And Recommendation
- `10.12` Benchmark Cards Health And Disclosure
- `10.14` Trace And Fork Explorer
- `10.15` Capability Atlas And Failure Explorer
- `10.16` Comparison And Leaderboard Policy
- `10.17` Hub Api And Leaderboard Policy
- `10.21` Web Ux Accessibility And Internationalization
- `10.22` Moderation Abuse And Content Policy

## §25 Biological Ir And Language — 13 uncovered

- `25.01` Bioworld Ir
- `25.02` Biostate Ir
- `25.06` Intervention And Action Ir
- `25.07` Fbc Ir
- `25.09` Bioworldline Ir
- `25.14` Model Pipeline Agent Ir
- `25.15` Bioweave Role And Act Ir
- `25.16` Bio Context Capsule Ir
- `25.17` Bio Capability Molecule Ir
- `25.18` Biooracle Ir
- `25.19` Biomutation Ir
- `25.20` Bioresult Bundle Ir
- `25.21` Bioql Query Language

## §40 Build Ready Engineering Contracts — 13 uncovered

- `40.01` Technology Baseline
- `40.02` Monorepo And Package Layout
- `40.10` Configuration Secrets And Feature Flags
- `40.15` Typescript Sdk Contract
- `40.34` Observability Telemetry And Audit
- `40.35` Performance Capacity And Load Model
- `40.38` Deployment Profiles And Infrastructure
- `40.39` Security Threat Model And Hardening
- `40.40` Ci Cd And Release Automation
- `40.41` First 100 Implementation Tickets
- `40.42` Alpha Acceptance Criteria
- `40.43` Engineering Adr Register
- `40.45` Ownership Raci And Maintainer Boundaries

## §26 Bio Evaluation Engine — 12 uncovered

- `26.01` Oracle Mesh And Priority
- `26.03` Evidence Grounding And Provenance
- `26.05` Information Acquisition And Context Value
- `26.06` Tissue Resource And Burden Efficiency
- `26.07` Temporal Reasoning And Worldline Validity
- `26.09` Causal And Intervention Evaluation
- `26.11` Reproducibility And Computational Validity
- `26.12` Metamorphic Robustness
- `26.16` Prospective Blind Reveal
- `26.17` Cross System Evaluation
- `26.18` Matched Counterfactual Architecture Studies
- `26.19` Biocapability Atlas

## §32 Biological Mutation And Stress Program — 12 uncovered

- `32.05` Specimen Identity Swap Mixture And Relatedness Mutations
- `32.10` Modality Missingness Access And Censoring Mutations
- `32.12` Label Noise Weak Supervision And Oracle Mutations
- `32.13` Multimodal Contradiction And Partial Alignment Mutations
- `32.14` Tool Pipeline Dependency And Execution Fault Mutations
- `32.15` Literature Citation Staleness And Adversarial Evidence Mutations
- `32.16` Units Scales Normalization And Threshold Mutations
- `32.17` Causal Intervention And Counterfactual Mutations
- `32.18` Expert Disagreement And Policy Mutations
- `32.19` Privacy Permission And Data Locality Mutations
- `32.20` Mechanistic Simulation And Digital Twin Mutations
- `32.21` Mutation Composition Interactions And Minimization

## §39 Token Efficient Biological Inference — 11 uncovered

- `39.01` Token Economy Thesis
- `39.11` Multi Agent Context Projection
- `39.13` Table Matrix Image And Sequence Summarization
- `39.14` Literature Claim Context
- `39.15` Oncoworld Temporal Context
- `39.18` Staleness Ttl And Recomputation
- `39.20` Context Compiler Api And Cli
- `39.21` Context Testing And Golden Fixtures
- `39.23` Ablations And Experimental Design
- `39.24` Failure Modes And Recovery
- `39.25` Implementation Plan

## §43 Fiber Query Compiled Epistemic Calculus — 11 uncovered

- `43.14` Submodular And Coverage Aware Evidence Selection
- `43.29` Multi Agent Separator Protocol
- `43.30` Weave Continuations Over Fibers
- `43.31` Biological Scope And Factor Library
- `43.32` Oncoworld Neuro Oncology Query Patterns
- `43.44` Research Paper And Theorem Agenda
- `43.45` Glossary And Notation
- `43.46` Primary Source And Implementation Ledger
- `43.47` Formal Semantics And Theorem Sketches
- `43.49` Dependent Types Optics And Query Lenses
- `43.50` Causal Decision Rate Distortion And Value Of Information

## §31 Biological Oracles And Reference Standards — 9 uncovered

- `31.05` Sample Identity And Lineage Oracles
- `31.06` Multi Reader Expert And Consensus Oracles
- `31.07` Orthogonal Assay And Cross Modal Oracles
- `31.08` Perturbation Rescue And Causal Oracles
- `31.09` Longitudinal Confirmation And Blind Reveal Oracles
- `31.10` Imaging Segmentation And Geometric Reference Standards
- `31.11` Pathology Molecular And Integrated Reference Standards
- `31.12` Survival Endpoint And Clinical Research Oracles
- `31.17` Oracle Audit Independence And Quality Management

## §36 Biology Security Privacy Ethics And Governance — 7 uncovered

- `36.07` Sandboxing Untrusted Code And Research Artifacts
- `36.10` Physical Experiment And Wetlab Action Boundaries
- `36.11` Dual Use Biosecurity And Capability Release
- `36.13` Fairness Representation And Global Resource Context
- `36.19` Security Privacy Safety Red Team Program
- `36.21` Quality Management Validation And Release Gates
- `36.22` Research Ethics Irb And Human Subject Boundaries

## §35 Million Scale Benchmark Factory And Infrastructure — 6 uncovered

- `35.02` Observed Data World Authoring
- `35.03` Semi Synthetic World Construction
- `35.04` Mechanistic Simulation And Assay Twin Factory
- `35.06` Trajectory Capture And Research Workflow Mining
- `35.07` Biodecision Compiler And Boundary Detection
- `35.13` Distributed Execution Scheduling And Fault Tolerance

## §04 Ingestion And Interop — 5 uncovered

- `04.01` Ingestion Pipeline
- `04.02` Opentelemetry Adapter
- `04.03` Runner And Benchmark Adapters
- `04.04` Environment And Artifact Capture
- `04.05` Redaction Privacy And Data Minimization

## §07 Evaluation Engine — 4 uncovered

- `07.02` Deterministic Property And Execution Evaluators
- `07.03` Trajectory And Decision Evaluators
- `07.09` Safety Privacy And Permission Metrics
- `07.13` Release Gates And Ci Policy

## §05 Execution Runtime — 3 uncovered

- `05.10` Distributed Execution Cache And Recovery
- `05.11` Shepherd And External Fork Runtime Integration
- `05.12` Runtime Conformance And Microbenchmarks

## §03 Core Specifications — 2 uncovered

- `03.03` World State Ir
- `03.11` Provenance Identifiers And Versioning

## §08 Adaptive Evaluation — 1 uncovered

- `08.06` Regression Focused Scheduling

## §13 Security Privacy And Safety — 1 uncovered

- `13.11` Effects Permissions And Human Approval
