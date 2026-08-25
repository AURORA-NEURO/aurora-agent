//! AssayLens: what a measurement can and cannot tell you.
//!
//! Implements blueprint 25.05. A lens is the whole path from material to number — target
//! quantity, material cost, instrument, protocol, processing chain, calibration, error model
//! and the comparisons the result may legally enter. Two of the blueprint's invariants shape
//! the type:
//!
//! **"A lens cannot claim greater identifiability than its calibration supports."** A relative
//! expression pipeline does not measure transcripts per cell no matter what the column header
//! says. [`AssayLens::validate`] refuses the claim rather than annotating it, because an
//! annotated over-claim is still an over-claim by the time it reaches a model.
//!
//! **"Processing versions are part of the lens identity."** Two runs of "the same" lens with
//! different aligner versions are two lenses. [`AssayLens::identity_hash`] therefore hashes the
//! processing chain, and [`AssayLens::comparable_with`] reports a changed step version as a
//! named reason rather than as a boolean `false`.
//!
//! # Why comparability returns a reason
//!
//! `bool` is the wrong return type. "These two numbers are not comparable" is actionable only
//! if you know *why*: a unit mismatch is a conversion, an uncontrolled batch is a covariate, a
//! changed aligner version is a re-run, and a required bridging study is a new experiment.
//! [`Incomparability`] names which of those you are looking at.
//!
//! # Not implemented
//!
//! 25.05 requires a "raw output schema" and a "processing path" and specifies neither.
//! [`ProtocolChain`] carries named, versioned steps and nothing about their input or output
//! types, so this crate cannot check that a chain composes. It also does not implement
//! quantitative error propagation: [`ErrorModel`] records the declared noise form and
//! missingness class, and 25.12 owns what happens to uncertainty downstream.

use crate::error::LensError;
use crate::ids::{LensId, SpecimenId};
use crate::lineage::LineageGraph;
use crate::quantity::Quantity;
use bioprism_ids::ContentHash;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use thiserror::Error;

/// The measurement scale of the target quantity.
///
/// Carried because 39.05 protects "units, coordinate system, ... and scale" and because the
/// scale decides which comparisons are meaningful at all: a nominal call and a ratio quantity
/// are not the same kind of answer even when both are serialised as numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementScale {
    Nominal,
    Ordinal,
    Interval,
    Ratio,
}

impl fmt::Display for MeasurementScale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            MeasurementScale::Nominal => "nominal",
            MeasurementScale::Ordinal => "ordinal",
            MeasurementScale::Interval => "interval",
            MeasurementScale::Ratio => "ratio",
        };
        f.write_str(name)
    }
}

/// How much the lens claims its number means on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Identifiability {
    /// Meaningful only against other values from the same run or batch.
    Relative,
    /// Ordered and roughly interpretable, but not on a physical scale.
    SemiQuantitative,
    /// A physical amount, transportable between runs.
    AbsoluteQuantity,
}

impl fmt::Display for Identifiability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Identifiability::Relative => "relative",
            Identifiability::SemiQuantitative => "semi-quantitative",
            Identifiability::AbsoluteQuantity => "absolute",
        };
        f.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementTarget {
    /// What is being measured: "gene expression", "ADC", "MGMT promoter methylation".
    pub quantity: String,
    /// The entity the quantity belongs to, in the source vocabulary. Binding is 25.03's job.
    pub entity: String,
    pub unit: String,
    pub scale: MeasurementScale,
    pub identifiability: Identifiability,
}

/// What the lens consumes to produce one measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialRequirement {
    pub material: String,
    pub minimum: Quantity,
    /// True when running the lens destroys the input. 25.05 calls this the destructive cost;
    /// it is the field that makes a measurement plan a resource-allocation problem.
    pub destructive: bool,
}

/// One named, versioned stage of processing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProcessingStep {
    pub name: String,
    pub version: String,
}

