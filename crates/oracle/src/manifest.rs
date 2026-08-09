//! Oracle identity, declaration, and admissibility (31.01, 31.16, 40.21).
//!
//! 31.01 gives every oracle a YAML declaration: an immutable id carrying a version, the planes it
//! addresses, a validity interval, an independence statement, an uncertainty model, and its known
//! failure modes. This module is that declaration as a Rust type, with the parts that can be
//! checked mechanically checked at construction rather than reviewed by eye.
//!
//! Two of those parts do real work at runtime rather than sitting in a manifest for auditors:
//!
//! * [`OracleManifest::admissibility`] turns the validity window and supersession pointer into a
//!   per-evaluation answer, so an expired oracle produces a judgement that *says* it is expired
//!   instead of a judgement that is quietly counted (31.16);
//! * [`OracleManifest::effective_tier`] applies the independence demotion of 31.01, so an oracle
//!   that shares training data with the system it evaluates cannot launder that circularity into
//!   top-of-ladder standing.
//!
//! Not implemented: `input_contract` / `output_contract` schema references, `access_policy`, and
//! `reference_population`. Those are registry concerns — they describe how an oracle is *fetched
//! and authorised*, not how its judgement combines — and the registry crate owns them.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::OracleError;
use crate::ladder::EvidenceTier;
use crate::plane::Plane;
use crate::time::{UtcTimestamp, ValidityWindow};

/// A `namespace:name` oracle identity, without the version.
///
/// The namespace is mandatory because 31.15 classifies disagreement partly by provenance, and
/// two independently authored oracles named `schema` are a different situation from one oracle
/// disagreeing with itself across versions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OracleId(String);

impl OracleId {
    pub fn parse(value: impl Into<String>) -> Result<Self, OracleError> {
        let value = value.into();
        let mut parts = value.split(':');
        let namespace = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default();

        if parts.next().is_some() {
            return Err(OracleError::MalformedOracleId {
                value,
                reason: "expected exactly one ':' separating namespace from name",
            });
        }
        if namespace.is_empty() || name.is_empty() {
            return Err(OracleError::MalformedOracleId {
                value,
                reason: "both the namespace and the name must be non-empty",
            });
        }
        if value.chars().any(char::is_control) {
            return Err(OracleError::MalformedOracleId {
                value,
                reason: "identifiers may not contain control characters",
            });
        }
        Ok(OracleId(value))
    }

    /// The full `namespace:name`, which is what [`crate::Oracle::kind`] reports.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn namespace(&self) -> &str {
        self.0.split(':').next().unwrap_or_default()
    }

    pub fn name(&self) -> &str {
        self.0.split(':').nth(1).unwrap_or_default()
    }
}

impl fmt::Display for OracleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<OracleId> for String {
    fn from(value: OracleId) -> Self {
        value.0
    }
}

impl TryFrom<String> for OracleId {
    type Error = OracleError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        OracleId::parse(value)
    }
}

/// A semantic version. 31.16: "Changes create a new immutable version".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OracleVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl OracleVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        OracleVersion {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for OracleVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A versioned reference to one oracle, rendered as `biooracle:<namespace>:<name>:<version>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OracleRef {
    pub id: OracleId,
    pub version: OracleVersion,
}

impl OracleRef {
    pub fn new(id: OracleId, version: OracleVersion) -> Self {
        OracleRef { id, version }
    }

    pub fn kind(&self) -> &str {
        self.id.as_str()
    }
}

impl fmt::Display for OracleRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "biooracle:{}:{}", self.id, self.version)
    }
}

/// A resource an oracle shares with the system it evaluates.
///
/// 31.01: "The hub records whether the oracle shares training data, preprocessing code, labels,
/// models, annotators, sites, or assumptions with the evaluated system."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedResource {
    TrainingData,
    PreprocessingCode,
    Labels,
    Model,
    Annotators,
    Sites,
    Assumptions,
}

/// The independence declaration of 31.01.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Independence {
    pub from_evaluated_system: bool,
    pub shared: BTreeSet<SharedResource>,
}

impl Independence {
    /// The clean case: nothing shared with the evaluated system.
    pub fn independent() -> Self {
        Independence {
            from_evaluated_system: true,
            shared: BTreeSet::new(),
        }
    }

    pub fn sharing(shared: impl IntoIterator<Item = SharedResource>) -> Self {
        Independence {
            from_evaluated_system: false,
            shared: shared.into_iter().collect(),
        }
    }

    /// Whether this oracle's agreement with the evaluated system could be an artefact of shared
    /// inputs rather than corroboration.
    pub fn is_circular(&self) -> bool {
        !self.from_evaluated_system || !self.shared.is_empty()
    }
}

impl Default for Independence {
    fn default() -> Self {
        Independence::independent()
    }
}

/// How an oracle represents what it does not know (31.01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyModel {
    /// The judgement is exact: the invariant holds or it does not.
    Exact,
    /// The judgement names a set of acceptable answers without ranking them.
    AcceptableSet,
    /// The judgement carries a distribution over positions.
    Distribution,
}

/// Why an oracle's judgement is or is not usable at a given instant (31.16).
///
/// An inadmissible judgement is *kept*. It appears in
/// [`crate::CombinedVerdict::inadmissible`] carrying this reason, because "we ran an oracle and
/// discarded it" and "we never ran an oracle" are different epistemic states and the audit trail
/// must be able to tell them apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Admissibility {
    Admissible,
    /// Evaluation time precedes the window: the reference standard did not exist yet.
    NotYetValid {
        at: UtcTimestamp,
        valid_from: UtcTimestamp,
    },
    /// Evaluation time is past the window. 31.16 calls this a stale result, and it is the reason
    /// "rerun affected result bundles" is a required function rather than an optimisation.
    Expired {
        at: UtcTimestamp,
        valid_until: UtcTimestamp,
    },
    /// A newer version exists. The old judgement stays auditable (31.15 worked case) but a fresh
    /// evaluation that reaches for the superseded oracle has called the wrong one.
    Superseded {
        by: OracleRef,
    },
}

