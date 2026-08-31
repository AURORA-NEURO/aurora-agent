#![allow(clippy::all, ambiguous_glob_reexports)]

//! The §36 remainder: physical-action boundaries, dual-use release, representation, validation
//! evidence, and the human-subject determination.
//!
//! Implements blueprint 36.10 (physical experiment and wet-lab action boundaries), 36.11 (dual-use
//! biosecurity and capability release), 36.13 (fairness, representation and global resource
//! context), 36.21 (quality management, validation and release gates) and 36.22 (research ethics,
//! IRB and human-subject boundaries), on top of what `bioprism-policy`, `bioprism-safety`,
//! `bioprism-onco`, `bioprism-hub` and `bioprism-stewardship` already hold.
//!
//! # The classification, first
//!
//! Seven modules of section 36 were uncovered. The test applied was the workspace's: *is the
//! detailed design a set of predicates over an artifact, or a description of what people do?*, with
//! `bioprism-bioevalx`'s refinement that the split can run inside a module rather than between
//! modules. The answer here is a third shape again — neither §14's clean process/code partition nor
//! §26's uniform per-block split:
//!
//! | Class | Count | Modules |
//! |---|---|---|
//! | Predicates over an artifact — implemented here | 4 | 36.10, 36.11, 36.13, 36.22 |
//! | Split module — the predicate implemented, the metrics named | 1 | 36.21 |
//! | Perimeter infrastructure a sibling already positioned — deliberately **not** cited | 2 | *Sandboxing Untrusted Code and Research Artifacts*; *Security, Privacy, and Safety Red-Team Program* |
//!
//! Not one of the seven was a *pure* process module, which is the opposite of what an ethics
//! section leads you to expect and the opposite of `bioprism-stewardship`'s finding for §14, where
//! twelve of eighteen described councils and votes. §36's programme content is real — expert safety
//! review, community review, institutional review, independent teams — but it is distributed as
//! *one bullet per module*, never as a whole module. The section's process load shows up instead in
//! the four blocks every module shares, which are measured below.
//!
//! ## The two modules deliberately not cited
//!
//! **Sandboxing Untrusted Code and Research Artifacts** (§36, module seven). Its scope is container
//! or microVM isolation, read-only source mounts, network disabled by default, resource quotas, a
//! separate grader, signed dependencies and artifact scanning; its required controls are escape
//! tests, secret canaries, filesystem and network policy, supply-chain scans, malware and macro
//! controls, and quarantine. All thirteen need a process boundary, a network stack or a scanner.
//! `bioprism-safety` states this workspace's position on every one of them under 13.03, 13.04,
//! 13.15 and 13.22 — a library of plain Rust types may model such a control and may never claim one
//! — and `bioprism_sdk::sandbox` holds the isolation-request ladder with its single
//! `Enforcement::DeclaredOnly` variant. Implementing it here would produce a second threat model
//! with its own opinion about what isolation means.
//!
//! **Security, Privacy, and Safety Red-Team Program** (§36, module nineteen). An attack library, an
//! independent team, canary assets, a severity and remediation scale, regression packs and a public
//! disclosure policy. `bioprism-safety`'s `disclosure` module owns the red-team corpus and the
//! epoch-driven vulnerability ladder under 13.21 and 13.24, and its `integrity` module owns 13.14's
//! witnesses. The one clause that looks checkable — "canary assets" — is the same gap 13.14 already
//! records: nothing in the blueprint says what makes a canary detectable, and a canary type with an
//! invented detectability rule would be fiction with a struct around it.
//!
//! Neither module's dotted id appears anywhere in this crate. `tools/coverage.sh` matches any
//! `NN.MM` token under `crates/`, so citing them would move a coverage number without moving a
//! capability; `tests/hygiene.rs` fails if either id ever appears here, and builds the forbidden
//! strings by formatting rather than by writing them. Their required controls are nonetheless in
//! [`register::section_36_remainder`], keyed by title, so the position is visible rather than
//! silent.
//!
//! # How much of section 36 is boilerplate
//!
//! **Method.** Normalise each line by trimming whitespace, drop blank lines, and count a line as
//! shared when the identical line occurs in at least *k* of the section's module files. §36 has 22
//! module files; the section index is excluded. Mean 70.5 lines per module.
//!
//! | Unit | Shared | Share |
//! |---|---|---|
//! | Lines | 1210 / 1551 | **78.0%** |
//! | `##` blocks | 88 / 154 | 57.1% |
//! | Body bytes | 40524 / 49259 | 82.3% |
//!
//! ## Sensitivity
//!
//! **Threshold: no effect whatsoever.** *k* = 50%, 80% and 100% give 78.0% every time. Every shared
//! line in this corpus is shared by *all twenty-two* modules; there is no middle stratum. This is
//! the pure-skeleton regime `bioprism-bioevalx` found in §26 and §07, and it means the parameter
//! most often argued about is the one that matters least here.
//!
//! **Definition: the narrowest range yet measured in this workspace.** Stripping YAML front matter
//! and markdown headings moves the figure across 2.3 points only — 78.0% with both kept, 77.2%
//! front matter stripped, 76.8% headings stripped, 75.7% with both stripped. §26 moved 17.6 points
//! under the same perturbation and §43 moved 15.1. The reason is structural: §36's headings are
//! identical across all twenty-two files *and* so is most of the body, so removing the headings
//! removes shared lines and total lines in nearly the same proportion.
//!
//! **Unit: the dominant parameter, by a wide margin.** Lines say 78.0%, blocks say 57.1%, bytes say
//! 82.3% — a spread of 25.2 points, an order of magnitude more than definition and infinitely more
//! than threshold. The cause is worth stating because it is the finding: **four of §36's seven
//! blocks are byte-identical in every module, and they are the long ones.** "Policy evaluation
//! cells", "Enforcement architecture", "Evaluation metrics" and "Release gates" repeat verbatim
//! twenty-two times; "Purpose", "Scope" and "Required controls" are unique to each module and run
//! about 15.5 lines between them. Counting blocks weights a two-line "Purpose" equally with a
//! twenty-line shared diagram and so understates; counting bytes weights the diagram and so
//! overstates.
//!
//! So: **§36 is in the regime where unit dominates, definition barely registers and threshold does
//! not register at all** — the same regime §07 was in, arrived at by a different route. The
//! practical consequence for anyone implementing from this section: the distinguishing content is
//! roughly fifteen lines per module and every one of them is a noun phrase.
//!
//! # The honesty constraint, and how it is held
//!
//! This is a single-process library with no network, no filesystem access, no process spawning and
//! no clock. It can *model* a control and must never imply it *enforces* one. [`safeguard`] makes
//! that a type distinction rather than a convention:
//!
//! * [`safeguard::DeclaredSafeguard`] and [`safeguard::EnforcedSafeguard`] are different types.
//!   There is no `From`, no `promote`, no builder path and no `Deserialize` on the second.
//! * The only route between them is [`safeguard::DeclaredSafeguard::enforce`], which demands a
//!   [`safeguard::Impossibility`] — a named, checkable fact about what this crate's public API
//!   cannot express — and which refuses every perimeter control outright.
//! * Serializing an enforced safeguard and reading it back **fails**. Enforcement is a property of
//!   the compiled crate, not of a byte sequence, and a silent downgrade to declared would be a
//!   quieter lie in place of a loud one.
//! * [`safeguard::DeclaredSafeguard::rely`] always returns an error, so relying on a declaration is
//!   a `?` at the call site rather than a judgement in a reviewer's head. The shape is
//!   `bioprism_safety::threat::Threat::rely`'s.
//!
//! [`register::section_36_remainder`] is the shipped instance: the seven modules' required-control
//! lists, forty-two entries, **six enforced and thirty-six declared**. The two numbers are never
//! added and never turned into a percentage, because deciding what a declaration is worth would
//! make one of them wrong. Every one of the six defends a claim this crate makes about itself —
//! that it did not act on the world, refer an unassessed capability, attribute a finding to a
//! group, drop an unmeasured stratum, verify on absent evidence, or exempt a study. Not one defends
//! a perimeter, and a test fails if any perimeter entry is ever marked enforced.
//!
//! Dual-use control in particular claims nothing. [`dualuse`] calls
//! `bioprism_safety::release::ReleaseGate` and reports its verdict unchanged; it applies no rate
//! limit, restricts no pack, and reads no content.
//!
//! # No invented biology, no invented thresholds
//!
//! Every closed vocabulary here is a transcription of a blueprint list, and each says so where it
//! is defined: [`dualuse::MisuseSurface`] is 36.11's Scope, [`representation::ContextAxis`] is
//! 36.13's, [`validation::EvidenceKind`] is 36.21's, [`humansubject::EngagementKind`] is 36.22's,
//! and [`action::ActionKind`]'s six physical variants are 36.10's Scope while its three in-silico
//! variants are the three activities 36.10's purpose sentence permits. Where the blueprint supplies
//! no vocabulary, none is written: there is no capability tier, no risk scale, no severity ladder
//! and no maturity rung between experimental and verified.
//!
//! No numeric threshold appears in this crate. `tests/hygiene.rs` asserts that no recorded control
//! name and no vocabulary description contains an ASCII digit, and a companion test feeds the
//! scanner a deliberately bad value to prove it fires — `bioprism-oraclex`'s rule that a scanner
//! detecting nothing is worse than no scanner. Small-cell suppression, the one place §36 implies a
//! number, reads its threshold from a `bioprism_policy::SmallCellRule` the caller supplies.
//!
//! Three placements *are* this crate's and are labelled where they are made:
//! [`safeguard::ControlSurface`]'s perimeter/claim split over §36's control lists;
//! [`representation::ContextAxis::is_resource_context`]'s split of 36.13's six axes into three that
//! describe access to measurement and three that do not; and the rule that structural non-identity
//! is the only independence criterion [`validation`] can check.
//!
//! # What the neighbours own, and is not reimplemented here
//!
//! - **`bioprism-onco`** owns the typed research boundary. [`action::ActionPlan::partition`] and
//!   [`humansubject::StudyDescription::check_return_of_results`] *ask* it and carry its refusal
//!   through [`BioethicsError::Onco`]. This crate has no clinical-use vocabulary and no override.
//! - **`bioprism-safety`** owns section 13's threat model, the dual-use release gate, the
//!   withholding rule, the red-team corpus and the disclosure ladder. [`dualuse`] calls the gate;
//!   there is no second gate, no second threat model, no asset list, no adversary and no attack
//!   class anywhere in this crate.
//! - **`bioprism-policy`** owns classification labels, the closed `Purpose` enumeration, consent,
//!   residency, redaction and small-cell suppression, under 36.01, 36.04, 36.06 and 36.18.
//!   [`humansubject`] calls `Consent::check` and [`representation`] calls `SmallCellRule::release`.
//!   There is no second consent model and no second purpose list.
//! - **`bioprism-hub`** owns 36.12's claim boundaries and labelling, 36.14's licence inheritance,
//!   36.16's conflicts and sponsorship, and 36.17's append-only event log. Nothing here publishes,
//!   moderates or ranks.
//! - **`bioprism-stewardship`** owns the medical-boundary governance: a refusal is terminal, the
//!   transition dossier has no waive, and no score, claim or tier is an input to a clinical
//!   transition certificate. [`validation`] governs an *implementation module*'s validation file,
//!   which is a different object from a claim, a pack, a score or a schema.
//! - **`bioprism-metrics`** owns aggregation, the worst-measured cell, intervals and the rule that
//!   an aggregate over a grid containing an unmeasured cell is not an aggregate over the grid.
//!   [`representation`] computes no statistic of any kind.
//! - **`bioprism-sdk`** owns the isolation-request ladder and `Enforcement::DeclaredOnly`.
//! - **`bioprism-governance`** owns versioning, compatibility and deprecation.
//!
//! # What this crate deliberately does not implement
//!
//! This list is the most important documentation in the crate. A missing capability that is stated
//! is a limitation; one that is implied to exist is a lie.
//!
//! **Execution, of anything.** No process spawning, no container, no microVM, no WebAssembly, no
//! sandbox, no resource quota, no filesystem access, no network, no device driver, no instrument
//! interface, no laboratory API client, no scheduler and no queue. [`action`] refers; nothing here
//! executes, and no type in this crate represents anything having been executed.
//!
//! **Detection and classification of content.** No scanner, no malware check, no macro check, no
//! sequence screening, no injection detector, no classifier, no heuristic and no pattern list.
//! Every label in this crate — a misuse surface, a stratum, an engagement, an action kind — is
//! applied by a caller and recorded, never inferred. Nothing here reads a sequence, a document, a
//! prompt or a model output.
//!
//! **Statistics.** No calibration, no interval, no disparity measure, no rate, no denominator, no
//! aggregation, no ranking, no significance test and no correction. §36's evaluation-metrics block
//! names nine metrics and defines none of them; this crate computes none of them.
//!
//! **Cryptography and identity.** No hashing, no signing, no verification, no keys, no
//! authentication and no authorization. Every name in this crate — an approver, a safety-review
//! body, an assessor, an author, a reproducer, an institutional body — is an opaque string that
//! nothing authenticates. An [`action::Authorisation`] and an
//! [`humansubject::InstitutionalDetermination`] are transcriptions of events outside this process.
//!
//! **Storage, transport and audit.** No database, no artifact store, no registry, no event log, no
//! append-only ledger, no export and no retention policy. §36 requires a signed audit event at the
//! end of its enforcement architecture; this crate emits nothing. `bioprism-hub` and
//! `bioprism-ledger` own that ground.
//!
//! **Time and randomness.** No clock and no RNG. Nothing expires, nothing is scheduled, nothing is
//! sampled, and every function here is deterministic. 36.22's "ongoing review" and 36.10's
//! authorisation have no validity period for exactly this reason.
//!
//! **Process.** No review board, no committee, no quorum, no vote, no reviewer assignment, no
//! notification, no escalation channel, no appeal, no bounty, no remediation workflow and no
//! disclosure timeline. Where §36 describes people doing things, this crate holds the record they
//! would fill in and reports which fields are empty.
//!
//! **The trusted kernel.** §36's shared enforcement architecture runs request → identity, purpose
//! and task-risk classification → policy intersection → short-lived capability grant → sandbox,
//! federated worker or human approval → continuous information-flow and effect monitor → export
//! filter and signed audit event. `bioprism-policy` is the policy-intersection step and says so.
//! This crate contributes the task-risk classification's *record* and the human-approval step's
//! *record*. The capability grant, the monitor, the export filter and the kernel that is supposed
//! to contain all of them do not exist in this workspace.
//!
//! # Every place section 36 names a control it never specifies
//!
//! Counted over the seven modules in scope plus the four blocks all twenty-two share. Each is a
//! place where a type here would have been fiction.
//!
//! **In the shared blocks, so in all twenty-two modules at once:**
//!
//! * *"task-risk classification"* — a step in the enforcement diagram. No classes, no scale, no
//!   classifier. [`dualuse::SurfaceAssessment`] records what a human said and attaches no score.
//! * *"short-lived capability grant"* — no lifetime, no authority vocabulary, no revocation. Not
//!   modelled; nothing here grants anything.
//! * *"policy intersection: user × data × tool × site × pack × intended output"* — six factors and
//!   no intersection rule. `bioprism-policy` implements the lattice; the six-way product is not
//!   specified anywhere.
//! * *"continuous information-flow and effect monitor"* — no definition of an effect, no sampling,
//!   no trigger.
//! * The nine evaluation metrics — *policy violation and near-miss rate* (near-miss undefined),
//!   *appropriate refusal and escalation precision/recall* (no ground-truth set), *retained utility
//!   on safe tasks* (no utility function), *unauthorized data-flow events*, *identifier and
//!   small-cell exposure*, *grant minimization* (no minimality criterion), *incident detection and
//!   containment*, *false-positive policy burden* (no denominator), *reproducibility of
//!   enforcement*. Nine names, zero denominators.
//! * Release gate seven — *"an independent reviewer verifies the policy implementation"* — with no
//!   independence criterion. `bioprism-policy`'s error module already records this gap.
//!
//! **In the five modules implemented here:**
//!
//! * **36.10** — *"restricted capability tiers"*, never enumerated; *"public MVP"*, never defined;
//!   *"human approval"* and *"institutional safety review"* with no role, criterion, quorum or
//!   expiry; *"sandbox simulation"* and *"full audit"* with no runtime and no schema.
//! * **36.11** — *"plausible misuse"* with no scale or horizon; *"rate and tool limits"* with no
//!   rate; *"restricted packs"* with no mechanism; *"expert safety review"* with no expertise
//!   criterion; *"capability withholding"* with no criterion, where 13.26 does supply one. And the
//!   structural gap: 36.11's six misuse surfaces and 13.26's six sensitive categories are never
//!   related to each other, so [`dualuse::refer`] requires the caller to state both.
//! * **36.13** — *"worst-group calibration"* with no calibration metric and no definition of
//!   "group"; *"coverage and abstention"* with no floor and no abstention rule;
//!   *"resource-constrained architecture"* with no resource model; *"small-group protection"* with
//!   no threshold in this module or anywhere in §36; *"community review"* with no composition or
//!   cadence; *"without essentializing groups"* with no operational test — the sentence this crate
//!   turned into a missing enum variant. Its purpose sentence also names *languages*, which appears
//!   in no scope bullet, so there is no language axis here.
//! * **36.21** — the module promises to *"define evidence required"* and defines none; its
//!   *"Required controls"* block is six measurements rather than six controls — *gate pass rate*,
//!   *open risk debt*, *validation coverage*, *post-release defect*, *reproducibility*, *audit
//!   findings* — with no denominator on any of them, and it is the only module in §36 that
//!   substitutes metrics for controls; *"independent reproduction"* has no independence criterion;
//!   the ladder from *experimental* to *verified* names no rung between them.
//! * **36.22** — *"institutional determination"* with no determiner, basis, form or validity
//!   period; *"consent or waiver"* with no waiver criterion; *"minimal-risk assessment"* with no
//!   risk scale; *"data-security plan"* with no required contents; *"publication and
//!   return-of-results policy"* with no policy; *"ongoing review"* with no period.
//!
//! # Boundary
//!
//! Research and developer infrastructure. This crate does not diagnose an individual, recommend
//! treatment, triage care, enroll participants, execute a physical action, or claim medical-device
//! functionality. `bioprism-onco` carries the typed research boundary and is consulted rather than
//! routed around; `bioprism-stewardship` carries the medical-boundary governance and nothing here
//! is an input to a clinical transition certificate.

