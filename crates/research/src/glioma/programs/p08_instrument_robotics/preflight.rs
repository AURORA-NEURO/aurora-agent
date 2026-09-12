//! Deterministic instrument and robotics preflight for preclinical glioma protocols.
//!
//! This module turns a caller-owned list of physical actions into an ordered, bounded plan. It
//! combines robust calibration, live interlock telemetry, typed parameter checks, and explicit
//! operator authorization. The result is an admission decision for a local gateway; this crate
//! never contacts hardware, consumes material, or treats admission as scientific evidence.

use super::calibration::{CalibrationDisposition, InstrumentCalibration};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentPreflight1@1";
pub const MAX_ACTIONS: usize = 1_024;
pub const MAX_PARAMETERS_PER_ACTION: usize = 64;
pub const MAX_TICK: u64 = 10_000_000_000;
pub const MAX_RISK_MILLI: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentOperation {
    AcquireImage,
    AddReagent,
    Aspirate,
    Dispense,
    Incubate,
    Wash,
    Transfer,
    Sequence,
    MoveStage,
    Shutdown,
}

impl InstrumentOperation {
    fn requires_volume(self) -> bool {
        matches!(
            self,
            Self::AddReagent | Self::Aspirate | Self::Dispense | Self::Wash | Self::Transfer
        )
    }

