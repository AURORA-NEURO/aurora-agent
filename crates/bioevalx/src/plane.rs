//! The scoring plane: one system, many dimensions, three states per cell (26.17).
//!
//! 26.17 asks for "fair comparisons among systems with different action capabilities without
//! pretending they are identical", and its protocol says to "report unavailable capabilities" and
//! "avoid penalizing a model for actions it was never designed to take". Taken seriously, that is
//! not a reporting convention — it is a claim that a cell of the evaluation grid has *three*
//! states, not two:
//!
//! | State | Means | Denominator |
//! |---|---|---|
//! | [`Cell::Scored`] | The system was asked and this is how it did | in |
//! | [`Cell::Unscored`] | Nobody asked | out, and the fold refuses |
//! | [`Cell::Inapplicable`] | Asking was meaningless: the system has no such action | out, and named |
//!
//! Collapsing the second into a zero is the failure `bioprism-atlas` names for capability cells
//! ("unmeasured is categorically distinct from measured-and-poor"). Collapsing the *third* into a
//! zero is a different and more specific failure that 26.17 exists to prevent: a fixed-input
//! predictive model scores zero on "chose the next assay" not because it chose badly but because
//! choosing was never in its action set, and a leaderboard that averages that zero has measured
//! the tier boundary and called it capability.
//!
//! # How the zero is made unreachable
//!
//! - [`Score`] has a private field and one fallible constructor. There is no `From<f64>`.
//! - [`Cell`] has no `Default`, no `score_or_zero`, and no accessor returning `f64` — the only
//!   readers are [`Cell::score`] returning `Option<&Score>` and [`Cell::as_scored`].
//! - [`ScorePlane::fold`] returns `Result` and refuses while any dimension is [`Cell::Unscored`].
//! - [`FoldPolicy`] has no variant that imputes a value. Its documentation says why.
//! - An unscored or inapplicable cell serializes with **no score key at all**, so a consumer that
//!   reads JSON cannot find a number to misread.
//!
//! A source-level test in `tests/plane.rs` fails if any file in this crate gains a
//! `unwrap_or(0.`, `unwrap_or_default` on a score, or an identifier containing `or_zero`. That is
//! the mechanism `bioprism-modalities` used for invented constants, transposed to imputation.
//!
//! # Not implemented
//!
//! No aggregation across *systems* and no ranking. `bioprism-metrics` owns that, including the
//! rule that an aggregate over a grid containing an unmeasured cell is not an aggregate over the
//! grid; this module folds one system's own dimensions and hands the result on. No confidence
//! interval: 26.17's metric list ("task success, evidence quality, resource use, calibration,
//! action value, reproducibility, human intervention") names seven dimensions and defines no
//! estimator for any of them, so the plane carries numbers a caller measured and never computes
//! one.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::PlaneError;

const MAX_PLANE_TEXT_BYTES: usize = 256;
const MAX_DIMENSIONS: usize = 4096;

/// The five system kinds 26.17 names under "Evaluation target", ordered by action breadth.
///
/// The order is the blueprint's own listing order and is load-bearing: a tier admits every
/// dimension that a narrower tier admits. Nothing here claims the ordering is a capability
/// ranking — a pipeline is not "better" than a predictive model, it can merely be asked more
/// kinds of question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityTier {
    /// A model that maps a fixed input to an output and takes no actions.
    FixedInputModel,
    /// A workflow pipeline: a fixed sequence of steps over declared inputs.
    WorkflowPipeline,
    /// An agent that may call tools and choose what to acquire next.
    ToolUsingAgent,
    /// A system whose loop includes a human decision point.
    HumanInTheLoop,
    /// A multi-agent molecule, in the blueprint's sense of a composed team.
    MultiAgentMolecule,
}

impl CapabilityTier {
    /// Every tier, in blueprint listing order.
    pub const ALL: [CapabilityTier; 5] = [
        CapabilityTier::FixedInputModel,
        CapabilityTier::WorkflowPipeline,
        CapabilityTier::ToolUsingAgent,
        CapabilityTier::HumanInTheLoop,
        CapabilityTier::MultiAgentMolecule,
    ];

