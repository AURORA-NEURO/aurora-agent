//! Bind approved glioma instrument actions to a version-pinned local device protocol profile.
//!
//! Protocol adapters normalize discovered SiLA 2 or OPC UA LADS commands into a capability
//! manifest. This module matches a complete admitted P08 plan to that manifest before any device
//! call, validates operation/schema/parameter/unit/range compatibility, and supplies a guarded
//! `InstrumentExecutor` bridge. It contains no network client and never bypasses preflight,
//! authorization, live-interlock checks, or the institution's local gateway.

use super::execution::{InstrumentExecutionFailure, InstrumentExecutionResult, InstrumentExecutor};
use super::preflight::{
    InstrumentAction, InstrumentAuthorization, InstrumentInterlockSnapshot, InstrumentOperation,
    InstrumentPreflightDisposition, InstrumentPreflightPlan,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaInstrumentProtocolBinding1@1";
pub const MAX_PROTOCOL_COMMANDS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentControlProtocol {
    Sila2,
    OpcUaLads,
}

/// A normalized command capability extracted by a local protocol adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCommandCapability {
    pub operation: InstrumentOperation,
    /// Protocol-specific SiLA Feature/Command or OPC UA LADS Function/Program path.
    pub feature_path: String,
    pub command_id: String,
    pub output_schema: String,
    /// Only declare true when the local gateway can deduplicate retries by action identity.
    pub retry_idempotent: bool,
    pub parameters: Vec<InstrumentCommandParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentCommandParameter {
    /// Name used by the stable AURORA `InstrumentAction` contract.
    pub action_parameter: String,
    /// Parameter name exposed by the protocol command.
    pub wire_parameter: String,
    /// Exact unit expected on the wire. This binder does not silently convert units.
    pub unit: String,
    pub minimum_milli: Option<i64>,
    pub maximum_milli: Option<i64>,
    pub required: bool,
}

/// Point-in-time capability snapshot supplied by an institution-local SiLA/LADS adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentProtocolManifest {
    pub instrument_id: String,
    pub protocol: InstrumentControlProtocol,
    pub protocol_version: String,
    pub manifest_id: String,
    pub revision: u64,
    pub commands: Vec<InstrumentCommandCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundInstrumentParameter {
    pub wire_parameter: String,
    pub value_milli: i64,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundInstrumentCommand {
    pub action_id: String,
    pub instrument_id: String,
    pub operation: InstrumentOperation,
    pub protocol: InstrumentControlProtocol,
    pub protocol_version: String,
    pub manifest_id: String,
    pub manifest_revision: u64,
    pub feature_path: String,
    pub command_id: String,
    pub output_schema: String,
    pub retry_idempotent: bool,
    pub parameters: Vec<BoundInstrumentParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundInstrumentAction {
    /// Snapshot checked against the action presented by the execution controller.
    pub action: InstrumentAction,
    pub command: BoundInstrumentCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentProtocolBindingPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_id: String,
    pub preflight_digest: bioprism_ids::ContentHash,
    pub protocol: InstrumentControlProtocol,
    pub protocol_version: String,
    pub manifest_id: String,
    pub manifest_revision: u64,
    pub action_order: Vec<String>,
    pub bindings: Vec<BoundInstrumentAction>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentProtocolBindingError {
    #[error("instrument protocol manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("instrument protocol binding request is invalid: {0}")]
    InvalidRequest(String),
    #[error("no protocol command supports action {action_id}: {reason}")]
    UnsupportedAction { action_id: String, reason: String },
    #[error("multiple protocol commands match action {0}; binding must be unambiguous")]
    AmbiguousAction(String),
    #[error("instrument protocol gateway could not provide its local capability profile: {0}")]
    Gateway(String),
}

fn can_safely_retry(operation: InstrumentOperation) -> bool {
    matches!(
        operation,
        InstrumentOperation::AcquireImage | InstrumentOperation::Shutdown
    )
}

impl InstrumentProtocolManifest {
    pub fn validate(&self) -> Result<(), InstrumentProtocolBindingError> {
        if self.instrument_id.trim().is_empty()
            || self.protocol_version.trim().is_empty()
            || self.manifest_id.trim().is_empty()
            || self.revision == 0
            || self.commands.is_empty()
            || self.commands.len() > MAX_PROTOCOL_COMMANDS
        {
            return Err(InstrumentProtocolBindingError::InvalidManifest(
                "instrument, pinned protocol version, manifest identity/revision, and bounded commands are required".into(),
            ));
        }
        let mut command_ids = BTreeSet::new();
        let mut signatures = Vec::new();
        for command in &self.commands {
            if command.feature_path.trim().is_empty()
                || command.command_id.trim().is_empty()
                || command.output_schema.trim().is_empty()
                || !command_ids.insert((command.feature_path.as_str(), command.command_id.as_str()))
                || signatures.iter().any(|(operation, schema)| {
                    *operation == command.operation && *schema == command.output_schema
                })
                || (command.retry_idempotent && !can_safely_retry(command.operation))
            {
                return Err(InstrumentProtocolBindingError::InvalidManifest(
                    "command paths/ids/schemas must be present and unique; unsupported retry semantics are refused".into(),
                ));
            }
            signatures.push((command.operation, command.output_schema.as_str()));
            let mut action_names = BTreeSet::new();
            let mut wire_names = BTreeSet::new();
            for parameter in &command.parameters {
                if parameter.action_parameter.trim().is_empty()
                    || parameter.wire_parameter.trim().is_empty()
                    || parameter.unit.trim().is_empty()
                    || !action_names.insert(parameter.action_parameter.as_str())
                    || !wire_names.insert(parameter.wire_parameter.as_str())
                    || parameter
                        .minimum_milli
                        .zip(parameter.maximum_milli)
                        .is_some_and(|(minimum, maximum)| minimum > maximum)
                {
                    return Err(InstrumentProtocolBindingError::InvalidManifest(
                        "parameter names, units, uniqueness, and ranges must be valid".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl InstrumentProtocolBindingPlan {
    pub fn validate(&self) -> Result<(), InstrumentProtocolBindingError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.protocol_version.trim().is_empty()
            || self.manifest_id.trim().is_empty()
            || self.manifest_revision == 0
            || self.action_order.is_empty()
            || self.action_order.len() != self.bindings.len()
            || self
                .bindings
                .iter()
                .zip(&self.action_order)
                .any(|(binding, id)| {
                    binding.action.action_id != *id
                        || binding.command.action_id != *id
                        || binding.action.instrument_id != self.instrument_id
                        || binding.command.instrument_id != self.instrument_id
                        || binding.command.protocol != self.protocol
                        || binding.command.protocol_version != self.protocol_version
                        || binding.command.manifest_id != self.manifest_id
                        || binding.command.manifest_revision != self.manifest_revision
                })
            || self.action_order.iter().collect::<BTreeSet<_>>().len() != self.action_order.len()
        {
            return Err(InstrumentProtocolBindingError::InvalidRequest(
                "binding identity, manifest pin, or action order is inconsistent".into(),
            ));
        }
        for binding in &self.bindings {
            let action = &binding.action;
            let command = &binding.command;
            let action_parameters = action
                .parameters
                .iter()
                .map(|parameter| (parameter.value_milli, parameter.unit.as_str()))
                .collect::<BTreeSet<_>>();
            let command_parameters = command
                .parameters
                .iter()
                .map(|parameter| (parameter.value_milli, parameter.unit.as_str()))
                .collect::<BTreeSet<_>>();
            if command.operation != action.operation
                || command.output_schema != action.output_schema
                || command.feature_path.trim().is_empty()
                || command.command_id.trim().is_empty()
                || (command.retry_idempotent && !can_safely_retry(action.operation))
                || command.parameters.len() != action.parameters.len()
                || command
                    .parameters
                    .iter()
                    .any(|parameter| parameter.wire_parameter.trim().is_empty())
                || command
                    .parameters
                    .windows(2)
                    .any(|pair| pair[0].wire_parameter >= pair[1].wire_parameter)
                || action_parameters != command_parameters
            {
                return Err(InstrumentProtocolBindingError::InvalidRequest(
                    "bound command does not preserve the action contract or safe retry policy"
                        .into(),
                ));
            }
        }
        Ok(())
    }
}

fn canonical_manifest(manifest: &InstrumentProtocolManifest) -> InstrumentProtocolManifest {
    let mut canonical = manifest.clone();
    canonical.commands.sort_by(|left, right| {
        left.feature_path
            .cmp(&right.feature_path)
            .then_with(|| left.command_id.cmp(&right.command_id))
            .then_with(|| left.output_schema.cmp(&right.output_schema))
    });
    for command in &mut canonical.commands {
        command.parameters.sort_by(|left, right| {
            left.action_parameter
                .cmp(&right.action_parameter)
                .then_with(|| left.wire_parameter.cmp(&right.wire_parameter))
        });
    }
    canonical
}

fn bind_action(
    action: &InstrumentAction,
    manifest: &InstrumentProtocolManifest,
) -> Result<InstrumentCommandCapability, InstrumentProtocolBindingError> {
    let matches = manifest
        .commands
        .iter()
        .filter(|command| {
            command.operation == action.operation && command.output_schema == action.output_schema
        })
        .collect::<Vec<_>>();
    let capability = match matches.as_slice() {
        [] => {
            return Err(InstrumentProtocolBindingError::UnsupportedAction {
                action_id: action.action_id.clone(),
                reason: "operation/output schema is not advertised".into(),
            });
        }
        [capability] => (*capability).clone(),
        _ => {
            return Err(InstrumentProtocolBindingError::AmbiguousAction(
                action.action_id.clone(),
            ));
        }
    };
    let action_parameters = action
        .parameters
        .iter()
        .map(|parameter| (parameter.name.as_str(), parameter))
        .collect::<BTreeMap<_, _>>();
    if action_parameters.len() != action.parameters.len() {
        return Err(InstrumentProtocolBindingError::UnsupportedAction {
            action_id: action.action_id.clone(),
            reason: "action repeats a parameter name".into(),
        });
    }
    let capability_parameters = capability
        .parameters
        .iter()
        .map(|parameter| (parameter.action_parameter.as_str(), parameter))
        .collect::<BTreeMap<_, _>>();
    if action_parameters
        .keys()
        .any(|name| !capability_parameters.contains_key(name))
        || capability_parameters.keys().any(|name| {
            capability_parameters[name].required && !action_parameters.contains_key(name)
        })
    {
        return Err(InstrumentProtocolBindingError::UnsupportedAction {
            action_id: action.action_id.clone(),
            reason: "required parameters are missing or action parameters are unrecognized".into(),
        });
    }
    for (action_name, parameter) in &action_parameters {
        let specification = capability_parameters[*action_name];
        if parameter.unit != specification.unit
            || specification
                .minimum_milli
                .is_some_and(|minimum| parameter.value_milli < minimum)
            || specification
                .maximum_milli
                .is_some_and(|maximum| parameter.value_milli > maximum)
        {
            return Err(InstrumentProtocolBindingError::UnsupportedAction {
                action_id: action.action_id.clone(),
                reason: format!(
                    "parameter {action_name} has an incompatible unit, value, or declared bounds"
                ),
            });
        }
    }
    Ok(capability)
}

pub fn compile_glioma_instrument_protocol_binding(
    preflight: &InstrumentPreflightPlan,
    actions: &[InstrumentAction],
    manifest: &InstrumentProtocolManifest,
) -> Result<InstrumentProtocolBindingPlan, InstrumentProtocolBindingError> {
    preflight
        .validate()
        .map_err(|error| InstrumentProtocolBindingError::InvalidRequest(error.to_string()))?;
    manifest.validate()?;
    if !preflight.dispatch_permitted
        || preflight.disposition != InstrumentPreflightDisposition::Admitted
        || preflight.action_order != preflight.admitted_order
        || preflight.instrument_id != manifest.instrument_id
        || actions.len() != preflight.action_order.len()
    {
        return Err(InstrumentProtocolBindingError::InvalidRequest(
            "only a complete admitted plan with the exact instrument action set can be bound"
                .into(),
        ));
    }
    let actions_by_id = actions
        .iter()
        .map(|action| (action.action_id.as_str(), action))
        .collect::<BTreeMap<_, _>>();
    if actions_by_id.len() != actions.len()
        || preflight
            .action_order
            .iter()
            .any(|id| !actions_by_id.contains_key(id.as_str()))
    {
        return Err(InstrumentProtocolBindingError::InvalidRequest(
            "actions must uniquely cover the admitted preflight order".into(),
        ));
    }
    let mut bindings = Vec::with_capacity(actions.len());
    for action_id in &preflight.action_order {
        let action = actions_by_id[action_id.as_str()];
        if action.instrument_id != preflight.instrument_id
            || action.model_system != preflight.model_system
        {
            return Err(InstrumentProtocolBindingError::InvalidRequest(format!(
                "action {action_id} differs from the admitted model/instrument scope"
            )));
        }
        let capability = bind_action(action, manifest)?;
        let protocol_parameters = action
            .parameters
            .iter()
            .map(|parameter| {
                let specification = capability
                    .parameters
                    .iter()
                    .find(|specification| specification.action_parameter == parameter.name)
                    .expect("bind_action validated parameter coverage");
                BoundInstrumentParameter {
                    wire_parameter: specification.wire_parameter.clone(),
                    value_milli: parameter.value_milli,
                    unit: parameter.unit.clone(),
                }
            })
            .collect::<Vec<_>>();
        let mut protocol_parameters = protocol_parameters;
        protocol_parameters.sort_by(|left, right| left.wire_parameter.cmp(&right.wire_parameter));
        let command = BoundInstrumentCommand {
            action_id: action.action_id.clone(),
            instrument_id: action.instrument_id.clone(),
            operation: action.operation,
            protocol: manifest.protocol,
            protocol_version: manifest.protocol_version.clone(),
            manifest_id: manifest.manifest_id.clone(),
            manifest_revision: manifest.revision,
            feature_path: capability.feature_path.clone(),
            command_id: capability.command_id.clone(),
            output_schema: capability.output_schema.clone(),
            retry_idempotent: capability.retry_idempotent,
            parameters: protocol_parameters,
        };
        bindings.push(BoundInstrumentAction {
            action: action.clone(),
            command,
        });
    }
    let plan = InstrumentProtocolBindingPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: preflight.objective.clone(),
        instrument_id: preflight.instrument_id.clone(),
        preflight_digest: preflight.digest.clone(),
        protocol: manifest.protocol,
        protocol_version: manifest.protocol_version.clone(),
        manifest_id: manifest.manifest_id.clone(),
        manifest_revision: manifest.revision,
        action_order: preflight.action_order.clone(),
        bindings,
    };
    plan.validate()?;
    Ok(plan)
}

/// Institution-local protocol adapter for SiLA 2 or OPC UA LADS.
pub trait InstrumentProtocolGateway {
    fn observe_interlocks(
        &mut self,
    ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure>;

    fn verify_authorization(
        &mut self,
        authorization: &InstrumentAuthorization,
    ) -> Result<(), InstrumentExecutionFailure>;

    /// Load the live, institution-local capability profile for this instrument.
    fn current_manifest(
        &mut self,
        instrument_id: &str,
    ) -> Result<InstrumentProtocolManifest, InstrumentExecutionFailure>;

    /// Enforce the command's pinned manifest revision atomically with dispatch.
    fn execute_bound_command(
        &mut self,
        command: &BoundInstrumentCommand,
        attempt: u8,
    ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure>;

    fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure>;

    fn simulation_only(&self) -> bool {
        false
    }
}

/// Borrowed gateways keep one institution-local session alive across adaptive campaign rounds.
impl<G: InstrumentProtocolGateway + ?Sized> InstrumentProtocolGateway for &mut G {
    fn observe_interlocks(
        &mut self,
    ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
        (**self).observe_interlocks()
    }

    fn verify_authorization(
        &mut self,
        authorization: &InstrumentAuthorization,
    ) -> Result<(), InstrumentExecutionFailure> {
        (**self).verify_authorization(authorization)
    }

    fn current_manifest(
        &mut self,
        instrument_id: &str,
    ) -> Result<InstrumentProtocolManifest, InstrumentExecutionFailure> {
        (**self).current_manifest(instrument_id)
    }

    fn execute_bound_command(
        &mut self,
        command: &BoundInstrumentCommand,
        attempt: u8,
    ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
        (**self).execute_bound_command(command, attempt)
    }

    fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
        (**self).emergency_stop()
    }

    fn simulation_only(&self) -> bool {
        (**self).simulation_only()
    }
}

/// Adapter enforcing the compiled command map while implementing the existing P08 executor API.
pub struct ProtocolBoundInstrumentExecutor<G: InstrumentProtocolGateway> {
    plan: InstrumentProtocolBindingPlan,
    manifest: InstrumentProtocolManifest,
    gateway: G,
}

impl<G: InstrumentProtocolGateway> ProtocolBoundInstrumentExecutor<G> {
    pub fn new(
        plan: InstrumentProtocolBindingPlan,
        preflight: &InstrumentPreflightPlan,
        actions: &[InstrumentAction],
        mut gateway: G,
    ) -> Result<Self, InstrumentProtocolBindingError> {
        plan.validate()?;
        if plan.preflight_digest != preflight.digest {
            return Err(InstrumentProtocolBindingError::InvalidRequest(
                "protocol bindings do not belong to the supplied admitted preflight".into(),
            ));
        }
        let manifest = gateway
            .current_manifest(&plan.instrument_id)
            .map_err(|error| InstrumentProtocolBindingError::Gateway(error.reason))?;
        let rebuilt = compile_glioma_instrument_protocol_binding(preflight, actions, &manifest)?;
        if rebuilt != plan {
            return Err(InstrumentProtocolBindingError::InvalidRequest(
                "compiled command bindings differ from the live local device profile or admitted actions".into(),
            ));
        }
        Ok(Self {
            plan,
            manifest,
            gateway,
        })
    }

    pub fn gateway(&self) -> &G {
        &self.gateway
    }

    pub fn gateway_mut(&mut self) -> &mut G {
        &mut self.gateway
    }

    pub fn into_gateway(self) -> G {
        self.gateway
    }
}

impl<G: InstrumentProtocolGateway> InstrumentExecutor for ProtocolBoundInstrumentExecutor<G> {
    fn observe_interlocks(
        &mut self,
    ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
        self.gateway.observe_interlocks()
    }

    fn verify_authorization(
        &mut self,
        authorization: &InstrumentAuthorization,
    ) -> Result<(), InstrumentExecutionFailure> {
        self.gateway.verify_authorization(authorization)
    }

    fn execute_action(
        &mut self,
        action: &InstrumentAction,
        attempt: u8,
    ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
        let Some(binding) = self
            .plan
            .bindings
            .iter()
            .find(|binding| binding.action.action_id == action.action_id)
        else {
            return Err(InstrumentExecutionFailure {
                reason: format!(
                    "action {} has no precompiled protocol binding",
                    action.action_id
                ),
                retryable: false,
            });
        };
        if &binding.action != action {
            return Err(InstrumentExecutionFailure {
                reason: format!("action {} changed after protocol binding", action.action_id),
                retryable: false,
            });
        }
        let current = self
            .gateway
            .current_manifest(&self.plan.instrument_id)
            .map_err(|mut failure| {
                failure.retryable = false;
                failure
            })?;
        if canonical_manifest(&current) != canonical_manifest(&self.manifest) {
            return Err(InstrumentExecutionFailure {
                reason: "live instrument protocol profile changed after command binding".into(),
                retryable: false,
            });
        }
        self.gateway
            .execute_bound_command(&binding.command, attempt)
            .map_err(|mut failure| {
                failure.retryable &= binding.command.retry_idempotent;
                failure
            })
    }

    fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
        self.gateway.emergency_stop()
    }

    fn simulation_only(&self) -> bool {
        self.gateway.simulation_only()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationDisposition, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::{
        execute_glioma_instrument_plan, InstrumentExecutionDisposition, InstrumentExecutionRequest,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        preflight_glioma_instrument, InstrumentPreflightRequest,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;

    fn action() -> InstrumentAction {
        InstrumentAction {
            action_id: "capture-01".into(),
            instrument_id: "organoid-imager-1".into(),
            operation: InstrumentOperation::AcquireImage,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: 1,
            duration_ticks: 10,
            risk_milli: 10,
            requires_operator: false,
            output_schema: "GliomaMicroscopyImage1@1".into(),
            parameters: vec![
                crate::glioma::programs::p08_instrument_robotics::preflight::InstrumentParameter {
                    name: "exposure_ms".into(),
                    value_milli: 1_250,
                    unit: "millisecond_milli".into(),
                    minimum_milli: Some(500),
                    maximum_milli: Some(2_000),
                },
            ],
        }
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.instrument-control+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn manifest() -> InstrumentProtocolManifest {
        InstrumentProtocolManifest {
            instrument_id: "organoid-imager-1".into(),
            protocol: InstrumentControlProtocol::Sila2,
            protocol_version: "1.1".into(),
            manifest_id: "imager-profile-7".into(),
            revision: 7,
            commands: vec![InstrumentCommandCapability {
                operation: InstrumentOperation::AcquireImage,
                feature_path: "org.aurora.ImagingFeature".into(),
                command_id: "AcquireImage".into(),
                output_schema: "GliomaMicroscopyImage1@1".into(),
                retry_idempotent: false,
                parameters: vec![InstrumentCommandParameter {
                    action_parameter: "exposure_ms".into(),
                    wire_parameter: "ExposureTime".into(),
                    unit: "millisecond_milli".into(),
                    minimum_milli: Some(500),
                    maximum_milli: Some(2_000),
                    required: true,
                }],
            }],
        }
    }

    fn admitted_plan(action: &InstrumentAction) -> InstrumentPreflightPlan {
        let runs = (1..=3)
            .map(|sequence_index| CalibrationRun {
                run_id: format!("cal-{sequence_index}"),
                sequence_index,
                batch_id: format!("control-{sequence_index}"),
                instrument_id: action.instrument_id.clone(),
                metric_name: "pixel-reference".into(),
                model_system: action.model_system,
                observed_milli: 10_000,
                expected_milli: 10_000,
                artifact: artifact(&format!("calibration-{sequence_index}")),
            })
            .collect::<Vec<_>>();
        let calibration = analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify organoid imaging calibration".into(),
                instrument_id: action.instrument_id.clone(),
                model_system: action.model_system,
                metric_name: "pixel-reference".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 100,
                max_drift_milli: 100,
                max_slope_milli_per_tick: 100,
            },
            &runs,
        )
        .expect("calibration fixture");
        assert_eq!(calibration.disposition, CalibrationDisposition::Qualified);
        preflight_glioma_instrument(&InstrumentPreflightRequest {
            objective: "image glioma organoid".into(),
            instrument_id: action.instrument_id.clone(),
            model_system: action.model_system,
            actions: vec![action.clone()],
            calibration,
            interlocks: InstrumentInterlockSnapshot {
                observed_tick: 1,
                emergency_stop_clear: true,
                guard_closed: true,
                deck_clear: true,
                consumables_available: true,
                waste_capacity_milli: 1_000,
                temperature_milli: Some(37_000),
                minimum_temperature_milli: Some(36_000),
                maximum_temperature_milli: Some(38_000),
                calibration_valid_until_tick: 100,
                calibration_sequence_index: 3,
            },
            authorization: InstrumentAuthorization {
                authorization_id: "operator-approval-1".into(),
                operator_id: "operator-1".into(),
                instrument_scope: action.instrument_id.clone(),
                approval_digest: ContentHash::of_bytes(b"approved-test-plan"),
                issued_tick: 0,
                expires_tick: 100,
                revoked: false,
            },
            current_tick: 1,
            maximum_total_risk_milli: 100,
            maximum_duration_ticks: 100,
            minimum_waste_capacity_milli: 100,
        })
        .expect("admitted preflight fixture")
    }

    fn binding_plan() -> (
        InstrumentAction,
        InstrumentPreflightPlan,
        InstrumentProtocolBindingPlan,
    ) {
        let action = action();
        let preflight = admitted_plan(&action);
        let plan =
            compile_glioma_instrument_protocol_binding(&preflight, &[action.clone()], &manifest())
                .expect("compatible command should bind");
        (action, preflight, plan)
    }

    #[test]
    fn binds_admitted_scientific_action_to_exact_versioned_command_and_unit() {
        let (_action, preflight, plan) = binding_plan();
        assert_eq!(plan.feature_id, FEATURE_ID);
        assert_eq!(plan.preflight_digest, preflight.digest);
        assert_eq!(plan.protocol_version, "1.1");
        assert_eq!(plan.action_order, vec!["capture-01"]);
        assert_eq!(
            plan.bindings[0].command.feature_path,
            "org.aurora.ImagingFeature"
        );
        assert_eq!(
            plan.bindings[0].command.parameters,
            vec![BoundInstrumentParameter {
                wire_parameter: "ExposureTime".into(),
                value_milli: 1_250,
                unit: "millisecond_milli".into(),
            }]
        );
        plan.validate().unwrap();
    }

    #[test]
    fn unit_or_command_mismatch_blocks_before_binding() {
        let action = action();
        let preflight = admitted_plan(&action);
        let mut wrong_unit = manifest();
        wrong_unit.commands[0].parameters[0].unit = "second_milli".into();
        assert!(matches!(
            compile_glioma_instrument_protocol_binding(&preflight, &[action.clone()], &wrong_unit),
            Err(InstrumentProtocolBindingError::UnsupportedAction { .. })
        ));
        let mut missing_command = manifest();
        missing_command.commands[0].output_schema = "UnrelatedOutput1@1".into();
        assert!(matches!(
            compile_glioma_instrument_protocol_binding(&preflight, &[action], &missing_command),
            Err(InstrumentProtocolBindingError::UnsupportedAction { .. })
        ));
    }

    struct Gateway {
        manifest: InstrumentProtocolManifest,
        interlocks: InstrumentInterlockSnapshot,
        manifest_reads: u8,
        stale_after_initial_read: bool,
        fail_retryably: bool,
        invoked: Vec<String>,
    }

    impl Gateway {
        fn stable(fail_retryably: bool) -> Self {
            Self {
                manifest: manifest(),
                interlocks: interlocks(),
                manifest_reads: 0,
                stale_after_initial_read: false,
                fail_retryably,
                invoked: Vec::new(),
            }
        }
    }

    impl InstrumentProtocolGateway for Gateway {
        fn observe_interlocks(
            &mut self,
        ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
            Ok(self.interlocks.clone())
        }

        fn verify_authorization(
            &mut self,
            _authorization: &InstrumentAuthorization,
        ) -> Result<(), InstrumentExecutionFailure> {
            Ok(())
        }

        fn current_manifest(
            &mut self,
            instrument_id: &str,
        ) -> Result<InstrumentProtocolManifest, InstrumentExecutionFailure> {
            if instrument_id != self.manifest.instrument_id {
                return Err(InstrumentExecutionFailure {
                    reason: "requested an unknown local instrument".into(),
                    retryable: false,
                });
            }
            self.manifest_reads = self.manifest_reads.saturating_add(1);
            let mut current = self.manifest.clone();
            if self.stale_after_initial_read && self.manifest_reads > 1 {
                current.revision += 1;
            }
            Ok(current)
        }

        fn execute_bound_command(
            &mut self,
            command: &BoundInstrumentCommand,
            _attempt: u8,
        ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
            self.invoked.push(command.command_id.clone());
            if self.fail_retryably {
                return Err(InstrumentExecutionFailure {
                    reason: "transport result is ambiguous".into(),
                    retryable: true,
                });
            }
            Ok(InstrumentExecutionResult {
                action_id: command.action_id.clone(),
                disposition: super::super::execution::InstrumentExecutionDisposition::Completed,
                attempt_count: 1,
                started_tick: Some(1),
                completed_tick: Some(2),
                artifact: None,
                note: "mock device command completed".into(),
                uncertainty: Vec::new(),
                negative_evidence: Vec::new(),
            })
        }

        fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
            Ok(())
        }
    }

    #[test]
    fn executor_refuses_changed_actions_stale_profiles_and_unsafe_retries() {
        let (action, preflight, plan) = binding_plan();
        let mut stale = ProtocolBoundInstrumentExecutor::new(
            plan.clone(),
            &preflight,
            std::slice::from_ref(&action),
            Gateway {
                stale_after_initial_read: true,
                ..Gateway::stable(false)
            },
        )
        .unwrap();
        assert!(stale.execute_action(&action, 1).is_err());
        assert!(stale.gateway().invoked.is_empty());

        let mut changed = ProtocolBoundInstrumentExecutor::new(
            plan.clone(),
            &preflight,
            std::slice::from_ref(&action),
            Gateway::stable(false),
        )
        .unwrap();
        let mut changed_action = action.clone();
        changed_action.parameters[0].value_milli += 1;
        assert!(changed.execute_action(&changed_action, 1).is_err());
        assert!(changed.gateway().invoked.is_empty());

        let mut retry = ProtocolBoundInstrumentExecutor::new(
            plan,
            &preflight,
            std::slice::from_ref(&action),
            Gateway::stable(true),
        )
        .unwrap();
        let failure = retry.execute_action(&action, 1).unwrap_err();
        assert!(!failure.retryable);
        assert_eq!(retry.gateway().invoked, vec!["AcquireImage"]);
    }

    #[test]
    fn executor_rebuilds_binding_against_live_profile_before_accepting_a_serialized_plan() {
        let (action, preflight, mut plan) = binding_plan();
        plan.bindings[0].command.command_id = "UnapprovedCommand".into();
        assert!(ProtocolBoundInstrumentExecutor::new(
            plan,
            &preflight,
            &[action],
            Gateway::stable(false),
        )
        .is_err());
    }

    #[test]
    fn protocol_adapter_runs_inside_the_existing_interlocked_execution_controller() {
        let (action, preflight, plan) = binding_plan();
        let mut gateway = Gateway::stable(true);
        gateway.interlocks = interlocks();
        let mut executor = ProtocolBoundInstrumentExecutor::new(
            plan,
            &preflight,
            std::slice::from_ref(&action),
            gateway,
        )
        .unwrap();
        let run = execute_glioma_instrument_plan(
            &InstrumentExecutionRequest {
                objective: preflight.objective.clone(),
                plan: preflight,
                actions: vec![action],
                authorization: authorization(),
                live_interlocks: interlocks(),
                current_tick: 1,
                minimum_waste_capacity_milli: 100,
                max_retries: 3,
                require_artifacts: false,
            },
            &mut executor,
        )
        .expect("interlocked protocol execution is representable");

        assert_eq!(run.disposition, InstrumentExecutionDisposition::Failed);
        assert_eq!(run.retry_count, 0);
        assert_eq!(run.results[0].attempt_count, 1);
        assert_eq!(executor.gateway().invoked, vec!["AcquireImage"]);
    }

    fn authorization() -> InstrumentAuthorization {
        InstrumentAuthorization {
            authorization_id: "operator-approval-1".into(),
            operator_id: "operator-1".into(),
            instrument_scope: "organoid-imager-1".into(),
            approval_digest: ContentHash::of_bytes(b"approved-test-plan"),
            issued_tick: 0,
            expires_tick: 100,
            revoked: false,
        }
    }

    fn interlocks() -> InstrumentInterlockSnapshot {
        InstrumentInterlockSnapshot {
            observed_tick: 1,
            emergency_stop_clear: true,
            guard_closed: true,
            deck_clear: true,
            consumables_available: true,
            waste_capacity_milli: 1_000,
            temperature_milli: Some(37_000),
            minimum_temperature_milli: Some(36_000),
            maximum_temperature_milli: Some(38_000),
            calibration_valid_until_tick: 100,
            calibration_sequence_index: 3,
        }
    }
}
