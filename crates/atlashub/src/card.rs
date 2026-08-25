//! World cards — blueprint 34.03.
//!
//! 34.03's purpose is one sentence: *"Publish enough information to assess scientific scope,
//! provenance, access, and validity before running a world."* The word doing the work is
//! **before**. A card is read by someone deciding whether to spend a week on a world, and the
//! decision they are actually making is not "is this world good" but "can this world answer my
//! question at all". So the card's most important field is the one 34.03's capability list buries
//! at position eight — *known limitations* — and the design question is whether an author is
//! allowed to leave it thin.
//!
//! # The one thing an author does not get to write
//!
//! They do not. [`Unsuitability`] is checked against the world's construction rung, not accepted on
//! the author's word.
//!
//! `bioprism-worldfactory` established the rungs (27.02 observed, 27.03 semi-synthetic, 27.04
//! mechanistic), the fact that they support different kinds of claim, and the rule that a
//! derivation carries its parents' rungs through. [`ProvenanceRung::supports`] is that table.
//! [`WorldCardDraft::publish`] then requires the card to name an [`Unsuitability`] for **every**
//! claim kind its deepest rung cannot support, and refuses with
//! [`CardError::UndeclaredUnsuitability`] naming the specific omission otherwise.
//!
//! The shape is `bioprism-scale`'s, applied to a different quantity: there, a nominal instance
//! count has no serializable form of its own and can only be published inside an effective size, so
//! a headline number cannot appear without the honest number beside it. Here, a world's scope
//! cannot be published without the limits its construction imposes.
//!
//! The consequence worth stating: [`ClaimKind::CausalEffectInReality`] is supported by no rung at
//! all, so *every* world card ever published by this crate carries an explicit statement that the
//! world cannot establish a causal effect in a patient. That is not a lint anybody can turn off,
//! and it is the direct type-level form of section 34's trust requirement that "the interface never
//! implies clinical recommendation or patient-level validity".
//!
//! # A card that omits its rung is not constructible
//!
//! [`Ancestry`] has no empty constructor, no `Default`, and deserializes through
//! `TryFrom<Vec<AncestryStep>>` so that `{"ancestry": []}` is a parse error rather than a card with
//! no history — the refusal `bioprism-worldfactory` arrived at, restated where a world is published
//! rather than where one is built. [`WorldCard`]'s fields are private and its only constructor is
//! [`WorldCardDraft::publish`]; its `Deserialize` runs the same validation, so a card cannot be
//! smuggled in as JSON either.
//!
//! # This is not `bioprism_hub::BioAtlasCard`
//!
//! That card describes a **submitted result** and decides whether a score may be shown. This one
//! describes a **world** and decides what may be asked of it. A [`WorldCard`] carries no score, no
//! verification status and no leaderboard position, and there is no conversion between the two —
//! a world is not a result and the shared word "card" in section 34 is the only thing they have in
//! common.
//!
//! # Not implemented
//!
//! No rendering, no card template, no completeness percentage (34.03's "card completeness" product
//! metric would need a weighting over fields that the blueprint does not give, and a number derived
//! from an invented weighting reads as measurement). No clock: [`WorldCard::currency`] takes the
//! reference epoch from the caller and answers [`Currency::Undetermined`] without one, the same
//! choice `bioprism-hubapi` makes for mirror freshness. No licence checking — [`AncestryStep`]
//! carries a [`Licence`] because 34.03 asks for it, and `bioprism_hub::LicenceStack` is where
//! propagation is decided.

use crate::error::CardError;
use bioprism_hub::{AccessTier, Epoch, Licence};
use bioprism_ids::{ContentHash, WorldId};
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The `resource_type` a world card serializes with.
///
/// Deliberately different from `bioprism_hub::RESOURCE_TYPE` (`bioatlas-card`), because a consumer
/// that cannot tell a world card from a result card will eventually show a world's access tier next
/// to a result's score.
pub const RESOURCE_TYPE: &str = "bioatlas-world-card";

/// How far a world's construction stands from measurement.
///
/// Mirrors `bioprism_worldfactory::Rung` rather than importing it: a card is a publication artifact
/// and must be constructible by a hub that has no world factory linked in. The vocabulary is kept
/// identical on purpose, and a deployment that holds both should assert their equality once at the
/// boundary rather than converting field by field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceRung {
    /// 27.02. Imported from real data and real workflows. Nothing in it has a known latent truth.
    Observed,
    /// 27.03. Controlled structure grafted onto an observed world; the injected part has a known
    /// answer and the rest does not.
    SemiSynthetic,
    /// 27.04. Generated by an explicit model. Everything in it is what somebody wrote down.
    Mechanistic,
}