    /// The blueprint's own phrase for this tier.
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityTier::FixedInputModel => "fixed-input predictive model",
            CapabilityTier::WorkflowPipeline => "workflow pipeline",
            CapabilityTier::ToolUsingAgent => "tool-using agent",
            CapabilityTier::HumanInTheLoop => "human-in-the-loop system",
            CapabilityTier::MultiAgentMolecule => "multi-agent molecule",
        }
    }

    /// Whether a system at this tier can be asked a question requiring `required`.
    ///
    /// Deliberately *not* symmetric: a tool-using agent can be scored on a dimension a fixed-input
    /// model is also scored on, because the narrower question is still a question it can answer.
    pub fn admits(self, required: CapabilityTier) -> bool {
        self >= required
    }
}

/// A number in the unit interval that arrived from a caller who measured it.
///
/// The private field is the point: there is no way to reach a `Score` except through
/// [`Score::new`], so an absent measurement cannot become a `Score` by defaulting.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Score(f64);

impl Score {
    /// Build a score, refusing anything outside `0.0..=1.0` or non-finite.
    pub fn new(dimension: &str, value: f64) -> Result<Self, PlaneError> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(Score(value))
        } else {
            Err(PlaneError::ScoreOutOfRange {
                dimension: dimension.to_string(),
                value,
            })
        }
    }

    /// The underlying value. Reaching it requires already holding a `Score`.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Score {
    type Error = PlaneError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Score::new("<deserialized>", value)
    }
}

impl From<Score> for f64 {
    fn from(score: Score) -> f64 {
        score.0
    }
}

/// Why a dimension has no score.
///
/// Every variant names a *cause outside the system under evaluation*. That is the distinction from
/// a low score: a low score is a fact about the system, and an [`UnscoredReason`] is a fact about
/// the evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum UnscoredReason {
    /// The panel of cells for this dimension was never run.
    NotAttempted,
    /// The evaluator for this dimension was itself unhealthy (07.02).
    EvaluatorUnhealthy { evaluator: String },
    /// The reference standard for this dimension is unavailable or disputed.
    NoReferenceStandard { note: String },
    /// The dimension was attempted and the result was withheld pending a reveal (26.16).
    Sealed { registration: String },
}

/// One dimension's state for one system.
///
/// Named `Cell` rather than `DimensionScore` because a cell is exactly what it is: the
/// intersection of a system and a dimension, which may legitimately hold no number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Cell {
    /// Measured.
    Scored { score: Score },
    /// Not measured, for a stated reason. Carries no score field.
    Unscored {
        #[serde(flatten)]
        reason: UnscoredReason,
    },
    /// Out of the system's action tier. Carries no score field, and names the tier that would
    /// have been needed, so a reader can see the shape of the comparison rather than a hole.
    Inapplicable {
        required: CapabilityTier,
        declared: CapabilityTier,
    },
}

impl Cell {
    /// The score, if there is one. There is deliberately no variant of this returning `f64`.
    pub fn score(&self) -> Option<&Score> {
        match self {
            Cell::Scored { score } => Some(score),
            _ => None,
        }
    }

    /// Whether this cell contributes to a fold's denominator.
    pub fn is_measured(&self) -> bool {
        matches!(self, Cell::Scored { .. })
    }

    /// Whether this cell blocks a fold. [`Cell::Inapplicable`] does not; [`Cell::Unscored`] does.
    pub fn blocks_fold(&self) -> bool {
        matches!(self, Cell::Unscored { .. })
    }
}

/// How a fold treats dimensions that are out of tier.
///
/// There is one variant. That is not an oversight — it is the module's argument. The two variants
/// a caller would ask for are `TreatAsZero`, which is the failure 26.17 exists to prevent, and
/// `ImputeFromPeers`, which turns a comparison into an extrapolation from systems that were never
/// the subject. Both are refused by not existing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoldPolicy {
    /// Renormalise the declared weights over the applicable dimensions and list the excluded ones
    /// in the result, so a reader can see that two systems were folded over different denominators.
    ExcludeInapplicable,
}

/// A dimension of the plane: its identifier, the tier it needs, and its weight in a fold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    pub id: String,
    /// The narrowest tier that can be asked this question.
    pub required: CapabilityTier,
    /// Relative weight in a fold. Normalised at fold time over the dimensions that survive.
    pub weight: f64,
}

