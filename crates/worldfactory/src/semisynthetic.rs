//! Semi-synthetic worlds: grafting known structure onto observed data (27.03).
//!
//! 27.03 buys one thing that neither of its neighbours can supply: **a known answer inside
//! realistic complexity**. An observed world has realism and no ground truth; a simulation has
//! ground truth and no realism. A graft has both, and pays for it with a permanent asterisk.
//!
//! The asterisk is the module. 27.03's *Critical design decision* is that "semi-synthetic status
//! remains visible in every result and cannot be erased by downstream packaging", and its failure
//! list ends with "synthetic label presented as observed fact". So in this module **every fact
//! carries its origin, and there is no third variant**. [`Origin`] is `Observed` or `Injected`;
//! there is no `Unknown`, no `Default`, and no `#[serde(default)]` anywhere near it. A world that
//! has forgotten which of its facts were invented cannot support a claim about real biology, and
//! the cheapest way to guarantee it never forgets is to make forgetting unrepresentable.
//!
//! # The three checks a graft must pass
//!
//! * **Blast radius.** 27.03 workflow step 5: "leave unrelated structure unchanged". A graft
//!   declares its target set; editing anything outside it is
//!   [`crate::error::GraftRefusal::OutsideTargetSet`]. Without this the "changed-state manifest"
//!   27.03 requires is a guess.
//! * **Observed substrate.** A graft onto a fact that was itself injected is grafting onto a
//!   graft, and the world has no observed structure under the point it claims to be testing.
//! * **Shortcut.** 27.03's failure "only one file changes and reveals answer". A graft whose entire
//!   footprint is the one fact the oracle asks about is a lookup table with extra steps.
//!   [`shortcut_scan`] is separate from [`apply`] because a single-fact graft is sometimes exactly
//!   what an author wants — the refusal belongs at publication, against a named oracle target.
//!
//! # What this buys downstream
//!
//! [`SemiSyntheticWorld::provenance`] returns a [`Provenance`] standing on the parent's rungs plus
//! [`Rung::SemiSynthetic`], carrying every injected quantity as an assumption. That is what makes
//! [`crate::provenance::support`] refuse a biological claim about an effect somebody injected, and
//! what makes the same claim about *detecting* that effect succeed.
//!
//! # What is deliberately not here
//!
//! No transformation library. 27.03's "constraint-aware transformation" is a domain operation over
//! real data — a simulated batch effect, a re-segmented volume, a shifted prevalence — and
//! `crates/stress` already implements that family against a cohort model. This module owns the
//! *bookkeeping* that makes any such transformation attributable afterwards, which is the part
//! that has no other home. Fact values are opaque `serde_json::Value`; nothing here interprets one.

use crate::error::GraftRefusal;
use crate::observed::ObservedWorld;
use crate::provenance::{Provenance, Rung};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A fact's key within a world. Opaque: §27 fixes no fact vocabulary.
pub type FactKey = String;

/// Where a fact came from. Two variants, permanently.
///
/// There is no `Unknown`. A world is assembled from an [`ObservedWorld`], where every fact is
/// `Observed` by construction, and the only operation that changes an origin is [`apply`], which
/// sets `Injected`. Every path through this module therefore produces a fact whose origin is a
/// fact about how it got there, not a guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum Origin {
    Observed { source: String },
    Injected { graft: GraftId },
}

impl Origin {
    pub fn is_injected(&self) -> bool {
        matches!(self, Origin::Injected { .. })
    }
}

/// A graft's identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GraftId(String);

impl GraftId {
    pub fn new(value: impl Into<String>) -> Self {
        GraftId(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GraftId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A fact and where it came from.
///
/// `origin` has no serde default, so a document that omits it fails to deserialise. [`parse_fact`]
/// turns that failure into a named refusal rather than a parser message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenancedFact {
    pub key: FactKey,
    pub value: Value,
    pub origin: Origin,
}

/// Parse a fact, refusing one whose origin was not declared.
///
/// The refusal exists because the alternative — defaulting an absent origin to `Observed` — is
/// precisely 27.03's failure "synthetic label presented as observed fact", and it is the kind of
/// convenience that gets added during an import and never noticed again.
pub fn parse_fact(value: &Value) -> Result<ProvenancedFact, GraftRefusal> {
    let key = value
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("<unnamed>")
        .to_string();
    serde_json::from_value(value.clone()).map_err(|_| GraftRefusal::OriginNotDeclared { fact: key })
}

/// A controlled change to an observed world — 27.03's "mutation program".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graft {
    pub id: GraftId,
    /// The facts this graft is permitted to touch. Declared before the edits so that the edits can
    /// be checked against it rather than described by it.
    pub targets: BTreeSet<FactKey>,
    pub edits: BTreeMap<FactKey, Value>,
    /// The quantities this graft fixes, which become assumptions of every world downstream.
    pub injects: BTreeSet<String>,
}

impl Graft {
    pub fn new(id: impl Into<String>) -> Self {
        Graft {
            id: GraftId::new(id),
            targets: BTreeSet::new(),
            edits: BTreeMap::new(),
            injects: BTreeSet::new(),
        }
    }

