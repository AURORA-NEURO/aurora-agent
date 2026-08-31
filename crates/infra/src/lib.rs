#![allow(ambiguous_glob_reexports)]

//! Data infrastructure: the part of blueprint section 12 that `bioprism-ledger` and
//! `bioprism-store` do not already own.
//!
//! # What is already taken, and why this crate does not rebuild it
//!
//! `bioprism-ledger` implements 40.09 and the bitemporal event log — valid, record and release
//! time as distinct newtypes, hash-chained entries, projections, checkpoints, retention and
//! redaction. `bioprism-store` implements the content-addressed indexed storage that made the
//! compile path output-sensitive: a measured 638x on a one-million-fact world, because compile
//! cost tracks the compiled region rather than the corpus. Neither is duplicated here. Where
//! this crate needs a retention boundary it asks the ledger's [`RetentionWindow`] rather than
//! inventing a second one, and it does so in the one direction that is safe:
//! [`lifecycle`] refuses to reclaim storage the ledger still promises to answer about.
//!
//! # What this crate owns
//!
//! | module | blueprint | the property it exists to hold |
//! |---|---|---|
//! | [`cache`] | 12.10, 12.08 | a hit is provably the same computation, and a miss says why |
//! | [`invalidation`] | 12.08 | an invalidation that cannot be proved total reports itself partial and names the unknown region |
//! | [`quality`] | 12.11, 40.31 | checks produce pass, fail-with-witness, or not-runnable-with-reason — three states, never two |
//! | [`tiering`] | 40.06 | hot/warm/cold under a stated rule, planned before it is applied |
//! | [`lifecycle`] | 40.06, 12.22 | deletion leaves a tombstone; result history is never rewritten |
//! | [`quota`] | 12.21, 12.20 | a storage budget that cannot be duplicated by copying |
//! | [`index`] | 12.07, 12.08 | a projection answers with candidates and a freshness, never with evidence |
//! | [`backup`] | 12.18, 12.19 | a restore never claims integrity it did not check |
//! | [`epoch`] | — | the caller-supplied logical clock all of the above advance on |
//!
//! # The idea the crate is built around
//!
//! A cache is where a platform that promises reproducibility usually starts lying, and it lies in
//! two places. The first is the key: a digest match is treated as a computation match, so two
//! different computations that reduce to one string share an answer. `bioprism-scale` already
//! closed that for its replay cache and [`cache::Cache`] follows it — private key components, no
//! constructor from a digest, and a component-by-component comparison *after* the digest matches,
//! with a difference reported as [`error::CacheError::KeyCollision`] naming the component.
//!
//! The second is invalidation, and it is the one this crate is really for. When something
//! upstream changes, a cache must say which entries are now wrong. It can only say that from
//! declared dependencies, and real dependency graphs are incomplete. The usual behaviour is to
//! invalidate what is declared and serve the rest, which converts a documentation gap into a
//! silent correctness bug. Here, [`invalidation::InvalidationPlan`] reports
//! [`invalidation::Completeness::Partial`] with the unknown region named — the opaque resources,
//! the resources nobody added to the graph, and the entries that reach them — and
//! [`cache::Cache::apply`] responds by marking that region [`cache::EntryStatus::Unproven`], which
//! stops it being served. **An incomplete invalidation costs hit rate, not correctness.**
//!
//! # Shape of the API
//!
//! ```
//! use bioprism_infra::{
//!     Cache, CodeIdentity, ComputationKey, DependencyDeclaration, DependencyGraph, Epoch,
//!     InvalidationPlan, KeySchema, MissReason, ResourceId, ReuseRule,
//! };
//! use serde_json::json;
//!
//! let schema = KeySchema::declare(
//!     "derived-table",
//!     ["inputs", "code", "policy"],
//!     ReuseRule::SameBuildOnly,
//! )?;
//! let key = ComputationKey::build(
//!     &schema,
//!     [("inputs", "cohort@7"), ("code", "etl@2"), ("policy", "strict")],
//! )?;
//! let build = CodeIdentity::parse("etl-build-91")?;
//!
//! let mut cache = Cache::new(schema);
//! cache.insert(
//!     key.clone(),
//!     json!({ "rows": 412 }),
//!     build.clone(),
//!     Epoch::new(1),
//!     DependencyDeclaration::Undeclared,
//! )?;
//!
//! let hit = cache.lookup(&key, &build)?;
//! assert_eq!(hit.hit().map(|hit| hit.proof.matched.len()), Some(3));
//!
//! // Nobody declared what that entry depends on, so an invalidation cannot prove it unaffected.
//! let mut graph = DependencyGraph::new();
//! graph.declare(ResourceId::parse("cohort")?, [])?;
//! let plan = InvalidationPlan::compute(&graph, ResourceId::parse("cohort")?, cache.declarations());
//! assert!(!plan.is_complete());
//!
//! cache.apply(&plan, Epoch::new(2))?;
//! let after = cache.lookup(&key, &build)?;
//! assert!(matches!(
//!     after.miss_reason(),
//!     Some(MissReason::UnprovenAfterPartialInvalidation { .. })
//! ));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Determinism
//!
//! Nothing in this crate reads a clock. Tier demotion, cache staleness and quota windows all
//! advance on an [`epoch::Epoch`] the caller supplies, the same discipline `bioprism-governance`
//! and `bioprism-ledger` already keep, so every plan this crate produces is reproducible on every
//! machine and comparable between two runs.
//!
//! # What is deliberately not implemented
//!
//! Everything here is **in-memory, single-process, and touches no filesystem, no database and no
//! network**. There is no storage backend: [`lifecycle::Lifecycle`] manages records *about*
//! objects, never their bytes, and [`lifecycle::LocalLayout`] computes the 40.06 paths as strings
//! without creating a directory. Specifically absent, each because it needs infrastructure this
//! crate does not have:
//!
//! - **Eviction.** 12.10 wants "cost-aware and pin-aware eviction with access recency, rebuild
//!   cost, dependency fan-out, and release pins". Access recency and pins are modelled in
//!   [`tiering`]; rebuild cost is not, because nothing here can measure it.
//! - **Tenancy.** 12.10 and 12.07 both require tenant isolation and pre-ranking tenant filters.
//!   There is no tenant in this crate at all, and adding one after the fact is exactly how a
//!   cross-tenant cache leak happens — this is a gap, not a decision that it is unnecessary.
//! - **Concurrency.** Single writer, no locks, no leases. 12.17's lease lifecycle belongs to the
//!   job worker.
//! - **Encryption, residency enforcement, and cryptographic erasure.** [`lifecycle`] records a
//!   classification and a residency on every object and refuses to guess them, but enforces
//!   neither; a tombstone here destroys a record, not a key.
//! - **Migration.** 12.22's five migration classes and compatibility readers are not modelled.
//! - **Queues.** 12.08 and 12.09 pair caching with a work queue; that is a runtime concern.
//! - **Sampled recomputation and poison canaries.** 12.10's validation section asks for both as
//!   the empirical check that the cache is not lying. This crate makes lying structurally hard
//!   instead, which is not the same thing and does not replace the measurement.
//!
//! # Where section 12 assumes infrastructure it never specifies
//!
//! Recorded here because the gaps shaped the design. 12.08 says content addressing "avoids most
//! invalidation" without saying what happens to the rest — the residue is this crate's central
//! module. 12.10 requires keys to "include all semantically relevant inputs" and lists thirteen,
//! without saying who decides the list is complete or what happens when it changes; [`cache::KeySchema`]
//! is that decision made explicit and versioned by digest. 12.21 requires budgets to be enforced
//! "per operation, project, organization, provider, day/month" with a "reserve" protecting
//! cleanup, without a mechanism preventing a budget from being handed to two subsystems at once;
//! [`quota::StorageQuota`] is not `Clone` for that reason. 12.18 lists backup priorities without
//! a rule for declaring a restore trustworthy; [`backup::RestoreVerdict`] refuses to say
//! `Verified` without a closure check.
//!
//! Two omissions are worth naming separately because they are the ones a later contributor is
//! most likely to assume were handled. Section 12 never says **who owns a cache key's component
//! list** — 12.10 lists the inputs and stops — so nothing in the specification catches a key that
//! quietly lost a component in a refactor; that is what [`cache::KeySchema`]'s digest is for.
//! And section 12 never says **what an incomplete dependency graph means for a cache**: 12.08
//! disposes of invalidation with "content-addressed immutability avoids most invalidation" and
//! leaves the word *most* unexamined, so a conforming implementation may serve the residue
//! indefinitely.
//!
//! ## How much of section 12 is template
//!
//! **69.7%**, by the following method. Split each of the 22 module files at its `##` headings and
//! take the H2 bodies (which excludes the YAML front matter and the H1 title). Classify a body as
//! template if, after whitespace normalization, it is byte-identical across all 22 modules; four
//! sections qualify — `Invariants`, `Failure modes and mitigations`, `Testing strategy`,
//! `Implementation sequence` — accounting for 55.5% of all H2 body characters. Add
//! `Why this module exists`, whose two paragraphs are identical apart from the module's own name
//! spliced into one sentence, and the template share reaches **69.7%**, leaving 30.3% —
//! `Purpose`, `Responsibilities`, `Detailed design`, and two one-off sections — as the
//! module-specific text this crate was actually built from.
//!
//! Two other definitions of the same corpus, stated so the number above can be compared against a
//! differently-defined one rather than taken as *the* boilerplate figure: counting **lines** whose
//! whitespace-normalized form recurs in at least half the 22 modules gives 67.9% of non-empty
//! lines (70.6% by character); restricting to lines present in *all* 22 gives 60.6% (64.0% by
//! character). The definition moves the answer by nine points, which is why it is stated rather
//! than assumed.
//!
//! By contrast 40.06, the one build-ready module this crate implements, is 34 lines with no
//! repeated section at all, and four of its sentences produced [`lifecycle::Tombstone`],
//! [`lifecycle::StorageArea`], [`lifecycle::LocalLayout::object_path`] and
//! [`lifecycle::GcReport`]. Where the two overlap, 40.06 wins.

