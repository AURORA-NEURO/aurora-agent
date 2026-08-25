//! Subclonal structure, specimen sampling and resistance (30.12).
//!
//! Blueprint 30.12 asks for reasoning about "subclones, regional variation, treatment selection,
//! recurrence, and resistance without equating observed allele frequencies with a complete
//! phylogeny". The single fact that shapes this module is that **a tumour is not one thing**: a
//! measurement on a specimen is a measurement on a sample of a heterogeneous population, and the
//! sample is small, regional, and taken at one moment.
//!
//! # The asymmetry between presence and absence
//!
//! Detection and non-detection are not mirror images, and this module's types say so.
//!
//! * A marker **detected** in a fragment of a tumour is present in that tumour. The fragment came
//!   from the tumour, so the existential claim promotes.
//! * A marker **not detected** in a fragment says nothing about regions that were not sampled, and
//!   inside the sampled regions it bounds only what the assay could have seen. It never promotes.
//!
//! So [`TumourClaim`] has a variant for presence and a variant for a *bound*, and none for
//! absence. There is no constructor, no `Deserialize` path and no combinator anywhere in this
//! crate that yields "this marker is absent from this tumour", because no sampling of a
//! heterogeneous population supports that sentence. [`SpecimenObservation::as_tumour_claim`] is
//! the only route from a specimen to a tumour-level statement, and its negative branch always
//! lands on [`TumourClaim::UndetectedAboveFraction`] carrying the fraction and the regions.
//!
//! This is `bioprism_lens`'s missingness discipline in the currency 30.12 uses, and it inherits
//! `bioprism_onco`'s [`bioprism_onco::ObservationStatus`] wholesale: an assay that never ran is
//! not a negative result, and [`bioprism_onco::ObservationStatus::BelowDetection`] — the one that
//! `is_informative_about_the_value` — is the only unobserved status that produces a bound at all.
//!
//! # Resistance is selected, not created
//!
//! The module's worked microbenchmark is a resistance-associated alteration "absent at diagnosis
//! under low depth and present at recurrence", where the agent "must compare de novo emergence,
//! prior undetected subclone, and sampling explanations".
//! [`explain_new_alteration`] returns the set of explanations the evidence does not exclude, and
//! [`ExplanationSet::sole`] refuses when more than one survives. An explanation leaves the set
//! only for an arithmetic reason — the diagnosis sampling covered the recurrence's regions, or the
//! diagnosis assay could have seen a subclone at the recurrence's fraction — never by preference.
//!
//! # Not implemented, deliberately
//!
//! - **No phylogeny inference.** [`ClonalHistory`] is a hypothesis a caller supplies;
//!   [`ClonalHistory::check`] audits it against the observed fractions. Enumerating trees from
//!   variant data is a research problem, and 30.12 names "overfitting one phylogeny" as a failure
//!   precisely because tools return one.
//! - **No allele-fraction arithmetic.** [`FractionEvidence`] records whether the caller declared
//!   purity, local copy number and multiplicity; it does not convert a variant allele fraction
//!   into a cellular fraction. 30.12 names "ignoring copy-number effects on allele fraction" as a
//!   failure, and the conversion belongs to whichever variant caller has the copy-number model.
//! - **No causal inference.** [`attribute_to_treatment`] always refuses, because temporal
//!   association is the only relation this module represents and 30.12 names "claiming treatment
//!   causation from temporal association" as a failure.
//! - **No detection-limit defaults.** [`DetectionSensitivity`] must be declared by the caller. No
//!   depth, VAF cutoff or purity threshold appears anywhere in this file; the blueprint states
//!   none and a fabricated one would be presented as domain knowledge.

use crate::error::{FractionError, PhylogenyRefusal, PromotionRefusal};
use bioprism_onco::{MarkerCall, MolecularMarker, Observed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A fraction of the tumour's cancer cells, in parts per ten thousand.
///
/// Integer-backed on purpose. This crate is deterministic and its invariants are comparisons and
/// sums of fractions; carrying those in `f64` would make "these children sum to their parent" a
/// question about rounding. The unit is a representation choice, not a claim about assay
/// resolution — nothing here asserts that ten thousandths are measurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CellularFraction {
    parts_per_ten_thousand: u16,
}

impl CellularFraction {
    pub const WHOLE: CellularFraction = CellularFraction {
        parts_per_ten_thousand: 10_000,
    };
    pub const NONE: CellularFraction = CellularFraction {
        parts_per_ten_thousand: 0,
    };

    pub const fn from_parts_per_ten_thousand(parts: u16) -> Result<Self, FractionError> {
        if parts > 10_000 {
            return Err(FractionError::AboveWhole {
                parts: parts as u32,
            });
        }
        Ok(CellularFraction {
            parts_per_ten_thousand: parts,
        })
    }

