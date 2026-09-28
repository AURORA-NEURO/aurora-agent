//! Compile validated preclinical assay protocols into typed instrument actions.
//!
//! The compiler is deliberately earlier than P08 preflight and protocol binding.  It translates
//! an investigator-owned protocol into an action plan, performs exact dimensional conversion,
//! checks the version-pinned device capability profile, and records unsupported work instead of
//! silently dropping it.  The result still requires ordinary P08 calibration, interlocks,
//! authorization, protocol binding, and local gateway execution before any physical effect.

use super::assay_run_schema::{AssayProtocolReference, AssaySampleToken};
use super::preflight::{InstrumentAction, InstrumentOperation, InstrumentParameter};
use super::protocol_binding::{
    InstrumentCommandCapability, InstrumentProtocolBindingError, InstrumentProtocolManifest,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentProtocolCompiler1@1";
pub const MAX_STEPS: usize = 512;
pub const MAX_PARAMETERS_PER_STEP: usize = 64;
pub const DEFAULT_ARTIFACT_TYPE: &str = "application/vnd.aurora.glioma.instrument-operation+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolUnit {
    MicroliterMilli,
    MilliliterMilli,
    Millisecond,
    SecondMilli,
    CelsiusMilli,
    WattMilli,
    PercentMilli,
    CountMilli,
    MicrometerMilli,
    UnitlessMilli,
}

