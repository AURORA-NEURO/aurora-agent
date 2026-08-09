//! Incident response and forensics, with containment that cannot be overclaimed.
//!
//! Implements blueprint 13.22 (incident response and secure release) and 13.23 (incident response
//! and forensics).
//!
//! # The one sentence this module enforces
//!
//! 13.23, under "Communication": *"State known facts and uncertainty; do not overclaim
//! containment."* That is a rule about a report, and a report is a thing a type can gate.
//! [`Incident::report_contained`] returns [`SafetyError::ContainmentOverclaimed`] unless two things
//! hold: the blast radius is [`LineageCompleteness::Complete`], and every dependent result has a
//! disposition other than [`ResultDisposition::UnderInvestigation`]. An incident whose lineage
//! query returned partial results cannot be reported contained no matter how confident anybody is,
//! because "we contained everything we found" and "we contained everything" are different
//! sentences and only one of them is a containment claim.
//!
//! # There is no `ContainmentPerformed`
//!
//! [`ContainmentRequest`] records that an action was called for. There is deliberately **no type in
//! this module that records an action as carried out**, because this crate cannot stop a worker,
//! revoke a token, unmount a volume, kill a process tree, quarantine an artifact or freeze
//! publication. 13.23's entire "First actions" list is outside this process. A
//! `ContainmentPerformed` struct would be the single most dangerous type in the workspace: an
//! incident report that says the pool was stopped when nothing stopped it.
//!
//! # Forensics without a clock
//!
//! [`Timeline`] is ordered by operator-supplied epoch and refuses an entry that moves backwards.
//! 13.23 wants an "immutable timeline from audit/operation/event logs"; the immutability that is
//! available here is [`crate::attest::AuditLog`]'s hash chain, and a timeline built on a clock the
//! compromised host controls would be a timeline the attacker controls.
//!
//! # What is deliberately not implemented
//!
//! * **No detection.** Nothing here notices an incident. Every incident is opened by an operator.
//! * **No notification, no incident channel, no commander assignment, no postmortem template.**
//! * **No lineage engine.** [`BlastRadius`] holds the answer to "which results depend on this"; the
//!   query that produces it belongs to the ledger and graph crates. What this module contributes is
//!   the [`LineageCompleteness`] field that stops a partial answer being read as a full one.
//! * **No secure-release pipeline.** 13.22's protected branches, signed artifacts and staged
//!   rollout are CI, not a library.

use crate::error::SafetyError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The incident classes 13.22 and 13.23 enumerate, merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentClass {
    ConfidentialityLeak,
    UnauthorizedEffect,
    SandboxEscape,
    CrossTenantExposure,
    MaliciousPack,
    CompromisedKey,
    ResultIntegrityFailure,
    BenchmarkExploit,
    HiddenHoldoutLeak,
    EvaluatorTampering,
    ArtifactSubstitution,
    DependencyVulnerability,
    PrivacyBreach,
    ServiceCompromise,
    WidespreadResultInvalidity,
}

impl IncidentClass {
    pub fn as_str(self) -> &'static str {
        match self {
            IncidentClass::ConfidentialityLeak => "confidentiality_leak",
            IncidentClass::UnauthorizedEffect => "unauthorized_effect",
            IncidentClass::SandboxEscape => "sandbox_escape",
            IncidentClass::CrossTenantExposure => "cross_tenant_exposure",
            IncidentClass::MaliciousPack => "malicious_pack",
            IncidentClass::CompromisedKey => "compromised_key",
            IncidentClass::ResultIntegrityFailure => "result_integrity_failure",
            IncidentClass::BenchmarkExploit => "benchmark_exploit",
            IncidentClass::HiddenHoldoutLeak => "hidden_holdout_leak",
            IncidentClass::EvaluatorTampering => "evaluator_tampering",
            IncidentClass::ArtifactSubstitution => "artifact_substitution",
            IncidentClass::DependencyVulnerability => "dependency_vulnerability",
            IncidentClass::PrivacyBreach => "privacy_breach",
            IncidentClass::ServiceCompromise => "service_compromise",
            IncidentClass::WidespreadResultInvalidity => "widespread_result_invalidity",
        }
    }

    /// Whether this class puts published numbers in doubt.
    ///
    /// Drives the requirement that every dependent result be dispositioned before containment can
    /// be claimed: a key compromise is contained when the key is dead, but a holdout leak is not
    /// contained until every result computed against that holdout has been dealt with.
    pub fn taints_results(self) -> bool {
        matches!(
            self,
            IncidentClass::ResultIntegrityFailure
                | IncidentClass::BenchmarkExploit
                | IncidentClass::HiddenHoldoutLeak
                | IncidentClass::EvaluatorTampering
                | IncidentClass::ArtifactSubstitution
                | IncidentClass::MaliciousPack
                | IncidentClass::WidespreadResultInvalidity
        )
    }
}

