//! Multimodal contradiction and reconciliation programs (27.14).
//!
//! Two modalities disagree. Imaging reports one thing about a quantity and pathology reports
//! another. The ordinary engineering reflex is to pick a winner, average them, or route the pair
//! to whichever pipeline is louder — and every one of those actions destroys the only interesting
//! object in the situation. 27.14's *Critical design decision* says so directly: the benchmark
//! "rewards constructing and narrowing a hypothesis set rather than forcing immediate consensus".
//!
//! So a contradiction is a first-class object here. [`Contradiction`] records which two modalities
//! disagree, on what quantity, over what scope, and — crucially — *what could account for it*.
//! There is no field naming the correct modality and no function that returns one.
//!
//! # The three resolution states
//!
//! [`ResolutionState`] has three variants and the distinction between the last two is the point of
//! the module:
//!
//! * [`ResolutionState::Resolved`] — discriminating evidence in the world was examined and the
//!   hypothesis set narrowed. It may still hold more than one hypothesis, and it usually will.
//! * [`ResolutionState::NotYetExamined`] — discriminating evidence *exists in the world* and
//!   nobody has looked at it. Nothing has been learned. The set is as wide as it started.
//! * [`ResolutionState::Unresolvable`] — every discriminating action the world offers has been
//!   examined and the set is still wider than one. This variant carries
//!   [`MissingEvidence`]: the named evidence that would settle it and does not exist.
//!
//! Collapsing the middle state into the last is the failure this module exists to prevent. "We
//! cannot tell" and "we have not looked" are the same sentence from a scoreboard's point of view
//! and opposite sentences from a scientist's. [`Contradiction::state`] can only return
//! `Unresolvable` when the unexamined-action list is empty, and a test asserts it.
//!
//! # Why scope alignment comes first
//!
//! 27.14's validation list opens with "scope and time alignment", and [`pose`] enforces it before
//! anything else using `bioprism_scope::meet`. If the two readings' scopes do not overlap they are
//! not in disagreement at all — they are statements about different material, or different
//! instants, and the dimension that separates them is the finding rather than an obstacle to it.
//! [`ContradictionRefusal::ScopesDoNotOverlap`] names that dimension.
//!
//! The same discipline covers the case `crates/oncoworlds` established for markers: a modality
//! that was never applied to the quantity has not disagreed with anything.
//! [`Reported::NotExamined`] is a distinct variant from any value, [`pose`] refuses a pair
//! containing one, and there is no way to coerce it into a negative reading.
//!
//! # What the hypotheses are
//!
//! 27.14's purpose sentence enumerates the axes an agent must reason about: "specimen, time,
//! sensitivity, spatial sampling, and assay scope". [`Discordance`] has one variant per axis, plus
//! [`Discordance::PreanalyticFault`] and [`Discordance::SpecimenIdentityError`] which hand off to
//! [`crate::preanalytic`] and [`crate::lineage`], plus
//! [`Discordance::IrreducibleDiscordance`] for the case where both readings are simply correct
//! about their own scopes and no reconciliation exists.
//!
//! Each variant declares its own *admissibility* condition against the posed contradiction, so an
//! author cannot list "sensitivity" as an explanation when the modality that reported nothing is
//! the one with the better declared floor. 27.14's failure "discordance impossible biologically"
//! is caught the same way: a contradiction with no admissible hypothesis is refused, because an
//! agent who cannot solve an unsolvable puzzle has not failed at anything.
//!
//! # What is deliberately not here
//!
//! No imaging, no pathology, no assay, no images and no measurements. This module holds
//! *declarations about* readings — a modality name, a lens, a scope key and an exact value — and
//! runs set arithmetic over them. There is likewise **no probability model**: 27.14's
//! "information-gain ordering" is implemented in [`next_actions`] as the count of live hypotheses
//! an action can refute, tie-broken by declared cost and then by identifier. That is a set-cover
//! surrogate and it is named as one, because calling an unweighted count "information gain" would
//! be borrowing authority from a theory this crate does not implement.
//!
//! The modality names, quantity names and lens identifiers are opaque strings. The blueprint fixes
//! no modality list, and inventing one would be inventing clinical facts.

use crate::error::ContradictionRefusal;
use bioprism_scope::{meet, EmptyReason, Meet, ScopeKey};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

