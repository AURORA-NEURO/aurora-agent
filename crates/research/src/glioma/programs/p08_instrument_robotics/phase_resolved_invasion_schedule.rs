//! Designs time-lapse sampling for phase-resolved preclinical glioma invasion assays.
//!
//! A validated reporter and a local pilot provide conservative phase-dwell and motility
//! timescale summaries. This planner compares only device-validated acquisition profiles and
//! selects a cadence that balances the chance of observing short phases, resolvable movement,
//! reporter/tracking quality, and calibrated illumination dose. Its fixed-point score is a
//! scheduling proxy, not a biological result or a claim that migration and proliferation are
//! causally coupled. A selected profile still passes through P08 protocol compilation and the
//! normal authorization/interlock path before any instrument is run.

use super::execution::MAX_ACTIONS;
use super::preflight::{InstrumentAction, InstrumentOperation, MAX_TICK};
use crate::glioma::programs::p03_multimodal_ingestion_qc::GliomaMicroscopyMaterial;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaPhaseResolvedInvasionSchedule1@1";
pub const ACTION_PLAN_SCHEMA: &str = "GliomaPhaseResolvedInvasionActionPlan1@1";
pub const MAX_PHASES: usize = 8;
pub const MAX_PROFILES: usize = 256;
pub const MAX_TIME_MILLIS: u64 = 1_000_000_000_000;
pub const MIN_INDEPENDENT_UNITS: u16 = 3;

/// Conservative lower-tail phase duration measured in an independent, local pilot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellCyclePhaseDwellPrior {
    pub phase_id: String,
    pub conservative_dwell_millis: u64,
    pub independent_unit_count: u16,
    pub source_digest: ContentHash,
}

/// Motion-resolution quantities estimated from independent preclinical pilot tracks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMotilityTimescalePrior {
    /// Time over which the pilot's directionality remains measurably persistent.
    pub persistence_time_millis: u64,
    /// Median displacement over one persistence time, in 1/1,000 micrometres.
    pub displacement_per_persistence_micrometre_milli: u64,
    /// Localization error of the validated track pipeline, in 1/1,000 micrometres.
    pub localization_error_micrometre_milli: u64,
    pub independent_unit_count: u16,
    pub source_digest: ContentHash,
}

