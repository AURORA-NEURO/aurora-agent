//! The evidence ladder.
//!
//! `Deterministic > Execution > Property > Statistical > Judge`.
//!
//! The ladder exists to make one rule mechanical rather than cultural. Blueprint 31.01 lists it
//! under "Failure containment" ("Deterministic contradictions cannot be overwritten by a semantic
//! judge"), 31.14 restates it as the limit of model judges ("a judge may score whether a plan
//! acknowledges confounding, but cannot override a deterministic finding that the cohort split
//! leaks patients"), and 40.21 raises it to non-negotiable invariant 2. In this workspace it is
//! the 11.11 invariant: **a nondeterministic judgement may never override a deterministic or
//! execution-grounded one.**
//!
//! # Why the variants are declared weakest-first
//!
//! [`EvidenceTier`] derives `Ord`, and derived `Ord` on a fieldless enum follows declaration
//! order. Declaring `Judge` first and `Deterministic` last makes `a > b` mean exactly "a is
//! stronger evidence than b", so every comparison in [`crate::combine`] reads as the rule it
//! enforces. The serialized names carry the meaning for humans; the declaration order carries it
//! for the compiler.
//!
//! # What the ladder is not
//!
//! It is not a confidence scale. A judge reporting 0.99 is still a judge — see
//! [`EvidenceTier::may_override`], which never consults confidence. It is also not a quality
//! ranking of individual oracles: a buggy deterministic oracle outranks a careful expert panel,
//! which is precisely why 31.15 routes same-tier deterministic contradictions to artifact repair
//! and independent review rather than to a tiebreak.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::plane::Determinism;

/// A rung on the evidence ladder, ordered from weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    /// A model or human scoring a rubric (31.14). Carries opinions, not checkable objects.
    Judge,
    /// An estimate over a sample: calibration, agreement rates, effect sizes.
    Statistical,
    /// A named scientific property recomputed over the artifact (31.03): ordering, bounds,
    /// monotonicity, conservation. Reproducible, but satisfaction only means no violation of the
    /// properties that were chosen.
    Property,
    /// The workflow was rerun and its outputs compared within a declared tolerance (31.03).
    Execution,
    /// A machine-checkable invariant of the artifact itself (31.02): schema, checksum, identifier,
    /// unit, temporal impossibility. A failure here is a proof of defect.
    Deterministic,
}

impl EvidenceTier {
    /// Every tier, weakest first.
    pub const ALL: [EvidenceTier; 5] = [
        EvidenceTier::Judge,
        EvidenceTier::Statistical,
        EvidenceTier::Property,
        EvidenceTier::Execution,
        EvidenceTier::Deterministic,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceTier::Judge => "judge",
            EvidenceTier::Statistical => "statistical",
            EvidenceTier::Property => "property",
            EvidenceTier::Execution => "execution",
            EvidenceTier::Deterministic => "deterministic",
        }
    }

    /// Whether a judgement at this tier is reproducible byte-for-byte.
    pub fn determinism(self) -> Determinism {
        match self {
            EvidenceTier::Judge | EvidenceTier::Statistical => Determinism::Nondeterministic,
            EvidenceTier::Property | EvidenceTier::Execution | EvidenceTier::Deterministic => {
                Determinism::Reproducible
            }
        }
    }

    /// The two tiers the 11.11 invariant protects: a finding here is grounded in the artifact or
    /// in an execution of it, so no amount of opinion may overturn it.
    pub fn is_grounded(self) -> bool {
        matches!(self, EvidenceTier::Deterministic | EvidenceTier::Execution)
    }

    pub fn is_nondeterministic(self) -> bool {
        self.determinism() == Determinism::Nondeterministic
    }

    /// The next stronger tier, or `None` at the top.
    pub fn stronger(self) -> Option<EvidenceTier> {
        match self {
            EvidenceTier::Judge => Some(EvidenceTier::Statistical),
            EvidenceTier::Statistical => Some(EvidenceTier::Property),
            EvidenceTier::Property => Some(EvidenceTier::Execution),
            EvidenceTier::Execution => Some(EvidenceTier::Deterministic),
            EvidenceTier::Deterministic => None,
        }
    }

    /// The next weaker tier, or `None` at the bottom.
    pub fn weaker(self) -> Option<EvidenceTier> {
        match self {
            EvidenceTier::Judge => None,
            EvidenceTier::Statistical => Some(EvidenceTier::Judge),
            EvidenceTier::Property => Some(EvidenceTier::Statistical),
            EvidenceTier::Execution => Some(EvidenceTier::Property),
            EvidenceTier::Deterministic => Some(EvidenceTier::Execution),
        }
    }

    /// The lowest tier a demotion may reach without changing determinism class.
    ///
    /// 31.01 demotes an oracle that shares data, code, labels, or assumptions with the system it
    /// evaluates: "apparent confirmation from a circular or non-independent oracle receives a
    /// lower evidence tier". Circularity weakens what a result *shows*; it does not make a
    /// reproducible computation stochastic. So a reproducible oracle floors at
    /// [`EvidenceTier::Property`] and never falls into the nondeterministic half of the ladder.
    pub fn demotion_floor(self) -> EvidenceTier {
        match self.determinism() {
            Determinism::Reproducible => EvidenceTier::Property,
            Determinism::Nondeterministic => EvidenceTier::Judge,
        }
    }

    /// One rung down, stopping at [`EvidenceTier::demotion_floor`].
    pub fn demoted(self) -> EvidenceTier {
        let floor = self.demotion_floor();
        match self.weaker() {
            Some(lower) if lower >= floor => lower,
            _ => floor.min(self),
        }
    }

    /// Whether a judgement at `self` is permitted to displace a standing judgement at `held`.
    ///
    /// Two rules, in order:
    ///
    /// 1. the 11.11 invariant — a nondeterministic tier may never displace a grounded one, at any
    ///    confidence, for any reason;
    /// 2. general precedence — a strictly weaker tier never displaces a stronger one.
    ///
    /// Equal tiers return `true`: same-tier conflict is not an override, it is a disagreement,
    /// and [`crate::combine`] answers it with [`crate::Position`] sets rather than a winner.
    pub fn may_override(self, held: EvidenceTier) -> bool {
        if self.is_nondeterministic() && held.is_grounded() {
            return false;
        }
        self >= held
    }
}

impl fmt::Display for EvidenceTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