    pub fn targeting(mut self, key: impl Into<FactKey>) -> Self {
        self.targets.insert(key.into());
        self
    }

    /// Record an edit. Does not add the key to the target set — that is the point of having two
    /// sets, and [`apply`] compares them.
    pub fn editing(mut self, key: impl Into<FactKey>, value: Value) -> Self {
        self.edits.insert(key.into(), value);
        self
    }

    pub fn injecting(mut self, quantity: impl Into<String>) -> Self {
        self.injects.insert(quantity.into());
        self
    }
}

/// 27.03's required "changed-state manifest", produced by the operation rather than written by the
/// author.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraftReport {
    pub graft: GraftId,
    pub changed: BTreeSet<FactKey>,
    /// Facts that were in the target set and were not edited. Reported because an unused target is
    /// a declaration that did not survive contact with the transformation, and an author should
    /// see it.
    pub declared_but_untouched: BTreeSet<FactKey>,
    pub untouched_facts: usize,
    /// Digest over every fact outside the target set, before and after. Equal digests are the
    /// executable form of "leave unrelated structure unchanged".
    pub outside_digest_before: String,
    pub outside_digest_after: String,
}

impl GraftReport {
    /// Whether the graft left everything outside its target set byte-identical.
    pub fn respected_target_set(&self) -> bool {
        self.outside_digest_before == self.outside_digest_after
    }
}

/// A world that is part observed and part invented, and knows which is which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemiSyntheticWorld {
    parent_id: String,
    parent_provenance: Provenance,
    facts: BTreeMap<FactKey, ProvenancedFact>,
    grafts: Vec<GraftId>,
    injected_quantities: BTreeSet<String>,
}

impl SemiSyntheticWorld {
    /// Start from an observed world. Every fact is `Observed` and named with the world's id, so
    /// the origin is a statement about where it came from rather than a placeholder.
    pub fn from_observed(
        parent: &ObservedWorld,
        facts: BTreeMap<FactKey, Value>,
    ) -> Self {
        let source = parent.id().to_string();
        SemiSyntheticWorld {
            parent_id: parent.id().to_string(),
            parent_provenance: parent.provenance(),
            facts: facts
                .into_iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        ProvenancedFact {
                            key,
                            value,
                            origin: Origin::Observed {
                                source: source.clone(),
                            },
                        },
                    )
                })
                .collect(),
            grafts: Vec::new(),
            injected_quantities: BTreeSet::new(),
        }
    }

    /// Start from any parent provenance, including a mechanistic one.
    ///
    /// 27.03's workflow says "select a validated observed world", but grafting onto a simulation is
    /// a thing people do, and the honest response is to let them and make the ancestry carry it —
    /// [`SemiSyntheticWorld::provenance`] will report both rungs and every simulator assumption.
    /// Forbidding the construction would just move it outside the type.
    pub fn from_parent(
        parent_id: impl Into<String>,
        parent_provenance: Provenance,
        facts: BTreeMap<FactKey, Value>,
    ) -> Self {
        let parent_id = parent_id.into();
        let source = parent_id.clone();
        SemiSyntheticWorld {
            parent_id,
            parent_provenance,
            facts: facts
                .into_iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        ProvenancedFact {
                            key,
                            value,
                            origin: Origin::Observed {
                                source: source.clone(),
                            },
                        },
                    )
                })
                .collect(),
            grafts: Vec::new(),
            injected_quantities: BTreeSet::new(),
        }
    }

    pub fn parent_id(&self) -> &str {
        &self.parent_id
    }

    pub fn facts(&self) -> &BTreeMap<FactKey, ProvenancedFact> {
        &self.facts
    }

    pub fn grafts(&self) -> &[GraftId] {
        &self.grafts
    }

    /// Where a fact came from. `None` only when the key is absent — never "present but unknown".
    pub fn origin_of(&self, key: &str) -> Option<&Origin> {
        self.facts.get(key).map(|f| &f.origin)
    }

    pub fn injected_keys(&self) -> BTreeSet<FactKey> {
        self.facts
            .values()
            .filter(|f| f.origin.is_injected())
            .map(|f| f.key.clone())
            .collect()
    }

    pub fn observed_keys(&self) -> BTreeSet<FactKey> {
        self.facts
            .values()
            .filter(|f| !f.origin.is_injected())
            .map(|f| f.key.clone())
            .collect()
    }

    /// The provenance this world confers: the parent's rungs plus semi-synthetic, and every
    /// injected quantity as an assumption.
    pub fn provenance(&self) -> Provenance {
        Provenance::semi_synthetic(&self.parent_provenance, self.injected_quantities.iter().cloned())
    }

    /// The published summary — 27.03's "cannot be erased by downstream packaging".
    ///
    /// `Serialize` only and no constructor, so a publisher cannot mint a card claiming a rung the
    /// world does not stand on. The counts come from the fact origins rather than from a field
    /// anyone can set.
    pub fn card(&self) -> WorldCard {
        WorldCard {
            parent: self.parent_id.clone(),
            furthest_from_observation: self.provenance().furthest_from_observation(),
            stands_on: self.provenance().stands_on().iter().copied().collect(),
            observed_facts: self.observed_keys().len(),
            injected_facts: self.injected_keys().len(),
            grafts: self.grafts.clone(),
        }
    }

    fn outside_digest(&self, targets: &BTreeSet<FactKey>) -> String {
        let outside: BTreeMap<&FactKey, &Value> = self
            .facts
            .iter()
            .filter(|(key, _)| !targets.contains(*key))
            .map(|(key, fact)| (key, &fact.value))
            .collect();
        let value = serde_json::to_value(&outside).unwrap_or(Value::Null);
        ContentHash::of_value(&value)
            .map(|h| h.as_str().to_string())
            .unwrap_or_else(|_| "uncanonicalisable".to_string())
    }
}