    fn inherently_destructive(self) -> bool {
        matches!(
            self,
            Self::AddReagent
                | Self::Aspirate
                | Self::Dispense
                | Self::Wash
                | Self::Transfer
                | Self::Sequence
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentParameter {
    pub name: String,
    pub value_milli: i64,
    pub unit: String,
    pub minimum_milli: Option<i64>,
    pub maximum_milli: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentAction {
    pub action_id: String,
    pub instrument_id: String,
    pub operation: InstrumentOperation,
    pub model_system: GliomaModelSystem,
    pub requested_start_tick: u64,
    pub duration_ticks: u64,
    pub risk_milli: u64,
    pub requires_operator: bool,
    pub output_schema: String,
    pub parameters: Vec<InstrumentParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentInterlockSnapshot {
    pub observed_tick: u64,
    pub emergency_stop_clear: bool,
    pub guard_closed: bool,
    pub deck_clear: bool,
    pub consumables_available: bool,
    pub waste_capacity_milli: u64,
    pub temperature_milli: Option<i64>,
    pub minimum_temperature_milli: Option<i64>,
    pub maximum_temperature_milli: Option<i64>,
    pub calibration_valid_until_tick: u64,
    pub calibration_sequence_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentAuthorization {
    pub authorization_id: String,
    pub operator_id: String,
    pub instrument_scope: String,
    pub approval_digest: ContentHash,
    pub issued_tick: u64,
    pub expires_tick: u64,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentPreflightRequest {
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub actions: Vec<InstrumentAction>,
    pub calibration: InstrumentCalibration,
    pub interlocks: InstrumentInterlockSnapshot,
    pub authorization: InstrumentAuthorization,
    pub current_tick: u64,
    pub maximum_total_risk_milli: u64,
    pub maximum_duration_ticks: u64,
    pub minimum_waste_capacity_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentActionDisposition {
    Admitted,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionDecision {
    pub action_id: String,
    pub disposition: InstrumentActionDisposition,
    pub scheduled_start_tick: Option<u64>,
    pub scheduled_end_tick: Option<u64>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentPreflightDisposition {
    Admitted,
    Blocked,
    Unresolved,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentPreflightPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub authorization_id: String,
    pub action_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub decisions: Vec<InstrumentActionDecision>,
    pub total_risk_milli: u64,
    pub total_duration_ticks: u64,
    pub required_interlocks: Vec<String>,
    pub compensation_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub dispatch_permitted: bool,
    pub disposition: InstrumentPreflightDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentPreflightError {
    #[error("instrument preflight request is invalid: {0}")]
    InvalidRequest(String),
    #[error("instrument calibration is invalid: {0}")]
    InvalidCalibration(String),
    #[error("instrument preflight output is invalid: {0}")]
    InvalidOutput(String),
    #[error("instrument preflight digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &InstrumentPreflightPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "instrument_id": plan.instrument_id,
        "model_system": plan.model_system,
        "authorization_id": plan.authorization_id,
        "action_order": plan.action_order,
        "admitted_order": plan.admitted_order,
        "blocked_order": plan.blocked_order,
        "unresolved_order": plan.unresolved_order,
        "decisions": plan.decisions,
        "total_risk_milli": plan.total_risk_milli,
        "total_duration_ticks": plan.total_duration_ticks,
        "required_interlocks": plan.required_interlocks,
        "compensation_order": plan.compensation_order,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "dispatch_permitted": plan.dispatch_permitted,
        "disposition": plan.disposition,
    })
}

impl InstrumentPreflightPlan {
    pub fn validate(&self) -> Result<(), InstrumentPreflightError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.authorization_id.trim().is_empty()
            || self.action_order.is_empty()
            || self.action_order.windows(2).any(|pair| pair[0] == pair[1])
            || !canonical(&self.admitted_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.required_interlocks)
            || !canonical(&self.compensation_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.decisions.len() != self.action_order.len()
            || self.decisions.iter().any(|decision| {
                decision.action_id.trim().is_empty()
                    || (decision.disposition != InstrumentActionDisposition::Admitted
                        && decision.reasons.is_empty())
                    || decision.reasons.windows(2).any(|pair| pair[0] >= pair[1])
                    || matches!(decision.disposition, InstrumentActionDisposition::Admitted)
                        != decision.scheduled_start_tick.is_some()
                    || matches!(decision.disposition, InstrumentActionDisposition::Admitted)
                        != decision.scheduled_end_tick.is_some()
            })
            || self
                .required_interlocks
                .iter()
                .any(|item| item.trim().is_empty())
            || self
                .compensation_order
                .iter()
                .any(|item| item.trim().is_empty())
            || self
                .negative_evidence
                .iter()
                .any(|item| item.trim().is_empty())
            || self.uncertainty.iter().any(|item| item.trim().is_empty())
        {
            return Err(InstrumentPreflightError::InvalidOutput(
                "identity, decisions, ordering, or gate explanation is invalid".into(),
            ));
        }
        let action_ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let decision_ids = self
            .decisions
            .iter()
            .map(|decision| decision.action_id.clone())
            .collect::<BTreeSet<_>>();
        if action_ids != decision_ids {
            return Err(InstrumentPreflightError::InvalidOutput(
                "action and decision identities do not reconcile".into(),
            ));
        }
        for (order, disposition) in [
            (&self.admitted_order, InstrumentActionDisposition::Admitted),
            (&self.blocked_order, InstrumentActionDisposition::Blocked),
            (
                &self.unresolved_order,
                InstrumentActionDisposition::Unresolved,
            ),
        ] {
            let expected = self
                .decisions
                .iter()
                .filter(|decision| decision.disposition == disposition)
                .map(|decision| decision.action_id.clone())
                .collect::<BTreeSet<_>>();
            if order.iter().cloned().collect::<BTreeSet<_>>() != expected {
                return Err(InstrumentPreflightError::InvalidOutput(
                    "action disposition partitions do not reconcile".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentPreflightError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentPreflightError::InvalidOutput(
                "preflight digest is not bound to the ordered plan".into(),
            ));
        }
        Ok(())
    }
}

fn action_parameter<'a>(
    action: &'a InstrumentAction,
    name: &str,
) -> Option<&'a InstrumentParameter> {
    action
        .parameters
        .iter()
        .find(|parameter| parameter.name == name)
}

fn validate_parameter(parameter: &InstrumentParameter) -> Option<String> {
    if parameter.name.trim().is_empty() || parameter.unit.trim().is_empty() {
        return Some("parameter name and unit are required".into());
    }
    if let Some(minimum) = parameter.minimum_milli {
        if parameter.value_milli < minimum {
            return Some(format!(
                "parameter {} is below its declared minimum",
                parameter.name
            ));
        }
    }
    if let Some(maximum) = parameter.maximum_milli {
        if parameter.value_milli > maximum {
            return Some(format!(
                "parameter {} exceeds its declared maximum",
                parameter.name
            ));
        }
    }
    None
}

fn action_reasons(
    action: &InstrumentAction,
    request: &InstrumentPreflightRequest,
    schedule_tick: u64,
    end_tick: u64,
) -> (Vec<String>, Vec<String>) {
    let mut blocked = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    if action.instrument_id != request.instrument_id {
        blocked.insert("action-instrument-mismatch".into());
    }
    if action.model_system != request.model_system {
        blocked.insert("action-model-system-mismatch".into());
    }
    if action.duration_ticks == 0 {
        blocked.insert("action-duration-is-zero".into());
    }
    if action.risk_milli > MAX_RISK_MILLI {
        blocked.insert("action-risk-exceeds-bound".into());
    }
    if action.operation.inherently_destructive() && !action.requires_operator {
        blocked.insert("destructive-action-requires-operator".into());
    }
    if action.output_schema.trim().is_empty() {
        blocked.insert("action-output-schema-missing".into());
    }
    if action.operation.requires_volume() {
        match action_parameter(action, "volume_microliter") {
            Some(parameter) if parameter.value_milli > 0 => {}
            Some(_) => {
                blocked.insert("volume-must-be-positive".into());
            }
            None => {
                blocked.insert("volume-parameter-missing".into());
            }
        };
    }
    for parameter in &action.parameters {
        if let Some(reason) = validate_parameter(parameter) {
            blocked.insert(reason);
        }
    }
    if action.requested_start_tick < request.current_tick {
        unresolved.insert("requested-start-is-in-the-past".into());
    }
    if end_tick
        > request
            .current_tick
            .saturating_add(request.maximum_duration_ticks)
    {
        unresolved.insert("action-exceeds-duration-budget".into());
    }
    if schedule_tick > MAX_TICK || end_tick > MAX_TICK {
        blocked.insert("action-tick-exceeds-bound".into());
    }
    (
        blocked.into_iter().collect(),
        unresolved.into_iter().collect(),
    )
}

fn validate_request(request: &InstrumentPreflightRequest) -> Result<(), InstrumentPreflightError> {
    if request.objective.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.actions.is_empty()
        || request.actions.len() > MAX_ACTIONS
        || request.current_tick > MAX_TICK
        || request.maximum_total_risk_milli == 0
        || request.maximum_total_risk_milli > MAX_ACTIONS as u64 * MAX_RISK_MILLI
        || request.maximum_duration_ticks == 0
        || request.maximum_duration_ticks > MAX_TICK
        || request.minimum_waste_capacity_milli > 1_000_000
    {
        return Err(InstrumentPreflightError::InvalidRequest(
            "objective, instrument, bounded actions, current tick, risk, duration, and waste budget are required".into(),
        ));
    }
    if request.interlocks.observed_tick > MAX_TICK
        || request.interlocks.calibration_valid_until_tick < request.current_tick
        || request.authorization.issued_tick > request.current_tick
        || request.authorization.expires_tick <= request.current_tick
        || request.authorization.expires_tick > MAX_TICK
        || request.authorization.approval_digest.as_str().len() != 64
        || request.authorization.authorization_id.trim().is_empty()
        || request.authorization.operator_id.trim().is_empty()
        || request.authorization.instrument_scope != request.instrument_id
        || request.authorization.revoked
    {
        return Err(InstrumentPreflightError::InvalidRequest(
            "authorization and interlock time bounds, scope, digest, and revocation state are invalid".into(),
        ));
    }
    if request.interlocks.temperature_milli.is_some()
        != request.interlocks.minimum_temperature_milli.is_some()
        || request.interlocks.temperature_milli.is_some()
            != request.interlocks.maximum_temperature_milli.is_some()
    {
        return Err(InstrumentPreflightError::InvalidRequest(
            "temperature telemetry and bounds must be jointly present or absent".into(),
        ));
    }
    if let (Some(minimum), Some(maximum)) = (
        request.interlocks.minimum_temperature_milli,
        request.interlocks.maximum_temperature_milli,
    ) {
        if minimum > maximum {
            return Err(InstrumentPreflightError::InvalidRequest(
                "minimum temperature exceeds maximum temperature".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for action in &request.actions {
        if action.action_id.trim().is_empty()
            || !ids.insert(action.action_id.clone())
            || action.parameters.len() > MAX_PARAMETERS_PER_ACTION
        {
            return Err(InstrumentPreflightError::InvalidRequest(
                "action identities must be unique and parameter counts bounded".into(),
            ));
        }
        let mut parameters = BTreeSet::new();
        for parameter in &action.parameters {
            if !parameters.insert(parameter.name.clone()) {
                return Err(InstrumentPreflightError::InvalidRequest(format!(
                    "action {} repeats a parameter name",
                    action.action_id
                )));
            }
        }
    }
    Ok(())
}

/// Compile a local instrument/robotics admission plan. The caller remains responsible for
/// cryptographic signature verification and for executing the admitted plan through its gateway.
pub fn preflight_glioma_instrument(
    request: &InstrumentPreflightRequest,
) -> Result<InstrumentPreflightPlan, InstrumentPreflightError> {
    validate_request(request)?;
    request
        .calibration
        .validate()
        .map_err(|error| InstrumentPreflightError::InvalidCalibration(error.to_string()))?;
    let mut global_blocked = BTreeSet::new();
    let mut global_unresolved = BTreeSet::new();
    if request.calibration.instrument_id != request.instrument_id
        || request.calibration.model_system != request.model_system
    {
        global_blocked.insert("calibration-scope-mismatch".into());
    }
    if request.calibration.disposition != CalibrationDisposition::Qualified {
        global_blocked.insert(format!(
            "calibration-disposition-is-{:?}",
            request.calibration.disposition
        ));
    }
    if !request.interlocks.emergency_stop_clear {
        global_blocked.insert("emergency-stop-not-clear".into());
    }
    if !request.interlocks.guard_closed {
        global_blocked.insert("instrument-guard-not-closed".into());
    }
    if !request.interlocks.deck_clear {
        global_blocked.insert("instrument-deck-not-clear".into());
    }
    if !request.interlocks.consumables_available {
        global_unresolved.insert("consumable-availability-unconfirmed".into());
    }
    if request.interlocks.waste_capacity_milli < request.minimum_waste_capacity_milli {
        global_blocked.insert("waste-capacity-below-required-floor".into());
    }
    match (
        request.interlocks.temperature_milli,
        request.interlocks.minimum_temperature_milli,
        request.interlocks.maximum_temperature_milli,
    ) {
        (Some(temperature), Some(minimum), Some(_maximum)) if temperature < minimum => {
            global_blocked.insert("temperature-below-operating-window".into());
        }
        (Some(temperature), Some(_), Some(maximum)) if temperature > maximum => {
            global_blocked.insert("temperature-above-operating-window".into());
        }
        (Some(_), Some(_), Some(_)) => {}
        _ => {
            global_unresolved.insert("temperature-window-unmeasured".into());
        }
    }
    if request.interlocks.calibration_sequence_index > request.calibration.points.len() as u32 {
        global_unresolved.insert("calibration-sequence-attestation-is-newer-than-analysis".into());
    }
    if request.interlocks.observed_tick > request.current_tick {
        global_unresolved.insert("interlock-telemetry-is-from-the-future".into());
    }
    let mut ordered = request.actions.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.requested_start_tick
            .cmp(&right.requested_start_tick)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let action_order = ordered
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut decisions = Vec::with_capacity(ordered.len());
    let mut admitted_order = Vec::new();
    let mut blocked_order = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut negative_evidence = global_blocked.clone();
    let mut uncertainty = global_unresolved.clone();
    let mut cursor = request.current_tick;
    let mut total_risk_milli = 0_u64;
    let mut total_duration_ticks = 0_u64;
    let mut admitted_risk_milli = 0_u64;
    let mut admitted_duration_ticks = 0_u64;
    let mut seen_end = BTreeMap::<String, u64>::new();
    for action in ordered {
        let schedule_tick = action.requested_start_tick.max(cursor);
        let end_tick = schedule_tick.saturating_add(action.duration_ticks);
        let (mut blocked, mut unresolved) =
            action_reasons(action, request, schedule_tick, end_tick);
        if action.requested_start_tick < cursor {
            blocked.push("action-overlaps-earlier-serialized-action".into());
        }
        if let Some(previous_end) = seen_end.values().max().copied() {
            if action.requested_start_tick < previous_end {
                blocked.push("requested-actions-overlap-on-single-instrument".into());
            }
        }
        if !global_blocked.is_empty() {
            blocked.extend(global_blocked.iter().cloned());
        }
        if !global_unresolved.is_empty() {
            unresolved.extend(global_unresolved.iter().cloned());
        }
        blocked.sort();
        blocked.dedup();
        unresolved.sort();
        unresolved.dedup();
        let (disposition, reasons, start, end) = if !blocked.is_empty() {
            blocked_order.push(action.action_id.clone());
            (InstrumentActionDisposition::Blocked, blocked, None, None)
        } else if !unresolved.is_empty() {
            unresolved_order.push(action.action_id.clone());
            (
                InstrumentActionDisposition::Unresolved,
                unresolved,
                None,
                None,
            )
        } else if total_risk_milli.saturating_add(action.risk_milli)
            > request.maximum_total_risk_milli
            || total_duration_ticks.saturating_add(action.duration_ticks)
                > request.maximum_duration_ticks
        {
            let mut reasons = Vec::new();
            if total_risk_milli.saturating_add(action.risk_milli) > request.maximum_total_risk_milli
            {
                reasons.push("cumulative-risk-budget-exceeded".into());
            }
            if total_duration_ticks.saturating_add(action.duration_ticks)
                > request.maximum_duration_ticks
            {
                reasons.push("cumulative-duration-budget-exceeded".into());
            }
            blocked_order.push(action.action_id.clone());
            (InstrumentActionDisposition::Blocked, reasons, None, None)
        } else {
            admitted_order.push(action.action_id.clone());
            admitted_risk_milli = admitted_risk_milli.saturating_add(action.risk_milli);
            admitted_duration_ticks = admitted_duration_ticks.saturating_add(action.duration_ticks);
            cursor = end_tick;
            seen_end.insert(action.action_id.clone(), end_tick);
            (
                InstrumentActionDisposition::Admitted,
                Vec::new(),
                Some(schedule_tick),
                Some(end_tick),
            )
        };
        total_risk_milli = total_risk_milli.saturating_add(action.risk_milli);
        total_duration_ticks = total_duration_ticks.saturating_add(action.duration_ticks);
        decisions.push(InstrumentActionDecision {
            action_id: action.action_id.clone(),
            disposition,
            scheduled_start_tick: start,
            scheduled_end_tick: end,
            reasons,
        });
    }
    // Budgets describe the complete requested protocol; admitted totals describe what the gateway
    // may actually dispatch. Keeping both prevents blocked actions from disappearing silently.
    let _ = (admitted_risk_milli, admitted_duration_ticks);
    if !global_blocked.is_empty() {
        negative_evidence.extend(global_blocked.iter().cloned());
    }
    if !global_unresolved.is_empty() {
        uncertainty.extend(global_unresolved.iter().cloned());
    }
    for decision in &decisions {
        if decision.disposition == InstrumentActionDisposition::Blocked {
            negative_evidence.extend(
                decision
                    .reasons
                    .iter()
                    .map(|reason| format!("{}:{reason}", decision.action_id)),
            );
        } else if decision.disposition == InstrumentActionDisposition::Unresolved {
            uncertainty.extend(
                decision
                    .reasons
                    .iter()
                    .map(|reason| format!("{}:{reason}", decision.action_id)),
            );
        }
    }
    let mut required_interlocks = BTreeSet::from([
        "emergency-stop-clear".to_string(),
        "guard-closed".to_string(),
        "deck-clear".to_string(),
        "calibration-qualified".to_string(),
        "operator-authorization-unrevoked".to_string(),
    ]);
    if request.interlocks.temperature_milli.is_some() {
        required_interlocks.insert("temperature-within-window".into());
    }
    let mut compensation_order = admitted_order
        .iter()
        .rev()
        .map(|action_id| format!("compensate:{action_id}"))
        .collect::<Vec<_>>();
    compensation_order.sort();
    let mut negative_evidence = negative_evidence.into_iter().collect::<Vec<_>>();
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if !blocked_order.is_empty() {
        InstrumentPreflightDisposition::Blocked
    } else if !unresolved_order.is_empty() {
        InstrumentPreflightDisposition::Unresolved
    } else if request.calibration.disposition == CalibrationDisposition::Negative {
        InstrumentPreflightDisposition::Negative
    } else {
        InstrumentPreflightDisposition::Admitted
    };
    let dispatch_permitted = disposition == InstrumentPreflightDisposition::Admitted;
    let mut plan = InstrumentPreflightPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        instrument_id: request.instrument_id.clone(),
        model_system: request.model_system,
        authorization_id: request.authorization.authorization_id.clone(),
        action_order,
        admitted_order,
        blocked_order,
        unresolved_order,
        decisions,
        total_risk_milli,
        total_duration_ticks,
        required_interlocks: required_interlocks.into_iter().collect(),
        compensation_order,
        negative_evidence,
        uncertainty,
        dispatch_permitted,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-preflight"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| InstrumentPreflightError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationPoint, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn calibration() -> InstrumentCalibration {
        let points = vec![
            CalibrationPoint {
                run_id: "cal-1".into(),
                sequence_index: 1,
                observed_milli: 500,
                expected_milli: 500,
                residual_milli: 0,
                drift_from_reference_milli: 0,
                robust_z_milli: 0,
            },
            CalibrationPoint {
                run_id: "cal-2".into(),
                sequence_index: 2,
                observed_milli: 502,
                expected_milli: 500,
                residual_milli: 2,
                drift_from_reference_milli: 2,
                robust_z_milli: 2_000,
            },
            CalibrationPoint {
                run_id: "cal-3".into(),
                sequence_index: 3,
                observed_milli: 504,
                expected_milli: 500,
                residual_milli: 4,
                drift_from_reference_milli: 4,
                robust_z_milli: 4_000,
            },
        ];
        let mut output = InstrumentCalibration {
            feature_id: super::super::calibration::FEATURE_ID.into(),
            output_schema: super::super::calibration::OUTPUT_SCHEMA.into(),
            objective: "qualify imager".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            metric_name: "control".into(),
            run_order: vec!["cal-1".into(), "cal-2".into(), "cal-3".into()],
            reference_order: vec!["cal-1".into(), "cal-2".into()],
            points,
            reference_residual_median_milli: 0,
            reference_mad_milli: 0,
            final_drift_milli: 4,
            max_abs_drift_milli: 4,
            slope_milli_per_tick: 2,
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: CalibrationDisposition::Qualified,
            digest: hash("placeholder"),
        };
        let body = serde_json::json!({
            "objective":"qualify imager",
            "instrument_id":"imager-1",
            "model_system":"organoid",
            "metric_name":"control",
            "run_order":["cal-1","cal-2","cal-3"],
            "reference_order":["cal-1","cal-2"],
            "points":output.points,
            "reference_residual_median_milli":0,
            "reference_mad_milli":0,
            "final_drift_milli":4,
            "max_abs_drift_milli":4,
            "slope_milli_per_tick":2,
            "negative_evidence":[],
            "uncertainty":[],
            "disposition":"qualified",
            "feature_id":super::super::calibration::FEATURE_ID,
            "output_schema":super::super::calibration::OUTPUT_SCHEMA
        });
        output.digest = ContentHash::of_value(&body).unwrap();
        output
    }

    fn action(id: &str, start: u64, operation: InstrumentOperation) -> InstrumentAction {
        InstrumentAction {
            action_id: id.into(),
            instrument_id: "imager-1".into(),
            operation,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: start,
            duration_ticks: 2,
            risk_milli: 100,
            requires_operator: operation.inherently_destructive(),
            output_schema: format!("{id}1@1"),
            parameters: if operation.requires_volume() {
                vec![InstrumentParameter {
                    name: "volume_microliter".into(),
                    value_milli: 10_000,
                    unit: "microliter_milli".into(),
                    minimum_milli: Some(1),
                    maximum_milli: Some(100_000),
                }]
            } else {
                Vec::new()
            },
        }
    }

    fn request() -> InstrumentPreflightRequest {
        InstrumentPreflightRequest {
            objective: "preflight organoid imaging".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: vec![
                action("acquire", 1, InstrumentOperation::AcquireImage),
                action("wash", 3, InstrumentOperation::Wash),
            ],
            calibration: calibration(),
            interlocks: InstrumentInterlockSnapshot {
                observed_tick: 1,
                emergency_stop_clear: true,
                guard_closed: true,
                deck_clear: true,
                consumables_available: true,
                waste_capacity_milli: 100_000,
                temperature_milli: Some(37000),
                minimum_temperature_milli: Some(36000),
                maximum_temperature_milli: Some(38000),
                calibration_valid_until_tick: 100,
                calibration_sequence_index: 3,
            },
            authorization: InstrumentAuthorization {
                authorization_id: "approval-1".into(),
                operator_id: "operator-1".into(),
                instrument_scope: "imager-1".into(),
                approval_digest: hash("signed-approval"),
                issued_tick: 0,
                expires_tick: 100,
                revoked: false,
            },
            current_tick: 1,
            maximum_total_risk_milli: 500,
            maximum_duration_ticks: 20,
            minimum_waste_capacity_milli: 100,
        }
    }

    #[test]
    fn admits_serialized_actions_with_typed_volume_and_interlocks() {
        let plan = preflight_glioma_instrument(&request()).unwrap();
        assert_eq!(plan.disposition, InstrumentPreflightDisposition::Admitted);
        assert!(plan.dispatch_permitted);
        assert_eq!(plan.admitted_order, vec!["acquire", "wash"]);
        assert_eq!(plan.decisions[1].scheduled_start_tick, Some(3));
        plan.validate().unwrap();
    }

    #[test]
    fn stale_or_unsafe_controls_block_without_erasing_actions() {
        let mut request = request();
        request.interlocks.emergency_stop_clear = false;
        request.actions[1].parameters[0].value_milli = 0;
        let plan = preflight_glioma_instrument(&request).unwrap();
        assert_eq!(plan.disposition, InstrumentPreflightDisposition::Blocked);
        assert!(!plan.dispatch_permitted);
        assert_eq!(plan.blocked_order, vec!["acquire", "wash"]);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|item| item.contains("emergency-stop")));
        assert!(plan
            .negative_evidence
            .iter()
            .any(|item| item.contains("volume")));
    }

    #[test]
    fn missing_temperature_is_unresolved_not_imputed() {
        let mut request = request();
        request.interlocks.temperature_milli = None;
        request.interlocks.minimum_temperature_milli = None;
        request.interlocks.maximum_temperature_milli = None;
        let plan = preflight_glioma_instrument(&request).unwrap();
        assert_eq!(plan.disposition, InstrumentPreflightDisposition::Unresolved);
        assert!(!plan.dispatch_permitted);
        assert!(plan
            .uncertainty
            .iter()
            .any(|item| item.contains("temperature-window-unmeasured")));
    }

    #[test]
    fn calibration_analysis_can_supply_a_qualified_plan() {
        let runs = (1..=3)
            .map(|sequence_index| CalibrationRun {
                run_id: format!("run-{sequence_index}"),
                sequence_index,
                batch_id: format!("batch-{sequence_index}"),
                instrument_id: "imager-1".into(),
                metric_name: "control".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 500 + i64::from(sequence_index),
                expected_milli: 500,
                artifact: LocalArtifactRef {
                    artifact_id: format!("artifact-{sequence_index}"),
                    content_hash: hash(&format!("artifact-{sequence_index}")),
                    content_type: "application/vnd.aurora.glioma-control+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
            .collect::<Vec<_>>();
        let calibration = analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify imager".into(),
                instrument_id: "imager-1".into(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "control".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 5,
                max_drift_milli: 20,
                max_slope_milli_per_tick: 10,
            },
            &runs,
        )
        .unwrap();
        let mut request = request();
        request.calibration = calibration;
        let plan = preflight_glioma_instrument(&request).unwrap();
        assert_eq!(plan.disposition, InstrumentPreflightDisposition::Admitted);
    }
}
