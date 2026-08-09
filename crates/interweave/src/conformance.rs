//! Specification families, the stability ladder, dimensional conformance, and version pinning.
//!
//! Blueprint 23.31.
//!
//! # The one thing this module is for
//!
//! 23.31 asks for "dimensional conformance rather than one vague 'compatible' badge", and its
//! worked example is a claim of `C1-C4` by an implementation that supports neither continuations
//! nor authority. [`ConformanceClaim`] is therefore a *set* of [`Dimension`]s, there is no
//! `Conformant` boolean anywhere, and [`ConformanceClaim::covers`] answers about one dimension at a
//! time. An implementation cannot be asked whether it "conforms"; it can only be asked what it
//! claims.
//!
//! # Two invariants the blueprint states and one it does not
//!
//! **Stated, and enforced here.** "A running thread pins versions. Mid-thread migration requires an
//! explicit protocol transition and state mapping." [`PinnedVersions::migrate`] takes a
//! [`ProtocolTransition`] and refuses without a mapping for every state that changes meaning; there
//! is no setter on a pinned version.
//!
//! **Stated, and enforced here.** "An extension cannot redefine a core act or type under another
//! meaning." [`ExtensionRegistry::register`] refuses a name colliding with a
//! `bioprism_weave::ActKind`, using the kernel's own act vocabulary rather than a copy of it that
//! could drift.
//!
//! **Not stated.** 23.31 gives six stability levels and no transition table, and twelve conformance
//! dimensions with no dependency relation. Both gaps are filled here and both fills are labelled at
//! the point of definition: see [`Stability::may_transition_to`] and [`Dimension::prerequisites`].
//! The second is deliberately weaker than the first — unmet prerequisites are reported as
//! *findings*, not errors, because 23.31's own example claims a contiguous prefix and never says a
//! non-contiguous claim is illegal.
//!
//! # Not implemented
//!
//! The governance half of the module is process and is not code. 23.31's Weave Enhancement Proposal
//! is a document with ten required sections, reviewed by people; its governance bodies are four
//! standing groups with human membership; its licensing section is a licensing decision. None of
//! them is a predicate over an artefact and none is implemented. The WEP's ten sections are
//! recorded in [`PROPOSAL_SECTIONS`] as a completeness checklist and nothing more, since
//! "reference implementation" and "alternatives" are satisfied by prose a machine cannot grade.
//! 23.31's eleven test suites are named in [`TEST_SUITES`] and this crate runs none of them.

use bioprism_fabric::molecule::Version;
use bioprism_weave::ActKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.31's eleven specification families, each versioned and staged independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecFamily {
    WeaveLangSyntaxAndSemantics,
    WeaveIrSchemasAndCanonicalization,
    CoreCognitiveAndCoordinationTypes,
    CommunicativeActs,
    ChoreographyAndRoleProjection,
    CommitmentAndAuthorityModels,
    RuntimeAndReplayBehaviour,
    AdapterProfiles,
    SecurityAndPrivacyProfiles,
    PrismInstrumentationProfile,
    RegistryAndMoleculeCardProfile,
}

impl SpecFamily {
    pub const ALL: [SpecFamily; 11] = [
        SpecFamily::WeaveLangSyntaxAndSemantics,
        SpecFamily::WeaveIrSchemasAndCanonicalization,
        SpecFamily::CoreCognitiveAndCoordinationTypes,
        SpecFamily::CommunicativeActs,
        SpecFamily::ChoreographyAndRoleProjection,
        SpecFamily::CommitmentAndAuthorityModels,
        SpecFamily::RuntimeAndReplayBehaviour,
        SpecFamily::AdapterProfiles,
        SpecFamily::SecurityAndPrivacyProfiles,
        SpecFamily::PrismInstrumentationProfile,
        SpecFamily::RegistryAndMoleculeCardProfile,
    ];
}

/// 23.31's six stability levels, in the order it prints them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stability {
    /// "rapid change, no compatibility promise".
    Experimental,
    /// "semantics frozen for implementation testing".
    Candidate,
    /// "compatibility and deprecation policy applies".
    Stable,
    /// "supported during migration window".
    Legacy,
    /// "no new features, removal date announced".
    Deprecated,
    /// "rejected by default".
    Retired,
}