impl ProvenanceRung {
    pub const ALL: [ProvenanceRung; 3] = [
        ProvenanceRung::Observed,
        ProvenanceRung::SemiSynthetic,
        ProvenanceRung::Mechanistic,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProvenanceRung::Observed => "observed",
            ProvenanceRung::SemiSynthetic => "semi-synthetic",
            ProvenanceRung::Mechanistic => "mechanistic",
        }
    }

    /// Construction steps between this rung and measurement. Reported, not ranked: a larger
    /// distance is not worse, it is differently useful.
    pub fn distance_from_measurement(self) -> u8 {
        match self {
            ProvenanceRung::Observed => 0,
            ProvenanceRung::SemiSynthetic => 1,
            ProvenanceRung::Mechanistic => 2,
        }
    }

    /// Whether a world standing on this rung can support a claim of this kind.
    ///
    /// The table, and the reasoning for each cell:
    ///
    /// - [`ClaimKind::EvidenceSelection`] — every rung. Whether a selection of evidence was correct
    ///   given what the world contains is answerable from the world's own contents.
    /// - [`ClaimKind::InjectedStructureRecovery`] — construction rungs only. An observed world has
    ///   no injected structure, so there is nothing to recover and no key to mark it against.
    /// - [`ClaimKind::SimulatorBehaviour`] — mechanistic only. There is no simulator otherwise.
    /// - [`ClaimKind::CausalEffectInReality`] — none. A construction rung answers about the model;
    ///   an observed rung has no counterfactual. This is the cell that makes every card disclaim
    ///   something.
    pub fn supports(self, claim: ClaimKind) -> bool {
        match claim {
            ClaimKind::EvidenceSelection => true,
            ClaimKind::InjectedStructureRecovery => {
                matches!(
                    self,
                    ProvenanceRung::SemiSynthetic | ProvenanceRung::Mechanistic
                )
            }
            ClaimKind::SimulatorBehaviour => matches!(self, ProvenanceRung::Mechanistic),
            ClaimKind::CausalEffectInReality => false,
        }
    }
}

impl fmt::Display for ProvenanceRung {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a claim made on a world is *about*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// That a selection of evidence was correct given what the world contains.
    EvidenceSelection,
    /// That a method recovered structure that was deliberately injected.
    InjectedStructureRecovery,
    /// That an intervention has an effect in a real population.
    CausalEffectInReality,
    /// That the simulator behaves as its equations say.
    SimulatorBehaviour,
}

impl ClaimKind {
    pub const ALL: [ClaimKind; 4] = [
        ClaimKind::EvidenceSelection,
        ClaimKind::InjectedStructureRecovery,
        ClaimKind::CausalEffectInReality,
        ClaimKind::SimulatorBehaviour,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ClaimKind::EvidenceSelection => "evidence selection",
            ClaimKind::InjectedStructureRecovery => "recovery of injected structure",
            ClaimKind::CausalEffectInReality => "a causal effect in reality",
            ClaimKind::SimulatorBehaviour => "simulator behaviour",
        }
    }
}

impl fmt::Display for ClaimKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One construction step in a world's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AncestryStep {
    pub rung: ProvenanceRung,
    /// What this step consumed: a source dataset, a parent world release, a generator spec.
    pub source: ContentHash,
    /// 34.03 asks for "parent provenance and license" in one breath, so the licence rides with the
    /// step it came from rather than being flattened onto the card.
    pub licence: Licence,
    /// What the step did, in the author's words. Free text, because the space of construction steps
    /// is open and a closed enumeration would be wrong within a month.
    pub note: String,
}

impl AncestryStep {
    pub fn new(rung: ProvenanceRung, source: ContentHash, licence: Licence) -> AncestryStep {
        AncestryStep {
            rung,
            source,
            licence,
            note: String::new(),
        }
    }

    pub fn describing(mut self, note: impl Into<String>) -> AncestryStep {
        self.note = note.into();
        self
    }
}

/// Every construction step a world stands on, oldest first.
///
/// Non-empty by construction and by deserialization. There is no `Ancestry::new()` and no
/// `Default`: the only ways to get one are [`Ancestry::of`], [`Ancestry::derived_from`] and
/// `TryFrom<Vec<AncestryStep>>`, and all three require at least one step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<AncestryStep>", into = "Vec<AncestryStep>")]
pub struct Ancestry {
    steps: Vec<AncestryStep>,
}

