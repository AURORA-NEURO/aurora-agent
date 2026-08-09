//! Observed-data world authoring.
//!
//! Blueprint 35.02, whose purpose sentence is the whole design: "convert real datasets and
//! workflows into time-valid worlds **without pretending unavailable latent truth is known**."
//!
//! Authoring a world from real data is largely a description of what people do — a rights lawyer
//! reads a data use agreement, a domain expert reconstructs what a decision actually was, a
//! reviewer signs off. None of that is code. What *is* code is the shape of the artifact those
//! people produce, and four properties of it that a library can check without asking anyone:
//!
//! 1. The recorded observation instant of every artifact is at or before the decision instant it
//!    informs. A world assembled from evidence that did not exist yet is not time-valid, however
//!    real its parts.
//! 2. A derived form names, and the world preserves, the native artifact it came from.
//! 3. Every latent quantity is either [`LatentTruth::Established`] with a stated basis, or
//!    [`LatentTruth::Unavailable`] with a stated reason. There is no third state and no default.
//! 4. Every `Unavailable` latent appears on the expert limitations card. A world may be honest in
//!    its internals and still ship a card that omits the gap; the check is that the two agree.
//!
//! ## Why `LatentTruth` has no third variant
//!
//! `AGENTS.md` is categorical that unmeasured is not zero, and `bioprism-atlas` enforces it for
//! capability scores through a gated `Measurement` constructor. This is the same rule at a
//! different object: the latent state of a real specimen. The enum has exactly two variants, both
//! carry a mandatory string — a value needs a basis, an absence needs a reason — and there is no
//! `Default`, no `unwrap_or`, and no accessor that yields a value for the `Unavailable` case.
//! [`LatentTruth::value`] returns `Option<&str>`, so the caller who wants to pretend has to write
//! the pretence down.
//!
//! ## Release mode is inherited downward, never widened
//!
//! Section 35's scale constraints say "descendants inherit access and privacy restrictions". Read
//! as a predicate over a lineage chain, that is monotonicity: a descendant's release mode may equal
//! or exceed its parent's restriction and may never fall below it. [`check_release_lineage`] walks
//! the chain and refuses a widening.
//!
//! The *grant* side of access control — who may hold a key, under what contract, expiring when —
//! is `bioprism-stewardship`'s access module and is not restated here. This is only the rule that
//! the release mode a generated descendant carries is bounded by the world it came from.
//!
//! ## What this module does not do
//!
//! No connector, no ingest, no PHI detection, no de-identification. The rights and provenance
//! review is represented by its *outcome* — [`RightsReview`] — because a library cannot perform a
//! review and a type that claimed to would be worse than no type. Oracle mesh selection, which
//! 35.02 lists among its required components, belongs to `bioprism-oracle` and `bioprism-oraclex`;
//! nothing here selects an oracle.
//!
//! Of the eight release gates `bioprism_scale::QualityGate` enumerates, this module can evaluate
//! exactly one: rights permit the selected release mode. [`AuthoringReport::contribute_to`] records
//! that one and deliberately leaves the other seven unrecorded, so `ReleaseAudit::finish` refuses
//! until something or someone else evaluates them.

use crate::error::AuthoringError;
use bioprism_ids::ContentHash;
use bioprism_scale::audit::ReleaseAudit;
use bioprism_scale::QualityGate;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// How broadly a world may be released.
///
/// The blueprint names "the selected release mode" without enumerating the modes, so these four are
/// **illustrative**: what the type carries that the blueprint does state is the *order*. `Open` is
/// the broadest and `Enclave` the narrowest, and the derived `Ord` is the inheritance rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseMode {
    /// Public download, no registration.
    Open,
    /// Download after identifying yourself.
    Registered,
    /// Download under an executed agreement.
    Controlled,
    /// No download; computation goes to the data.
    Enclave,
}

impl ReleaseMode {
    pub const ALL: [ReleaseMode; 4] = [
        ReleaseMode::Open,
        ReleaseMode::Registered,
        ReleaseMode::Controlled,
        ReleaseMode::Enclave,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ReleaseMode::Open => "open",
            ReleaseMode::Registered => "registered",
            ReleaseMode::Controlled => "controlled",
            ReleaseMode::Enclave => "enclave",
        }
    }

    /// Whether `self` exposes the data to more people than `other`.
    pub fn is_broader_than(self, other: ReleaseMode) -> bool {
        self < other
    }
}

/// The outcome of a data-use and provenance review, which a library cannot perform.
///
/// `permitted` is what the reviewer concluded the licence and access policy allow; `selected` is
/// what the release actually asks for. The gate is `selected` no broader than `permitted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RightsReview {
    pub reviewer: String,
    pub permitted: ReleaseMode,
    pub selected: ReleaseMode,
    /// The instrument the conclusion rests on — an agreement, a consent form, a policy id.
    pub basis: String,
}