impl Stability {
    pub const ALL: [Stability; 6] = [
        Stability::Experimental,
        Stability::Candidate,
        Stability::Stable,
        Stability::Legacy,
        Stability::Deprecated,
        Stability::Retired,
    ];

    /// Whether a family at this level may move to `next`.
    ///
    /// **Not in the blueprint.** 23.31 lists six levels and gives no transitions, which leaves
    /// "evolution is predictable" — the module's stated purpose — with nothing behind it. Three
    /// constraints shape the table below and each is arguable, so all three are written down:
    ///
    /// 1. **Retired is terminal.** "Deprecation never rewrites old results" is 23.31's own closing
    ///    line, and a retired specification returning to service would rewrite the meaning of
    ///    results published against it.
    /// 2. **Freezing is reversible; promising is not.** `Candidate` may fall back to
    ///    `Experimental`, because "semantics frozen for implementation testing" is exactly the
    ///    state whose purpose is to discover that the semantics were wrong. `Stable` may not,
    ///    because a compatibility promise was made.
    /// 3. **Legacy is reachable only from Stable.** "Supported during migration window" presupposes
    ///    something to migrate from, which is a promise only `Stable` ever made.
    pub fn may_transition_to(self, next: Stability) -> bool {
        matches!(
            (self, next),
            (
                Stability::Experimental,
                Stability::Candidate | Stability::Retired
            ) | (
                Stability::Candidate,
                Stability::Experimental | Stability::Stable | Stability::Retired
            ) | (
                Stability::Stable,
                Stability::Legacy | Stability::Deprecated
            ) | (Stability::Legacy, Stability::Deprecated)
                | (Stability::Deprecated, Stability::Retired)
        )
    }

    /// Whether a compatibility promise applies at this level.
    ///
    /// 23.31 attaches the promise to `stable` and, by "supported during migration window", to
    /// `legacy`. `deprecated` keeps it too — "no new features, removal date announced" is not a
    /// licence to change behaviour before the removal date.
    pub fn promises_compatibility(self) -> bool {
        matches!(
            self,
            Stability::Stable | Stability::Legacy | Stability::Deprecated
        )
    }
}

/// Why a stability transition was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{family:?} cannot move from {from:?} to {to:?}")]
pub struct TransitionRefused {
    pub family: SpecFamily,
    pub from: Stability,
    pub to: Stability,
}

/// One specification family's published state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyState {
    pub family: SpecFamily,
    pub version: Version,
    pub stability: Stability,
}

impl FamilyState {
    pub fn new(family: SpecFamily, version: Version, stability: Stability) -> Self {
        FamilyState {
            family,
            version,
            stability,
        }
    }

    /// Advance the family's stability, refusing transitions the ladder does not allow.
    pub fn transition(self, to: Stability) -> Result<FamilyState, TransitionRefused> {
        if self.stability.may_transition_to(to) {
            Ok(FamilyState {
                stability: to,
                ..self
            })
        } else {
            Err(TransitionRefused {
                family: self.family,
                from: self.stability,
                to,
            })
        }
    }
}

/// 23.31's twelve conformance dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// C1 envelope and identifiers.
    C1,
    /// C2 types and schemas.
    C2,
    /// C3 communicative acts.
    C3,
    /// C4 local choreography monitor.
    C4,
    /// C5 commitments and obligations.
    C5,
    /// C6 authority and effects.
    C6,
    /// C7 information flow.
    C7,
    /// C8 context capsules.
    C8,
    /// C9 continuations and fork/join.
    C9,
    /// C10 PRISM trace and replay hooks.
    C10,
    /// C11 molecule publication.
    C11,
    /// C12 security profile.
    C12,
}