impl ProcessingStep {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        ProcessingStep {
            name: name.into(),
            version: version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolChain {
    pub instrument: String,
    pub protocol: String,
    pub protocol_version: String,
    pub steps: Vec<ProcessingStep>,
}

impl ProtocolChain {
    fn step_versions(&self) -> BTreeMap<&str, &str> {
        self.steps
            .iter()
            .map(|step| (step.name.as_str(), step.version.as_str()))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "calibration", rename_all = "snake_case")]
pub enum CalibrationKind {
    /// No calibration at all. Supports [`Identifiability::Relative`] and nothing more.
    Uncalibrated,
    /// Normalised against an internal anchor: a housekeeping gene, a reference region.
    Relative { anchor: String },
    /// Traced to an external reference standard with a known value.
    AbsoluteAgainstStandard { standard: String },
}

impl CalibrationKind {
    /// The strongest identifiability this calibration can support.
    fn supports(&self) -> Identifiability {
        match self {
            CalibrationKind::Uncalibrated => Identifiability::Relative,
            CalibrationKind::Relative { .. } => Identifiability::SemiQuantitative,
            CalibrationKind::AbsoluteAgainstStandard { .. } => Identifiability::AbsoluteQuantity,
        }
    }
}

impl fmt::Display for CalibrationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalibrationKind::Uncalibrated => f.write_str("uncalibrated"),
            CalibrationKind::Relative { anchor } => write!(f, "relative to {anchor}"),
            CalibrationKind::AbsoluteAgainstStandard { standard } => {
                write!(f, "absolute against {standard}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub kind: CalibrationKind,
    pub calibrated_at: Option<Timestamp>,
    /// The value below which the lens cannot distinguish presence from absence.
    ///
    /// Without it a negative result is uninterpretable, which is why 25.05 requires negative
    /// results to include sensitivity context and why [`AssayLens::check_reading`] enforces it.
    pub limit_of_detection: Option<f64>,
    pub limit_of_quantification: Option<f64>,
}

impl Calibration {
    pub fn uncalibrated() -> Self {
        Calibration {
            kind: CalibrationKind::Uncalibrated,
            calibrated_at: None,
            limit_of_detection: None,
            limit_of_quantification: None,
        }
    }
}

/// Why a value can be absent. Collapsing these to "null" destroys the reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingnessClass {
    NotMissing,
    MissingCompletelyAtRandom,
    MissingAtRandom,
    /// Missing *because* of the value: the assay failed on exactly the hardest samples.
    MissingNotAtRandom,
    BelowDetection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorModel {
    /// The declared noise form, e.g. "additive gaussian", "negative binomial".
    pub form: String,
    pub noise_sd: Option<f64>,
    pub missingness: MissingnessClass,
    /// Failure modes the lens is known to produce: batch drift, motion artifact, index hopping.
    pub known_artifacts: Vec<String>,
}

/// What must match before two measurements from this lens may be compared.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparabilityRule {
    pub requires_same_lens: bool,
    pub requires_same_processing_versions: bool,
    pub requires_same_batch: bool,
    pub requires_same_site: bool,
    /// A named study that establishes a mapping between this lens and others.
    ///
    /// Its presence does not make measurements comparable. It names the work that would.
    pub bridging_study: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QcMetric {
    pub name: String,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QcContract {
    pub metrics: Vec<QcMetric>,
}

/// The QC verdict attached to one measurement.
///
/// `Ungradable` is not `Fail`. A slide that could not be read tells you nothing about the
/// specimen; a slide that failed a cellularity threshold tells you something. 25.12 requires an
/// ungradable reason for the same reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "qc", rename_all = "snake_case")]
pub enum QcOutcome {
    Pass,
    Fail { metric: String, value: String },
    Ungradable { reason: String },
}

impl fmt::Display for QcOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QcOutcome::Pass => f.write_str("pass"),
            QcOutcome::Fail { metric, value } => write!(f, "failed {metric} at {value}"),
            QcOutcome::Ungradable { reason } => write!(f, "ungradable: {reason}"),
        }
    }
}

/// What the lens actually returned.
///
/// `BelowLimitOfDetection` and `Absent` are separate from a zero quantity on purpose. "We saw
/// none" and "we saw none and could have seen 0.01" are different claims, and only the second
/// supports a negative conclusion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reading", rename_all = "snake_case")]
pub enum Reading {
    Quantity { value: f64 },
    Categorical { label: String },
    BelowLimitOfDetection,
    Absent,
}

impl Reading {
    /// True for readings that assert the target was not found.
    pub fn is_negative_call(&self) -> bool {
        matches!(self, Reading::BelowLimitOfDetection | Reading::Absent)
    }
}

/// One number produced by one lens on one specimen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub lens: LensId,
    pub lens_version: String,
    pub specimen: SpecimenId,
    pub reading: Reading,
    pub unit: String,
    pub batch: Option<String>,
    pub site: Option<String>,
    pub observed_at: Timestamp,
    pub qc: QcOutcome,
}