impl fmt::Display for IncidentClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 13.23's first actions, as things somebody asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainmentAction {
    StopExecutionPool,
    RevokeLeases,
    RevokeCredentials,
    QuarantineArtifacts,
    FreezePublication,
    PreserveLogs,
    RotateKeys,
    NotifyFederationPeers,
}

impl ContainmentAction {
    pub fn as_str(self) -> &'static str {
        match self {
            ContainmentAction::StopExecutionPool => "stop_execution_pool",
            ContainmentAction::RevokeLeases => "revoke_leases",
            ContainmentAction::RevokeCredentials => "revoke_credentials",
            ContainmentAction::QuarantineArtifacts => "quarantine_artifacts",
            ContainmentAction::FreezePublication => "freeze_publication",
            ContainmentAction::PreserveLogs => "preserve_logs",
            ContainmentAction::RotateKeys => "rotate_keys",
            ContainmentAction::NotifyFederationPeers => "notify_federation_peers",
        }
    }
}

impl fmt::Display for ContainmentAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An action somebody called for. Nothing here carries it out. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainmentRequest {
    pub action: ContainmentAction,
    pub requested_at: u64,
    pub requested_by: String,
}

impl ContainmentRequest {
    pub fn new(action: ContainmentAction, requested_by: impl Into<String>, epoch: u64) -> Self {
        ContainmentRequest {
            action,
            requested_at: epoch,
            requested_by: requested_by.into(),
        }
    }

    pub fn honest_label(&self) -> String {
        format!(
            "{} was requested by {} at epoch {}; this process performs no containment",
            self.action, self.requested_by, self.requested_at
        )
    }
}

/// How much of the dependency graph the lineage query actually covered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "completeness", rename_all = "snake_case")]
pub enum LineageCompleteness {
    /// Every dependent artifact was enumerated.
    Complete,
    /// The query terminated with edges it could not follow.
    Partial { unreachable_edges: usize },
    /// No lineage query ran, or it failed. Not the same as zero dependents.
    Unknown,
}

impl LineageCompleteness {
    pub fn as_str(&self) -> &'static str {
        match self {
            LineageCompleteness::Complete => "complete",
            LineageCompleteness::Partial { .. } => "partial",
            LineageCompleteness::Unknown => "unknown",
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, LineageCompleteness::Complete)
    }
}

impl fmt::Display for LineageCompleteness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What was decided about one dependent result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultDisposition {
    /// Not yet decided. Blocks a containment claim.
    UnderInvestigation,
    /// The number is wrong and has been withdrawn.
    Invalidated,
    /// The number stands only if the run is repeated.
    RequiresReproduction,
    /// Examined and unaffected.
    Cleared,
}

impl ResultDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            ResultDisposition::UnderInvestigation => "under_investigation",
            ResultDisposition::Invalidated => "invalidated",
            ResultDisposition::RequiresReproduction => "requires_reproduction",
            ResultDisposition::Cleared => "cleared",
        }
    }

    pub fn is_resolved(self) -> bool {
        !matches!(self, ResultDisposition::UnderInvestigation)
    }
}

impl fmt::Display for ResultDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which published results depend on the compromised thing, and how sure we are that this is all
/// of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlastRadius {
    pub completeness: LineageCompleteness,
    pub dispositions: BTreeMap<String, ResultDisposition>,
}

impl BlastRadius {
    /// A radius from a lineage query that ran to completion.
    pub fn complete() -> Self {
        BlastRadius {
            completeness: LineageCompleteness::Complete,
            dispositions: BTreeMap::new(),
        }
    }

    /// A radius nobody computed. The default, because it is the honest starting state.
    pub fn unknown() -> Self {
        BlastRadius {
            completeness: LineageCompleteness::Unknown,
            dispositions: BTreeMap::new(),
        }
    }

    pub fn partial(unreachable_edges: usize) -> Self {
        BlastRadius {
            completeness: LineageCompleteness::Partial { unreachable_edges },
            dispositions: BTreeMap::new(),
        }
    }

    pub fn with(mut self, result: impl Into<String>, disposition: ResultDisposition) -> Self {
        self.dispositions.insert(result.into(), disposition);
        self
    }

    pub fn dispose(&mut self, result: impl Into<String>, disposition: ResultDisposition) {
        self.dispositions.insert(result.into(), disposition);
    }

    pub fn unresolved(&self) -> Vec<&str> {
        self.dispositions
            .iter()
            .filter(|(_, disposition)| !disposition.is_resolved())
            .map(|(result, _)| result.as_str())
            .collect()
    }
}

