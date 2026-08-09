//! The policy trace.
//!
//! Blueprint 43.33 step 7: "Emit a policy trace in the certificate." 36.01 and 36.17 want every
//! grant, access, export and policy decision recorded; 39.19 wants a privacy exposure ledger
//! alongside the authorized subgraph rather than instead of it.
//!
//! A [`PolicyTrace`] records admissions as well as refusals. Recording only refusals would make an
//! all-clear trace indistinguishable from a trace that was never written — the same defect
//! [`crate::redaction::RedactedView`] is shaped against, one level up.
//!
//! The trace is also the bridge into `bioprism_section`. [`PolicyTrace::omission_group`] turns the
//! refusals into an [`OmissionGroup`] classified [`InfluenceClass::InaccessibleByPolicy`], which is
//! the class the certificate already reserves for exactly this, and
//! [`PolicyTrace::unresolved_obligations`] turns them into
//! [`UnresolvedObligation::PolicyBlocked`] entries that appear in a Decision Section *before* any
//! narrative rendering. Policy-driven omission therefore lands in the same receipt as every other
//! omission instead of quietly shortening the answer.
//!
//! Not implemented: append-only storage, signing, tamper evidence or clock attestation. The trace
//! is an in-memory value; the append-only ledger of 36.17 is a different component and this one
//! could be discarded by its caller without anything noticing.

use crate::decision::{Admission, Refusal};
use crate::flow::DeclassificationReceipt;
use crate::redaction::RedactionReceipt;
use bioprism_section::{InfluenceClass, OmissionGroup, UnresolvedObligation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Representative refused ids carried inline in an omission group. Larger sets are summarised by
/// count, matching how 43.26 keeps manifests from becoming the thing they describe.
const MAX_EXAMPLES: usize = 5;

/// What happened to one subject during compilation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum TraceOutcome {
    Admitted { admission: Box<Admission> },
    Refused { refusal: Refusal },
    Declassified { receipt: Box<DeclassificationReceipt> },
    Redacted { receipt: Box<RedactionReceipt> },
}

/// One entry in the trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEntry {
    /// The fact, factor, message or cache key the decision was about.
    pub subject: String,
    /// The evidence scope, rendered for a human reading the receipt.
    pub scope: String,
    pub outcome: TraceOutcome,
}

impl TraceEntry {
    pub fn admitted(subject: String, scope: String, admission: &Admission) -> Self {
        TraceEntry {
            subject,
            scope,
            outcome: TraceOutcome::Admitted {
                admission: Box::new(admission.clone()),
            },
        }
    }

    pub fn refused(subject: String, scope: String, refusal: Refusal) -> Self {
        TraceEntry {
            subject,
            scope,
            outcome: TraceOutcome::Refused { refusal },
        }
    }

    pub fn declassified(subject: String, scope: String, receipt: DeclassificationReceipt) -> Self {
        TraceEntry {
            subject,
            scope,
            outcome: TraceOutcome::Declassified {
                receipt: Box::new(receipt),
            },
        }
    }

    pub fn redacted(subject: String, scope: String, receipt: RedactionReceipt) -> Self {
        TraceEntry {
            subject,
            scope,
            outcome: TraceOutcome::Redacted {
                receipt: Box::new(receipt),
            },
        }
    }

    pub fn refusal(&self) -> Option<&Refusal> {
        match &self.outcome {
            TraceOutcome::Refused { refusal } => Some(refusal),
            _ => None,
        }
    }
}

/// The ordered record of every policy decision taken during one compilation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyTrace {
    /// The lattice version the decisions were taken under. A trace replayed against a different
    /// version is not a replay, and this is how a verifier notices.
    pub policy_version: String,
    entries: Vec<TraceEntry>,
}