macro_rules! opaque_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                $name(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

opaque_id!(
    ModalityId,
    "A modality. Opaque: 27.14 names no modality list, and a fixed enumeration here would be an \
     invented clinical vocabulary."
);
opaque_id!(
    QuantityId,
    "The quantity two modalities are asked about. Opaque for the same reason as [`ModalityId`]."
);
opaque_id!(
    LensId,
    "A modality-specific lens — 27.14's second required artifact. Opaque; the lens *content* that \
     this module reasons over is in [`Lens`]."
);
opaque_id!(
    EvidenceId,
    "A piece of discriminating evidence that exists in the world."
);
opaque_id!(HypothesisId, "A member of the hypothesis set.");

/// How much of the subject a modality actually looked at.
///
/// 27.14 names "spatial sampling" as one of the five axes. The distinction that matters is between
/// a modality that surveyed the whole thing and one that examined a fragment: the second can miss
/// what the first found without either being wrong.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "extent", rename_all = "snake_case")]
pub enum SpatialExtent {
    /// The modality surveyed the whole region under discussion.
    Whole,
    /// The modality examined a named part of it.
    Sampled { region: String },
    /// Not declared. Distinct from `Whole`: an undeclared extent is not a survey, and treating it
    /// as one is how a sampling explanation gets silently ruled out.
    Undeclared,
}

/// A modality-specific lens: what this instrument can and cannot report.
///
/// 27.14 requires "modality-specific lenses" as an artifact. The three fields here are the ones
/// the module's own purpose sentence requires an agent to reason about; anything else a real lens
/// carries (acquisition parameters, stains, reconstruction kernels) is absent because this crate
/// has no assay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lens {
    pub id: LensId,
    /// The smallest thing this lens can report, in the quantity's own units, when declared. `None`
    /// means the floor is undeclared — which is not the same as "no floor", and
    /// [`Discordance::SensitivityLimit`] refuses to be admissible without one.
    pub detection_floor: Option<i64>,
    pub spatial_extent: SpatialExtent,
    /// What the assay is able to report at all. Two lenses whose declared scopes differ can
    /// disagree without either being in error, which is 27.14's "assay scope" axis.
    pub assay_scope: String,
}

impl Lens {
    pub fn new(id: impl Into<String>, assay_scope: impl Into<String>) -> Self {
        Lens {
            id: LensId::new(id),
            detection_floor: None,
            spatial_extent: SpatialExtent::Undeclared,
            assay_scope: assay_scope.into(),
        }
    }

    pub fn detecting_down_to(mut self, floor: i64) -> Self {
        self.detection_floor = Some(floor);
        self
    }

    pub fn over(mut self, extent: SpatialExtent) -> Self {
        self.spatial_extent = extent;
        self
    }
}

/// An exact reading value.
///
/// Integers rather than floats, so that "these two agree" is a reproducible decision rather than a
/// tolerance argument. The unit is the caller's: the blueprint fixes none, and this crate measures
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "value", rename_all = "snake_case")]
pub enum ReadingValue {
    Categorical(String),
    /// A closed interval `[low, high]`. Overlapping intervals **agree**: two readings whose
    /// uncertainty ranges intersect have not contradicted each other, and 27.14's failure
    /// "uncertainty not represented" is exactly the mistake of comparing point estimates and
    /// manufacturing a disagreement out of precision the measurements never had.
    Interval {
        low: i64,
        high: i64,
    },
}

impl ReadingValue {
    pub fn interval(low: i64, high: i64) -> Self {
        ReadingValue::Interval {
            low: low.min(high),
            high: low.max(high),
        }
    }

    /// True when the two values are compatible — equal categories, or intersecting intervals.
    pub fn agrees_with(&self, other: &ReadingValue) -> bool {
        match (self, other) {
            (ReadingValue::Categorical(a), ReadingValue::Categorical(b)) => a == b,
            (
                ReadingValue::Interval { low: al, high: ah },
                ReadingValue::Interval { low: bl, high: bh },
            ) => al.max(bl) <= ah.min(bh),
            _ => false,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            ReadingValue::Categorical(v) => v.clone(),
            ReadingValue::Interval { low, high } => format!("[{low},{high}]"),
        }
    }
}

/// What a modality reported, or the fact that it did not look.
///
/// The second variant is the rule `crates/oncoworlds` fixed for markers, restated in this
/// module's currency: a modality that was never applied to the quantity has not made a negative
/// finding, and there is no variant that would let it be read as one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reported", rename_all = "snake_case")]
pub enum Reported {
    Value(ReadingValue),
    NotExamined,
}

/// One modality's statement about one quantity over one scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    pub modality: ModalityId,
    pub quantity: QuantityId,
    pub lens: Lens,
    pub scope: ScopeKey,
    pub reported: Reported,
    /// Free-text annotations travelling with the reading. Present because they are the commonest
    /// vector for 27.14's failure "agent can read mutation metadata", and [`cue_scan`] reads them.
    #[serde(default)]
    pub annotations: Vec<String>,
}

impl Reading {
    pub fn new(
        modality: impl Into<String>,
        quantity: impl Into<String>,
        lens: Lens,
        scope: ScopeKey,
        reported: Reported,
    ) -> Self {
        Reading {
            modality: ModalityId::new(modality),
            quantity: QuantityId::new(quantity),
            lens,
            scope,
            reported,
            annotations: Vec::new(),
        }
    }

    pub fn annotated(mut self, note: impl Into<String>) -> Self {
        self.annotations.push(note.into());
        self
    }

    pub fn value(&self) -> Option<&ReadingValue> {
        match &self.reported {
            Reported::Value(v) => Some(v),
            Reported::NotExamined => None,
        }
    }
}