impl ProtocolUnit {
    fn wire_name(self) -> &'static str {
        match self {
            Self::MicroliterMilli => "microliter_milli",
            Self::MilliliterMilli => "milliliter_milli",
            Self::Millisecond => "millisecond",
            Self::SecondMilli => "second_milli",
            Self::CelsiusMilli => "celsius_milli",
            Self::WattMilli => "milliwatt",
            Self::PercentMilli => "percent_milli",
            Self::CountMilli => "count_milli",
            Self::MicrometerMilli => "micrometer_milli",
            Self::UnitlessMilli => "unitless_milli",
        }
    }

    fn dimension(self) -> ProtocolDimension {
        match self {
            Self::MicroliterMilli | Self::MilliliterMilli => ProtocolDimension::Volume,
            Self::Millisecond | Self::SecondMilli => ProtocolDimension::Time,
            Self::CelsiusMilli => ProtocolDimension::Temperature,
            Self::WattMilli => ProtocolDimension::Power,
            Self::PercentMilli => ProtocolDimension::Percent,
            Self::CountMilli => ProtocolDimension::Count,
            Self::MicrometerMilli => ProtocolDimension::Length,
            Self::UnitlessMilli => ProtocolDimension::Unitless,
        }
    }

    /// Integer multiplier into the dimension's canonical base unit.
    fn base_multiplier(self) -> i128 {
        match self {
            Self::MilliliterMilli | Self::SecondMilli => 1_000,
            _ => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProtocolDimension {
    Volume,
    Time,
    Temperature,
    Power,
    Percent,
    Count,
    Length,
    Unitless,
}

fn wire_unit_spec(unit: &str) -> Option<(ProtocolDimension, i128)> {
    match unit {
        "microliter_milli" => Some((ProtocolDimension::Volume, 1)),
        "milliliter_milli" => Some((ProtocolDimension::Volume, 1_000)),
        "millisecond" => Some((ProtocolDimension::Time, 1)),
        "second_milli" => Some((ProtocolDimension::Time, 1_000)),
        "celsius_milli" => Some((ProtocolDimension::Temperature, 1)),
        "milliwatt" => Some((ProtocolDimension::Power, 1)),
        "percent_milli" => Some((ProtocolDimension::Percent, 1)),
        "count_milli" => Some((ProtocolDimension::Count, 1)),
        "micrometer_milli" => Some((ProtocolDimension::Length, 1)),
        "unitless_milli" => Some((ProtocolDimension::Unitless, 1)),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolParameter {
    pub name: String,
    pub value_milli: i64,
    pub unit: ProtocolUnit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStep {
    pub step_id: String,
    pub operation: InstrumentOperation,
    pub offset_ticks: u64,
    pub duration_ticks: u64,
    pub risk_milli: u64,
    pub output_schema: String,
    pub parameters: Vec<ProtocolParameter>,
    pub expected_artifact_type: Option<String>,
    pub compensation_actions: Vec<CompensationAction>,
    pub requires_operator: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationAction {
    EmergencyStop,
    PreserveLocalArtifact,
    MarkSampleState,
    OperatorReview,
    Decontaminate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentPreflightChecklistItem {
    pub step_id: String,
    pub checks: Vec<PreflightCheck>,
    pub evidence_binding: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightCheck {
    CalibrationQualified,
    EmergencyStopClear,
    GuardClosed,
    DeckClear,
    ConsumablesAvailable,
    OperatorAuthorization,
    LocalArtifactStore,
    WasteCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedProtocolArtifact {
    pub step_id: String,
    pub content_type: String,
    pub local_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedProtocolStep {
    pub step_id: String,
    pub operation: InstrumentOperation,
    pub disposition: UnsupportedStepDisposition,
    pub reason: String,
    pub requested_units: Vec<String>,
    pub required_capability: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedStepDisposition {
    UnsupportedCommand,
    DimensionalMismatch,
    InvalidParameter,
    PolicyBlocked,
    ScheduleBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolInterlockPolicy {
    pub require_calibration: bool,
    pub require_emergency_stop_clear: bool,
    pub require_guard_closed: bool,
    pub require_deck_clear: bool,
    pub require_consumables: bool,
    pub require_local_artifact_store: bool,
    pub require_operator_for_destructive: bool,
    pub maximum_total_risk_milli: u64,
    pub maximum_duration_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentProtocolCompileRequest {
    pub objective: String,
    pub protocol: AssayProtocolReference,
    pub sample: AssaySampleToken,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub device_profile: InstrumentProtocolManifest,
    pub steps: Vec<ProtocolStep>,
    pub interlock_policy: ProtocolInterlockPolicy,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledProtocolDisposition {
    Ready,
    Unsupported,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledInstrumentProtocol {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub protocol: AssayProtocolReference,
    pub sample_token: String,
    pub instrument_id: String,
    pub model_system: GliomaModelSystem,
    pub device_manifest_id: String,
    pub device_manifest_revision: u64,
    pub action_order: Vec<String>,
    pub actions: Vec<InstrumentAction>,
    pub expected_artifacts: Vec<ExpectedProtocolArtifact>,
    pub preflight_checklist: Vec<InstrumentPreflightChecklistItem>,
    pub compensation_boundaries: Vec<(String, Vec<CompensationAction>)>,
    pub unsupported_steps: Vec<UnsupportedProtocolStep>,
    pub total_risk_milli: u64,
    pub total_duration_ticks: u64,
    pub dispatch_permitted: bool,
    pub disposition: CompiledProtocolDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentProtocolCompilerError {
    #[error("instrument protocol compiler request is invalid: {0}")]
    InvalidRequest(String),
    #[error("device profile is invalid: {0}")]
    InvalidManifest(String),
    #[error("instrument protocol compiler digest failed: {0}")]
    Digest(String),
}

fn inherently_destructive(operation: InstrumentOperation) -> bool {
    matches!(
        operation,
        InstrumentOperation::AddReagent
            | InstrumentOperation::Aspirate
            | InstrumentOperation::Dispense
            | InstrumentOperation::Wash
            | InstrumentOperation::Transfer
            | InstrumentOperation::Sequence
    )
}

fn convert_value(
    value_milli: i64,
    source: ProtocolUnit,
    target: &str,
) -> Result<i64, (UnsupportedStepDisposition, String)> {
    let Some((target_dimension, target_multiplier)) = wire_unit_spec(target) else {
        return Err((
            UnsupportedStepDisposition::DimensionalMismatch,
            format!("device unit {target} is not in the compiler's versioned unit vocabulary"),
        ));
    };
    if source.dimension() != target_dimension {
        return Err((
            UnsupportedStepDisposition::DimensionalMismatch,
            format!(
                "{} cannot be converted to device unit {target}",
                source.wire_name()
            ),
        ));
    }
    let numerator = i128::from(value_milli).saturating_mul(source.base_multiplier());
    if numerator % target_multiplier != 0 {
        return Err((
            UnsupportedStepDisposition::DimensionalMismatch,
            format!(
                "conversion from {} to {target} is not exact at fixed-point precision",
                source.wire_name()
            ),
        ));
    }
    let converted = numerator / target_multiplier;
    i64::try_from(converted).map_err(|_| {
        (
            UnsupportedStepDisposition::InvalidParameter,
            format!("converted value for {target} exceeds the signed fixed-point range"),
        )
    })
}

fn digest_input(output: &CompiledInstrumentProtocol) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "protocol": output.protocol,
        "sample_token": output.sample_token,
        "instrument_id": output.instrument_id,
        "model_system": output.model_system,
        "device_manifest_id": output.device_manifest_id,
        "device_manifest_revision": output.device_manifest_revision,
        "action_order": output.action_order,
        "actions": output.actions,
        "expected_artifacts": output.expected_artifacts,
        "preflight_checklist": output.preflight_checklist,
        "compensation_boundaries": output.compensation_boundaries,
        "unsupported_steps": output.unsupported_steps,
        "total_risk_milli": output.total_risk_milli,
        "total_duration_ticks": output.total_duration_ticks,
        "dispatch_permitted": output.dispatch_permitted,
        "disposition": output.disposition,
    })
}

impl CompiledInstrumentProtocol {
    pub fn validate(&self) -> Result<(), InstrumentProtocolCompilerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.protocol.protocol_id.trim().is_empty()
            || self.sample_token.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.device_manifest_id.trim().is_empty()
            || self.device_manifest_revision == 0
            || self.action_order.len() != self.actions.len()
            || self.action_order.windows(2).any(|pair| pair[0] == pair[1])
            || self.expected_artifacts.len() != self.actions.len()
            || self.preflight_checklist.len() != self.actions.len()
            || self
                .unsupported_steps
                .windows(2)
                .any(|pair| pair[0].step_id >= pair[1].step_id)
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty() || action.instrument_id != self.instrument_id
            })
            || self
                .expected_artifacts
                .iter()
                .any(|artifact| artifact.content_type.trim().is_empty() || !artifact.local_only)
            || self
                .preflight_checklist
                .iter()
                .any(|item| item.step_id.trim().is_empty() || item.checks.is_empty())
            || self
                .compensation_boundaries
                .iter()
                .any(|(step_id, actions)| step_id.trim().is_empty() || actions.is_empty())
        {
            return Err(InstrumentProtocolCompilerError::InvalidRequest(
                "compiled protocol identity, action coverage, locality, or safety partitions are invalid"
                    .into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<Vec<_>>();
        let compensation_by_step = self
            .compensation_boundaries
            .iter()
            .map(|(step_id, _)| step_id.as_str())
            .collect::<BTreeSet<_>>();
        if action_ids != self.action_order
            || self
                .expected_artifacts
                .iter()
                .zip(&self.action_order)
                .any(|(artifact, action_id)| artifact.step_id != *action_id)
            || self
                .preflight_checklist
                .iter()
                .zip(&self.action_order)
                .any(|(item, action_id)| item.step_id != *action_id)
            || self
                .action_order
                .iter()
                .any(|action_id| !compensation_by_step.contains(action_id.as_str()))
            || self
                .compensation_boundaries
                .windows(2)
                .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err(InstrumentProtocolCompilerError::InvalidRequest(
                "compiled action order does not reconcile with downstream contracts".into(),
            ));
        }
        if self.disposition == CompiledProtocolDisposition::Ready
            && (!self.dispatch_permitted || !self.unsupported_steps.is_empty())
        {
            return Err(InstrumentProtocolCompilerError::InvalidRequest(
                "ready protocol must be dispatch-permitted and fully supported".into(),
            ));
        }
        if self.disposition != CompiledProtocolDisposition::Ready && self.dispatch_permitted {
            return Err(InstrumentProtocolCompilerError::InvalidRequest(
                "blocked, unsupported, and unresolved protocols cannot be dispatch-permitted"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InstrumentProtocolCompilerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InstrumentProtocolCompilerError::InvalidRequest(
                "compiled protocol digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn capability_for<'a>(
    step: &ProtocolStep,
    profile: &'a InstrumentProtocolManifest,
) -> Result<&'a InstrumentCommandCapability, UnsupportedProtocolStep> {
    let matches = profile
        .commands
        .iter()
        .filter(|command| {
            command.operation == step.operation && command.output_schema == step.output_schema
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [capability] => Ok(capability),
        [] => Err(UnsupportedProtocolStep {
            step_id: step.step_id.clone(),
            operation: step.operation,
            disposition: UnsupportedStepDisposition::UnsupportedCommand,
            reason: "device profile does not advertise the requested operation/output schema"
                .into(),
            requested_units: step
                .parameters
                .iter()
                .map(|parameter| parameter.unit.wire_name().to_string())
                .collect(),
            required_capability: Some(format!(
                "operation={:?};output_schema={}",
                step.operation, step.output_schema
            )),
        }),
        _ => Err(UnsupportedProtocolStep {
            step_id: step.step_id.clone(),
            operation: step.operation,
            disposition: UnsupportedStepDisposition::UnsupportedCommand,
            reason: "device profile advertises multiple indistinguishable commands".into(),
            requested_units: Vec::new(),
            required_capability: Some(format!(
                "operation={:?};output_schema={}",
                step.operation, step.output_schema
            )),
        }),
    }
}

fn compile_parameters(
    step: &ProtocolStep,
    capability: &InstrumentCommandCapability,
) -> Result<Vec<InstrumentParameter>, UnsupportedProtocolStep> {
    if step.parameters.len() > MAX_PARAMETERS_PER_STEP {
        return Err(UnsupportedProtocolStep {
            step_id: step.step_id.clone(),
            operation: step.operation,
            disposition: UnsupportedStepDisposition::InvalidParameter,
            reason: "parameter count exceeds the bounded compiler limit".into(),
            requested_units: Vec::new(),
            required_capability: Some(capability.command_id.clone()),
        });
    }
    let mut requested = BTreeMap::new();
    for parameter in &step.parameters {
        if parameter.name.trim().is_empty()
            || requested
                .insert(parameter.name.clone(), parameter)
                .is_some()
        {
            return Err(UnsupportedProtocolStep {
                step_id: step.step_id.clone(),
                operation: step.operation,
                disposition: UnsupportedStepDisposition::InvalidParameter,
                reason: "parameter names must be non-empty and unique".into(),
                requested_units: Vec::new(),
                required_capability: Some(capability.command_id.clone()),
            });
        }
    }
    let capability_parameters = capability
        .parameters
        .iter()
        .map(|parameter| (parameter.action_parameter.as_str(), parameter))
        .collect::<BTreeMap<_, _>>();
    for (name, parameter) in &requested {
        let Some(specification) = capability_parameters.get(name.as_str()) else {
            return Err(UnsupportedProtocolStep {
                step_id: step.step_id.clone(),
                operation: step.operation,
                disposition: UnsupportedStepDisposition::UnsupportedCommand,
                reason: format!("device command does not accept parameter {name}"),
                requested_units: vec![parameter.unit.wire_name().into()],
                required_capability: Some(capability.command_id.clone()),
            });
        };
        let value_milli = convert_value(parameter.value_milli, parameter.unit, &specification.unit)
            .map_err(|(disposition, reason)| UnsupportedProtocolStep {
                step_id: step.step_id.clone(),
                operation: step.operation,
                disposition,
                reason,
                requested_units: vec![parameter.unit.wire_name().into()],
                required_capability: Some(capability.command_id.clone()),
            })?;
        if specification
            .minimum_milli
            .is_some_and(|minimum| value_milli < minimum)
            || specification
                .maximum_milli
                .is_some_and(|maximum| value_milli > maximum)
        {
            return Err(UnsupportedProtocolStep {
                step_id: step.step_id.clone(),
                operation: step.operation,
                disposition: UnsupportedStepDisposition::InvalidParameter,
                reason: format!("converted parameter {name} falls outside the device range"),
                requested_units: vec![parameter.unit.wire_name().into()],
                required_capability: Some(capability.command_id.clone()),
            });
        }
    }
    if capability
        .parameters
        .iter()
        .any(|parameter| parameter.required && !requested.contains_key(&parameter.action_parameter))
    {
        return Err(UnsupportedProtocolStep {
            step_id: step.step_id.clone(),
            operation: step.operation,
            disposition: UnsupportedStepDisposition::InvalidParameter,
            reason: "required device parameter is missing from the protocol step".into(),
            requested_units: Vec::new(),
            required_capability: Some(capability.command_id.clone()),
        });
    }
    let mut output = requested
        .into_iter()
        .map(|(name, parameter)| {
            let specification = capability_parameters[name.as_str()];
            let value_milli =
                convert_value(parameter.value_milli, parameter.unit, &specification.unit)
                    .expect("parameter conversion was checked above");
            InstrumentParameter {
                name,
                value_milli,
                unit: specification.unit.clone(),
                minimum_milli: specification.minimum_milli,
                maximum_milli: specification.maximum_milli,
            }
        })
        .collect::<Vec<_>>();
    output.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(output)
}

fn checklist_for(
    step: &ProtocolStep,
    policy: &ProtocolInterlockPolicy,
    protocol: &AssayProtocolReference,
    profile: &InstrumentProtocolManifest,
) -> InstrumentPreflightChecklistItem {
    let mut checks = BTreeSet::new();
    if policy.require_calibration {
        checks.insert(PreflightCheck::CalibrationQualified);
    }
    if policy.require_emergency_stop_clear {
        checks.insert(PreflightCheck::EmergencyStopClear);
    }
    if policy.require_guard_closed {
        checks.insert(PreflightCheck::GuardClosed);
    }
    if policy.require_deck_clear {
        checks.insert(PreflightCheck::DeckClear);
    }
    if policy.require_consumables {
        checks.insert(PreflightCheck::ConsumablesAvailable);
    }
    if policy.require_local_artifact_store {
        checks.insert(PreflightCheck::LocalArtifactStore);
    }
    if policy.require_operator_for_destructive
        && (step.requires_operator || inherently_destructive(step.operation))
    {
        checks.insert(PreflightCheck::OperatorAuthorization);
    }
    if matches!(
        step.operation,
        InstrumentOperation::AddReagent
            | InstrumentOperation::Aspirate
            | InstrumentOperation::Dispense
            | InstrumentOperation::Wash
            | InstrumentOperation::Transfer
    ) {
        checks.insert(PreflightCheck::WasteCapacity);
    }
    InstrumentPreflightChecklistItem {
        step_id: step.step_id.clone(),
        checks: checks.into_iter().collect(),
        evidence_binding: format!(
            "protocol:{}@{}:{};device:{}:{};step:{}",
            protocol.protocol_id,
            protocol.protocol_version,
            protocol.protocol_digest,
            profile.manifest_id,
            profile.revision,
            step.step_id
        ),
    }
}

pub fn compile_glioma_instrument_protocol(
    request: &InstrumentProtocolCompileRequest,
) -> Result<CompiledInstrumentProtocol, InstrumentProtocolCompilerError> {
    if request.objective.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.current_tick == 0
        || request.steps.is_empty()
        || request.steps.len() > MAX_STEPS
        || request.model_system != request.sample.model_system
        || request.device_profile.instrument_id != request.instrument_id
        || request.interlock_policy.maximum_total_risk_milli == 0
        || request.interlock_policy.maximum_duration_ticks == 0
    {
        return Err(InstrumentProtocolCompilerError::InvalidRequest(
            "bounded objective, matching preclinical sample/device, steps, and safety budgets are required"
                .into(),
        ));
    }
    request
        .sample
        .validate()
        .map_err(|error| InstrumentProtocolCompilerError::InvalidRequest(error.to_string()))?;
    request
        .protocol
        .validate()
        .map_err(|error| InstrumentProtocolCompilerError::InvalidRequest(error.to_string()))?;
    request
        .device_profile
        .validate()
        .map_err(|error: InstrumentProtocolBindingError| {
            InstrumentProtocolCompilerError::InvalidManifest(error.to_string())
        })?;
    let mut step_ids = BTreeSet::new();
    let mut actions = Vec::new();
    let mut expected_artifacts = Vec::new();
    let mut checklist = Vec::new();
    let mut compensation_boundaries = Vec::new();
    let mut unsupported_steps = Vec::new();
    let mut total_risk_milli = 0_u64;
    let mut total_duration_ticks = 0_u64;
    let mut previous_end_tick = request.current_tick;
    for step in &request.steps {
        if step.step_id.trim().is_empty()
            || !step_ids.insert(step.step_id.clone())
            || step.output_schema.trim().is_empty()
            || step.duration_ticks == 0
            || step.compensation_actions.is_empty()
            || step
                .compensation_actions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(InstrumentProtocolCompilerError::InvalidRequest(
                "step identity, duration, output schema, and canonical compensation boundary are required"
                    .into(),
            ));
        }
        let start_tick = request
            .current_tick
            .checked_add(step.offset_ticks)
            .ok_or_else(|| {
                InstrumentProtocolCompilerError::InvalidRequest(
                    "step offset overflows the bounded tick domain".into(),
                )
            })?;
        let end_tick = start_tick.checked_add(step.duration_ticks).ok_or_else(|| {
            InstrumentProtocolCompilerError::InvalidRequest(
                "step duration overflows the bounded tick domain".into(),
            )
        })?;
        if start_tick < previous_end_tick {
            unsupported_steps.push(UnsupportedProtocolStep {
                step_id: step.step_id.clone(),
                operation: step.operation,
                disposition: UnsupportedStepDisposition::ScheduleBlocked,
                reason:
                    "protocol steps overlap; compiler requires an explicit sequential action order"
                        .into(),
                requested_units: Vec::new(),
                required_capability: None,
            });
            compensation_boundaries.push((step.step_id.clone(), step.compensation_actions.clone()));
            previous_end_tick = previous_end_tick.max(end_tick);
            continue;
        }
        previous_end_tick = end_tick;
        total_risk_milli = total_risk_milli.saturating_add(step.risk_milli);
        total_duration_ticks = total_duration_ticks.saturating_add(step.duration_ticks);
        let capability = match capability_for(step, &request.device_profile) {
            Ok(capability) => capability,
            Err(unsupported) => {
                unsupported_steps.push(unsupported);
                compensation_boundaries
                    .push((step.step_id.clone(), step.compensation_actions.clone()));
                continue;
            }
        };
        let parameters = match compile_parameters(step, capability) {
            Ok(parameters) => parameters,
            Err(unsupported) => {
                unsupported_steps.push(unsupported);
                compensation_boundaries
                    .push((step.step_id.clone(), step.compensation_actions.clone()));
                continue;
            }
        };
        let artifact_type = step
            .expected_artifact_type
            .clone()
            .unwrap_or_else(|| DEFAULT_ARTIFACT_TYPE.into());
        if artifact_type.trim().is_empty() {
            unsupported_steps.push(UnsupportedProtocolStep {
                step_id: step.step_id.clone(),
                operation: step.operation,
                disposition: UnsupportedStepDisposition::InvalidParameter,
                reason: "expected artifact content type cannot be empty".into(),
                requested_units: Vec::new(),
                required_capability: Some(capability.command_id.clone()),
            });
            compensation_boundaries.push((step.step_id.clone(), step.compensation_actions.clone()));
            continue;
        }
        let action = InstrumentAction {
            action_id: step.step_id.clone(),
            instrument_id: request.instrument_id.clone(),
            operation: step.operation,
            model_system: request.model_system,
            requested_start_tick: start_tick,
            duration_ticks: step.duration_ticks,
            risk_milli: step.risk_milli,
            requires_operator: step.requires_operator
                || (request.interlock_policy.require_operator_for_destructive
                    && inherently_destructive(step.operation)),
            output_schema: step.output_schema.clone(),
            parameters,
        };
        actions.push(action);
        expected_artifacts.push(ExpectedProtocolArtifact {
            step_id: step.step_id.clone(),
            content_type: artifact_type,
            local_only: true,
            required: true,
        });
        checklist.push(checklist_for(
            step,
            &request.interlock_policy,
            &request.protocol,
            &request.device_profile,
        ));
        compensation_boundaries.push((step.step_id.clone(), step.compensation_actions.clone()));
    }
    if total_risk_milli > request.interlock_policy.maximum_total_risk_milli {
        unsupported_steps.push(UnsupportedProtocolStep {
            step_id: "__policy__".into(),
            operation: InstrumentOperation::Shutdown,
            disposition: UnsupportedStepDisposition::PolicyBlocked,
            reason: "compiled total risk exceeds the protocol policy budget".into(),
            requested_units: Vec::new(),
            required_capability: None,
        });
    }
    if total_duration_ticks > request.interlock_policy.maximum_duration_ticks {
        unsupported_steps.push(UnsupportedProtocolStep {
            step_id: "__duration__".into(),
            operation: InstrumentOperation::Shutdown,
            disposition: UnsupportedStepDisposition::PolicyBlocked,
            reason: "compiled total duration exceeds the protocol policy budget".into(),
            requested_units: Vec::new(),
            required_capability: None,
        });
    }
    unsupported_steps.sort_by(|left, right| left.step_id.cmp(&right.step_id));
    let disposition = if unsupported_steps.iter().any(|step| {
        matches!(
            step.disposition,
            UnsupportedStepDisposition::PolicyBlocked | UnsupportedStepDisposition::ScheduleBlocked
        )
    }) {
        CompiledProtocolDisposition::Blocked
    } else if !unsupported_steps.is_empty() {
        CompiledProtocolDisposition::Unsupported
    } else if actions.is_empty() {
        CompiledProtocolDisposition::Unresolved
    } else {
        CompiledProtocolDisposition::Ready
    };
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut output = CompiledInstrumentProtocol {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        protocol: request.protocol.clone(),
        sample_token: request.sample.sample_token.clone(),
        instrument_id: request.instrument_id.clone(),
        model_system: request.model_system,
        device_manifest_id: request.device_profile.manifest_id.clone(),
        device_manifest_revision: request.device_profile.revision,
        action_order,
        actions,
        expected_artifacts,
        preflight_checklist: checklist,
        compensation_boundaries,
        unsupported_steps,
        total_risk_milli,
        total_duration_ticks,
        dispatch_permitted: disposition == CompiledProtocolDisposition::Ready,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-instrument-protocol"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InstrumentProtocolCompilerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::assay_run_schema::{
        AssayMaterialKind, AssayProtocolReference,
    };
    use crate::glioma::programs::p08_instrument_robotics::protocol_binding::{
        InstrumentCommandParameter, InstrumentControlProtocol,
    };

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn request() -> InstrumentProtocolCompileRequest {
        InstrumentProtocolCompileRequest {
            objective: "capture organoid invasion morphology".into(),
            protocol: AssayProtocolReference {
                protocol_id: "organoid-invasion-v2".into(),
                protocol_version: "2.0.0".into(),
                protocol_digest: hash("protocol"),
                approved_by: "methods-board".into(),
                approved_tick: 1,
                required_operations: vec![InstrumentOperation::AcquireImage],
            },
            sample: AssaySampleToken {
                sample_token: "sample-opaque-1".into(),
                study_id: "study-1".into(),
                model_system: GliomaModelSystem::Organoid,
                material_kind: AssayMaterialKind::Organoid,
                lineage_token: Some("lineage-a".into()),
                local_scope: "site-a".into(),
                human_origin: false,
                contains_direct_identifiers: false,
            },
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            device_profile: InstrumentProtocolManifest {
                instrument_id: "imager-1".into(),
                protocol: InstrumentControlProtocol::Sila2,
                protocol_version: "1.7.0".into(),
                manifest_id: "manifest-1".into(),
                revision: 3,
                commands: vec![InstrumentCommandCapability {
                    operation: InstrumentOperation::AcquireImage,
                    feature_path: "sila/imaging".into(),
                    command_id: "AcquireImage".into(),
                    output_schema: "image@1".into(),
                    retry_idempotent: true,
                    parameters: vec![InstrumentCommandParameter {
                        action_parameter: "exposure".into(),
                        wire_parameter: "exposure".into(),
                        unit: "millisecond".into(),
                        minimum_milli: Some(1),
                        maximum_milli: Some(120_000),
                        required: true,
                    }],
                }],
            },
            steps: vec![ProtocolStep {
                step_id: "capture-01".into(),
                operation: InstrumentOperation::AcquireImage,
                offset_ticks: 0,
                duration_ticks: 10,
                risk_milli: 50,
                output_schema: "image@1".into(),
                parameters: vec![ProtocolParameter {
                    name: "exposure".into(),
                    value_milli: 2,
                    unit: ProtocolUnit::SecondMilli,
                }],
                expected_artifact_type: Some("application/vnd.aurora.glioma.image+ome-ngff".into()),
                compensation_actions: vec![
                    CompensationAction::EmergencyStop,
                    CompensationAction::PreserveLocalArtifact,
                ],
                requires_operator: false,
            }],
            interlock_policy: ProtocolInterlockPolicy {
                require_calibration: true,
                require_emergency_stop_clear: true,
                require_guard_closed: true,
                require_deck_clear: true,
                require_consumables: false,
                require_local_artifact_store: true,
                require_operator_for_destructive: true,
                maximum_total_risk_milli: 500,
                maximum_duration_ticks: 100,
            },
            current_tick: 1,
        }
    }

    #[test]
    fn golden_protocol_converts_units_and_preserves_order() {
        let first = compile_glioma_instrument_protocol(&request()).unwrap();
        let second = compile_glioma_instrument_protocol(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, CompiledProtocolDisposition::Ready);
        assert!(first.dispatch_permitted);
        assert_eq!(first.actions[0].parameters[0].value_milli, 2_000);
        assert_eq!(first.actions[0].parameters[0].unit, "millisecond");
        assert_eq!(first.action_order, vec!["capture-01"]);
        first.validate().unwrap();
    }

    #[test]
    fn unsupported_command_is_preserved_and_not_dispatchable() {
        let mut request = request();
        request.steps[0].operation = InstrumentOperation::MoveStage;
        let compiled = compile_glioma_instrument_protocol(&request).unwrap();
        assert_eq!(
            compiled.disposition,
            CompiledProtocolDisposition::Unsupported
        );
        assert!(!compiled.dispatch_permitted);
        assert!(compiled.actions.is_empty());
        assert_eq!(compiled.unsupported_steps.len(), 1);
        assert!(compiled.unsupported_steps[0]
            .reason
            .contains("does not advertise"));
    }

    #[test]
    fn dimensional_mismatch_is_not_silently_coerced() {
        let mut request = request();
        request.steps[0].parameters[0].unit = ProtocolUnit::CelsiusMilli;
        let compiled = compile_glioma_instrument_protocol(&request).unwrap();
        assert_eq!(
            compiled.disposition,
            CompiledProtocolDisposition::Unsupported
        );
        assert_eq!(
            compiled.unsupported_steps[0].disposition,
            UnsupportedStepDisposition::DimensionalMismatch
        );
        assert!(!compiled.dispatch_permitted);
    }

    #[test]
    fn risk_budget_blocks_without_erasing_compensation() {
        let mut request = request();
        request.interlock_policy.maximum_total_risk_milli = 10;
        let compiled = compile_glioma_instrument_protocol(&request).unwrap();
        assert_eq!(compiled.disposition, CompiledProtocolDisposition::Blocked);
        assert!(!compiled.dispatch_permitted);
        assert_eq!(compiled.compensation_boundaries[0].0, "capture-01");
        assert!(compiled
            .unsupported_steps
            .iter()
            .any(|step| step.disposition == UnsupportedStepDisposition::PolicyBlocked));
    }
}