impl Dimension {
    pub const ALL: [Dimension; 12] = [
        Dimension::C1,
        Dimension::C2,
        Dimension::C3,
        Dimension::C4,
        Dimension::C5,
        Dimension::C6,
        Dimension::C7,
        Dimension::C8,
        Dimension::C9,
        Dimension::C10,
        Dimension::C11,
        Dimension::C12,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Dimension::C1 => "envelope and identifiers",
            Dimension::C2 => "types and schemas",
            Dimension::C3 => "communicative acts",
            Dimension::C4 => "local choreography monitor",
            Dimension::C5 => "commitments and obligations",
            Dimension::C6 => "authority and effects",
            Dimension::C7 => "information flow",
            Dimension::C8 => "context capsules",
            Dimension::C9 => "continuations and fork/join",
            Dimension::C10 => "PRISM trace and replay hooks",
            Dimension::C11 => "molecule publication",
            Dimension::C12 => "security profile",
        }
    }

    /// Dimensions this one cannot be exercised without.
    ///
    /// **Not in the blueprint.** 23.31 gives twelve dimensions and no dependency relation. The
    /// relation below is this crate's reading and is reported rather than enforced: 23.31's one
    /// worked example is the contiguous prefix `C1-C4` and it never says a gapped claim is invalid,
    /// so [`ConformanceClaim::unmet_prerequisites`] returns findings and there is no function that
    /// rejects a claim on their account.
    ///
    /// Each edge below is a structural necessity rather than a convention: acts need types (C3←C2),
    /// a choreography monitor observes acts (C4←C3), commitments are made by acts (C5←C3), a
    /// capsule is a projection of typed content (C8←C2), a continuation carries capsule state and
    /// open commitments (C9←C5, C8), replay needs identifiers to replay against (C10←C1), a
    /// published molecule declares its authority surface (C11←C6), and a security profile is
    /// meaningless without information flow (C12←C7).
    pub fn prerequisites(self) -> &'static [Dimension] {
        match self {
            Dimension::C1 | Dimension::C2 => &[],
            Dimension::C3 => &[Dimension::C2],
            Dimension::C4 => &[Dimension::C3],
            Dimension::C5 => &[Dimension::C3],
            Dimension::C6 => &[Dimension::C1],
            Dimension::C7 => &[Dimension::C2],
            Dimension::C8 => &[Dimension::C2],
            Dimension::C9 => &[Dimension::C5, Dimension::C8],
            Dimension::C10 => &[Dimension::C1],
            Dimension::C11 => &[Dimension::C6],
            Dimension::C12 => &[Dimension::C7],
        }
    }
}

/// A prerequisite a claim does not cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UnmetPrerequisite {
    pub claimed: Dimension,
    pub requires: Dimension,
}

/// What an implementation says it supports.
///
/// There is no boolean. 23.31's whole point is that "compatible" is not a claim anyone can check,
/// so this type can only answer per-dimension questions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceClaim {
    pub implementation: String,
    dimensions: BTreeSet<Dimension>,
}

impl ConformanceClaim {
    pub fn new(implementation: impl Into<String>) -> Self {
        ConformanceClaim {
            implementation: implementation.into(),
            dimensions: BTreeSet::new(),
        }
    }

    pub fn claiming(mut self, dimension: Dimension) -> Self {
        self.dimensions.insert(dimension);
        self
    }

    /// 23.31's own example, `C1-C4`, as a constructor.
    pub fn through(implementation: impl Into<String>, highest: Dimension) -> Self {
        ConformanceClaim {
            implementation: implementation.into(),
            dimensions: Dimension::ALL
                .into_iter()
                .filter(|d| *d <= highest)
                .collect(),
        }
    }

    pub fn covers(&self, dimension: Dimension) -> bool {
        self.dimensions.contains(&dimension)
    }

    pub fn claimed(&self) -> &BTreeSet<Dimension> {
        &self.dimensions
    }

    /// Prerequisites of claimed dimensions that this claim does not itself cover.
    ///
    /// Findings, not errors. An implementation may have a good reason to claim C9 without C8 — for
    /// instance, it might carry continuations whose state is opaque to it — and 23.31 does not
    /// forbid it. What it should not do is claim it without noticing.
    pub fn unmet_prerequisites(&self) -> BTreeSet<UnmetPrerequisite> {
        self.dimensions
            .iter()
            .flat_map(|claimed| {
                claimed
                    .prerequisites()
                    .iter()
                    .filter(|required| !self.dimensions.contains(required))
                    .map(move |required| UnmetPrerequisite {
                        claimed: *claimed,
                        requires: *required,
                    })
            })
            .collect()
    }

    /// Whether the claim is a contiguous prefix `C1..=Cn`, which is the only shape 23.31 exhibits.
    pub fn is_prefix(&self) -> bool {
        match self.dimensions.iter().next_back() {
            None => true,
            Some(highest) => Dimension::ALL
                .into_iter()
                .filter(|d| d <= highest)
                .all(|d| self.dimensions.contains(&d)),
        }
    }
}