/// An account of why two modalities disagree.
///
/// The first five variants are 27.14's own axes, in the order its purpose sentence lists them. The
/// next two hand off to the neighbouring mutation families rather than duplicating them. The last
/// is the case the module's title calls irreducible.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "discordance", rename_all = "snake_case")]
pub enum Discordance {
    /// The two modalities measured different material. Admissible only when the readings' scopes
    /// bind the specimen dimension differently, or one leaves it unbound.
    DifferentSpecimen,
    /// The subject changed between the two acquisitions. Admissible only when the readings' scopes
    /// bind the time dimension differently, or one leaves it unbound.
    DifferentTime,
    /// One modality cannot see what the other reported. Admissible only when the named modality
    /// declares a detection floor *and* is the one reporting the lower value — a lens with a
    /// declared floor cannot explain away a finding it made itself.
    SensitivityLimit { modality: ModalityId },
    /// One modality examined a fragment and the other a survey. Admissible only when at least one
    /// lens declares [`SpatialExtent::Sampled`].
    SpatialSampling { modality: ModalityId },
    /// The two assays do not report the same thing. Admissible only when the two lenses declare
    /// different assay scopes.
    AssayScope,
    /// A handling fault before measurement degraded one reading. Always admissible: a specimen can
    /// always have been mishandled. See [`crate::preanalytic`].
    PreanalyticFault { stage: String },
    /// The material one modality measured was not the material it was labelled as. Always
    /// admissible for the same reason. See [`crate::lineage`].
    SpecimenIdentityError,
    /// Both readings are correct about their own scope and no single value reconciles them. Always
    /// admissible, and deliberately so: a hypothesis set from which this is excluded by
    /// construction cannot represent the situation 27.14 exists to pose.
    IrreducibleDiscordance,
}

impl Discordance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Discordance::DifferentSpecimen => "different_specimen",
            Discordance::DifferentTime => "different_time",
            Discordance::SensitivityLimit { .. } => "sensitivity_limit",
            Discordance::SpatialSampling { .. } => "spatial_sampling",
            Discordance::AssayScope => "assay_scope",
            Discordance::PreanalyticFault { .. } => "preanalytic_fault",
            Discordance::SpecimenIdentityError => "specimen_identity_error",
            Discordance::IrreducibleDiscordance => "irreducible_discordance",
        }
    }

    /// Which modality's reading this account would render non-authoritative, if any.
    ///
    /// [`Discordance::IrreducibleDiscordance`] and [`Discordance::AssayScope`] implicate nobody:
    /// under either, both readings are correct statements about what they measured. That is why
    /// there is no "correct modality" anywhere in this module — for two of the eight accounts the
    /// question has no answer.
    pub fn implicates(&self) -> Option<&ModalityId> {
        match self {
            Discordance::SensitivityLimit { modality }
            | Discordance::SpatialSampling { modality } => Some(modality),
            _ => None,
        }
    }
}

/// Why an account cannot apply to a particular contradiction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "inadmissible", rename_all = "snake_case")]
pub enum Inadmissible {
    #[serde(rename = "scopes_agree_on_dimension")]
    ScopesAgreeOnDimension {
        dimension: String,
    },
    NoDeclaredDetectionFloor {
        modality: ModalityId,
    },
    FloorBelongsToTheReportingModality {
        modality: ModalityId,
    },
    NoLensDeclaresSampling,
    AssayScopesAreIdentical {
        scope: String,
    },
    UnknownModality {
        modality: ModalityId,
    },
}

/// A candidate account, carrying the identifier the discriminating actions refer to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub account: Discordance,
}

impl Hypothesis {
    pub fn new(id: impl Into<String>, account: Discordance) -> Self {
        Hypothesis {
            id: HypothesisId::new(id),
            account,
        }
    }
}

/// The set-valued oracle of 27.14's workflow step 5.
///
/// A set, never a choice. [`HypothesisSet::sole`] returns an `Option` so that a caller who wants a
/// single answer has to handle the case where there is not one, rather than receiving a default.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HypothesisSet {
    members: BTreeMap<HypothesisId, Hypothesis>,
}

impl HypothesisSet {
    pub fn new() -> Self {
        HypothesisSet::default()
    }

    pub fn with(mut self, hypothesis: Hypothesis) -> Self {
        self.members.insert(hypothesis.id.clone(), hypothesis);
        self
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn contains(&self, id: &HypothesisId) -> bool {
        self.members.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Hypothesis> {
        self.members.values()
    }

    pub fn ids(&self) -> BTreeSet<HypothesisId> {
        self.members.keys().cloned().collect()
    }

    /// The single surviving account, when there is exactly one.
    ///
    /// Returns `None` for a set of two as readily as for a set of eight. A caller that wants to
    /// print an answer must decide what to do about that, which is the whole design.
    pub fn sole(&self) -> Option<&Hypothesis> {
        if self.members.len() == 1 {
            self.members.values().next()
        } else {
            None
        }
    }

    fn remove(&mut self, id: &HypothesisId) -> bool {
        self.members.remove(id).is_some()
    }
}

/// Evidence in the world that can refute some accounts.
///
/// 27.14's fourth required artifact, "discriminating actions".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscriminatingAction {
    pub evidence: EvidenceId,
    /// The accounts this action can rule out. An action that refutes nothing is not discriminating
    /// and [`next_actions`] ranks it last rather than hiding it.
    pub refutes: BTreeSet<HypothesisId>,
    /// A caller-supplied ordinal cost. Used only for deterministic tie-breaking; the blueprint
    /// fixes no cost model and this crate does not supply one.
    pub cost: u32,
}

impl DiscriminatingAction {
    pub fn new(evidence: impl Into<String>, cost: u32) -> Self {
        DiscriminatingAction {
            evidence: EvidenceId::new(evidence),
            refutes: BTreeSet::new(),
            cost,
        }
    }

