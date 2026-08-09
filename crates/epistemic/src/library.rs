//! The biological scope, factor and observation library: blueprint 43.31.
//!
//! > Ship a first-party biological semantics library so FIBER optimizes native biological objects
//! > rather than generic text and file relationships.
//!
//! ## Where the boundary is
//!
//! `bioprism-scope` owns the *vocabulary*: [`ScopeClass`], the partial order on scopes, the meet,
//! and the transport taxonomy. `bioprism-standards` owns *units and frames*: the dimension system,
//! anatomical axes, handedness, and ontology term catalogues. This module is neither. It is the
//! **library** 43.31 asks for — a named catalogue of the dimensions and factor signatures the
//! neuro-oncology domain actually uses, each bound to the class `bioprism-scope` would give it.
//!
//! 43.31's release gate makes the library's job explicit: "a general compiler cannot infer that a
//! specimen was consumed, a variant is tied to a genome build, a negative assay has a detection
//! limit, or a scan derives from a particular coordinate frame". Those four are the test, and the
//! last one is the reason [`NegativeResult`] exists as a type with a detection limit in it rather
//! than a boolean.
//!
//! ## The disagreement is the finding
//!
//! The brief for this module said that if the library disagrees with `bioprism-scope` or
//! `bioprism-standards`, the disagreement is the result. It does, in two ways, and
//! [`disagreement_with_scope_registry`] measures the first mechanically:
//!
//! 1. **Dimensions the registry has never heard of.** `bioprism_scope::DimensionRegistry`'s
//!    default table covers 30-odd names, and every one it does not cover classifies as
//!    [`ScopeClass::Unclassified`]. 43.03's own reasoning is that "an unclassified dimension cannot
//!    be proven to be closed over", so a fact scoped by one of these cannot enter protected
//!    closure by class. Run the function for the current list.
//! 2. **`units` is classified `Coordinate`, and `bioprism-standards` disagrees by construction.**
//!    The registry puts `units` alongside `frame` and `orientation` in the coordinate class.
//!    `bioprism-standards` models units as a six-base dimension system with exact conversion, a
//!    structure with nothing to do with coordinate frames — a millilitre is not a position. Both
//!    are defensible: the registry is classifying *what a scope key means for closure*, and
//!    standards is modelling *what a quantity is*. They are not reconcilable by editing one table,
//!    and this crate does not edit either. It is recorded here because a contributor who assumes
//!    they agree will write a unit conversion that reads a coordinate frame.
//!
//! This module does not depend on `bioprism-standards`. The claim above was established by
//! reading it, not by linking it, and is stated at that strength.
//!
//! ## Missing, negative, censored and failed
//!
//! 43.31's fourth non-negotiable invariant. [`Observation`] has five variants and no `Option<f64>`
//! anywhere, because an `Option` collapses all four absences into one. A failed assay and a
//! genuine negative differ in what they license: the first licenses nothing, the second constrains
//! a hypothesis whenever its expected signal is above the detection limit.

use crate::error::EpistemicError;
use crate::evidence::EvidenceItem;
use bioprism_scope::{DimensionRegistry, ScopeClass};
use serde::{Deserialize, Serialize};

/// One scope dimension the domain uses, with the class it should carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ScopeDimension {
    pub name: &'static str,
    /// The class this library asserts. Compared against `bioprism-scope` by
    /// [`disagreement_with_scope_registry`], not merged into it.
    pub class: ScopeClass,
    /// The concrete error that follows from erasing this dimension.
    pub erasure_error: &'static str,
}