impl Default for BlastRadius {
    fn default() -> Self {
        BlastRadius::unknown()
    }
}

/// One dated entry in the forensic timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub epoch: u64,
    pub actor: String,
    pub event: String,
}

/// An append-only, epoch-ordered forensic timeline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timeline {
    entries: Vec<TimelineEntry>,
}

impl Timeline {
    pub fn push(
        &mut self,
        epoch: u64,
        actor: impl Into<String>,
        event: impl Into<String>,
    ) -> Result<(), SafetyError> {
        let actor = actor.into();
        if let Some(last) = self.entries.last() {
            if epoch < last.epoch {
                return Err(SafetyError::EpochNotAdvancing {
                    subject: actor,
                    previous: last.epoch,
                    epoch,
                });
            }
        }
        self.entries.push(TimelineEntry {
            epoch,
            actor,
            event: event.into(),
        });
        Ok(())
    }

    pub fn entries(&self) -> &[TimelineEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// A containment claim that passed the gate.
///
/// Fields are private and there is no `Deserialize`, so [`Incident::report_contained`] is the only
/// way a value of this type comes into existence. That is what makes "contained" a claim the type
/// system underwrites rather than a boolean somebody set — there is no struct literal, and a stored
/// report cannot be read back into a fresh one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContainmentReport {
    incident: String,
    class: IncidentClass,
    results_examined: usize,
    results_invalidated: usize,
    requests_issued: usize,
    caveat: String,
}

impl ContainmentReport {
    pub fn incident(&self) -> &str {
        &self.incident
    }

    pub fn class(&self) -> IncidentClass {
        self.class
    }

    pub fn results_examined(&self) -> usize {
        self.results_examined
    }

    pub fn results_invalidated(&self) -> usize {
        self.results_invalidated
    }

    pub fn requests_issued(&self) -> usize {
        self.requests_issued
    }

    /// Stated because 13.23 requires uncertainty to be stated, and because it is true.
    pub fn caveat(&self) -> &str {
        &self.caveat
    }
}

/// One incident.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incident {
    pub id: String,
    pub class: IncidentClass,
    pub opened_at: u64,
    pub requests: Vec<ContainmentRequest>,
    pub blast_radius: BlastRadius,
    pub timeline: Timeline,
}

impl Incident {
    pub fn open(id: impl Into<String>, class: IncidentClass, epoch: u64) -> Self {
        Incident {
            id: id.into(),
            class,
            opened_at: epoch,
            requests: Vec::new(),
            blast_radius: BlastRadius::unknown(),
            timeline: Timeline::default(),
        }
    }

    pub fn requesting(mut self, request: ContainmentRequest) -> Self {
        self.requests.push(request);
        self
    }

    pub fn with_blast_radius(mut self, radius: BlastRadius) -> Self {
        self.blast_radius = radius;
        self
    }

    /// The gate. See the module docs for why both conditions are required.
    pub fn report_contained(&self) -> Result<ContainmentReport, SafetyError> {
        let unresolved = self.blast_radius.unresolved().len();
        if !self.blast_radius.completeness.is_complete() || unresolved > 0 {
            return Err(SafetyError::ContainmentOverclaimed {
                incident: self.id.clone(),
                completeness: self.blast_radius.completeness.to_string(),
                unresolved,
            });
        }
        if self.class.taints_results() && self.blast_radius.dispositions.is_empty() {
            return Err(SafetyError::ContainmentOverclaimed {
                incident: self.id.clone(),
                completeness: "complete-but-empty".into(),
                unresolved: 0,
            });
        }
        let results_invalidated = self
            .blast_radius
            .dispositions
            .values()
            .filter(|disposition| **disposition == ResultDisposition::Invalidated)
            .count();
        Ok(ContainmentReport {
            incident: self.id.clone(),
            class: self.class,
            results_examined: self.blast_radius.dispositions.len(),
            results_invalidated,
            requests_issued: self.requests.len(),
            caveat: "containment actions were requested, not observed; this process performs none"
                .into(),
        })
    }