    pub fn refuting(mut self, id: impl Into<String>) -> Self {
        self.refutes.insert(HypothesisId::new(id));
        self
    }
}

/// Whether evidence that would settle the question could be obtained at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "obtainability", rename_all = "snake_case")]
pub enum Obtainability {
    RequiresNewSpecimen,
    RequiresNewAcquisition,
    /// It cannot be obtained, and the reason is recorded. This is the honest terminus: some
    /// disagreements are permanent, and a benchmark that pretends otherwise trains agents to
    /// invent a resolution.
    NotObtainable {
        because: String,
    },
}

/// Evidence that would narrow the set and is not in the world.
///
/// Carried by [`ResolutionState::Unresolvable`] so that "we cannot tell" always comes with "here
/// is what would tell us". A bare `Unresolvable` is a shrug; this one is a study design.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingEvidence {
    pub description: String,
    pub would_refute: BTreeSet<HypothesisId>,
    pub obtainability: Obtainability,
}

/// A posed disagreement: two aligned readings that do not agree.
///
/// Constructed only by [`pose`], which is where scope alignment, the not-examined check and the
/// agreement check happen. There is no public constructor, so a `Contradiction` in hand is a
/// disagreement that survived those three checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Contradiction {
    left: Reading,
    right: Reading,
    quantity: QuantityId,
    overlap: ScopeKey,
}

impl Contradiction {
    pub fn left(&self) -> &Reading {
        &self.left
    }

    pub fn right(&self) -> &Reading {
        &self.right
    }

    pub fn quantity(&self) -> &QuantityId {
        &self.quantity
    }

    /// The scope over which the two readings are both valid, and therefore the scope the
    /// disagreement is about. Narrower than either reading's own scope.
    pub fn overlap(&self) -> &ScopeKey {
        &self.overlap
    }

    fn reading_of(&self, modality: &ModalityId) -> Option<&Reading> {
        if &self.left.modality == modality {
            Some(&self.left)
        } else if &self.right.modality == modality {
            Some(&self.right)
        } else {
            None
        }
    }

    fn other_than(&self, modality: &ModalityId) -> Option<&Reading> {
        if &self.left.modality == modality {
            Some(&self.right)
        } else if &self.right.modality == modality {
            Some(&self.left)
        } else {
            None
        }
    }

    fn differ_on(&self, dimension: &str) -> bool {
        match (
            self.left.scope.get(dimension),
            self.right.scope.get(dimension),
        ) {
            (Some(a), Some(b)) => a != b,
            (None, None) => false,
            _ => true,
        }
    }

    /// Whether an account can apply to this disagreement at all.
    ///
    /// 27.14's failure "discordance impossible biologically" made executable. The conditions are
    /// structural — they read the scopes and lenses that were declared — and none of them appeals
    /// to a fact about biology that this crate would have had to invent.
    pub fn admissibility(&self, account: &Discordance) -> Result<(), Inadmissible> {
        match account {
            Discordance::DifferentSpecimen => {
                if self.differ_on(SPECIMEN_DIMENSION) {
                    Ok(())
                } else {
                    Err(Inadmissible::ScopesAgreeOnDimension {
                        dimension: SPECIMEN_DIMENSION.to_string(),
                    })
                }
            }
            Discordance::DifferentTime => {
                if self.differ_on(TIME_DIMENSION) {
                    Ok(())
                } else {
                    Err(Inadmissible::ScopesAgreeOnDimension {
                        dimension: TIME_DIMENSION.to_string(),
                    })
                }
            }
            Discordance::SensitivityLimit { modality } => {
                let blind =
                    self.reading_of(modality)
                        .ok_or_else(|| Inadmissible::UnknownModality {
                            modality: modality.clone(),
                        })?;
                if blind.lens.detection_floor.is_none() {
                    return Err(Inadmissible::NoDeclaredDetectionFloor {
                        modality: modality.clone(),
                    });
                }
                let seeing =
                    self.other_than(modality)
                        .ok_or_else(|| Inadmissible::UnknownModality {
                            modality: modality.clone(),
                        })?;
                if reports_more_than(blind, seeing) {
                    Err(Inadmissible::FloorBelongsToTheReportingModality {
                        modality: modality.clone(),
                    })
                } else {
                    Ok(())
                }
            }
            Discordance::SpatialSampling { modality } => {
                if self.reading_of(modality).is_none() {
                    return Err(Inadmissible::UnknownModality {
                        modality: modality.clone(),
                    });
                }
                let sampled = [&self.left, &self.right]
                    .iter()
                    .any(|r| matches!(r.lens.spatial_extent, SpatialExtent::Sampled { .. }));
                if sampled {
                    Ok(())
                } else {
                    Err(Inadmissible::NoLensDeclaresSampling)
                }
            }
            Discordance::AssayScope => {
                if self.left.lens.assay_scope == self.right.lens.assay_scope {
                    Err(Inadmissible::AssayScopesAreIdentical {
                        scope: self.left.lens.assay_scope.clone(),
                    })
                } else {
                    Ok(())
                }
            }
            Discordance::PreanalyticFault { .. }
            | Discordance::SpecimenIdentityError
            | Discordance::IrreducibleDiscordance => Ok(()),
        }
    }
}

/// The scope dimension names this module reads.
///
/// Both are `bioprism_scope`'s canonical dimensions — `specimen` is classified
/// `ScopeClass::Specimen` and `time` `ScopeClass::Time` in its default registry — so this module
/// is not introducing a parallel vocabulary.
pub const SPECIMEN_DIMENSION: &str = "specimen";
/// See [`SPECIMEN_DIMENSION`].
pub const TIME_DIMENSION: &str = "time";