impl Dimension {
    /// A dimension any system can be asked, with unit weight.
    pub fn universal(id: impl Into<String>) -> Self {
        Dimension {
            id: id.into(),
            required: CapabilityTier::FixedInputModel,
            weight: 1.0,
        }
    }

    /// A dimension that needs at least `required`.
    pub fn requiring(id: impl Into<String>, required: CapabilityTier) -> Self {
        Dimension {
            id: id.into(),
            required,
            weight: 1.0,
        }
    }

    /// Set the fold weight.
    pub fn weighing(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// One system's scores across a declared set of dimensions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScorePlane {
    pub system: String,
    pub tier: CapabilityTier,
    dimensions: Vec<Dimension>,
    cells: BTreeMap<String, Cell>,
}

#[derive(Deserialize)]
struct ScorePlaneWire {
    system: String,
    tier: CapabilityTier,
    dimensions: Vec<Dimension>,
    cells: BTreeMap<String, Cell>,
}

impl<'de> Deserialize<'de> for ScorePlane {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ScorePlaneWire::deserialize(deserializer)?;
        let mut plane = ScorePlane::declare(wire.system, wire.tier, wire.dimensions)
            .map_err(serde::de::Error::custom)?;
        if wire.cells.len() != plane.dimensions.len() {
            return Err(serde::de::Error::custom(
                "serialized cells must contain exactly one entry per declared dimension",
            ));
        }
        for dimension in &plane.dimensions {
            let cell = wire.cells.get(&dimension.id).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "serialized cells are missing dimension {}",
                    dimension.id
                ))
            })?;
            match cell {
                Cell::Scored { .. } | Cell::Unscored { .. }
                    if !plane.tier.admits(dimension.required) =>
                {
                    return Err(serde::de::Error::custom(format!(
                        "out-of-tier dimension {} cannot carry a scored or unscored cell",
                        dimension.id
                    )));
                }
                Cell::Inapplicable { required, declared }
                    if plane.tier.admits(dimension.required)
                        || *required != dimension.required
                        || *declared != plane.tier =>
                {
                    return Err(serde::de::Error::custom(format!(
                        "inapplicable cell for {} does not match its declared tier",
                        dimension.id
                    )));
                }
                _ => {}
            }
            plane.cells.insert(dimension.id.clone(), cell.clone());
        }
        plane.validate().map_err(serde::de::Error::custom)?;
        Ok(plane)
    }
}

impl ScorePlane {
    /// Start a plane for `system` at `tier`. Every declared dimension begins
    /// [`UnscoredReason::NotAttempted`], or [`Cell::Inapplicable`] if it is out of tier.
    ///
    /// Starting at `NotAttempted` rather than absent is deliberate: a dimension that was declared
    /// and never run is a hole in *this* evaluation, and a plane that simply omitted it would
    /// serialize as if the dimension had never been part of the design.
    pub fn declare(
        system: impl Into<String>,
        tier: CapabilityTier,
        dimensions: Vec<Dimension>,
    ) -> Result<Self, PlaneError> {
        let system = system.into();
        validate_plane_text(&system, "system")?;
        if dimensions.len() > MAX_DIMENSIONS {
            return Err(PlaneError::TooManyDimensions(MAX_DIMENSIONS));
        }
        let mut seen = BTreeSet::new();
        let mut cells = BTreeMap::new();
        for dimension in &dimensions {
            validate_dimension(dimension)?;
            if !seen.insert(dimension.id.clone()) {
                return Err(PlaneError::DuplicateDimension(dimension.id.clone()));
            }
            if !dimension.weight.is_finite() || dimension.weight <= 0.0 {
                return Err(PlaneError::BadWeight(dimension.id.clone()));
            }
            let cell = if tier.admits(dimension.required) {
                Cell::Unscored {
                    reason: UnscoredReason::NotAttempted,
                }
            } else {
                Cell::Inapplicable {
                    required: dimension.required,
                    declared: tier,
                }
            };
            cells.insert(dimension.id.clone(), cell);
        }
        Ok(ScorePlane {
            system,
            tier,
            dimensions,
            cells,
        })
    }

