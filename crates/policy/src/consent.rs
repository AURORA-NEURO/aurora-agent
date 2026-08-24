//! Consent scopes and their binding to a request's purpose.
//!
//! Blueprint 36.04 (consent, data use and purpose limitation) and 36.18 (retention, deletion,
//! withdrawal). 43.33 requires that "evidence sections carry purpose and consent constraints" and
//! that the check happens while the compiler selects, not while it renders.
//!
//! A [`Consent`] is a machine-readable record of what participants agreed to. It contributes two
//! things to the policy fiber: an effective [`PurposeSet`] that joins into the scope's label, and
//! a set of lifecycle conditions — expiry and withdrawal — that no label axis can express because
//! they are events rather than points in an order.
//!
//! Withdrawal is deliberately *not* evaluated against the decision-time cut. A replay of a
//! decision taken before a participant withdrew must still refuse today, because withdrawal is a
//! fact about the present rather than about the cut. The alternative — honouring the historical
//! timestamp — would make withdrawal cosmetic for exactly the workloads (replay, re-derivation,
//! cache refill) that 36.18 exists to control.
//!
//! Not implemented: signature verification on a consent document, the identity of the participant,
//! the broker that issues short-lived credentials (36.05), or the appeal workflow. A `Consent`
//! here is an already-authenticated assertion handed to the compiler.

use crate::decision::Refusal;
use crate::purpose::{Purpose, PurposeSet};
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The lifecycle state of a consent at a given decision time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentStatus {
    Active,
    Expired,
    Withdrawn,
}

/// A machine-readable consent scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Consent {
    pub id: String,
    /// Purposes the participants agreed to.
    pub permitted: PurposeSet,
    /// Purposes explicitly refused. These override [`Consent::permitted`], so a consent that says
    /// both "any research purpose" and "not commercial development" behaves as written rather
    /// than as the broader clause alone.
    #[serde(default)]
    pub prohibited: BTreeSet<Purpose>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withdrawn_at: Option<Timestamp>,
}

impl Consent {
    pub fn new(id: impl Into<String>, permitted: PurposeSet) -> Self {
        Consent {
            id: id.into(),
            permitted,
            prohibited: BTreeSet::new(),
            expires_at: None,
            withdrawn_at: None,
        }
    }

    pub fn prohibiting(mut self, purpose: Purpose) -> Self {
        self.prohibited.insert(purpose);
        self
    }

    pub fn expiring_at(mut self, at: Timestamp) -> Self {
        self.expires_at = Some(at);
        self
    }

    pub fn withdrawn_at(mut self, at: Timestamp) -> Self {
        self.withdrawn_at = Some(at);
        self
    }

    /// The purposes this consent actually permits, after prohibitions are removed.
    ///
    /// This is what joins into the scope's [`crate::label::PolicyLabel`], so a prohibition
    /// propagates to every artifact derived from the evidence rather than being checked once at
    /// the point of access.
    pub fn effective_purposes(&self) -> PurposeSet {
        match &self.permitted {
            PurposeSet::Any if self.prohibited.is_empty() => PurposeSet::Any,
            PurposeSet::Any => PurposeSet::of(
                Purpose::ALL
                    .into_iter()
                    .filter(|purpose| !self.prohibited.contains(purpose)),
            ),
            PurposeSet::Only(set) => PurposeSet::of(
                set.iter()
                    .copied()
                    .filter(|purpose| !self.prohibited.contains(purpose)),
            ),
        }
    }

    pub fn status(&self, at: Timestamp) -> ConsentStatus {
        if self.withdrawn_at.is_some() {
            return ConsentStatus::Withdrawn;
        }
        match self.expires_at {
            Some(expiry) if at >= expiry => ConsentStatus::Expired,
            _ => ConsentStatus::Active,
        }
    }

