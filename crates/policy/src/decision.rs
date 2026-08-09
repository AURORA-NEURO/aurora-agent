//! What the policy layer answers, and how it says no.
//!
//! Blueprint 43.33 step 4 ("choose central, local, federated, redacted, or unavailable
//! execution") and its failure-mode list, which requires that no legal execution path produce "a
//! typed inaccessible-evidence abstention" rather than an empty or quietly shortened answer.
//!
//! [`Refusal`] is the load-bearing type. Every variant names the constraint that blocked the
//! request and the values that made it fail, because the difference between "you are not cleared
//! for the paediatric compartment" and "the participants withdrew consent" decides whether the
//! remedy is an access request, a different cohort, or nothing at all. A single opaque `Denied`
//! would be cheaper to implement and would turn every one of those into the same dead end.
//!
//! [`Decision::Admit`] carries [`Obligation`]s rather than performing them. This crate has no
//! audit ledger, no cache, no scheduler and no aggregation engine, so it states what the caller
//! owes and leaves discharge to the layer that owns those things. An admission whose obligations
//! are dropped on the floor is a caller defect that this crate cannot detect, which is worth
//! saying out loud.

use crate::label::{Classification, ExportPolicy, PolicyLabel};
use crate::purpose::{Purpose, PurposeSet};
use crate::request::{Authority, Channel};
use crate::residency::{Jurisdiction, Residency};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A typed reason a request was refused, naming the constraint that blocked it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[serde(tag = "constraint", rename_all = "snake_case")]
pub enum Refusal {
    /// No policy rule claims this scope. 43.33: unknown policy defaults to no export.
    #[error("no policy rule claims scope {scope}; unknown policy denies by default")]
    UnlabelledEvidence { scope: String },

    #[error(
        "purpose {requested} is outside consent {consent_id}, which permits {permitted}"
    )]
    PurposeNotConsented {
        consent_id: String,
        requested: Purpose,
        permitted: PurposeSet,
    },

    #[error("consent {consent_id} explicitly prohibits purpose {requested}")]
    PurposeProhibited {
        consent_id: String,
        requested: Purpose,
    },

    #[error("consent {consent_id} was withdrawn at {withdrawn_at}")]
    ConsentWithdrawn {
        consent_id: String,
        withdrawn_at: String,
    },

    #[error("consent {consent_id} expired at {expired_at}, before decision time {decision_time}")]
    ConsentExpired {
        consent_id: String,
        expired_at: String,
        decision_time: String,
    },

    /// The scope's own label admits fewer purposes than any single consent does, because it is the
    /// join of several. Distinct from [`Refusal::PurposeNotConsented`]: no one consent refused
    /// this, their combination did.
    #[error("purpose {requested} is outside the joined permitted set {permitted} of this scope")]
    PurposeNotPermittedByLabel {
        requested: Purpose,
        permitted: PurposeSet,
    },

    #[error(
        "principal {principal} is cleared to {held} but this evidence is classified {required}"
    )]
    ClearanceInsufficient {
        principal: String,
        required: Classification,
        held: Classification,
    },

    #[error("principal {principal} does not hold compartments {missing:?}")]
    CompartmentNotCleared {
        principal: String,
        missing: Vec<String>,
    },

    #[error("evidence may reside in {permitted} and site {site} is not among them")]
    ResidencyViolation {
        site: Jurisdiction,
        permitted: Residency,
    },

    /// Every jurisdiction has been excluded by the join, so no site may hold this combination.
    #[error("no legal execution path: {detail}")]
    NoLegalExecutionPath { detail: String },

    #[error("export policy {export} forbids this move: {detail}")]
    ExportForbidden { export: ExportPolicy, detail: String },

    #[error(
        "channel {channel} carries at most {ceiling}; this evidence is classified {classification}"
    )]
    ChannelCeilingExceeded {
        channel: Channel,
        classification: Classification,
        ceiling: Classification,
    },

    /// A zero-day retention window and a destination that persists. 36.18.
    #[error("retention forbids persisting this evidence on channel {channel}")]
    RetentionForbidsPersistence { channel: Channel },

    /// A derived artifact was labelled less restrictively than its sources allow, on the named
    /// axis. Distinct from a declassification refusal: nobody claimed an authority here, the
    /// relabelling was simply attempted.
    #[error("flow would downgrade the {axis} axis: {detail}")]
    FlowWouldDowngrade { axis: String, detail: String },

    #[error("declassification rule {rule_id} v{version} is not registered")]
    UnknownDeclassificationRule { rule_id: String, version: u32 },

    #[error(
        "principal {principal} does not hold authority {authority} required by \
         declassification rule {rule_id} v{version}"
    )]
    DeclassificationUnauthorized {
        rule_id: String,
        version: u32,
        authority: Authority,
        principal: String,
    },

    #[error("declassification rule {rule_id} v{version} does not apply here: {detail}")]
    DeclassificationOutOfRange {
        rule_id: String,
        version: u32,
        detail: String,
    },

    #[error("a move into {to} did not declare where it lands: no {dimension} binding")]
    UndeclaredDestination { to: String, dimension: String },

    #[error("a transport into {to} was proposed without a justification")]
    TransportWithoutJustification { to: String },
}

