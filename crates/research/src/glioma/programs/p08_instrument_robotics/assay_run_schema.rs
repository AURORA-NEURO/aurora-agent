//! Typed assay-run and instrument-result contracts for preclinical glioma workflows.
//!
//! This module is the boundary between an admitted local instrument operation and the
//! downstream ingestion/computation programs.  It deliberately keeps the sample identity
//! opaque, binds the run to the exact preflight and calibration artifacts that admitted it, and
//! preserves every missing, failed, unresolved, or simulation-only channel.  A run record is
//! therefore useful to P03/P09 without allowing a transport success to be mistaken for a
//! measured biological result.

use super::calibration::{CalibrationDisposition, InstrumentCalibration};
use super::execution::{InstrumentExecutionDisposition, InstrumentExecutionRun};
use super::preflight::{
    InstrumentOperation, InstrumentPreflightDisposition, InstrumentPreflightPlan,
};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaAssayRunRecord1@1";
pub const MAX_CHANNELS: usize = 128;
pub const MAX_ACTIONS: usize = 1_024;
pub const MAX_NOTE_LENGTH: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayMaterialKind {
    CellLine,
    Organoid,
    PatientDerivedXenograft,
    MouseTissue,
    ZebrafishTissue,
}

impl AssayMaterialKind {
    fn model_system(self) -> GliomaModelSystem {
        match self {
            Self::CellLine => GliomaModelSystem::CellLine,
            Self::Organoid => GliomaModelSystem::Organoid,
            Self::PatientDerivedXenograft => GliomaModelSystem::PatientDerivedXenograft,
            Self::MouseTissue => GliomaModelSystem::MouseModel,
            Self::ZebrafishTissue => GliomaModelSystem::ZebrafishModel,
        }
    }
}

/// Opaque local sample/material identity.  The token is intentionally not a human identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssaySampleToken {
    pub sample_token: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub material_kind: AssayMaterialKind,
    pub lineage_token: Option<String>,
    pub local_scope: String,
    pub human_origin: bool,
    pub contains_direct_identifiers: bool,
}

