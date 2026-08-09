//! What an oracle returns: a position, its confidence, and the objects that support it.
//!
//! Blueprint 31.01 is titled "Biological Truth as a Distribution, Partial Order, and Unresolved
//! State", and its contract says an oracle "may return `supported`, `contradicted`, `unresolved`,
//! `not-evaluable`, or a structured distribution". Three consequences shape this module.
//!
//! **Abstention is a first-class answer.** [`Position::Unresolved`] and
//! [`Position::NotEvaluable`] are not soft failures. 31.01: "`Unresolved` is not scored as a
//! hidden negative unless the task explicitly requires an action under uncertainty." Combination
//! honours that literally — see [`crate::combine`], where an abstention at the deciding tier
//! cannot manufacture a disagreement with a colleague that found a real contradiction.
//!
//! **A judgement carries its own confidence, and confidence never buys rank.** [`Judgement`]
//! records both [`Confidence`] and [`crate::EvidenceTier`], and nothing in this crate lets the
//! former substitute for the latter. That separation is the whole content of the 11.11 invariant:
//! a judge at 0.99 and a judge at 0.51 are equally unable to overturn a checksum.
//!
//! **Findings are objects, not scores.** `crates/section/src/verdict.rs` puts it exactly right for
//! the split-integrity case: a witness is "a concrete, checkable object … not a score. That is
//! what makes the first vertical slice objective." [`Finding`] generalises that, and
//! [`Finding::is_checkable`] marks the one variant — [`Finding::Remark`] — that a human cannot
//! verify by hand. Judges may only emit remarks.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bioprism_section::{LeakageWitness, OracleStatus, OracleVerdict};
use serde::{Deserialize, Serialize};

use crate::error::OracleError;
use crate::ladder::EvidenceTier;
use crate::manifest::{Admissibility, OracleManifest, OracleRef};
use crate::plane::Plane;
use crate::time::UtcTimestamp;

/// Tolerance used when checking that a distribution's mass sums to one.
const MASS_TOLERANCE: f64 = 1e-9;

/// The four answers an oracle may give.
///
/// `Ord` is derived for deterministic ordering inside `BTreeSet` and for stable serialisation. It
/// is presentation order only and carries no epistemic ranking: `Supported < Contradicted` here
/// means nothing more than that `Supported` prints first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    /// The evidence is consistent with the claim under this oracle's checks.
    Supported,
    /// The evidence contradicts the claim, and the finding says where.
    Contradicted,
    /// The oracle applies and cannot decide — genuine ambiguity, not absence of input.
    Unresolved,
    /// The oracle does not apply to this evidence at all: required inputs absent, scope missed.
    NotEvaluable,
}

impl Position {
    /// Whether this position declines to take a side.
    ///
    /// The distinction matters in combination: abstentions are counted as participation but not
    /// as positions, so a single oracle saying "I cannot tell" never drags a decided verdict into
    /// [`OracleStatus::Underdetermined`].
    pub fn is_abstention(self) -> bool {
        matches!(self, Position::Unresolved | Position::NotEvaluable)
    }

    /// Projects onto the three-valued status of `bioprism_section`.
    ///
    /// Lossy on purpose. `bioprism_section::OracleStatus` has three values and this enum has
    /// four, so `Unresolved` and `NotEvaluable` both land on
    /// [`OracleStatus::Underdetermined`] — "the oracle applies but cannot decide" and "the oracle
    /// does not apply" become indistinguishable downstream. The full position stays on the
    /// [`Judgement`], which is why nothing in this crate consumes the projection internally.
    pub fn to_status(self) -> OracleStatus {
        match self {
            Position::Supported => OracleStatus::Valid,
            Position::Contradicted => OracleStatus::Invalid,
            Position::Unresolved | Position::NotEvaluable => OracleStatus::Underdetermined,
        }
    }