    /// The containment actions 13.23 lists that nobody has requested for this incident.
    pub fn unrequested_actions(&self, expected: &[ContainmentAction]) -> Vec<ContainmentAction> {
        expected
            .iter()
            .copied()
            .filter(|action| !self.requests.iter().any(|r| r.action == *action))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_incident_with_an_unknown_blast_radius_cannot_be_reported_contained() {
        let incident = Incident::open("I-1", IncidentClass::CompromisedKey, 3);
        let error = incident
            .report_contained()
            .expect_err("nobody ran the lineage query");
        assert!(matches!(error, SafetyError::ContainmentOverclaimed { .. }));
        assert!(error.to_string().contains("unknown"), "{error}");
    }

    #[test]
    fn a_partial_lineage_query_blocks_containment_however_few_edges_it_missed() {
        let incident = Incident::open("I-2", IncidentClass::CompromisedKey, 1)
            .with_blast_radius(BlastRadius::partial(1).with("r-1", ResultDisposition::Cleared));
        assert!(incident.report_contained().is_err());
    }

    #[test]
    fn one_result_still_under_investigation_blocks_containment() {
        let incident = Incident::open("I-3", IncidentClass::HiddenHoldoutLeak, 1)
            .with_blast_radius(
                BlastRadius::complete()
                    .with("r-1", ResultDisposition::Invalidated)
                    .with("r-2", ResultDisposition::UnderInvestigation),
            );
        let error = incident.report_contained().expect_err("one is still open");
        assert!(error.to_string().contains("1 dependent result"), "{error}");
    }

    #[test]
    fn a_result_tainting_incident_with_an_empty_radius_is_refused_as_suspiciously_clean() {
        let incident = Incident::open("I-4", IncidentClass::HiddenHoldoutLeak, 1)
            .with_blast_radius(BlastRadius::complete());
        assert!(
            incident.report_contained().is_err(),
            "a holdout leak that touched zero results is a lineage query that found nothing"
        );
    }

    #[test]
    fn a_non_tainting_incident_with_an_empty_complete_radius_may_be_contained() {
        let incident = Incident::open("I-5", IncidentClass::DependencyVulnerability, 1)
            .with_blast_radius(BlastRadius::complete());
        assert!(!IncidentClass::DependencyVulnerability.taints_results());
        assert!(incident.report_contained().is_ok());
    }

    #[test]
    fn a_containment_report_carries_the_caveat_that_nothing_was_performed() {
        let incident = Incident::open("I-6", IncidentClass::BenchmarkExploit, 1)
            .requesting(ContainmentRequest::new(
                ContainmentAction::FreezePublication,
                "operator:bo",
                2,
            ))
            .with_blast_radius(
                BlastRadius::complete()
                    .with("r-1", ResultDisposition::Invalidated)
                    .with("r-2", ResultDisposition::RequiresReproduction),
            );
        let report = incident.report_contained().expect("everything is resolved");
        assert_eq!(report.results_examined, 2);
        assert_eq!(report.results_invalidated, 1);
        assert_eq!(report.requests_issued, 1);
        assert!(report.caveat.contains("requested, not observed"));
    }

    #[test]
    fn a_containment_request_says_it_was_requested_rather_than_done() {
        let request = ContainmentRequest::new(ContainmentAction::StopExecutionPool, "sre", 4);
        assert!(request
            .honest_label()
            .contains("this process performs no containment"));
    }

    #[test]
    fn the_forensic_timeline_refuses_an_entry_that_moves_backwards() {
        let mut timeline = Timeline::default();
        timeline
            .push(5, "sre", "pool stop requested")
            .expect("first");
        timeline
            .push(5, "sre", "logs preserved")
            .expect("same epoch");
        let error = timeline
            .push(4, "sre", "backdated note")
            .expect_err("a compromised host does not get to reorder its own history");
        assert!(matches!(error, SafetyError::EpochNotAdvancing { .. }));
        assert_eq!(timeline.len(), 2);
    }

    #[test]
    fn expected_containment_actions_nobody_requested_are_listed() {
        let incident = Incident::open("I-7", IncidentClass::SandboxEscape, 1).requesting(
            ContainmentRequest::new(ContainmentAction::StopExecutionPool, "sre", 1),
        );
        let missing = incident.unrequested_actions(&[
            ContainmentAction::StopExecutionPool,
            ContainmentAction::RevokeCredentials,
            ContainmentAction::PreserveLogs,
        ]);
        assert_eq!(
            missing,
            vec![
                ContainmentAction::RevokeCredentials,
                ContainmentAction::PreserveLogs
            ]
        );
    }

    #[test]
    fn an_unknown_blast_radius_is_the_default_rather_than_an_empty_complete_one() {
        assert_eq!(
            BlastRadius::default().completeness,
            LineageCompleteness::Unknown
        );
        assert!(!BlastRadius::default().completeness.is_complete());
    }

    #[test]
    fn a_disposition_can_be_changed_and_unblocks_containment_once_resolved() {
        let mut incident = Incident::open("I-8", IncidentClass::EvaluatorTampering, 1)
            .with_blast_radius(
                BlastRadius::complete().with("r-1", ResultDisposition::UnderInvestigation),
            );
        assert!(incident.report_contained().is_err());
        incident
            .blast_radius
            .dispose("r-1", ResultDisposition::RequiresReproduction);
        assert!(incident.report_contained().is_ok());
    }
}
