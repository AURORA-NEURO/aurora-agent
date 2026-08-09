//! BioOracle Mesh IR — blueprint 25.18.
//!
//! The wire form of an evaluator: what it is, what version, what it takes, what it establishes, how
//! confident it is, when it fails, where it sits in the priority order, how it is calibrated and
//! what it is independent of.
//!
//! # This is a projection of `bioprism-oracle`
//!
//! That crate implements the semantics: the [`EvidenceTier`] ladder, the rule that a nondeterministic
//! judgement may never override a deterministic one, set-valued combination that yields
//! `Underdetermined` rather than a majority, retained disagreement, and expiry that speaks. None of
//! that is reimplemented here. What this module adds is the serialisable manifest a world publishes
//! and a bundle cites, plus the three 25.18 invariants restated as checks over that manifest.
//!
//! The tier ordering below is a *mirror* of `bioprism-oracle`'s, and the projection test asserts the
//! two agree. A mirror rather than a re-export because the IR must be readable by a consumer that
//! has the JSON and not the crate — that is what an IR is for — but a mirror that drifted would be
//! worse than no mirror, so it is checked.
//!
//! # What is deliberately not implemented
//!
//! No evaluation. No combination. No mesh. `OracleMesh` in `bioprism-oracle` does the combining, in
//! the caller's process, and even there the crate is explicit that it provides no isolation and no
//! authorization. An IR provides less than that, not more.

use crate::error::OracleIrError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The evidence ladder, mirroring `bioprism_oracle::EvidenceTier`.
///
/// Ordered strongest first in [`EvidenceTier::ALL`]; `Ord` runs the other way, so the derived
/// comparison puts `Deterministic` lowest. [`EvidenceTier::may_override`] is written against the
/// explicit rank rather than the derive, so the rule does not depend on variant order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    Deterministic,
    Execution,
    Property,
    Statistical,
    Judge,
}

impl EvidenceTier {
    pub const ALL: [EvidenceTier; 5] = [
        EvidenceTier::Deterministic,
        EvidenceTier::Execution,
        EvidenceTier::Property,
        EvidenceTier::Statistical,
        EvidenceTier::Judge,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceTier::Deterministic => "deterministic",
            EvidenceTier::Execution => "execution",
            EvidenceTier::Property => "property",
            EvidenceTier::Statistical => "statistical",
            EvidenceTier::Judge => "judge",
        }
    }

    /// Rank, strongest first.
    pub fn rank(self) -> u8 {
        match self {
            EvidenceTier::Deterministic => 0,
            EvidenceTier::Execution => 1,
            EvidenceTier::Property => 2,
            EvidenceTier::Statistical => 3,
            EvidenceTier::Judge => 4,
        }
    }

    /// The two tiers grounded in the artifact or in an execution of it.
    pub fn is_grounded(self) -> bool {
        matches!(self, EvidenceTier::Deterministic | EvidenceTier::Execution)
    }

    /// True for the tiers that are not reproducible byte for byte.
    pub fn is_nondeterministic(self) -> bool {
        matches!(self, EvidenceTier::Judge | EvidenceTier::Statistical)
    }

    /// 25.18: "Model judges cannot silently override stronger oracles."
    ///
    /// Mirrors `bioprism_oracle::EvidenceTier::may_override` exactly, including the part that is
    /// easy to get backwards: **equal tiers return `true`**. A same-tier conflict is not an
    /// override, it is a disagreement, and `bioprism-oracle` answers it with a set of positions
    /// rather than a winner. Refusing it here would make the IR stricter than the semantics it
    /// projects, which is a different kind of wrong from being laxer but is still wrong.
    ///
    /// Confidence is never consulted. A judge at 0.99 contradicting a checksum is still a judge
    /// contradicting a checksum.
    pub fn may_override(self, held: EvidenceTier) -> bool {
        if self.is_nondeterministic() && held.is_grounded() {
            return false;
        }
        self.rank() <= held.rank()
    }
}