impl Refusal {
    /// A stable name for the violated constraint, for traces, metrics and conformance fixtures.
    pub fn constraint(&self) -> &'static str {
        match self {
            Refusal::UnlabelledEvidence { .. } => "unlabelled_evidence",
            Refusal::PurposeNotConsented { .. } => "purpose_not_consented",
            Refusal::PurposeProhibited { .. } => "purpose_prohibited",
            Refusal::ConsentWithdrawn { .. } => "consent_withdrawn",
            Refusal::ConsentExpired { .. } => "consent_expired",
            Refusal::PurposeNotPermittedByLabel { .. } => "purpose_not_permitted_by_label",
            Refusal::ClearanceInsufficient { .. } => "clearance_insufficient",
            Refusal::CompartmentNotCleared { .. } => "compartment_not_cleared",
            Refusal::ResidencyViolation { .. } => "residency_violation",
            Refusal::NoLegalExecutionPath { .. } => "no_legal_execution_path",
            Refusal::ExportForbidden { .. } => "export_forbidden",
            Refusal::ChannelCeilingExceeded { .. } => "channel_ceiling_exceeded",
            Refusal::RetentionForbidsPersistence { .. } => "retention_forbids_persistence",
            Refusal::FlowWouldDowngrade { .. } => "flow_would_downgrade",
            Refusal::UnknownDeclassificationRule { .. } => "unknown_declassification_rule",
            Refusal::DeclassificationUnauthorized { .. } => "declassification_unauthorized",
            Refusal::DeclassificationOutOfRange { .. } => "declassification_out_of_range",
            Refusal::UndeclaredDestination { .. } => "undeclared_destination",
            Refusal::TransportWithoutJustification { .. } => "transport_without_justification",
        }
    }

    /// Whether a different request could succeed where this one failed.
    ///
    /// A clearance or compartment gap is answerable by an access request; a withdrawn consent is
    /// not answerable by anything. 39.19 asks for refusal *and escalation* to be distinguishable,
    /// and this is the distinction that decides which one a caller should offer.
    pub fn is_escalatable(&self) -> bool {
        matches!(
            self,
            Refusal::ClearanceInsufficient { .. }
                | Refusal::CompartmentNotCleared { .. }
                | Refusal::ChannelCeilingExceeded { .. }
                | Refusal::ResidencyViolation { .. }
                | Refusal::DeclassificationUnauthorized { .. }
        )
    }
}

/// Where the computation has to happen for the answer to be legal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ExecutionMode {
    /// The requester may read the sections where it sits.
    Central,
    /// The evidence stays at `site`; computation moves there and results come back.
    Local { site: Jurisdiction },
    /// Only an approved aggregate and a signed local certificate cross the boundary (36.06).
    Federated { site: Jurisdiction },
}