/// A published world's provenance summary. See [`SemiSyntheticWorld::card`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorldCard {
    pub parent: String,
    pub furthest_from_observation: Rung,
    pub stands_on: Vec<Rung>,
    pub observed_facts: usize,
    pub injected_facts: usize,
    pub grafts: Vec<GraftId>,
}

/// Apply a graft, or refuse it.
///
/// On success the edited facts' origins become [`Origin::Injected`] naming this graft, the graft's
/// injected quantities join the world's assumption set, and a [`GraftReport`] records the blast
/// radius with a digest over everything outside the target set.
///
/// The world is left untouched on refusal: the checks all run before any mutation, so a rejected
/// graft cannot leave a half-applied world behind. That matters more here than usual, because a
/// half-applied graft is a world whose origins are correct and whose values are not.
pub fn apply(
    world: &mut SemiSyntheticWorld,
    graft: &Graft,
) -> Result<GraftReport, GraftRefusal> {
    for key in graft.edits.keys() {
        if !graft.targets.contains(key) {
            return Err(GraftRefusal::OutsideTargetSet {
                graft: graft.id.to_string(),
                fact: key.clone(),
            });
        }
        if world
            .facts
            .get(key)
            .is_some_and(|fact| fact.origin.is_injected())
        {
            return Err(GraftRefusal::TargetIsItselfInjected {
                graft: graft.id.to_string(),
                fact: key.clone(),
            });
        }
    }

    let before = world.outside_digest(&graft.targets);
    let mut changed = BTreeSet::new();
    for (key, value) in &graft.edits {
        world.facts.insert(
            key.clone(),
            ProvenancedFact {
                key: key.clone(),
                value: value.clone(),
                origin: Origin::Injected {
                    graft: graft.id.clone(),
                },
            },
        );
        changed.insert(key.clone());
    }
    let after = world.outside_digest(&graft.targets);

    world.grafts.push(graft.id.clone());
    world
        .injected_quantities
        .extend(graft.injects.iter().cloned());

    let declared_but_untouched = graft.targets.difference(&changed).cloned().collect();
    Ok(GraftReport {
        graft: graft.id.clone(),
        changed: changed.clone(),
        declared_but_untouched,
        untouched_facts: world.facts.len() - changed.len(),
        outside_digest_before: before,
        outside_digest_after: after,
    })
}

/// 27.03's "shortcut scan", against a named oracle target.
///
/// Separate from [`apply`] because a one-fact graft is legitimate until somebody points the oracle
/// at that fact. The scan needs to know what the benchmark asks, and only the author of the
/// decision cell knows that.
pub fn shortcut_scan(report: &GraftReport, oracle_target: &str) -> Result<(), GraftRefusal> {
    if report.changed.len() == 1 && report.changed.contains(oracle_target) {
        return Err(GraftRefusal::SingleFactTell {
            graft: report.graft.to_string(),
            fact: oracle_target.to_string(),
        });
    }
    Ok(())
}