/// The dimensions 43.31 enumerates, in its own order.
///
/// > subjects, organisms, tissues, lesions, regions, specimens, aliquots, cells, assays,
/// > sequences, coordinates, time, interventions, cohorts, and studies
pub const DIMENSIONS: &[ScopeDimension] = &[
    ScopeDimension {
        name: "subject",
        class: ScopeClass::Identity,
        erasure_error: "two scans from one subject land on opposite sides of a split",
    },
    ScopeDimension {
        name: "organism",
        class: ScopeClass::Identity,
        erasure_error: "a mouse xenograft result is transported to a human cohort claim",
    },
    ScopeDimension {
        name: "tissue",
        class: ScopeClass::Specimen,
        erasure_error: "a blood-derived normal is treated as tumour tissue",
    },
    ScopeDimension {
        name: "lesion",
        class: ScopeClass::Region,
        erasure_error: "measurements from two lesions in one subject are averaged as one",
    },
    ScopeDimension {
        name: "region",
        class: ScopeClass::Region,
        erasure_error: "an enhancing-core statistic is read as a whole-tumour statistic",
    },
    ScopeDimension {
        name: "specimen",
        class: ScopeClass::Specimen,
        erasure_error: "a consumed specimen is scheduled for a second destructive assay",
    },
    ScopeDimension {
        name: "aliquot",
        class: ScopeClass::Specimen,
        erasure_error: "quantity is not conserved across a split, so more material is spent than existed",
    },
    ScopeDimension {
        name: "cell",
        class: ScopeClass::Specimen,
        erasure_error: "a single-cell fraction is read as a bulk fraction",
    },
    ScopeDimension {
        name: "assay",
        class: ScopeClass::Specimen,
        erasure_error: "a negative from one assay is credited with another assay's sensitivity",
    },
    ScopeDimension {
        name: "sequence",
        class: ScopeClass::Coordinate,
        erasure_error: "a T1 intensity is compared against a T2 intensity",
    },
    ScopeDimension {
        name: "genome_build",
        class: ScopeClass::Coordinate,
        erasure_error: "a GRCh37 position is looked up in a GRCh38 annotation",
    },
    ScopeDimension {
        name: "transcript",
        class: ScopeClass::Coordinate,
        erasure_error: "a protein-level consequence is computed against the wrong isoform",
    },
    ScopeDimension {
        name: "coordinate_frame",
        class: ScopeClass::Coordinate,
        erasure_error: "a viewport voxel is dragged onto a pathology region and silently identified",
    },
    ScopeDimension {
        name: "units",
        class: ScopeClass::Coordinate,
        erasure_error: "a millimetre volume is summed with a centimetre volume",
    },
    ScopeDimension {
        name: "time",
        class: ScopeClass::Time,
        erasure_error: "evidence released after the decision cut is used to make the decision",
    },
    ScopeDimension {
        name: "intervention",
        class: ScopeClass::Time,
        erasure_error: "a post-treatment scan is compared to baseline as if untreated",
    },
    ScopeDimension {
        name: "cohort",
        class: ScopeClass::Identity,
        erasure_error: "a training cohort statistic is presented as external validation",
    },
    ScopeDimension {
        name: "study",
        class: ScopeClass::Identity,
        erasure_error: "two studies' inclusion criteria are pooled without a transport argument",
    },
    ScopeDimension {
        name: "classifier_version",
        class: ScopeClass::Ontology,
        erasure_error: "a WHO 2016 integrated diagnosis is compared against a WHO 2021 one",
    },
    ScopeDimension {
        name: "consent",
        class: ScopeClass::Policy,
        erasure_error: "evidence outside its consent scope enters a compiled context",
    },
];

/// A reusable factor signature, in 43.31's own notation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FactorTemplate {
    pub name: &'static str,
    /// Input scope dimensions, which must all appear in [`DIMENSIONS`].
    pub inputs: &'static [&'static str],
    pub output: &'static str,
    /// The unit or frame the output is stated in. `None` means dimensionless.
    pub unit: Option<&'static str>,
    /// How uncertainty on the output is represented. Never "not modelled".
    pub uncertainty: &'static str,
    /// Dimensions that must enter protected closure whenever this factor is in a slice.
    pub protected: &'static [&'static str],
}