pub mod action;
pub mod boundary_integrity_support;
pub mod contract_frontier_assurance;
pub mod dependency_composition_assurance;
pub mod dualuse;
pub mod error;
pub mod evidence_surveillance_assurance;
pub mod experiment_design_workflow_fabric;
pub mod federated_continual_boundary_integrity_contract_model;
pub mod federated_continual_boundary_integrity_inference;
pub mod federated_continual_boundary_integrity_research_copilot;
pub mod federated_continual_boundary_integrity_workflow_fabric;
pub mod federated_continual_evidence_surveillance_contract_model;
pub mod humansubject;
pub mod local_boundary_integrity_contract_model;
pub mod local_boundary_integrity_inference;
pub mod local_boundary_integrity_research_copilot;
pub mod local_boundary_integrity_workflow_fabric;
pub mod multimodal_boundary_integrity_contract_model;
pub mod multimodal_boundary_integrity_inference;
pub mod multimodal_boundary_integrity_research_copilot;
pub mod multimodal_boundary_integrity_workflow_fabric;
pub mod multimodal_bounded_evolution_assurance;
pub mod multimodal_context_compilation_assurance;
pub mod prospective_computational_execution_assurance;
pub mod register;
pub mod representation;
pub mod safeguard;
pub mod scale_frontier_contract;
pub mod statistical_analysis_assurance;
pub mod throughput_boundary_integrity_contract_model;
pub mod throughput_boundary_integrity_inference;
pub mod throughput_boundary_integrity_research_copilot;
pub mod throughput_boundary_integrity_workflow_fabric;
pub mod validation;