    /// Record a measurement, refusing if the dimension is out of the system's tier.
    ///
    /// The refusal is the interesting half. A harness that scores a fixed-input model on assay
    /// selection has a bug, and returning [`PlaneError::OutOfTier`] surfaces it at the moment the
    /// number is offered rather than at the moment somebody reads the leaderboard.
    pub fn score(&mut self, dimension: &str, value: f64) -> Result<(), PlaneError> {
        self.validate()?;
        let declared = self.declared(dimension)?;
        if !self.tier.admits(declared.required) {
            return Err(PlaneError::OutOfTier {
                dimension: dimension.to_string(),
                declared: self.tier.as_str().to_string(),
                required: declared.required.as_str().to_string(),
            });
        }
        let score = Score::new(dimension, value)?;
        self.cells
            .insert(dimension.to_string(), Cell::Scored { score });
        Ok(())
    }

    /// Record that a dimension was not measured, and why.
    pub fn leave_unscored(
        &mut self,
        dimension: &str,
        reason: UnscoredReason,
    ) -> Result<(), PlaneError> {
        self.validate()?;
        self.declared(dimension)?;
        validate_reason(&reason)?;
        self.cells
            .insert(dimension.to_string(), Cell::Unscored { reason });
        Ok(())
    }

    /// The cell for `dimension`.
    pub fn cell(&self, dimension: &str) -> Option<&Cell> {
        self.cells.get(dimension)
    }

    /// The declared dimensions, in declaration order.
    pub fn dimensions(&self) -> &[Dimension] {
        &self.dimensions
    }

    /// Dimensions with no score and no tier excuse, in identifier order.
    pub fn unscored(&self) -> Vec<&str> {
        self.cells
            .iter()
            .filter(|(_, cell)| cell.blocks_fold())
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Dimensions excluded because the system's tier cannot be asked them.
    pub fn inapplicable(&self) -> Vec<&str> {
        self.cells
            .iter()
            .filter(|(_, cell)| matches!(cell, Cell::Inapplicable { .. }))
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Fold to a single number, or refuse.
    ///
    /// Refuses while any dimension is unscored. This is the whole point of the type: the caller
    /// who wants a number must first decide, in the open, what to do about the dimensions nobody
    /// measured — by measuring them, or by removing them from the design, or by not folding.
    pub fn fold(&self, policy: FoldPolicy) -> Result<Fold, PlaneError> {
        self.validate()?;
        let unscored: Vec<String> = self.unscored().into_iter().map(str::to_string).collect();
        if !unscored.is_empty() {
            return Err(PlaneError::UnscoredDimensions { unscored });
        }
        let FoldPolicy::ExcludeInapplicable = policy;
        let mut total_weight = 0.0f64;
        let mut accumulated = 0.0f64;
        let mut included = Vec::new();
        let mut excluded = Vec::new();
        for dimension in &self.dimensions {
            match self.cells.get(&dimension.id) {
                Some(Cell::Scored { score }) => {
                    total_weight += dimension.weight;
                    accumulated += dimension.weight * score.value();
                    included.push(dimension.id.clone());
                }
                Some(Cell::Inapplicable { required, .. }) => excluded.push(ExcludedDimension {
                    id: dimension.id.clone(),
                    required: *required,
                }),
                _ => {
                    return Err(PlaneError::InvalidCell {
                        dimension: dimension.id.clone(),
                        detail: "a dimension remained unscored after the unscored-cell check"
                            .into(),
                    });
                }
            }
        }
        if included.is_empty() {
            return Err(PlaneError::Empty);
        }
        if !total_weight.is_finite() || total_weight <= 0.0 || !accumulated.is_finite() {
            return Err(PlaneError::FoldOverflow);
        }
        Ok(Fold {
            system: self.system.clone(),
            tier: self.tier,
            policy,
            value: accumulated / total_weight,
            included,
            excluded,
        })
    }

    fn declared(&self, dimension: &str) -> Result<&Dimension, PlaneError> {
        self.dimensions
            .iter()
            .find(|d| d.id == dimension)
            .ok_or_else(|| PlaneError::UnknownDimension(dimension.to_string()))
    }

    fn validate(&self) -> Result<(), PlaneError> {
        validate_plane_text(&self.system, "system")?;
        if self.dimensions.len() > MAX_DIMENSIONS {
            return Err(PlaneError::TooManyDimensions(MAX_DIMENSIONS));
        }
        let mut ids = BTreeSet::new();
        for dimension in &self.dimensions {
            validate_dimension(dimension)?;
            if !ids.insert(&dimension.id) {
                return Err(PlaneError::DuplicateDimension(dimension.id.clone()));
            }
            let cell = self
                .cells
                .get(&dimension.id)
                .ok_or_else(|| PlaneError::InvalidCell {
                    dimension: dimension.id.clone(),
                    detail: "no cell exists for the declared dimension".into(),
                })?;
            if let Cell::Unscored { reason } = cell {
                validate_reason(reason)?;
            }
            match cell {
                Cell::Scored { .. } | Cell::Unscored { .. }
                    if !self.tier.admits(dimension.required) =>
                {
                    return Err(PlaneError::InvalidCell {
                        dimension: dimension.id.clone(),
                        detail: "out-of-tier dimensions must be inapplicable".into(),
                    });
                }
                Cell::Inapplicable { required, declared }
                    if self.tier.admits(dimension.required)
                        || *required != dimension.required
                        || *declared != self.tier =>
                {
                    return Err(PlaneError::InvalidCell {
                        dimension: dimension.id.clone(),
                        detail: "inapplicable metadata does not match the dimension".into(),
                    });
                }
                _ => {}
            }
        }
        if self.cells.keys().any(|id| !ids.contains(id)) {
            return Err(PlaneError::InvalidCell {
                dimension: "<unknown>".into(),
                detail: "the cell map contains an undeclared dimension".into(),
            });
        }
        Ok(())
    }
}

fn validate_plane_text(value: &str, field: &str) -> Result<(), PlaneError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_PLANE_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(PlaneError::InvalidSystem(format!(
            "{field} must be bounded, trimmed, and control-free"
        )));
    }
    Ok(())
}

