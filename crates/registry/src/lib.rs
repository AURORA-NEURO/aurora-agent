//! The registry: benchmark packs, trust tiers, promotion, release gating and a publication log.
//!
//! Implements blueprint 10 (Registry and Hub — 10.02 architecture, 10.05 publication lifecycle,
//! 10.07 trust tiers and review), 27 (Benchmark Factory and Hub — 27.16 million-scale accounting
//! and release policy, 27.18 trust tiers, review and promotion) and 40.32 (conformance suites and
//! certification), against the release gates of 00.09 and the claim ladder of 43.40.
//!
//! # The one idea
//!
//! A registry that lets a publisher assert a badge is a marketing surface, not an evidence system.
//! Here a tier is **computed from checkable properties of the artifact** and nothing else. Every
//! requirement in [`tier::Requirement`] is a predicate a third party can run on a pack it did not
//! build, from a document it downloaded, without asking the publisher anything. A pack that fails
//! a requirement is demoted, and the requirement that demoted it is named.
//!
//! The consequence worth stating plainly: there is no API in this crate that sets a tier. There is
//! [`promote::promote`], which asks for one and returns the unmet requirements when the evidence
//! does not support it.
//!
//! # Two digests, and why
//!
//! A pack has an **artifact digest** ([`pack::BenchmarkPack::digest`]) over the whole document,
//! and a **core digest** ([`pack::BenchmarkPack::core_digest`]) over everything except the
//! provenance block. Reviews and rebuild attestations live inside the pack and name the core
//! digest, so accumulating evidence about a pack does not invalidate the evidence already
//! attached to it — while any edit to the benchmark *content* invalidates every review of it at
//! once. That is the anti-staleness mechanism 27.18 asks for ("review becomes stale"), obtained
//! structurally rather than by expiry dates, which is fortunate because this crate has no clock.
//!
//! # What this crate is not
//!
//! Local-first, per blueprint 10.02's "local-first contract". Deliberately absent:
//!
//! - **No network.** No REST surface, no OCI transport, no federation or replication. The index in
//!   [`index`] is an in-process value that serialises to JSON; hosting it is somebody else's job.
//! - **No signing keys, no transparency log.** Provenance records carry publisher and reviewer
//!   *names*, which are claims about identity, not proofs of it. 10.02 separates benchmark trust
//!   from publisher identity; only the first is implemented here. A reviewer name is used solely
//!   for the independence check (reviewer ≠ publisher), which is a structural check on a document,
//!   not an authentication.
//! - **No multi-tenancy, visibility rules, embargo, moderation or quarantine workflow.** A CI
//!   quarantine list exists as a plain set of digests in [`gate::Policy`]; the policy layer of
//!   10.02 is not built.
//! - **No exploit, contamination, privacy or license *scanning*.** A license must be declared;
//!   nobody checks it is true.
//! - **No search, ranking, recommendation, benchmark cards or health dashboards** (10.10–10.13).
//!
//! Per 40.32's first invariant — *conformance is not trust or accuracy* — a pack that passes the
//! gate here has demonstrated that its evidence is internally consistent and sufficient for the
//! tier it claims. It has not been shown to be scientifically good.

pub mod context_assurance;
pub mod gate;
pub mod index;
pub mod pack;
pub mod promote;
pub mod resource_assurance;
pub mod tier;

pub use context_assurance::{
    assure_context_compilation, CompiledContext, ContextAssuranceError, ContextAssuranceReceipt,
    ContextCompilationRequest, ContextDisposition, ContextFact, FactState,
    CONTRACT_VERSION as CONTEXT_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_ASSURANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY as CONTEXT_ASSURANCE_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as CONTEXT_ASSURANCE_SCHEMA_VERSION,
};
pub use gate::{gate, gate_document, GateFinding, GateOutcome, Policy};
pub use index::{PackStatus, PublicationEvent, RegistryError, RegistryIndex};
pub use pack::{
    BenchmarkPack, OracleDisagreement, PackBuilder, PackError, PackInstance, ParentRef,
    PostconditionEvidence, Provenance, RebuildAttestation, Resolution, ReviewFinding, ReviewRecord,
    YieldLedger, PACK_DIGEST_FIELD, PACK_SCHEMA_VERSION,
};
pub use promote::{promote, promote_with, Promotion, PromotionError};
pub use resource_assurance::{
    assure_resource_discovery, resource_discovery_assurance_manifest, FederatedResourceDescriptor,
    QualifiedFederatedResource, ResourceAssuranceDisposition, ResourceDiscoveryAssuranceError,
    ResourceDiscoveryAssuranceReceipt, ResourceDiscoveryAssuranceRequest, ResourceState,
    CONTRACT_VERSION as RESOURCE_DISCOVERY_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as RESOURCE_DISCOVERY_ASSURANCE_FEATURE_ID,
};
pub use tier::{
    assess, evaluate_tier, evaluate_tier_with, reassess, Requirement, RungAssessment,
    TierAssessment, TierPolicy, TierVerdict, TrustTier, UnmetRequirement,
};