pub mod backup;
pub mod cache;
pub mod epoch;
pub mod error;
pub mod federated_continual_reliability_integrity_contract_model;
pub mod federated_continual_reliability_integrity_inference;
pub mod federated_continual_reliability_integrity_research_copilot;
pub mod federated_continual_reliability_integrity_workflow_fabric;
pub mod index;
pub mod invalidation;
pub mod lifecycle;
pub mod local_reliability_integrity_contract_model;
pub mod local_reliability_integrity_inference;
pub mod local_reliability_integrity_research_copilot;
pub mod local_reliability_integrity_workflow_fabric;
pub mod multimodal_reliability_integrity_contract_model;
pub mod multimodal_reliability_integrity_inference;
pub mod multimodal_reliability_integrity_research_copilot;
pub mod multimodal_reliability_integrity_workflow_fabric;
pub mod quality;
pub mod quota;
pub mod reliability_integrity_support;
pub mod throughput_reliability_integrity_contract_model;
pub mod throughput_reliability_integrity_inference;
pub mod throughput_reliability_integrity_research_copilot;
pub mod throughput_reliability_integrity_workflow_fabric;
pub mod tiering;

pub use backup::{BackedUpItem, BackupClass, BackupSet, RestoreReport, RestoreVerdict};
pub use cache::{
    ApplyReport, Cache, CacheEntry, CacheHit, CodeIdentity, ComputationKey, EntryStatus, HitProof,
    KeySchema, Lookup, MissReason, ReuseRule,
};
pub use epoch::Epoch;
pub use error::{
    BackupError, CacheError, IndexError, InvalidationError, LifecycleError, QualityError,
    QuotaError,
};
pub use federated_continual_reliability_integrity_contract_model::*;
pub use federated_continual_reliability_integrity_inference::*;
pub use federated_continual_reliability_integrity_research_copilot::*;
pub use federated_continual_reliability_integrity_workflow_fabric::*;
pub use index::{CandidateId, Freshness, IndexAnswer, Projection};
pub use invalidation::{
    Completeness, DependencyDeclaration, DependencyGraph, InvalidationPlan, ResourceId,
    UnknownRegion,
};
pub use lifecycle::{
    Classification, DeletionBasis, Durability, GcPolicy, GcReport, Lifecycle, LocalLayout,
    ObjectId, ObjectRecord, Residency, StorageArea, Tombstone,
};
pub use local_reliability_integrity_contract_model::*;
pub use local_reliability_integrity_inference::*;
pub use local_reliability_integrity_research_copilot::*;
pub use local_reliability_integrity_workflow_fabric::*;
pub use multimodal_reliability_integrity_contract_model::*;
pub use multimodal_reliability_integrity_inference::*;
pub use multimodal_reliability_integrity_research_copilot::*;
pub use multimodal_reliability_integrity_workflow_fabric::*;
pub use quality::{
    Check, CheckOutcome, Dataset, Gate, GateReport, GateVerdict, NotRunnable, ReferenceSets,
    Witness,
};
pub use quota::{Purpose, StorageClass, StorageQuota};
pub use reliability_integrity_support::*;
pub use throughput_reliability_integrity_contract_model::*;
pub use throughput_reliability_integrity_inference::*;
pub use throughput_reliability_integrity_research_copilot::*;
pub use throughput_reliability_integrity_workflow_fabric::*;
pub use tiering::{AccessRecord, Tier, TierReason, TierTransition, TieringPlan, TieringPolicy};

#[doc(inline)]
pub use bioprism_ledger::RetentionWindow;