impl AssaySampleToken {
    pub(crate) fn validate(&self) -> Result<(), AssayRunSchemaError> {
        if self.sample_token.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.local_scope.trim().is_empty()
            || self.lineage_token.as_deref().is_some_and(str::is_empty)
            || self.human_origin
            || self.contains_direct_identifiers
            || self.model_system == GliomaModelSystem::InSilico
            || self.material_kind.model_system() != self.model_system
        {
            return Err(AssayRunSchemaError::InvalidRequest(
                "sample token must be opaque, non-human, local, and model/material consistent"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayProtocolReference {
    pub protocol_id: String,
    pub protocol_version: String,
    pub protocol_digest: ContentHash,
    pub approved_by: String,
    pub approved_tick: u64,
    pub required_operations: Vec<InstrumentOperation>,
}

impl AssayProtocolReference {
    pub(crate) fn validate(&self) -> Result<(), AssayRunSchemaError> {
        if self.protocol_id.trim().is_empty()
            || self.protocol_version.trim().is_empty()
            || self.approved_by.trim().is_empty()
            || self.approved_tick == 0
            || self.protocol_digest.as_str().len() != 64
            || self.required_operations.is_empty()
            || self
                .required_operations
                .windows(2)
                .any(|pair| pair[0] == pair[1])
        {
            return Err(AssayRunSchemaError::InvalidRequest(
                "approved protocol identity, digest, and unique required operations are required"
                    .into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayChannelSpec {
    pub channel_id: String,
    pub label: String,
    pub modality: GliomaModality,
    pub required: bool,
    pub expected_content_type: String,
    pub unit: String,
    pub exposure_milli: Option<u64>,
}

impl AssayChannelSpec {
    fn validate(&self) -> Result<(), AssayRunSchemaError> {
        if self.channel_id.trim().is_empty()
            || self.label.trim().is_empty()
            || self.expected_content_type.trim().is_empty()
            || self.unit.trim().is_empty()
            || self.exposure_milli.is_some_and(|value| value == 0)
        {
            return Err(AssayRunSchemaError::InvalidRequest(
                "channel identity, content type, unit, and positive exposure are required".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayAcquisitionMode {
    Endpoint,
    TimeLapse,
    ZStack,
    PlateReader,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayAcquisitionConfiguration {
    pub mode: AssayAcquisitionMode,
    pub start_tick: u64,
    pub frame_interval_ticks: u64,
    pub frame_count: u32,
    pub maximum_duration_ticks: u64,
    pub channels: Vec<AssayChannelSpec>,
    pub raw_data_local: bool,
}

impl AssayAcquisitionConfiguration {
    fn validate(&self) -> Result<(), AssayRunSchemaError> {
        if self.start_tick == 0
            || self.frame_interval_ticks == 0
            || self.frame_count == 0
            || self.maximum_duration_ticks == 0
            || self.channels.is_empty()
            || self.channels.len() > MAX_CHANNELS
            || !self.raw_data_local
            || self
                .channels
                .windows(2)
                .any(|pair| pair[0].channel_id >= pair[1].channel_id)
        {
            return Err(AssayRunSchemaError::InvalidRequest(
                "bounded local acquisition with canonical non-empty channels is required".into(),
            ));
        }
        if matches!(
            self.mode,
            AssayAcquisitionMode::Endpoint | AssayAcquisitionMode::PlateReader
        ) && self.frame_count != 1
        {
            return Err(AssayRunSchemaError::InvalidRequest(
                "endpoint and plate-reader acquisitions must contain exactly one frame".into(),
            ));
        }
        let duration =
            u64::from(self.frame_count.saturating_sub(1)).saturating_mul(self.frame_interval_ticks);
        if duration > self.maximum_duration_ticks {
            return Err(AssayRunSchemaError::InvalidRequest(
                "frame cadence exceeds the declared acquisition duration".into(),
            ));
        }
        for channel in &self.channels {
            channel.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayRunSpecRequest {
    pub objective: String,
    pub run_id: String,
    pub sample: AssaySampleToken,
    pub protocol: AssayProtocolReference,
    pub instrument_id: String,
    pub preflight: InstrumentPreflightPlan,
    pub calibration: InstrumentCalibration,
    pub calibration_valid_until_tick: u64,
    pub configuration: AssayAcquisitionConfiguration,
    pub action_operations: Vec<InstrumentOperation>,
    pub current_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayRunSpec {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub run_id: String,
    pub sample: AssaySampleToken,
    pub protocol: AssayProtocolReference,
    pub instrument_id: String,
    pub preflight_digest: ContentHash,
    pub calibration_digest: ContentHash,
    pub calibration_valid_until_tick: u64,
    pub configuration: AssayAcquisitionConfiguration,
    pub action_order: Vec<String>,
    pub action_operations: Vec<InstrumentOperation>,
    pub channel_order: Vec<String>,
    pub expected_artifact_types: Vec<String>,
    pub dispatch_permitted: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayChannelState {
    Measured,
    Missing,
    Failed,
    NotAttempted,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayChannelObservation {
    pub channel_id: String,
    pub state: AssayChannelState,
    pub artifact: Option<LocalArtifactRef>,
    pub started_tick: Option<u64>,
    pub completed_tick: Option<u64>,
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayRunDisposition {
    Completed,
    Negative,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayRunResult {
    pub feature_id: String,
    pub output_schema: String,
    pub run_id: String,
    pub spec_digest: ContentHash,
    pub execution_digest: ContentHash,
    pub instrument_id: String,
    pub sample_token: String,
    pub channel_results: Vec<AssayChannelObservation>,
    pub measured_channel_order: Vec<String>,
    pub missing_channel_order: Vec<String>,
    pub local_artifacts: Vec<LocalArtifactRef>,
    pub observed_start_tick: Option<u64>,
    pub observed_end_tick: Option<u64>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub simulation_only: bool,
    pub disposition: AssayRunDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssayRunSchemaError {
    #[error("assay-run request is invalid: {0}")]
    InvalidRequest(String),
    #[error("assay-run result is invalid: {0}")]
    InvalidResult(String),
    #[error("assay-run digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn spec_digest_input(spec: &AssayRunSpec) -> serde_json::Value {
    serde_json::json!({
        "feature_id": spec.feature_id,
        "output_schema": spec.output_schema,
        "objective": spec.objective,
        "run_id": spec.run_id,
        "sample": spec.sample,
        "protocol": spec.protocol,
        "instrument_id": spec.instrument_id,
        "preflight_digest": spec.preflight_digest,
        "calibration_digest": spec.calibration_digest,
        "calibration_valid_until_tick": spec.calibration_valid_until_tick,
        "configuration": spec.configuration,
        "action_order": spec.action_order,
        "action_operations": spec.action_operations,
        "channel_order": spec.channel_order,
        "expected_artifact_types": spec.expected_artifact_types,
        "dispatch_permitted": spec.dispatch_permitted,
    })
}

fn result_digest_input(result: &AssayRunResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "run_id": result.run_id,
        "spec_digest": result.spec_digest,
        "execution_digest": result.execution_digest,
        "instrument_id": result.instrument_id,
        "sample_token": result.sample_token,
        "channel_results": result.channel_results,
        "measured_channel_order": result.measured_channel_order,
        "missing_channel_order": result.missing_channel_order,
        "local_artifacts": result.local_artifacts,
        "observed_start_tick": result.observed_start_tick,
        "observed_end_tick": result.observed_end_tick,
        "uncertainty": result.uncertainty,
        "negative_evidence": result.negative_evidence,
        "simulation_only": result.simulation_only,
        "disposition": result.disposition,
    })
}

impl AssayRunSpec {
    pub fn validate(&self) -> Result<(), AssayRunSchemaError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.run_id.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.preflight_digest.as_str().len() != 64
            || self.calibration_digest.as_str().len() != 64
            || self.calibration_valid_until_tick == 0
            || self.action_order.is_empty()
            || self.action_order.len() > MAX_ACTIONS
            || self.action_order.windows(2).any(|pair| pair[0] == pair[1])
            || self.action_operations.len() != self.action_order.len()
            || self.channel_order.len() != self.configuration.channels.len()
            || !canonical(&self.channel_order)
            || !canonical(&self.expected_artifact_types)
            || !self.dispatch_permitted
        {
            return Err(AssayRunSchemaError::InvalidResult(
                "assay spec identity, bounded action/channel order, locality, or admission is invalid"
                    .into(),
            ));
        }
        self.sample.validate()?;
        self.protocol.validate()?;
        self.configuration.validate()?;
        if self
            .protocol
            .required_operations
            .iter()
            .any(|operation| !self.action_operations.contains(operation))
        {
            return Err(AssayRunSchemaError::InvalidResult(
                "approved protocol operations are not covered by the admitted action operations"
                    .into(),
            ));
        }
        let expected_channels = self
            .configuration
            .channels
            .iter()
            .map(|channel| channel.channel_id.clone())
            .collect::<Vec<_>>();
        if expected_channels != self.channel_order
            || self.expected_artifact_types
                != self
                    .configuration
                    .channels
                    .iter()
                    .map(|channel| channel.expected_content_type.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
        {
            return Err(AssayRunSchemaError::InvalidResult(
                "channel and artifact declarations do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&spec_digest_input(self))
            .map_err(|error| AssayRunSchemaError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AssayRunSchemaError::InvalidResult(
                "assay spec digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

impl AssayRunResult {
    pub fn validate(&self) -> Result<(), AssayRunSchemaError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.run_id.trim().is_empty()
            || self.spec_digest.as_str().len() != 64
            || self.execution_digest.as_str().len() != 64
            || self.instrument_id.trim().is_empty()
            || self.sample_token.trim().is_empty()
            || self.channel_results.is_empty()
            || self.channel_results.windows(2).any(|pair| {
                pair[0].channel_id >= pair[1].channel_id
                    || pair[0].channel_id.trim().is_empty()
                    || pair[1].channel_id.trim().is_empty()
            })
            || self
                .channel_results
                .iter()
                .any(|observation| observation.channel_id.trim().is_empty())
            || !canonical(&self.measured_channel_order)
            || !canonical(&self.missing_channel_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self
                .channel_results
                .iter()
                .any(|observation| observation.note.trim().is_empty())
            || self.channel_results.iter().any(|observation| {
                observation
                    .artifact
                    .as_ref()
                    .is_some_and(|artifact| artifact.validate().is_err())
            })
            || self
                .observed_start_tick
                .zip(self.observed_end_tick)
                .is_some_and(|(start, end)| end < start)
        {
            return Err(AssayRunSchemaError::InvalidResult(
                "result identity, channel ordering, artifact locality, timing, or evidence ordering is invalid"
                    .into(),
            ));
        }
        let expected_measured = self
            .channel_results
            .iter()
            .filter(|observation| observation.state == AssayChannelState::Measured)
            .map(|observation| observation.channel_id.clone())
            .collect::<Vec<_>>();
        if expected_measured != self.measured_channel_order {
            return Err(AssayRunSchemaError::InvalidResult(
                "measured channel partition does not reconcile".into(),
            ));
        }
        let expected_missing = self
            .channel_results
            .iter()
            .filter(|observation| observation.state != AssayChannelState::Measured)
            .map(|observation| observation.channel_id.clone())
            .collect::<Vec<_>>();
        if expected_missing != self.missing_channel_order {
            return Err(AssayRunSchemaError::InvalidResult(
                "missing channel partition does not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&result_digest_input(self))
            .map_err(|error| AssayRunSchemaError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AssayRunSchemaError::InvalidResult(
                "assay result digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

pub fn compile_glioma_assay_run_spec(
    request: &AssayRunSpecRequest,
) -> Result<AssayRunSpec, AssayRunSchemaError> {
    if request.objective.trim().is_empty()
        || request.run_id.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.current_tick == 0
        || request.calibration_valid_until_tick < request.current_tick
        || request.preflight.instrument_id != request.instrument_id
        || request.preflight.model_system != request.sample.model_system
        || request.preflight.disposition != InstrumentPreflightDisposition::Admitted
        || !request.preflight.dispatch_permitted
        || request.preflight.action_order != request.preflight.admitted_order
        || request.action_operations.len() != request.preflight.action_order.len()
        || request.calibration.instrument_id != request.instrument_id
        || request.calibration.model_system != request.sample.model_system
        || request.calibration.disposition != CalibrationDisposition::Qualified
        || request.preflight.action_order.len() > MAX_ACTIONS
    {
        return Err(AssayRunSchemaError::InvalidRequest(
            "only an admitted, complete preflight with qualified matching calibration may become an assay spec"
                .into(),
        ));
    }
    request.sample.validate()?;
    request.protocol.validate()?;
    request.configuration.validate()?;
    if request
        .protocol
        .required_operations
        .iter()
        .any(|operation| !request.action_operations.contains(operation))
    {
        return Err(AssayRunSchemaError::InvalidRequest(
            "required protocol operations must be represented by the admitted action operations"
                .into(),
        ));
    }
    // Operation-level capability matching is performed by P08 protocol binding.  This schema
    // intentionally carries the approved operation declaration forward rather than attempting
    // to infer an operation from opaque preflight action IDs.
    let channel_order = request
        .configuration
        .channels
        .iter()
        .map(|channel| channel.channel_id.clone())
        .collect::<Vec<_>>();
    let mut expected_artifact_types = request
        .configuration
        .channels
        .iter()
        .map(|channel| channel.expected_content_type.clone())
        .collect::<Vec<_>>();
    expected_artifact_types.sort();
    expected_artifact_types.dedup();
    let mut spec = AssayRunSpec {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        run_id: request.run_id.clone(),
        sample: request.sample.clone(),
        protocol: request.protocol.clone(),
        instrument_id: request.instrument_id.clone(),
        preflight_digest: request.preflight.digest.clone(),
        calibration_digest: request.calibration.digest.clone(),
        calibration_valid_until_tick: request.calibration_valid_until_tick,
        configuration: request.configuration.clone(),
        action_order: request.preflight.action_order.clone(),
        action_operations: request.action_operations.clone(),
        channel_order,
        expected_artifact_types,
        dispatch_permitted: true,
        digest: ContentHash::of_bytes(b"unsealed-glioma-assay-spec"),
    };
    spec.digest = ContentHash::of_value(&spec_digest_input(&spec))
        .map_err(|error| AssayRunSchemaError::Digest(error.to_string()))?;
    spec.validate()?;
    Ok(spec)
}

pub fn record_glioma_assay_run_result(
    spec: &AssayRunSpec,
    execution: &InstrumentExecutionRun,
    mut observations: Vec<AssayChannelObservation>,
    uncertainty: Vec<String>,
    negative_evidence: Vec<String>,
) -> Result<AssayRunResult, AssayRunSchemaError> {
    spec.validate()?;
    execution
        .validate()
        .map_err(|error| AssayRunSchemaError::InvalidResult(error.to_string()))?;
    if execution.plan_digest != spec.preflight_digest
        || execution.instrument_id != spec.instrument_id
        || execution.action_order != spec.action_order
        || observations.len() != spec.configuration.channels.len()
    {
        return Err(AssayRunSchemaError::InvalidResult(
            "execution and observations do not cover the admitted assay specification".into(),
        ));
    }
    observations.sort_by(|left, right| left.channel_id.cmp(&right.channel_id));
    let expected_ids = spec.channel_order.iter().cloned().collect::<BTreeSet<_>>();
    let observed_ids = observations
        .iter()
        .map(|observation| observation.channel_id.clone())
        .collect::<BTreeSet<_>>();
    if expected_ids != observed_ids
        || observations.iter().any(|observation| {
            observation.note.trim().is_empty()
                || (observation.state == AssayChannelState::Measured
                    && observation.artifact.is_none())
                || (observation.state != AssayChannelState::Measured
                    && observation.artifact.is_some())
                || observation
                    .started_tick
                    .zip(observation.completed_tick)
                    .is_some_and(|(start, end)| end < start)
        })
    {
        return Err(AssayRunSchemaError::InvalidResult(
            "every declared channel must have one explicit state with a matching local artifact"
                .into(),
        ));
    }
    let channel_by_id = spec
        .configuration
        .channels
        .iter()
        .map(|channel| (channel.channel_id.as_str(), channel))
        .collect::<std::collections::BTreeMap<_, _>>();
    for observation in &observations {
        let channel = channel_by_id[observation.channel_id.as_str()];
        if let Some(artifact) = &observation.artifact {
            if artifact.content_type != channel.expected_content_type {
                return Err(AssayRunSchemaError::InvalidResult(format!(
                    "channel {} artifact content type does not match its declared schema",
                    observation.channel_id
                )));
            }
        }
    }
    let measured = observations
        .iter()
        .filter(|observation| observation.state == AssayChannelState::Measured)
        .map(|observation| observation.channel_id.clone())
        .collect::<Vec<_>>();
    let missing = observations
        .iter()
        .filter(|observation| observation.state != AssayChannelState::Measured)
        .map(|observation| observation.channel_id.clone())
        .collect::<Vec<_>>();
    let required_missing = observations.iter().any(|observation| {
        observation.state != AssayChannelState::Measured
            && channel_by_id[observation.channel_id.as_str()].required
    });
    let any_measured = !measured.is_empty();
    let has_explicit_missing = observations.iter().any(|observation| {
        matches!(
            observation.state,
            AssayChannelState::Missing
                | AssayChannelState::Failed
                | AssayChannelState::NotAttempted
        )
    });
    let has_unresolved_channel = observations
        .iter()
        .any(|observation| observation.state == AssayChannelState::Unresolved);
    let disposition = if required_missing {
        match execution.disposition {
            InstrumentExecutionDisposition::Blocked | InstrumentExecutionDisposition::Failed => {
                if any_measured {
                    AssayRunDisposition::Partial
                } else {
                    AssayRunDisposition::Blocked
                }
            }
            InstrumentExecutionDisposition::Unresolved => {
                if has_explicit_missing || any_measured {
                    AssayRunDisposition::Partial
                } else {
                    AssayRunDisposition::Unresolved
                }
            }
            InstrumentExecutionDisposition::Partial => {
                if has_unresolved_channel && !has_explicit_missing && !any_measured {
                    AssayRunDisposition::Unresolved
                } else {
                    AssayRunDisposition::Partial
                }
            }
            _ => AssayRunDisposition::Partial,
        }
    } else {
        match execution.disposition {
            InstrumentExecutionDisposition::Completed => AssayRunDisposition::Completed,
            InstrumentExecutionDisposition::Negative => AssayRunDisposition::Negative,
            InstrumentExecutionDisposition::Blocked | InstrumentExecutionDisposition::Failed => {
                AssayRunDisposition::Partial
            }
            InstrumentExecutionDisposition::Unresolved
            | InstrumentExecutionDisposition::Partial => AssayRunDisposition::Partial,
        }
    };
    let mut uncertainty = uncertainty;
    uncertainty.extend(execution.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let mut negative_evidence = negative_evidence;
    negative_evidence.extend(execution.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let local_artifacts = observations
        .iter()
        .filter_map(|observation| observation.artifact.clone())
        .collect::<Vec<_>>();
    let observed_start_tick = observations
        .iter()
        .filter_map(|observation| observation.started_tick)
        .min();
    let observed_end_tick = observations
        .iter()
        .filter_map(|observation| observation.completed_tick)
        .max();
    let mut result = AssayRunResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        run_id: spec.run_id.clone(),
        spec_digest: spec.digest.clone(),
        execution_digest: execution.digest.clone(),
        instrument_id: spec.instrument_id.clone(),
        sample_token: spec.sample.sample_token.clone(),
        channel_results: observations,
        measured_channel_order: measured,
        missing_channel_order: missing,
        local_artifacts,
        observed_start_tick,
        observed_end_tick,
        uncertainty,
        negative_evidence,
        simulation_only: execution.negative_evidence.iter().any(|item| {
            item == "simulation-only-operation-is-not-biological-evidence"
                || item == "synthetic-operation-is-not-biological-evidence"
        }),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-assay-result"),
    };
    result.digest = ContentHash::of_value(&result_digest_input(&result))
        .map_err(|error| AssayRunSchemaError::Digest(error.to_string()))?;
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationRequest, CalibrationRun,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        preflight_glioma_instrument, InstrumentAction, InstrumentAuthorization,
        InstrumentInterlockSnapshot, InstrumentPreflightRequest,
    };

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn sample() -> AssaySampleToken {
        AssaySampleToken {
            sample_token: "sample-local-001".into(),
            study_id: "study-invasion-01".into(),
            model_system: GliomaModelSystem::Organoid,
            material_kind: AssayMaterialKind::Organoid,
            lineage_token: Some("lineage-a".into()),
            local_scope: "site-a".into(),
            human_origin: false,
            contains_direct_identifiers: false,
        }
    }

    fn calibration() -> InstrumentCalibration {
        let runs = (1..=3)
            .map(|index| CalibrationRun {
                run_id: format!("cal-{index}"),
                sequence_index: index,
                batch_id: format!("batch-{index}"),
                instrument_id: "imager-1".into(),
                metric_name: "control".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 500 + i64::from(index),
                expected_milli: 500,
                artifact: LocalArtifactRef {
                    artifact_id: format!("cal-artifact-{index}"),
                    content_hash: hash(&format!("cal-artifact-{index}")),
                    content_type: "application/vnd.aurora.glioma-control+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
            .collect::<Vec<_>>();
        analyze_instrument_calibration(
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
        .unwrap()
    }

    fn action(id: &str, operation: InstrumentOperation, start: u64) -> InstrumentAction {
        InstrumentAction {
            action_id: id.into(),
            instrument_id: "imager-1".into(),
            operation,
            model_system: GliomaModelSystem::Organoid,
            requested_start_tick: start,
            duration_ticks: 2,
            risk_milli: 100,
            requires_operator: false,
            output_schema: format!("{id}1@1"),
            parameters: Vec::new(),
        }
    }

    fn preflight() -> InstrumentPreflightPlan {
        preflight_glioma_instrument(&InstrumentPreflightRequest {
            objective: "capture organoid invasion".into(),
            instrument_id: "imager-1".into(),
            model_system: GliomaModelSystem::Organoid,
            actions: vec![action("capture", InstrumentOperation::AcquireImage, 1)],
            calibration: calibration(),
            interlocks: InstrumentInterlockSnapshot {
                observed_tick: 1,
                emergency_stop_clear: true,
                guard_closed: true,
                deck_clear: true,
                consumables_available: true,
                waste_capacity_milli: 100_000,
                temperature_milli: Some(37_000),
                minimum_temperature_milli: Some(36_000),
                maximum_temperature_milli: Some(38_000),
                calibration_valid_until_tick: 100,
                calibration_sequence_index: 3,
            },
            authorization: InstrumentAuthorization {
                authorization_id: "operator-approval".into(),
                operator_id: "operator-1".into(),
                instrument_scope: "imager-1".into(),
                approval_digest: hash("approval"),
                issued_tick: 0,
                expires_tick: 100,
                revoked: false,
            },
            current_tick: 1,
            maximum_total_risk_milli: 500,
            maximum_duration_ticks: 20,
            minimum_waste_capacity_milli: 100,
        })
        .unwrap()
    }

    fn spec() -> AssayRunSpec {
        compile_glioma_assay_run_spec(&AssayRunSpecRequest {
            objective: "measure invasion-front morphology".into(),
            run_id: "run-001".into(),
            sample: sample(),
            protocol: AssayProtocolReference {
                protocol_id: "invasion-imaging".into(),
                protocol_version: "2.1.0".into(),
                protocol_digest: hash("protocol"),
                approved_by: "methods-board".into(),
                approved_tick: 1,
                required_operations: vec![InstrumentOperation::AcquireImage],
            },
            instrument_id: "imager-1".into(),
            preflight: preflight(),
            calibration: calibration(),
            calibration_valid_until_tick: 100,
            action_operations: vec![InstrumentOperation::AcquireImage],
            configuration: AssayAcquisitionConfiguration {
                mode: AssayAcquisitionMode::Endpoint,
                start_tick: 1,
                frame_interval_ticks: 1,
                frame_count: 1,
                maximum_duration_ticks: 2,
                channels: vec![AssayChannelSpec {
                    channel_id: "morphology".into(),
                    label: "phase contrast".into(),
                    modality: GliomaModality::Imaging,
                    required: true,
                    expected_content_type: "application/vnd.aurora.glioma.image+ome-ngff".into(),
                    unit: "pixel".into(),
                    exposure_milli: Some(10),
                }],
                raw_data_local: true,
            },
            current_tick: 1,
        })
        .unwrap()
    }

    fn execution(spec: &AssayRunSpec) -> InstrumentExecutionRun {
        let mut run = InstrumentExecutionRun {
            feature_id: super::super::execution::FEATURE_ID.into(),
            output_schema: super::super::execution::OUTPUT_SCHEMA.into(),
            objective: spec.objective.clone(),
            plan_digest: spec.preflight_digest.clone(),
            instrument_id: spec.instrument_id.clone(),
            action_order: spec.action_order.clone(),
            results: vec![super::super::execution::InstrumentExecutionResult {
                action_id: "capture".into(),
                disposition: InstrumentExecutionDisposition::Completed,
                attempt_count: 1,
                started_tick: Some(1),
                completed_tick: Some(2),
                artifact: Some(LocalArtifactRef {
                    artifact_id: "transport-artifact".into(),
                    content_hash: hash("transport"),
                    content_type: "application/vnd.aurora.glioma.instrument-operation+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }),
                note: "local dry-run operation".into(),
                uncertainty: Vec::new(),
                negative_evidence: Vec::new(),
            }],
            completed_order: vec!["capture".into()],
            negative_order: Vec::new(),
            partial_order: Vec::new(),
            failed_order: Vec::new(),
            unresolved_order: Vec::new(),
            skipped_order: Vec::new(),
            retry_count: 0,
            emergency_stop_requested: false,
            emergency_stop_succeeded: false,
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            disposition: InstrumentExecutionDisposition::Completed,
            stop_reason: super::super::execution::InstrumentExecutionStopReason::Completed,
            digest: hash("placeholder"),
        };
        run.digest = ContentHash::of_value(&super::super::execution::digest_input(&run)).unwrap();
        run
    }

    fn observation(state: AssayChannelState) -> AssayChannelObservation {
        AssayChannelObservation {
            channel_id: "morphology".into(),
            state,
            artifact: (state == AssayChannelState::Measured).then(|| LocalArtifactRef {
                artifact_id: "image-001".into(),
                content_hash: hash("image"),
                content_type: "application/vnd.aurora.glioma.image+ome-ngff".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            started_tick: Some(1),
            completed_tick: Some(2),
            note: "channel outcome recorded".into(),
        }
    }

    #[test]
    fn admitted_spec_is_digest_bound_and_round_trips() {
        let spec = spec();
        spec.validate().unwrap();
        let encoded = serde_json::to_vec(&spec).unwrap();
        let decoded: AssayRunSpec = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, spec);
    }

    #[test]
    fn mismatched_sample_or_calibration_cannot_be_admitted() {
        let mut request = AssayRunSpecRequest {
            objective: "measure invasion-front morphology".into(),
            run_id: "run-001".into(),
            sample: sample(),
            protocol: AssayProtocolReference {
                protocol_id: "invasion-imaging".into(),
                protocol_version: "2.1.0".into(),
                protocol_digest: hash("protocol"),
                approved_by: "methods-board".into(),
                approved_tick: 1,
                required_operations: vec![InstrumentOperation::AcquireImage],
            },
            instrument_id: "imager-1".into(),
            preflight: preflight(),
            calibration: calibration(),
            calibration_valid_until_tick: 100,
            action_operations: vec![InstrumentOperation::AcquireImage],
            configuration: spec().configuration,
            current_tick: 1,
        };
        request.sample.model_system = GliomaModelSystem::MouseModel;
        assert!(compile_glioma_assay_run_spec(&request).is_err());
    }

    #[test]
    fn completed_result_preserves_local_measurement() {
        let spec = spec();
        let result = record_glioma_assay_run_result(
            &spec,
            &execution(&spec),
            vec![observation(AssayChannelState::Measured)],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(result.disposition, AssayRunDisposition::Completed);
        assert_eq!(result.measured_channel_order, vec!["morphology"]);
        assert!(!result.simulation_only);
        result.validate().unwrap();
    }

    #[test]
    fn missing_channel_is_partial_and_never_imputed() {
        let spec = spec();
        let result = record_glioma_assay_run_result(
            &spec,
            &execution(&spec),
            vec![observation(AssayChannelState::Missing)],
            Vec::new(),
            vec!["channel-dropout".into()],
        )
        .unwrap();
        assert_eq!(result.disposition, AssayRunDisposition::Partial);
        assert_eq!(result.missing_channel_order, vec!["morphology"]);
        assert!(result.local_artifacts.is_empty());
        assert!(result
            .negative_evidence
            .iter()
            .any(|item| item == "channel-dropout"));
    }
}