impl Admissibility {
    pub fn is_admissible(&self) -> bool {
        matches!(self, Admissibility::Admissible)
    }

    /// A short human-readable reason, used in error messages and disagreement records.
    pub fn reason(&self) -> String {
        match self {
            Admissibility::Admissible => "admissible".to_string(),
            Admissibility::NotYetValid { at, valid_from } => {
                format!("evaluated at {at}, not valid before {valid_from}")
            }
            Admissibility::Expired { at, valid_until } => {
                format!("evaluated at {at}, expired at {valid_until}")
            }
            Admissibility::Superseded { by } => format!("superseded by {by}"),
        }
    }
}

/// The full declaration an oracle must make before it may contribute evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleManifest {
    pub oracle: OracleRef,
    /// The tier claimed by the author. [`OracleManifest::effective_tier`] is what combination uses.
    pub declared_tier: EvidenceTier,
    pub establishes: BTreeSet<Plane>,
    pub cannot_establish: BTreeSet<Plane>,
    pub validity: ValidityWindow,
    pub superseded_by: Option<OracleRef>,
    pub independence: Independence,
    pub uncertainty_model: UncertaintyModel,
    /// 31.01 release gate 1: scope and failure modes are documented. Free prose, because a failure
    /// mode that fits an enum has usually already been turned into a check.
    pub known_failure_modes: Vec<String>,
}

impl OracleManifest {
    /// Builds a manifest, rejecting the two declarations that are self-contradictory.
    ///
    /// An oracle establishing nothing has no reason to run, and an oracle both establishing and
    /// disclaiming a plane has told the mesh nothing about that plane while appearing to have
    /// spoken twice. Both are 40.21 invariant 1 violations and both are caught here rather than
    /// producing a verdict whose `establishes` set is meaningless.
    pub fn new(
        oracle: OracleRef,
        declared_tier: EvidenceTier,
        establishes: impl IntoIterator<Item = Plane>,
        cannot_establish: impl IntoIterator<Item = Plane>,
        validity: ValidityWindow,
    ) -> Result<Self, OracleError> {
        let establishes: BTreeSet<Plane> = establishes.into_iter().collect();
        let cannot_establish: BTreeSet<Plane> = cannot_establish.into_iter().collect();

        if establishes.is_empty() {
            return Err(OracleError::NoEstablishedPlane {
                kind: oracle.kind().to_string(),
            });
        }
        if let Some(plane) = establishes.intersection(&cannot_establish).next() {
            return Err(OracleError::ContradictoryPlaneDeclaration {
                kind: oracle.kind().to_string(),
                plane: *plane,
            });
        }

        Ok(OracleManifest {
            oracle,
            declared_tier,
            establishes,
            cannot_establish,
            validity,
            superseded_by: None,
            independence: Independence::independent(),
            uncertainty_model: UncertaintyModel::Exact,
            known_failure_modes: Vec::new(),
        })
    }

    /// Disclaims every plane the manifest does not establish.
    ///
    /// The honest default for a narrow oracle. A schema oracle that says only "I establish
    /// artifact integrity" leaves seven planes unstated, and an unstated plane reads as silence
    /// where 40.21 invariant 1 wants an explicit refusal.
    pub fn disclaiming_the_rest(mut self) -> Self {
        self.cannot_establish = Plane::ALL
            .iter()
            .copied()
            .filter(|plane| !self.establishes.contains(plane))
            .collect();
        self
    }

    pub fn with_independence(mut self, independence: Independence) -> Self {
        self.independence = independence;
        self
    }

    pub fn with_uncertainty_model(mut self, model: UncertaintyModel) -> Self {
        self.uncertainty_model = model;
        self
    }

    pub fn superseded_by(mut self, successor: OracleRef) -> Self {
        self.superseded_by = Some(successor);
        self
    }

    pub fn with_failure_mode(mut self, mode: impl Into<String>) -> Self {
        self.known_failure_modes.push(mode.into());
        self
    }

    pub fn kind(&self) -> &str {
        self.oracle.kind()
    }

    /// The tier combination actually uses: the declared tier, demoted one rung if the oracle is
    /// not independent of the system it evaluates (31.01, "Independence analysis").
    pub fn effective_tier(&self) -> EvidenceTier {
        if self.independence.is_circular() {
            self.declared_tier.demoted()
        } else {
            self.declared_tier
        }
    }

    /// Whether this oracle's judgement may be counted at `at`, and if not, why.
    ///
    /// Supersession is checked before the window, because a superseded oracle inside its window is
    /// the more dangerous case: it looks entirely healthy.
    pub fn admissibility(&self, at: &UtcTimestamp) -> Admissibility {
        if let Some(successor) = &self.superseded_by {
            return Admissibility::Superseded {
                by: successor.clone(),
            };
        }
        if at < &self.validity.valid_from {
            return Admissibility::NotYetValid {
                at: at.clone(),
                valid_from: self.validity.valid_from.clone(),
            };
        }
        if let Some(until) = &self.validity.valid_until {
            if at > until {
                return Admissibility::Expired {
                    at: at.clone(),
                    valid_until: until.clone(),
                };
            }
        }
        Admissibility::Admissible
    }
}