/// One acquisition bundle already validated for a named instrument and protocol.
///
/// Dose uses the device calibration's integer unit, so candidates from different calibration
/// versions must not be mixed in the same request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedAcquisitionProfile {
    pub profile_id: String,
    pub instrument_id: String,
    pub device_profile_id: String,
    pub protocol_profile_id: String,
    pub capability_manifest_digest: ContentHash,
    pub dose_calibration_digest: ContentHash,
    pub capture_interval_millis: u64,
    pub dose_units_per_capture: u64,
    pub maximum_assay_duration_millis: u64,
    pub maximum_capture_count: u32,
    /// Local pilot's reporter-state calling quality for this channel/cadence bundle.
    pub reporter_quality_permille: u16,
    /// Local pilot's track recovery quality for this channel/cadence bundle.
    pub tracking_quality_permille: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedInvasionScheduleRequest {
    pub study_id: String,
    pub material: GliomaMicroscopyMaterial,
    pub assay_duration_millis: u64,
    pub maximum_total_dose_units: u64,
    pub minimum_independent_units: u16,
    pub minimum_phase_observability_permille: u16,
    pub minimum_joint_resolution_permille: u16,
    /// Strictly sorted by phase_id; phase labels are defined by the validated reporter assay.
    pub phase_dwell_priors: Vec<CellCyclePhaseDwellPrior>,
    pub motility_prior: GliomaMotilityTimescalePrior,
    /// Strictly sorted by profile_id; profiles are produced by the local protocol/device layer.
    pub acquisition_profiles: Vec<PhaseResolvedAcquisitionProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseResolvedProfileDisposition {
    Feasible,
    DoseBudgetExceeded,
    CaptureCountExceeded,
    AssayDurationUnsupported,
    PhaseObservabilityBelowFloor,
    JointResolutionBelowFloor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseSamplingResolution {
    pub phase_id: String,
    /// Lower-bound dwell divided by the capture interval, capped at 1,000; a coverage proxy.
    pub observability_proxy_permille: u16,
    /// Product of observability, movement resolution, reporter quality, and tracking quality.
    pub joint_resolution_permille: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedProfileScore {
    pub profile_id: String,
    pub instrument_id: String,
    pub device_profile_id: String,
    pub protocol_profile_id: String,
    pub capability_manifest_digest: ContentHash,
    pub dose_calibration_digest: ContentHash,
    pub capture_interval_millis: u64,
    pub capture_count: u64,
    pub dose_units_per_capture: u64,
    pub total_dose_units: u64,
    pub movement_resolution_proxy_permille: u16,
    pub phase_scores: Vec<PhaseSamplingResolution>,
    pub mean_joint_resolution_permille: u16,
    pub worst_phase_joint_resolution_permille: u16,
    /// 70% macro-average phase score plus 30% worst-phase score.
    pub balanced_resolution_score_permille: u16,
    pub disposition: PhaseResolvedProfileDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseResolvedScheduleDisposition {
    Selected,
    NoFeasibleProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedInvasionSchedule {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub material: GliomaMicroscopyMaterial,
    pub phase_order: Vec<String>,
    pub profile_order: Vec<String>,
    pub ranked_profiles: Vec<PhaseResolvedProfileScore>,
    pub selected_profile_id: Option<String>,
    pub disposition: PhaseResolvedScheduleDisposition,
    pub scoring_method: String,
    pub limitations: Vec<String>,
    pub digest: ContentHash,
}

/// Local, version-pinned template used to turn a selected cadence into P08 image actions.
///
/// The resulting actions remain subject to P08 calibration, operator authorization, interlocks,
/// protocol binding, and execution checks; compiling a schedule grants no execution authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedInstrumentActionTemplate {
    pub profile_id: String,
    pub instrument_id: String,
    pub device_profile_id: String,
    pub protocol_profile_id: String,
    pub capability_manifest_digest: ContentHash,
    pub dose_calibration_digest: ContentHash,
    pub dose_units_per_capture: u64,
    pub first_capture_tick: u64,
    pub ticks_per_millisecond: u64,
    pub action_template: InstrumentAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseResolvedInvasionActionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub schedule_digest: ContentHash,
    pub study_id: String,
    pub profile_id: String,
    pub action_id_prefix: String,
    pub capability_manifest_digest: ContentHash,
    pub dose_calibration_digest: ContentHash,
    pub instrument_id: String,
    pub model_system: crate::glioma_engine::GliomaModelSystem,
    pub action_order: Vec<String>,
    pub actions: Vec<InstrumentAction>,
    pub capture_interval_ticks: u64,
    pub assay_duration_ticks: u64,
    pub first_capture_tick: u64,
    pub final_capture_tick: u64,
    pub completion_tick: u64,
    pub dose_units_per_capture: u64,
    pub total_dose_units: u64,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PhaseResolvedScheduleError {
    #[error("phase-resolved invasion schedule request is invalid: {0}")]
    InvalidRequest(String),
    #[error("no feasible acquisition profile can be compiled into instrument actions")]
    NoFeasibleProfile,
    #[error("phase-resolved invasion schedule digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &PhaseResolvedInvasionSchedule) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "material": output.material,
        "phase_order": output.phase_order,
        "profile_order": output.profile_order,
        "ranked_profiles": output.ranked_profiles,
        "selected_profile_id": output.selected_profile_id,
        "disposition": output.disposition,
        "scoring_method": output.scoring_method,
        "limitations": output.limitations,
    })
}

impl PhaseResolvedInvasionSchedule {
    pub fn verify_digest(&self) -> Result<(), PhaseResolvedScheduleError> {
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| PhaseResolvedScheduleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(PhaseResolvedScheduleError::Digest(
                "schedule digest does not match its contents".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn action_plan_digest_input(
    plan: &PhaseResolvedInvasionActionPlan,
) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "schedule_digest": plan.schedule_digest,
        "study_id": plan.study_id,
        "profile_id": plan.profile_id,
        "action_id_prefix": plan.action_id_prefix,
        "capability_manifest_digest": plan.capability_manifest_digest,
        "dose_calibration_digest": plan.dose_calibration_digest,
        "instrument_id": plan.instrument_id,
        "model_system": plan.model_system,
        "action_order": plan.action_order,
        "actions": plan.actions,
        "capture_interval_ticks": plan.capture_interval_ticks,
        "assay_duration_ticks": plan.assay_duration_ticks,
        "first_capture_tick": plan.first_capture_tick,
        "final_capture_tick": plan.final_capture_tick,
        "completion_tick": plan.completion_tick,
        "dose_units_per_capture": plan.dose_units_per_capture,
        "total_dose_units": plan.total_dose_units,
    })
}

impl PhaseResolvedInvasionActionPlan {
    /// Reconcile the schedule's action sequence, cadence, model, and dose before P08 consumes it.
    pub fn validate(&self) -> Result<(), PhaseResolvedScheduleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != ACTION_PLAN_SCHEMA
            || self.schedule_digest.as_str().len() != 64
            || self.capability_manifest_digest.as_str().len() != 64
            || self.dose_calibration_digest.as_str().len() != 64
            || self.study_id.trim().is_empty()
            || self.profile_id.trim().is_empty()
            || self.action_id_prefix.trim().is_empty()
            || self.instrument_id.trim().is_empty()
            || self.actions.is_empty()
            || self.actions.len() > MAX_ACTIONS
            || self.actions.len() != self.action_order.len()
            || self.capture_interval_ticks == 0
            || self.assay_duration_ticks == 0
            || self.dose_units_per_capture == 0
            || self.first_capture_tick > self.final_capture_tick
            || self.completion_tick > MAX_TICK
        {
            return Err(invalid(
                "compiled plan identity, bounds, or non-empty contract fields are invalid",
            ));
        }
        let expected_final = self
            .first_capture_tick
            .checked_add(self.assay_duration_ticks)
            .ok_or_else(|| invalid("assay completion tick overflows"))?;
        let expected_dose = (self.actions.len() as u64)
            .checked_mul(self.dose_units_per_capture)
            .ok_or_else(|| invalid("compiled dose total overflows"))?;
        if expected_final != self.final_capture_tick
            || expected_dose != self.total_dose_units
            || self.final_capture_tick > MAX_TICK
        {
            return Err(invalid(
                "compiled plan final capture or dose total does not reconcile",
            ));
        }
        for (index, (action_id, action)) in self.action_order.iter().zip(&self.actions).enumerate()
        {
            let expected_id = format!("{}-capture-{:04}", self.action_id_prefix, index + 1);
            let expected_start = self
                .capture_interval_ticks
                .checked_mul(index as u64)
                .and_then(|offset| {
                    self.first_capture_tick
                        .checked_add(offset.min(self.assay_duration_ticks))
                })
                .ok_or_else(|| invalid("compiled capture start tick overflows"))?;
            if action_id != &expected_id
                || action.action_id != expected_id
                || action.instrument_id != self.instrument_id
                || action.model_system != self.model_system
                || action.operation != InstrumentOperation::AcquireImage
                || !action.requires_operator
                || action.requested_start_tick != expected_start
                || action.duration_ticks == 0
                || action.output_schema.trim().is_empty()
                || action.requested_start_tick > MAX_TICK
                || action
                    .requested_start_tick
                    .saturating_add(action.duration_ticks)
                    > MAX_TICK
            {
                return Err(invalid(
                    "compiled actions do not match the ordered, operator-gated cadence contract",
                ));
            }
        }
        let last_action = self.actions.last().expect("non-empty actions were checked");
        if last_action.requested_start_tick != self.final_capture_tick
            || last_action
                .requested_start_tick
                .saturating_add(last_action.duration_ticks)
                != self.completion_tick
        {
            return Err(invalid(
                "compiled completion tick does not follow the final capture action",
            ));
        }
        Ok(())
    }

    pub fn verify_digest(&self) -> Result<(), PhaseResolvedScheduleError> {
        self.validate()?;
        let expected = ContentHash::of_value(&action_plan_digest_input(self))
            .map_err(|error| PhaseResolvedScheduleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(PhaseResolvedScheduleError::Digest(
                "instrument action plan digest does not match its contents".into(),
            ));
        }
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> PhaseResolvedScheduleError {
    PhaseResolvedScheduleError::InvalidRequest(message.into())
}

fn validate_request(
    request: &PhaseResolvedInvasionScheduleRequest,
) -> Result<(), PhaseResolvedScheduleError> {
    if request.study_id.trim().is_empty()
        || request.assay_duration_millis == 0
        || request.assay_duration_millis > MAX_TIME_MILLIS
        || request.maximum_total_dose_units == 0
        || request.minimum_independent_units < MIN_INDEPENDENT_UNITS
        || !(2..=MAX_PHASES).contains(&request.phase_dwell_priors.len())
        || request.acquisition_profiles.is_empty()
        || request.acquisition_profiles.len() > MAX_PROFILES
        || request.minimum_phase_observability_permille == 0
        || request.minimum_phase_observability_permille > 1_000
        || request.minimum_joint_resolution_permille == 0
        || request.minimum_joint_resolution_permille > 1_000
    {
        return Err(invalid(
            "study, duration, budgets, thresholds, or bounded profile counts are invalid",
        ));
    }
    if !request
        .phase_dwell_priors
        .windows(2)
        .all(|pair| pair[0].phase_id < pair[1].phase_id)
    {
        return Err(invalid(
            "phase priors must have unique, strictly sorted phase IDs",
        ));
    }
    for phase in &request.phase_dwell_priors {
        if phase.phase_id.trim().is_empty()
            || phase.conservative_dwell_millis == 0
            || phase.conservative_dwell_millis > MAX_TIME_MILLIS
            || phase.independent_unit_count < request.minimum_independent_units
            || phase.source_digest.as_str().len() != 64
        {
            return Err(invalid(format!(
                "phase {} lacks a bounded dwell estimate from enough independent units",
                phase.phase_id
            )));
        }
    }
    let motion = &request.motility_prior;
    if motion.persistence_time_millis == 0
        || motion.persistence_time_millis > MAX_TIME_MILLIS
        || motion.displacement_per_persistence_micrometre_milli == 0
        || motion.displacement_per_persistence_micrometre_milli > 1_000_000_000_000
        || motion.localization_error_micrometre_milli == 0
        || motion.localization_error_micrometre_milli > 1_000_000_000_000
        || motion.independent_unit_count < request.minimum_independent_units
        || motion.source_digest.as_str().len() != 64
    {
        return Err(invalid(
            "motility prior lacks bounded persistence, localization, or independent-unit evidence",
        ));
    }
    if !request
        .acquisition_profiles
        .windows(2)
        .all(|pair| pair[0].profile_id < pair[1].profile_id)
    {
        return Err(invalid(
            "acquisition profiles must have unique, strictly sorted profile IDs",
        ));
    }
    for profile in &request.acquisition_profiles {
        if profile.profile_id.trim().is_empty()
            || profile.instrument_id.trim().is_empty()
            || profile.device_profile_id.trim().is_empty()
            || profile.protocol_profile_id.trim().is_empty()
            || profile.capability_manifest_digest.as_str().len() != 64
            || profile.dose_calibration_digest.as_str().len() != 64
            || profile.capture_interval_millis == 0
            || profile.capture_interval_millis > MAX_TIME_MILLIS
            || profile.dose_units_per_capture == 0
            || profile.maximum_assay_duration_millis == 0
            || profile.maximum_assay_duration_millis > MAX_TIME_MILLIS
            || profile.maximum_capture_count == 0
            || profile.reporter_quality_permille > 1_000
            || profile.tracking_quality_permille > 1_000
        {
            return Err(invalid(format!(
                "profile {} has invalid device, calibration, cadence, dose, or quality values",
                profile.profile_id
            )));
        }
    }
    let dose_calibration_digest = &request.acquisition_profiles[0].dose_calibration_digest;
    if request
        .acquisition_profiles
        .iter()
        .any(|profile| &profile.dose_calibration_digest != dose_calibration_digest)
    {
        return Err(invalid(
            "profiles with different dose-calibration versions cannot be compared in one ranking",
        ));
    }
    Ok(())
}

fn ratio_permille(numerator: u128, denominator: u128) -> u16 {
    if denominator == 0 {
        return 1_000;
    }
    ((numerator.saturating_mul(1_000) / denominator).min(1_000)) as u16
}

fn movement_resolution_proxy(interval_millis: u64, prior: &GliomaMotilityTimescalePrior) -> u16 {
    let interval = interval_millis as u128;
    let persistence = prior.persistence_time_millis as u128;
    let displacement = prior.displacement_per_persistence_micrometre_milli as u128;
    let localization_error = prior.localization_error_micrometre_milli as u128;
    let resolved_displacement = displacement
        .saturating_mul(interval)
        .min(displacement.saturating_mul(persistence))
        / persistence;
    let localization_threshold = localization_error.saturating_mul(3);
    let movement_detectability = ratio_permille(resolved_displacement, localization_threshold);
    let persistence_sampling = ratio_permille(persistence, interval);
    movement_detectability.min(persistence_sampling)
}

fn phase_observability_proxy(dwell_millis: u64, interval_millis: u64) -> u16 {
    ratio_permille(dwell_millis as u128, interval_millis as u128)
}

fn score_profile(
    request: &PhaseResolvedInvasionScheduleRequest,
    profile: &PhaseResolvedAcquisitionProfile,
) -> Result<PhaseResolvedProfileScore, PhaseResolvedScheduleError> {
    let frames = request.assay_duration_millis / profile.capture_interval_millis
        + u64::from(request.assay_duration_millis % profile.capture_interval_millis != 0)
        + 1; // Always include time zero and the final assay boundary.
    let dose = frames.saturating_mul(profile.dose_units_per_capture);
    let movement =
        movement_resolution_proxy(profile.capture_interval_millis, &request.motility_prior);
    let phase_scores = request
        .phase_dwell_priors
        .iter()
        .map(|phase| {
            let observability = phase_observability_proxy(
                phase.conservative_dwell_millis,
                profile.capture_interval_millis,
            );
            let joint = ((observability as u128)
                .saturating_mul(movement as u128)
                .saturating_mul(profile.reporter_quality_permille as u128)
                .saturating_mul(profile.tracking_quality_permille as u128)
                / 1_000_000_000)
                .min(1_000) as u16;
            PhaseSamplingResolution {
                phase_id: phase.phase_id.clone(),
                observability_proxy_permille: observability,
                joint_resolution_permille: joint,
            }
        })
        .collect::<Vec<_>>();
    let score_sum: u128 = phase_scores
        .iter()
        .map(|phase| phase.joint_resolution_permille as u128)
        .sum();
    let mean = (score_sum / phase_scores.len() as u128) as u16;
    let worst = phase_scores
        .iter()
        .map(|phase| phase.joint_resolution_permille)
        .min()
        .unwrap_or(0);
    let balanced = ((mean as u32 * 700 + worst as u32 * 300 + 500) / 1_000) as u16;

    let disposition = if request.assay_duration_millis > profile.maximum_assay_duration_millis {
        PhaseResolvedProfileDisposition::AssayDurationUnsupported
    } else if frames > profile.maximum_capture_count as u64 {
        PhaseResolvedProfileDisposition::CaptureCountExceeded
    } else if dose > request.maximum_total_dose_units {
        PhaseResolvedProfileDisposition::DoseBudgetExceeded
    } else if phase_scores.iter().any(|phase| {
        phase.observability_proxy_permille < request.minimum_phase_observability_permille
    }) {
        PhaseResolvedProfileDisposition::PhaseObservabilityBelowFloor
    } else if worst < request.minimum_joint_resolution_permille {
        PhaseResolvedProfileDisposition::JointResolutionBelowFloor
    } else {
        PhaseResolvedProfileDisposition::Feasible
    };
    Ok(PhaseResolvedProfileScore {
        profile_id: profile.profile_id.clone(),
        instrument_id: profile.instrument_id.clone(),
        device_profile_id: profile.device_profile_id.clone(),
        protocol_profile_id: profile.protocol_profile_id.clone(),
        capability_manifest_digest: profile.capability_manifest_digest.clone(),
        dose_calibration_digest: profile.dose_calibration_digest.clone(),
        capture_interval_millis: profile.capture_interval_millis,
        capture_count: frames,
        dose_units_per_capture: profile.dose_units_per_capture,
        total_dose_units: dose,
        movement_resolution_proxy_permille: movement,
        phase_scores,
        mean_joint_resolution_permille: mean,
        worst_phase_joint_resolution_permille: worst,
        balanced_resolution_score_permille: balanced,
        disposition,
    })
}

/// Choose the best feasible validated acquisition profile using a phase-balanced fixed-point score.
///
/// Candidate ranking maximizes 70% mean and 30% worst-phase joint resolution, then prefers lower
/// calibrated dose, shorter capture intervals, and lexicographic profile ID. The score is a proxy
/// whose assumptions must be checked against local held-out trajectories before physical use.
pub fn design_glioma_phase_resolved_invasion_schedule(
    request: &PhaseResolvedInvasionScheduleRequest,
) -> Result<PhaseResolvedInvasionSchedule, PhaseResolvedScheduleError> {
    validate_request(request)?;
    let mut ranked_profiles = request
        .acquisition_profiles
        .iter()
        .map(|profile| score_profile(request, profile))
        .collect::<Result<Vec<_>, _>>()?;
    ranked_profiles.sort_by(|left, right| {
        let left_feasible = left.disposition == PhaseResolvedProfileDisposition::Feasible;
        let right_feasible = right.disposition == PhaseResolvedProfileDisposition::Feasible;
        right_feasible
            .cmp(&left_feasible)
            .then_with(|| {
                right
                    .balanced_resolution_score_permille
                    .cmp(&left.balanced_resolution_score_permille)
            })
            .then_with(|| left.total_dose_units.cmp(&right.total_dose_units))
            .then_with(|| {
                left.capture_interval_millis
                    .cmp(&right.capture_interval_millis)
            })
            .then_with(|| left.profile_id.cmp(&right.profile_id))
    });
    let selected_profile_id = ranked_profiles
        .iter()
        .find(|profile| profile.disposition == PhaseResolvedProfileDisposition::Feasible)
        .map(|profile| profile.profile_id.clone());
    let disposition = if selected_profile_id.is_some() {
        PhaseResolvedScheduleDisposition::Selected
    } else {
        PhaseResolvedScheduleDisposition::NoFeasibleProfile
    };
    let profile_order = ranked_profiles
        .iter()
        .map(|profile| profile.profile_id.clone())
        .collect();
    let phase_order = request
        .phase_dwell_priors
        .iter()
        .map(|phase| phase.phase_id.clone())
        .collect();
    let mut output = PhaseResolvedInvasionSchedule {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        material: request.material,
        phase_order,
        profile_order,
        ranked_profiles,
        selected_profile_id,
        disposition,
        scoring_method: "macro-worst-phase fixed-point resolution proxy: 70/30 phase-balanced score; per-phase dwell/cadence observability, motility displacement/localization detectability, persistence sampling, reporter quality, tracking quality; hard duration/frame/dose/coverage gates".into(),
        limitations: vec![
            "Phase observability is a conservative dwell-to-cadence proxy, not a posterior probability of observing a transition.".into(),
            "Motion resolution uses pilot displacement, localization error, and persistence summaries; it does not reconstruct individual trajectories or establish causal coupling.".into(),
            "Phase labels and pilot summaries must be independently validated in local non-human preclinical material; unknown or missing reporter states remain unknown.".into(),
            "A schedule is a design recommendation only; P08 protocol compilation, calibration, signed authorization, and instrument interlocks remain mandatory.".into(),
        ],
        digest: ContentHash::of_bytes(b"unsealed-phase-resolved-invasion-schedule"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| PhaseResolvedScheduleError::Digest(error.to_string()))?;
    Ok(output)
}

/// Compile the chosen time-lapse cadence into ordered `AcquireImage` actions for P08 preflight.
///
/// This joins experiment design to the existing instrument workflow without bypassing any gate:
/// the exact device/protocol capability digest must match, action parameters are preserved from a
/// local validated template, and the resulting list still needs ordinary calibration, signed
/// operator approval, protocol binding, live interlocks, and guarded execution.
pub fn compile_glioma_phase_resolved_invasion_actions(
    request: &PhaseResolvedInvasionScheduleRequest,
    schedule: &PhaseResolvedInvasionSchedule,
    template: &PhaseResolvedInstrumentActionTemplate,
) -> Result<PhaseResolvedInvasionActionPlan, PhaseResolvedScheduleError> {
    validate_request(request)?;
    schedule.verify_digest()?;
    let expected_schedule = design_glioma_phase_resolved_invasion_schedule(request)?;
    if expected_schedule.digest != schedule.digest {
        return Err(invalid(
            "schedule must be the deterministic result for the supplied request",
        ));
    }
    let selected_profile_id = schedule
        .selected_profile_id
        .as_deref()
        .ok_or(PhaseResolvedScheduleError::NoFeasibleProfile)?;
    let profile = request
        .acquisition_profiles
        .iter()
        .find(|profile| profile.profile_id == selected_profile_id)
        .ok_or_else(|| invalid("selected profile is absent from the validated request"))?;
    let selected_score = schedule
        .ranked_profiles
        .iter()
        .find(|score| score.profile_id == selected_profile_id)
        .ok_or_else(|| invalid("selected profile score is absent from the schedule"))?;
    let action = &template.action_template;
    if template.profile_id != profile.profile_id
        || template.instrument_id != profile.instrument_id
        || template.device_profile_id != profile.device_profile_id
        || template.protocol_profile_id != profile.protocol_profile_id
        || template.capability_manifest_digest != profile.capability_manifest_digest
        || template.dose_calibration_digest != profile.dose_calibration_digest
        || template.dose_units_per_capture != profile.dose_units_per_capture
        || action.instrument_id != profile.instrument_id
        || action.model_system != request.material.model_system()
        || action.operation != InstrumentOperation::AcquireImage
        || !action.requires_operator
        || action.action_id.trim().is_empty()
        || action.output_schema.trim().is_empty()
        || action.duration_ticks == 0
        || template.ticks_per_millisecond == 0
    {
        return Err(invalid(
            "action template does not match the selected preclinical device/protocol profile",
        ));
    }
    let capture_count = usize::try_from(selected_score.capture_count)
        .map_err(|_| invalid("capture count does not fit this platform"))?;
    if capture_count == 0 || capture_count > MAX_ACTIONS {
        return Err(invalid(
            "selected profile exceeds the bounded P08 action count",
        ));
    }
    let interval_ticks = selected_score
        .capture_interval_millis
        .checked_mul(template.ticks_per_millisecond)
        .ok_or_else(|| invalid("capture interval conversion overflows device ticks"))?;
    let assay_duration_ticks = request
        .assay_duration_millis
        .checked_mul(template.ticks_per_millisecond)
        .ok_or_else(|| invalid("assay duration conversion overflows device ticks"))?;
    let final_gap_millis = request.assay_duration_millis % selected_score.capture_interval_millis;
    let minimum_gap_ticks = if final_gap_millis == 0 {
        interval_ticks
    } else {
        final_gap_millis
            .checked_mul(template.ticks_per_millisecond)
            .ok_or_else(|| invalid("final capture interval conversion overflows device ticks"))?
    };
    if action.duration_ticks > minimum_gap_ticks {
        return Err(invalid(
            "capture action duration overlaps the selected cadence or final assay boundary",
        ));
    }
    let final_capture_tick = template
        .first_capture_tick
        .checked_add(assay_duration_ticks)
        .ok_or_else(|| invalid("final capture tick overflows"))?;
    let completion_tick = final_capture_tick
        .checked_add(action.duration_ticks)
        .ok_or_else(|| invalid("instrument completion tick overflows"))?;
    if completion_tick > MAX_TICK {
        return Err(invalid(
            "compiled actions exceed the P08 instrument tick bound",
        ));
    }

    let mut actions = Vec::with_capacity(capture_count);
    for capture_index in 0..capture_count {
        let offset_millis = selected_score
            .capture_interval_millis
            .checked_mul(capture_index as u64)
            .ok_or_else(|| invalid("capture offset overflows"))?
            .min(request.assay_duration_millis);
        let start_tick = template
            .first_capture_tick
            .checked_add(
                offset_millis
                    .checked_mul(template.ticks_per_millisecond)
                    .ok_or_else(|| invalid("capture offset conversion overflows"))?,
            )
            .ok_or_else(|| invalid("capture start tick overflows"))?;
        let mut planned_action = action.clone();
        planned_action.action_id = format!("{}-capture-{:04}", action.action_id, capture_index + 1);
        planned_action.requested_start_tick = start_tick;
        actions.push(planned_action);
    }
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut plan = PhaseResolvedInvasionActionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: ACTION_PLAN_SCHEMA.into(),
        schedule_digest: schedule.digest.clone(),
        study_id: schedule.study_id.clone(),
        profile_id: profile.profile_id.clone(),
        action_id_prefix: action.action_id.clone(),
        capability_manifest_digest: profile.capability_manifest_digest.clone(),
        dose_calibration_digest: profile.dose_calibration_digest.clone(),
        instrument_id: profile.instrument_id.clone(),
        model_system: action.model_system,
        action_order,
        actions,
        capture_interval_ticks: interval_ticks,
        assay_duration_ticks,
        first_capture_tick: template.first_capture_tick,
        final_capture_tick,
        completion_tick,
        dose_units_per_capture: profile.dose_units_per_capture,
        total_dose_units: selected_score.total_dose_units,
        digest: ContentHash::of_bytes(b"unsealed-phase-resolved-invasion-action-plan"),
    };
    plan.validate()?;
    plan.digest = ContentHash::of_value(&action_plan_digest_input(&plan))
        .map_err(|error| PhaseResolvedScheduleError::Digest(error.to_string()))?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn profile(
        profile_id: &str,
        interval: u64,
        dose: u64,
        max_frames: u32,
    ) -> PhaseResolvedAcquisitionProfile {
        PhaseResolvedAcquisitionProfile {
            profile_id: profile_id.into(),
            instrument_id: "organoid-imager-1".into(),
            device_profile_id: "local-confocal-v1".into(),
            protocol_profile_id: format!("protocol-{profile_id}"),
            capability_manifest_digest: digest("device-capabilities"),
            dose_calibration_digest: digest("dose-calibration-v1"),
            capture_interval_millis: interval,
            dose_units_per_capture: dose,
            maximum_assay_duration_millis: 60_000,
            maximum_capture_count: max_frames,
            reporter_quality_permille: 980,
            tracking_quality_permille: 960,
        }
    }

    fn sample_request() -> PhaseResolvedInvasionScheduleRequest {
        PhaseResolvedInvasionScheduleRequest {
            study_id: "synthetic-preclinical-invasion-study".into(),
            material: GliomaMicroscopyMaterial::MurineOrganoid,
            assay_duration_millis: 60_000,
            maximum_total_dose_units: 800,
            minimum_independent_units: 3,
            minimum_phase_observability_permille: 600,
            minimum_joint_resolution_permille: 250,
            phase_dwell_priors: vec![
                CellCyclePhaseDwellPrior {
                    phase_id: "g1".into(),
                    conservative_dwell_millis: 24_000,
                    independent_unit_count: 8,
                    source_digest: digest("g1-pilot"),
                },
                CellCyclePhaseDwellPrior {
                    phase_id: "g2m".into(),
                    conservative_dwell_millis: 9_000,
                    independent_unit_count: 8,
                    source_digest: digest("g2m-pilot"),
                },
                CellCyclePhaseDwellPrior {
                    phase_id: "s".into(),
                    conservative_dwell_millis: 15_000,
                    independent_unit_count: 8,
                    source_digest: digest("s-pilot"),
                },
            ],
            motility_prior: GliomaMotilityTimescalePrior {
                persistence_time_millis: 18_000,
                displacement_per_persistence_micrometre_milli: 1_800,
                localization_error_micrometre_milli: 100,
                independent_unit_count: 8,
                source_digest: digest("motility-pilot"),
            },
            acquisition_profiles: vec![
                profile("cadence-10s", 10_000, 10, 7),
                profile("cadence-20s", 20_000, 10, 4),
                profile("cadence-5s", 5_000, 100, 13),
            ],
        }
    }

    fn phase_resolved_action_template(
        request: &PhaseResolvedInvasionScheduleRequest,
        profile_id: &str,
    ) -> PhaseResolvedInstrumentActionTemplate {
        let profile = request
            .acquisition_profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
            .expect("test profile is present");
        PhaseResolvedInstrumentActionTemplate {
            profile_id: profile.profile_id.clone(),
            instrument_id: profile.instrument_id.clone(),
            device_profile_id: profile.device_profile_id.clone(),
            protocol_profile_id: profile.protocol_profile_id.clone(),
            capability_manifest_digest: profile.capability_manifest_digest.clone(),
            dose_calibration_digest: profile.dose_calibration_digest.clone(),
            dose_units_per_capture: profile.dose_units_per_capture,
            first_capture_tick: 100,
            ticks_per_millisecond: 1,
            action_template: InstrumentAction {
                action_id: "phase-resolved-glioma-run".into(),
                instrument_id: profile.instrument_id.clone(),
                operation: InstrumentOperation::AcquireImage,
                model_system: request.material.model_system(),
                requested_start_tick: 100,
                duration_ticks: 500,
                risk_milli: 100,
                requires_operator: true,
                output_schema: "GliomaTimeLapseFrame1@1".into(),
                parameters: vec![],
            },
        }
    }

    #[derive(Clone, Copy)]
    struct SyntheticPhase {
        dwell_millis: u64,
        speed_units_per_millis: u64,
    }

    struct SyntheticTrajectoryCase {
        case_id: u64,
        phases: [SyntheticPhase; 3],
        start_offset_millis: u64,
    }

    /// Held-out synthetic worlds vary dwell time, phase offset, and go-or-grow/go-and-grow
    /// movement. These are algorithm stress fixtures, never biological evidence.
    fn synthetic_case(case_id: u64) -> SyntheticTrajectoryCase {
        let g1 = 18_000 + case_id % 5 * 1_000;
        let s = 11_000 + case_id.wrapping_mul(3) % 5 * 1_000;
        let g2m = 6_000 + case_id.wrapping_mul(7) % 4 * 1_000;
        let go_or_grow = case_id % 2 == 0;
        let phases = [
            SyntheticPhase {
                dwell_millis: g1,
                speed_units_per_millis: if go_or_grow { 5 } else { 2 },
            },
            SyntheticPhase {
                dwell_millis: s,
                speed_units_per_millis: if go_or_grow { 2 } else { 4 },
            },
            SyntheticPhase {
                dwell_millis: g2m,
                speed_units_per_millis: if go_or_grow { 1 } else { 5 },
            },
        ];
        let cycle_millis = phases.iter().map(|phase| phase.dwell_millis).sum::<u64>();
        let start_offset_millis = case_id.wrapping_mul(7_919) % cycle_millis;
        SyntheticTrajectoryCase {
            case_id,
            phases,
            start_offset_millis,
        }
    }

    fn synthetic_phase_at(
        case: &SyntheticTrajectoryCase,
        absolute_time_millis: u64,
    ) -> (usize, u64) {
        let cycle_millis = case
            .phases
            .iter()
            .map(|phase| phase.dwell_millis)
            .sum::<u64>();
        let cycle_index = absolute_time_millis / cycle_millis;
        let within_cycle = absolute_time_millis % cycle_millis;
        let mut phase_start = 0;
        for (phase_index, phase) in case.phases.iter().enumerate() {
            let phase_end = phase_start + phase.dwell_millis;
            if within_cycle < phase_end {
                return (
                    phase_index,
                    cycle_index * case.phases.len() as u64 + phase_index as u64,
                );
            }
            phase_start = phase_end;
        }
        unreachable!("modulo cycle duration must select a phase")
    }

    fn synthetic_position_at(case: &SyntheticTrajectoryCase, absolute_time_millis: u64) -> u64 {
        let cycle_millis = case
            .phases
            .iter()
            .map(|phase| phase.dwell_millis)
            .sum::<u64>();
        let completed_cycles = absolute_time_millis / cycle_millis;
        let within_cycle = absolute_time_millis % cycle_millis;
        let distance_per_cycle = case
            .phases
            .iter()
            .map(|phase| phase.dwell_millis as u128 * phase.speed_units_per_millis as u128)
            .sum::<u128>();
        let mut distance = completed_cycles as u128 * distance_per_cycle;
        let mut remaining = within_cycle;
        for phase in &case.phases {
            let elapsed = remaining.min(phase.dwell_millis);
            distance += elapsed as u128 * phase.speed_units_per_millis as u128;
            remaining -= elapsed;
            if remaining == 0 {
                break;
            }
        }
        distance as u64
    }

    #[derive(Clone, Copy)]
    struct SyntheticCapture {
        phase_index: Option<usize>,
        phase_run: u64,
        position: u64,
        time_millis: u64,
        track_usable: bool,
    }

    fn synthetic_profile_recovery(case: &SyntheticTrajectoryCase, interval_millis: u64) -> u16 {
        const ASSAY_DURATION_MILLIS: u64 = 60_000;
        let mut times = vec![0];
        while *times.last().expect("time zero is present") < ASSAY_DURATION_MILLIS {
            let next = times
                .last()
                .expect("time zero is present")
                .saturating_add(interval_millis)
                .min(ASSAY_DURATION_MILLIS);
            times.push(next);
        }
        let captures = times
            .iter()
            .enumerate()
            .map(|(sample_index, time_millis)| {
                let absolute_time = case.start_offset_millis + time_millis;
                let (phase_index, phase_run) = synthetic_phase_at(case, absolute_time);
                let reporter_missing = (sample_index as u64 + case.case_id) % 7 == 0;
                let track_usable = (sample_index as u64 * 3 + case.case_id) % 11 != 0;
                let signed_localization_error =
                    ((sample_index as i64 * 37 + case.case_id as i64 * 19) % 9 - 4) * 20;
                let position = synthetic_position_at(case, absolute_time)
                    .saturating_add_signed(signed_localization_error);
                SyntheticCapture {
                    phase_index: (!reporter_missing).then_some(phase_index),
                    phase_run,
                    position,
                    time_millis: *time_millis,
                    track_usable,
                }
            })
            .collect::<Vec<_>>();

        let visible_phases = captures
            .iter()
            .filter_map(|capture| capture.phase_index)
            .collect::<std::collections::BTreeSet<_>>();
        let phase_coverage =
            (visible_phases.len() as u128 * 1_000 / case.phases.len() as u128) as u16;

        let mut phase_distance = [0_u128; 3];
        let mut phase_time = [0_u128; 3];
        let mut detected_transitions = 0_u64;
        for pair in captures.windows(2) {
            if let (Some(left_phase), Some(right_phase)) =
                (pair[0].phase_index, pair[1].phase_index)
            {
                if left_phase != right_phase {
                    detected_transitions += 1;
                }
                if left_phase == right_phase
                    && pair[0].phase_run == pair[1].phase_run
                    && pair[0].track_usable
                    && pair[1].track_usable
                {
                    phase_distance[left_phase] +=
                        pair[1].position.abs_diff(pair[0].position) as u128;
                    phase_time[left_phase] += (pair[1].time_millis - pair[0].time_millis) as u128;
                }
            }
        }

        let speed_accuracy_sum = case
            .phases
            .iter()
            .enumerate()
            .map(|(phase_index, phase)| {
                if phase_time[phase_index] == 0 {
                    return 0_u16;
                }
                let estimated_speed = phase_distance[phase_index] / phase_time[phase_index];
                let absolute_error = estimated_speed.abs_diff(phase.speed_units_per_millis as u128);
                let error_permille =
                    ratio_permille(absolute_error, phase.speed_units_per_millis.max(1) as u128);
                1_000 - error_permille
            })
            .map(u128::from)
            .sum::<u128>();
        let phase_speed_accuracy = (speed_accuracy_sum / case.phases.len() as u128) as u16;
        let (_, first_run) = synthetic_phase_at(case, case.start_offset_millis);
        let (_, final_run) =
            synthetic_phase_at(case, case.start_offset_millis + ASSAY_DURATION_MILLIS);
        let true_transitions = final_run.saturating_sub(first_run);
        let transition_recall = if true_transitions == 0 {
            1_000
        } else {
            ratio_permille(detected_transitions as u128, true_transitions as u128)
        };

        ((phase_coverage as u32 * 400
            + phase_speed_accuracy as u32 * 400
            + transition_recall as u32 * 200)
            / 1_000) as u16
    }

    fn equal_dose_benchmark_request() -> PhaseResolvedInvasionScheduleRequest {
        let mut request = sample_request();
        request.assay_duration_millis = 60_000;
        request.maximum_total_dose_units = 56;
        request.minimum_phase_observability_permille = 250;
        request.minimum_joint_resolution_permille = 150;
        request.phase_dwell_priors = vec![
            CellCyclePhaseDwellPrior {
                phase_id: "g1".into(),
                conservative_dwell_millis: 18_000,
                independent_unit_count: 8,
                source_digest: digest("benchmark-g1-pilot"),
            },
            CellCyclePhaseDwellPrior {
                phase_id: "g2m".into(),
                conservative_dwell_millis: 6_000,
                independent_unit_count: 8,
                source_digest: digest("benchmark-g2m-pilot"),
            },
            CellCyclePhaseDwellPrior {
                phase_id: "s".into(),
                conservative_dwell_millis: 11_000,
                independent_unit_count: 8,
                source_digest: digest("benchmark-s-pilot"),
            },
        ];
        request.acquisition_profiles = vec![
            profile("adaptive-10s", 10_000, 8, 7),
            profile("fixed-20s", 20_000, 14, 4),
        ];
        request
    }

    #[test]
    fn balances_short_phase_observation_and_motility_resolution_under_dose_budget() {
        let mut request = sample_request();
        request.maximum_total_dose_units = 800;
        let output = design_glioma_phase_resolved_invasion_schedule(&request).unwrap();
        assert_eq!(
            output.disposition,
            PhaseResolvedScheduleDisposition::Selected
        );
        assert_eq!(output.selected_profile_id.as_deref(), Some("cadence-10s"));
        let selected = output
            .ranked_profiles
            .iter()
            .find(|profile| profile.profile_id == "cadence-10s")
            .unwrap();
        assert_eq!(selected.capture_count, 7);
        assert_eq!(selected.total_dose_units, 70);
        assert!(selected.phase_scores.iter().all(|phase| {
            phase.observability_proxy_permille >= request.minimum_phase_observability_permille
                && phase.joint_resolution_permille >= request.minimum_joint_resolution_permille
        }));
        output.verify_digest().unwrap();
    }

    #[test]
    fn a_short_phase_can_block_an_otherwise_feasible_slow_schedule() {
        let mut request = sample_request();
        request.minimum_phase_observability_permille = 700;
        let output = design_glioma_phase_resolved_invasion_schedule(&request).unwrap();
        assert_eq!(output.selected_profile_id.as_deref(), Some("cadence-10s"));
        let slow = output
            .ranked_profiles
            .iter()
            .find(|profile| profile.profile_id == "cadence-20s")
            .unwrap();
        assert_eq!(
            slow.disposition,
            PhaseResolvedProfileDisposition::PhaseObservabilityBelowFloor
        );
        assert_eq!(
            slow.phase_scores
                .iter()
                .find(|phase| phase.phase_id == "g2m")
                .unwrap()
                .observability_proxy_permille,
            450
        );
    }

    #[test]
    fn reports_no_feasible_profile_instead_of_relaxing_the_dose_limit() {
        let mut request = sample_request();
        request.maximum_total_dose_units = 30;
        let output = design_glioma_phase_resolved_invasion_schedule(&request).unwrap();
        assert_eq!(
            output.disposition,
            PhaseResolvedScheduleDisposition::NoFeasibleProfile
        );
        assert_eq!(output.selected_profile_id, None);
        assert!(output.ranked_profiles.iter().all(|profile| {
            profile.disposition == PhaseResolvedProfileDisposition::DoseBudgetExceeded
        }));
    }

    #[test]
    fn requires_independent_nonhuman_pilot_evidence_and_canonical_inputs() {
        let mut request = sample_request();
        request.phase_dwell_priors[1].independent_unit_count = 2;
        assert!(matches!(
            design_glioma_phase_resolved_invasion_schedule(&request),
            Err(PhaseResolvedScheduleError::InvalidRequest(_))
        ));

        let mut reordered = sample_request();
        reordered.acquisition_profiles.swap(0, 1);
        assert!(matches!(
            design_glioma_phase_resolved_invasion_schedule(&reordered),
            Err(PhaseResolvedScheduleError::InvalidRequest(_))
        ));
    }

    #[test]
    fn changes_to_output_invalidate_its_digest() {
        let output = design_glioma_phase_resolved_invasion_schedule(&sample_request()).unwrap();
        output.verify_digest().unwrap();
        let mut altered = output;
        altered.selected_profile_id = Some("unreviewed-profile".into());
        assert!(matches!(
            altered.verify_digest(),
            Err(PhaseResolvedScheduleError::Digest(_))
        ));
    }

    #[test]
    fn phase_balanced_policy_improves_held_out_synthetic_recovery_over_equal_dose_fixed_cadence() {
        let request = equal_dose_benchmark_request();
        let schedule = design_glioma_phase_resolved_invasion_schedule(&request).unwrap();
        assert_eq!(
            schedule.selected_profile_id.as_deref(),
            Some("adaptive-10s")
        );
        let selected = schedule
            .ranked_profiles
            .iter()
            .find(|profile| profile.profile_id == "adaptive-10s")
            .expect("selected schedule is present");
        let fixed = schedule
            .ranked_profiles
            .iter()
            .find(|profile| profile.profile_id == "fixed-20s")
            .expect("fixed-cadence baseline is present");
        assert_eq!(selected.total_dose_units, fixed.total_dose_units);

        let cases = (0..48).map(synthetic_case).collect::<Vec<_>>();
        let improved_cases = cases
            .iter()
            .filter(|case| {
                synthetic_profile_recovery(case, selected.capture_interval_millis)
                    > synthetic_profile_recovery(case, fixed.capture_interval_millis)
            })
            .count();
        assert!(
            improved_cases * 10 >= cases.len() * 9,
            "equal-dose schedule improved synthetic phase/motility recovery in {improved_cases}/{} held-out cases",
            cases.len()
        );
    }

    #[test]
    fn compiles_selected_cadence_to_bounded_operator_gated_p08_actions() {
        let request = equal_dose_benchmark_request();
        let schedule = design_glioma_phase_resolved_invasion_schedule(&request).unwrap();
        let selected_profile = schedule.selected_profile_id.as_deref().unwrap();
        let template = phase_resolved_action_template(&request, selected_profile);

        let plan =
            compile_glioma_phase_resolved_invasion_actions(&request, &schedule, &template).unwrap();
        plan.verify_digest().unwrap();
        assert_eq!(plan.action_order.len(), 7);
        assert_eq!(plan.capture_interval_ticks, 10_000);
        assert_eq!(plan.total_dose_units, 56);
        assert_eq!(plan.first_capture_tick, 100);
        assert_eq!(plan.final_capture_tick, 60_100);
        assert_eq!(plan.completion_tick, 60_600);
        assert!(plan.actions.iter().all(|action| {
            action.operation == InstrumentOperation::AcquireImage
                && action.requires_operator
                && action.instrument_id == "organoid-imager-1"
                && action.parameters == template.action_template.parameters
        }));
        assert!(plan
            .actions
            .windows(2)
            .all(|pair| pair[1].requested_start_tick - pair[0].requested_start_tick == 10_000));
    }

    #[test]
    fn action_compiler_rejects_profile_mismatch_and_plan_sequence_mutation() {
        let request = equal_dose_benchmark_request();
        let schedule = design_glioma_phase_resolved_invasion_schedule(&request).unwrap();
        let selected_profile = schedule.selected_profile_id.as_deref().unwrap();
        let mut template = phase_resolved_action_template(&request, selected_profile);
        template.instrument_id = "different-imager".into();
        assert!(matches!(
            compile_glioma_phase_resolved_invasion_actions(&request, &schedule, &template),
            Err(PhaseResolvedScheduleError::InvalidRequest(_))
        ));

        let template = phase_resolved_action_template(&request, selected_profile);
        let mut plan =
            compile_glioma_phase_resolved_invasion_actions(&request, &schedule, &template).unwrap();
        plan.actions[2].requested_start_tick += 1;
        assert!(matches!(
            plan.verify_digest(),
            Err(PhaseResolvedScheduleError::InvalidRequest(_))
        ));
    }

    #[test]
    fn compiled_actions_complete_the_guarded_calibration_preflight_binding_and_dry_run_path() {
        use crate::glioma::programs::p08_instrument_robotics::calibration::{
            analyze_instrument_calibration, CalibrationRequest, CalibrationRun,
        };
        use crate::glioma::programs::p08_instrument_robotics::execution::{
            execute_glioma_instrument_plan, DryRunInstrumentExecutor,
            InstrumentExecutionDisposition, InstrumentExecutionRequest,
        };
        use crate::glioma::programs::p08_instrument_robotics::preflight::{
            preflight_glioma_instrument, InstrumentAuthorization, InstrumentInterlockSnapshot,
            InstrumentPreflightDisposition, InstrumentPreflightRequest,
        };
        use crate::glioma::programs::p08_instrument_robotics::protocol_binding::{
            compile_glioma_instrument_protocol_binding, InstrumentCommandCapability,
            InstrumentControlProtocol, InstrumentProtocolManifest,
        };
        use crate::glioma_engine::GliomaModelSystem;

        let request = equal_dose_benchmark_request();
        let schedule = design_glioma_phase_resolved_invasion_schedule(&request).unwrap();
        let selected_profile = schedule.selected_profile_id.as_deref().unwrap();
        let template = phase_resolved_action_template(&request, selected_profile);
        let action_plan =
            compile_glioma_phase_resolved_invasion_actions(&request, &schedule, &template).unwrap();
        let instrument_id = template.instrument_id.clone();
        let calibration_runs = (1..=3)
            .map(|sequence_index| CalibrationRun {
                run_id: format!("phase-calibration-{sequence_index}"),
                sequence_index,
                batch_id: format!("phase-reference-{sequence_index}"),
                instrument_id: instrument_id.clone(),
                metric_name: "image-reference".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 1_000,
                expected_milli: 1_000,
                artifact: crate::glioma_engine::LocalArtifactRef {
                    artifact_id: format!("phase-calibration-artifact-{sequence_index}"),
                    content_hash: digest(&format!("phase-calibration-artifact-{sequence_index}")),
                    content_type: "application/vnd.aurora.glioma.calibration+json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
            })
            .collect::<Vec<_>>();
        let calibration = analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify phase-resolved organoid imaging".into(),
                instrument_id: instrument_id.clone(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "image-reference".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 100,
                max_drift_milli: 100,
                max_slope_milli_per_tick: 100,
            },
            &calibration_runs,
        )
        .unwrap();
        assert_eq!(
            calibration.disposition,
            super::super::calibration::CalibrationDisposition::Qualified
        );

        let authorization = InstrumentAuthorization {
            authorization_id: "phase-resolved-operator-approval".into(),
            operator_id: "operator-1".into(),
            instrument_scope: instrument_id.clone(),
            approval_digest: digest("phase-resolved-approval"),
            issued_tick: 99,
            expires_tick: 100_000,
            revoked: false,
        };
        let interlocks = InstrumentInterlockSnapshot {
            observed_tick: 100,
            emergency_stop_clear: true,
            guard_closed: true,
            deck_clear: true,
            consumables_available: true,
            waste_capacity_milli: 1_000,
            temperature_milli: Some(37_000),
            minimum_temperature_milli: Some(36_000),
            maximum_temperature_milli: Some(38_000),
            calibration_valid_until_tick: 100_000,
            calibration_sequence_index: 3,
        };
        let preflight_request = InstrumentPreflightRequest {
            objective: "phase-resolved glioma invasion capture".into(),
            instrument_id: instrument_id.clone(),
            model_system: GliomaModelSystem::Organoid,
            actions: action_plan.actions.clone(),
            calibration,
            interlocks: interlocks.clone(),
            authorization: authorization.clone(),
            current_tick: 100,
            maximum_total_risk_milli: 1_000,
            maximum_duration_ticks: 61_000,
            minimum_waste_capacity_milli: 100,
        };
        let preflight = preflight_glioma_instrument(&preflight_request).unwrap();
        assert_eq!(
            preflight.disposition,
            InstrumentPreflightDisposition::Admitted
        );
        assert!(preflight.dispatch_permitted);

        let manifest = InstrumentProtocolManifest {
            instrument_id: instrument_id.clone(),
            protocol: InstrumentControlProtocol::Sila2,
            protocol_version: "1.1".into(),
            manifest_id: "phase-resolved-imager-capabilities".into(),
            revision: 1,
            commands: vec![InstrumentCommandCapability {
                operation: InstrumentOperation::AcquireImage,
                feature_path: "org.aurora.GliomaPhaseImaging".into(),
                command_id: "AcquireTimeLapseFrame".into(),
                output_schema: "GliomaTimeLapseFrame1@1".into(),
                retry_idempotent: false,
                parameters: Vec::new(),
            }],
        };
        let binding =
            compile_glioma_instrument_protocol_binding(&preflight, &action_plan.actions, &manifest)
                .unwrap();
        binding.validate().unwrap();

        let execution_request = InstrumentExecutionRequest {
            objective: preflight.objective.clone(),
            plan: preflight,
            actions: action_plan.actions,
            authorization,
            live_interlocks: interlocks.clone(),
            current_tick: 100,
            minimum_waste_capacity_milli: 100,
            max_retries: 0,
            require_artifacts: true,
        };
        let mut executor = DryRunInstrumentExecutor {
            interlocks,
            emergency_stop_called: false,
        };
        let run = execute_glioma_instrument_plan(&execution_request, &mut executor).unwrap();
        assert_eq!(run.disposition, InstrumentExecutionDisposition::Completed);
        assert_eq!(run.completed_order.len(), 7);
        assert_eq!(run.results.len(), 7);
        assert!(run.results.iter().all(|result| result.artifact.is_some()));
        assert!(!executor.emergency_stop_called);
    }
}