    /// From a ratio in `[0, 1]`, rounding half away from zero to the nearest ten-thousandth.
    pub fn from_ratio(value: f64) -> Result<Self, FractionError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(FractionError::NotAUnitRatio {
                value: format!("{value}"),
            });
        }
        let parts = (value * 10_000.0).round();
        CellularFraction::from_parts_per_ten_thousand(parts as u16)
    }

    pub const fn parts_per_ten_thousand(self) -> u16 {
        self.parts_per_ten_thousand
    }

    fn describe(self) -> String {
        format!("{}/10000", self.parts_per_ten_thousand)
    }
}

/// A subclone label. Opaque: 30.12 supplies no naming scheme and this crate invents none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SubcloneId(String);

impl SubcloneId {
    pub fn new(value: impl Into<String>) -> Self {
        SubcloneId(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One subclone: a cancer-cell fraction and the alterations that define it.
///
/// The marker vocabulary is `bioprism_onco::MolecularMarker`, which that crate documents as a
/// worked instantiation rather than a blueprint enumeration. Nothing here adds to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subclone {
    pub id: SubcloneId,
    /// Cancer-cell fraction: the share of tumour cells carrying this subclone's alterations,
    /// inclusive of its descendants.
    pub fraction: CellularFraction,
    pub alterations: BTreeSet<MolecularMarker>,
    /// Regions in which this subclone has been observed. Empty means "not observed anywhere",
    /// which for a latent population is a normal state, not a defect.
    #[serde(default)]
    pub observed_in: BTreeSet<RegionId>,
}

impl Subclone {
    pub fn new(id: SubcloneId, fraction: CellularFraction) -> Self {
        Subclone {
            id,
            fraction,
            alterations: BTreeSet::new(),
            observed_in: BTreeSet::new(),
        }
    }

    pub fn carrying(mut self, marker: MolecularMarker) -> Self {
        self.alterations.insert(marker);
        self
    }
}

/// A region of the tumour, as named by whoever recorded the sampling map.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegionId(String);

impl RegionId {
    pub fn new(value: impl Into<String>) -> Self {
        RegionId(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The latent population a specimen is a sample of.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TumourPopulation {
    subclones: BTreeMap<SubcloneId, Subclone>,
}

impl TumourPopulation {
    pub fn new() -> Self {
        TumourPopulation::default()
    }

    pub fn with(mut self, subclone: Subclone) -> Self {
        self.subclones.insert(subclone.id.clone(), subclone);
        self
    }

    pub fn get(&self, id: &SubcloneId) -> Option<&Subclone> {
        self.subclones.get(id)
    }

    pub fn subclones(&self) -> impl Iterator<Item = &Subclone> {
        self.subclones.values()
    }

    /// Subclones carrying a marker, whatever their fraction.
    pub fn carrying(&self, marker: MolecularMarker) -> Vec<&Subclone> {
        self.subclones
            .values()
            .filter(|subclone| subclone.alterations.contains(&marker))
            .collect()
    }
}

/// A hypothesised ancestry over the subclones: `(parent, child)` edges.
///
/// A hypothesis, not a derivation. 30.12's ladder asks a system to "construct compatible clonal
/// histories" — plural — and its metric is "tree or partial-order compatibility", so the type this
/// module supports is *is this history compatible with the fractions*, not *what is the history*.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonalHistory {
    edges: BTreeSet<(SubcloneId, SubcloneId)>,
}

impl ClonalHistory {
    pub fn new() -> Self {
        ClonalHistory::default()
    }

    pub fn descends(mut self, parent: SubcloneId, child: SubcloneId) -> Self {
        self.edges.insert((parent, child));
        self
    }

    pub fn edges(&self) -> impl Iterator<Item = (&SubcloneId, &SubcloneId)> {
        self.edges.iter().map(|(parent, child)| (parent, child))
    }

    /// Whether this history is arithmetically compatible with the observed fractions.
    ///
    /// Three checks, all arithmetic and none of them a threshold:
    /// every edge endpoint exists; the edges are acyclic; and a parent's cancer-cell fraction is
    /// at least the sum of its children's, because a nested subclone's cells are a subset of its
    /// ancestor's. Root fractions must together fit inside the tumour.
    pub fn check(&self, population: &TumourPopulation) -> Result<(), PhylogenyRefusal> {
        for (parent, child) in &self.edges {
            for id in [parent, child] {
                if population.get(id).is_none() {
                    return Err(PhylogenyRefusal::UnknownSubclone {
                        subclone: id.as_str().to_string(),
                    });
                }
            }
        }
        self.check_acyclic()?;
        self.check_nesting(population)?;
        self.check_roots(population)
    }

    fn children_of<'a>(&'a self, parent: &SubcloneId) -> impl Iterator<Item = &'a SubcloneId> {
        let parent = parent.clone();
        self.edges
            .iter()
            .filter(move |(p, _)| *p == parent)
            .map(|(_, child)| child)
    }

    fn check_acyclic(&self) -> Result<(), PhylogenyRefusal> {
        let mut nodes: BTreeSet<&SubcloneId> = BTreeSet::new();
        for (parent, child) in &self.edges {
            nodes.insert(parent);
            nodes.insert(child);
        }
        let mut visiting: BTreeSet<SubcloneId> = BTreeSet::new();
        let mut done: BTreeSet<SubcloneId> = BTreeSet::new();
        for node in nodes {
            self.visit(node, &mut visiting, &mut done)?;
        }
        Ok(())
    }

    fn visit(
        &self,
        node: &SubcloneId,
        visiting: &mut BTreeSet<SubcloneId>,
        done: &mut BTreeSet<SubcloneId>,
    ) -> Result<(), PhylogenyRefusal> {
        if done.contains(node) {
            return Ok(());
        }
        if !visiting.insert(node.clone()) {
            return Err(PhylogenyRefusal::Cyclic {
                subclone: node.as_str().to_string(),
            });
        }
        let children: Vec<SubcloneId> = self.children_of(node).cloned().collect();
        for child in &children {
            self.visit(child, visiting, done)?;
        }
        visiting.remove(node);
        done.insert(node.clone());
        Ok(())
    }

    fn check_nesting(&self, population: &TumourPopulation) -> Result<(), PhylogenyRefusal> {
        let parents: BTreeSet<&SubcloneId> = self.edges.iter().map(|(parent, _)| parent).collect();
        for parent in parents {
            let parent_fraction = population
                .get(parent)
                .expect("endpoints were checked above")
                .fraction;
            let mut total: u32 = 0;
            for child in self.children_of(parent) {
                let child_fraction = population
                    .get(child)
                    .expect("endpoints were checked above")
                    .fraction;
                total += u32::from(child_fraction.parts_per_ten_thousand());
                if total > u32::from(parent_fraction.parts_per_ten_thousand()) {
                    return Err(PhylogenyRefusal::ChildExceedsParent {
                        parent: parent.as_str().to_string(),
                        child: child.as_str().to_string(),
                        parent_fraction: parent_fraction.describe(),
                        child_fraction: child_fraction.describe(),
                    });
                }
            }
        }
        Ok(())
    }

    fn check_roots(&self, population: &TumourPopulation) -> Result<(), PhylogenyRefusal> {
        let children: BTreeSet<&SubcloneId> = self.edges.iter().map(|(_, child)| child).collect();
        let total: u32 = population
            .subclones()
            .filter(|subclone| !children.contains(&subclone.id))
            .map(|subclone| u32::from(subclone.fraction.parts_per_ten_thousand()))
            .sum();
        if total > 10_000 {
            return Err(PhylogenyRefusal::FractionsExceedWhole {
                total: format!("{total}/10000"),
            });
        }
        Ok(())
    }
}

/// Several histories a caller wants to consider.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibleHistories {
    pub compatible: Vec<ClonalHistory>,
    pub rejected: Vec<(ClonalHistory, PhylogenyRefusal)>,
}

/// Partitions candidate histories by compatibility with the observed fractions.
pub fn compatible_histories(
    population: &TumourPopulation,
    candidates: Vec<ClonalHistory>,
) -> CompatibleHistories {
    let mut result = CompatibleHistories::default();
    for candidate in candidates {
        match candidate.check(population) {
            Ok(()) => result.compatible.push(candidate),
            Err(refusal) => result.rejected.push((candidate, refusal)),
        }
    }
    result
}

impl CompatibleHistories {
    /// The single compatible history, or a refusal naming how many survived.
    ///
    /// Two compatible histories are two histories. 30.12's "alternative-history coverage" metric
    /// exists because reporting the first one is how a tool converts ambiguity into a fact.
    pub fn sole(&self) -> Result<&ClonalHistory, PhylogenyRefusal> {
        match self.compatible.as_slice() {
            [only] => Ok(only),
            others => Err(PhylogenyRefusal::Ambiguous {
                count: others.len(),
            }),
        }
    }
}

/// The smallest subclone fraction an assay could have detected, as declared by the caller.
///
/// No default. Depth, purity and caller sensitivity interact in ways 30.12 does not specify, and
/// this crate refuses to supply the number rather than invent one. `declared_by` records who did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionSensitivity {
    pub smallest_detectable_fraction: CellularFraction,
    pub declared_by: String,
}

