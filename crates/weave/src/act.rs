//! Communicative acts.
//!
//! Blueprint 23.05: collaboration is a sequence of **typed state transitions**, not transcript
//! exchange. An act is a move in a protocol, and the kernel accepts it only if it is legal from
//! the current role state (23.06, session types).
//!
//! The distinction that matters: a `claim` asserts something into the epistemic ledger, a
//! `propose` offers a commitment, an `accept` creates one, and a `challenge` disputes a claim
//! without retracting it. Collapsing these into "messages" is what makes transcript-based
//! multi-agent systems impossible to audit — you cannot tell what anyone actually undertook.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActKind {
    /// Request information or work. Opens an exchange.
    Ask,
    /// Assert a proposition with evidence. Enters the epistemic ledger.
    Claim,
    /// Offer to undertake something. Creates no obligation by itself.
    Propose,
    /// Accept a proposal. This is the only act that creates a commitment.
    Accept,
    /// Decline a proposal, closing it without a commitment.
    Reject,
    /// Dispute a claim. Does not retract it; both survive in the ledger.
    Challenge,
    /// Discharge an accepted commitment.
    Discharge,
    /// Pass authority to another participant, necessarily attenuated.
    Delegate,
    /// Withdraw a grant, transitively.
    Revoke,
    /// Vouch for an artifact or result with a digest.
    Attest,
}

impl ActKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActKind::Ask => "ask",
            ActKind::Claim => "claim",
            ActKind::Propose => "propose",
            ActKind::Accept => "accept",
            ActKind::Reject => "reject",
            ActKind::Challenge => "challenge",
            ActKind::Discharge => "discharge",
            ActKind::Delegate => "delegate",
            ActKind::Revoke => "revoke",
            ActKind::Attest => "attest",
        }
    }

    /// Acts that must refer to a prior act, and what kind that must be.
    ///
    /// This is the session type in its smallest useful form: you cannot accept what was not
    /// proposed, challenge what was not claimed, or discharge what was not accepted.
    pub fn requires_antecedent(self) -> Option<&'static [ActKind]> {
        match self {
            ActKind::Accept | ActKind::Reject => Some(&[ActKind::Propose]),
            ActKind::Challenge => Some(&[ActKind::Claim]),
            ActKind::Discharge => Some(&[ActKind::Accept]),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Act {
    pub kind: ActKind,
    pub from: String,
    pub to: String,
    /// The act this one responds to, where the protocol requires one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    /// Subject matter. Opaque to the kernel: it enforces protocol, not scientific truth (23.49).
    pub payload: Value,
}

impl Act {
    pub fn new(
        kind: ActKind,
        from: impl Into<String>,
        to: impl Into<String>,
        payload: Value,
    ) -> Self {
        Act {
            kind,
            from: from.into(),
            to: to.into(),
            in_reply_to: None,
            payload,
        }
    }

    pub fn replying_to(mut self, event_id: impl Into<String>) -> Self {
        self.in_reply_to = Some(event_id.into());
        self
    }
}
