//! A reference standard is a claim about a measurement process, not about truth.
//!
//! Blueprint 31.10 (Imaging, Segmentation, and Geometric Reference Standards) and 31.11 (Pathology,
//! Molecular, and Integrated Reference Standards). 31.11's required functions are the whole design:
//! "store source observations separately from integrated labels", "pin classifier and ontology
//! versions", "allow retrospective mapping without rewriting history".
//!
//! # There is no `LatentTruth`
//!
//! [`ReferenceBasis`] enumerates the processes that can produce a reference — readers, an orthogonal
//! assay, a versioned classifier, a simulator, a clinical record, a later confirmed event. It has no
//! variant meaning "this is the biological fact". That absence is the module. 31.00: "Biological
//! evaluation fails when a convenient label is treated as the latent biological truth." A type with
//! a `GroundTruth` variant makes that failure one keystroke away; a type without one makes every
//! consumer read the process off the standard before using it.
//!
//! The simulator is the one basis that *does* have access to a latent state — 32.20's twin "supplies
//! exact latent burden and noisy MRI observations". [`ReferenceBasis::Simulator`] therefore carries
//! its own misspecification admission, because 32.20's failure risk is "realism mistaken for
//! validity": exact latent truth *inside the simulator* is not evidence about a patient.
//!
//! # Levels, not a scalar (31.10)
//!
//! 31.10's worked case: "Two algorithms with similar Dice are differentiated by missed-lesion rate,
//! surface error, and effect on response classification." A standard therefore records agreement per
//! [`ReferenceLevel`], and [`ReferenceStandard::agreement_at`] answers for a level that was never
//! measured with [`Determination::NotEvaluable`] rather than with the level that was. There is no
//! method returning one number for the standard as a whole.
//!
//! # History is append-only (31.11, 32.12)
//!
//! 32.12 mutates the oracle itself — "oracle-version corrections", "versioned regrade". 31.11 requires
//! that a retrospective remap not rewrite history. [`StandardHistory::regrade`] takes `self` by value
//! and returns a new history with one more revision; there is no method that edits a revision in
//! place, so a regrade can never make the earlier reference unrecoverable.
//!
//! # Not implemented
//!
//! No image data, no mask arithmetic, no Dice, no surface distance. This crate holds no pixels; a
//! [`LevelAgreement`] records a number a caller measured elsewhere together with the level it
//! measured. 31.10's "consensus with uncertainty maps" and "validate registration and coordinate
//! systems" need voxels and an affine and are not here.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_oracle::EvidenceTier;
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};

use crate::error::OracleXError;
use crate::verdict::Determination;

/// The measurement process a reference came out of.
///
/// Every variant names something a person could go and inspect. None of them names the biology.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum ReferenceBasis {
    /// Independent reads plus a consensus rule (31.06). The rule is part of the basis: change it
    /// and the reference distribution changes, which is 32.18's declared relation.
    ReaderConsensus { rule: String, readers: usize },
    /// A measurement with different failure modes from the one under evaluation (31.07).
    OrthogonalAssay { assay: String },
    /// A versioned classifier plus the ontology it emits into (31.11).
    IntegratedClassifier {
        classifier: String,
        classifier_version: String,
        ontology_version: String,
    },
    /// A mechanistic model (32.20). Carries its own admission of misspecification, because a
    /// simulator's exact latent state is exact only about the simulator.
    Simulator {
        model: String,
        known_misspecification: String,
    },
    /// An abstracted clinical record (31.12).
    ClinicalRecord { source: String },
    /// An event that happened after the decision under evaluation (31.09).
    LaterEvent { event: String },
}

impl ReferenceBasis {
    pub fn kind(&self) -> &'static str {
        match self {
            ReferenceBasis::ReaderConsensus { .. } => "reader_consensus",
            ReferenceBasis::OrthogonalAssay { .. } => "orthogonal_assay",
            ReferenceBasis::IntegratedClassifier { .. } => "integrated_classifier",
            ReferenceBasis::Simulator { .. } => "simulator",
            ReferenceBasis::ClinicalRecord { .. } => "clinical_record",
            ReferenceBasis::LaterEvent { .. } => "later_event",
        }
    }

    /// The strongest rung a reference on this basis may claim.
    ///
    /// A consensus of readers is a statistic over a social process, so it caps at
    /// [`EvidenceTier::Statistical`] no matter how many readers agreed — 31.06's purpose sentence is
    /// "use experts as a measured reference process rather than an anonymous source of final
    /// labels". A simulator caps at [`EvidenceTier::Property`]: its output is reproducible, and
    /// reproducibility is all it is.
    pub fn ceiling(&self) -> EvidenceTier {
        match self {
            ReferenceBasis::ReaderConsensus { .. } => EvidenceTier::Statistical,
            ReferenceBasis::OrthogonalAssay { .. } => EvidenceTier::Statistical,
            ReferenceBasis::IntegratedClassifier { .. } => EvidenceTier::Property,
            ReferenceBasis::Simulator { .. } => EvidenceTier::Property,
            ReferenceBasis::ClinicalRecord { .. } => EvidenceTier::Deterministic,
            ReferenceBasis::LaterEvent { .. } => EvidenceTier::Deterministic,
        }
    }
}