/// What a specimen actually sampled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecimenSampling {
    pub specimen: String,
    pub regions: BTreeSet<RegionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<DetectionSensitivity>,
}

impl SpecimenSampling {
    pub fn new(specimen: impl Into<String>) -> Self {
        SpecimenSampling {
            specimen: specimen.into(),
            regions: BTreeSet::new(),
            sensitivity: None,
        }
    }

    pub fn sampling(mut self, region: RegionId) -> Self {
        self.regions.insert(region);
        self
    }

    pub fn detecting_down_to(mut self, sensitivity: DetectionSensitivity) -> Self {
        self.sensitivity = Some(sensitivity);
        self
    }

    /// Whether this sampling covers every region the other sampling touched.
    pub fn covers(&self, other: &SpecimenSampling) -> bool {
        other.regions.is_subset(&self.regions)
    }
}

/// How a caller expressed a fraction, and whether the conversion it needed was declared.
///
/// A variant allele fraction is not a cellular fraction: it depends on purity, local copy number
/// and the multiplicity of the variant. This module does not perform that conversion. It records
/// which side of the line a number is on, so that a claim resting on an unconverted allele
/// fraction is visible as such.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fraction_evidence", rename_all = "snake_case")]
pub enum FractionEvidence {
    AlleleFraction {
        vaf: CellularFraction,
    },
    Cellular {
        fraction: CellularFraction,
        derivation: FractionDerivation,
    },
}