fn reports_more_than(candidate: &Reading, other: &Reading) -> bool {
    match (candidate.value(), other.value()) {
        (
            Some(ReadingValue::Interval { high: a, .. }),
            Some(ReadingValue::Interval { high: b, .. }),
        ) => a > b,
        _ => false,
    }
}

/// Pose two readings as a contradiction, or say why they are not one.
///
/// The three ways a pair fails to be a contradiction are each a distinct refusal, because each is a
/// different fact about the world:
///
/// * they report different quantities — no shared subject to disagree about;
/// * one modality did not look — an absence of evidence, not a conflict;
/// * their values are compatible — including two intervals that overlap, which is agreement within
///   the uncertainty the measurements actually declared;
///
/// and then the scope check, which is the one that matters: two readings whose scopes do not meet
/// describe different material or different instants, and the separating dimension is returned.
pub fn pose(left: Reading, right: Reading) -> Result<Contradiction, ContradictionRefusal> {
    if left.quantity != right.quantity {
        return Err(ContradictionRefusal::DifferentQuantities {
            left: left.quantity.to_string(),
            right: right.quantity.to_string(),
        });
    }
    for reading in [&left, &right] {
        if matches!(reading.reported, Reported::NotExamined) {
            return Err(ContradictionRefusal::ModalityNotExamined {
                modality: reading.modality.to_string(),
            });
        }
    }
    let overlap = match meet(&left.scope, &right.scope) {
        Meet::Scope(key) => key,
        Meet::Empty { dimension, reason } => {
            return Err(ContradictionRefusal::ScopesDoNotOverlap {
                dimension,
                reason: describe_empty(reason).to_string(),
            })
        }
        Meet::Conflict { dimension, .. } => {
            return Err(ContradictionRefusal::IncomparableScopes { dimension })
        }
    };
    let Some(lv) = left.value() else {
        return Err(ContradictionRefusal::InvalidReading {
            modality: left.modality.to_string(),
            detail: "the reading was not examined after passing the examined-reading gate".into(),
        });
    };
    let Some(rv) = right.value() else {
        return Err(ContradictionRefusal::InvalidReading {
            modality: right.modality.to_string(),
            detail: "the reading was not examined after passing the examined-reading gate".into(),
        });
    };
    if lv.agrees_with(rv) {
        return Err(ContradictionRefusal::ReadingsAgree {
            value: lv.describe(),
        });
    }
    let quantity = left.quantity.clone();
    Ok(Contradiction {
        left,
        right,
        quantity,
        overlap,
    })
}

fn describe_empty(reason: EmptyReason) -> &'static str {
    match reason {
        EmptyReason::UnequalExactValues => "the readings bind it to different values",
        EmptyReason::DisjointSets => "the readings bind it to disjoint sets of values",
        EmptyReason::EmptyInterval => "the readings' intervals do not intersect",
    }
}

/// What the author intended the disagreement to be — 27.14 workflow step 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscordanceClass {
    /// The modality pair disagrees at this rate routinely. Requires a reference distribution to
    /// mean anything, which is why [`expectedness`] refuses without one.
    Expected,
    /// The evidence in the world can narrow the set to one.
    Resolvable,
    /// It cannot, and that is the intended lesson.
    Irreducible,
}

impl DiscordanceClass {
    pub fn as_str(self) -> &'static str {
        match self {
            DiscordanceClass::Expected => "expected",
            DiscordanceClass::Resolvable => "resolvable",
            DiscordanceClass::Irreducible => "irreducible",
        }
    }
}

/// The base rate of disagreement for a modality pair — 27.14's fifth required artifact.
///
/// Counts, supplied by the caller from their own comparison series. This crate contains no such
/// series and computes no rate it has not been given the numerator and denominator for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceDiscordance {
    pub left: ModalityId,
    pub right: ModalityId,
    pub pairs_compared: u64,
    pub discordant: u64,
}

impl ReferenceDiscordance {
    pub fn new(
        left: impl Into<String>,
        right: impl Into<String>,
        pairs_compared: u64,
        discordant: u64,
    ) -> Self {
        ReferenceDiscordance {
            left: ModalityId::new(left),
            right: ModalityId::new(right),
            pairs_compared,
            discordant,
        }
    }

    /// The observed discordance rate in parts per ten thousand, integer-truncated.
    ///
    /// Integer so that two runs of the same program produce the same number on every platform.
    /// Returns `None` for an empty series rather than zero, because no comparisons is not a rate
    /// of nought.
    pub fn rate_per_ten_thousand(&self) -> Option<u64> {
        self.discordant
            .saturating_mul(10_000)
            .checked_div(self.pairs_compared)
    }

    fn covers(&self, a: &ModalityId, b: &ModalityId) -> bool {
        (&self.left == a && &self.right == b) || (&self.left == b && &self.right == a)
    }
}

/// Whether this disagreement is surprising, against a declared reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "expectedness", rename_all = "snake_case")]
pub enum Expectedness {
    /// The pair disagrees at least this often routinely.
    Routine { rate_per_ten_thousand: u64 },
    /// It does not.
    Notable { rate_per_ten_thousand: u64 },
}

