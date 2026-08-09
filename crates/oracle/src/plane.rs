//! What an oracle can and cannot establish.
//!
//! Blueprint 31.00 lists eight things an oracle may establish and warns that an oracle "rarely"
//! establishes all of them: a pipeline that reproduces a number passes computational validity
//! while saying nothing about biological validity. 40.21 turns that observation into
//! non-negotiable invariant 1 — each oracle states what it establishes *and* what it cannot.
//!
//! Planes are therefore not a taxonomy for reporting. They are the mechanism that stops a
//! satisfied checksum from being read as a confirmed biological claim: a combined verdict
//! establishes only the union of planes its supporting oracles claimed, and explicitly reports
//! the planes every contributing oracle disclaimed.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The eight evidential planes of 31.00.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plane {
    /// The bytes are well formed: structure, checksums, required fields.
    Artifact,
    /// The computation is correct: arithmetic, transformations, re-execution.
    Analytical,
    /// The measurement means what it claims: assay performance, calibration.
    Measurement,
    /// An association holds in some population.
    Biological,
    /// An intervention produces an effect.
    Causal,
    /// A later observation confirms an earlier forecast.
    Longitudinal,
    /// The finding transfers to the intended deployment population.
    Translational,
    /// A workflow, claim boundary, or governance rule was respected.
    Policy,
}

impl Plane {
    /// Every plane, in declaration order. Used by manifests that disclaim "everything else".
    pub const ALL: [Plane; 8] = [
        Plane::Artifact,
        Plane::Analytical,
        Plane::Measurement,
        Plane::Biological,
        Plane::Causal,
        Plane::Longitudinal,
        Plane::Translational,
        Plane::Policy,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Plane::Artifact => "artifact",
            Plane::Analytical => "analytical",
            Plane::Measurement => "measurement",
            Plane::Biological => "biological",
            Plane::Causal => "causal",
            Plane::Longitudinal => "longitudinal",
            Plane::Translational => "translational",
            Plane::Policy => "policy",
        }
    }
}

impl fmt::Display for Plane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether re-running the oracle on identical evidence must produce the identical judgement.
///
/// This is deliberately independent of [`crate::EvidenceTier`]. A property oracle sits below a
/// schema oracle on the ladder because a satisfied property establishes less, not because it is
/// less reproducible — both are [`Determinism::Reproducible`]. Keeping the two axes apart is what
/// lets the independence demotion of 31.01 weaken a circular oracle's *standing* without
/// pretending its arithmetic became stochastic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determinism {
    /// Same evidence, same judgement, on any machine, forever.
    Reproducible,
    /// Sampling, model weights, or human variation sit between evidence and judgement.
    Nondeterministic,
}

impl Determinism {
    pub fn as_str(self) -> &'static str {
        match self {
            Determinism::Reproducible => "reproducible",
            Determinism::Nondeterministic => "nondeterministic",
        }
    }
}

impl fmt::Display for Determinism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