    /// Lifts a `bioprism_section` status, as emitted by the hardcoded `fiber` split-integrity
    /// oracle. `Underdetermined` becomes `Unresolved` rather than `NotEvaluable`, because the
    /// fiber oracle abstains when it applies and cannot decide.
    pub fn from_status(status: OracleStatus) -> Self {
        match status {
            OracleStatus::Valid => Position::Supported,
            OracleStatus::Invalid => Position::Contradicted,
            OracleStatus::Underdetermined => Position::Unresolved,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Position::Supported => "supported",
            Position::Contradicted => "contradicted",
            Position::Unresolved => "unresolved",
            Position::NotEvaluable => "not_evaluable",
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A probability in the closed unit interval.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Confidence(f64);

impl Confidence {
    /// What a deterministic oracle reports. A schema violation is not 0.97 likely to be a schema
    /// violation.
    pub const CERTAIN: Confidence = Confidence(1.0);

    pub fn new(value: f64) -> Result<Self, OracleError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(OracleError::ConfidenceOutOfRange { value });
        }
        Ok(Confidence(value))
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

impl From<Confidence> for f64 {
    fn from(value: Confidence) -> Self {
        value.0
    }
}

impl TryFrom<f64> for Confidence {
    type Error = OracleError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Confidence::new(value)
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The observed range of confidence among the judgements that decided a verdict.
///
/// Deliberately not a mean. Averaging two oracles at 0.4 and 1.0 into 0.7 invents a number no
/// oracle reported and destroys the information that one of them was certain. 31.01 asks for
/// calibration against reference distributions, which a fabricated central tendency defeats.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceEnvelope {
    pub low: Confidence,
    pub high: Confidence,
}

impl ConfidenceEnvelope {
    /// `None` when the iterator is empty, because an envelope over nothing is not zero.
    pub fn over(values: impl IntoIterator<Item = Confidence>) -> Option<Self> {
        let mut iter = values.into_iter();
        let first = iter.next()?;
        let (low, high) = iter.fold((first, first), |(low, high), next| {
            (
                if next.value() < low.value() {
                    next
                } else {
                    low
                },
                if next.value() > high.value() {
                    next
                } else {
                    high
                },
            )
        });
        Some(ConfidenceEnvelope { low, high })
    }

    pub fn is_point(&self) -> bool {
        self.low.value() == self.high.value()
    }
}

/// A distribution over positions: 31.01's "structured distribution" return.
///
/// The worked case in 31.01 is a progression call spread 0.55 / 0.35 / 0.10 across progression,
/// treatment effect, and mixed process, and its point is that such a spread "can be more correct
/// than an unqualified categorical label".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionDistribution(BTreeMap<Position, f64>);

impl PositionDistribution {
    pub fn new(mass: impl IntoIterator<Item = (Position, f64)>) -> Result<Self, OracleError> {
        let mass: BTreeMap<Position, f64> = mass.into_iter().collect();
        if mass.is_empty() {
            return Err(OracleError::MalformedDistribution {
                reason: "a distribution over no positions carries no information".to_string(),
            });
        }
        if let Some((position, weight)) = mass.iter().find(|(_, w)| !w.is_finite() || **w < 0.0) {
            return Err(OracleError::MalformedDistribution {
                reason: format!(
                    "position {position} carries mass {weight}, which is not a probability"
                ),
            });
        }
        let total: f64 = mass.values().sum();
        if (total - 1.0).abs() > MASS_TOLERANCE {
            return Err(OracleError::MalformedDistribution {
                reason: format!("total mass is {total}, not 1"),
            });
        }
        Ok(PositionDistribution(mass))
    }

    pub fn mass(&self, position: Position) -> f64 {
        self.0.get(&position).copied().unwrap_or(0.0)
    }

    pub fn as_map(&self) -> &BTreeMap<Position, f64> {
        &self.0
    }

    /// Every position holding the maximum mass.
    ///
    /// Returns a set rather than a winner, and ties are returned as ties. 31.01's required
    /// functions include "avoid consensus collapse when minority interpretations are supported";
    /// picking the first of two equal modes is exactly that collapse, performed silently.
    pub fn modes(&self) -> BTreeSet<Position> {
        let peak = self.0.values().copied().fold(f64::NEG_INFINITY, f64::max);
        self.0
            .iter()
            .filter(|(_, mass)| (**mass - peak).abs() <= MASS_TOLERANCE)
            .map(|(position, _)| *position)
            .collect()
    }
}

/// A concrete, checkable object supporting a judgement — or, for judges, an admittedly
/// uncheckable remark.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum Finding {
    /// A split-integrity witness lifted from a `bioprism_section::OracleVerdict`. This is the
    /// bridge to the hardcoded oracle in `crates/fiber`, which this crate generalises without
    /// modifying.
    Leakage { witness: LeakageWitness },
    /// A required field is absent.
    MissingField { pointer: String },
    /// A field is present with the wrong JSON type.
    TypeMismatch {
        pointer: String,
        expected: String,
        actual: String,
    },
    /// A declared content hash does not match the payload it addresses.
    ChecksumMismatch {
        pointer: String,
        declared: String,
        computed: String,
    },
    /// A document failed to parse against its schema.
    Malformed { pointer: String, detail: String },
    /// A named scientific property does not hold.
    PropertyViolated {
        property: String,
        pointer: String,
        detail: String,
    },
    /// A reported number and its recomputation differ by more than the declared tolerance.
    NumericDivergence {
        reported: String,
        recomputed: String,
        reported_value: f64,
        recomputed_value: f64,
        tolerance: f64,
    },
    /// A check was configured but its inputs were absent, so it neither passed nor failed. Kept
    /// because silent skipping is how property suites drift into checking nothing.
    NotApplicable { check: String, reason: String },
    /// Prose from a semantic reviewer. Carries no object a reader can independently verify, which
    /// is why 31.14 confines judges to "dimensions that cannot be reduced to stronger oracles".
    Remark { rubric: String, text: String },
}

impl Finding {
    /// Whether a human could verify this finding by hand from the artifact alone.
    ///
    /// False only for [`Finding::Remark`]. The asymmetry is the point: everything a deterministic
    /// or property oracle emits is falsifiable by inspection, and everything a judge emits is not.
    pub fn is_checkable(&self) -> bool {
        !matches!(self, Finding::Remark { .. })
    }

    /// Whether this finding asserts a defect, as opposed to recording a skipped check.
    pub fn is_violation(&self) -> bool {
        !matches!(self, Finding::NotApplicable { .. } | Finding::Remark { .. })
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Finding::Leakage { .. } => "leakage",
            Finding::MissingField { .. } => "missing_field",
            Finding::TypeMismatch { .. } => "type_mismatch",
            Finding::ChecksumMismatch { .. } => "checksum_mismatch",
            Finding::Malformed { .. } => "malformed",
            Finding::PropertyViolated { .. } => "property_violated",
            Finding::NumericDivergence { .. } => "numeric_divergence",
            Finding::NotApplicable { .. } => "not_applicable",
            Finding::Remark { .. } => "remark",
        }
    }
}

/// One oracle's answer about one piece of evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Judgement {
    pub oracle: OracleRef,
    /// The tier this judgement actually carries: the manifest's declared tier after any
    /// independence demotion. Combination reads this and only this.
    pub tier: EvidenceTier,
    /// The tier the manifest claimed, retained so a demotion is visible rather than inferred.
    pub declared_tier: EvidenceTier,
    pub position: Position,
    pub confidence: Confidence,
    /// An optional distribution over positions, for oracles whose uncertainty model is
    /// [`crate::UncertaintyModel::Distribution`].
    pub belief: Option<PositionDistribution>,
    pub establishes: BTreeSet<Plane>,
    pub cannot_establish: BTreeSet<Plane>,
    pub findings: Vec<Finding>,
    /// Whether this judgement may be counted at the evidence's instant, and if not, why (31.16).
    pub admissibility: Admissibility,
    pub rationale: String,
}