impl PolicyTrace {
    pub fn new(policy_version: impl Into<String>) -> Self {
        PolicyTrace {
            policy_version: policy_version.into(),
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, entry: TraceEntry) {
        self.entries.push(entry);
    }

    pub fn entries(&self) -> &[TraceEntry] {
        &self.entries
    }

    pub fn refusals(&self) -> impl Iterator<Item = (&str, &Refusal)> {
        self.entries.iter().filter_map(|entry| {
            entry
                .refusal()
                .map(|refusal| (entry.subject.as_str(), refusal))
        })
    }

    pub fn admissions(&self) -> impl Iterator<Item = (&str, &Admission)> {
        self.entries.iter().filter_map(|entry| match &entry.outcome {
            TraceOutcome::Admitted { admission } => {
                Some((entry.subject.as_str(), admission.as_ref()))
            }
            _ => None,
        })
    }

    pub fn refusal_count(&self) -> usize {
        self.refusals().count()
    }

    /// The refusals as a certificate omission group.
    ///
    /// [`InfluenceClass::InaccessibleByPolicy`] is deliberately not
    /// [`InfluenceClass::Zero`]: policy-blocked evidence may well have changed the decision, and
    /// the certificate must say the gap exists rather than claim it could not matter.
    pub fn omission_group(&self) -> OmissionGroup {
        let refused: Vec<&str> = self.refusals().map(|(subject, _)| subject).collect();
        OmissionGroup {
            reason: "refused by policy during compilation".to_string(),
            influence: InfluenceClass::InaccessibleByPolicy,
            count: refused.len(),
            bound: None,
            examples: refused
                .into_iter()
                .take(MAX_EXAMPLES)
                .map(str::to_string)
                .collect(),
        }
    }

    /// The refusals as Decision Section obligations the compiler could not discharge.
    pub fn unresolved_obligations(&self) -> Vec<UnresolvedObligation> {
        self.refusals()
            .map(|(subject, refusal)| UnresolvedObligation::PolicyBlocked {
                detail: format!("{subject}: [{}] {refusal}", refusal.constraint()),
            })
            .collect()
    }

    /// Whether a context compiled under this trace may claim it saw everything relevant.
    ///
    /// False as soon as anything was refused, for the same reason a `bioprism_weave` capsule with
    /// withheld items cannot support the claim: nobody can vouch for the completeness of a set
    /// they were not shown.
    pub fn supports_sufficiency_claim(&self) -> bool {
        self.refusal_count() == 0
    }

    /// The wire form for embedding in a Context Certificate.
    pub fn to_json(&self) -> Value {
        json!({
            "policy_version": self.policy_version,
            "decisions": self.entries.len(),
            "refused": self.refusal_count(),
            "supports_sufficiency_claim": self.supports_sufficiency_claim(),
            "entries": self.entries,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::ExecutionMode;
    use crate::label::{Classification, PolicyLabel};
    use crate::purpose::{Purpose, PurposeSet};

    fn admission() -> Admission {
        Admission {
            label: PolicyLabel::public(),
            mode: ExecutionMode::Central,
            obligations: Vec::new(),
            rules: vec!["rule.public@1".to_string()],
        }
    }

    fn trace_with_one_of_each() -> PolicyTrace {
        let mut trace = PolicyTrace::new("sha256:deadbeef");
        trace.push(TraceEntry::admitted(
            "fact.open".into(),
            "{cohort=A}".into(),
            &admission(),
        ));
        trace.push(TraceEntry::refused(
            "fact.controlled".into(),
            "{cohort=B}".into(),
            Refusal::ClearanceInsufficient {
                principal: "p".into(),
                required: Classification::ControlledGenomicOrImaging,
                held: Classification::PublicAggregate,
            },
        ));
        trace.push(TraceEntry::refused(
            "fact.training".into(),
            "{cohort=C}".into(),
            Refusal::PurposeNotConsented {
                consent_id: "c1".into(),
                requested: Purpose::ModelTraining,
                permitted: PurposeSet::of([Purpose::ResearchAnalysis]),
            },
        ));
        trace
    }

    #[test]
    fn the_trace_records_admissions_as_well_as_refusals() {
        let trace = trace_with_one_of_each();
        assert_eq!(trace.entries().len(), 3);
        assert_eq!(trace.admissions().count(), 1);
        assert_eq!(trace.refusal_count(), 2);
    }

    #[test]
    fn a_refused_fact_becomes_an_omission_group_that_cannot_support_sufficiency() {
        let trace = trace_with_one_of_each();
        let group = trace.omission_group();

        assert_eq!(group.influence, InfluenceClass::InaccessibleByPolicy);
        assert!(!group.influence.supports_sufficiency());
        assert_eq!(group.count, 2);
        assert!(group.examples.contains(&"fact.controlled".to_string()));
        assert!(!trace.supports_sufficiency_claim());
    }

    #[test]
    fn an_empty_trace_is_still_a_trace_and_supports_a_sufficiency_claim() {
        let trace = PolicyTrace::new("sha256:deadbeef");
        assert!(trace.supports_sufficiency_claim());
        assert_eq!(trace.omission_group().count, 0);
        assert_eq!(trace.to_json()["refused"], json!(0));
    }

    #[test]
    fn each_refusal_becomes_an_unresolved_obligation_naming_its_constraint() {
        let obligations = trace_with_one_of_each().unresolved_obligations();

        assert_eq!(obligations.len(), 2);
        match &obligations[0] {
            UnresolvedObligation::PolicyBlocked { detail } => {
                assert!(detail.contains("fact.controlled"));
                assert!(detail.contains("clearance_insufficient"));
            }
            other => panic!("expected a policy-blocked obligation, got {other:?}"),
        }
    }

    #[test]
    fn the_trace_carries_the_policy_version_it_was_taken_under() {
        let trace = trace_with_one_of_each();
        assert_eq!(trace.to_json()["policy_version"], json!("sha256:deadbeef"));
    }

    #[test]
    fn a_trace_round_trips_through_json() {
        let trace = trace_with_one_of_each();
        let text = serde_json::to_string(&trace).unwrap();
        let parsed: PolicyTrace = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, trace);
    }
}
