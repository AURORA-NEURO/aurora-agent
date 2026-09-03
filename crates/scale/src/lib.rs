//! The content half of the million-scale benchmark factory.
//!
//! Implements blueprint section 35 — 35.01 portfolio design, 35.05 prospective escrow and blind
//! reveal, 35.09 procedural generation and mutation scheduling, 35.10 deduplication and effective
//! size, 35.11 contamination and hidden-family splits, 35.12 content-addressed storage with delta
//! snapshots and a replay cache, 35.14 adaptive selection and stopping, 35.15 cost and capacity,
//! 35.16 the million-scale accounting example, 35.17 release versioning, and 35.18 factory quality
//! control with independent audit.
//!
//! The scheduling half of 35 — jobs, workers, leases, idempotency-aware recovery, blueprint 40.30 —
//! belongs to `bioprism-factory` and is deliberately not duplicated here. This crate never runs
//! work; it decides what work is worth running and what the resulting pile of items is actually
//! worth.
//!
//! ## The one claim this crate exists to defend
//!
//! `AGENTS.md` states it as non-negotiable: **instance count is not benchmark count.** The section
//! index states the same thing from the blueprint side — "the factory scales by compiling decisions
//! and validated counterfactuals, not by paraphrasing questions."
//!
//! So the central type is [`EffectiveSize`], and the central design decision is that the nominal
//! count has no serializable form of its own. [`NominalCount`] implements neither `Serialize` nor
//! `Deserialize`; the only way to publish an instance count is inside an [`EffectiveSize`], which
//! carries the effective count and the inflation ratio in the same object. A report that headlines
//! a million cannot be produced without the honest number beside it.
//!
//! [`accounting::million_scale_example`] runs 35.16's worked design and reports both figures. The
//! finding is in `accounting`'s module docs and it is not flattering to the nominal number.
//!
//! ## Not implemented
//!
//! In-memory and single-process throughout. There is no distributed execution, no real object
//! storage backend, no network, and no embedding model: 35.10 asks for "embedding and graph
//! similarity" and this crate supplies only the relations it can compute exactly and defend —
//! content digest, equivalence class, and lineage — because an approximate similarity with no
//! ground truth is a false-merge generator, and false merge rate is one of 35.10's own metrics.
//!
//! Nothing reads a clock. Ordering — which matters most in [`escrow`], where the whole point is
//! proving a commitment preceded a run — uses a monotone logical [`escrow::Sequence`] issued by the
//! vault, because a wall-clock timestamp supplied by the party being audited proves nothing.
//! Randomness is `bioprism_worldgen::rng::SplitMix64`, seeded explicitly.
//!
//! [`EffectiveSize`]: effective::EffectiveSize
//! [`NominalCount`]: corpus::NominalCount

pub mod accounting;
pub mod adaptive;
pub mod audit;
pub mod cas;
pub mod corpus;
pub mod cost;
pub mod effective;
pub mod error;
pub mod escrow;
pub mod federation_trust_control_plane;
pub mod portfolio;
pub mod release;
pub mod schedule;
pub mod split;
pub mod quality_control_contract_model;
pub mod interpretation_visualization_assurance;
pub mod interpretation_interoperability_gateway;

pub use accounting::{million_scale_example, MillionScaleAccounting, Reconciliation};
pub use adaptive::{AdaptivePlan, Selection, StoppingDecision, Stratum};
pub use audit::{AuditReport, Auditor, QualityGate, ReleaseAudit};
pub use cas::{ComputationKey, Delta, ObjectStore, ReplayCache, Snapshot};
pub use corpus::{content_digest, Corpus, GeneratedItem, NominalCount};
pub use cost::{CostForecast, CostModel, FactoryPlan};
pub use effective::{
    ClusterStability, EffectiveSize, EffectiveSizeReport, RelationQuality, SimilarityRelation,
    STABILITY_PAIR_LIMIT,
};
pub use error::{
    AdaptiveError, AuditError, CacheError, EscrowError, ReleaseError, ScaleError, SplitError,
};
pub use escrow::{EscrowVault, Reveal, RevealCondition, Sequence};
pub use federation_trust_control_plane::{
    assure_federation, federation_trust_control_plane_manifest, FederationDisposition,
    FederationEnvelope8, FederationPeer7, FederationRequest4, FederationTrustError,
    CONTRACT_VERSION as FEDERATION_TRUST_CONTRACT_VERSION,
    FEATURE_ID as FEDERATION_TRUST_FEATURE_ID,
};
pub use portfolio::{Cell, PortfolioPlan, PortfolioReport};
pub use release::{ReleaseLedger, ReleaseVersion, Supersession};
pub use schedule::{GenerationSchedule, HiddenSeed, MutationBudget};
pub use split::{Contamination, FamilySplit, SplitReport, Tier};
pub use quality_control_contract_model::{
    model_prospective_quality_control_contract as model_quality_control_contract,
    model_prospective_quality_control_contract_json as model_quality_control_contract_json,
    prospective_quality_control_contract_manifest as quality_control_contract_manifest,
    validate_prospective_quality_control_contract_json as validate_quality_control_contract_json,
    ContractQualityMetric, ContractResearchObject, QualityControlContractError,
    QualityControlContractRequest, QualityVerdict2,
    CONTRACT_VERSION as QUALITY_CONTROL_CONTRACT_MODEL_CONTRACT_VERSION,
    FEATURE_ID as QUALITY_CONTROL_CONTRACT_MODEL_FEATURE_ID,
    INPUT_SCHEMA as QUALITY_CONTROL_CONTRACT_MODEL_INPUT_SCHEMA,
    OUTPUT_SCHEMA as QUALITY_CONTROL_CONTRACT_MODEL_OUTPUT_SCHEMA,
};
pub use interpretation_visualization_assurance::{
    assure_interpretation_visualization, assure_interpretation_visualization_json,
    interpretation_visualization_assurance_manifest, validate_interpretation_visualization_json,
    EvidenceBackedResult4, InteractiveInterpretation7, InterpretationCandidate4,
    InterpretationVisualizationAssuranceError,
    CONTRACT_VERSION as INTERPRETATION_VISUALIZATION_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_VISUALIZATION_FEATURE_ID,
    INPUT_SCHEMA as INTERPRETATION_VISUALIZATION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as INTERPRETATION_VISUALIZATION_OUTPUT_SCHEMA,
};
pub use interpretation_interoperability_gateway::{
    interoperate_interpretations, interoperate_interpretations_json,
    interpretation_interoperability_gateway_manifest,
    validate_interpretation_interoperability_json, EvidenceBackedResult2,
    InterpretationEndpoint2, InterpretationInteroperabilityError, InteractiveInterpretation6,
    CONTRACT_VERSION as INTERPRETATION_INTEROPERABILITY_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_INTEROPERABILITY_FEATURE_ID,
};