impl Judgement {
    /// Builds a judgement from the manifest of the oracle emitting it.
    ///
    /// Tier, planes, and admissibility all come from the manifest rather than from the call site,
    /// so an oracle cannot claim a stronger rung or a wider scope for one convenient result than
    /// it declared for itself.
    pub fn from_manifest(
        manifest: &OracleManifest,
        at: &UtcTimestamp,
        position: Position,
        confidence: Confidence,
    ) -> Self {
        Judgement {
            oracle: manifest.oracle.clone(),
            tier: manifest.effective_tier(),
            declared_tier: manifest.declared_tier,
            position,
            confidence,
            belief: None,
            establishes: manifest.establishes.clone(),
            cannot_establish: manifest.cannot_establish.clone(),
            findings: Vec::new(),
            admissibility: manifest.admissibility(at),
            rationale: String::new(),
        }
    }

    pub fn with_findings(mut self, findings: impl IntoIterator<Item = Finding>) -> Self {
        self.findings.extend(findings);
        self
    }

    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = rationale.into();
        self
    }

    pub fn with_belief(mut self, belief: PositionDistribution) -> Self {
        self.belief = Some(belief);
        self
    }

    pub fn is_admissible(&self) -> bool {
        self.admissibility.is_admissible()
    }

    /// Whether the demotion of 31.01 was applied to this judgement.
    pub fn was_demoted(&self) -> bool {
        self.tier != self.declared_tier
    }

    /// Whether every finding is one a reader could check by hand.
    pub fn is_fully_checkable(&self) -> bool {
        self.findings.iter().all(Finding::is_checkable)
    }

    /// Lifts a `bioprism_section::OracleVerdict` — the shape `crates/fiber` emits — onto the
    /// ladder.
    ///
    /// This is the generalisation path. The fiber split-integrity oracle is hardcoded, untiered,
    /// unversioned, and returns leakage witnesses with a three-valued status; supplying a manifest
    /// gives that same output a tier, a validity window, and a declared scope, without changing
    /// one line of the oracle that produced it. Confidence is [`Confidence::CERTAIN`] because a
    /// leakage witness either holds or does not.
    pub fn lift_verdict(
        manifest: &OracleManifest,
        at: &UtcTimestamp,
        verdict: &OracleVerdict,
    ) -> Self {
        Judgement::from_manifest(
            manifest,
            at,
            Position::from_status(verdict.status),
            Confidence::CERTAIN,
        )
        .with_findings(
            verdict
                .witnesses
                .iter()
                .cloned()
                .map(|witness| Finding::Leakage { witness }),
        )
        .with_rationale(format!(
            "lifted from bioprism_section::OracleVerdict emitted by {}",
            verdict.oracle_kind
        ))
    }

    /// Projects back onto `bioprism_section::OracleVerdict` for consumers that read the older
    /// shape.
    ///
    /// Lossy in three ways, all of them silent downstream and therefore stated here: the tier is
    /// dropped, so a judge's opinion becomes indistinguishable from a checksum; the confidence is
    /// dropped; and every finding that is not a [`Finding::Leakage`] is dropped, because
    /// `LeakageWitness` has no variant for a schema violation and inventing one would put a
    /// falsehood into the certificate. Callers needing the whole judgement should serialise
    /// [`Judgement`] itself.
    pub fn to_verdict(&self) -> OracleVerdict {
        let witnesses: Vec<LeakageWitness> = self
            .findings
            .iter()
            .filter_map(|finding| match finding {
                Finding::Leakage { witness } => Some(witness.clone()),
                _ => None,
            })
            .collect();
        OracleVerdict {
            status: self.position.to_status(),
            witnesses,
            oracle_kind: self.oracle.kind().to_string(),
        }
    }
}
