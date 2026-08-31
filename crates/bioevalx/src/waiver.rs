//! Waiving a release gate, and the gate that cannot be waived (07.13).
//!
//! Most of 07.13 is already built. `bioprism-metrics::gate` owns release predicates with met /
//! violated / **unevaluable** outcomes, and `bioprism-evalengine::posterior` owns the coverage
//! floors that make an overall figure refuse. Neither owns 07.13's override paragraph:
//!
//! > Authorized humans may override with rationale, expiry, affected versions, and required
//! > follow-up. Overrides are public within the organization's evidence record.
//!
//! Four required fields, an expiry, and a publicity requirement. That is a set of predicates over
//! an artifact, so it is here. The rest of 07.13 — the CI comment's content, the review cadence,
//! what a change-aware panel should contain — is process, or belongs to `bioprism-adaptive`, and
//! is named as such in this crate's `lib.rs` rather than reimplemented.
//!
//! # A veto is not waivable
//!
//! [`Waiver::apply`] refuses a gate marked [`GateKind::SafetyVeto`]. 07.09's rule is that a
//! materialized forbidden action is a veto, and a veto an authorised human can sign away is a
//! warning with extra paperwork. The refusal is [`WaiverError::VetoNotWaivable`] and there is no
//! flag, force parameter or second method that gets around it.
//!
//! # Expiry is evaluated against a supplied instant
//!
//! [`Waiver::apply`] takes the instant to evaluate at. Nothing here reads a clock: a waiver that
//! silently expires between two runs of the same test is not deterministic, and this workspace's
//! certificates hash canonical bytes. An expired waiver is [`WaiverError::Expired`] — the gate is
//! in force again, which is what an expiry *means* and is routinely implemented as a warning
//! instead.
//!
//! # A waiver does not change the gate's verdict
//!
//! [`WaivedGate`] carries the original [`GateVerdict`] alongside the waiver. The gate still says
//! violated; what the waiver changes is whether the violation blocks. Overwriting the verdict
//! would erase the evidence record 07.13 requires to be public, and a later reader would see a
//! passing gate with no trace of why.
//!
//! # Not implemented
//!
//! No gate predicates and no statistical gate. 07.13 lists eight gate types ("non-inferiority
//! margin", "confidence requirement", "maximum unknown rate") and requires intervals or posterior
//! probabilities rather than point estimates; `bioprism-metrics` implements that arithmetic and
//! this module consumes its verdicts as data. No CI comment rendering. No approval workflow, no
//! identity system, and no check that the named authoriser is actually authorised — this crate
//! records the assertion and cannot verify it, which is the same position `bioprism-safety` takes
//! on signatures.

use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};

use crate::error::WaiverError;

const MAX_WAIVER_TEXT_BYTES: usize = 256;
const MAX_GATES: usize = 4096;
const MAX_WAIVERS: usize = 4096;

/// The kinds of gate 07.13 enumerates under "Gate types".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateKind {
    /// "No severe safety violation". The one that cannot be waived.
    SafetyVeto,
    /// "benchmark-health minimum"
    BenchmarkHealth,
    /// "capability floor"
    CapabilityFloor,
    /// "non-inferiority margin"
    NonInferiority,
    /// "required improvement"
    RequiredImprovement,
    /// "cost ceiling"
    CostCeiling,
    /// "confidence requirement"
    ConfidenceRequirement,
    /// "maximum unknown rate"
    MaximumUnknownRate,
}

impl GateKind {
    /// All eight, in blueprint listing order.
    pub const ALL: [GateKind; 8] = [
        GateKind::SafetyVeto,
        GateKind::BenchmarkHealth,
        GateKind::CapabilityFloor,
        GateKind::NonInferiority,
        GateKind::RequiredImprovement,
        GateKind::CostCeiling,
        GateKind::ConfidenceRequirement,
        GateKind::MaximumUnknownRate,
    ];

    /// Whether a human may sign this gate away.
    pub fn is_waivable(self) -> bool {
        !matches!(self, GateKind::SafetyVeto)
    }
}

/// What a gate decided.
///
/// The third variant is the important one and is taken from `bioprism-metrics`, which reports a
/// gate as unevaluable when the evidence needed to evaluate it is absent. An unevaluable gate is
/// not a passing gate, and 07.13's "maximum unknown rate" exists precisely because a release can
/// accumulate them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum GateVerdict {
    Met,
    Violated { detail: String },
    Unevaluable { missing: String },
}

impl GateVerdict {
    /// Whether this verdict stops a release in the absence of a waiver.
    pub fn blocks(&self) -> bool {
        !matches!(self, GateVerdict::Met)
    }
}

/// One gate and its verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gate {
    pub id: String,
    pub kind: GateKind,
    pub verdict: GateVerdict,
}

impl Gate {
    /// Declare a gate and its verdict.
    pub fn new(id: impl Into<String>, kind: GateKind, verdict: GateVerdict) -> Self {
        Gate {
            id: id.into(),
            kind,
            verdict,
        }
    }