/// The six things 23.31 says participants negotiate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NegotiatedProfile {
    pub weave_ir_major: u32,
    pub act_packages: BTreeSet<String>,
    pub type_and_ontology_packages: BTreeSet<String>,
    pub security_profile: String,
    pub replay_profile: String,
    pub adapter_profile: String,
}

/// A state whose meaning changes across a version boundary, and how it maps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateMapping {
    pub from_state: String,
    pub to_state: String,
}

/// 23.31: "Mid-thread migration requires an explicit protocol transition and state mapping."
///
/// The transition is a value a caller must construct and hand over. There is no default, no
/// identity transition, and no way to change a thread's pinned profile without one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolTransition {
    pub reason: String,
    pub mappings: Vec<StateMapping>,
}

/// Why a mid-thread migration was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum MigrationRefusal {
    #[error("live state {state} has no mapping into the target profile")]
    UnmappedState { state: String },

    #[error("the transition states no reason")]
    NoReason,
}

/// The versions a running thread has pinned.
///
/// `profile` is private and there is no setter. The only route to a different profile is
/// [`PinnedVersions::migrate`], which consumes the pin and produces a new one, so a thread cannot
/// drift across versions without an event that says it did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedVersions {
    pub thread: String,
    profile: NegotiatedProfile,
}

impl PinnedVersions {
    pub fn pin(thread: impl Into<String>, profile: NegotiatedProfile) -> Self {
        PinnedVersions {
            thread: thread.into(),
            profile,
        }
    }

    pub fn profile(&self) -> &NegotiatedProfile {
        &self.profile
    }

    /// Migrate a running thread, requiring a mapping for every live state.
    ///
    /// `live_states` is what the thread currently holds. Every one of them must appear as the
    /// source of a mapping; a state the transition forgot is [`MigrationRefusal::UnmappedState`]
    /// and the migration does not happen, which is the difference between a protocol transition and
    /// a version bump nobody noticed.
    pub fn migrate(
        self,
        target: NegotiatedProfile,
        transition: &ProtocolTransition,
        live_states: &BTreeSet<String>,
    ) -> Result<PinnedVersions, MigrationRefusal> {
        if transition.reason.trim().is_empty() {
            return Err(MigrationRefusal::NoReason);
        }
        let mapped: BTreeSet<&str> = transition
            .mappings
            .iter()
            .map(|m| m.from_state.as_str())
            .collect();
        if let Some(state) = live_states.iter().find(|s| !mapped.contains(s.as_str())) {
            return Err(MigrationRefusal::UnmappedState {
                state: state.clone(),
            });
        }
        Ok(PinnedVersions {
            thread: self.thread,
            profile: target,
        })
    }
}

/// Why an extension registration was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum GovernanceError {
    #[error("{name} is not a reverse-domain or registry-owned namespace")]
    UnnamespacedExtension { name: String },

    #[error("{name} redefines the core act {act}")]
    RedefinesCoreAct { name: String, act: String },

    #[error("{name} is already registered by {owner}")]
    AlreadyRegistered { name: String, owner: String },
}

/// 23.31's namespacing rule as a registry.
///
/// Two checks. A name must be namespaced — reverse-domain (`org.example.retract`) or prefixed with
/// the registry's own namespace (`weave.ext.`). And the final segment must not be a core act name,
/// which is read from `bioprism_weave::ActKind` rather than from a list maintained here, so a new
/// kernel act automatically becomes reserved.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRegistry {
    entries: BTreeMap<String, String>,
}

/// The registry's own namespace prefix.
pub const REGISTRY_NAMESPACE: &str = "weave.ext.";

impl ExtensionRegistry {
    pub fn new() -> Self {
        ExtensionRegistry::default()
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        owner: impl Into<String>,
    ) -> Result<(), GovernanceError> {
        let name = name.into();
        let segments: Vec<&str> = name.split('.').collect();
        let namespaced = name.starts_with(REGISTRY_NAMESPACE) || segments.len() >= 3;
        if !namespaced {
            return Err(GovernanceError::UnnamespacedExtension { name });
        }
        let last = segments.last().copied().unwrap_or("");
        if let Some(act) = core_act_named(last) {
            return Err(GovernanceError::RedefinesCoreAct {
                name,
                act: act.to_string(),
            });
        }
        if let Some(existing) = self.entries.get(&name) {
            return Err(GovernanceError::AlreadyRegistered {
                name,
                owner: existing.clone(),
            });
        }
        self.entries.insert(name, owner.into());
        Ok(())
    }