impl fmt::Display for EvidenceTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What an oracle is able to say something about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePlane {
    Artifact,
    Analytical,
    Policy,
    Measurement,
    Biological,
    Causal,
    Longitudinal,
    Translational,
}

/// What an oracle concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    Pass,
    Fail { reason: String },
    /// 25.18: "An oracle can abstain." A first-class outcome, not a missing one.
    Abstain { reason: String },
    /// Two positions, neither dominant.
    Underdetermined { positions: Vec<String> },
}

impl Verdict {
    pub fn is_abstention(&self) -> bool {
        matches!(self, Verdict::Abstain { .. })
    }
}

/// A confidence, with the fact that confidence buys no rank stated in the type's documentation
/// rather than in its value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Confidence {
    pub value: f64,
}

impl Confidence {
    pub fn new(value: f64) -> Self {
        Confidence { value }
    }
}

/// What the oracle shares with the system it evaluates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Independence {
    pub from_evaluated_system: bool,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub shared_resources: BTreeSet<String>,
}

impl Independence {
    /// True when the oracle is not independent of what it judges in a way that voids its verdict.
    pub fn is_circular(&self) -> bool {
        !self.from_evaluated_system
    }
}

/// An oracle's published manifest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OracleIr {
    pub oracle_id: String,
    pub kind: String,
    pub version: String,
    pub tier: EvidenceTier,
    pub inputs: BTreeSet<String>,
    pub outputs: BTreeSet<String>,
    /// What the oracle can establish.
    pub establishes: BTreeSet<EvidencePlane>,
    /// What it explicitly cannot. 25.18 and `bioprism-oracle` both treat this as load-bearing.
    pub cannot_establish: BTreeSet<EvidencePlane>,
    /// What its verdicts rest on, in prose.
    pub evidence_basis: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failure_conditions: Vec<String>,
    /// Priority within a mesh, lower first. Distinct from tier: two deterministic oracles can be
    /// ordered without either being stronger evidence.
    pub priority: u32,
    /// How calibration was established, in prose, or the admission that it was not.
    pub calibration: String,
    pub independence: Independence,
}

impl OracleIr {
    pub fn validate(&self) -> Result<(), OracleIrError> {
        if let Some(plane) = self
            .establishes
            .intersection(&self.cannot_establish)
            .next()
        {
            return Err(OracleIrError::PlaneClaimedAndDisclaimed {
                oracle: self.oracle_id.clone(),
                plane: format!("{plane:?}").to_lowercase(),
            });
        }
        if self.independence.is_circular() {
            return Err(OracleIrError::CircularIndependence {
                oracle: self.oracle_id.clone(),
            });
        }
        Ok(())
    }

    /// Whether this oracle's judgement may override another's.
    pub fn may_override(&self, other: &OracleIr) -> Result<(), OracleIrError> {
        if self.tier.may_override(other.tier) {
            Ok(())
        } else {
            Err(OracleIrError::WeakerTierOverrides {
                judge: self.oracle_id.clone(),
                judge_tier: self.tier.to_string(),
                stronger: other.oracle_id.clone(),
                stronger_tier: other.tier.to_string(),
            })
        }
    }
}

/// Two oracles that disagreed, and what would settle it. 25.18: "Oracle disagreement is retained."
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisagreementIr {
    pub left_oracle: String,
    pub left_verdict: Verdict,
    pub right_oracle: String,
    pub right_verdict: Verdict,
    /// What evidence would resolve it. Empty means nobody knows, which is worth recording.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub would_settle: Option<String>,
    /// The position that lost, retained. `None` here is the failure 25.18 forbids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retained_losing_position: Option<String>,
}

impl DisagreementIr {
    /// Checks that a resolved disagreement kept the position that lost.
    pub fn validate_resolution(&self) -> Result<(), OracleIrError> {
        if self.retained_losing_position.is_none() {
            return Err(OracleIrError::DisagreementDiscarded {
                left: self.left_oracle.clone(),
                right: self.right_oracle.clone(),
            });
        }
        Ok(())
    }
}
