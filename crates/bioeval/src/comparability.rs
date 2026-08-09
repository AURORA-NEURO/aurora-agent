//! Comparability gates (26.10, 26.08).
//!
//! A score computed across measurements that were never on the same footing is not a weak score,
//! it is not a score. Two RNA-seq cohorts quantified with different pipelines differ by more than
//! the biology under test; a coordinate compared across GRCh37 and GRCh38 is a different locus;
//! micrograms against milligrams is a factor of a thousand wearing the same label. 26.10 asks
//! whether performance "survives biologically and technically meaningful shifts", and the
//! precondition for asking is knowing when a comparison was legitimate at all.
//!
//! So the gate is a *type*, not a warning. [`gate`] returns a [`ComparabilityWitness`], the
//! witness has no public constructor, and [`crate::score::Grader::score`] demands one. A caller
//! who wants a number across two frames must either prove the frames match or name the bridge
//! that reconciles them. There is no path that produces a number and a caveat.
//!
//! # Silence is not agreement
//!
//! The sharpest rule here: two frames that both leave `coordinate_frame` undeclared are **not**
//! comparable. Undeclared matches undeclared trivially under equality, and that is exactly the
//! failure — nobody knows whether the frames agree, and the absence of evidence has been read as
//! evidence of absence. [`Incomparability::Undeclared`] is the same move as
//! `InfluenceClass::Unknown` in blueprint 43.26: representable, and disqualifying.
//!
//! # Not implemented
//!
//! Bridges carry a declared loss but the loss is not propagated into the score's uncertainty
//! band. 26.10 would want a liftover with 2% unmappable positions to widen the interval on every
//! coordinate claim downstream; here it is recorded on the witness and left for a consumer to
//! read. Nor is there a bridge *registry*: the caller supplies the bridge, and this module only
//! checks that one was supplied and stays inside its stated tolerance.

use std::collections::BTreeMap;
use std::fmt;

use bioprism_scope::ScopeClass;
use serde::{Deserialize, Serialize};

/// A dimension along which two measurements can fail to be comparable.
///
/// Closed on purpose. Each entry maps to a class of the typed scope base of blueprint 43.03, so
/// that a comparability failure can be traced to the scope dimension that caused it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameDimension {
    /// Sequencer, array, stain, scanner, antibody clone.
    AssayPlatform,
    /// Genome or transcriptome build. GRCh37 and GRCh38 name different positions.
    ReferenceBuild,
    /// Anatomical or image coordinate convention. RAS against LPS silently mirrors laterality.
    CoordinateFrame,
    /// The unit the value is expressed in.
    Unit,
    /// The normalisation or batch-correction applied. TPM against raw counts against
    /// batch-corrected TPM are three incomparable quantities.
    Normalisation,
    /// Fixation, dissociation, storage. FFPE and fresh-frozen are not interchangeable inputs.
    SpecimenPreparation,
}

impl FrameDimension {
    pub const CANONICAL: [FrameDimension; 6] = [
        FrameDimension::AssayPlatform,
        FrameDimension::ReferenceBuild,
        FrameDimension::CoordinateFrame,
        FrameDimension::Unit,
        FrameDimension::Normalisation,
        FrameDimension::SpecimenPreparation,
    ];

    /// The scope class of blueprint 43.03 this dimension lives in.
    pub fn scope_class(self) -> ScopeClass {
        match self {
            FrameDimension::AssayPlatform | FrameDimension::SpecimenPreparation => {
                ScopeClass::Specimen
            }
            FrameDimension::ReferenceBuild | FrameDimension::CoordinateFrame => {
                ScopeClass::Coordinate
            }
            FrameDimension::Unit | FrameDimension::Normalisation => ScopeClass::Ontology,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            FrameDimension::AssayPlatform => "assay_platform",
            FrameDimension::ReferenceBuild => "reference_build",
            FrameDimension::CoordinateFrame => "coordinate_frame",
            FrameDimension::Unit => "unit",
            FrameDimension::Normalisation => "normalisation",
            FrameDimension::SpecimenPreparation => "specimen_preparation",
        }
    }
}

impl fmt::Display for FrameDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which side of a comparison failed to declare a dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameSide {
    Prediction,
    Reference,
    Both,
}