/// Classify a disagreement against the program's reference distribution.
///
/// Refuses when no reference covers the modality pair. 27.14 lists the reference distribution as a
/// required artifact, and without one the word "unexpected" is an aesthetic judgement dressed as a
/// measurement. The threshold is the caller's, in the same integer units, because the blueprint
/// fixes none.
pub fn expectedness(
    program: &ContradictionProgram,
    notable_below_per_ten_thousand: u64,
) -> Result<Expectedness, ContradictionRefusal> {
    let left = &program.contradiction.left.modality;
    let right = &program.contradiction.right.modality;
    let reference = program
        .references
        .iter()
        .find(|r| r.covers(left, right))
        .and_then(|r| r.rate_per_ten_thousand())
        .ok_or_else(|| ContradictionRefusal::NoReferenceDistribution {
            left: left.to_string(),
            right: right.to_string(),
        })?;
    Ok(if reference < notable_below_per_ten_thousand {
        Expectedness::Notable {
            rate_per_ten_thousand: reference,
        }
    } else {
        Expectedness::Routine {
            rate_per_ten_thousand: reference,
        }
    })
}

/// A contradiction together with its hypothesis set, its discriminating actions and what has been
/// examined so far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContradictionProgram {
    contradiction: Contradiction,
    intent: DiscordanceClass,
    live: HypothesisSet,
    declared: HypothesisSet,
    actions: BTreeMap<EvidenceId, DiscriminatingAction>,
    examined: BTreeSet<EvidenceId>,
    missing: Vec<MissingEvidence>,
    references: Vec<ReferenceDiscordance>,
}

impl ContradictionProgram {
    /// Start a program from a posed contradiction.
    ///
    /// The hypothesis set starts full and is only ever narrowed by [`ContradictionProgram::examine`].
    /// There is no setter that replaces it.
    pub fn new(contradiction: Contradiction, intent: DiscordanceClass, set: HypothesisSet) -> Self {
        ContradictionProgram {
            contradiction,
            intent,
            live: set.clone(),
            declared: set,
            actions: BTreeMap::new(),
            examined: BTreeSet::new(),
            missing: Vec::new(),
            references: Vec::new(),
        }
    }

    pub fn with_action(mut self, action: DiscriminatingAction) -> Self {
        self.actions.insert(action.evidence.clone(), action);
        self
    }

    pub fn with_missing(mut self, missing: MissingEvidence) -> Self {
        self.missing.push(missing);
        self
    }

    pub fn with_reference(mut self, reference: ReferenceDiscordance) -> Self {
        self.references.push(reference);
        self
    }

    pub fn contradiction(&self) -> &Contradiction {
        &self.contradiction
    }

    pub fn intent(&self) -> DiscordanceClass {
        self.intent
    }

    /// The accounts still standing.
    pub fn live(&self) -> &HypothesisSet {
        &self.live
    }

    /// The accounts the author declared, before any narrowing. Kept so a result can report how far
    /// an agent got rather than only where it ended.
    pub fn declared(&self) -> &HypothesisSet {
        &self.declared
    }

    pub fn examined(&self) -> &BTreeSet<EvidenceId> {
        &self.examined
    }

    pub fn missing_evidence(&self) -> &[MissingEvidence] {
        &self.missing
    }

    /// Actions in the world that have not been examined yet.
    pub fn unexamined(&self) -> Vec<&DiscriminatingAction> {
        self.actions
            .values()
            .filter(|a| !self.examined.contains(&a.evidence))
            .collect()
    }

    /// Examine one piece of evidence and remove the accounts it refutes.
    ///
    /// Refuses to leave the set empty. An action that refutes every remaining account has not
    /// resolved the contradiction — it has proved that the world contains no account of its own
    /// contents, which is a defect in the program. Reporting that as a confident resolution is
    /// exactly the silent reconciliation this module exists to prevent, so the narrowing is not
    /// applied and the caller is told.
    pub fn examine(
        &mut self,
        evidence: &EvidenceId,
    ) -> Result<&HypothesisSet, ContradictionRefusal> {
        let Some(action) = self.actions.get(evidence) else {
            return Ok(&self.live);
        };
        let doomed: Vec<HypothesisId> = action
            .refutes
            .iter()
            .filter(|id| self.live.contains(id))
            .cloned()
            .collect();
        if !doomed.is_empty() && doomed.len() == self.live.len() {
            return Err(ContradictionRefusal::AllHypothesesRefuted {
                discriminator: evidence.to_string(),
            });
        }
        for id in doomed {
            self.live.remove(&id);
        }
        self.examined.insert(evidence.clone());
        Ok(&self.live)
    }

    /// Assert that one modality is simply correct.
    ///
    /// Always refuses. The method exists so that the refusal has somewhere to live and so that
    /// 27.14's failure "one modality arbitrarily labeled correct" is a compile-time-visible part of
    /// the API rather than a sentence in a document nobody reads. Narrowing goes through
    /// [`ContradictionProgram::examine`], which requires a discriminator that names what it
    /// refutes.
    pub fn prefer_modality(&self, modality: &ModalityId) -> ContradictionRefusal {
        ContradictionRefusal::ModalityPreferredWithoutEvidence {
            modality: modality.to_string(),
        }
    }

