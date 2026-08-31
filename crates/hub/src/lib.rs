//! BioAtlas public hub: the submission and ecosystem contract layer.
//!
//! Implements blueprint section 34 (BioAtlas Public Hub and Ecosystem) — principally 34.13
//! (Benchmark Pack Authoring Studio), 34.14 (Result Cards, Signed Bundles and Reproduction), 34.15
//! (Leaderboards, Pareto Fronts and Claim Policy), 34.16 (Challenges, Submissions and Hidden
//! Holdouts) and 34.18 (Expert Review Network and Credit) — under the constraints of section 36
//! (security, privacy, ethics and governance) and the claim discipline of 43.43.
//!
//! # What this crate is for
//!
//! A public hub is the part of an evaluation system most likely to produce a dishonest artifact,
//! because it is the part with an audience. The dishonesty is rarely a fabricated number. It is a
//! true number shown next to another true number that was produced under different conditions; an
//! instance count presented where an independent-class count belongs; a score on a benchmark that
//! leaked last spring; a derived pack whose research-only ancestor quietly became permissive; an
//! entry that stopped existing when it became inconvenient.
//!
//! Each module here is aimed at one of those:
//!
//! | Module | Refuses |
//! |---|---|
//! | [`submission`] | a submission that does not carry a digest, scope, licence, provenance, and a statement of what it does not establish |
//! | [`moderation`] | silent deletion, backdated audit events, unreasoned rejection, self-review |
//! | [`leaderboard`] | ordering two entries evaluated under different conditions |
//! | [`disclosure`] | a headline score on a contaminated pack, and "unreported" read as "held out" |
//! | [`attribution`] | a derived artifact that drops its ancestors' licences or credit |
//! | [`card`] | a non-publishable state rendered as a zero or a blank |
//!
//! # Boundary with `bioprism-registry`
//!
//! `bioprism-registry` owns benchmark packs, trust tiers and promotion: what an artifact *is* and
//! what its evidence earns it. This crate owns the layer above — who may submit one, what a
//! submission must carry, how it is moderated, and what the hub is permitted to say about it. The
//! two do not depend on each other, and the split is deliberate: tier evaluation is a pure function
//! of pack bytes, whereas everything here is a function of assertions made by people. Keeping the
//! second kind out of the first is what makes the first replayable.
//!
//! # What is not implemented, and will not be
//!
//! - **No network.** No HTTP, no API server, no event stream. 34.23 describes those; this is the
//!   contract they would enforce.
//! - **No identity provider.** [`id::SubmitterId`] is an opaque string. Deciding that a request
//!   really came from that party is the deployment's job; deciding what that party may then do is
//!   this crate's.
//! - **No storage.** [`moderation::ModerationLedger`] and [`disclosure::DisclosureLedger`] are
//!   in-memory values with serde derives. Durability, signing and replication are elsewhere.
//! - **No web UI.** [`card::BioAtlasCard`] is the object a renderer consumes, not a rendering.
//! - **No score computation, leak detection, or licence-text parsing.** Those results are inputs.
//!
//! # Earned, not asserted
//!
//! The recurring shape, shared with `bioprism_registry::tier`: nothing that confers standing can be
//! set by the party that benefits from it. A submitter cannot declare their result
//! [`submission::VerificationStatus::Verified`]; a publisher cannot declare a card `available` while
//! its submission is disputed; a derived pack cannot declare a licence looser than its ancestry
//! permits; a leaderboard cannot declare two entries comparable by putting them in the same table.

pub mod attribution;
pub mod card;
pub mod disclosure;
pub mod error;
pub mod id;
pub mod leaderboard;
pub mod moderation;
pub mod submission;
pub mod policy_autonomy_inference_engine;
pub mod submission_release_integrity_support;
pub mod local_submission_release_integrity_inference;
pub mod multimodal_submission_release_integrity_inference;
pub mod throughput_submission_release_integrity_inference;
pub mod federated_continual_submission_release_integrity_inference;
pub mod local_submission_release_integrity_contract_model;
pub mod multimodal_submission_release_integrity_contract_model;
pub mod throughput_submission_release_integrity_contract_model;
pub mod federated_continual_submission_release_integrity_contract_model;
pub mod local_submission_release_integrity_research_copilot;
pub mod multimodal_submission_release_integrity_research_copilot;
pub mod throughput_submission_release_integrity_research_copilot;
pub mod federated_continual_submission_release_integrity_research_copilot;
pub mod local_submission_release_integrity_workflow_fabric;
pub mod multimodal_submission_release_integrity_workflow_fabric;
pub mod throughput_submission_release_integrity_workflow_fabric;
pub mod federated_continual_submission_release_integrity_workflow_fabric;

#[cfg(test)]
mod fixtures;

pub use attribution::{AccessTier, Ancestor, Attribution, Licence, LicenceStack, Redistribution};
pub use card::{BioAtlasCard, PublicationState, ScoreDisplay, RESOURCE_TYPE};
pub use disclosure::{
    ContaminationKind, ContaminationWitness, DisclosureLedger, DisclosureState, HeadlineLabel,
};
pub use error::HubError;
pub use id::{BoardId, Epoch, SubmissionId, SubmitterId};
pub use leaderboard::{
    lint_claim, partition, rank_order, Board, BudgetEnvelope, Comparability,
    ComparabilityConditions, ConditionDifference, Entry, Interval, RankedBoard, RankedEntry, Score,
    UnrankableReason, UnrankedEntry,
};
pub use moderation::{
    Decision, EventKind, ModerationEvent, ModerationLedger, ModerationRecord, ModerationState,
    Tombstone,
};
pub use submission::{
    accept, BuildProvenance, DeclaredScope, EvidenceScale, NonClaim, NonClaimKind, Provenance,
    Standing, Submission, SubmissionDraft, Submitter, VerificationStatus,
};
pub use policy_autonomy_inference_engine::{
    infer_policy_receipt, infer_policy_receipt_json,
    policy_autonomy_inference_engine_manifest, validate_policy_receipt_json,
    ActionAndAuthority3, PolicyAutonomyInferenceError, PolicyInferenceRequest3, PolicyReceipt1,
    CONTRACT_VERSION as POLICY_AUTONOMY_INFERENCE_CONTRACT_VERSION,
    FEATURE_ID as POLICY_AUTONOMY_INFERENCE_FEATURE_ID,
    INPUT_SCHEMA as POLICY_AUTONOMY_INFERENCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as POLICY_AUTONOMY_INFERENCE_OUTPUT_SCHEMA,
};
pub use submission_release_integrity_support::{
    release as release_submission_release_integrity,
    manifest as submission_release_integrity_manifest,
    SubmissionArtifact4, SubmissionCandidate4, SubmissionReleaseCard7,
    SubmissionReleaseIntegrityError, SubmissionReleaseRequest4,
};