impl RightsReview {
    pub fn new(
        reviewer: impl Into<String>,
        permitted: ReleaseMode,
        selected: ReleaseMode,
        basis: impl Into<String>,
    ) -> Self {
        RightsReview {
            reviewer: reviewer.into(),
            permitted,
            selected,
            basis: basis.into(),
        }
    }

    pub fn permits_selection(&self) -> bool {
        !self.selected.is_broader_than(self.permitted)
    }
}

/// What is known about a latent quantity, in the only two states that exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "latent_truth", rename_all = "snake_case")]
pub enum LatentTruth {
    /// A value, and how it came to be known. Both are required.
    Established { value: String, basis: String },
    /// Nobody can know this here, and why. Not a value of zero, not an empty string.
    Unavailable { reason: String },
}

impl LatentTruth {
    pub fn established(value: impl Into<String>, basis: impl Into<String>) -> Self {
        LatentTruth::Established {
            value: value.into(),
            basis: basis.into(),
        }
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        LatentTruth::Unavailable {
            reason: reason.into(),
        }
    }

    /// The value, when there is one. There is no variant of this that returns a placeholder.
    pub fn value(&self) -> Option<&str> {
        match self {
            LatentTruth::Established { value, .. } => Some(value.as_str()),
            LatentTruth::Unavailable { .. } => None,
        }
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self, LatentTruth::Unavailable { .. })
    }
}

/// A latent question the world asks, and what is actually known about its answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatentClaim {
    pub question: String,
    pub truth: LatentTruth,
}

impl LatentClaim {
    pub fn new(question: impl Into<String>, truth: LatentTruth) -> Self {
        LatentClaim {
            question: question.into(),
            truth,
        }
    }
}

/// Whether an artifact is the thing that came off the instrument, or something computed from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "form", rename_all = "snake_case")]
pub enum ArtifactForm {
    /// As produced. 35.02's "native artifact preservation" is a statement about these.
    Native,
    /// Computed from a native artifact, which it must name.
    Derived { from_native: String },
}

/// One preserved artifact and when it was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: String,
    pub form: ArtifactForm,
    /// Recorded by the producer. Nothing here reads a clock; this is data.
    pub observed_at: Timestamp,
    pub digest: ContentHash,
}

impl ArtifactRecord {
    pub fn native(id: impl Into<String>, observed_at: Timestamp, digest: ContentHash) -> Self {
        ArtifactRecord {
            id: id.into(),
            form: ArtifactForm::Native,
            observed_at,
            digest,
        }
    }

    pub fn derived(
        id: impl Into<String>,
        from_native: impl Into<String>,
        observed_at: Timestamp,
        digest: ContentHash,
    ) -> Self {
        ArtifactRecord {
            id: id.into(),
            form: ArtifactForm::Derived {
                from_native: from_native.into(),
            },
            observed_at,
            digest,
        }
    }
}

/// The decision the world was authored around, reconstructed from a real workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconstructedDecision {
    pub id: String,
    pub question: String,
    /// The instant the decision was taken. Evidence observed after it was not available to it.
    pub decided_at: Timestamp,
}

/// One entry on 35.02's expert limitations card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limitation {
    pub question: String,
    pub why_unanswerable: String,
}

/// What the authoring expert says this world cannot settle.
///
/// The card is the world's honest-labelling surface, and [`AuthoredWorld::check`] cross-checks it
/// against the world's own latents rather than trusting it: a card is written by hand and the
/// interesting failure is the gap it forgets to mention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LimitationsCard {
    pub author: String,
    pub entries: Vec<Limitation>,
}

impl LimitationsCard {
    pub fn new(author: impl Into<String>) -> Self {
        LimitationsCard {
            author: author.into(),
            entries: Vec::new(),
        }
    }

    pub fn stating(
        mut self,
        question: impl Into<String>,
        why_unanswerable: impl Into<String>,
    ) -> Self {
        self.entries.push(Limitation {
            question: question.into(),
            why_unanswerable: why_unanswerable.into(),
        });
        self
    }

    pub fn covers(&self, question: &str) -> bool {
        self.entries.iter().any(|entry| entry.question == question)
    }
}

/// A world authored from observed data, as the checks in this module see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoredWorld {
    pub id: String,
    /// The world this one was derived from, if any. Release mode is inherited along this edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<String>,
    pub rights: RightsReview,
    pub decision: ReconstructedDecision,
    pub artifacts: Vec<ArtifactRecord>,
    pub latents: Vec<LatentClaim>,
    pub limitations: LimitationsCard,
}

/// One checkable property of an authored world, and whether it held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringFinding {
    pub property: AuthoringProperty,
    pub held: bool,
    pub detail: String,
}

