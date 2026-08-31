//! The rest of blueprint section 12: storage architecture, the relational catalog, service-level
//! objectives, compute providers, placement, and local and federated deployment.
//!
//! Three sibling crates already hold most of section 12. `bioprism-ledger` is the bitemporal
//! event log — valid, record and release time as distinct newtypes, hash-chained entries
//! committing to a payload digest so that redaction preserves tamper-evidence. `bioprism-store`
//! is the content-addressed index that made compilation output-sensitive, at a measured 638x.
//! `bioprism-infra` is the cache whose hits are provably the same computation, the invalidation
//! that reports its own completeness, the quality gates, the tiering and the storage quota. This
//! crate is the seven modules none of them claimed, and it builds no second log, no second cache
//! and no second quota: where it needs an epoch it uses `bioprism-infra`'s, where it needs a
//! durability class it uses `bioprism-infra`'s, and where it needs an audit record it constructs
//! a `bioprism-ledger` event and hands it back rather than storing one.
//!
//! # The classification
//!
//! The test is the workspace's settled one — *is the detailed design a set of predicates over an
//! artifact, or a description of what people do?* — with `bioprism-bioevalx`'s refinement that
//! the split can run **inside** a module rather than between modules. For these seven it runs
//! inside four of them, and there is not one pure process module in the set:
//!
//! | id | module | class | implemented here | named and not built |
//! |---|---|---|---|---|
//! | 12.03 | Relational Catalog Schema | predicates | [`catalog`] — six constraint clauses, publication atomicity, the outbox | the twenty other tables |
//! | 12.12 | Observability and SLOs | predicates | [`slo`] — taxonomy, budgets, exact-rational conformance, burn rate | collection, dashboards, alert delivery |
//! | 12.14 | Distributed Compute and Placement | predicates | [`placement`] — all five subsections | the scheduler that would act on a decision |
//! | 12.02 | Storage Architecture | split inside | [`topology`] — the promise assignment and its parity | the five product choices |
//! | 12.13 | Compute Providers and Kubernetes | split inside | [`provider`] — publication, isolation adequacy, warm pools, the no-Kubernetes fallback | *Kubernetes roles*, whole |
//! | 12.15 | Local-First Deployment | split inside | [`local`] — offline contract, envelope, doctor, parity claim | *Install*, whole |
//! | 12.16 | Cloud and Federated Deployment | split inside | [`federation`] — planes, patterns, residency, import trust | VPCs, secrets, replication transport |
//!
//! This is the opposite of the split `bioprism-stewardship` found in section 14, where twelve of
//! eighteen modules were councils and cadences no code should implement, and it matches what
//! `bioprism-bioevalx` found in section 26. The reason is visible in the text: section 12's
//! process content is not distributed by module but by *block*, and the blocks are identical
//! everywhere, which is the subject of the measurement below.
//!
//! ## How much distinguishing content there is
//!
//! Little, and it is worth being precise about how little, because it bounds what this crate
//! could have been. Across the seven, the `## Detailed design` body — the only section that
//! differs materially between modules — averages **125 words**, from 96 (Compute Providers) to
//! 156 (Relational Catalog Schema). Counting lines unique to a single one of section 12's
//! twenty-two module files gives an average of **19.7 lines of 65**, so roughly **30.3%** of each
//! in-scope file is that file's own. The seven together contain 148 unique lines. Everything in
//! this crate descends from those.
//!
//! # The rule this section keeps needing
//!
//! `bioprism-infra` found the search-and-cache module disposing of invalidation with
//! "content-addressed immutability avoids most invalidation", leaving *most* unexamined, so that
//! a conforming implementation may serve the residue forever. The seven modules here are the
//! residue's other half: every one of them answers a question from something that is not the
//! authoritative source. A search index instead of the catalog. A projection behind an outbox. A
//! provider's published conformance instead of a suite the platform ran. A worker's
//! self-declaration instead of a probe. An availability figure over the telemetry that happened
//! to arrive. A hub's replicated record instead of a local one. In every case the value has the
//! same shape as an authoritative one.
//!
//! So the invariant is the section's usual one — **a stale or unverified answer must not be
//! indistinguishable from a fresh, verified one** — and this crate states it in a currency the
//! three siblings that reached it first do not have. `bioprism-infra`'s index reports lag,
//! `bioprism-hubapi`'s mirror reports replication and has no `is_current`, `bioprism-tokens`'
//! `Currency` reports declared validity and has no `is_fresh`. All three answer *how old is it*.
//! [`basis::Basis`] answers **who looked**, and [`basis::Coverage`] answers **at how much of it**;
//! the second is the one this section needs most, because it names eight service objectives and
//! gives none of them a denominator, so a rate computed from whatever telemetry survived an
//! outage is indistinguishable from one computed over everything.
//!
//! The enforcement is structural rather than advisory. [`basis::Attested`] has private fields, no
//! `From<T>`, no `Deref`, and derives `PartialEq` over the value *and* the basis *and* the
//! coverage — so two answers that agree on the value and disagree on where it came from are `!=`,
//! and every caller that compares them sees it without having been told to look.
//!
//! # Shape of the API
//!
//! ```
//! use bioprism_dataops::{
//!     reference_local, Attribution, Basis, BudgetPolicy, Confidence, Coverage, DataClass, Epoch,
//!     FailureDomain, Observations, ServiceObjective, Target, Window,
//! };
//!
//! let topology = reference_local()?;
//!
//! // The same number, read from two places, is not the same answer.
//! let from_catalog = topology.attest(
//!     DataClass::Metadata,
//!     412u64,
//!     Epoch::new(7),
//!     0,
//!     Coverage::Complete { observed: 412 },
//! );
//! let from_index = topology.attest(
//!     DataClass::Search,
//!     412u64,
//!     Epoch::new(7),
//!     3,
//!     Coverage::Complete { observed: 412 },
//! );
//! assert_eq!(from_catalog.value(), from_index.value());
//! assert_ne!(from_catalog, from_index);
//! assert!(from_catalog.basis().is_first_hand());
//! assert!(matches!(from_index.basis(), Basis::Derived { lag_epochs: 3, .. }));
//!
//! // An objective cannot be met on telemetry with a hole in it, and an agent failure is not a
//! // service failure.
//! let objective = ServiceObjective::new(
//!     "api-read-availability",
//!     Target::new(999, 1000, "api-read-availability")?,
//!     Window::new(Epoch::new(0), Epoch::new(30))?,
//! )?;
//! let seen = Observations::new(
//!     900,
//!     [Attribution::classified(
//!         FailureDomain::Agent,
//!         ["agent returned an invalid action".to_string()],
//!         Confidence::Certain,
//!     )?],
//!     Coverage::Partial { observed: 901, expected: 1000 },
//! );
//! let report = objective.evaluate(&seen, &BudgetPolicy::platform_default());
//! assert!(!report.conformance.is_met());
//! assert_eq!(report.charged, 0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Determinism and the absence of a clock
//!
//! Nothing here reads the system time, opens a file, or touches a socket. Windows, leases,
//! observation epochs and replication receipts are all [`Epoch`] values the caller supplies, the
//! discipline `bioprism-governance`, `bioprism-ledger` and `bioprism-infra` already keep. All
//! objective arithmetic is exact integer arithmetic in `u128` over rational targets — `999 per
//! 1000`, never `0.999` — so no comparison rounds and a report can be hashed. Shard seeds are
//! digests of the unit id and a caller-supplied salt, so two runs of the same pipeline shard
//! identically.
//!
//! # What is deliberately not implemented
//!
//! **Everything here is in-memory, single-process, and touches no filesystem, no database, no
//! container runtime and no network.** Specifically:
//!
//! - **No storage engine.** [`topology`] holds statements about where bytes would go and holds no
//!   bytes. There is no SQLite, no PostgreSQL, no object store, no Parquet writer.
//! - **No transactions across processes.** [`catalog::Catalog::publish`] validates before it
//!   mutates, which is atomic within one `&mut self` call and is nothing at all against a
//!   concurrent writer. There is no lock in this crate.
//! - **No telemetry.** [`slo::Observations`] is filled in by a caller that measured something
//!   this crate cannot see, so a green report attests to that caller's counting.
//! - **No isolation.** [`provider::IsolationStrength`] is a declared property this crate
//!   compares. Modelling a boundary is not providing one, which is the position
//!   `bioprism-safety` states for the workspace.
//! - **No verification of anything signed.** [`placement::ResultEvidence`] has no `Verified`
//!   variant and [`federation::SignedRecord`]'s attestation is checked for presence only. Both
//!   need cryptography and a key distribution story this crate does not have.
//! - **No probes.** [`local::DoctorReport`] records outcomes somebody else obtained.
//! - **No resource quantities.** Placement lists CPU, GPU, memory and disk per worker; the
//!   bin-packing that would use them is absent, which is a real gap and not a simplification.
//! - **No tenancy enforcement.** [`catalog::Visibility`] is a filter this crate applies on read.
//!   `bioprism-infra` records having no tenant concept at all and calls its absence a gap; this
//!   is a slightly smaller gap of the same kind.
//!
//! # Where section 12 assumes infrastructure it never specifies
//!
//! Recorded because each of these shaped a decision above.
//!
//! 1. **12.02 never says which data class is evidence.** "Preserve immutable evidence" is a
//!    responsibility with no subject, so nothing in the section refuses a topology that puts
//!    traces in a mutable store. [`topology::DataClass::holds_immutable_evidence`] is that
//!    decision made explicit, and it is a reading rather than a quotation.
//! 2. **12.02's rebuild chain has no required bottom.** Search indexes "can be rebuilt from
//!    canonical metadata"; nothing requires the metadata store to itself be canonical, so a
//!    conforming topology can be rebuildable all the way down.
//! 3. **12.02 requires local and team deployments and never says what makes them the same
//!    platform.** Any two unrelated topologies satisfy the sentence. [`topology::parity`]
//!    compares promises and deliberately ignores technology, because the section *requires* the
//!    technologies to differ.
//! 4. **12.03 stops at "search updates consume the outbox".** It never states what a query
//!    answered from a projection with unconsumed events means — the same unexamined residue
//!    `bioprism-infra` found one module along.
//! 5. **12.03's row security is enforced "where supported".** An unbounded exemption, with no
//!    requirement that an implementation report which half of the rule applied to a given read.
//! 6. **12.12 gives eight objectives and no denominators.** No target, no window, no population.
//!    [`slo::declared_objective_names`] returns exactly what the section specifies, which is a
//!    list of names.
//! 7. **12.12 requires classification evidence and confidence to be retained and never says what
//!    an unclassified failure costs.** Excluding it is the arithmetic that makes availability
//!    improve as diagnosis degrades; [`slo::BudgetPolicy::charges`] charges it.
//! 8. **12.12 asks for burn-rate alerts and never says what one does on a window with missing
//!    telemetry.** [`slo::AlertDecision::CannotEvaluate`] is the third state that answer needs.
//! 9. **12.13 uses one verb, "publishes", for a commercial fact and a test result.** A provider's
//!    conformance claim and a suite the platform ran are the same sentence in the section and
//!    different values here.
//! 10. **12.13 lists isolation mechanisms "according to threat level" and gives no mapping and no
//!     threat vocabulary.** [`provider::ThreatLevel::minimum_isolation`] is one function, so a
//!     disagreement has one place to land.
//! 11. **12.13 promises the local path needs none of four named services and gives no mechanism
//!     that would notice the promise breaking.** [`provider::local_path_is_self_contained`] is a
//!     weak substring test over declared technologies, and its weakness is stated at the function.
//! 12. **12.14 asks for "remote attestation where justified" without defining justified**, and
//!     never says what a placement made from an unattested declaration is worth.
//! 13. **12.14 says an agent-caused timeout is an evaluation outcome and is silent on an
//!     unattributable one.** [`placement::attribute_timeout`] refuses to guess, which is the
//!     expensive direction.
//! 14. **12.15 requires degrading "with clear diagnostics, not hangs" and supplies no timeout
//!     model**, and asks for a "shareable redacted report" without saying what redaction
//!     preserves. Here it preserves the probe name, the outcome variant, and a digest that keeps
//!     two machines comparable.
//! 15. **12.15 requires "semantic parity with hosted mode" and does not say which semantics.**
//!     [`local::ParityClaim`] can only be established from an actual comparison, so an
//!     unsubstantiated claim is a distinguishable value rather than one that is true by default.
//! 16. **12.16 says execution pools may live in customer networks and classifies no other
//!     plane.** The conservative reading is [`federation::Plane::is_control_plane`].
//! 17. **12.16 leaves federation trust to "local policy" and never says what an accepted external
//!     record's provenance becomes.** [`federation::import_record`] answers: replicated, always,
//!     carrying both the origin's epoch and the local receipt epoch, because either one alone
//!     cannot tell a fresh copy of stale data from a stale copy of fresh data.
//!
//! ## How much of section 12 is template
//!
//! **69.8%**, by `bioprism-infra`'s method, independently recomputed here: split each of the 22
//! module files at its `##` headings, take the H2 bodies, and classify a body as template when
//! its whitespace-normalized form is byte-identical across all 22 files. Four qualify —
//! `Invariants`, `Failure modes and mitigations`, `Testing strategy`, `Implementation sequence` —
//! for 55.6% of H2 body characters; adding `Why this module exists`, identical apart from the
//! module's own name spliced into one sentence, reaches 69.8%. That agrees with the 69.7% already
//! on record to within a rounding difference in whitespace normalization, which is the useful
//! part of having recomputed it.
//!
//! ### Sensitivity
//!
//! **Threshold dominates, and for one identifiable reason.** Counting lines whose
//! whitespace-normalized form recurs in at least *k* of the 22 files: *k* = 50% and *k* = 80%
//! both give **67.9%** of non-empty lines, identical to each other; *k* = 100% gives **60.6%**.
//! The 7.3-point gap is not a gradient. Exactly **five distinct lines** sit in the middle
//! stratum, they are the whole of the `Metrics` block, and they appear in 21 of the 22 files —
//! the section overview is the one that omits it. So the parameter most often argued about is
//! decided here by one block's presence in one file.
//!
//! **Definition barely matters.** At *k* = 100%, keeping front matter and headings gives 60.6%,
//! stripping front matter 57.7%, stripping headings 61.8%, stripping both 57.9% — a range of 4.1
//! points. This is the reverse of what `bioprism-bioevalx` measured for section 26, where
//! definition moved the answer 17.6 points and threshold moved it not at all.
//!
//! **Unit costs about three points.** Character share rather than line share: 70.6% against 67.9%
//! at *k* = 50%, and 64.0% against 60.6% at *k* = 100%.
//!
//! Across the three definitions the answer spans **60.6% to 69.8%**, nine points. It is stated
//! with its method for that reason, and should not be quoted without one.
//!
//! # Citation discipline
//!
//! `tools/coverage.sh` counts a module as covered when its `NN.MM` token appears anywhere under
//! `crates/`, so writing the id of a module this crate did not implement would move the coverage
//! figure without moving the platform. Eight other section 12 modules are discussed in these docs
//! — the event and object models, content-addressed object storage, the search and vector
//! projections, the cache, the queue, idempotency, backup, and the quota model — and every one is
//! named by title and owning crate, never by id. [`citations::DECLARED`] is the permitted set,
//! and `tests/citations.rs` reimplements the script's token rule, scans every file in this crate,
//! and asserts the two are equal.

