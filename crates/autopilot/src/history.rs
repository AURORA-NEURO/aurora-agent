//! The drive's memory: the base mission and every dispatch attempt, with digests.
//!
//! History is the planner's only input besides the grant, so it is built through validating
//! constructors: an [`AttemptRecord`] cannot exist without a mission report that parses and
//! matches its dispatched plan, a retained invalid response, or an explicit transport-failure
//! note, and a [`DriveHistory`] cannot exist without a base mission that satisfies the mission
//! contract's own deserialization. An invalid response is retained because the dispatch may
//! already have had effects; it is never retried.
//!
//! An undelivered attempt — the dispatcher returned an error instead of a report — still counts
//! against the grant's budget. The dispatch was attempted and side effects may have run; a
//! budget that only counted acknowledged dispatches would let a flaky transport mint free
//! attempts.

use crate::error::AutopilotError;
use bioprism_devplat::{
    mission_claim_lineage_with_review, plan_mission, MissionReport, MissionRequest,
    MissionStepResult, MISSION_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde_json::Value;
use std::collections::BTreeSet;

/// Check that a parsed report belongs to the exact request this attempt dispatched.
///
/// A syntactically valid report can still describe another mission, or contain ambiguous rows
/// that make `step_result()` choose one of several conflicting outcomes. Those are invalid
/// receipts and must stop the drive as outcome-unknown; they are not ordinary tool failures that
/// the grant may authorize for retry. Missing rows remain admissible here because the planner
/// represents them explicitly as unknown per-step evidence.
fn validate_report_against_mission(
    mission: &MissionRequest,
    report: &MissionReport,
) -> Result<(), String> {
    if report.schema_version != MISSION_SCHEMA_VERSION {
        return Err(format!(
            "mission report schema_version must be {MISSION_SCHEMA_VERSION:?}"
        ));
    }
    let expected_plan = plan_mission(mission)
        .map_err(|error| format!("dispatched mission no longer validates: {error}"))?;
    if report.plan != expected_plan {
        return Err("mission report plan does not match the exact dispatched mission plan".into());
    }
    if report.claim_requests != mission.claim_requests {
        return Err(
            "mission report claim requests do not match the exact dispatched mission".into(),
        );
    }
    if report.evaluator_review != mission.evaluator_review {
        return Err(
            "mission report evaluator review does not match the exact dispatched mission".into(),
        );
    }
    let expected_execution = if mission.policy.execute {
        "executed"
    } else {
        "planned"
    };
    if report.execution != expected_execution {
        return Err(format!(
            "mission report execution posture {:?} does not match dispatched posture {:?}",
            report.execution, expected_execution
        ));
    }

    let planned_steps = expected_plan
        .steps
        .iter()
        .map(|step| (step.id.as_str(), step))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut result_ids = BTreeSet::new();
    for result in &report.results {
        if !result_ids.insert(result.id.as_str()) {
            return Err(format!(
                "mission report contains duplicate result rows for step {:?}",
                result.id
            ));
        }
        let Some(step) = planned_steps.get(result.id.as_str()) else {
            return Err(format!(
                "mission report contains a result for undispatched step {:?}",
                result.id
            ));
        };
        if result.tool != step.tool || result.required != step.required {
            return Err(format!(
                "mission report result identity for step {:?} does not match its dispatched tool and required posture",
                result.id
            ));
        }
    }
    let expected_claim_lineage = mission_claim_lineage_with_review(
        &mission.claim_requests,
        &report.results,
        mission.evaluator_review.as_ref(),
    );
    if report.claim_lineage != expected_claim_lineage {
        return Err(
            "mission report claim lineage does not match its dispatched claims and results".into(),
        );
    }

    // Aggregate counts are part of the mission receipt, not advisory display fields. A report
    // whose per-step rows contradict its summary cannot safely drive planning or success checks.
    // Missing rows remain admissible above as explicit unknown evidence; these totals are checked
    // against the rows that were actually returned.
    for (status, reported) in [
        ("succeeded", report.succeeded),
        ("refused", report.refused),
        ("blocked", report.blocked),
        ("cancelled", report.cancelled),
    ] {
        let observed = report
            .results
            .iter()
            .filter(|result| result.status == status)
            .count();
        if observed != reported {
            return Err(format!(
                "mission reports {reported} `{status}` results but contains {observed}"
            ));
        }
    }

    let required_step_count = planned_steps.values().filter(|step| step.required).count();
    if report.required_failures > required_step_count {
        return Err(format!(
            "mission reports {} required failures but the dispatched plan has only {required_step_count} required steps",
            report.required_failures
        ));
    }
    let observed_required_failures = report
        .results
        .iter()
        .filter(|result| result.required && matches!(result.status.as_str(), "refused" | "blocked"))
        .count();
    if report.required_failures < observed_required_failures {
        return Err(format!(
            "mission reports {} required failures but contains {observed_required_failures} required refused or blocked results",
            report.required_failures
        ));
    }
    if report.results.len() == planned_steps.len()
        && report.required_failures != observed_required_failures
    {
        return Err(format!(
            "mission reports {} required failures but contains {observed_required_failures} required refused or blocked results",
            report.required_failures
        ));
    }

    // Successful results necessarily contribute their measured bytes to the mission total. A
    // retained wire envelope on a refused result also proves that the nested response was
    // accepted into the total; wire-less refused rows can instead represent an over-budget
    // response whose bytes were deliberately excluded. The lower bound remains valid when rows
    // are missing or raw wires were not retained.
    let minimum_returned_bytes = report
        .results
        .iter()
        .filter(|result| result.status == "succeeded" || result.wire.is_some())
        .map(|result| result.bytes)
        .try_fold(0usize, usize::checked_add);
    let Some(minimum_returned_bytes) = minimum_returned_bytes else {
        return Err("mission result byte totals overflow the supported range".into());
    };
    if report.returned_bytes < minimum_returned_bytes {
        return Err(format!(
            "mission reports {} returned bytes but retained successful and wire-backed results account for at least {minimum_returned_bytes}",
            report.returned_bytes
        ));
    }
    Ok(())
}

/// Whether an attempt dispatched the whole instantiated mission or a repair subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptKind {
    Full,
    Repair,
}