/// The declarations a cellular fraction rests on, recorded rather than recomputed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FractionDerivation {
    pub purity: CellularFraction,
    pub local_copy_number: u32,
    pub multiplicity: u32,
    /// The tool that performed the conversion. This crate never does.
    pub derived_by: String,
}

impl FractionEvidence {
    /// The cellular fraction, or a refusal naming what was never declared.
    pub fn cellular(&self) -> Result<CellularFraction, PromotionRefusal> {
        match self {
            FractionEvidence::Cellular { fraction, .. } => Ok(*fraction),
            FractionEvidence::AlleleFraction { .. } => Err(PromotionRefusal::CopyNumberUnknown {
                missing: "purity, local copy number and multiplicity".to_string(),
            }),
        }
    }
}

/// One marker measured on one specimen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecimenObservation {
    pub marker: MolecularMarker,
    pub sampling: SpecimenSampling,
    pub call: Observed<MarkerCall>,
    /// Present only when the marker was detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fraction: Option<FractionEvidence>,
}

impl SpecimenObservation {
    pub fn new(marker: MolecularMarker, sampling: SpecimenSampling, call: Observed<MarkerCall>) -> Self {
        SpecimenObservation {
            marker,
            sampling,
            call,
            fraction: None,
        }
    }

    pub fn at_fraction(mut self, fraction: FractionEvidence) -> Self {
        self.fraction = Some(fraction);
        self
    }

    /// What this specimen-level observation licenses saying about the tumour.
    ///
    /// The whole point of the module. A detection promotes to an existential claim about the
    /// tumour; a non-detection becomes a bound over the sampled regions and nothing more.
    pub fn as_tumour_claim(&self) -> Result<TumourClaim, PromotionRefusal> {
        if self.sampling.regions.is_empty() {
            return Err(PromotionRefusal::NoRegionSampled);
        }
        match &self.call {
            Observed::Value(MarkerCall::Present) => Ok(TumourClaim::PresentInSampledRegions {
                marker: self.marker,
                regions: self.sampling.regions.clone(),
            }),
            Observed::Value(MarkerCall::Absent) => self.bound(),
            Observed::Unobserved(status) if status.is_informative_about_the_value() => self.bound(),
            Observed::Unobserved(status) => Err(PromotionRefusal::NotAnAbsence {
                marker: self.marker.describe().to_string(),
                status: status.describe().to_string(),
            }),
        }
    }

    fn bound(&self) -> Result<TumourClaim, PromotionRefusal> {
        let sensitivity = self
            .sampling
            .sensitivity
            .as_ref()
            .ok_or(PromotionRefusal::UndeclaredSensitivity)?;
        Ok(TumourClaim::UndetectedAboveFraction {
            marker: self.marker,
            fraction: sensitivity.smallest_detectable_fraction,
            regions: self.sampling.regions.clone(),
        })
    }
}

/// What may be said about the tumour, given a specimen.
///
/// Note the missing variant. There is no `Absent`, and no function in this crate returns one:
/// a negative result on a fragment of a heterogeneous population is a bound with a region list,
/// which is what [`TumourClaim::UndetectedAboveFraction`] is. That is the difference between
/// "absent in this specimen" and "absent in this tumour", and it is enforced by the shape of the
/// enum rather than by a warning in prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tumour_claim", rename_all = "snake_case")]
pub enum TumourClaim {
    PresentInSampledRegions {
        marker: MolecularMarker,
        regions: BTreeSet<RegionId>,
    },
    UndetectedAboveFraction {
        marker: MolecularMarker,
        fraction: CellularFraction,
        regions: BTreeSet<RegionId>,
    },
}

impl TumourClaim {
    pub fn marker(&self) -> MolecularMarker {
        match self {
            TumourClaim::PresentInSampledRegions { marker, .. }
            | TumourClaim::UndetectedAboveFraction { marker, .. } => *marker,
        }
    }

    /// Whether this claim rules out a subclone at `fraction` in `region`.
    ///
    /// False for every region that was not sampled, at every fraction, forever. That is the
    /// heterogeneity fact: a fragment constrains the fragment.
    pub fn excludes_subclone(&self, fraction: CellularFraction, region: &RegionId) -> bool {
        match self {
            TumourClaim::PresentInSampledRegions { .. } => false,
            TumourClaim::UndetectedAboveFraction {
                fraction: limit,
                regions,
                ..
            } => regions.contains(region) && fraction >= *limit,
        }
    }
}