/// One of the levels 31.10 requires a geometric reference to distinguish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceLevel {
    /// Was the lesion found at all.
    Detection,
    /// Where the boundary runs.
    Boundary,
    /// Connectedness, holes, component count.
    Topology,
    /// Change between timepoints, which is not the difference of two independent segmentations.
    LongitudinalChange,
    /// The decision the geometry feeds — 31.10's "downstream regret".
    DownstreamUse,
}

impl ReferenceLevel {
    pub const ALL: [ReferenceLevel; 5] = [
        ReferenceLevel::Detection,
        ReferenceLevel::Boundary,
        ReferenceLevel::Topology,
        ReferenceLevel::LongitudinalChange,
        ReferenceLevel::DownstreamUse,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ReferenceLevel::Detection => "detection",
            ReferenceLevel::Boundary => "boundary",
            ReferenceLevel::Topology => "topology",
            ReferenceLevel::LongitudinalChange => "longitudinal_change",
            ReferenceLevel::DownstreamUse => "downstream_use",
        }
    }
}

/// An agreement figure a caller measured, tagged with the level it is about and the metric it came
/// from.
///
/// The metric name is mandatory and free-form on purpose: this crate does not compute overlap and
/// therefore has no business enumerating the metrics that exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelAgreement {
    pub level: ReferenceLevel,
    pub metric: String,
    pub value: f64,
}

impl LevelAgreement {
    pub fn new(
        level: ReferenceLevel,
        metric: impl Into<String>,
        value: f64,
    ) -> Result<Self, OracleXError> {
        if !value.is_finite() {
            return Err(OracleXError::NonFinite {
                field: "LevelAgreement::value",
                value,
            });
        }
        Ok(LevelAgreement {
            level,
            metric: metric.into(),
            value,
        })
    }
}

/// A raw observation, retained separately from any label derived from it (31.11).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceObservation {
    pub assay: String,
    pub observation: String,
    /// 31.11: "capture sample adequacy and regional sampling". `None` means nobody recorded it,
    /// which is not the same as adequate.
    pub adequacy: Option<String>,
}

impl SourceObservation {
    pub fn new(assay: impl Into<String>, observation: impl Into<String>) -> Self {
        SourceObservation {
            assay: assay.into(),
            observation: observation.into(),
            adequacy: None,
        }
    }
}

/// What a reference standard says the class is.
///
/// 31.11's worked case: "A borderline methylation score and suggestive histology form a
/// probabilistic reference, not a forced class." [`ClassCall::Spread`] is that reference, and there
/// is deliberately no `into_definite` that picks the mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum ClassCall {
    Definite { class: String },
    /// Mass over candidate classes. Not normalised by this type: a caller who supplies masses that
    /// do not sum to one has an unnormalised belief, and silently rescaling it would invent one.
    Spread { mass: BTreeMap<String, f64> },
}

impl ClassCall {
    /// The single class, when there is one. `None` for any spread, including a spread whose mass is
    /// concentrated on one class — because "0.97 on astrocytoma" is a different statement from
    /// "astrocytoma", and the difference is exactly what 31.11 asks to preserve.
    pub fn single_class(&self) -> Option<&str> {
        match self {
            ClassCall::Definite { class } => Some(class.as_str()),
            ClassCall::Spread { .. } => None,
        }
    }

    /// Every class carrying the maximum mass, as a set. Ties are returned as ties.
    pub fn modes(&self) -> BTreeSet<String> {
        match self {
            ClassCall::Definite { class } => {
                let mut set = BTreeSet::new();
                set.insert(class.clone());
                set
            }
            ClassCall::Spread { mass } => {
                let peak = mass.values().copied().fold(f64::NEG_INFINITY, f64::max);
                mass.iter()
                    .filter(|(_, m)| **m >= peak)
                    .map(|(class, _)| class.clone())
                    .collect()
            }
        }
    }
}

/// A reference standard: one claim, about one population, from one process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceStandard {
    pub id: String,
    /// 31.05's `reference_population`. `bioprism-oracle`'s manifest deliberately omits this field
    /// as a registry concern; a reference standard cannot omit it, because a standard without a
    /// population is a claim about nothing.
    pub population: ScopeKey,
    pub basis: ReferenceBasis,
    /// Never empty: a standard with no source observation is an integrated label with its evidence
    /// thrown away.
    sources: Vec<SourceObservation>,
    /// The class this standard calls, when it calls one. Geometric standards leave it `None` and
    /// speak through [`ReferenceStandard::agreement_at`].
    pub call: Option<ClassCall>,
    agreements: BTreeMap<ReferenceLevel, LevelAgreement>,
}