    fn validate(&self) -> Result<(), WaiverError> {
        validate_waiver_text(&self.id, "id").map_err(|detail| WaiverError::InvalidGate {
            id: self.id.clone(),
            detail,
        })?;
        match &self.verdict {
            GateVerdict::Met => {}
            GateVerdict::Violated { detail } => {
                validate_waiver_text(detail, "verdict detail").map_err(|detail| {
                    WaiverError::InvalidGate {
                        id: self.id.clone(),
                        detail,
                    }
                })?;
            }
            GateVerdict::Unevaluable { missing } => {
                validate_waiver_text(missing, "missing evidence").map_err(|detail| {
                    WaiverError::InvalidGate {
                        id: self.id.clone(),
                        detail,
                    }
                })?;
            }
        }
        Ok(())
    }
}

/// A human decision to let a blocking gate through, with everything 07.13 requires.
///
/// The fields are private and [`Waiver::sign`] checks all four, so a `Waiver` in hand is a complete
/// one. An incomplete waiver is not a weak waiver; it is not a waiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Waiver {
    gate: String,
    authoriser: String,
    rationale: String,
    expiry: Timestamp,
    affected_versions: Vec<String>,
    follow_up: String,
}

impl Waiver {
    /// Sign a waiver, refusing an incomplete one.
    ///
    /// The four refusals correspond one-to-one with 07.13's four required elements. Whitespace
    /// counts as absent: the check cannot tell a real rationale from the word "ok", and it does not
    /// pretend to — what it can guarantee is that the field exists to be read in review, which is
    /// what "public within the organization's evidence record" is for.
    pub fn sign(
        gate: impl Into<String>,
        authoriser: impl Into<String>,
        rationale: impl Into<String>,
        expiry: Timestamp,
        affected_versions: Vec<String>,
        follow_up: impl Into<String>,
    ) -> Result<Self, WaiverError> {
        let waiver = Waiver {
            gate: gate.into(),
            authoriser: authoriser.into(),
            rationale: rationale.into(),
            expiry,
            affected_versions,
            follow_up: follow_up.into(),
        };
        if waiver.authoriser.trim().is_empty() {
            return Err(WaiverError::NoAuthoriser);
        }
        if waiver.rationale.trim().is_empty() {
            return Err(WaiverError::NoRationale);
        }
        if waiver.affected_versions.iter().all(|v| v.trim().is_empty()) {
            return Err(WaiverError::NoAffectedVersion);
        }
        if waiver.follow_up.trim().is_empty() {
            return Err(WaiverError::NoFollowUp);
        }
        waiver.validate()?;
        Ok(waiver)
    }

    /// Apply this waiver to a gate at a given instant.
    ///
    /// Three refusals: a gate that was not blocking has nothing to waive, a safety veto cannot be
    /// waived at all, and an expired waiver leaves the gate in force.
    pub fn apply(self, gate: &Gate, at: Timestamp) -> Result<WaivedGate, WaiverError> {
        self.validate()?;
        gate.validate()?;
        if self.gate != gate.id {
            return Err(WaiverError::GateMismatch {
                waiver: self.gate,
                gate: gate.id.clone(),
            });
        }
        if !gate.verdict.blocks() {
            return Err(WaiverError::NotBlocking(gate.id.clone()));
        }
        if !gate.kind.is_waivable() {
            return Err(WaiverError::VetoNotWaivable {
                gate: gate.id.clone(),
            });
        }
        if at > self.expiry {
            return Err(WaiverError::Expired {
                gate: gate.id.clone(),
                expiry: self.expiry.to_rfc3339(),
            });
        }
        Ok(WaivedGate {
            gate: gate.clone(),
            waiver: self,
            applied_at: at,
        })
    }

    /// The gate this waiver names.
    pub fn gate(&self) -> &str {
        &self.gate
    }

    /// Who signed it.
    pub fn authoriser(&self) -> &str {
        &self.authoriser
    }

    /// Why.
    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    /// When it stops applying.
    pub fn expiry(&self) -> Timestamp {
        self.expiry
    }

    /// Which versions it covers.
    pub fn affected_versions(&self) -> &[String] {
        &self.affected_versions
    }

    /// What must be done afterwards.
    pub fn follow_up(&self) -> &str {
        &self.follow_up
    }

    /// Whether this waiver covers a version.
    ///
    /// Exact match only. A waiver that covers "whatever version ships next" is a standing
    /// exception, and 07.13's requirement that a waiver name affected versions exists to stop one.
    pub fn covers(&self, version: &str) -> bool {
        self.affected_versions.iter().any(|v| v == version)
    }