/// A duty attached to an admission.
///
/// Stated, not performed: this crate owns no ledger, cache or aggregation engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "obligation", rename_all = "snake_case")]
pub enum Obligation {
    /// Run at this site; do not fetch the sections.
    ComputeAt { site: Jurisdiction },
    /// Release aggregates only; individual sections may not cross.
    AggregatesOnly,
    /// Suppress any cell with fewer than `threshold` members (36.02, 36.03).
    SuppressSmallCells { threshold: u32 },
    /// Return a signed certificate from the local pod alongside the aggregate.
    EmitLocalCertificate,
    /// Delete by this instant. Derived from the label's retention and the decision time.
    DeleteBy { at: String },
    /// Stamp the cache entry with this policy version so a rule change invalidates it (43.33:
    /// "policy changes invalidate cached sections and messages").
    KeyCacheToPolicyVersion { version: String },
    /// Record the access in the append-only audit ledger (36.17).
    RecordAccess,
}

/// An admission, with the label the result inherits and the duties it carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Admission {
    /// The label anything derived from this evidence must carry. Already the join of every rule
    /// that applied, so a caller that folds these across selected evidence gets the right answer
    /// without re-consulting the lattice.
    pub label: PolicyLabel,
    pub mode: ExecutionMode,
    pub obligations: Vec<Obligation>,
    /// Ids of the policy rules that produced `label`, for the trace and the certificate.
    pub rules: Vec<String>,
}

/// The answer the lattice gives while the compiler is still selecting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Admit(Admission),
    Refuse(Refusal),
}

impl Decision {
    pub fn is_admitted(&self) -> bool {
        matches!(self, Decision::Admit(_))
    }

    pub fn admission(&self) -> Option<&Admission> {
        match self {
            Decision::Admit(admission) => Some(admission),
            Decision::Refuse(_) => None,
        }
    }

    pub fn refusal(&self) -> Option<&Refusal> {
        match self {
            Decision::Refuse(refusal) => Some(refusal),
            Decision::Admit(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_renders_a_message_naming_its_constraint() {
        let refusals = [
            Refusal::UnlabelledEvidence {
                scope: "{cohort=X}".into(),
            },
            Refusal::PurposeNotConsented {
                consent_id: "c1".into(),
                requested: Purpose::ModelTraining,
                permitted: PurposeSet::of([Purpose::ResearchAnalysis]),
            },
            Refusal::ClearanceInsufficient {
                principal: "p1".into(),
                required: Classification::ControlledGenomicOrImaging,
                held: Classification::PublicAggregate,
            },
            Refusal::CompartmentNotCleared {
                principal: "p1".into(),
                missing: vec!["pediatric".into()],
            },
            Refusal::ResidencyViolation {
                site: Jurisdiction::new("us"),
                permitted: Residency::only(["eu"]),
            },
            Refusal::ChannelCeilingExceeded {
                channel: Channel::Cache,
                classification: Classification::ControlledGenomicOrImaging,
                ceiling: Classification::InstitutionalConfidential,
            },
        ];

        for refusal in refusals {
            let rendered = refusal.to_string();
            assert!(!rendered.is_empty());
            assert!(!refusal.constraint().is_empty());
        }
    }

    #[test]
    fn a_withdrawn_consent_is_not_escalatable_but_a_missing_compartment_is() {
        let withdrawn = Refusal::ConsentWithdrawn {
            consent_id: "c1".into(),
            withdrawn_at: "2026-01-01T00:00:00Z".into(),
        };
        let compartment = Refusal::CompartmentNotCleared {
            principal: "p1".into(),
            missing: vec!["pediatric".into()],
        };

        assert!(!withdrawn.is_escalatable());
        assert!(compartment.is_escalatable());
    }

    #[test]
    fn a_refusal_round_trips_through_json_keeping_its_constraint_tag() {
        let refusal = Refusal::PurposeNotConsented {
            consent_id: "consent.glioma-eu.v3".into(),
            requested: Purpose::ModelTraining,
            permitted: PurposeSet::of([Purpose::ResearchAnalysis]),
        };
        let text = serde_json::to_string(&refusal).unwrap();
        assert!(text.contains("purpose_not_consented"));
        let parsed: Refusal = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, refusal);
    }
}
