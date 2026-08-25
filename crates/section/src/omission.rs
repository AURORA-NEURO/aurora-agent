//! The omission manifest.
//!
//! This is the load-bearing idea of blueprint 43.26. A compact context is dangerous when nobody
//! can tell what was excluded, so omitted evidence is grouped by *structural reason* and each
//! group is assigned an influence class.
//!
//! The distinction the specification refuses to let slide: [`InfluenceClass::Zero`] means the
//! omission provably cannot change the decision, while [`InfluenceClass::Unknown`] means nobody
//! checked. Only [`InfluenceClass::Zero`] and [`InfluenceClass::Bounded`] support a sufficiency
//! claim; a manifest containing any `Unknown` group must not be labelled sufficient.
//!
//! ## Why `Bounded` carries a validated bound rather than an `f64`
//!
//! A bound of `1.0` on a quantity that lives in `[0, 1]` is sound and permits every answer. Read
//! as a class it is reassuring, read as a number it constrains nothing, and a manifest full of
//! such groups is formally sufficient and practically empty. The predicate that separates the two
//! is not new — it was already written down as `bioprism_influence::manifest::is_informative` —
//! but it lived at the *fold*, where a group had already been built and pushed. A predicate at the
//! fold is a check a caller can forget to run; [`InformativeBound`] is the same predicate placed
//! where it cannot be forgotten, because there is no way to name a vacuous value in the type.
//!
//! Three gates enforce it, because [`OmissionGroup`]'s fields are public and a struct literal
//! bypasses any constructor. [`OmissionGroup::bounded`] is the sanctioned constructor and takes an
//! [`InformativeBound`]; [`OmissionManifest::push`] and [`OmissionManifest::from_groups`] refuse to
//! *admit* a `Bounded` claim whose bound is absent or vacuous, recording the refusal in the group's
//! reason; and deserialisation runs the same admission check, because parsing is how a group enters
//! a *verifier*. This crate deliberately depends on neither `world` nor `fiber` so that a consumer
//! can check a certificate without linking the engine that produced it, and that consumer's
//! `OmissionGroup` values all arrive through serde. A gate that covered only the compiler would
//! have left the verification path — the one facing untrusted bytes — completely open.
//!
//! The fold itself is left alone: [`OmissionManifest::supports_sufficiency_claim`] still reads the
//! class and nothing else, which is what makes it auditable.
//!
//! What is *not* closed, stated rather than implied: [`OmissionGroup`]'s fields and
//! [`OmissionManifest::groups`] are public, so `manifest.groups.push(group)` still writes an
//! unadmitted group straight into a manifest. Making those fields private is the change that would
//! make a vacuous claim unrepresentable, and it is a breaking API change across the thirteen crates
//! that build groups by struct literal. Until that lands, the invariant holds on every path a group
//! can travel *into* a manifest through this module's own API, and on no other.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfluenceClass {
    /// No dependency path reaches the target; excluding it cannot move the decision.
    ///
    /// This is a *proof*, and the two ways of arriving at it are not equally good. A group whose
    /// members were each shown to provide no variable the slice needs has earned the class. A group
    /// arrived at by subtracting every other group's cardinality from a total has earned it only if
    /// the other groups are exhaustive, and a population nobody thought of lands here silently and
    /// with a bound of zero. [`ProvenUnreachable`] is where that obligation is discharged; a caller
    /// that cannot discharge it belongs in [`InfluenceClass::Unknown`], which costs the sufficiency
    /// claim and is the honest price.
    Zero,
    /// Influence is non-zero but bounded by a stated quantity that excludes something.
    ///
    /// A group only reaches a manifest in this class through [`OmissionGroup::bounded`], or by
    /// surviving [`OmissionManifest::push`]'s admission check with an [`InformativeBound`]-valued
    /// `bound`. A vacuous bound is not a bound and is admitted as [`InfluenceClass::Unknown`].
    Bounded,
    /// Policy or consent forbids access. The decision must account for the gap, not ignore it.
    InaccessibleByPolicy,
    /// Not available at the temporal cut; may become available later.
    DeferredAcquisition,
    /// Not analysed. Never counts toward sufficiency.
    Unknown,
}

impl InfluenceClass {
    /// Whether a group in this class may participate in a sufficiency claim.
    pub fn supports_sufficiency(self) -> bool {
        matches!(self, InfluenceClass::Zero | InfluenceClass::Bounded)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            InfluenceClass::Zero => "zero",
            InfluenceClass::Bounded => "bounded",
            InfluenceClass::InaccessibleByPolicy => "inaccessible_by_policy",
            InfluenceClass::DeferredAcquisition => "deferred_acquisition",
            InfluenceClass::Unknown => "unknown",
        }
    }
}