impl FrameSide {
    pub fn as_str(self) -> &'static str {
        match self {
            FrameSide::Prediction => "prediction",
            FrameSide::Reference => "reference",
            FrameSide::Both => "both",
        }
    }
}

/// The declared measurement conditions of one side of a comparison.
///
/// Empty by default, which is the honest starting point: a frame that has declared nothing
/// cannot pass a gate that requires anything.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MeasurementFrame {
    declared: BTreeMap<FrameDimension, String>,
}

impl MeasurementFrame {
    pub fn declaring(entries: impl IntoIterator<Item = (FrameDimension, String)>) -> Self {
        MeasurementFrame {
            declared: entries.into_iter().collect(),
        }
    }

    pub fn with(mut self, dimension: FrameDimension, value: impl Into<String>) -> Self {
        self.declared.insert(dimension, value.into());
        self
    }

    pub fn get(&self, dimension: FrameDimension) -> Option<&str> {
        self.declared.get(&dimension).map(String::as_str)
    }

    pub fn declared_dimensions(&self) -> impl Iterator<Item = FrameDimension> + '_ {
        self.declared.keys().copied()
    }
}

/// A named, caller-supplied transformation that makes two values on one dimension comparable.
///
/// A liftover, a unit conversion, a platform-harmonisation model. The bridge must be *declared*:
/// this module never infers that GRCh37 and GRCh38 are reconcilable, because inferring it is how
/// a benchmark ends up scoring positions that were never aligned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bridge {
    pub bridge_id: String,
    pub dimension: FrameDimension,
    pub from: String,
    pub to: String,
    /// The share of the quantity the bridge does not preserve — unmappable positions, rounding,
    /// harmonisation residual. Recorded, never zero by default.
    pub loss: f64,
}

impl Bridge {
    pub fn applies_to(&self, dimension: FrameDimension, left: &str, right: &str) -> bool {
        self.dimension == dimension
            && ((self.from == left && self.to == right) || (self.from == right && self.to == left))
    }
}

/// Why two measurements cannot be compared.
///
/// A list, not a single reason. [`gate`] reports every failing dimension at once, because fixing
/// one and re-running to discover the next is how a comparability problem gets declared solved
/// halfway through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Incomparability {
    /// Both sides declared the dimension and the declarations differ, with no bridge offered.
    ValueDiffers {
        dimension: FrameDimension,
        prediction: String,
        reference: String,
    },
    /// At least one side never declared the dimension. Two undeclared sides land here too:
    /// mutual silence is not a match.
    Undeclared {
        dimension: FrameDimension,
        side: FrameSide,
    },
    /// A bridge was offered and its declared loss exceeds what the requirement tolerates.
    BridgeTooLossy {
        dimension: FrameDimension,
        bridge_id: String,
        loss: f64,
        tolerance: f64,
    },
}

impl Incomparability {
    pub fn dimension(&self) -> FrameDimension {
        match self {
            Incomparability::ValueDiffers { dimension, .. }
            | Incomparability::Undeclared { dimension, .. }
            | Incomparability::BridgeTooLossy { dimension, .. } => *dimension,
        }
    }
}

/// Which dimensions this comparison must reconcile, and how much bridge loss it tolerates.
///
/// Named and versioned so that a published score can state the gate it passed. 26.20 forbids
/// "retroactive weight changes"; the same applies to quietly relaxing a comparability gate after
/// seeing which submissions it excluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparabilityRequirement {
    pub requirement_id: String,
    pub dimensions: Vec<FrameDimension>,
    /// Maximum declared bridge loss admitted on any single dimension.
    pub loss_tolerance: f64,
}

impl ComparabilityRequirement {
    /// Every canonical dimension must match, and no bridge loss is tolerated.
    ///
    /// The strict default is deliberate. A permissive default gate is indistinguishable from no
    /// gate, and it is the version that ends up in production.
    pub fn strict(requirement_id: impl Into<String>) -> Self {
        ComparabilityRequirement {
            requirement_id: requirement_id.into(),
            dimensions: FrameDimension::CANONICAL.to_vec(),
            loss_tolerance: 0.0,
        }
    }

