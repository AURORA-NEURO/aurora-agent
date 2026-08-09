//! The eight obligation states and the record that carries each one.
//!
//! Blueprint 39.06 names the state set exactly: `unseen`, `open`, `partially_supported`,
//! `satisfied`, `contradicted`, `waived_with_reason`, `not_applicable`, `unresolvable`. It is a
//! closed set on purpose — a compiler that can invent a ninth state can always find one that
//! reads as "fine", which is how obligations quietly stop gating anything.
//!
//! The same sentence requires that *each state records evidence, actor, time and confidence*, so
//! [`StateRecord`] makes all four mandatory rather than optional metadata. `unseen` is the one
//! state with nothing to record, and it is represented by the absence of any record rather than
//! by a record with empty fields: an obligation nobody has looked at should not carry a
//! timestamp and an actor implying that somebody did.
//!
//! Not implemented here: the calibration of `confidence`. The number is whatever the recording
//! actor asserts, and no part of this crate treats it as a probability. Gates may only compare it
//! against a declared floor.

use crate::error::ObligationError;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};

/// The closed state set of blueprint 39.06.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationState {
    /// Nobody has looked at this obligation yet. The default, and never a reason to proceed.
    Unseen,
    /// Looked at, not answered.
    Open,
    /// Some evidence supports it, not enough to discharge it.
    PartiallySupported,
    /// Discharged by evidence.
    Satisfied,
    /// Evidence refutes it. Distinct from `open`: the answer is known and it is the wrong one.
    Contradicted,
    /// A named actor accepted the gap and recorded why. Discharged by decision, not by evidence.
    WaivedWithReason,
    /// The obligation does not apply to this decision, with a reason.
    NotApplicable,
    /// Cannot be resolved with any evidence reachable at this decision time.
    Unresolvable,
}

impl ObligationState {
    pub const ALL: [ObligationState; 8] = [
        ObligationState::Unseen,
        ObligationState::Open,
        ObligationState::PartiallySupported,
        ObligationState::Satisfied,
        ObligationState::Contradicted,
        ObligationState::WaivedWithReason,
        ObligationState::NotApplicable,
        ObligationState::Unresolvable,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ObligationState::Unseen => "unseen",
            ObligationState::Open => "open",
            ObligationState::PartiallySupported => "partially_supported",
            ObligationState::Satisfied => "satisfied",
            ObligationState::Contradicted => "contradicted",
            ObligationState::WaivedWithReason => "waived_with_reason",
            ObligationState::NotApplicable => "not_applicable",
            ObligationState::Unresolvable => "unresolvable",
        }
    }

    pub fn parse(text: &str) -> Option<ObligationState> {
        ObligationState::ALL
            .into_iter()
            .find(|state| state.as_str() == text)
    }

    /// Whether evidence, rather than a human decision, has closed the obligation.
    ///
    /// `waived_with_reason` is deliberately excluded. A waiver is admissible at a gate that
    /// declares it acceptable, but it is not evidence and never counts toward sufficiency.
    pub fn is_evidentially_discharged(self) -> bool {
        matches!(
            self,
            ObligationState::Satisfied | ObligationState::NotApplicable
        )
    }

    /// Whether the state is a known negative rather than an absence of knowledge.
    pub fn is_negative(self) -> bool {
        matches!(
            self,
            ObligationState::Contradicted | ObligationState::Unresolvable
        )
    }

    /// States that may not be recorded without a written reason.
    pub fn requires_reason(self) -> bool {
        matches!(
            self,
            ObligationState::WaivedWithReason
                | ObligationState::NotApplicable
                | ObligationState::Unresolvable
        )
    }

    /// States that may not be recorded without at least one evidence locator.
    ///
    /// `contradicted` is in this set: an unsupported claim that something is refuted is as much a
    /// fabrication as an unsupported claim that it is proved.
    pub fn requires_evidence(self) -> bool {
        matches!(
            self,
            ObligationState::Satisfied
                | ObligationState::PartiallySupported
                | ObligationState::Contradicted
        )
    }

    /// Ordering used when a dependency constrains a dependent obligation.
    ///
    /// This is a support ordering, not the declaration order of the enum: lower means the
    /// decision rests on less. `not_applicable` ranks above `satisfied` because an obligation
    /// that does not apply cannot be weakened by anything upstream.
    pub(crate) fn support_rank(self) -> u8 {
        match self {
            ObligationState::Contradicted => 0,
            ObligationState::Unresolvable => 1,
            ObligationState::Unseen => 2,
            ObligationState::Open => 3,
            ObligationState::PartiallySupported => 4,
            ObligationState::WaivedWithReason => 5,
            ObligationState::Satisfied => 6,
            ObligationState::NotApplicable => 7,
        }
    }

    pub(crate) fn weaker_of(self, other: ObligationState) -> ObligationState {
        if other.support_rank() < self.support_rank() {
            other
        } else {
            self
        }
    }
}

/// One transition of an obligation, with the four attributes 39.06 requires.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateRecord {
    pub state: ObligationState,
    /// Who asserted it. A state with no actor cannot be audited or revoked.
    pub actor: String,
    /// When the assertion was made, in the decision's time base.
    pub at: Timestamp,
    /// The actor's own confidence, in `0.0..=1.0`. Uncalibrated by construction.
    pub confidence: f64,
    /// Stable locators for the evidence behind the assertion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// Required for waivers, inapplicability and unresolvability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl StateRecord {
    pub fn new(
        state: ObligationState,
        actor: impl Into<String>,
        at: Timestamp,
        confidence: f64,
    ) -> Self {
        StateRecord {
            state,
            actor: actor.into(),
            at,
            confidence,
            evidence: Vec::new(),
            reason: None,
        }
    }

    pub fn with_evidence<I, S>(mut self, evidence: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.evidence = evidence.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Rejects records that would leave an unauditable state behind.
    ///
    /// Called by [`crate::graph::Obligation::record`], so an obligation cannot reach an
    /// inconsistent state through the public API.
    pub fn validate(&self, obligation: &str) -> Result<(), ObligationError> {
        if self.actor.trim().is_empty() {
            return Err(ObligationError::ActorRequired(obligation.to_string()));
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(ObligationError::ConfidenceOutOfRange {
                obligation: obligation.to_string(),
                confidence: self.confidence,
            });
        }
        if self.state.requires_reason()
            && self
                .reason
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return Err(ObligationError::ReasonRequired {
                obligation: obligation.to_string(),
                state: self.state.as_str().to_string(),
            });
        }
        if self.state.requires_evidence() && self.evidence.is_empty() {
            return Err(ObligationError::EvidenceRequired {
                obligation: obligation.to_string(),
                state: self.state.as_str().to_string(),
            });
        }
        if self.state == ObligationState::Unseen && !self.evidence.is_empty() {
            return Err(ObligationError::EvidenceOnUnseen(obligation.to_string()));
        }
        Ok(())
    }
}