/// The four properties this module checks. Named so a failing report says which one failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoringProperty {
    /// No artifact's recorded observation instant is after the decision instant.
    TimeValid,
    /// Every derived form names a native artifact the world preserves.
    NativeArtifactPreserved,
    /// Every latent is `Established` with a basis or `Unavailable` with a reason.
    LatentTruthLabelled,
    /// Every `Unavailable` latent appears on the expert limitations card.
    LimitationsCardComplete,
}

impl AuthoringProperty {
    pub const ALL: [AuthoringProperty; 4] = [
        AuthoringProperty::TimeValid,
        AuthoringProperty::NativeArtifactPreserved,
        AuthoringProperty::LatentTruthLabelled,
        AuthoringProperty::LimitationsCardComplete,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AuthoringProperty::TimeValid => "time_valid",
            AuthoringProperty::NativeArtifactPreserved => "native_artifact_preserved",
            AuthoringProperty::LatentTruthLabelled => "latent_truth_labelled",
            AuthoringProperty::LimitationsCardComplete => "limitations_card_complete",
        }
    }
}

/// What [`AuthoredWorld::check`] found, plus the one release gate it is entitled to speak to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoringReport {
    pub world: String,
    pub findings: Vec<AuthoringFinding>,
    /// Latents the world itself says nobody can answer. Not failures — the point of the card.
    pub unavailable_latents: Vec<String>,
    pub rights_permit_selection: bool,
    /// The gates in `bioprism_scale::QualityGate` this crate cannot evaluate, named so a reader of
    /// a passing report does not read it as a release decision.
    pub gates_left_to_others: Vec<String>,
}

/// The seven release gates this module has no standing to record.
///
/// Two need an environment (a clean replay, a second site), one needs a domain reviewer, and four
/// are owned by other crates — mutation relations, duplicate and contamination scans, oracle
/// coverage, and parent-clustered statistics. Recording any of them here would be a library
/// asserting something it did not observe.
const GATES_LEFT_TO_OTHERS: [&str; 7] = [
    "clean_replay",
    "meaningful_boundaries",
    "non_llm_oracle",
    "executable_relations",
    "duplicate_and_contamination_scans",
    "parent_clustered_statistics",
    "independent_reproduction",
];

impl AuthoringReport {
    /// Whether every property this module can check held.
    ///
    /// Deliberately *not* named `is_releasable`. Four properties out of a world's many, and one
    /// release gate out of eight, is not a release decision; [`Self::gates_left_to_others`] names
    /// what is still missing.
    pub fn all_checked_properties_held(&self) -> bool {
        self.findings.iter().all(|finding| finding.held)
    }

    pub fn failing(&self) -> Vec<&AuthoringFinding> {
        self.findings.iter().filter(|f| !f.held).collect()
    }

    /// Records the one gate an authoring check is entitled to speak to.
    ///
    /// The other seven are left unrecorded on purpose, so `ReleaseAudit::finish` returns its
    /// unevaluated-gate error rather than a release-ready report built from one crate's opinion.
    pub fn contribute_to(&self, audit: &mut ReleaseAudit) {
        audit.record(
            QualityGate::RightsPermitReleaseMode,
            self.rights_permit_selection,
            format!(
                "world {}: rights review {} the selected release mode",
                self.world,
                if self.rights_permit_selection {
                    "permits"
                } else {
                    "does not permit"
                }
            ),
        );
    }
}