impl ReferenceStandard {
    pub fn new(
        id: impl Into<String>,
        population: ScopeKey,
        basis: ReferenceBasis,
        sources: impl IntoIterator<Item = SourceObservation>,
    ) -> Result<Self, OracleXError> {
        let id = id.into();
        let sources: Vec<SourceObservation> = sources.into_iter().collect();
        if sources.is_empty() {
            return Err(OracleXError::StandardWithoutProcess { standard: id });
        }
        Ok(ReferenceStandard {
            id,
            population,
            basis,
            sources,
            call: None,
            agreements: BTreeMap::new(),
        })
    }

    pub fn with_call(mut self, call: ClassCall) -> Self {
        self.call = Some(call);
        self
    }

    pub fn with_agreement(mut self, agreement: LevelAgreement) -> Self {
        self.agreements.insert(agreement.level, agreement);
        self
    }

    /// The observations the integrated label was derived from. Always non-empty.
    pub fn sources(&self) -> &[SourceObservation] {
        &self.sources
    }

    /// The levels this standard was actually measured at.
    pub fn measured_levels(&self) -> BTreeSet<ReferenceLevel> {
        self.agreements.keys().copied().collect()
    }

    /// What this standard can say about one level.
    ///
    /// The point of the method is its abstention. A boundary-level agreement figure says nothing
    /// about whether the lesion was found, so asking a boundary-only standard about
    /// [`ReferenceLevel::Detection`] returns [`Determination::NotEvaluable`] and names the level. A
    /// caller wanting a single number has to decide which level they meant, in the open.
    pub fn agreement_at(&self, level: ReferenceLevel) -> Determination {
        match self.agreements.get(&level) {
            Some(agreement) => Determination::supported(
                self.basis.ceiling(),
                format!(
                    "{} measured at level {} by {}",
                    self.id,
                    level.as_str(),
                    agreement.metric
                ),
            ),
            None => Determination::unresolved(
                format!("agreement at level {}", level.as_str()),
                format!(
                    "standard {} was measured at {:?} only",
                    self.id,
                    self.measured_levels()
                        .iter()
                        .map(|l| l.as_str())
                        .collect::<Vec<_>>()
                ),
            ),
        }
    }

    /// The tier a consumer may claim for this standard.
    ///
    /// Capped by [`ReferenceBasis::ceiling`], so a caller cannot promote a reader consensus to
    /// deterministic by asserting it confidently. 31.05's failure containment: "A weak oracle cannot
    /// be promoted to definitive truth merely because it is convenient."
    pub fn admissible_tier(&self, claimed: EvidenceTier) -> EvidenceTier {
        claimed.min(self.basis.ceiling())
    }
}

/// One revision of a standard, with the reason it superseded its predecessor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub standard: ReferenceStandard,
    /// Empty on the first revision; every later one says what changed and why (32.12's
    /// "oracle-version corrections").
    pub reason: String,
}

/// The append-only history of one standard (31.11, 32.12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardHistory {
    revisions: Vec<Revision>,
}

impl StandardHistory {
    pub fn begin(standard: ReferenceStandard) -> Self {
        StandardHistory {
            revisions: vec![Revision {
                standard,
                reason: String::new(),
            }],
        }
    }

    /// Appends a revision. Consumes and returns, so no caller holds a handle through which the
    /// previous revision could be edited.
    pub fn regrade(mut self, standard: ReferenceStandard, reason: impl Into<String>) -> Self {
        self.revisions.push(Revision {
            standard,
            reason: reason.into(),
        });
        self
    }

    /// The revision in force now.
    pub fn current(&self) -> &ReferenceStandard {
        &self.revisions[self.revisions.len() - 1].standard
    }

    /// The revision that was in force at index `at`, which is how a result graded under an older
    /// standard stays interpretable after a regrade.
    pub fn at(&self, at: usize) -> Option<&ReferenceStandard> {
        self.revisions.get(at).map(|revision| &revision.standard)
    }

    pub fn revisions(&self) -> &[Revision] {
        &self.revisions
    }

    pub fn len(&self) -> usize {
        self.revisions.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Whether any regrade changed the class this standard calls.
    ///
    /// 32.12's failure risk is that a versioned regrade silently changes prevalence. A caller that
    /// knows a regrade moved calls can requalify results computed under the old one; a caller that
    /// cannot tell will not.
    pub fn calls_changed(&self) -> bool {
        self.revisions
            .windows(2)
            .any(|pair| pair[0].standard.call != pair[1].standard.call)
    }
}