impl Ancestry {
    /// A world built in one step.
    pub fn of(step: AncestryStep) -> Ancestry {
        Ancestry { steps: vec![step] }
    }

    /// A world built on top of another. The parent's steps carry through unchanged, which is
    /// `bioprism-worldfactory`'s rule at the publication boundary: a derivation cannot shed a rung
    /// by forgetting it.
    pub fn derived_from(parent: &Ancestry, step: AncestryStep) -> Ancestry {
        let mut steps = parent.steps.clone();
        steps.push(step);
        Ancestry { steps }
    }

    pub fn steps(&self) -> &[AncestryStep] {
        &self.steps
    }

    pub fn rungs(&self) -> BTreeSet<ProvenanceRung> {
        self.steps.iter().map(|s| s.rung).collect()
    }

    /// The rung furthest from measurement anywhere in the ancestry.
    ///
    /// This, not the last step's rung, is what governs. A mechanistic world that was later
    /// "observed" by exporting it to a file is still mechanistic, and taking the last rung would
    /// let a laundering step erase the history.
    pub fn deepest(&self) -> ProvenanceRung {
        self.steps
            .iter()
            .map(|s| s.rung)
            .max_by_key(|r| r.distance_from_measurement())
            .expect("Ancestry is non-empty by construction")
    }

    pub fn supports(&self, claim: ClaimKind) -> bool {
        self.deepest().supports(claim)
    }

    /// Exactly the claim kinds this ancestry cannot support, in a stable order.
    pub fn unsupported_claims(&self) -> Vec<ClaimKind> {
        ClaimKind::ALL
            .into_iter()
            .filter(|c| !self.supports(*c))
            .collect()
    }
}

impl TryFrom<Vec<AncestryStep>> for Ancestry {
    type Error = CardError;

    fn try_from(steps: Vec<AncestryStep>) -> Result<Self, Self::Error> {
        if steps.is_empty() {
            return Err(CardError::NoProvenanceRung {
                world: "<unnamed>".to_string(),
            });
        }
        Ok(Ancestry { steps })
    }
}

impl From<Ancestry> for Vec<AncestryStep> {
    fn from(value: Ancestry) -> Self {
        value.steps
    }
}

/// A statement that the world cannot answer a kind of question, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unsuitability {
    pub claim: ClaimKind,
    /// The author's explanation. Required to be non-empty: "not suitable" with no reason is a
    /// checkbox, and the reader needs to know whether the limit is fundamental or fixable.
    pub because: String,
}

impl Unsuitability {
    pub fn new(claim: ClaimKind, because: impl Into<String>) -> Unsuitability {
        Unsuitability {
            claim,
            because: because.into(),
        }
    }
}

/// 34.03's "latent-state availability".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatentState {
    /// Nothing in the world has a known answer. The only honest setting for a purely observed
    /// world, and the reason an observed world can never be a recovery benchmark.
    Absent,
    /// The injected part has a known answer; the rest is whatever it was.
    KnownForInjectedStructure,
    /// Every latent variable has a known value, because a model wrote them all.
    KnownForEverything,
}

impl LatentState {
    pub fn as_str(self) -> &'static str {
        match self {
            LatentState::Absent => "absent",
            LatentState::KnownForInjectedStructure => "known for injected structure",
            LatentState::KnownForEverything => "known for everything",
        }
    }

    fn requires(self) -> Option<ClaimKind> {
        match self {
            LatentState::Absent => None,
            LatentState::KnownForInjectedStructure => Some(ClaimKind::InjectedStructureRecovery),
            LatentState::KnownForEverything => Some(ClaimKind::SimulatorBehaviour),
        }
    }
}

impl fmt::Display for LatentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The three health states 34.03's API object actually enumerates.
///
/// The shared failure-states paragraph lists eight states across all of section 34, but five of
/// them (under-review, disputed, withdrawn, non-reproducible, not-comparable) are properties of a
/// *result*, and `bioprism_hub::PublicationState` already carries them there. Restating them on a
/// world would let a world be "disputed" with no submission to dispute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldHealth {
    Active,
    /// Its checks last passed too long ago, or a dependency moved.
    Stale,
    /// Something is wrong with it and it must not be run.
    Quarantined,
}

impl WorldHealth {
    pub fn as_str(self) -> &'static str {
        match self {
            WorldHealth::Active => "active",
            WorldHealth::Stale => "stale",
            WorldHealth::Quarantined => "quarantined",
        }
    }

    /// Whether a new run may be started against this world. Reading a stale card is always
    /// allowed; starting work on it is not, and the two must not share one boolean.
    pub fn may_be_offered(self) -> bool {
        matches!(self, WorldHealth::Active)
    }
}