fn validate_dimension(dimension: &Dimension) -> Result<(), PlaneError> {
    if dimension.id.trim().is_empty()
        || dimension.id != dimension.id.trim()
        || dimension.id.len() > MAX_PLANE_TEXT_BYTES
        || dimension.id.chars().any(char::is_control)
    {
        return Err(PlaneError::InvalidDimension {
            dimension: dimension.id.clone(),
            detail: "identifier must be bounded, trimmed, and control-free".into(),
        });
    }
    if !dimension.weight.is_finite() || dimension.weight <= 0.0 {
        return Err(PlaneError::BadWeight(dimension.id.clone()));
    }
    Ok(())
}

fn validate_reason(reason: &UnscoredReason) -> Result<(), PlaneError> {
    let (field, value) = match reason {
        UnscoredReason::NotAttempted => return Ok(()),
        UnscoredReason::EvaluatorUnhealthy { evaluator } => ("evaluator", evaluator),
        UnscoredReason::NoReferenceStandard { note } => ("note", note),
        UnscoredReason::Sealed { registration } => ("registration", registration),
    };
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_PLANE_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(PlaneError::InvalidCell {
            dimension: "<unscored>".into(),
            detail: format!("{field} must be bounded, trimmed, and control-free"),
        });
    }
    Ok(())
}

/// A dimension left out of a fold because the system's tier could not be asked it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcludedDimension {
    pub id: String,
    pub required: CapabilityTier,
}

/// The result of a fold, which always carries what it left out.
///
/// There is no constructor. A `Fold` can only come from [`ScorePlane::fold`], so a number in this
/// shape has necessarily passed the unscored check, and the excluded list travels with it — two
/// systems folded over different denominators cannot be compared without that being visible.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Fold {
    pub system: String,
    pub tier: CapabilityTier,
    pub policy: FoldPolicy,
    pub value: f64,
    pub included: Vec<String>,
    pub excluded: Vec<ExcludedDimension>,
}

impl Fold {
    /// Whether two folds ran over the same dimensions.
    ///
    /// 26.17's whole protocol is "compare within matched resource envelopes"; comparing folds with
    /// different `included` sets is comparing two different measurements that share a scale.
    pub fn same_basis(&self, other: &Fold) -> bool {
        self.included == other.included
    }
}
