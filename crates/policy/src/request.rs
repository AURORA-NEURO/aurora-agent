//! Who is asking, why, and where the answer is going.
//!
//! Blueprint 43.33 step 1 ("bind user/agent role and purpose to the query") and 39.19, whose
//! first non-negotiable invariant is that policy filtering happens *before* model-based relevance.
//! A [`Request`] is the object that makes that possible: it exists before selection begins and
//! carries everything the lattice needs, so nothing has to be re-derived from a rendered answer.
//!
//! The [`Channel`] is the part that is easy to leave out and expensive to leave out. 43.33's
//! invariant "raw controlled evidence never enters public prompts or caches" is a statement about
//! *destinations*, so a policy check that only knows the principal cannot enforce it. The same
//! principal asking the same question is permitted or refused depending on whether the answer
//! stays in the pod, enters a model's context window, is written to a separator cache, or is
//! published.
//!
//! Not implemented: authentication. A [`Principal`] here is an already-verified claim. This crate
//! has no notion of tokens, sessions, IAM roles or credential brokers (13.06, 13.08, 36.05), and
//! deliberately cannot tell a forged principal from a real one — that is the trusted kernel's job
//! in 36.01's enforcement architecture, and putting a plausible-looking check here would only
//! disguise its absence.

use crate::label::Classification;
use crate::purpose::Purpose;
use crate::residency::Jurisdiction;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A named authority a principal may hold, minted by the governance process of 36.15.
///
/// Used only for declassification (43.33: "explicit approved operator with leakage analysis and
/// accountable principal"). Opaque on purpose: this crate does not define which authorities exist
/// and cannot be extended into a general-purpose permission system by accident.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Authority(pub String);

impl Authority {
    pub fn new(name: impl Into<String>) -> Self {
        Authority(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Authority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a principal is cleared to see.
///
/// Two independent gates, matching the two independent axes of a label: a sensitivity ceiling and
/// a set of compartments. Holding a high ceiling does not confer a compartment, which is the whole
/// point of compartments — a senior investigator without paediatric approval is still without
/// paediatric approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clearance {
    pub max_classification: Classification,
    #[serde(default)]
    pub compartments: BTreeSet<String>,
}

impl Clearance {
    pub fn up_to(max_classification: Classification) -> Self {
        Clearance {
            max_classification,
            compartments: BTreeSet::new(),
        }
    }

    /// The clearance of an anonymous or public reader.
    pub fn public() -> Self {
        Clearance::up_to(Classification::PublicAggregate)
    }

    pub fn cleared_for(mut self, compartment: impl Into<String>) -> Self {
        self.compartments.insert(compartment.into());
        self
    }

    /// Compartments the label requires that this clearance does not hold.
    pub fn missing_compartments(&self, required: &BTreeSet<String>) -> Vec<String> {
        required.difference(&self.compartments).cloned().collect()
    }
}

/// The identified party a decision is made for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    /// Free-text role, used for the audit trail and for recipient projection upstream in
    /// `bioprism_weave`. It carries no authority here: nothing in this crate branches on it,
    /// because a role string that granted access would be an unaudited permission system.
    pub role: String,
    pub clearance: Clearance,
    /// Where this principal is operating. Compared against a label's residency to decide whether
    /// evidence may come to the principal or the computation must go to the evidence.
    pub site: Jurisdiction,
    #[serde(default)]
    pub authorities: BTreeSet<Authority>,
}

impl Principal {
    pub fn new(id: impl Into<String>, role: impl Into<String>, site: impl Into<String>) -> Self {
        Principal {
            id: id.into(),
            role: role.into(),
            clearance: Clearance::public(),
            site: Jurisdiction::new(site),
            authorities: BTreeSet::new(),
        }
    }

    pub fn with_clearance(mut self, clearance: Clearance) -> Self {
        self.clearance = clearance;
        self
    }

    pub fn holding(mut self, authority: Authority) -> Self {
        self.authorities.insert(authority);
        self
    }