impl fmt::Display for WorldHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A recorded health check, with the epoch at which it last ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub epoch: Epoch,
    pub passed: bool,
}

impl HealthCheck {
    pub fn new(name: impl Into<String>, epoch: Epoch, passed: bool) -> HealthCheck {
        HealthCheck {
            name: name.into(),
            epoch,
            passed,
        }
    }
}

/// How current a card's checks are relative to a reference epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "currency")]
pub enum Currency {
    Fresh,
    Stale {
        by: u64,
    },
    /// No reference epoch was supplied. This crate has no clock, and an absent reference is not
    /// evidence of currency — the same rule `bioprism_hubapi::mirror::Freshness::Undetermined`
    /// encodes for mirrors.
    Undetermined,
}

/// 34.03's `links` object: the outbound edges that make a world's results reachable from it.
///
/// Every target is a non-empty string, and result links are [`ContentHash`] because section 34's
/// trust requirement is that "every rendered score resolves to immutable result objects" — a
/// result referenced by mutable name would not.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardLinks {
    #[serde(default)]
    pub worlds: Vec<WorldId>,
    #[serde(default)]
    pub cells: Vec<String>,
    #[serde(default)]
    pub oracles: Vec<String>,
    #[serde(default)]
    pub results: Vec<ContentHash>,
}

impl CardLinks {
    pub fn new() -> CardLinks {
        CardLinks::default()
    }

    pub fn to_cell(mut self, cell: impl Into<String>) -> CardLinks {
        self.cells.push(cell.into());
        self
    }

    pub fn to_oracle(mut self, oracle: impl Into<String>) -> CardLinks {
        self.oracles.push(oracle.into());
        self
    }

    pub fn to_result(mut self, result: ContentHash) -> CardLinks {
        self.results.push(result);
        self
    }

