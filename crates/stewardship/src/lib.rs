//! Governance and quality: the five checkable modules of section 14.
//!
//! Implements blueprint 14.09 (Oracle and Evaluator Review), 14.10 (Metric, Score and Statistical
//! Governance), 14.11 (Result Claims and Leaderboard Governance), 14.14 (Medical and Neuroscience
//! Boundary) and 14.15 (Data Governance, Federation and Access), plus the one sentence of 14.08
//! that is a rule rather than a workflow.
//!
//! # Read this first: most of section 14 is not code, and this crate does not pretend otherwise
//!
//! Section 14 has 25 modules. Seven were already cited by `bioprism-governance`; eighteen were not,
//! and the first job here was to read all eighteen and decide which of them specify behaviour.
//! Twelve do not. They describe councils, votes, RFC stages, maintainer promotion, recusal,
//! sponsorship disclosure, conduct reporting, budget publication and periodic review cadence — real
//! obligations of a real project, and all of them statements about people rather than about
//! artifacts. A `Council` type with a `vote` method would assert that a council met, which is
//! precisely what software cannot know.
//!
//! `docs/COVERAGE.md` already excludes ten whole sections from its denominator for this reason, and
//! records that a citation means *"someone has read this and taken a position"*. "This is a process
//! document, not code" is such a position — but writing the twelve ids into this file would move a
//! coverage number without moving any capability, so their ids are deliberately **not** written
//! here in the dotted form the coverage script matches. The modules are named instead: project
//! governance; open governance and the RFC process; roles, ownership and the maintainer model; the
//! contributor model and code ownership; RFC/ADR and technical decisions; benchmark governance and
//! stewardship; benchmark ethics, fairness and conflicts; conflicts of interest and sponsorship;
//! documentation information architecture and review; community conduct, inclusion and appeals;
//! sustainability, finance and public benefit; and periodic programme review.
//!
//! Two of the twelve have a code-bearing clause each that another crate already discharges, so
//! reimplementing it here would have been the second-worst outcome after ignoring it. Benchmark
//! governance says "repeated queries reduce holdout status": that is `bioprism_lab::Holdout`'s
//! exposure ledger, exactly. Benchmark ethics says authors do not solely certify their own systems:
//! that is the separation-of-duties check in [`review`], and it also lives in `bioprism-registry`'s
//! reviewer-independence rule. Neither is re-implemented; both are named where they belong.
//!
//! # What the six code-bearing modules turn into
//!
//! | Module | This crate | Refuses |
//! |---|---|---|
//! | 14.08 | [`review`] | an approval read as universal endorsement rather than as named dimensions |
//! | 14.09 | [`review`] | an approval that reads as broader than the dimensions actually examined |
//! | 14.10 | [`predeclaration`] | a confirmatory finding on a metric named after the results were seen |
//! | 14.11 | [`claim`] | a sentence that outruns the evidence licensing it |
//! | 14.14 | [`boundary`] | a medical pack with no domain review, and a clinical claim with no transition |
//! | 14.15 | [`access`] | an access grant that exceeds its contract, and one that never expires |
//!
//! 14.08 is the borderline call, and it is called this way rather than left out: its review packet,
//! reviewer assignment, sampling design and signed record are workflow, and `bioprism-registry`'s
//! gate plus `bioprism-hub`'s submission pipeline already discharge most of the checkable part. One
//! sentence is neither — *"Approval names dimensions and expiry/review date; it is not universal
//! endorsement"* — and [`review::DimensionScopedApproval`] is that sentence, so the module is cited
//! for it and for nothing else.
//!
//! # What the neighbours already own, and are not re-implemented here
//!
//! Every one of these was read before writing, and the boundary is stated in the module that
//! touches it.
//!
//! - **`bioprism-governance`** owns schema versioning, compatibility classification and the
//!   deprecation lifecycle, including the rule that a change moving an artifact's digest cannot be
//!   classified compatible. [`review`] *uses* it — a scoring-semantic change is exactly such a
//!   change — rather than restating it.
//! - **`bioprism-registry`** owns trust tiers computed from pack bytes. [`claim`] reads a
//!   `TrustTier` as an eligibility input and has no way to set one.
//! - **`bioprism-atlas`** owns the claim ladder of 43.40: which tier the *evidence* licenses.
//!   [`claim`] is the layer above — which *sentence* that tier permits — and treats
//!   `ClaimTier` as a ceiling it cannot raise.
//! - **`bioprism-hub`** owns submissions, moderation, leaderboards and expert review. There is no
//!   second leaderboard here: [`claim::ClaimRegister`] governs the lifecycle of a published
//!   *claim*, and its only ordering question is whether a claim may be promoted to a highlight.
//! - **`bioprism-metrics`** owns aggregation, comparability, `Incomparable`, release gates and
//!   *predeclared weights*. [`predeclaration`] deliberately holds no weighting: it predeclares
//!   which metric is primary, not how a composite is combined.
//! - **`bioprism-lab`** owns holdout exposure and the type that carries a score.
//!   [`predeclaration::ConfirmatoryFinding`] is its sibling for the other half of the same problem:
//!   `lab` refuses a score on a holdout the configuration was selected with, this refuses a
//!   confirmatory reading of a metric the analyst chose after seeing the number.
//! - **`bioprism-benchcompiler`** owns the oracle gate — `ProposedOracle` has no `grade`.
//!   [`review`] is the record of the review that passed through that gate.
//! - **`bioprism-onco`** owns the typed research boundary and its request-time triage.
//!   [`boundary`] does not triage anything; it governs pack admissibility, the human disposition of
//!   a raised crossing, and the transition gate to clinical use.
//! - **`bioprism-policy`** owns information-flow labels, the closed `Purpose` enumeration, consent
//!   and residency. [`access`] keeps purposes as opaque tags on purpose, so the workspace has one
//!   closed purpose enumeration rather than two that drift.
//! - **`bioprism-hubapi`** owns federation and the rule that trust does not transit.
//!   [`access::SiteAttestation`] confers no standing on anybody; it decides which fields a central
//!   analysis may receive.
//!
//! # What is deliberately not implemented
//!
//! - **No clock.** Every lifecycle advances on an operator-supplied [`id::Epoch`], as
//!   `bioprism-governance` and `bioprism-ledger` already do. A grant expires because an epoch
//!   passed its ceiling. 14.08's "expiry/review date" therefore becomes structural staleness — an
//!   approval stops applying when the scoring digest moves — rather than a date nobody can check.
//! - **No identity system.** [`id::ActorId`] is an opaque string. Nothing authenticates it. The
//!   only questions asked about a name are structural: is it the same as that other name, and does
//!   its declared role permit it to approve alone.
//! - **No voting, no councils, no quorum, no appeals workflow.** See the first section.
//! - **No review workflow.** Nothing assigns a reviewer, tracks a queue, or notifies anybody.
//! - **No signatures.** A "signed" attestation here is a record that names five digests; whether
//!   the named site produced it is the deployment's problem and `bioprism-hubapi`'s subject.
//! - **No text analysis.** [`claim::ClaimSentence`] is a structured object, not prose to be
//!   scanned for forbidden words. `bioprism_hub::lint_claim` does the phrase check; a governance
//!   layer that depended on regexes over English would be defeated by a synonym.
//! - **No statistics.** [`predeclaration`] declares which metric is primary and which trials may be
//!   excluded. It computes no estimate, no interval and no multiple-comparison correction.
//!
//! # Where section 14 names a governance mechanism it never specifies
//!
//! Recorded here rather than invented: "impact analysis" (named in three modules, never defined);
//! "supermajority" and "public notice" with no threshold or notice period; "independent reviewer"
//! with no independence criterion beyond not being the author; "council review" with no composition;
//! "bridge studies across releases" with no design; "statistically/structurally designed human
//! samples" with no sampling frame; "expiry/review date" with no interval; "periodic review" of
//! access grants with no period; "proportional enforcement" with no scale. Each is a place where a
//! type here would have been fiction. Where this crate had to choose a vocabulary anyway —
//! [`review::ReviewDimension`], [`claim::ClaimClass`], [`boundary::ClinicalObligation`] — the choice
//! is derived from a list the module does give, and the derivation is stated at the definition.