impl Measurement {
    pub fn new(
        lens: LensId,
        lens_version: impl Into<String>,
        specimen: SpecimenId,
        reading: Reading,
        unit: impl Into<String>,
        observed_at: Timestamp,
    ) -> Self {
        Measurement {
            lens,
            lens_version: lens_version.into(),
            specimen,
            reading,
            unit: unit.into(),
            batch: None,
            site: None,
            observed_at,
            qc: QcOutcome::Pass,
        }
    }

    pub fn in_batch(mut self, batch: impl Into<String>) -> Self {
        self.batch = Some(batch.into());
        self
    }

    pub fn with_qc(mut self, qc: QcOutcome) -> Self {
        self.qc = qc;
        self
    }

    fn label(&self) -> String {
        format!("{}@{} on {}", self.lens, self.lens_version, self.specimen)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssayLens {
    pub id: LensId,
    pub version: String,
    pub target: MeasurementTarget,
    pub material: MaterialRequirement,
    pub protocol: ProtocolChain,
    pub calibration: Calibration,
    pub error_model: ErrorModel,
    pub comparability: ComparabilityRule,
    pub qc: QcContract,
    /// Documented ways this lens is known to be wrong, in the source vocabulary.
    pub known_failure_modes: BTreeSet<String>,
}

/// The identity-bearing projection of a lens.
///
/// Hashing the whole lens would make an edit to `known_failure_modes` — documentation — look
/// like a new instrument. Hashing only the id and version would let an aligner upgrade pass as
/// the same lens, which 25.05 explicitly forbids.
#[derive(Serialize)]
struct LensIdentity<'a> {
    id: &'a str,
    version: &'a str,
    target: &'a MeasurementTarget,
    protocol: &'a ProtocolChain,
    calibration: &'a Calibration,
}

impl AssayLens {
    /// Declaration-level checks that need no measurement.
    pub fn validate(&self) -> Result<(), LensError> {
        let supported = self.calibration.kind.supports();
        if self.target.identifiability > supported {
            return Err(LensError::UncalibratedAbsoluteClaim {
                lens: self.id.to_string(),
                claimed: self.target.identifiability.to_string(),
                calibration: self.calibration.kind.to_string(),
            });
        }
        for metric in &self.qc.metrics {
            if let (Some(minimum), Some(maximum)) = (metric.minimum, metric.maximum) {
                if minimum > maximum {
                    return Err(LensError::EmptyQcBand {
                        lens: self.id.to_string(),
                        metric: metric.name.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// A content hash over everything that makes this lens a distinct measuring device.
    pub fn identity_hash(&self) -> Result<ContentHash, LensError> {
        let projection = LensIdentity {
            id: self.id.as_str(),
            version: &self.version,
            target: &self.target,
            protocol: &self.protocol,
            calibration: &self.calibration,
        };
        let value = serde_json::to_value(&projection).map_err(|error| LensError::Canonical {
            lens: self.id.to_string(),
            message: error.to_string(),
        })?;
        ContentHash::of_value(&value).map_err(|error| LensError::canonical(self.id.as_str(), error))
    }

    /// Whether `specimen` can supply this lens, given what its lineage says is left.
    ///
    /// Uses remaining quantity rather than collected quantity: a tube that has already been
    /// aliquoted three times does not still hold what the collection record says it held.
    pub fn admits(&self, lineage: &LineageGraph, specimen: &SpecimenId) -> Result<(), LensError> {
        let material = lineage.get(specimen)?;
        if material.material != self.material.material {
            return Err(LensError::WrongMaterial {
                lens: self.id.to_string(),
                specimen: specimen.to_string(),
                required: self.material.material.clone(),
                found: material.material.clone(),
            });
        }
        let available = lineage.remaining(specimen)?;
        if self
            .material
            .minimum
            .exceeds(&available, specimen.as_str())?
        {
            return Err(LensError::InsufficientMaterial {
                lens: self.id.to_string(),
                specimen: specimen.to_string(),
                required: self.material.minimum.to_string(),
                available: available.to_string(),
            });
        }
        Ok(())
    }

    /// A negative call is only a claim if the lens can say how small a signal it would have seen.
    pub fn check_reading(&self, measurement: &Measurement) -> Result<(), LensError> {
        if measurement.reading.is_negative_call() && self.calibration.limit_of_detection.is_none() {
            return Err(LensError::NegativeWithoutSensitivity {
                lens: self.id.to_string(),
            });
        }
        Ok(())
    }

    /// Whether values from this lens may be placed alongside values from `other`.
    ///
    /// Returns the *first* blocking reason rather than all of them: the reasons are ordered
    /// from most fundamental (different target) to most tractable (an uncontrolled site), and
    /// reporting a batch effect on two lenses that measure different quantities would bury the
    /// real problem.
    pub fn comparable_with(&self, other: &AssayLens) -> Result<(), Incomparability> {
        if self.target.quantity != other.target.quantity {
            return Err(Incomparability::DifferentTarget {
                left: self.target.quantity.clone(),
                right: other.target.quantity.clone(),
            });
        }
        if self.target.unit != other.target.unit {
            return Err(Incomparability::DifferentUnit {
                left: self.target.unit.clone(),
                right: other.target.unit.clone(),
            });
        }
        if self.target.scale != other.target.scale {
            return Err(Incomparability::DifferentScale {
                left: self.target.scale.to_string(),
                right: other.target.scale.to_string(),
            });
        }
        if self.calibration.kind.supports() != other.calibration.kind.supports() {
            return Err(Incomparability::CalibrationIncompatible {
                left: self.calibration.kind.to_string(),
                right: other.calibration.kind.to_string(),
            });
        }
        if self.id != other.id {
            if let Some(study) = self
                .comparability
                .bridging_study
                .as_ref()
                .or(other.comparability.bridging_study.as_ref())
            {
                return Err(Incomparability::BridgingStudyRequired {
                    study: study.clone(),
                });
            }
            if self.comparability.requires_same_lens || other.comparability.requires_same_lens {
                return Err(Incomparability::DifferentLens {
                    left: self.id.to_string(),
                    right: other.id.to_string(),
                });
            }
        }
        if self.comparability.requires_same_processing_versions
            || other.comparability.requires_same_processing_versions
        {
            let mine = self.protocol.step_versions();
            let theirs = other.protocol.step_versions();
            let names: BTreeSet<&str> =
                mine.keys().copied().chain(theirs.keys().copied()).collect();
            for name in names {
                let left = mine.get(name).copied().unwrap_or("absent");
                let right = theirs.get(name).copied().unwrap_or("absent");
                if left != right {
                    return Err(Incomparability::ProcessingVersionChanged {
                        step: name.to_string(),
                        left: left.to_string(),
                        right: right.to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Why two measurements may not be placed side by side.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Incomparability {
    #[error("lenses measure different quantities: {left:?} and {right:?}")]
    DifferentTarget { left: String, right: String },

    #[error("lenses report different units: {left:?} and {right:?}")]
    DifferentUnit { left: String, right: String },

    #[error("lenses report on different scales: {left} and {right}")]
    DifferentScale { left: String, right: String },

    #[error("calibrations are not interchangeable: {left} and {right}")]
    CalibrationIncompatible { left: String, right: String },

    #[error("measurements come from different lenses {left:?} and {right:?}, which this lens does not permit")]
    DifferentLens { left: String, right: String },

    #[error("processing step {step:?} differs: {left:?} and {right:?}")]
    ProcessingVersionChanged {
        step: String,
        left: String,
        right: String,
    },

    #[error("comparison requires bridging study {study:?}, which does not make the raw values interchangeable")]
    BridgingStudyRequired { study: String },

    #[error("measurements were taken in different batches {left:?} and {right:?} and the lens does not control for batch")]
    UncontrolledBatch { left: String, right: String },

    #[error("measurements were taken at different sites {left:?} and {right:?} and the lens does not control for site")]
    UncontrolledSite { left: String, right: String },

    #[error("measurement {measurement:?} did not pass QC: {detail}")]
    QualityGateFailed { measurement: String, detail: String },

    #[error("measurement {measurement:?} reports unit {unit:?}, which its lens does not produce")]
    UnitNotProducedByLens { measurement: String, unit: String },

    #[error("lens {lens:?} version {version:?} is not in the catalog")]
    UnknownLens { lens: String, version: String },
}

/// The lenses an evaluation is allowed to draw on, keyed by identifier and version.
///
/// Version is part of the key, not a field to overwrite: 25.05 makes processing versions part
/// of lens identity, so a catalog that let version 2 replace version 1 would silently
/// re-interpret every historical measurement that named version 1.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LensCatalog {
    lenses: BTreeMap<LensId, BTreeMap<String, AssayLens>>,
}

impl LensCatalog {
    pub fn new() -> Self {
        LensCatalog {
            lenses: BTreeMap::new(),
        }
    }

    /// Registers a lens after checking its own declaration.
    pub fn register(&mut self, lens: AssayLens) -> Result<(), LensError> {
        lens.validate()?;
        let versions = self.lenses.entry(lens.id.clone()).or_default();
        if versions.contains_key(&lens.version) {
            return Err(LensError::DuplicateLens {
                lens: lens.id.to_string(),
                version: lens.version.clone(),
            });
        }
        versions.insert(lens.version.clone(), lens);
        Ok(())
    }

    pub fn get(&self, lens: &LensId, version: &str) -> Result<&AssayLens, LensError> {
        self.lenses
            .get(lens)
            .and_then(|versions| versions.get(version))
            .ok_or_else(|| LensError::UnknownLens {
                lens: format!("{lens}@{version}"),
            })
    }

    pub fn len(&self) -> usize {
        self.lenses.values().map(BTreeMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.lenses.values().all(BTreeMap::is_empty)
    }

    /// Whether two measurements may be placed side by side, and why not when they may not.
    ///
    /// Checks in three layers: the measurements themselves must be gradable, the lenses must
    /// be comparable, and the conditions the lenses say they do not control for must match.
    pub fn comparable_with(
        &self,
        left: &Measurement,
        right: &Measurement,
    ) -> Result<(), Incomparability> {
        let left_lens = self.resolve(left)?;
        let right_lens = self.resolve(right)?;

        for measurement in [left, right] {
            if measurement.qc != QcOutcome::Pass {
                return Err(Incomparability::QualityGateFailed {
                    measurement: measurement.label(),
                    detail: measurement.qc.to_string(),
                });
            }
        }
        for (measurement, lens) in [(left, left_lens), (right, right_lens)] {
            if measurement.unit != lens.target.unit {
                return Err(Incomparability::UnitNotProducedByLens {
                    measurement: measurement.label(),
                    unit: measurement.unit.clone(),
                });
            }
        }

        left_lens.comparable_with(right_lens)?;

        let controls_batch = left_lens.comparability.requires_same_batch
            || right_lens.comparability.requires_same_batch;
        if controls_batch && left.batch != right.batch {
            return Err(Incomparability::UncontrolledBatch {
                left: describe(&left.batch),
                right: describe(&right.batch),
            });
        }
        let controls_site = left_lens.comparability.requires_same_site
            || right_lens.comparability.requires_same_site;
        if controls_site && left.site != right.site {
            return Err(Incomparability::UncontrolledSite {
                left: describe(&left.site),
                right: describe(&right.site),
            });
        }
        Ok(())
    }

    fn resolve(&self, measurement: &Measurement) -> Result<&AssayLens, Incomparability> {
        self.lenses
            .get(&measurement.lens)
            .and_then(|versions| versions.get(&measurement.lens_version))
            .ok_or_else(|| Incomparability::UnknownLens {
                lens: measurement.lens.to_string(),
                version: measurement.lens_version.clone(),
            })
    }
}

fn describe(value: &Option<String>) -> String {
    value.clone().unwrap_or_else(|| "unrecorded".to_string())
}