/// The three signatures 43.31 writes out, plus the ones the first vertical slice needs.
pub const FACTOR_TEMPLATES: &[FactorTemplate] = &[
    FactorTemplate {
        name: "specimen_quantity",
        inputs: &["specimen", "aliquot"],
        output: "quantity",
        unit: Some("microgram"),
        uncertainty: "interval, from the balance's stated precision",
        protected: &["specimen", "aliquot"],
    },
    FactorTemplate {
        name: "variant_annotation",
        inputs: &["genome_build", "transcript"],
        output: "annotation",
        unit: None,
        uncertainty: "alternative annotations retained as a set when transcripts disagree",
        protected: &["genome_build", "transcript", "classifier_version"],
    },
    FactorTemplate {
        name: "segmentation_volume",
        inputs: &["sequence", "region", "coordinate_frame"],
        output: "volume",
        unit: Some("cubic millimetre"),
        uncertainty: "inter-rater interval over the segmentations available",
        protected: &["coordinate_frame", "units", "region"],
    },
    FactorTemplate {
        name: "assay_detection",
        inputs: &["assay", "specimen"],
        output: "observation",
        unit: None,
        uncertainty: "detection limit and coverage, carried on the observation itself",
        protected: &["assay", "specimen"],
    },
    FactorTemplate {
        name: "integrated_diagnosis",
        inputs: &["subject", "lesion", "classifier_version"],
        output: "diagnosis",
        unit: None,
        uncertainty: "expert disagreement retained as a distribution, never collapsed to a mode",
        protected: &["classifier_version", "subject", "lesion"],
    },
];

/// What was actually observed, with the four absences kept apart.
///
/// 43.31: "Missing, negative, censored, and failed are distinct." They are distinct here in the
/// type, so no code path can produce one where another was meant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "observation")]
pub enum Observation {
    /// A value was measured.
    Measured { value: f64, unit: String },
    /// The assay ran and saw nothing. Constrains a hypothesis only above the detection limit.
    Negative(NegativeResult),
    /// Nobody looked. Carries no information about the value at all.
    Missing { reason: String },
    /// The value is known to lie beyond a bound, without being known.
    Censored { bound: f64, unit: String, above: bool },
    /// The assay ran and did not produce a usable result. Distinct from `Negative`: a failed run
    /// licenses nothing, and treating it as a negative is how a detection limit gets credited to
    /// an assay that never reported one.
    Failed { reason: String },
}

impl Observation {
    /// Whether this observation can constrain any hypothesis.
    ///
    /// `Missing` and `Failed` are always false. A `Negative` depends on its detection limit, which
    /// is why the argument is required.
    pub fn is_informative(&self, expected_fraction: f64) -> bool {
        match self {
            Observation::Measured { .. } | Observation::Censored { .. } => true,
            Observation::Missing { .. } | Observation::Failed { .. } => false,
            Observation::Negative(result) => result.is_informative_about(expected_fraction),
        }
    }
}

/// A negative sequencing result, with everything needed to know what it rules out.
///
/// 43.31's worked micro-example in one struct:
///
/// > A "negative" sequencing result is encoded with assay, specimen, coverage, purity, limit of
/// > detection, and pipeline version. It is not reduced to a Boolean node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NegativeResult {
    pub assay: String,
    pub specimen: String,
    /// Reads covering the locus.
    pub coverage: u32,
    /// Tumour fraction of the specimen, in `[0, 1]`.
    pub purity: f64,
    /// Smallest variant allele fraction the assay reports, in `[0, 1]`.
    pub limit_of_detection: f64,
    pub pipeline_version: String,
}