pub use experiment_design_workflow_fabric::{
    compile_experiment_design_workflow, compile_experiment_design_workflow_json,
    experiment_design_workflow_fabric_manifest, validate_experiment_design_workflow_json,
    ExecutableExperimentDesign4, ExperimentDesignWorkflowError, ExperimentDesignWorkflowRequest1,
    ExperimentObjective1, WorkflowStep1,
    CONTRACT_VERSION as EXPERIMENT_DESIGN_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as EXPERIMENT_DESIGN_WORKFLOW_FEATURE_ID,
    INPUT_SCHEMA as EXPERIMENT_DESIGN_WORKFLOW_INPUT_SCHEMA,
    OUTPUT_SCHEMA as EXPERIMENT_DESIGN_WORKFLOW_OUTPUT_SCHEMA,
};
pub use multimodal_bounded_evolution_assurance::{
    assure_multimodal_bounded_evolution, assure_multimodal_bounded_evolution_json,
    multimodal_bounded_evolution_assurance_manifest, validate_bioethics_evolution_json,
    BioethicsEvolutionCandidate2, BioethicsEvolutionDecision7, BioethicsEvolutionError,
    BioethicsEvolutionRequest3, CONTRACT_VERSION as MULTIMODAL_BOUNDED_EVOLUTION_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_BOUNDED_EVOLUTION_FEATURE_ID,
    INPUT_SCHEMA as MULTIMODAL_BOUNDED_EVOLUTION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as MULTIMODAL_BOUNDED_EVOLUTION_OUTPUT_SCHEMA,
};
pub use multimodal_context_compilation_assurance::{
    assure_multimodal_context_compilation, multimodal_context_compilation_assurance_manifest,
    CertifiedDecisionSection7, CertifiedDecisionSectionArtifact7, ContextCompilationAssuranceError,
    ContextFact4, DecisionQuery2,
    CONTRACT_VERSION as MULTIMODAL_CONTEXT_COMPILATION_CONTRACT_VERSION,
    FEATURE_ID as MULTIMODAL_CONTEXT_COMPILATION_FEATURE_ID,
    INPUT_SCHEMA as MULTIMODAL_CONTEXT_COMPILATION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as MULTIMODAL_CONTEXT_COMPILATION_OUTPUT_SCHEMA,
};
pub use prospective_computational_execution_assurance::{
    capability_manifest as prospective_computational_execution_assurance_manifest,
    verify as assure_prospective_computational_execution,
    verify_json as assure_prospective_computational_execution_json, ExecutionAssuranceError,
    ExecutionNode, ExecutionRun, ResearchWorkflowSpec,
    CONTRACT_VERSION as PROSPECTIVE_COMPUTATIONAL_EXECUTION_CONTRACT_VERSION,
    FEATURE_ID as PROSPECTIVE_COMPUTATIONAL_EXECUTION_FEATURE_ID,
    INPUT_SCHEMA as PROSPECTIVE_COMPUTATIONAL_EXECUTION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as PROSPECTIVE_COMPUTATIONAL_EXECUTION_OUTPUT_SCHEMA,
};
pub use statistical_analysis_assurance::{
    assure_statistical_analysis, statistical_analysis_assurance_manifest, AnalysisCandidate4,
    AnalysisQuestion3, QualifiedAnalysisArtifact7, QualifiedAnalysisResult7,
    StatisticalAnalysisAssuranceError,
    CONTRACT_VERSION as STATISTICAL_ANALYSIS_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as STATISTICAL_ANALYSIS_ASSURANCE_FEATURE_ID,
    INPUT_SCHEMA as STATISTICAL_ANALYSIS_ASSURANCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as STATISTICAL_ANALYSIS_ASSURANCE_OUTPUT_SCHEMA,
};