    /// Whether this consent admits `purpose` at decision time `at`.
    ///
    /// The refusal names which clause blocked the request, so a caller can distinguish "the
    /// participants never agreed to this" from "they did, and then withdrew" — different problems
    /// with different remedies.
    pub fn check(&self, purpose: Purpose, at: Timestamp) -> Result<(), Refusal> {
        if let Some(withdrawn_at) = self.withdrawn_at {
            return Err(Refusal::ConsentWithdrawn {
                consent_id: self.id.clone(),
                withdrawn_at: withdrawn_at.to_rfc3339(),
            });
        }
        if let Some(expiry) = self.expires_at {
            if at >= expiry {
                return Err(Refusal::ConsentExpired {
                    consent_id: self.id.clone(),
                    expired_at: expiry.to_rfc3339(),
                    decision_time: at.to_rfc3339(),
                });
            }
        }
        if self.prohibited.contains(&purpose) {
            return Err(Refusal::PurposeProhibited {
                consent_id: self.id.clone(),
                requested: purpose,
            });
        }
        if !self.permitted.admits(purpose) {
            return Err(Refusal::PurposeNotConsented {
                consent_id: self.id.clone(),
                requested: purpose,
                permitted: self.permitted.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        Timestamp::parse(text).expect("fixture timestamp parses")
    }

    fn research_consent() -> Consent {
        Consent::new(
            "consent.glioma-eu.v3",
            PurposeSet::of([Purpose::ResearchAnalysis, Purpose::MethodDevelopment]),
        )
    }

    #[test]
    fn a_purpose_outside_the_consent_is_refused_naming_the_consent_and_the_permitted_set() {
        let refusal = research_consent()
            .check(Purpose::ModelTraining, at("2026-08-08T00:00:00Z"))
            .expect_err("training was never consented");

        match refusal {
            Refusal::PurposeNotConsented {
                consent_id,
                requested,
                permitted,
            } => {
                assert_eq!(consent_id, "consent.glioma-eu.v3");
                assert_eq!(requested, Purpose::ModelTraining);
                assert!(!permitted.admits(Purpose::ModelTraining));
            }
            other => panic!("expected a purpose refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_withdrawn_consent_refuses_a_purpose_it_previously_permitted() {
        let consent = research_consent().withdrawn_at(at("2026-01-01T00:00:00Z"));

        let refusal = consent
            .check(Purpose::ResearchAnalysis, at("2026-08-08T00:00:00Z"))
            .expect_err("withdrawal removes a previously granted purpose");

        assert!(matches!(refusal, Refusal::ConsentWithdrawn { .. }));
    }

    #[test]
    fn withdrawal_also_refuses_a_replay_of_a_decision_taken_before_the_withdrawal() {
        let consent = research_consent().withdrawn_at(at("2026-06-01T00:00:00Z"));

        let refusal = consent
            .check(Purpose::ResearchAnalysis, at("2026-01-01T00:00:00Z"))
            .expect_err("a historical cut does not resurrect a withdrawn consent");

        assert!(matches!(refusal, Refusal::ConsentWithdrawn { .. }));
        assert_eq!(
            consent.status(at("2026-01-01T00:00:00Z")),
            ConsentStatus::Withdrawn
        );
    }

    #[test]
    fn a_consent_expired_at_the_decision_time_refuses_and_names_both_instants() {
        let consent = research_consent().expiring_at(at("2026-07-01T00:00:00Z"));

        let refusal = consent
            .check(Purpose::ResearchAnalysis, at("2026-08-08T00:00:00Z"))
            .expect_err("the consent had expired by the decision time");

        match refusal {
            Refusal::ConsentExpired {
                expired_at,
                decision_time,
                ..
            } => {
                assert_eq!(expired_at, "2026-07-01T00:00:00Z");
                assert_eq!(decision_time, "2026-08-08T00:00:00Z");
            }
            other => panic!("expected an expiry refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_decision_before_the_expiry_is_still_admitted() {
        let consent = research_consent().expiring_at(at("2026-07-01T00:00:00Z"));
        assert!(consent
            .check(Purpose::ResearchAnalysis, at("2026-06-30T23:59:59Z"))
            .is_ok());
    }

    #[test]
    fn an_explicit_prohibition_beats_a_broad_permission() {
        let consent = Consent::new("consent.open", PurposeSet::Any)
            .prohibiting(Purpose::CommercialDevelopment);

        assert!(consent
            .check(Purpose::ResearchAnalysis, at("2026-08-08T00:00:00Z"))
            .is_ok());
        let refusal = consent
            .check(Purpose::CommercialDevelopment, at("2026-08-08T00:00:00Z"))
            .expect_err("the prohibition overrides the broad clause");
        assert!(matches!(refusal, Refusal::PurposeProhibited { .. }));
    }

    #[test]
    fn a_prohibition_survives_into_the_effective_purpose_set_so_it_propagates_to_derived_work() {
        let consent = Consent::new("consent.open", PurposeSet::Any)
            .prohibiting(Purpose::CommercialDevelopment);

        let effective = consent.effective_purposes();

        assert!(effective.admits(Purpose::ResearchAnalysis));
        assert!(!effective.admits(Purpose::CommercialDevelopment));
        assert_ne!(effective, PurposeSet::Any);
    }
}