    pub fn holds(&self, authority: &Authority) -> bool {
        self.authorities.contains(authority)
    }
}

/// Where an answer is going.
///
/// The ceilings below are this crate's reading of 43.33's invariant list rather than a table the
/// blueprint supplies; 43.33 names the invariant and leaves the mapping to an implementation. The
/// shape of the reading: `LocalCompute` is not an egress at all, `PublicArtifact` may only carry
/// what is already public, and the three intermediate channels — a model's context window, a
/// separator cache, and a federated aggregate boundary — are treated identically because all three
/// place data somewhere the original steward no longer controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// Stays inside the pod holding the evidence. Results, not sections, leave later.
    LocalCompute,
    /// Enters a model's context window (43.27, 39.19).
    ModelPrompt,
    /// Persisted as a separator message or reusable context cache (43.24).
    Cache,
    /// Crosses a federation boundary as an approved aggregate with a local certificate (36.06).
    FederatedAggregate,
    /// Published: a bundle, a leaderboard entry, a paper figure.
    PublicArtifact,
}

impl Channel {
    pub const ALL: [Channel; 5] = [
        Channel::LocalCompute,
        Channel::ModelPrompt,
        Channel::Cache,
        Channel::FederatedAggregate,
        Channel::PublicArtifact,
    ];

    /// The most sensitive classification this channel may carry without declassification.
    pub fn ceiling(self) -> Classification {
        match self {
            Channel::LocalCompute => Classification::RestrictedDualUse,
            Channel::ModelPrompt | Channel::Cache | Channel::FederatedAggregate => {
                Classification::InstitutionalConfidential
            }
            Channel::PublicArtifact => Classification::PublicAggregate,
        }
    }

    /// Whether using this channel moves data outside the steward's control.
    pub fn is_egress(self) -> bool {
        !matches!(self, Channel::LocalCompute)
    }

    /// Whether the destination persists what it receives, and therefore inherits the retention
    /// and deletion obligations of 36.18.
    pub fn is_persistent(self) -> bool {
        matches!(self, Channel::Cache | Channel::PublicArtifact)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Channel::LocalCompute => "local_compute",
            Channel::ModelPrompt => "model_prompt",
            Channel::Cache => "cache",
            Channel::FederatedAggregate => "federated_aggregate",
            Channel::PublicArtifact => "public_artifact",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A bound request: principal, purpose, destination and decision time.
///
/// Every field is mandatory. There is no `Request::default`, and no constructor that leaves the
/// purpose or the channel to be filled in later, because a partially bound request is exactly the
/// object a compiler would be tempted to evaluate optimistically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub principal: Principal,
    pub purpose: Purpose,
    pub channel: Channel,
    /// The decision-time cut. Consent expiry is judged against this instant.
    pub at: Timestamp,
}

impl Request {
    pub fn new(principal: Principal, purpose: Purpose, channel: Channel, at: Timestamp) -> Self {
        Request {
            principal,
            purpose,
            channel,
            at,
        }
    }

    /// The same request aimed at a different destination.
    ///
    /// Useful for the question a compiler actually asks: "this is admissible locally — is it also
    /// admissible in the prompt?"
    pub fn through(&self, channel: Channel) -> Request {
        Request {
            channel,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_high_clearance_does_not_confer_a_compartment() {
        let clearance = Clearance::up_to(Classification::RestrictedDualUse);
        let required: BTreeSet<String> = ["pediatric".to_string()].into_iter().collect();
        assert_eq!(clearance.missing_compartments(&required), vec!["pediatric"]);
    }

    #[test]
    fn holding_a_compartment_removes_it_from_the_missing_set() {
        let clearance =
            Clearance::up_to(Classification::InstitutionalConfidential).cleared_for("pediatric");
        let required: BTreeSet<String> = ["pediatric".to_string(), "dual_use".to_string()]
            .into_iter()
            .collect();
        assert_eq!(clearance.missing_compartments(&required), vec!["dual_use"]);
    }

    #[test]
    fn only_local_compute_is_not_an_egress() {
        for channel in Channel::ALL {
            assert_eq!(
                channel.is_egress(),
                channel != Channel::LocalCompute,
                "{channel} classified wrongly"
            );
        }
    }

    #[test]
    fn a_public_artifact_channel_carries_nothing_above_public_aggregate() {
        assert_eq!(
            Channel::PublicArtifact.ceiling(),
            Classification::PublicAggregate
        );
        assert!(Channel::ModelPrompt.ceiling() < Classification::ControlledGenomicOrImaging);
        assert!(Channel::Cache.ceiling() < Classification::ControlledGenomicOrImaging);
    }
}