impl NegativeResult {
    pub fn new(
        assay: impl Into<String>,
        specimen: impl Into<String>,
        coverage: u32,
        purity: f64,
        limit_of_detection: f64,
        pipeline_version: impl Into<String>,
    ) -> Result<Self, EpistemicError> {
        let assay = assay.into();
        for (label, value) in [("purity", purity), ("limit_of_detection", limit_of_detection)] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(EpistemicError::InadmissibleCost {
                    item: format!("{assay}/{label}"),
                    value,
                });
            }
        }
        Ok(NegativeResult {
            assay,
            specimen: specimen.into(),
            coverage,
            purity,
            limit_of_detection,
            pipeline_version: pipeline_version.into(),
        })
    }

    /// `P(this negative | a model whose true variant allele fraction is `expected_fraction`)`.
    ///
    /// Two regimes, and the discontinuity between them is the point of the type:
    ///
    /// - Below the detection limit after correcting for purity, the assay is blind. The
    ///   probability of a negative is exactly `1.0` under every such model, so the likelihood
    ///   ratio is flat and the observation moves no posterior. A negative from a blind assay is
    ///   not weak evidence of absence; it is no evidence of absence.
    /// - Above it, the probability of observing zero supporting reads at `coverage` depth is
    ///   `(1 − effective)^coverage`, the binomial zero term.
    pub fn likelihood_of_negative(&self, expected_fraction: f64) -> f64 {
        let effective = (expected_fraction * self.purity).clamp(0.0, 1.0);
        if effective < self.limit_of_detection {
            return 1.0;
        }
        (1.0 - effective).powi(self.coverage as i32)
    }

    /// Whether this negative constrains a hypothesis with the given expected fraction.
    pub fn is_informative_about(&self, expected_fraction: f64) -> bool {
        self.likelihood_of_negative(expected_fraction) < 1.0 - 1e-12
    }

    /// Turns the negative into evidence against a set of models, each with an expected fraction.
    ///
    /// This is the join between 43.31 and 43.50: the library says what a negative *means*, and the
    /// calculus prices what it is *worth*. Without the join the library is a table nothing reads.
    pub fn as_evidence(
        &self,
        id: impl Into<String>,
        cost: f64,
        expected_fraction_per_model: &[f64],
    ) -> Result<EvidenceItem, EpistemicError> {
        EvidenceItem::new(
            id,
            cost,
            expected_fraction_per_model
                .iter()
                .map(|f| self.likelihood_of_negative(*f))
                .collect(),
        )
    }
}

/// How one library dimension lines up with `bioprism-scope`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum DimensionAgreement {
    /// Both give the same class.
    Agreed { name: String, class: String },
    /// `bioprism-scope`'s default registry does not know the name, so it classifies as
    /// `Unclassified` and no closure rule keyed on class will fire for it.
    RegistryUnclassified { name: String, library_class: String },
    /// Both classify it and they differ. Neither is edited here.
    Conflict {
        name: String,
        library_class: String,
        registry_class: String,
    },
}

/// The full comparison of this library against `bioprism_scope::DimensionRegistry`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisagreementReport {
    pub entries: Vec<DimensionAgreement>,
    pub agreed: usize,
    pub unclassified: usize,
    pub conflicts: usize,
}

impl DisagreementReport {
    /// Names the library dimensions the registry cannot classify.
    ///
    /// Every one of these is a dimension a fact can be scoped by and that protected closure cannot
    /// reason about by class.
    pub fn unclassified_names(&self) -> Vec<&str> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                DimensionAgreement::RegistryUnclassified { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Compares every [`DIMENSIONS`] entry against the default `bioprism-scope` registry.
pub fn disagreement_with_scope_registry() -> DisagreementReport {
    let registry = DimensionRegistry::default();
    let mut entries = Vec::with_capacity(DIMENSIONS.len());
    let (mut agreed, mut unclassified, mut conflicts) = (0usize, 0usize, 0usize);

    for dimension in DIMENSIONS {
        let registry_class = registry.classify(dimension.name);
        if !registry_class.is_classified() {
            unclassified += 1;
            entries.push(DimensionAgreement::RegistryUnclassified {
                name: dimension.name.to_string(),
                library_class: dimension.class.as_str().to_string(),
            });
        } else if registry_class == dimension.class {
            agreed += 1;
            entries.push(DimensionAgreement::Agreed {
                name: dimension.name.to_string(),
                class: dimension.class.as_str().to_string(),
            });
        } else {
            conflicts += 1;
            entries.push(DimensionAgreement::Conflict {
                name: dimension.name.to_string(),
                library_class: dimension.class.as_str().to_string(),
                registry_class: registry_class.as_str().to_string(),
            });
        }
    }

    DisagreementReport {
        entries,
        agreed,
        unclassified,
        conflicts,
    }
}

/// Factor templates naming an input this library does not define.
///
/// Empty is the healthy state. A non-empty result means the catalogue has drifted from its own
/// vocabulary, which is the failure 43.31's "domain semantics are not inferred from filenames
/// alone" is about, one level up.
pub fn templates_with_undefined_inputs() -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    for template in FACTOR_TEMPLATES {
        for input in template.inputs.iter().chain(template.protected.iter()) {
            if !DIMENSIONS.iter().any(|d| d.name == *input) {
                out.push((template.name, *input));
            }
        }
    }
    out
}