/// Explanations for an alteration seen at recurrence and not at diagnosis (30.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResistanceExplanation {
    /// The subclone was there at diagnosis and therapy selected for it.
    PreexistingSubcloneSelected,
    /// The alteration arose after diagnosis.
    DeNovoEmergence,
    /// The diagnostic specimen did not sample the region the recurrence came from.
    UnsampledRegionAtDiagnosis,
    /// The subclone was present at diagnosis below what the assay could see.
    BelowDetectionAtDiagnosis,
    /// The diagnosis and recurrence numbers are allele fractions in different copy-number
    /// contexts, so the apparent change may be arithmetic.
    CopyNumberEffectOnAlleleFraction,
}

impl ResistanceExplanation {
    pub const ALL: [ResistanceExplanation; 5] = [
        ResistanceExplanation::PreexistingSubcloneSelected,
        ResistanceExplanation::DeNovoEmergence,
        ResistanceExplanation::UnsampledRegionAtDiagnosis,
        ResistanceExplanation::BelowDetectionAtDiagnosis,
        ResistanceExplanation::CopyNumberEffectOnAlleleFraction,
    ];
}

/// Which explanations the evidence excludes, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationSet {
    pub not_excluded: BTreeSet<ResistanceExplanation>,
    pub excluded: BTreeMap<String, String>,
}

impl ExplanationSet {
    /// The single surviving explanation, or a refusal naming how many survived.
    pub fn sole(&self) -> Result<ResistanceExplanation, PhylogenyRefusal> {
        let mut iter = self.not_excluded.iter();
        match (iter.next(), iter.next()) {
            (Some(only), None) => Ok(*only),
            _ => Err(PhylogenyRefusal::Ambiguous {
                count: self.not_excluded.len(),
            }),
        }
    }

    pub fn contains(&self, explanation: ResistanceExplanation) -> bool {
        self.not_excluded.contains(&explanation)
    }
}

/// Compares a diagnosis observation with a recurrence observation of the same marker.
///
/// Every exclusion below is arithmetic. `UnsampledRegionAtDiagnosis` leaves the set when the
/// diagnostic sampling covered every region the recurrence sampled; `BelowDetectionAtDiagnosis`
/// leaves when the diagnostic bound excludes a subclone at the recurrence's own cellular fraction
/// in a covered region; `CopyNumberEffectOnAlleleFraction` leaves when both sides declared the
/// conversion. `PreexistingSubcloneSelected` leaves only when both sampling explanations have.
///
/// `DeNovoEmergence` never leaves the set: nothing in a pair of specimen observations rules out
/// that an alteration arose after diagnosis. It also never becomes the answer on its own unless
/// every rival has been excluded on the above grounds.
pub fn explain_new_alteration(
    diagnosis: &SpecimenObservation,
    recurrence: &SpecimenObservation,
) -> ExplanationSet {
    let mut not_excluded: BTreeSet<ResistanceExplanation> =
        ResistanceExplanation::ALL.into_iter().collect();
    let mut excluded = BTreeMap::new();

    let regions_covered = diagnosis.sampling.covers(&recurrence.sampling);
    if regions_covered {
        not_excluded.remove(&ResistanceExplanation::UnsampledRegionAtDiagnosis);
        excluded.insert(
            "unsampled_region_at_diagnosis".to_string(),
            format!(
                "the diagnostic specimen {} sampled every region the recurrence sampled",
                diagnosis.sampling.specimen
            ),
        );
    }

    let sensitivity_excludes = regions_covered
        && match (
            diagnosis.as_tumour_claim(),
            recurrence.fraction.as_ref().map(FractionEvidence::cellular),
        ) {
            (Ok(claim), Some(Ok(fraction))) => recurrence
                .sampling
                .regions
                .iter()
                .all(|region| claim.excludes_subclone(fraction, region)),
            _ => false,
        };
    if sensitivity_excludes {
        not_excluded.remove(&ResistanceExplanation::BelowDetectionAtDiagnosis);
        excluded.insert(
            "below_detection_at_diagnosis".to_string(),
            "the diagnostic assay could have detected a subclone at the recurrence's cellular fraction in every sampled region".to_string(),
        );
    }

    let both_converted = diagnosis
        .fraction
        .as_ref()
        .is_some_and(|f| f.cellular().is_ok())
        && recurrence
            .fraction
            .as_ref()
            .is_some_and(|f| f.cellular().is_ok());
    if both_converted {
        not_excluded.remove(&ResistanceExplanation::CopyNumberEffectOnAlleleFraction);
        excluded.insert(
            "copy_number_effect_on_allele_fraction".to_string(),
            "both fractions declare purity, local copy number and multiplicity".to_string(),
        );
    }

    if regions_covered && sensitivity_excludes {
        not_excluded.remove(&ResistanceExplanation::PreexistingSubcloneSelected);
        excluded.insert(
            "preexisting_subclone_selected".to_string(),
            "the diagnostic specimen covered the regions and could have seen the subclone"
                .to_string(),
        );
    }

    ExplanationSet {
        not_excluded,
        excluded,
    }
}