pub mod access;
pub mod boundary;
pub mod claim;
pub mod error;
pub mod id;
pub mod predeclaration;
pub mod release_workbench;
pub mod review;
pub mod snapshot_integrity_support;
pub mod local_snapshot_integrity_inference;
pub mod multimodal_snapshot_integrity_inference;
pub mod throughput_snapshot_integrity_inference;
pub mod federated_continual_snapshot_integrity_inference;
pub mod local_snapshot_integrity_contract_model;
pub mod multimodal_snapshot_integrity_contract_model;
pub mod throughput_snapshot_integrity_contract_model;
pub mod federated_continual_snapshot_integrity_contract_model;
pub mod local_snapshot_integrity_research_copilot;
pub mod multimodal_snapshot_integrity_research_copilot;
pub mod throughput_snapshot_integrity_research_copilot;
pub mod federated_continual_snapshot_integrity_research_copilot;
pub mod local_snapshot_integrity_workflow_fabric;
pub mod multimodal_snapshot_integrity_workflow_fabric;
pub mod throughput_snapshot_integrity_workflow_fabric;
pub mod federated_continual_snapshot_integrity_workflow_fabric;

pub use access::{
    AccessEvent, AccessGrant, AccessLedger, ContractId, DataContract, DerivativeRights, GrantId,
    ModelRestriction, ProcessingPath, PublicationRight, PurposeTag, Retention, SiteAttestation,
    SiteAttestationDraft, WithdrawalAssessment, WithdrawalDisposition,
};
pub use boundary::{
    BoundaryLedger, ClinicalObligation, ClinicalTransitionCertificate, CrossingDisposition,
    CrossingTrigger, DomainPackCard, DomainReview, EscalationDocket, ExcludedUse,
    PermittedResearchScope, ResearchStanding, TransitionDossier,
};
pub use claim::{
    ClaimClass, ClaimEvent, ClaimId, ClaimRegister, ClaimSentence, ClaimStatus, ClaimSubject,
    ComparisonAssertion, EvidenceRequirements, IssuedClaim, PromotionBasis, ReproductionRecord,
    Tombstone,
};
pub use error::{
    AccessError, BoundaryError, ClaimError, PredeclarationError, ReviewError, StewardshipError,
};
pub use id::{Actor, ActorId, Epoch, Role};
pub use predeclaration::{
    AnalysisPlan, ClusteringUnit, ConfirmatoryFinding, ExclusionRule, ExploratoryFinding,
    MetricDefinition, MetricDirection, MetricRegistry, ObservedResults, SealedPlan,
    TrialDisposition,
};
pub use release_workbench::{
    prepare_release_workbench, release_workbench_manifest, ReleaseObjectCandidate,
    ReleaseObjectState, ReleaseWorkbenchDisposition, ReleaseWorkbenchError,
    ReleaseWorkbenchReceipt, ReleaseWorkbenchRequest,
    CONTRACT_VERSION as RELEASE_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as RELEASE_WORKBENCH_FEATURE_ID,
};
pub use review::{
    full_corpus, CorpusClass, DimensionScopedApproval, EvaluatorRevision, Finding, ReviewDimension,
    ReviewRecord, TestCorpus,
};
pub use snapshot_integrity_support::{qualify as qualify_snapshot_integrity, manifest as snapshot_integrity_manifest, StewardshipItem4, SnapshotIntegrityArtifact4, SnapshotIntegrityCard7, SnapshotIntegrityError, SnapshotIntegrityRequest4, BOUNDARY as SNAPSHOT_INTEGRITY_BOUNDARY, CONTENT_TYPE as SNAPSHOT_INTEGRITY_CONTENT_TYPE};
pub use local_snapshot_integrity_inference::*;
pub use multimodal_snapshot_integrity_inference::*;
pub use throughput_snapshot_integrity_inference::*;
pub use federated_continual_snapshot_integrity_inference::*;
pub use local_snapshot_integrity_contract_model::*;
pub use multimodal_snapshot_integrity_contract_model::*;
pub use throughput_snapshot_integrity_contract_model::*;
pub use federated_continual_snapshot_integrity_contract_model::*;
pub use local_snapshot_integrity_research_copilot::*;
pub use multimodal_snapshot_integrity_research_copilot::*;
pub use throughput_snapshot_integrity_research_copilot::*;
pub use federated_continual_snapshot_integrity_research_copilot::*;
pub use local_snapshot_integrity_workflow_fabric::*;
pub use multimodal_snapshot_integrity_workflow_fabric::*;
pub use throughput_snapshot_integrity_workflow_fabric::*;
pub use federated_continual_snapshot_integrity_workflow_fabric::*;