    /// Where the program stands. See the module header for why there are three of these.
    pub fn state(&self) -> ResolutionState {
        let unexamined: Vec<DiscriminatingAction> =
            self.unexamined().into_iter().cloned().collect();
        if !unexamined.is_empty() {
            return ResolutionState::NotYetExamined {
                available: unexamined,
            };
        }
        if self.live.len() <= 1 && !self.examined.is_empty() {
            return ResolutionState::Resolved {
                by: self.examined.iter().cloned().collect(),
                surviving: self.live.clone(),
            };
        }
        if self.examined.is_empty() && self.actions.is_empty() {
            return ResolutionState::Unresolvable {
                examined: Vec::new(),
                would_resolve: self.missing.clone(),
            };
        }
        if self.live.len() > 1 {
            return ResolutionState::Unresolvable {
                examined: self.examined.iter().cloned().collect(),
                would_resolve: self.missing.clone(),
            };
        }
        ResolutionState::Resolved {
            by: self.examined.iter().cloned().collect(),
            surviving: self.live.clone(),
        }
    }
}

/// The three states. See the module header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ResolutionState {
    Resolved {
        by: Vec<EvidenceId>,
        surviving: HypothesisSet,
    },
    /// Discriminating evidence exists in the world and has not been looked at. Never returned
    /// alongside a claim that the question cannot be answered.
    NotYetExamined {
        available: Vec<DiscriminatingAction>,
    },
    Unresolvable {
        examined: Vec<EvidenceId>,
        would_resolve: Vec<MissingEvidence>,
    },
}

impl ResolutionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionState::Resolved { .. } => "resolved",
            ResolutionState::NotYetExamined { .. } => "not_yet_examined",
            ResolutionState::Unresolvable { .. } => "unresolvable",
        }
    }
}

/// An action ranked by how many live accounts it could refute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankedAction {
    pub evidence: EvidenceId,
    /// How many currently-live accounts this action can rule out.
    pub refutes_live: usize,
    pub cost: u32,
}

/// Order the unexamined actions by how much they could narrow the live set.
///
/// 27.14's validation item "information-gain ordering", implemented as a **set-cover surrogate**:
/// the count of live accounts an action refutes, descending, then cost ascending, then identifier.
/// It is not information gain in the Shannon sense and this crate does not claim it is — there is
/// no probability distribution over the hypothesis set anywhere in the workspace, and computing an
/// entropy from a uniform prior nobody declared would dress an arbitrary choice as a measurement.
pub fn next_actions(program: &ContradictionProgram) -> Vec<RankedAction> {
    let mut ranked: Vec<RankedAction> = program
        .unexamined()
        .into_iter()
        .map(|action| RankedAction {
            evidence: action.evidence.clone(),
            refutes_live: action
                .refutes
                .iter()
                .filter(|id| program.live.contains(id))
                .count(),
            cost: action.cost,
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.refutes_live
            .cmp(&a.refutes_live)
            .then(a.cost.cmp(&b.cost))
            .then(a.evidence.cmp(&b.evidence))
    });
    ranked
}

/// A way the seeded answer can be read off the surface of the world.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "cue", rename_all = "snake_case")]
pub enum Cue {
    /// An account's identifier or name appears verbatim in a reading's annotations.
    AccountNamedInAnnotation { modality: String, account: String },
    /// The authored intent leaked into the artifacts.
    IntentNamedInAnnotation { modality: String, intent: String },
    /// Exactly one reading carries annotations at all, so an agent can find "the interesting one"
    /// without reading them. 27.14's failure "agent can read mutation metadata" does not require
    /// the metadata to be legible — asymmetric presence is enough.
    SoleAnnotatedReading { modality: String },
    /// Exactly one reading declares a detection floor, and an account implicates that same
    /// modality. The structural asymmetry points at the answer.
    SoleDeclaredFloorMatchesAccount { modality: String, account: String },
}

/// Scan a program for ways its answer can be guessed from surface structure.
///
/// 27.14's validation item "answer-cue scan". Every check here is structural and deterministic;
/// none of them is a claim that a *particular* agent would exploit the cue, only that the cue
/// exists. [`validate`] treats any finding as a refusal, because a benchmark that is solvable
/// without reasoning measures something other than what it claims to.
pub fn cue_scan(program: &ContradictionProgram) -> Vec<Cue> {
    let readings = [&program.contradiction.left, &program.contradiction.right];
    let mut cues = Vec::new();

    for reading in readings {
        for note in &reading.annotations {
            let lowered = note.to_lowercase();
            for hypothesis in program.declared.iter() {
                if lowered.contains(hypothesis.account.as_str())
                    || lowered.contains(&hypothesis.id.as_str().to_lowercase())
                {
                    cues.push(Cue::AccountNamedInAnnotation {
                        modality: reading.modality.to_string(),
                        account: hypothesis.account.as_str().to_string(),
                    });
                }
            }
            if lowered.contains(program.intent.as_str()) {
                cues.push(Cue::IntentNamedInAnnotation {
                    modality: reading.modality.to_string(),
                    intent: program.intent.as_str().to_string(),
                });
            }
        }
    }

    let annotated: Vec<&Reading> = readings
        .iter()
        .copied()
        .filter(|r| !r.annotations.is_empty())
        .collect();
    if annotated.len() == 1 {
        cues.push(Cue::SoleAnnotatedReading {
            modality: annotated[0].modality.to_string(),
        });
    }

    let with_floor: Vec<&Reading> = readings
        .iter()
        .copied()
        .filter(|r| r.lens.detection_floor.is_some())
        .collect();
    if with_floor.len() == 1 {
        for hypothesis in program.declared.iter() {
            if hypothesis.account.implicates() == Some(&with_floor[0].modality) {
                cues.push(Cue::SoleDeclaredFloorMatchesAccount {
                    modality: with_floor[0].modality.to_string(),
                    account: hypothesis.account.as_str().to_string(),
                });
            }
        }
    }

    cues.sort();
    cues.dedup();
    cues
}