pub use scale_frontier_contract::{
    capability_manifest as scale_frontier_contract_manifest, evaluate_capacity,
    BioethicsCapacityReport2, BioethicsScaleFrontierError, BioethicsScaleFrontierRequest,
    BioethicsScaleWorkload4, CONTENT_TYPE as BIOETHICS_CAPACITY_CONTENT_TYPE,
    CONTRACT_VERSION as BIOETHICS_CAPACITY_CONTRACT_VERSION,
    FEATURE_ID as BIOETHICS_CAPACITY_FEATURE_ID, INPUT_SCHEMA as BIOETHICS_CAPACITY_INPUT_SCHEMA,
    OUTPUT_SCHEMA as BIOETHICS_CAPACITY_OUTPUT_SCHEMA,
};

pub use boundary_integrity_support::*;
pub use error::BioethicsError;
pub use evidence_surveillance_assurance::{
    assure_evidence_surveillance, evidence_surveillance_assurance_manifest,
    BioethicsEvidenceDisposition, BioethicsEvidenceReceipt, BioethicsEvidenceRequest,
    EvidenceSurveillanceAssuranceError,
    CONTRACT_VERSION as EVIDENCE_SURVEILLANCE_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_SURVEILLANCE_ASSURANCE_FEATURE_ID,
};
pub use federated_continual_boundary_integrity_contract_model::*;
pub use federated_continual_boundary_integrity_inference::*;
pub use federated_continual_boundary_integrity_research_copilot::*;
pub use federated_continual_boundary_integrity_workflow_fabric::*;
pub use federated_continual_evidence_surveillance_contract_model::{
    federated_continual_evidence_surveillance_contract_model_manifest,
    model_federated_continual_evidence_surveillance_contract, FederatedContinualContractClaim,
    FederatedContinualContractCompatibility, FederatedContinualContractDisposition,
    FederatedContinualEvidenceSurveillanceContractError,
    FederatedContinualEvidenceSurveillanceContractReceipt,
    FederatedContinualEvidenceSurveillanceContractRequest,
    CONTRACT_VERSION as BIOETHICS_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as BIOETHICS_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_FEATURE_ID,
    INPUT_SCHEMA as BIOETHICS_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_INPUT_SCHEMA,
    OUTPUT_SCHEMA as BIOETHICS_FEDERATED_CONTINUAL_EVIDENCE_SURVEILLANCE_CONTRACT_MODEL_OUTPUT_SCHEMA,
};
pub use local_boundary_integrity_contract_model::*;
pub use local_boundary_integrity_inference::*;
pub use local_boundary_integrity_research_copilot::*;
pub use local_boundary_integrity_workflow_fabric::*;
pub use multimodal_boundary_integrity_contract_model::*;
pub use multimodal_boundary_integrity_inference::*;
pub use multimodal_boundary_integrity_research_copilot::*;
pub use multimodal_boundary_integrity_workflow_fabric::*;
pub use register::{section_36_remainder, RegisterCounts, SafeguardRegister};
pub use safeguard::{
    BlueprintModule, ControlSurface, DeclaredSafeguard, EnforcedSafeguard, Impossibility,
    Safeguard, SafeguardDocument,
};
pub use throughput_boundary_integrity_contract_model::*;
pub use throughput_boundary_integrity_inference::*;
pub use throughput_boundary_integrity_research_copilot::*;
pub use throughput_boundary_integrity_workflow_fabric::*;
