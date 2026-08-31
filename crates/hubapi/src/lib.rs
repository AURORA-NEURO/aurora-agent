//! The distribution surface of the registry: discovery, resolution, mirroring and federation.
//!
//! Implements the part of blueprint section 10 (Registry and Hub) that the two crates below it do
//! not: 10.04 (Packaging, Publishing and Resolution — the resolution half), 10.08 (Versioning,
//! License and Provenance — the versioning half), 10.10 and 10.11 (Catalog / Search, Discovery and
//! Recommendation), 10.18 and 10.19 (Private Registries, Federation and Data Residency) and the
//! revocation half of 10.20 (Supply Chain Security and Revocation).
//!
//! # What the other two crates already discharge
//!
//! This crate builds above `bioprism-registry` and `bioprism-hub` and duplicates neither.
//!
//! | Blueprint module | Owned by |
//! |---|---|
//! | 10.02 registry architecture, 10.05 publication lifecycle | `bioprism-registry` (`index`, `gate`) |
//! | 10.03 entity and metadata model, the packaging half of 10.04 | `bioprism-registry` (`pack`) |
//! | 10.07 trust tiers and review, 10.09 result ingestion and attestation | `bioprism-registry` (`tier`, `promote`) |
//! | 10.06 submission, review and maintenance | `bioprism-hub` (`submission`, `moderation`) |
//! | 10.12 benchmark cards and disclosure | `bioprism-hub` (`card`, `disclosure`) |
//! | 10.16, 10.17 comparison and leaderboard policy | `bioprism-hub` (`leaderboard`) |
//! | 10.22 moderation and content policy | `bioprism-hub` (`moderation`) |
//! | the licence half of 10.08 | `bioprism-hub` (`attribution`) |
//! | 10.13–10.15, 10.21 dashboards, explorers, atlas, web UX | nobody — they are renderings |
//!
//! What was left is the *distribution* surface, and it is what this crate is: name resolution with
//! an explicit authority, dependency resolution that reports what it could not do, mirror and
//! offline semantics, yank and deprecation propagation, and trust across a federation.
//!
//! # Offline is the normal case
//!
//! This workspace builds with `--offline` against pinned dependencies because the registry it
//! would otherwise talk to is unreachable. The same is true of the system section 10 describes:
//! 10.19 exists for air-gapped sites, and 10.04's offline paragraph is one of the few in section
//! 10 with content rather than aspiration. So offline is not a degraded mode here. It is the mode,
//! and the rule the crate is built around is this:
//!
//! > A resolution that succeeded against a mirror is **indistinguishable in result** from one
//! > against the origin, and **distinguishable in provenance**.
//!
//! Indistinguishable in result: [`Resolution::agrees_with`] compares name, version and digest, and
//! must hold across registries — a mirror that answers differently is not stale, it is
//! [`MirrorError::Divergent`], because a version binding is immutable ([`catalog`]).
//! Distinguishable in provenance: every [`Resolution`] carries which registry answered
//! ([`Resolution::answered_by`]), whether that registry was authoritative for the namespace
//! ([`Resolution::is_authoritative`]), how current its copy is ([`mirror::Freshness`]) and under
//! what policy that was accepted. There is no way to obtain the first without the second.
//!
//! The corollary, stated as a prohibition because it is the failure this crate exists to prevent:
//! *"I could not reach the origin"* must never render as *"this is the current version"*. Hence
//! [`mirror::Freshness`] has no `is_current`, [`mirror::Freshness::Undetermined`] is a real
//! outcome rather than an optimistic default, and a stale mirror produces a different value from a
//! fresh one rather than the same value with a flag set.
//!
//! # Five things the types refuse
//!
//! | Refused | Where |
//! |---|---|
//! | a registry answering for a namespace it neither owns nor carries | [`registry::AuthorityError::OutsideAuthority`] — an error, not a caveat |
//! | an unsatisfiable dependency set reported without naming the pair that collides | [`deps`], which always names two requirements |
//! | a mirror's silence read as currency | [`mirror::Freshness::Undetermined`] |
//! | a yank rewriting a build that was already correct | [`lifecycle::Intent`] |
//! | trust travelling from one registry to another by being copied | [`federation`], where the transitive claim is unconstructible |
//!
//! # What is deliberately absent
//!
//! - **No network, no transport, no storage.** Nothing here fetches, serves, replicates, signs or
//!   persists. A [`catalog::Catalog`] is a value somebody built; a [`mirror::Replication`]
//!   describes a copy somebody else made. Every type is named for the *decision* it encodes rather
//!   than for an action it does not perform.
//! - **No keys or signatures.** 10.18 wants signed delegations and key rotation, 10.20 wants
//!   transparency logs. A [`registry::RegistryId`] is a name and a delegation is a statement in a
//!   document. What is supplied is the predicate a verified delegation would be checked against.
//! - **No ranking by popularity, and no embeddings.** 10.11 requires that recommendation "avoid
//!   popularity-only ranking" and explain itself. [`mod@search`] has no popularity input at all —
//!   there is no telemetry in this crate, and a popularity field would be an unverifiable number.
//! - **No clock and no randomness.** Staleness, deprecation and yanks all advance on
//!   operator-supplied epochs (`bioprism_hub::Epoch`), for the reason `bioprism-governance` gives:
//!   a lifecycle that moved with the machine's date would not be reproducible.
//! - **No scanning, no residency enforcement, no redaction.** 10.19's residency rules and 10.20's
//!   vulnerability scanning are policy engines fed by results this crate does not compute.
//!
//! # Where section 10 assumes a mechanism it never specifies
//!
//! Recorded here because the gaps are load-bearing, and every one of them was filled by a decision
//! stated in a module doc rather than by a guess:
//!
//! - 10.04 says "aliases resolve to exact manifests" without defining a version, an alias, or a
//!   requirement syntax. [`name`] defines all three, and explains why a requirement is an interval.
//! - 10.04 says "dependencies are recursively pinned" without saying what happens when two
//!   requirements conflict. [`deps`] answers, and can always name the offending pair.
//! - 10.04's offline paragraph exports a bundle without saying what a resolution against one may
//!   claim. [`mirror`] answers.
//! - 10.04 lists a `withdrawn` channel and 10.20 wants revocation to propagate; neither says what
//!   an existing dependent does. [`lifecycle`] separates a yank from a withdrawal on exactly that
//!   question.
//! - 10.18 says "federation proves origin, not universal trust" without saying how a consumer
//!   decides whether the answering registry had standing. [`registry`] answers.
//! - 10.18 and 10.19 both assume trust policies are "registry-specific" without preventing a tier
//!   from being copied across the boundary. [`federation`] prevents it.
//! - 10.11 requires ranking to be explainable and non-popularity-driven without naming a single
//!   ranking signal that is checkable offline. [`mod@search`] uses only what is in the catalog.
//!
//! # Blueprint boilerplate in section 10
//!
//! Measured, not assumed. Method: each of the 22 module files is reduced to its non-blank lines,
//! trimmed; a line is *shared* when its exact text occurs in at least 11 of the 22 files; the
//! figure is shared lines over all lines, counted with multiplicity across the corpus (1493 lines
//! total). **63.6%.** Raising the threshold to all 22 files gives 51.6% and lowering it to 2 gives
//! 64.3%, so the figure is insensitive to where the threshold sits between those — section 10 is
//! sharply bimodal, with `Invariants`, `Testing strategy` and `Implementation sequence` identical
//! in every file (100% shared) and `Failure modes` 95.9% shared. The distinctive content is almost
//! entirely in two sections: `Responsibilities` (16.7% shared) and `Detailed design` (8.3%).