    fn validate(&self) -> Result<(), WaiverError> {
        validate_waiver_text(&self.gate, "gate").map_err(|detail| WaiverError::InvalidWaiver {
            gate: self.gate.clone(),
            detail,
        })?;
        validate_waiver_text(&self.authoriser, "authoriser").map_err(|detail| {
            WaiverError::InvalidWaiver {
                gate: self.gate.clone(),
                detail,
            }
        })?;
        validate_waiver_text(&self.rationale, "rationale").map_err(|detail| {
            WaiverError::InvalidWaiver {
                gate: self.gate.clone(),
                detail,
            }
        })?;
        validate_waiver_text(&self.follow_up, "follow_up").map_err(|detail| {
            WaiverError::InvalidWaiver {
                gate: self.gate.clone(),
                detail,
            }
        })?;
        if self.affected_versions.is_empty() || self.affected_versions.len() > MAX_WAIVERS {
            return Err(WaiverError::InvalidWaiver {
                gate: self.gate.clone(),
                detail: "affected_versions must contain a bounded non-empty list".into(),
            });
        }
        let mut versions = std::collections::BTreeSet::new();
        for version in &self.affected_versions {
            validate_waiver_text(version, "affected version").map_err(|detail| {
                WaiverError::InvalidWaiver {
                    gate: self.gate.clone(),
                    detail,
                }
            })?;
            if !versions.insert(version) {
                return Err(WaiverError::InvalidWaiver {
                    gate: self.gate.clone(),
                    detail: format!("affected version {} appears more than once", version),
                });
            }
        }
        Ok(())
    }
}

/// A blocking gate plus the waiver that let it through.
///
/// Carries the original verdict. A reader of this object can always see what the gate said, which
/// is what makes the waiver auditable rather than merely recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WaivedGate {
    pub gate: Gate,
    pub waiver: Waiver,
    pub applied_at: Timestamp,
}

impl WaivedGate {
    /// The verdict the gate reached, unchanged.
    pub fn underlying_verdict(&self) -> &GateVerdict {
        &self.gate.verdict
    }
}

/// The set of gates for one release, and the waivers applied to them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReleaseDecision {
    pub version: String,
    gates: Vec<Gate>,
    waived: Vec<WaivedGate>,
}

impl ReleaseDecision {
    /// Start a decision for a version.
    pub fn for_version(version: impl Into<String>, gates: Vec<Gate>) -> Self {
        ReleaseDecision {
            version: version.into(),
            gates,
            waived: Vec::new(),
        }
    }

    /// Waive one gate, refusing a waiver that does not cover this version.
    pub fn waive(&mut self, waiver: Waiver, at: Timestamp) -> Result<(), WaiverError> {
        validate_waiver_text(&self.version, "version").map_err(|detail| {
            WaiverError::InvalidWaiver {
                gate: waiver.gate.clone(),
                detail,
            }
        })?;
        if self.gates.len() > MAX_GATES {
            return Err(WaiverError::TooManyGates(MAX_GATES));
        }
        if self.waived.len() >= MAX_WAIVERS {
            return Err(WaiverError::TooManyWaivers(MAX_WAIVERS));
        }
        for gate in &self.gates {
            gate.validate()?;
        }
        waiver.validate()?;
        let gate = self
            .gates
            .iter()
            .find(|g| g.id == waiver.gate)
            .ok_or_else(|| WaiverError::NotBlocking(waiver.gate.clone()))?
            .clone();
        if !waiver.covers(&self.version) {
            return Err(WaiverError::NoAffectedVersion);
        }
        if self.waived.iter().any(|applied| applied.gate.id == gate.id) {
            return Err(WaiverError::DuplicateWaiver {
                gate: gate.id.clone(),
            });
        }
        self.waived.push(waiver.apply(&gate, at)?);
        Ok(())
    }

    /// Gates that still block after waivers.
    pub fn blocking(&self) -> Vec<&Gate> {
        self.gates
            .iter()
            .filter(|g| g.verdict.blocks())
            .filter(|g| !self.waived.iter().any(|w| w.gate.id == g.id))
            .collect()
    }

    /// Whether the release may proceed.
    pub fn releasable(&self) -> bool {
        self.blocking().is_empty()
    }

    /// Gates that could not be evaluated, waived or not.
    ///
    /// Reported separately from blocking gates because 07.13's "maximum unknown rate" is a gate
    /// over exactly this count, and a release whose gates are mostly unevaluable is a release with
    /// no evidence rather than a release with good evidence.
    pub fn unevaluable(&self) -> Vec<&Gate> {
        self.gates
            .iter()
            .filter(|g| matches!(g.verdict, GateVerdict::Unevaluable { .. }))
            .collect()
    }

    /// The waivers applied, in application order.
    pub fn waivers(&self) -> &[WaivedGate] {
        &self.waived
    }

    /// The gates, in declaration order.
    pub fn gates(&self) -> &[Gate] {
        &self.gates
    }
}

fn validate_waiver_text(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_WAIVER_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(format!(
            "{field} must be bounded, trimmed, and control-free"
        ));
    }
    Ok(())
}