/// The evidence a caller has for linking a treatment to an alteration.
///
/// One variant, because temporal association is the only relation this module represents. A real
/// design — a randomised comparison, an instrument, a matched control arm — belongs to a causal
/// layer this crate does not contain, and leaving room for one here without implementing it would
/// be an invitation to pass `TrialArm` and receive an unearned `Ok`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalDesign {
    TemporalAssociationOnly,
}

/// Always refuses (30.12: "claiming treatment causation from temporal association").
pub fn attribute_to_treatment(
    treatment: &str,
    alteration: MolecularMarker,
    design: CausalDesign,
) -> Result<std::convert::Infallible, PhylogenyRefusal> {
    let CausalDesign::TemporalAssociationOnly = design;
    Err(PhylogenyRefusal::UnsupportedDirectionality {
        treatment: treatment.to_string(),
        alteration: alteration.describe().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_onco::ObservationStatus;

    fn fraction(parts: u16) -> CellularFraction {
        CellularFraction::from_parts_per_ten_thousand(parts).expect("within the whole")
    }

    fn region(name: &str) -> RegionId {
        RegionId::new(name)
    }

    fn sensitivity(parts: u16) -> DetectionSensitivity {
        DetectionSensitivity {
            smallest_detectable_fraction: fraction(parts),
            declared_by: "assay validation report supplied by the caller".to_string(),
        }
    }

    fn negative_specimen(regions: &[&str], limit: u16) -> SpecimenObservation {
        let mut sampling = SpecimenSampling::new("S1").detecting_down_to(sensitivity(limit));
        for name in regions {
            sampling = sampling.sampling(region(name));
        }
        SpecimenObservation::new(
            MolecularMarker::EgfrAmplification,
            sampling,
            Observed::Value(MarkerCall::Absent),
        )
    }

    #[test]
    fn a_marker_absent_from_a_specimen_is_not_absent_from_the_tumour() {
        let observation = negative_specimen(&["core"], 500);
        let claim = observation.as_tumour_claim().expect("a bound is available");
        assert!(matches!(
            claim,
            TumourClaim::UndetectedAboveFraction { .. }
        ));
        assert!(claim.excludes_subclone(fraction(1_000), &region("core")));
        assert!(!claim.excludes_subclone(fraction(100), &region("core")));
    }

    #[test]
    fn an_unsampled_region_is_never_excluded_however_sensitive_the_assay() {
        let observation = negative_specimen(&["core"], 0);
        let claim = observation.as_tumour_claim().expect("a bound is available");
        assert!(claim.excludes_subclone(fraction(0), &region("core")));
        assert!(!claim.excludes_subclone(CellularFraction::WHOLE, &region("infiltrating edge")));
    }

    #[test]
    fn presence_in_a_fragment_promotes_to_the_tumour_but_absence_does_not() {
        let sampling = SpecimenSampling::new("S1").sampling(region("core"));
        let positive = SpecimenObservation::new(
            MolecularMarker::EgfrAmplification,
            sampling,
            Observed::Value(MarkerCall::Present),
        );
        assert!(matches!(
            positive.as_tumour_claim().expect("presence promotes"),
            TumourClaim::PresentInSampledRegions { .. }
        ));
        let negative = negative_specimen(&["core"], 500);
        assert!(matches!(
            negative.as_tumour_claim().expect("a bound is available"),
            TumourClaim::UndetectedAboveFraction { .. }
        ));
    }

    #[test]
    fn a_negative_result_without_a_declared_detection_limit_bounds_nothing() {
        let sampling = SpecimenSampling::new("S1").sampling(region("core"));
        let observation = SpecimenObservation::new(
            MolecularMarker::EgfrAmplification,
            sampling,
            Observed::Value(MarkerCall::Absent),
        );
        assert_eq!(
            observation.as_tumour_claim().unwrap_err(),
            PromotionRefusal::UndeclaredSensitivity
        );
    }

    #[test]
    fn an_assay_that_never_ran_is_not_an_absence() {
        let sampling = SpecimenSampling::new("S1")
            .sampling(region("core"))
            .detecting_down_to(sensitivity(500));
        let observation = SpecimenObservation::new(
            MolecularMarker::EgfrAmplification,
            sampling,
            Observed::Unobserved(ObservationStatus::NotCollected),
        );
        assert!(matches!(
            observation.as_tumour_claim().unwrap_err(),
            PromotionRefusal::NotAnAbsence { .. }
        ));
    }

    #[test]
    fn a_below_detection_status_is_the_one_unobserved_status_that_yields_a_bound() {
        let sampling = SpecimenSampling::new("S1")
            .sampling(region("core"))
            .detecting_down_to(sensitivity(500));
        let observation = SpecimenObservation::new(
            MolecularMarker::EgfrAmplification,
            sampling,
            Observed::Unobserved(ObservationStatus::BelowDetection),
        );
        assert!(observation.as_tumour_claim().is_ok());
        for status in [
            ObservationStatus::Missing,
            ObservationStatus::TechnicallyFailed,
            ObservationStatus::NotApplicable,
            ObservationStatus::Redacted,
        ] {
            let mut other = observation.clone();
            other.call = Observed::Unobserved(status);
            assert!(other.as_tumour_claim().is_err(), "{status:?} bounded a tumour");
        }
    }

    #[test]
    fn a_specimen_that_sampled_no_region_makes_no_tumour_claim_at_all() {
        let observation = SpecimenObservation::new(
            MolecularMarker::EgfrAmplification,
            SpecimenSampling::new("S1").detecting_down_to(sensitivity(500)),
            Observed::Value(MarkerCall::Present),
        );
        assert_eq!(
            observation.as_tumour_claim().unwrap_err(),
            PromotionRefusal::NoRegionSampled
        );
    }

    #[test]
    fn an_allele_fraction_is_not_a_cellular_fraction() {
        let vaf = FractionEvidence::AlleleFraction {
            vaf: fraction(2_500),
        };
        assert!(matches!(
            vaf.cellular().unwrap_err(),
            PromotionRefusal::CopyNumberUnknown { .. }
        ));
        let declared = FractionEvidence::Cellular {
            fraction: fraction(5_000),
            derivation: FractionDerivation {
                purity: fraction(8_000),
                local_copy_number: 2,
                multiplicity: 1,
                derived_by: "the caller's variant caller".to_string(),
            },
        };
        assert_eq!(declared.cellular().unwrap(), fraction(5_000));
    }

    #[test]
    fn a_resistance_alteration_at_recurrence_does_not_exclude_a_preexisting_subclone() {
        let diagnosis = negative_specimen(&["core"], 2_000);
        let mut recurrence = negative_specimen(&["core", "infiltrating edge"], 500);
        recurrence.call = Observed::Value(MarkerCall::Present);
        let explanations = explain_new_alteration(&diagnosis, &recurrence);
        assert!(explanations.contains(ResistanceExplanation::PreexistingSubcloneSelected));
        assert!(explanations.contains(ResistanceExplanation::UnsampledRegionAtDiagnosis));
        assert!(explanations.sole().is_err());
    }

    #[test]
    fn de_novo_emergence_is_never_excluded_by_a_pair_of_specimens() {
        let diagnosis = negative_specimen(&["core"], 100);
        let mut recurrence = negative_specimen(&["core"], 100);
        recurrence.call = Observed::Value(MarkerCall::Present);
        recurrence.fraction = Some(FractionEvidence::Cellular {
            fraction: fraction(6_000),
            derivation: FractionDerivation {
                purity: fraction(9_000),
                local_copy_number: 2,
                multiplicity: 1,
                derived_by: "caller".to_string(),
            },
        });
        let explanations = explain_new_alteration(&diagnosis, &recurrence);
        assert!(explanations.contains(ResistanceExplanation::DeNovoEmergence));
    }

    #[test]
    fn covering_the_regions_and_the_fraction_excludes_the_sampling_explanations() {
        let mut diagnosis = negative_specimen(&["core", "infiltrating edge"], 100);
        diagnosis.fraction = Some(FractionEvidence::Cellular {
            fraction: CellularFraction::NONE,
            derivation: FractionDerivation {
                purity: fraction(9_000),
                local_copy_number: 2,
                multiplicity: 1,
                derived_by: "caller".to_string(),
            },
        });
        let mut recurrence = negative_specimen(&["core"], 100);
        recurrence.call = Observed::Value(MarkerCall::Present);
        recurrence.fraction = Some(FractionEvidence::Cellular {
            fraction: fraction(6_000),
            derivation: FractionDerivation {
                purity: fraction(9_000),
                local_copy_number: 2,
                multiplicity: 1,
                derived_by: "caller".to_string(),
            },
        });
        let explanations = explain_new_alteration(&diagnosis, &recurrence);
        assert!(!explanations.contains(ResistanceExplanation::UnsampledRegionAtDiagnosis));
        assert!(!explanations.contains(ResistanceExplanation::BelowDetectionAtDiagnosis));
        assert!(!explanations.contains(ResistanceExplanation::PreexistingSubcloneSelected));
        assert_eq!(
            explanations.sole().unwrap(),
            ResistanceExplanation::DeNovoEmergence
        );
    }

    #[test]
    fn uncorrected_allele_fractions_leave_the_copy_number_explanation_open() {
        let mut diagnosis = negative_specimen(&["core"], 100);
        diagnosis.fraction = Some(FractionEvidence::AlleleFraction { vaf: fraction(200) });
        let mut recurrence = negative_specimen(&["core"], 100);
        recurrence.call = Observed::Value(MarkerCall::Present);
        recurrence.fraction = Some(FractionEvidence::AlleleFraction {
            vaf: fraction(4_000),
        });
        let explanations = explain_new_alteration(&diagnosis, &recurrence);
        assert!(explanations.contains(ResistanceExplanation::CopyNumberEffectOnAlleleFraction));
    }

    #[test]
    fn temporal_association_with_a_treatment_never_yields_causation() {
        let refusal = attribute_to_treatment(
            "alkylating chemoradiation",
            MolecularMarker::EgfrAmplification,
            CausalDesign::TemporalAssociationOnly,
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            PhylogenyRefusal::UnsupportedDirectionality { .. }
        ));
    }

    #[test]
    fn a_child_subclone_cannot_carry_more_cells_than_its_parent() {
        let population = TumourPopulation::new()
            .with(Subclone::new(SubcloneId::new("C1"), fraction(4_000)))
            .with(Subclone::new(SubcloneId::new("C2"), fraction(6_000)));
        let history =
            ClonalHistory::new().descends(SubcloneId::new("C1"), SubcloneId::new("C2"));
        assert!(matches!(
            history.check(&population).unwrap_err(),
            PhylogenyRefusal::ChildExceedsParent { .. }
        ));
    }

    #[test]
    fn nested_subclones_that_fit_inside_their_parent_are_compatible() {
        let population = TumourPopulation::new()
            .with(Subclone::new(SubcloneId::new("C1"), CellularFraction::WHOLE))
            .with(Subclone::new(SubcloneId::new("C2"), fraction(4_000)))
            .with(Subclone::new(SubcloneId::new("C3"), fraction(3_000)));
        let history = ClonalHistory::new()
            .descends(SubcloneId::new("C1"), SubcloneId::new("C2"))
            .descends(SubcloneId::new("C1"), SubcloneId::new("C3"));
        assert!(history.check(&population).is_ok());
    }

    #[test]
    fn an_ancestry_cycle_is_rejected() {
        let population = TumourPopulation::new()
            .with(Subclone::new(SubcloneId::new("C1"), CellularFraction::WHOLE))
            .with(Subclone::new(SubcloneId::new("C2"), CellularFraction::WHOLE));
        let history = ClonalHistory::new()
            .descends(SubcloneId::new("C1"), SubcloneId::new("C2"))
            .descends(SubcloneId::new("C2"), SubcloneId::new("C1"));
        assert!(matches!(
            history.check(&population).unwrap_err(),
            PhylogenyRefusal::Cyclic { .. }
        ));
    }

    #[test]
    fn disjoint_roots_cannot_together_exceed_the_tumour() {
        let population = TumourPopulation::new()
            .with(Subclone::new(SubcloneId::new("C1"), fraction(7_000)))
            .with(Subclone::new(SubcloneId::new("C2"), fraction(7_000)));
        assert!(matches!(
            ClonalHistory::new().check(&population).unwrap_err(),
            PhylogenyRefusal::FractionsExceedWhole { .. }
        ));
    }

    #[test]
    fn an_edge_naming_a_subclone_outside_the_population_is_rejected() {
        let population =
            TumourPopulation::new().with(Subclone::new(SubcloneId::new("C1"), fraction(1_000)));
        let history =
            ClonalHistory::new().descends(SubcloneId::new("C1"), SubcloneId::new("ghost"));
        assert!(matches!(
            history.check(&population).unwrap_err(),
            PhylogenyRefusal::UnknownSubclone { .. }
        ));
    }

    #[test]
    fn two_compatible_histories_do_not_make_one_history() {
        let population = TumourPopulation::new()
            .with(Subclone::new(SubcloneId::new("C1"), CellularFraction::WHOLE))
            .with(Subclone::new(SubcloneId::new("C2"), fraction(3_000)))
            .with(Subclone::new(SubcloneId::new("C3"), fraction(2_000)));
        let linear = ClonalHistory::new()
            .descends(SubcloneId::new("C1"), SubcloneId::new("C2"))
            .descends(SubcloneId::new("C2"), SubcloneId::new("C3"));
        let branching = ClonalHistory::new()
            .descends(SubcloneId::new("C1"), SubcloneId::new("C2"))
            .descends(SubcloneId::new("C1"), SubcloneId::new("C3"));
        let histories = compatible_histories(&population, vec![linear, branching]);
        assert_eq!(histories.compatible.len(), 2);
        assert!(matches!(
            histories.sole().unwrap_err(),
            PhylogenyRefusal::Ambiguous { count: 2 }
        ));
    }

    #[test]
    fn a_fraction_above_the_whole_is_not_representable() {
        assert!(CellularFraction::from_parts_per_ten_thousand(10_001).is_err());
        assert!(CellularFraction::from_ratio(1.5).is_err());
        assert!(CellularFraction::from_ratio(f64::NAN).is_err());
        assert_eq!(
            CellularFraction::from_ratio(0.25).unwrap(),
            fraction(2_500)
        );
    }
}