    fn check(&self, world: &WorldId) -> Result<(), CardError> {
        for (kind, target) in self
            .cells
            .iter()
            .map(|c| ("cell", c.as_str()))
            .chain(self.oracles.iter().map(|o| ("oracle", o.as_str())))
        {
            if target.trim().is_empty() {
                return Err(CardError::UnresolvableLink {
                    world: world.to_string(),
                    kind: kind.to_string(),
                    target: target.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Whether every link resolves against a set of known identifiers.
    ///
    /// Separate from construction because a card is published before a hub knows its whole
    /// catalogue, and requiring the catalogue at construction time would make cards unbuildable
    /// offline. This is the residue of the *Information Architecture and Navigation* module's
    /// "broken-link rate" metric, expressed as a predicate rather than as a percentage.
    pub fn resolvable_against(
        &self,
        world: &WorldId,
        known: &BTreeSet<String>,
    ) -> Result<(), CardError> {
        let targets = self
            .worlds
            .iter()
            .map(|w| ("world", w.to_string()))
            .chain(self.cells.iter().map(|c| ("cell", c.clone())))
            .chain(self.oracles.iter().map(|o| ("oracle", o.clone())))
            .chain(
                self.results
                    .iter()
                    .map(|r| ("result", r.as_str().to_string())),
            );
        for (kind, target) in targets {
            if !known.contains(&target) {
                return Err(CardError::UnresolvableLink {
                    world: world.to_string(),
                    kind: kind.to_string(),
                    target,
                });
            }
        }
        Ok(())
    }
}

/// What running the world costs and what an agent may do inside it — 34.03's "action and resource
/// model", kept as declared quantities because this crate does not execute anything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceModel {
    /// The action names an agent may take. Empty means the world is observational.
    #[serde(default)]
    pub actions: Vec<String>,
    /// Named budget axes and their limits, e.g. `("tissue", 4)`. Deliberately not a fixed struct:
    /// 34.10 names four axes and 34.03 names none, and a world may meter something neither lists.
    #[serde(default)]
    pub budgets: Vec<(String, u64)>,
}

/// A mutable, unvalidated card. The only route to a [`WorldCard`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldCardDraft {
    pub world: WorldId,
    pub version: String,
    /// 34.03's disease/modality/decision-family triple, expressed in `bioprism-scope`'s vocabulary
    /// so that a card can be compared against a request with the refinement order rather than by
    /// string matching.
    pub scope: ScopeKey,
    pub ancestry: Ancestry,
    pub access: AccessTier,
    pub latent_state: LatentState,
    #[serde(default)]
    pub resources: ResourceModel,
    pub health: WorldHealth,
    #[serde(default)]
    pub checks: Vec<HealthCheck>,
    #[serde(default)]
    pub not_suitable_for: Vec<Unsuitability>,
    #[serde(default)]
    pub links: CardLinks,
    /// 34.03's "known limitations" as prose, for everything the claim-kind table does not reach.
    #[serde(default)]
    pub limitations: String,
}

impl WorldCardDraft {
    pub fn new(
        world: WorldId,
        version: impl Into<String>,
        scope: ScopeKey,
        ancestry: Ancestry,
    ) -> WorldCardDraft {
        let latent_state = match ancestry.deepest() {
            ProvenanceRung::Observed => LatentState::Absent,
            ProvenanceRung::SemiSynthetic => LatentState::KnownForInjectedStructure,
            ProvenanceRung::Mechanistic => LatentState::KnownForEverything,
        };
        WorldCardDraft {
            world,
            version: version.into(),
            scope,
            ancestry,
            access: AccessTier::Public,
            latent_state,
            resources: ResourceModel::default(),
            health: WorldHealth::Active,
            checks: Vec::new(),
            not_suitable_for: Vec::new(),
            links: CardLinks::default(),
            limitations: String::new(),
        }
    }

    pub fn at_access(mut self, access: AccessTier) -> WorldCardDraft {
        self.access = access;
        self
    }

    pub fn with_latent_state(mut self, latent_state: LatentState) -> WorldCardDraft {
        self.latent_state = latent_state;
        self
    }

    pub fn in_health(mut self, health: WorldHealth) -> WorldCardDraft {
        self.health = health;
        self
    }

    pub fn checked(mut self, check: HealthCheck) -> WorldCardDraft {
        self.checks.push(check);
        self
    }

    pub fn not_suitable_for(mut self, unsuitability: Unsuitability) -> WorldCardDraft {
        self.not_suitable_for.push(unsuitability);
        self
    }

    pub fn with_links(mut self, links: CardLinks) -> WorldCardDraft {
        self.links = links;
        self
    }

    pub fn limited_by(mut self, limitations: impl Into<String>) -> WorldCardDraft {
        self.limitations = limitations.into();
        self
    }

    pub fn with_resources(mut self, resources: ResourceModel) -> WorldCardDraft {
        self.resources = resources;
        self
    }

    /// Adds the disclaimers the rung requires and nothing else, using a stock reason.
    ///
    /// Provided because the alternative — authors hand-writing four disclaimers on every card —
    /// produces four identical strings, and a stock reason that is honest beats a bespoke one that
    /// is copied. An author with something specific to say still writes
    /// [`WorldCardDraft::not_suitable_for`] and this method leaves their wording alone.
    pub fn disclaiming_what_the_rung_cannot_support(mut self) -> WorldCardDraft {
        let rung = self.ancestry.deepest();
        for claim in self.ancestry.unsupported_claims() {
            if self.not_suitable_for.iter().any(|u| u.claim == claim) {
                continue;
            }
            let because = match claim {
                ClaimKind::InjectedStructureRecovery => format!(
                    "this world stands on {rung} data, into which no structure was injected, so there is no key to score a recovery against"
                ),
                ClaimKind::CausalEffectInReality => format!(
                    "this world stands on {rung} data; an effect measured here is an effect in the world as built, and no rung of construction makes it an effect in a patient"
                ),
                ClaimKind::SimulatorBehaviour => format!(
                    "this world stands on {rung} data and has no simulator whose behaviour could be the subject of a claim"
                ),
                ClaimKind::EvidenceSelection => String::new(),
            };
            self.not_suitable_for
                .push(Unsuitability::new(claim, because));
        }
        self
    }

    /// Validate and freeze.
    pub fn publish(self) -> Result<WorldCard, CardError> {
        WorldCard::try_from(self)
    }
}

/// A published world card.
///
/// Fields are private and there is no public constructor other than [`WorldCardDraft::publish`].
/// Deserialization routes through the draft and runs the same checks, so there is no path — in
/// code or in JSON — to a card that has forgotten what it cannot do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "WorldCardDraft")]
pub struct WorldCard {
    resource_type: String,
    world: WorldId,
    version: String,
    scope: ScopeKey,
    ancestry: Ancestry,
    access: AccessTier,
    latent_state: LatentState,
    resources: ResourceModel,
    health: WorldHealth,
    checks: Vec<HealthCheck>,
    not_suitable_for: Vec<Unsuitability>,
    links: CardLinks,
    limitations: String,
}

impl TryFrom<WorldCardDraft> for WorldCard {
    type Error = CardError;

    fn try_from(draft: WorldCardDraft) -> Result<Self, Self::Error> {
        let world_name = draft.world.to_string();

        if draft.scope.is_empty() {
            return Err(CardError::UnscopedCard { world: world_name });
        }

        let rung = draft.ancestry.deepest();

        if let Some(required) = draft.latent_state.requires() {
            if !rung.supports(required) {
                return Err(CardError::LatentStateWithoutConstruction {
                    world: world_name,
                    offered: draft.latent_state.to_string(),
                });
            }
        }

        for claim in draft.ancestry.unsupported_claims() {
            let declared = draft
                .not_suitable_for
                .iter()
                .any(|u| u.claim == claim && !u.because.trim().is_empty());
            if !declared {
                return Err(CardError::UndeclaredUnsuitability {
                    world: world_name,
                    rung: rung.to_string(),
                    claim: claim.to_string(),
                });
            }
        }

        draft.links.check(&draft.world)?;

        Ok(WorldCard {
            resource_type: RESOURCE_TYPE.to_string(),
            world: draft.world,
            version: draft.version,
            scope: draft.scope,
            ancestry: draft.ancestry,
            access: draft.access,
            latent_state: draft.latent_state,
            resources: draft.resources,
            health: draft.health,
            checks: draft.checks,
            not_suitable_for: draft.not_suitable_for,
            links: draft.links,
            limitations: draft.limitations,
        })
    }
}

impl WorldCard {
    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }

    pub fn world(&self) -> &WorldId {
        &self.world
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }

    pub fn ancestry(&self) -> &Ancestry {
        &self.ancestry
    }

    /// The rung that governs what may be claimed. Always present: that is the point of the type.
    pub fn rung(&self) -> ProvenanceRung {
        self.ancestry.deepest()
    }

    pub fn access(&self) -> AccessTier {
        self.access
    }

    pub fn latent_state(&self) -> LatentState {
        self.latent_state
    }

    pub fn health(&self) -> WorldHealth {
        self.health
    }

    pub fn checks(&self) -> &[HealthCheck] {
        &self.checks
    }

    pub fn resources(&self) -> &ResourceModel {
        &self.resources
    }

    pub fn not_suitable_for(&self) -> &[Unsuitability] {
        &self.not_suitable_for
    }

    pub fn links(&self) -> &CardLinks {
        &self.links
    }

    pub fn limitations(&self) -> &str {
        &self.limitations
    }

    /// Whether this card permits a claim of the given kind.
    ///
    /// A card never *grants* a claim; it either fails to forbid it or names the reason it does.
    pub fn permits(&self, claim: ClaimKind) -> Option<&Unsuitability> {
        self.not_suitable_for.iter().find(|u| u.claim == claim)
    }

    /// Whether a request whose scope is `request` falls inside this card's declared scope.
    ///
    /// Uses `bioprism-scope`'s refinement order rather than equality: a card scoped to
    /// `disease=glioma` answers a request scoped to `disease=glioma, site=A`, and not the reverse.
    pub fn covers(&self, request: &ScopeKey) -> bool {
        request.refines(&self.scope)
    }

    /// May a new run be started against this world?
    pub fn offerable(&self) -> Result<(), CardError> {
        if self.health.may_be_offered() {
            Ok(())
        } else {
            Err(CardError::NotOfferable {
                world: self.world.to_string(),
                health: self.health.to_string(),
            })
        }
    }

    /// How current the card's checks are, judged against a caller-supplied epoch.
    ///
    /// `as_of == None` is the ordinary air-gapped case and yields [`Currency::Undetermined`]. A
    /// card with no checks at all is likewise undetermined rather than fresh.
    pub fn currency(&self, as_of: Option<Epoch>, bound: u64) -> Currency {
        let (Some(now), Some(latest)) = (as_of, self.checks.iter().map(|c| c.epoch.get()).max())
        else {
            return Currency::Undetermined;
        };
        let age = now.get().saturating_sub(latest);
        if age <= bound {
            Currency::Fresh
        } else {
            Currency::Stale { by: age - bound }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_hub::Licence;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn licence() -> Licence {
        Licence::permissive("CC-BY-4.0")
    }

    fn observed() -> Ancestry {
        Ancestry::of(
            AncestryStep::new(ProvenanceRung::Observed, hash("cohort"), licence())
                .describing("imported from a real registry extract"),
        )
    }

    fn semi_synthetic() -> Ancestry {
        Ancestry::derived_from(
            &observed(),
            AncestryStep::new(ProvenanceRung::SemiSynthetic, hash("graft"), licence())
                .describing("injected a known confounder"),
        )
    }

    fn mechanistic() -> Ancestry {
        Ancestry::of(AncestryStep::new(
            ProvenanceRung::Mechanistic,
            hash("generator"),
            licence(),
        ))
    }

    fn scope() -> ScopeKey {
        ScopeKey::new()
            .exact("disease", "glioma")
            .exact("modality", "mri")
    }

    fn draft(ancestry: Ancestry) -> WorldCardDraft {
        WorldCardDraft::new(
            WorldId::parse("world/demo@1").unwrap(),
            "1.0.0",
            scope(),
            ancestry,
        )
    }

    #[test]
    fn a_world_card_that_omits_its_provenance_rung_is_not_constructible() {
        let empty: Result<Ancestry, _> = Vec::new().try_into();
        assert!(matches!(empty, Err(CardError::NoProvenanceRung { .. })));
    }

    #[test]
    fn an_ancestry_serialised_as_an_empty_list_is_a_parse_error_not_an_empty_card() {
        let parsed = serde_json::from_str::<Ancestry>("[]");
        assert!(parsed.is_err());
    }

    #[test]
    fn a_card_that_does_not_disclaim_causal_effect_in_reality_is_refused() {
        let err = draft(mechanistic())
            .not_suitable_for(Unsuitability::new(
                ClaimKind::SimulatorBehaviour,
                "irrelevant",
            ))
            .publish()
            .unwrap_err();
        assert!(matches!(
            err,
            CardError::UndeclaredUnsuitability { ref claim, .. }
                if claim == ClaimKind::CausalEffectInReality.as_str()
        ));
    }

    #[test]
    fn every_publishable_card_disclaims_a_causal_effect_in_reality_whatever_its_rung() {
        for ancestry in [observed(), semi_synthetic(), mechanistic()] {
            let card = draft(ancestry)
                .disclaiming_what_the_rung_cannot_support()
                .publish()
                .unwrap();
            assert!(card.permits(ClaimKind::CausalEffectInReality).is_some());
        }
    }

    #[test]
    fn an_observed_world_must_disclaim_recovery_of_injected_structure() {
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .publish()
            .unwrap();
        assert!(card.permits(ClaimKind::InjectedStructureRecovery).is_some());
    }

    #[test]
    fn a_semi_synthetic_world_need_not_disclaim_recovery_of_injected_structure() {
        let card = draft(semi_synthetic())
            .disclaiming_what_the_rung_cannot_support()
            .publish()
            .unwrap();
        assert!(card.permits(ClaimKind::InjectedStructureRecovery).is_none());
    }

    #[test]
    fn an_unsuitability_with_a_blank_reason_does_not_discharge_the_requirement() {
        let err = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .not_suitable_for(Unsuitability::new(ClaimKind::EvidenceSelection, "   "))
            .publish();
        assert!(
            err.is_ok(),
            "evidence selection is supported, so a blank reason for it is merely unused"
        );

        let mut d = draft(observed());
        d.not_suitable_for = ClaimKind::ALL
            .into_iter()
            .map(|c| Unsuitability::new(c, "  "))
            .collect();
        assert!(matches!(
            d.publish(),
            Err(CardError::UndeclaredUnsuitability { .. })
        ));
    }

    #[test]
    fn the_deepest_rung_governs_even_when_a_later_step_is_shallower() {
        let laundered = Ancestry::derived_from(
            &mechanistic(),
            AncestryStep::new(ProvenanceRung::Observed, hash("export"), licence())
                .describing("exported the simulated cohort to a csv"),
        );
        assert_eq!(laundered.deepest(), ProvenanceRung::Mechanistic);
    }

    #[test]
    fn a_derivation_carries_its_parents_rungs_through() {
        let child = semi_synthetic();
        assert_eq!(child.steps().len(), 2);
        assert!(child.rungs().contains(&ProvenanceRung::Observed));
        assert!(child.rungs().contains(&ProvenanceRung::SemiSynthetic));
    }

    #[test]
    fn no_rung_supports_a_causal_effect_in_reality() {
        for rung in ProvenanceRung::ALL {
            assert!(!rung.supports(ClaimKind::CausalEffectInReality));
        }
    }

    #[test]
    fn an_observed_world_may_not_advertise_latent_state() {
        let err = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .with_latent_state(LatentState::KnownForInjectedStructure)
            .publish()
            .unwrap_err();
        assert!(matches!(
            err,
            CardError::LatentStateWithoutConstruction { .. }
        ));
    }

    #[test]
    fn a_semi_synthetic_world_may_not_claim_latent_state_for_everything() {
        let err = draft(semi_synthetic())
            .disclaiming_what_the_rung_cannot_support()
            .with_latent_state(LatentState::KnownForEverything)
            .publish()
            .unwrap_err();
        assert!(matches!(
            err,
            CardError::LatentStateWithoutConstruction { .. }
        ));
    }

    #[test]
    fn a_card_that_binds_no_scope_dimension_is_refused() {
        let d = WorldCardDraft::new(
            WorldId::parse("world/demo@1").unwrap(),
            "1.0.0",
            ScopeKey::new(),
            observed(),
        )
        .disclaiming_what_the_rung_cannot_support();
        assert!(matches!(d.publish(), Err(CardError::UnscopedCard { .. })));
    }

    #[test]
    fn a_card_covers_a_request_that_refines_its_scope_but_not_one_that_widens_it() {
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .publish()
            .unwrap();
        let narrower = scope().exact("site", "A");
        let wider = ScopeKey::new().exact("disease", "glioma");
        assert!(card.covers(&narrower));
        assert!(!card.covers(&wider));
    }

    #[test]
    fn a_stale_card_may_be_read_but_not_offered_for_a_new_run() {
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .in_health(WorldHealth::Stale)
            .publish()
            .unwrap();
        assert_eq!(card.health(), WorldHealth::Stale);
        assert!(matches!(
            card.offerable(),
            Err(CardError::NotOfferable { .. })
        ));
    }

    #[test]
    fn currency_without_a_reference_epoch_is_undetermined_not_fresh() {
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .checked(HealthCheck::new("replay", Epoch(10), true))
            .publish()
            .unwrap();
        assert_eq!(card.currency(None, 5), Currency::Undetermined);
        assert_eq!(card.currency(Some(Epoch(12)), 5), Currency::Fresh);
        assert_eq!(card.currency(Some(Epoch(20)), 5), Currency::Stale { by: 5 });
    }

    #[test]
    fn a_card_with_no_checks_is_undetermined_rather_than_fresh() {
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .publish()
            .unwrap();
        assert_eq!(card.currency(Some(Epoch(99)), 5), Currency::Undetermined);
    }

    #[test]
    fn a_blank_link_target_is_a_construction_error() {
        let err = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .with_links(CardLinks::new().to_cell("  "))
            .publish()
            .unwrap_err();
        assert!(matches!(err, CardError::UnresolvableLink { .. }));
    }

    #[test]
    fn a_link_to_an_unknown_result_does_not_resolve() {
        let result = hash("result");
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .with_links(CardLinks::new().to_result(result.clone()))
            .publish()
            .unwrap();
        let known = BTreeSet::new();
        assert!(matches!(
            card.links().resolvable_against(card.world(), &known),
            Err(CardError::UnresolvableLink { .. })
        ));
        let known: BTreeSet<String> = [result.as_str().to_string()].into_iter().collect();
        assert!(card
            .links()
            .resolvable_against(card.world(), &known)
            .is_ok());
    }

    #[test]
    fn a_card_round_trips_through_json_and_revalidates_on_the_way_back() {
        let card = draft(semi_synthetic())
            .disclaiming_what_the_rung_cannot_support()
            .at_access(AccessTier::Controlled)
            .limited_by("single site, adult only")
            .publish()
            .unwrap();
        let json = serde_json::to_string(&card).unwrap();
        assert!(json.contains(RESOURCE_TYPE));
        assert!(json.contains("semi-synthetic"));
        let back: WorldCard = serde_json::from_str(&json).unwrap();
        assert_eq!(back, card);
    }

    #[test]
    fn a_json_card_stripped_of_its_disclaimers_will_not_deserialize() {
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .publish()
            .unwrap();
        let mut value: serde_json::Value = serde_json::to_value(&card).unwrap();
        value["not_suitable_for"] = serde_json::json!([]);
        let back = serde_json::from_value::<WorldCard>(value);
        assert!(back.is_err());
    }

    #[test]
    fn a_world_card_carries_no_score_field() {
        let card = draft(observed())
            .disclaiming_what_the_rung_cannot_support()
            .publish()
            .unwrap();
        let value = serde_json::to_value(&card).unwrap();
        let object = value.as_object().unwrap();
        assert!(!object.contains_key("score"));
        assert!(!object.contains_key("verification"));
        assert_eq!(object["resource_type"], RESOURCE_TYPE);
    }
}