/// A bound that excludes at least one answer.
///
/// The field is private and [`InformativeBound::new`] is the only constructor, so a value that
/// permits everything cannot be named. The admissible range is `[0.0, 1.0)`: the metric is a
/// distance between answer distributions, `1.0` is its maximum, and a ceiling at the maximum is a
/// statement that nothing was ruled out. Zero is admissible and is *not* the same claim as
/// [`InfluenceClass::Zero`] — a computed `0.0` says a dependency path exists and this perturbation
/// happened not to travel down it, which is a measurement, while `Zero` says no path exists at all.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct InformativeBound(f64);

impl InformativeBound {
    /// The bound, or `None` when the value carries no information.
    ///
    /// `None` is returned rather than a clamped value on purpose. Clamping would manufacture a
    /// claim the caller never computed, and the caller's honest move on refusal is to fall back to
    /// [`InfluenceClass::Unknown`], not to a tighter number nobody derived.
    pub fn new(value: f64) -> Option<InformativeBound> {
        (value.is_finite() && (0.0..1.0).contains(&value)).then_some(InformativeBound(value))
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

/// A count of omissions each of which has been shown to reach no target.
///
/// The field is private and [`ProvenUnreachable::remainder`] is the only constructor, so a
/// [`InfluenceClass::Zero`] count cannot be a bare `usize` a caller arrived at by whatever
/// arithmetic was to hand.
///
/// The v0.1 compiler cannot enumerate the omitted population: `total_facts - |selection|` is a
/// subtraction precisely so that compile cost tracks the compiled region rather than the corpus
/// (43.34), and materialising the complement would reintroduce the whole-world traversal the design
/// rejects. So the zero-influence count *is* a remainder, and the only question that matters is
/// whether everything that is not provably zero has been taken out of it first. This constructor
/// makes that question unavoidable by taking `still_reaching` — the number of omitted facts that do
/// provide a variable the slice needs — as a separate argument and doing the subtraction itself. A
/// caller who has not computed that population cannot pass it, and a caller who passes one larger
/// than the total gets `None` rather than a saturated zero, because an accounting error that
/// silently becomes "everything is provably irrelevant" is the worst available failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvenUnreachable(usize);

impl ProvenUnreachable {
    /// The omitted facts left once every fact still reaching a needed variable is removed.
    ///
    /// `None` on underflow. Facts withheld by policy or by the temporal cut must already have been
    /// removed from `omitted` by the caller: they have their own classes, and they are omissions
    /// nobody proved irrelevant either.
    pub fn remainder(omitted: usize, still_reaching: usize) -> Option<ProvenUnreachable> {
        omitted.checked_sub(still_reaching).map(ProvenUnreachable)
    }

    pub fn count(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "UnadmittedGroup")]
pub struct OmissionGroup {
    /// Structural reason this family was excluded.
    pub reason: String,
    pub influence: InfluenceClass,
    pub count: usize,
    /// A bound on the decision distortion this group can cause, when `influence` is
    /// [`InfluenceClass::Bounded`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<f64>,
    /// Representative members, for a human reading the receipt. Never the whole list: large
    /// manifests are content-addressed rather than inlined.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
}

impl OmissionGroup {
    /// The sanctioned constructor for a group in [`InfluenceClass::Bounded`].
    ///
    /// Infallible because the caller has already discharged the obligation by producing an
    /// [`InformativeBound`]; there is no branch here in which a vacuous claim could be built.
    pub fn bounded(
        reason: impl Into<String>,
        count: usize,
        bound: InformativeBound,
        examples: impl IntoIterator<Item = String>,
    ) -> OmissionGroup {
        OmissionGroup {
            reason: reason.into(),
            influence: InfluenceClass::Bounded,
            count,
            bound: Some(bound.value()),
            examples: examples.into_iter().collect(),
        }
    }

    /// The sanctioned constructor for a group in [`InfluenceClass::Zero`].
    ///
    /// `bound` is `Some(0.0)` and is a restatement of the class rather than a measurement — there
    /// is no dependency path down which a perturbation could have travelled, so there was nothing
    /// to measure. [`OmissionGroup::has_informative_bound`] is false here on purpose, and the
    /// difference between this zero and a *computed* zero is exactly the difference the manifest
    /// exists to keep.
    pub fn structurally_zero(
        reason: impl Into<String>,
        proven: ProvenUnreachable,
        examples: impl IntoIterator<Item = String>,
    ) -> OmissionGroup {
        OmissionGroup {
            reason: reason.into(),
            influence: InfluenceClass::Zero,
            count: proven.count(),
            bound: Some(0.0),
            examples: examples.into_iter().collect(),
        }
    }

    /// Whether this group's `bound` field would survive [`InformativeBound::new`].
    ///
    /// True only for a `Bounded` group: the field is defined for that class alone, and a `0.0`
    /// sitting on a `Zero` group is a restatement of the class rather than a measurement.
    pub fn has_informative_bound(&self) -> bool {
        self.influence == InfluenceClass::Bounded
            && self.bound.and_then(InformativeBound::new).is_some()
    }

    /// The group as a manifest is willing to hold it.
    ///
    /// A `Bounded` claim without an informative bound is downgraded to [`InfluenceClass::Unknown`]
    /// and the refused value is written into the reason, so the certificate says a bound was
    /// offered and refused rather than silently losing it. Nothing else is rewritten: this is an
    /// admission check on one claim, not a normalisation pass over the manifest.
    pub fn admitted(mut self) -> OmissionGroup {
        if self.influence != InfluenceClass::Bounded || self.has_informative_bound() {
            return self;
        }
        let refused = match self.bound {
            Some(value) => format!("a bound of {value} permits every answer"),
            None => "a bounded class was claimed with no bound at all".to_string(),
        };
        self.reason = format!(
            "{}; refused as a sufficiency-supporting bound because {refused}",
            self.reason
        );
        self.influence = InfluenceClass::Unknown;
        self.bound = None;
        self
    }
}

/// An omission group exactly as it appeared on the wire, before admission.
///
/// Deserialisation cannot go straight into [`OmissionGroup`] without making the admission check
/// skippable by anyone holding bytes, and it cannot call [`OmissionGroup::admitted`] from inside a
/// hand-written `Deserialize` without duplicating the field list twice over. `#[serde(from)]` over
/// this shape is the smaller of the two: one private mirror, and the conversion *is* the check.
///
/// The field set must track [`OmissionGroup`]'s, including the `default` attributes, or a document
/// the old derive accepted would start failing to parse.
#[derive(Deserialize)]
struct UnadmittedGroup {
    reason: String,
    influence: InfluenceClass,
    count: usize,
    #[serde(default)]
    bound: Option<f64>,
    #[serde(default)]
    examples: Vec<String>,
}

impl From<UnadmittedGroup> for OmissionGroup {
    fn from(wire: UnadmittedGroup) -> OmissionGroup {
        OmissionGroup {
            reason: wire.reason,
            influence: wire.influence,
            count: wire.count,
            bound: wire.bound,
            examples: wire.examples,
        }
        .admitted()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OmissionManifest {
    pub groups: Vec<OmissionGroup>,
}

impl OmissionManifest {
    /// Admits a group, refusing any vacuous `Bounded` claim it carries.
    ///
    /// The check lives here and not in [`Self::supports_sufficiency_claim`] because a fold that
    /// second-guesses its own inputs cannot be audited: a reader of a manifest would have to know
    /// which of the two disagreed with the other. Admission is the last point at which the group
    /// can still be corrected while the manifest a reader sees stays a faithful transcript of what
    /// it holds.
    pub fn push(&mut self, group: OmissionGroup) {
        self.groups.push(group.admitted());
    }

    /// Builds a manifest from groups already in hand, admitting each one.
    ///
    /// Exists because the one-shot `OmissionManifest { groups: vec![…] }` literal is the shape
    /// callers reach for and the shape that skips [`Self::push`]. Offering the admitting version
    /// under a name that is no harder to type is the only lever this module has while the field
    /// stays public.
    pub fn from_groups(groups: impl IntoIterator<Item = OmissionGroup>) -> OmissionManifest {
        OmissionManifest {
            groups: groups.into_iter().map(OmissionGroup::admitted).collect(),
        }
    }

    pub fn total_omitted(&self) -> usize {
        self.groups.iter().map(|g| g.count).sum()
    }

    pub fn count_in(&self, class: InfluenceClass) -> usize {
        self.groups
            .iter()
            .filter(|g| g.influence == class)
            .map(|g| g.count)
            .sum()
    }

    /// True only when every group is provably zero-influence or explicitly bounded.
    ///
    /// A manifest with any unknown, policy-blocked or deferred group is *not* sufficient, and
    /// the compiler must abstain or refine rather than present the context as complete.
    pub fn supports_sufficiency_claim(&self) -> bool {
        self.groups
            .iter()
            .all(|g| g.influence.supports_sufficiency())
    }

    pub fn blocking_groups(&self) -> impl Iterator<Item = &OmissionGroup> {
        self.groups
            .iter()
            .filter(|g| !g.influence.supports_sufficiency())
    }
}