pub mod catalog;
pub mod context_compilation_assurance;
pub mod experiment_design_assurance;
pub mod deps;
pub mod federation;
pub mod interpretation_assurance;
pub mod lifecycle;
pub mod mirror;
pub mod name;
pub mod quality_assurance;
pub mod registry;
pub mod resolve;
pub mod search;

pub use catalog::{Catalog, CatalogError, Dependency, PackRelease};
pub use context_compilation_assurance::{
    assure_context_compilation, context_compilation_assurance_manifest, ContextAssuranceError,
    ContextAssuranceReport, ContextFact, DecisionQuery,
    CONTRACT_VERSION as CONTEXT_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as CONTEXT_ASSURANCE_FEATURE_ID,
    INPUT_SCHEMA as CONTEXT_ASSURANCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as CONTEXT_ASSURANCE_OUTPUT_SCHEMA,
};
pub use experiment_design_assurance::{
    assure_federated_experiment_design, experiment_design_assurance_manifest,
    ExecutableExperimentDesign7, ExecutableExperimentDesignArtifact7,
    ExperimentDesignAssuranceError, ExperimentDesignCandidate4, ExperimentDesignPeer4,
    ExperimentObjective4,
    CONTRACT_VERSION as EXPERIMENT_DESIGN_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as EXPERIMENT_DESIGN_ASSURANCE_FEATURE_ID,
    INPUT_SCHEMA as EXPERIMENT_DESIGN_ASSURANCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as EXPERIMENT_DESIGN_ASSURANCE_OUTPUT_SCHEMA,
};
pub use deps::{
    resolve_dependencies, Collision, DependencyError, Lock, Locked, Requirement, Source,
};
pub use federation::{
    adopt, Adoption, AdoptionPolicy, Attestation, Basis, FederationError, Subject, TrustStanding,
};
pub use interpretation_assurance::{
    assure_multimodal_interpretations, capability_manifest as interpretation_assurance_manifest,
    InterpretationAssuranceError, InterpretationDisposition, InterpretationState,
    MultimodalInterpretationAssuranceReceipt, MultimodalInterpretationAssuranceRequest,
    MultimodalInterpretationCandidate,
    CONTRACT_VERSION as INTERPRETATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as INTERPRETATION_ASSURANCE_FEATURE_ID,
};
pub use lifecycle::{Admission, Availability, Intent, LifecycleError, Note, PackLifecycle};
pub use mirror::{Freshness, FreshnessPolicy, MirrorError, Replication, StalenessBound};
pub use name::{Bounds, NameError, Namespace, PackName, Version, VersionReq};
pub use quality_assurance::{
    assure as assure_quality, capability_manifest as quality_assurance_manifest, MetricState,
    QualityAssuranceError, QualityDisposition, QualityMetric, QualityVerdict, ResearchObject,
    CONTRACT_VERSION as QUALITY_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as QUALITY_ASSURANCE_FEATURE_ID,
};
pub use registry::{Authority, AuthorityError, Federation, NameAuthority, RegistryId};
pub use resolve::{resolve, resolve_in, Provenance, Request, Resolution, ResolveError, Resolved};
pub use search::{search, Excluded, Facet, Match, Query, Results, SearchError, Why};