impl AttemptKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AttemptKind::Full => "full",
            AttemptKind::Repair => "repair",
        }
    }

    /// The scope a reconciliation attached to this attempt can honestly claim.
    pub fn reconciliation_scope(self) -> &'static str {
        match self {
            AttemptKind::Full => "full_plan",
            AttemptKind::Repair => "repair_subset",
        }
    }
}

fn digest_of(value: &Value) -> Result<String, AutopilotError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| AutopilotError::Canonicalisation {
            reason: error.to_string(),
        })
}

fn reconciliation_payload(record: &Value) -> &Value {
    record
        .get("canonical_record")
        .filter(|value| value.is_object())
        .unwrap_or(record)
}

pub(crate) fn reconciliation_digest_verified(record: &Value, report: Option<&Value>) -> bool {
    let payload = reconciliation_payload(record);
    let Some(claimed) = payload.get("reconciliation_digest").and_then(Value::as_str) else {
        return false;
    };
    if ContentHash::parse(claimed.to_string()).is_err() {
        return false;
    }

    let mut unsigned = payload.clone();
    let Some(object) = unsigned.as_object_mut() else {
        return false;
    };
    object.remove("reconciliation_digest");
    if !ContentHash::of_value(&unsigned).is_ok_and(|computed| computed.to_string() == claimed) {
        return false;
    }

    let Ok(validated_import) =
        bioprism_devplat::DomainWorkflowReconciliationRegistry::new().import(payload)
    else {
        return false;
    };
    if validated_import
        .get("reconciliation_digest")
        .and_then(Value::as_str)
        != Some(claimed)
    {
        return false;
    }
    if payload == record {
        return true;
    }

    // The MCP boundary embeds the canonical record inside its operator projection. Bind the
    // duplicated summary and import receipt back to a record accepted by the registry contract
    // instead of trusting a marker supplied by the dispatcher.
    record.get("present") == Some(&Value::Bool(true))
        && record.get("automatic") == Some(&Value::Bool(true))
        && record.get("reconciliation_digest") == Some(&Value::String(claimed.into()))
        && record.get("completion") == payload.get("completion")
        && record.get("integrity") == payload.get("integrity")
        && record.get("workflow_id") == payload.get("workflow_id")
        && record.get("mission_id") == payload.get("mission_id")
        && record
            .pointer("/registry_import/ok")
            .and_then(Value::as_bool)
            == Some(true)
        && record
            .pointer("/registry_import/schema")
            .and_then(Value::as_str)
            == Some(bioprism_devplat::DOMAIN_WORKFLOW_RECONCILIATION_IMPORT_SCHEMA_VERSION)
        && record
            .pointer("/registry_import/workflow")
            .and_then(Value::as_str)
            == Some("domain_workflow_reconciliation_import")
        && record
            .pointer("/registry_import/execution")
            .and_then(Value::as_str)
            == Some("not_started")
        && record
            .pointer("/registry_import/reconciliation_digest")
            .and_then(Value::as_str)
            == Some(claimed)
        && report.is_none_or(|report| report.get("workflow_reconciliation") == Some(record))
}