impl AuthoredWorld {
    /// Checks the four properties, or fails structurally if the world is not well formed.
    ///
    /// A duplicate artifact id or a duplicate latent question is a structural error rather than a
    /// finding: the checks below would silently examine only one of the two records, and a check
    /// that inspects half its input and reports success is the failure mode this whole crate is
    /// about.
    pub fn check(&self) -> Result<AuthoringReport, AuthoringError> {
        let mut ids: BTreeSet<&str> = BTreeSet::new();
        for artifact in &self.artifacts {
            if !ids.insert(artifact.id.as_str()) {
                return Err(AuthoringError::DuplicateArtifact {
                    world: self.id.clone(),
                    artifact: artifact.id.clone(),
                });
            }
        }
        let natives: BTreeSet<&str> = self
            .artifacts
            .iter()
            .filter(|artifact| artifact.form == ArtifactForm::Native)
            .map(|artifact| artifact.id.as_str())
            .collect();

        let mut questions: BTreeSet<&str> = BTreeSet::new();
        for latent in &self.latents {
            if !questions.insert(latent.question.as_str()) {
                return Err(AuthoringError::DuplicateLatentQuestion {
                    world: self.id.clone(),
                    question: latent.question.clone(),
                });
            }
        }

        let late: Vec<&str> = self
            .artifacts
            .iter()
            .filter(|artifact| artifact.observed_at > self.decision.decided_at)
            .map(|artifact| artifact.id.as_str())
            .collect();

        let mut orphan_derived: Vec<String> = Vec::new();
        for artifact in &self.artifacts {
            if let ArtifactForm::Derived { from_native } = &artifact.form {
                if !natives.contains(from_native.as_str()) {
                    orphan_derived.push(format!("{} -> {}", artifact.id, from_native));
                }
            }
        }

        let unavailable: Vec<String> = self
            .latents
            .iter()
            .filter(|latent| latent.truth.is_unavailable())
            .map(|latent| latent.question.clone())
            .collect();
        let undisclosed: Vec<&String> = unavailable
            .iter()
            .filter(|question| !self.limitations.covers(question))
            .collect();

        let findings = vec![
            AuthoringFinding {
                property: AuthoringProperty::TimeValid,
                held: late.is_empty(),
                detail: if late.is_empty() {
                    format!(
                        "all {} artifacts were observed at or before the decision instant",
                        self.artifacts.len()
                    )
                } else {
                    format!(
                        "{} artifact(s) observed after the decision instant: {}",
                        late.len(),
                        late.join(", ")
                    )
                },
            },
            AuthoringFinding {
                property: AuthoringProperty::NativeArtifactPreserved,
                held: orphan_derived.is_empty(),
                detail: if orphan_derived.is_empty() {
                    format!(
                        "{} native and {} derived artifacts; every derived form names a native the \
                         world preserves",
                        natives.len(),
                        self.artifacts.len() - natives.len()
                    )
                } else {
                    format!(
                        "{} derived form(s) name a native this world does not preserve: {}",
                        orphan_derived.len(),
                        orphan_derived.join(", ")
                    )
                },
            },
            AuthoringFinding {
                property: AuthoringProperty::LatentTruthLabelled,
                held: true,
                detail: format!(
                    "{} latent(s), {} established with a basis and {} recorded unavailable with a \
                     reason; the type admits no third state",
                    self.latents.len(),
                    self.latents.len() - unavailable.len(),
                    unavailable.len()
                ),
            },
            AuthoringFinding {
                property: AuthoringProperty::LimitationsCardComplete,
                held: undisclosed.is_empty(),
                detail: if undisclosed.is_empty() {
                    format!(
                        "the card by {} states all {} unavailable latents",
                        self.limitations.author,
                        unavailable.len()
                    )
                } else {
                    format!(
                        "{} unavailable latent(s) absent from the card: {}",
                        undisclosed.len(),
                        undisclosed
                            .iter()
                            .map(|question| question.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
            },
        ];

        Ok(AuthoringReport {
            world: self.id.clone(),
            findings,
            unavailable_latents: unavailable,
            rights_permit_selection: self.rights.permits_selection(),
            gates_left_to_others: GATES_LEFT_TO_OTHERS
                .iter()
                .map(|gate| (*gate).to_string())
                .collect(),
        })
    }
}

/// Walks a lineage and refuses any descendant whose release mode is broader than its parent's.
///
/// Section 35's scale constraint reads "descendants inherit access and privacy restrictions".
///
/// Each world is compared against *every* ancestor rather than only its parent. Because
/// [`ReleaseMode`] is totally ordered, checking each edge would in fact suffice — the transitive
/// case cannot arise on its own. The walk is here for the two things an edge check does not give:
/// a world whose parent is absent from the set is [`AuthoringError::UnknownParent`] rather than
/// silently exempt, and a lineage cycle is [`AuthoringError::LineageCycle`] rather than a hang.
/// Both are the failure modes of a set assembled from separate exports.
pub fn check_release_lineage(worlds: &[AuthoredWorld]) -> Result<(), AuthoringError> {
    let by_id: BTreeMap<&str, &AuthoredWorld> = worlds
        .iter()
        .map(|world| (world.id.as_str(), world))
        .collect();

    for world in worlds {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        seen.insert(world.id.as_str());
        let mut cursor = world;
        while let Some(parent_id) = cursor.derived_from.as_deref() {
            let parent =
                by_id
                    .get(parent_id)
                    .copied()
                    .ok_or_else(|| AuthoringError::UnknownParent {
                        world: cursor.id.clone(),
                        parent: parent_id.to_string(),
                    })?;
            if !seen.insert(parent.id.as_str()) {
                return Err(AuthoringError::LineageCycle(world.id.clone()));
            }
            if world
                .rights
                .selected
                .is_broader_than(parent.rights.selected)
            {
                return Err(AuthoringError::ReleaseModeWidened {
                    world: world.id.clone(),
                    parent: parent.id.clone(),
                    parent_mode: parent.rights.selected.as_str(),
                    requested: world.rights.selected.as_str(),
                });
            }
            cursor = parent;
        }
    }
    Ok(())
}