/// Whether the authored intent matches what the world's evidence can actually do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "intent_check", rename_all = "snake_case")]
pub enum IntentCheck {
    Consistent,
    /// The author said resolvable; examining every action in the world still leaves this many
    /// accounts standing.
    DeclaredResolvableButCannotNarrow {
        remaining: usize,
    },
    /// The author said irreducible; the evidence already in the world settles it.
    DeclaredIrreducibleButEvidenceSettlesIt {
        by: Vec<EvidenceId>,
    },
}

/// Check the authored intent against the discriminating actions the world actually contains.
///
/// This is the executable half of 27.14's workflow step 2. An author declares a discordance
/// "resolvable" or "irreducible"; this runs every action in the world and reports whether the
/// declaration survives contact with the evidence. It uses the actions' declared `refutes` sets
/// and nothing else, so it is exact rather than a simulation.
pub fn check_intent(program: &ContradictionProgram) -> IntentCheck {
    let mut exhausted = program.clone();
    let all: Vec<EvidenceId> = program.actions.keys().cloned().collect();
    let mut used = Vec::new();
    for evidence in &all {
        if exhausted.examine(evidence).is_ok() {
            used.push(evidence.clone());
        }
    }
    let remaining = exhausted.live.len();
    match program.intent {
        DiscordanceClass::Resolvable if remaining > 1 => {
            IntentCheck::DeclaredResolvableButCannotNarrow { remaining }
        }
        DiscordanceClass::Irreducible if remaining <= 1 && !used.is_empty() => {
            IntentCheck::DeclaredIrreducibleButEvidenceSettlesIt { by: used }
        }
        _ => IntentCheck::Consistent,
    }
}

/// A program that has passed 27.14's validation list.
///
/// No public constructor and no `Deserialize`, so an unvalidated program does not typecheck as a
/// benchmark. The same device `bioprism_oncoworlds::models::PatientRelevantClaim` uses, for the
/// same reason: the check is not optional if the only way to obtain the type is to run it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidatedProgram {
    program: ContradictionProgram,
    admissible: BTreeMap<HypothesisId, Discordance>,
    intent_check: IntentCheck,
}

impl ValidatedProgram {
    pub fn program(&self) -> &ContradictionProgram {
        &self.program
    }

    pub fn into_program(self) -> ContradictionProgram {
        self.program
    }

    /// The accounts that survived the admissibility check, with the reason each is possible left
    /// implicit in its own variant's documented condition.
    pub fn admissible(&self) -> &BTreeMap<HypothesisId, Discordance> {
        &self.admissible
    }

    pub fn intent_check(&self) -> &IntentCheck {
        &self.intent_check
    }
}

/// Run 27.14's validation list over a program.
///
/// In order: every declared account must be admissible against the posed contradiction; at least
/// one must survive; the answer-cue scan must find nothing. The intent check is *recorded* rather
/// than enforced, because a mismatch between what an author intended and what their evidence
/// supports is a finding about the program worth publishing, not necessarily a defect that should
/// block it — but it travels inside [`ValidatedProgram`] where a result card can read it.
///
/// Inadmissible accounts are dropped rather than fatal, which is 27.14's workflow step 3 read
/// literally: the author proposes plausible explanations and validation decides which are
/// plausible *here*. If none are, the program described a disagreement that cannot happen and
/// [`ContradictionRefusal::NoAdmissibleExplanation`] says so.
///
/// The cue scan is strict on purpose and one of its findings has a practical remedy worth stating:
/// [`Cue::SoleDeclaredFloorMatchesAccount`] fires whenever exactly one lens declares a detection
/// floor and an account implicates that modality, because the structural asymmetry points at the
/// answer. The fix is to declare a floor for both lenses — which a real world would have anyway,
/// and the absence of which is itself a gap in 27.14's "modality-specific lenses".
pub fn validate(program: ContradictionProgram) -> Result<ValidatedProgram, ContradictionRefusal> {
    let mut admissible = BTreeMap::new();
    for hypothesis in program.declared.iter() {
        if program
            .contradiction
            .admissibility(&hypothesis.account)
            .is_ok()
        {
            admissible.insert(hypothesis.id.clone(), hypothesis.account.clone());
        }
    }
    if admissible.is_empty() {
        return Err(ContradictionRefusal::NoAdmissibleExplanation {
            left: program.contradiction.left.modality.to_string(),
            right: program.contradiction.right.modality.to_string(),
        });
    }
    if let Some(cue) = cue_scan(&program).first() {
        return Err(ContradictionRefusal::TrivialCue {
            cue: serde_json::to_string(cue).unwrap_or_else(|_| "unserialisable cue".to_string()),
        });
    }
    let intent_check = check_intent(&program);
    Ok(ValidatedProgram {
        program,
        admissible,
        intent_check,
    })
}