pub mod basis;
pub mod catalog;
pub mod citations;
pub mod error;
pub mod federation;
pub mod ingestion_integrity_support;
pub mod local_ingestion_integrity_inference;
pub mod multimodal_ingestion_integrity_inference;
pub mod throughput_ingestion_integrity_inference;
pub mod federated_continual_ingestion_integrity_inference;
pub mod local_ingestion_integrity_contract_model;
pub mod multimodal_ingestion_integrity_contract_model;
pub mod throughput_ingestion_integrity_contract_model;
pub mod federated_continual_ingestion_integrity_contract_model;
pub mod local_ingestion_integrity_research_copilot;
pub mod multimodal_ingestion_integrity_research_copilot;
pub mod throughput_ingestion_integrity_research_copilot;
pub mod federated_continual_ingestion_integrity_research_copilot;
pub mod local_ingestion_integrity_workflow_fabric;
pub mod multimodal_ingestion_integrity_workflow_fabric;
pub mod throughput_ingestion_integrity_workflow_fabric;
pub mod federated_continual_ingestion_integrity_workflow_fabric;
pub mod local;
pub mod placement;
pub mod provider;
pub mod slo;
pub mod topology;
pub mod provenance_signing_workflow_fabric;

pub use basis::{Attested, Basis, Coverage, PartyId};
pub use catalog::{
    AliasBinding, AliasName, AuditContext, Catalog, MediaType, Namespace, ObjectHeader, ObjectId,
    OutboxCursor, OutboxEvent, Publication, PublicationId, PublicationReceipt, PublicationRequest,
    Revision, RevisionId, Status, StatusEntry, Visibility,
};
pub use error::{
    BasisError, CatalogError, FederationError, LocalError, PlacementError, ProviderError, SloError,
    TopologyError,
};
pub use federation::{
    import_record, minimize_cross_region, plan_replication, private_worker_link,
    ArtifactSensitivity, ConnectionDirection, DeploymentPlan, FederationPolicy, Plane,
    PlanePlacement, Replication, SignedRecord, TenantPattern,
};
pub use local::{
    Demand, Detail, DoctorReport, NetworkPolicy, OfflineContract, ParityClaim, ProbeOutcome,
    Readiness, Requirement, Resolution, ResourceEnvelope, Source, Unsatisfiable,
};
pub use placement::{
    accept_result, attribute_timeout, plan_shards, AttemptRecord, Fleet, JobRequirements,
    MatchReason, PlacementDecision, PlacementPolicy, Refusal, Repeatability, ResultEvidence, Seed,
    ShardPlan, TaskId, TaskLease, TimeoutEvidence, UnitId, WorkerDeclaration, WorkerId,
};
pub use provider::{
    local_path_is_self_contained, AdmissionPolicy, Capability, ConformanceLevel, ExternalService,
    IsolationStrength, ProviderCatalog, ProviderId, ProviderKind, ProviderProfile, Region,
    ThreatLevel, TrustDomain, WarmPool,
};
pub use slo::{
    burn_rate_alert, declared_objective_names, AlertDecision, Attribution, BudgetPolicy,
    BurnThreshold, Confidence, Conformance, FailureDomain, Indeterminate, Observations,
    ServiceObjective, SloReport, Target, Window,
};
pub use topology::{
    parity, reference_local, reference_team, DataClass, Deployment, Mutability, ParityReport,
    PromiseDifference, Promises, StorageTopology, StoreProfile, TopologyDraft,
};
pub use provenance_signing_workflow_fabric::{
    assure_prospective_provenance, prospective_provenance_assurance_manifest,
    ArtifactAndDerivation3, ArtifactAndDerivationRequest3, DerivationEvidenceState,
    ProspectiveProvenanceError, SignedProvenanceEnvelope7,
    FEATURE_ID as PROVENANCE_SIGNING_WORKFLOW_FABRIC_FEATURE_ID,
    CONTRACT_VERSION as PROVENANCE_SIGNING_WORKFLOW_FABRIC_CONTRACT_VERSION,
};
pub use ingestion_integrity_support::*;
pub use local_ingestion_integrity_inference::*;
pub use multimodal_ingestion_integrity_inference::*;
pub use throughput_ingestion_integrity_inference::*;
pub use federated_continual_ingestion_integrity_inference::*;
pub use local_ingestion_integrity_contract_model::*;
pub use multimodal_ingestion_integrity_contract_model::*;
pub use throughput_ingestion_integrity_contract_model::*;
pub use federated_continual_ingestion_integrity_contract_model::*;
pub use local_ingestion_integrity_research_copilot::*;
pub use multimodal_ingestion_integrity_research_copilot::*;
pub use throughput_ingestion_integrity_research_copilot::*;
pub use federated_continual_ingestion_integrity_research_copilot::*;
pub use local_ingestion_integrity_workflow_fabric::*;
pub use multimodal_ingestion_integrity_workflow_fabric::*;
pub use throughput_ingestion_integrity_workflow_fabric::*;
pub use federated_continual_ingestion_integrity_workflow_fabric::*;

/// The caller-supplied logical clock every lifecycle decision in this crate advances on.
#[doc(inline)]
pub use bioprism_infra::Epoch;

/// What losing a store costs, reused rather than redefined.
#[doc(inline)]
pub use bioprism_infra::Durability;

/// The digest type every content-addressed reference in this crate carries.
#[doc(inline)]
pub use bioprism_ids::ContentHash;

/// The audit vocabulary. Re-exported because [`catalog::Catalog::publish`] takes and returns
/// these, and a caller should not have to depend on the ledger directly to name the types in its
/// own signature — while the fact that they *are* the ledger's is exactly the point being made.
#[doc(inline)]
pub use bioprism_ledger::{Actor, Event, EventTimes, RecordTime, ValidTime};