/// One dispatch and what came back, validated and digested at construction.
#[derive(Debug, Clone)]
pub struct AttemptRecord {
    kind: AttemptKind,
    mission: Value,
    parsed_mission: MissionRequest,
    mission_digest: String,
    report: Option<Value>,
    parsed_report: Option<MissionReport>,
    report_digest: Option<String>,
    report_validation_error: Option<String>,
    reconciliation: Option<Value>,
    reconciliation_note: Option<String>,
    dispatch_error: Option<String>,
}

impl AttemptRecord {
    /// Record a dispatch that returned a value, whether or not it satisfied the mission-report
    /// contract. An invalid value stays in private history with its digest and validation error
    /// so the planner can stop without losing evidence that side effects may have run.
    ///
    /// `reconciliation` is the record the shell obtained for this attempt — the auto-attached
    /// `workflow_reconciliation` projection or a full `reconcile_domain_workflow` output. Full
    /// records and MCP projections with an embedded canonical record are verified by recomputing
    /// the digest; summary-only projections cannot support success.
    /// `reconciliation_note` states why one is absent, so "not attempted" and "attempted and
    /// failed" never share the silent representation `None`.
    pub fn delivered(
        kind: AttemptKind,
        mission: Value,
        report: Value,
        reconciliation: Option<Value>,
        reconciliation_note: Option<String>,
    ) -> Result<Self, AutopilotError> {
        let parsed_mission: MissionRequest =
            serde_json::from_value(mission.clone()).map_err(|error| {
                AutopilotError::InvalidMission {
                    reason: error.to_string(),
                }
            })?;
        let (parsed_report, report_validation_error) =
            match serde_json::from_value::<MissionReport>(report.clone()) {
                Ok(parsed) => match validate_report_against_mission(&parsed_mission, &parsed) {
                    Ok(()) => (Some(parsed), None),
                    Err(error) => (None, Some(error)),
                },
                Err(error) => (None, Some(error.to_string())),
            };
        let mission_digest = digest_of(&mission)?;
        let report_digest = digest_of(&report)?;
        let (reconciliation, reconciliation_note) = if report_validation_error.is_some() {
            (
                None,
                Some("no reconciliation: the returned mission report failed validation".into()),
            )
        } else {
            (reconciliation, reconciliation_note)
        };
        Ok(AttemptRecord {
            kind,
            mission,
            parsed_mission,
            mission_digest,
            report: Some(report),
            parsed_report,
            report_digest: Some(report_digest),
            report_validation_error,
            reconciliation,
            reconciliation_note,
            dispatch_error: None,
        })
    }

    /// Record an attempt whose dispatch returned no report, including a caught dispatcher panic:
    /// the mission outcome is unknown at mission level, and the drive will stop rather than
    /// re-send blind.
    pub fn undelivered(
        kind: AttemptKind,
        mission: Value,
        dispatch_error: String,
    ) -> Result<Self, AutopilotError> {
        let parsed_mission: MissionRequest =
            serde_json::from_value(mission.clone()).map_err(|error| {
                AutopilotError::InvalidMission {
                    reason: error.to_string(),
                }
            })?;
        let mission_digest = digest_of(&mission)?;
        Ok(AttemptRecord {
            kind,
            mission,
            parsed_mission,
            mission_digest,
            report: None,
            parsed_report: None,
            report_digest: None,
            report_validation_error: None,
            reconciliation: None,
            reconciliation_note: Some(
                "no reconciliation: the dispatch returned no mission report".into(),
            ),
            dispatch_error: Some(dispatch_error),
        })
    }

    pub fn kind(&self) -> AttemptKind {
        self.kind
    }

    pub fn mission(&self) -> &Value {
        &self.mission
    }

    pub fn parsed_mission(&self) -> &MissionRequest {
        &self.parsed_mission
    }

    pub fn mission_digest(&self) -> &str {
        &self.mission_digest
    }

