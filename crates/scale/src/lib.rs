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
pub mod portfolio;
pub mod release;
pub mod schedule;
pub mod split;

pub use accounting::{million_scale_example, MillionScaleAccounting, Reconciliation};
pub use adaptive::{AdaptivePlan, Selection, Stratum, StoppingDecision};
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
pub use portfolio::{Cell, PortfolioPlan, PortfolioReport};
pub use release::{ReleaseLedger, ReleaseVersion, Supersession};
pub use schedule::{GenerationSchedule, HiddenSeed, MutationBudget};
pub use split::{Contamination, FamilySplit, SplitReport, Tier};