    /// A gate over a named subset of dimensions — for a task where, say, specimen preparation
    /// genuinely does not enter the quantity being compared.
    pub fn over(
        requirement_id: impl Into<String>,
        dimensions: impl IntoIterator<Item = FrameDimension>,
    ) -> Self {
        ComparabilityRequirement {
            requirement_id: requirement_id.into(),
            dimensions: dimensions.into_iter().collect(),
            loss_tolerance: 0.0,
        }
    }

    pub fn tolerating_loss(mut self, tolerance: f64) -> Self {
        self.loss_tolerance = tolerance;
        self
    }
}

/// A bridge that was actually used to reconcile a dimension, retained on the witness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedBridge {
    pub bridge_id: String,
    pub dimension: FrameDimension,
    pub from: String,
    pub to: String,
    pub loss: f64,
}

/// Proof that two measurements were checked against a named requirement and found comparable.
///
/// The fields are private and there is no public constructor. [`gate`] is the only way to obtain
/// one, which is what makes "score computed without a comparability check" unrepresentable
/// rather than merely discouraged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparabilityWitness {
    requirement_id: String,
    reconciled: BTreeMap<FrameDimension, String>,
    bridges: Vec<AppliedBridge>,
}

impl ComparabilityWitness {
    pub fn requirement_id(&self) -> &str {
        &self.requirement_id
    }

    pub fn reconciled(&self, dimension: FrameDimension) -> Option<&str> {
        self.reconciled.get(&dimension).map(String::as_str)
    }

    pub fn bridges(&self) -> &[AppliedBridge] {
        &self.bridges
    }

    /// Total declared loss across every bridge used.
    ///
    /// A witness with non-zero loss is still a witness: the comparison was legitimate, and it was
    /// not free. A consumer publishing the score should say so.
    pub fn total_bridge_loss(&self) -> f64 {
        self.bridges.iter().map(|b| b.loss).sum()
    }

    pub fn is_direct(&self) -> bool {
        self.bridges.is_empty()
    }
}

/// Checks two measurement frames against a requirement.
///
/// Returns every reason the comparison fails, or a witness that it does not. `bridges` are the
/// transformations the caller is willing to stand behind; an empty slice means the frames must
/// match outright.
pub fn gate(
    requirement: &ComparabilityRequirement,
    prediction: &MeasurementFrame,
    reference: &MeasurementFrame,
    bridges: &[Bridge],
) -> Result<ComparabilityWitness, Vec<Incomparability>> {
    let mut failures = Vec::new();
    let mut reconciled = BTreeMap::new();
    let mut applied = Vec::new();

    for &dimension in &requirement.dimensions {
        match (prediction.get(dimension), reference.get(dimension)) {
            (None, None) => failures.push(Incomparability::Undeclared {
                dimension,
                side: FrameSide::Both,
            }),
            (None, Some(_)) => failures.push(Incomparability::Undeclared {
                dimension,
                side: FrameSide::Prediction,
            }),
            (Some(_), None) => failures.push(Incomparability::Undeclared {
                dimension,
                side: FrameSide::Reference,
            }),
            (Some(left), Some(right)) if left == right => {
                reconciled.insert(dimension, left.to_string());
            }
            (Some(left), Some(right)) => {
                match bridges
                    .iter()
                    .find(|b| b.applies_to(dimension, left, right))
                {
                    Some(bridge) if bridge.loss <= requirement.loss_tolerance => {
                        reconciled.insert(dimension, right.to_string());
                        applied.push(AppliedBridge {
                            bridge_id: bridge.bridge_id.clone(),
                            dimension,
                            from: left.to_string(),
                            to: right.to_string(),
                            loss: bridge.loss,
                        });
                    }
                    Some(bridge) => failures.push(Incomparability::BridgeTooLossy {
                        dimension,
                        bridge_id: bridge.bridge_id.clone(),
                        loss: bridge.loss,
                        tolerance: requirement.loss_tolerance,
                    }),
                    None => failures.push(Incomparability::ValueDiffers {
                        dimension,
                        prediction: left.to_string(),
                        reference: right.to_string(),
                    }),
                }
            }
        }
    }

    if failures.is_empty() {
        Ok(ComparabilityWitness {
            requirement_id: requirement.requirement_id.clone(),
            reconciled,
            bridges: applied,
        })
    } else {
        Err(failures)
    }
}