    pub fn report(&self) -> Option<&Value> {
        self.report.as_ref()
    }

    pub fn parsed_report(&self) -> Option<&MissionReport> {
        self.parsed_report.as_ref()
    }

    pub fn report_digest(&self) -> Option<&str> {
        self.report_digest.as_deref()
    }

    /// The dispatch returned a value, but parsing or attempt-binding validation failed.
    pub fn report_validation_error(&self) -> Option<&str> {
        self.report_validation_error.as_deref()
    }

    pub(crate) fn with_reconciliation(
        mut self,
        reconciliation: Option<Value>,
        reconciliation_note: Option<String>,
    ) -> Self {
        if self.report_validation_error.is_none() {
            self.reconciliation = reconciliation;
            self.reconciliation_note = reconciliation_note;
        }
        self
    }

    pub fn reconciliation(&self) -> Option<&Value> {
        self.reconciliation.as_ref()
    }

    pub fn reconciliation_note(&self) -> Option<&str> {
        self.reconciliation_note.as_deref()
    }

    pub fn dispatch_error(&self) -> Option<&str> {
        self.dispatch_error.as_deref()
    }

    /// The step ids this attempt dispatched, in the mission's own order.
    pub fn dispatched_step_ids(&self) -> Vec<String> {
        self.parsed_mission
            .steps
            .iter()
            .map(|step| step.id.clone())
            .collect()
    }

    /// The recorded result for one step, when the report holds one.
    pub fn step_result(&self, step_id: &str) -> Option<&MissionStepResult> {
        self.parsed_report
            .as_ref()?
            .results
            .iter()
            .find(|result| result.id == step_id)
    }

    /// Completion, integrity, claimed digest, and verification posture for this attempt.
    pub fn reconciliation_summary(&self) -> Option<(String, bool, Option<String>, bool)> {
        let record = self.reconciliation.as_ref()?;
        let status = record
            .pointer("/completion/status")
            .and_then(Value::as_str)?
            .to_string();
        let integrity_valid = record
            .pointer("/integrity/valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let digest = record
            .get("reconciliation_digest")
            .and_then(Value::as_str)
            .map(str::to_string);
        let digest_verified = reconciliation_digest_verified(record, self.report.as_ref());
        Some((status, integrity_valid, digest, digest_verified))
    }
}

/// The base mission and the ordered attempts made against it.
#[derive(Debug, Clone)]
pub struct DriveHistory {
    base_mission: Value,
    parsed_base: MissionRequest,
    attempts: Vec<AttemptRecord>,
}

impl DriveHistory {
    /// Validate and hold the mission the drive was asked to complete, exactly as authored. The
    /// grant's policy overrides are applied at dispatch construction, not here, so the retained
    /// base stays byte-faithful to the instantiation it came from.
    pub fn new(base_mission: Value) -> Result<Self, AutopilotError> {
        let parsed_base: MissionRequest =
            serde_json::from_value(base_mission.clone()).map_err(|error| {
                AutopilotError::InvalidMission {
                    reason: error.to_string(),
                }
            })?;
        if parsed_base.steps.is_empty() {
            return Err(AutopilotError::InvalidMission {
                reason: "mission has no steps".into(),
            });
        }
        Ok(DriveHistory {
            base_mission,
            parsed_base,
            attempts: Vec::new(),
        })
    }

    /// Rebuild a history from caller-owned rehydrated attempts after a process restart.
    ///
    /// The checkpoint layer verifies that these attempts match its digest-only projections; this
    /// constructor deliberately does not accept raw serialized state by itself.
    pub fn from_attempts(
        base_mission: Value,
        attempts: Vec<AttemptRecord>,
    ) -> Result<Self, AutopilotError> {
        let mut history = Self::new(base_mission)?;
        history.attempts = attempts;
        Ok(history)
    }

    pub fn base_mission(&self) -> &Value {
        &self.base_mission
    }

    pub fn parsed_base(&self) -> &MissionRequest {
        &self.parsed_base
    }

    pub fn attempts(&self) -> &[AttemptRecord] {
        &self.attempts
    }

    pub fn latest(&self) -> Option<&AttemptRecord> {
        self.attempts.last()
    }

    /// Dispatches consumed so far, undelivered ones included.
    pub fn dispatches_used(&self) -> usize {
        self.attempts.len()
    }

    pub fn push(&mut self, attempt: AttemptRecord) {
        self.attempts.push(attempt);
    }
}