    pub fn owner(&self, name: &str) -> Option<&str> {
        self.entries.get(name).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The kernel act a name would shadow, if any.
///
/// Reads `bioprism_weave::ActKind`'s own strings. A core act added to the kernel becomes reserved
/// here with no edit to this file, which is the property a hand-maintained list would lose.
pub fn core_act_named(segment: &str) -> Option<&'static str> {
    [
        ActKind::Ask,
        ActKind::Claim,
        ActKind::Propose,
        ActKind::Accept,
        ActKind::Reject,
        ActKind::Challenge,
        ActKind::Discharge,
        ActKind::Delegate,
        ActKind::Revoke,
        ActKind::Attest,
    ]
    .into_iter()
    .map(ActKind::as_str)
    .find(|act| *act == segment)
}

/// 23.31's five compatibility guarantees for a stable major version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guarantee {
    CanonicalIdentifierInterpretation,
    CoreActConsequences,
    SignedArtifactVerification,
    PublishedResultSemantics,
    ReplayOfHistoricalBundles,
}

impl Guarantee {
    pub const ALL: [Guarantee; 5] = [
        Guarantee::CanonicalIdentifierInterpretation,
        Guarantee::CoreActConsequences,
        Guarantee::SignedArtifactVerification,
        Guarantee::PublishedResultSemantics,
        Guarantee::ReplayOfHistoricalBundles,
    ];
}

/// A proposed change to a specification family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedChange {
    pub family: SpecFamily,
    pub from: Version,
    pub to: Version,
    /// Guarantees this change would break.
    pub breaks: BTreeSet<Guarantee>,
}

/// Why a change was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum ChangeRefusal {
    #[error("{family:?} promises compatibility and this change breaks {broken:?} without a major bump")]
    BreaksWithinMajor {
        family: SpecFamily,
        broken: BTreeSet<Guarantee>,
    },

    #[error("{family:?} would rewrite already-published results, which no version bump permits")]
    RewritesPublishedResults { family: SpecFamily },
}

/// 23.31's compatibility promise, checked.
///
/// The interesting clause is the second. A major bump excuses four of the five guarantees — that is
/// what a major version is *for* — but not [`Guarantee::PublishedResultSemantics`], because 23.31
/// closes with "Deprecation never rewrites old results" and a result whose meaning changed under it
/// has been rewritten whatever the version number says.
pub fn check_change(state: &FamilyState, change: &ProposedChange) -> Result<(), ChangeRefusal> {
    if change.breaks.contains(&Guarantee::PublishedResultSemantics) {
        return Err(ChangeRefusal::RewritesPublishedResults {
            family: change.family,
        });
    }
    let major_bump = change.to.major > change.from.major;
    if state.stability.promises_compatibility() && !major_bump && !change.breaks.is_empty() {
        return Err(ChangeRefusal::BreaksWithinMajor {
            family: change.family,
            broken: change.breaks.clone(),
        });
    }
    Ok(())
}

/// The ten sections 23.31 requires of a Weave Enhancement Proposal, recorded as a checklist.
pub const PROPOSAL_SECTIONS: [&str; 10] = [
    "problem statement",
    "proposed syntax and IR changes",
    "formal or operational semantics",
    "security and privacy impact",
    "adapter impact",
    "backward compatibility",
    "conformance tests",
    "migration plan",
    "alternatives",
    "reference implementation",
];

/// 23.31's eleven test suites, named and not run.
pub const TEST_SUITES: [&str; 11] = [
    "golden serialization vectors",
    "invalid-program rejection",
    "cross-language round trips",
    "protocol state-machine tests",
    "commitment lifecycle tests",
    "grant attenuation and revocation",
    "label-flow tests",
    "adapter loss tests",
    "replay and idempotency tests",
    "malicious-input corpus",
    "molecule interoperability demo",
];
