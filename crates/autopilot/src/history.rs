//! The drive's memory: the base mission and every dispatch attempt, with digests.
//!
//! History is the planner's only input besides the grant, so it is built through validating
//! constructors: an [`AttemptRecord`] cannot exist without either a parseable mission report or
//! an explicit transport-failure note, and a [`DriveHistory`] cannot exist without a base
//! mission that satisfies the mission contract's own deserialization. Holding the value is
//! holding the proof.
//!
//! An undelivered attempt — the dispatcher returned an error instead of a report — still counts
//! against the grant's budget. The dispatch was attempted and side effects may have run; a
//! budget that only counted acknowledged dispatches would let a flaky transport mint free
//! attempts.

use crate::error::AutopilotError;
use bioprism_devplat::{MissionReport, MissionRequest, MissionStepResult};
use bioprism_ids::ContentHash;
use serde_json::Value;

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
    reconciliation: Option<Value>,
    reconciliation_note: Option<String>,
    dispatch_error: Option<String>,
}

impl AttemptRecord {
    /// Record an attempt whose dispatch returned a mission report.
    ///
    /// `reconciliation` is the record the shell obtained for this attempt — the auto-attached
    /// `workflow_reconciliation` summary or a full `reconcile_domain_workflow` output; both carry
    /// `completion`, `integrity`, and `reconciliation_digest`, which is all the planner reads.
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
        let parsed_report: MissionReport =
            serde_json::from_value(report.clone()).map_err(|error| {
                AutopilotError::InvalidReport {
                    reason: error.to_string(),
                }
            })?;
        let mission_digest = digest_of(&mission)?;
        let report_digest = digest_of(&report)?;
        Ok(AttemptRecord {
            kind,
            mission,
            parsed_mission,
            mission_digest,
            report: Some(report),
            parsed_report: Some(parsed_report),
            report_digest: Some(report_digest),
            reconciliation,
            reconciliation_note,
            dispatch_error: None,
        })
    }

    /// Record an attempt whose dispatch failed at the transport: no report exists, the mission
    /// outcome is unknown at mission level, and the drive will stop rather than re-send blind.
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

    /// Completion status, integrity validity, and digest of this attempt's reconciliation.
    pub fn reconciliation_summary(&self) -> Option<(String, bool, Option<String>)> {
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
        Some((status, integrity_valid, digest))
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
        let parsed_base: MissionRequest = serde_json::from_value(base_mission.clone())
            .map_err(|error| AutopilotError::InvalidMission {
                reason: error.to_string(),
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
