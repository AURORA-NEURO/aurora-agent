//! Autonomous replication and interpretation campaigns for preclinical glioma research.
//!
//! The individual P10 analyses answer different questions: does an effect replicate, is the
//! pooled estimate stable, and does it transport to a declared model system?  This controller
//! composes those analyses into an executable research loop.  It ranks the next bounded action
//! from the observed evidence, calls a local executor, incorporates only returned typed studies,
//! and repeats until a qualification, negative result, resource stop, or unresolved gate is
//! reached.  It never turns a pooled effect into a clinical decision and never invents a study
//! result when an executor returns no observation.

use super::meta_analysis::{
    analyze_replication_meta_analysis, MetaAnalysisDisposition, MetaAnalysisError,
    MetaAnalysisRequest, ReplicationMetaAnalysis,
};
use super::transportability::{
    analyze_glioma_transportability, TransportStudy, TransportabilityAnalysis,
    TransportabilityDisposition, TransportabilityError, TransportabilityRequest,
};
use crate::glioma::replication::{
    assess_replication, ReplicationAssessment, ReplicationDisposition, ReplicationError,
    ReplicationRequest, ReplicationStudy,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F18";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationCampaign1@1";
pub const MAX_ROUNDS: u16 = 64;
pub const MAX_STUDIES: usize = 4_096;
pub const MAX_ACTIONS_PER_ROUND: usize = 32;
pub const MAX_SIGNATURE_DIMENSIONS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReplicationCampaignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub target_model_system: GliomaModelSystem,
    pub target_signature: Vec<i64>,
    pub min_sites: usize,
    pub min_replicates_per_site: u16,
    pub min_studies: usize,
    pub min_replicates_per_study: u16,
    pub effect_threshold_milli: u64,
    pub max_heterogeneity_milli: u64,
    pub max_i2_milli: u16,
    pub min_signal_to_noise_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub min_quality_milli: u16,
    pub distance_scale_milli: u32,
    pub max_transport_gap_milli: u16,
    pub max_transport_heterogeneity_milli: u16,
    pub budget_units: u32,
    pub max_rounds: u16,
    pub max_actions_per_round: usize,
    pub max_retries: u8,
    pub initial_studies: Vec<ReplicationStudy>,
    pub initial_transport_studies: Vec<TransportStudy>,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaReplicationActionKind {
    ReplicateStudy,
    ResolveHeterogeneity,
    ReassayInfluentialStudy,
    AcquireTargetModel,
    PublishNegativeResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReplicationAction {
    pub action_id: String,
    pub kind: GliomaReplicationActionKind,
    pub target_id: String,
    pub model_system: GliomaModelSystem,
    pub population_signature: Vec<i64>,
    pub expected_information_milli: u64,
    pub estimated_cost_units: u32,
    pub priority_score_milli: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReplicationCampaignObservation {
    pub action_id: String,
    pub simulation_only: bool,
    pub studies: Vec<ReplicationStudy>,
    pub transport_studies: Vec<TransportStudy>,
    pub artifact_order: Vec<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GliomaReplicationExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// Execute one selected local replication or interpretation action.  The host owns the lab,
/// instrument, or compute gateway; the controller only admits the typed action and validates the
/// returned preclinical study objects.
pub trait GliomaReplicationCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &GliomaReplicationAction,
        round: u16,
    ) -> Result<GliomaReplicationCampaignObservation, GliomaReplicationExecutionFailure>;
}

/// Deterministic synthetic executor for integration tests and sandbox demonstrations.  Its
/// studies are explicitly marked by their artifact type and are never biological evidence.
#[derive(Debug, Default)]
pub struct DryRunGliomaReplicationCampaignExecutor {
    sequence: u64,
}

impl GliomaReplicationCampaignExecutor for DryRunGliomaReplicationCampaignExecutor {
    fn execute_action(
        &mut self,
        action: &GliomaReplicationAction,
        round: u16,
    ) -> Result<GliomaReplicationCampaignObservation, GliomaReplicationExecutionFailure> {
        self.sequence = self.sequence.saturating_add(1);
        let study_id = format!("dry-run-replication:{}:{}", action.action_id, self.sequence);
        let site_id = format!("dry-run-site:{}", self.sequence);
        let artifact_hash = ContentHash::of_value(&serde_json::json!({
            "action_id": action.action_id,
            "round": round,
            "sequence": self.sequence,
            "simulation_only": true,
        }))
        .map_err(|error| GliomaReplicationExecutionFailure {
            reason: format!("dry-run replication artifact digest failed: {error}"),
            retryable: false,
        })?;
        let artifact = crate::glioma_engine::LocalArtifactRef {
            artifact_id: format!("dry-run-replication-artifact:{}", self.sequence),
            content_hash: artifact_hash.clone(),
            content_type: "application/vnd.aurora.glioma-replication-observation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let study = ReplicationStudy {
            study_id: study_id.clone(),
            site_id,
            model_system: action.model_system,
            artifact: artifact.clone(),
            effect_milli: 250,
            uncertainty_milli: 100,
            replicate_count: 3,
        };
        let transport_studies = if action.kind == GliomaReplicationActionKind::AcquireTargetModel {
            vec![TransportStudy {
                study_id,
                model_system: action.model_system,
                population_signature: action.population_signature.clone(),
                effect_milli: 250,
                uncertainty_milli: 100,
                replicates: 3,
                quality_milli: 900,
                artifact,
            }]
        } else {
            Vec::new()
        };
        Ok(GliomaReplicationCampaignObservation {
            action_id: action.action_id.clone(),
            simulation_only: true,
            studies: if transport_studies.is_empty() {
                vec![study]
            } else {
                Vec::new()
            },
            transport_studies,
            artifact_order: vec![artifact_hash],
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReplicationCampaignRound {
    pub round: u16,
    pub assessment: ReplicationAssessment,
    pub meta_analysis: ReplicationMetaAnalysis,
    pub transportability: Option<TransportabilityAnalysis>,
    pub candidate_actions: Vec<GliomaReplicationAction>,
    pub selected_action_order: Vec<String>,
    pub observation_order: Vec<String>,
    pub observation_artifact_digest_order: Vec<ContentHash>,
    pub simulation_only_order: Vec<String>,
    pub budget_before_units: u32,
    pub budget_after_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaReplicationCampaignDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaReplicationCampaignStopReason {
    Qualified,
    Negative,
    NoActions,
    NoProgress,
    BudgetExhausted,
    MaxRounds,
    ExecutorFailed,
    Unresolved,
    TransportBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaReplicationCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub target_model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub rounds: Vec<GliomaReplicationCampaignRound>,
    pub studies: Vec<ReplicationStudy>,
    pub transport_studies: Vec<TransportStudy>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub retry_count: u32,
    pub budget_used_units: u32,
    pub remaining_budget_units: u32,
    pub final_assessment: ReplicationAssessment,
    pub final_meta_analysis: ReplicationMetaAnalysis,
    pub final_transportability: Option<TransportabilityAnalysis>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: GliomaReplicationCampaignDisposition,
    pub stop_reason: GliomaReplicationCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaReplicationCampaignError {
    #[error("replication campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication assessment failed: {0}")]
    Replication(#[from] ReplicationError),
    #[error("replication meta-analysis failed: {0}")]
    MetaAnalysis(#[from] MetaAnalysisError),
    #[error("replication transportability failed: {0}")]
    Transportability(#[from] TransportabilityError),
    #[error("replication campaign executor failed: {0}")]
    Executor(String),
    #[error("replication campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(campaign: &GliomaReplicationCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "objective": campaign.objective,
        "model_system": campaign.model_system,
        "target_model_system": campaign.target_model_system,
        "replay_identity": campaign.replay_identity,
        "rounds": campaign.rounds,
        "studies": campaign.studies,
        "transport_studies": campaign.transport_studies,
        "completed_action_order": campaign.completed_action_order,
        "failed_action_order": campaign.failed_action_order,
        "retry_count": campaign.retry_count,
        "budget_used_units": campaign.budget_used_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "final_assessment": campaign.final_assessment,
        "final_meta_analysis": campaign.final_meta_analysis,
        "final_transportability": campaign.final_transportability,
        "uncertainty": campaign.uncertainty,
        "negative_evidence": campaign.negative_evidence,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn validate_request(
    request: &GliomaReplicationCampaignRequest,
) -> Result<(), GliomaReplicationCampaignError> {
    if request.objective.trim().is_empty()
        || request.min_sites == 0
        || request.min_replicates_per_site == 0
        || request.min_studies == 0
        || request.min_replicates_per_study == 0
        || request.effect_threshold_milli == 0
        || request.max_i2_milli > 1_000
        || request.min_signal_to_noise_milli == 0
        || request.max_transport_gap_milli > 1_000
        || request.max_transport_heterogeneity_milli > 1_000
        || request.min_quality_milli > 1_000
        || request.distance_scale_milli == 0
        || request.target_signature.is_empty()
        || request.target_signature.len() > MAX_SIGNATURE_DIMENSIONS
        || request.budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_actions_per_round == 0
        || request.max_actions_per_round > MAX_ACTIONS_PER_ROUND
        || request.replay_identity.as_str().len() != 64
        || request.initial_studies.len() > MAX_STUDIES
        || request.initial_transport_studies.len() > MAX_STUDIES
    {
        return Err(GliomaReplicationCampaignError::InvalidRequest(
            "objective, bounded replication/transport thresholds, target signature, budget, rounds, actions, and replay identity are required".into(),
        ));
    }
    Ok(())
}

fn replication_request(request: &GliomaReplicationCampaignRequest) -> ReplicationRequest {
    ReplicationRequest {
        objective: request.objective.clone(),
        model_system: request.model_system,
        min_sites: request.min_sites,
        min_replicates_per_site: request.min_replicates_per_site,
        effect_threshold_milli: request.effect_threshold_milli,
        heterogeneity_tolerance_milli: request.max_heterogeneity_milli,
    }
}

fn meta_request(request: &GliomaReplicationCampaignRequest) -> MetaAnalysisRequest {
    MetaAnalysisRequest {
        objective: request.objective.clone(),
        model_system: request.model_system,
        min_studies: request.min_studies,
        min_replicates_per_study: request.min_replicates_per_study,
        effect_threshold_milli: request.effect_threshold_milli,
        max_i2_milli: request.max_i2_milli,
        min_signal_to_noise_milli: request.min_signal_to_noise_milli,
        max_leave_one_out_shift_milli: request.max_leave_one_out_shift_milli,
    }
}

fn transport_request(request: &GliomaReplicationCampaignRequest) -> TransportabilityRequest {
    TransportabilityRequest {
        objective: request.objective.clone(),
        target_model_system: request.target_model_system,
        target_signature: request.target_signature.clone(),
        min_studies: request.min_studies,
        min_replicates_per_study: request.min_replicates_per_study,
        min_quality_milli: request.min_quality_milli,
        distance_scale_milli: request.distance_scale_milli,
        max_transport_gap_milli: request.max_transport_gap_milli,
        max_heterogeneity_milli: request.max_transport_heterogeneity_milli,
        effect_threshold_milli: request.effect_threshold_milli,
        min_signal_to_noise_milli: request.min_signal_to_noise_milli,
        max_leave_one_out_shift_milli: request.max_leave_one_out_shift_milli,
    }
}

fn analyze_current(
    request: &GliomaReplicationCampaignRequest,
    studies: &[ReplicationStudy],
    transport_studies: &[TransportStudy],
) -> Result<
    (
        ReplicationAssessment,
        ReplicationMetaAnalysis,
        Option<TransportabilityAnalysis>,
    ),
    GliomaReplicationCampaignError,
> {
    let assessment = assess_replication(&replication_request(request), studies)?;
    let meta_analysis = analyze_replication_meta_analysis(&meta_request(request), studies)?;
    let transportability =
        if request.target_model_system != request.model_system && !transport_studies.is_empty() {
            Some(analyze_glioma_transportability(
                &transport_request(request),
                transport_studies,
            )?)
        } else {
            None
        };
    Ok((assessment, meta_analysis, transportability))
}

#[allow(clippy::too_many_arguments)]
fn action(
    kind: GliomaReplicationActionKind,
    target_id: impl Into<String>,
    model_system: GliomaModelSystem,
    population_signature: Vec<i64>,
    expected_information_milli: u64,
    estimated_cost_units: u32,
    priority_score_milli: u64,
    rationale: impl Into<String>,
) -> GliomaReplicationAction {
    let target_id = target_id.into();
    let kind_label = serde_json::to_string(&kind)
        .unwrap_or_else(|_| "\"unknown\"".into())
        .trim_matches('"')
        .to_string();
    GliomaReplicationAction {
        action_id: format!("{kind_label}:{target_id}"),
        kind,
        target_id,
        model_system,
        population_signature,
        expected_information_milli,
        estimated_cost_units,
        priority_score_milli,
        rationale: rationale.into(),
    }
}

fn candidate_actions(
    request: &GliomaReplicationCampaignRequest,
    assessment: &ReplicationAssessment,
    meta_analysis: &ReplicationMetaAnalysis,
    transportability: Option<&TransportabilityAnalysis>,
) -> Vec<GliomaReplicationAction> {
    let mut actions = Vec::new();
    let site_deficit = request
        .min_sites
        .saturating_sub(assessment.site_order.len());
    let study_deficit = request
        .min_studies
        .saturating_sub(meta_analysis.included_order.len());
    if site_deficit > 0 || study_deficit > 0 {
        let deficit = site_deficit.max(study_deficit) as u64;
        actions.push(action(
            GliomaReplicationActionKind::ReplicateStudy,
            format!("batch-{}", deficit),
            request.model_system,
            request.target_signature.clone(),
            800_000_u64.saturating_add(deficit.min(100) * 1_000),
            8,
            950_000,
            "increase independent site/study coverage before interpreting a pooled effect",
        ));
    }
    if meta_analysis.i2_milli > request.max_i2_milli {
        let target = meta_analysis
            .contributions
            .iter()
            .max_by_key(|contribution| {
                (
                    contribution.leave_one_out_shift_milli,
                    contribution.study_id.clone(),
                )
            })
            .map(|contribution| contribution.study_id.clone())
            .unwrap_or_else(|| "heterogeneity".into());
        actions.push(action(
            GliomaReplicationActionKind::ResolveHeterogeneity,
            target,
            request.model_system,
            request.target_signature.clone(),
            u64::from(meta_analysis.i2_milli).saturating_mul(700),
            12,
            900_000_u64.saturating_add(u64::from(meta_analysis.i2_milli)),
            "run an orthogonal preclinical replication or moderator assay before pooling discordant studies",
        ));
    }
    if meta_analysis.max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
        let target = meta_analysis
            .contributions
            .iter()
            .max_by_key(|contribution| {
                (
                    contribution.leave_one_out_shift_milli,
                    contribution.study_id.clone(),
                )
            })
            .map(|contribution| contribution.study_id.clone())
            .unwrap_or_else(|| "influential-study".into());
        actions.push(action(
            GliomaReplicationActionKind::ReassayInfluentialStudy,
            target,
            request.model_system,
            request.target_signature.clone(),
            meta_analysis.max_leave_one_out_shift_milli.saturating_mul(900),
            10,
            850_000_u64.saturating_add(meta_analysis.max_leave_one_out_shift_milli),
            "reassay the study that controls the pooled direction so influence is independently tested",
        ));
    }
    if request.target_model_system != request.model_system
        && (transportability.is_none()
            || transportability.is_some_and(|analysis| {
                analysis.disposition != TransportabilityDisposition::Qualified
            }))
    {
        let target = transportability
            .map(|analysis| analysis.target_model_system)
            .unwrap_or(request.target_model_system);
        actions.push(action(
            GliomaReplicationActionKind::AcquireTargetModel,
            format!("{:?}", target).to_lowercase(),
            request.target_model_system,
            request.target_signature.clone(),
            700_000,
            16,
            780_000,
            "collect a declared target-model study instead of extrapolating across a transport gap",
        ));
    }
    if matches!(
        assessment.disposition,
        ReplicationDisposition::NotReplicated | ReplicationDisposition::Mixed
    ) || matches!(
        meta_analysis.disposition,
        MetaAnalysisDisposition::Negative | MetaAnalysisDisposition::Heterogeneous
    ) {
        actions.push(action(
            GliomaReplicationActionKind::PublishNegativeResult,
            "current-evidence",
            request.model_system,
            request.target_signature.clone(),
            350_000,
            2,
            500_000,
            "preserve the null, mixed, or heterogeneous result as a first-class research output",
        ));
    }
    actions.sort_by(|left, right| {
        right
            .priority_score_milli
            .cmp(&left.priority_score_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    actions.dedup_by(|left, right| left.action_id == right.action_id);
    actions
}

fn qualified(
    request: &GliomaReplicationCampaignRequest,
    assessment: &ReplicationAssessment,
    meta_analysis: &ReplicationMetaAnalysis,
    transportability: Option<&TransportabilityAnalysis>,
) -> bool {
    matches!(assessment.disposition, ReplicationDisposition::Replicated)
        && matches!(
            meta_analysis.disposition,
            MetaAnalysisDisposition::Qualified
        )
        && (request.target_model_system == request.model_system
            || matches!(
                transportability.map(|analysis| analysis.disposition),
                Some(TransportabilityDisposition::Qualified)
            ))
}

fn negative(assessment: &ReplicationAssessment, meta_analysis: &ReplicationMetaAnalysis) -> bool {
    matches!(
        assessment.disposition,
        ReplicationDisposition::NotReplicated
    ) && matches!(
        meta_analysis.disposition,
        MetaAnalysisDisposition::Negative | MetaAnalysisDisposition::Heterogeneous
    )
}

fn add_study(
    studies: &mut Vec<ReplicationStudy>,
    candidate: ReplicationStudy,
) -> Result<bool, GliomaReplicationCampaignError> {
    if candidate.study_id.trim().is_empty() || candidate.site_id.trim().is_empty() {
        return Err(GliomaReplicationCampaignError::InvalidRequest(
            "executor returned a study with empty identity".into(),
        ));
    }
    if let Some(existing) = studies
        .iter()
        .find(|study| study.study_id == candidate.study_id)
    {
        if existing != &candidate {
            return Err(GliomaReplicationCampaignError::InvalidRequest(
                "executor changed the typed contract for an existing study".into(),
            ));
        }
        return Ok(false);
    }
    if studies.len() >= MAX_STUDIES {
        return Err(GliomaReplicationCampaignError::InvalidRequest(
            "campaign study registry exceeds 4096 studies".into(),
        ));
    }
    studies.push(candidate);
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    Ok(true)
}

fn add_transport_study(
    studies: &mut Vec<TransportStudy>,
    candidate: TransportStudy,
) -> Result<bool, GliomaReplicationCampaignError> {
    if candidate.study_id.trim().is_empty() {
        return Err(GliomaReplicationCampaignError::InvalidRequest(
            "executor returned a transport study with empty identity".into(),
        ));
    }
    if let Some(existing) = studies
        .iter()
        .find(|study| study.study_id == candidate.study_id)
    {
        if existing != &candidate {
            return Err(GliomaReplicationCampaignError::InvalidRequest(
                "executor changed the typed contract for an existing transport study".into(),
            ));
        }
        return Ok(false);
    }
    if studies.len() >= MAX_STUDIES {
        return Err(GliomaReplicationCampaignError::InvalidRequest(
            "campaign transport-study registry exceeds 4096 studies".into(),
        ));
    }
    studies.push(candidate);
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    Ok(true)
}

impl GliomaReplicationCampaign {
    pub fn validate(&self) -> Result<(), GliomaReplicationCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || self.rounds.len() > MAX_ROUNDS as usize
            || self
                .budget_used_units
                .saturating_add(self.remaining_budget_units)
                == 0
            || !canonical(&self.completed_action_order)
            || !canonical(&self.failed_action_order)
            || self
                .completed_action_order
                .iter()
                .any(|id| self.failed_action_order.contains(id))
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.rounds.iter().enumerate().any(|(index, round)| {
                round.round != index as u16 + 1
                    || round.budget_after_units > round.budget_before_units
                    || !unique(&round.selected_action_order)
                    || !unique(&round.observation_order)
                    || !unique(&round.simulation_only_order)
                    || round
                        .observation_artifact_digest_order
                        .iter()
                        .any(|digest| digest.as_str().len() != 64)
                    || round.assessment.validate().is_err()
                    || round.meta_analysis.validate().is_err()
                    || round
                        .transportability
                        .as_ref()
                        .is_some_and(|analysis| analysis.validate().is_err())
            })
            || self.final_assessment.validate().is_err()
            || self.final_meta_analysis.validate().is_err()
            || self
                .final_transportability
                .as_ref()
                .is_some_and(|analysis| analysis.validate().is_err())
        {
            return Err(GliomaReplicationCampaignError::InvalidOutput(
                "identity, budget, canonical ordering, round analysis, or final-analysis invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaReplicationCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaReplicationCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Run a bounded replication/interpretation campaign over local preclinical study objects.
pub fn execute_glioma_replication_campaign<E: GliomaReplicationCampaignExecutor>(
    request: &GliomaReplicationCampaignRequest,
    executor: &mut E,
) -> Result<GliomaReplicationCampaign, GliomaReplicationCampaignError> {
    validate_request(request)?;
    let mut studies = request.initial_studies.clone();
    let mut transport_studies = request.initial_transport_studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    transport_studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let mut rounds = Vec::new();
    let mut completed_action_order = Vec::new();
    let mut failed_action_order = Vec::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut used_actions = BTreeSet::new();
    let mut budget_remaining = request.budget_units;
    let mut retry_count = 0_u32;
    let mut stop_reason = GliomaReplicationCampaignStopReason::MaxRounds;

    for round in 1..=request.max_rounds {
        let (assessment, meta_analysis, transportability) =
            analyze_current(request, &studies, &transport_studies)?;
        uncertainty.extend(assessment.uncertainty.iter().cloned());
        uncertainty.extend(meta_analysis.uncertainty.iter().cloned());
        if let Some(analysis) = &transportability {
            uncertainty.extend(analysis.uncertainty.iter().cloned());
        }
        negative_evidence.extend(assessment.negative_evidence.iter().cloned());
        negative_evidence.extend(meta_analysis.negative_evidence.iter().cloned());
        if let Some(analysis) = &transportability {
            negative_evidence.extend(analysis.negative_evidence.iter().cloned());
        }
        if qualified(
            request,
            &assessment,
            &meta_analysis,
            transportability.as_ref(),
        ) {
            stop_reason = GliomaReplicationCampaignStopReason::Qualified;
            break;
        }
        if negative(&assessment, &meta_analysis)
            && assessment.site_order.len() >= request.min_sites
            && meta_analysis.included_order.len() >= request.min_studies
        {
            stop_reason = GliomaReplicationCampaignStopReason::Negative;
            break;
        }
        let candidates = candidate_actions(
            request,
            &assessment,
            &meta_analysis,
            transportability.as_ref(),
        );
        let selected = candidates
            .iter()
            .filter(|candidate| {
                !used_actions.contains(&candidate.action_id)
                    && candidate.estimated_cost_units <= budget_remaining
            })
            .take(request.max_actions_per_round)
            .cloned()
            .collect::<Vec<_>>();
        if selected.is_empty() {
            stop_reason = if budget_remaining == 0 {
                GliomaReplicationCampaignStopReason::BudgetExhausted
            } else if candidates.is_empty() {
                GliomaReplicationCampaignStopReason::NoActions
            } else {
                GliomaReplicationCampaignStopReason::NoProgress
            };
            rounds.push(GliomaReplicationCampaignRound {
                round,
                assessment,
                meta_analysis,
                transportability,
                candidate_actions: candidates,
                selected_action_order: Vec::new(),
                observation_order: Vec::new(),
                observation_artifact_digest_order: Vec::new(),
                simulation_only_order: Vec::new(),
                budget_before_units: budget_remaining,
                budget_after_units: budget_remaining,
            });
            break;
        }
        let budget_before = budget_remaining;
        let mut observations = Vec::new();
        let mut made_progress = false;
        for selected_action in &selected {
            used_actions.insert(selected_action.action_id.clone());
            let mut result = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.execute_action(selected_action, round) {
                    Ok(observation) => {
                        result = Some(observation);
                        retry_count = retry_count.saturating_add(u32::from(attempt - 1));
                        break;
                    }
                    Err(failure) if failure.retryable && attempt <= request.max_retries => {
                        retry_count = retry_count.saturating_add(1);
                    }
                    Err(failure) => {
                        failed_action_order.push(selected_action.action_id.clone());
                        stop_reason = GliomaReplicationCampaignStopReason::ExecutorFailed;
                        return build_campaign(
                            request,
                            rounds,
                            studies,
                            transport_studies,
                            completed_action_order,
                            failed_action_order,
                            retry_count,
                            budget_remaining,
                            uncertainty,
                            negative_evidence,
                            stop_reason,
                            assessment,
                            meta_analysis,
                            transportability,
                            Some(format!("{}: {}", selected_action.action_id, failure.reason)),
                        );
                    }
                }
            }
            let observation = result.ok_or_else(|| {
                GliomaReplicationCampaignError::Executor(
                    "executor returned no observation after bounded retries".into(),
                )
            })?;
            if observation.action_id != selected_action.action_id
                || observation
                    .artifact_order
                    .iter()
                    .any(|hash| hash.as_str().len() != 64)
            {
                return Err(GliomaReplicationCampaignError::Executor(
                    "executor observation is not bound to the selected action".into(),
                ));
            }
            let mut observation_progress = false;
            if !observation.simulation_only {
                for study in observation.studies.iter().cloned() {
                    observation_progress |= add_study(&mut studies, study)?;
                }
                for study in observation.transport_studies.iter().cloned() {
                    observation_progress |= add_transport_study(&mut transport_studies, study)?;
                }
            }
            observations.push(observation);
            budget_remaining =
                budget_remaining.saturating_sub(selected_action.estimated_cost_units);
            completed_action_order.push(selected_action.action_id.clone());
            made_progress |= observation_progress
                || selected_action.kind == GliomaReplicationActionKind::PublishNegativeResult;
        }
        rounds.push(GliomaReplicationCampaignRound {
            round,
            assessment,
            meta_analysis,
            transportability,
            candidate_actions: candidates,
            selected_action_order: selected
                .iter()
                .map(|action| action.action_id.clone())
                .collect(),
            observation_order: observations
                .iter()
                .map(|observation| observation.action_id.clone())
                .collect(),
            observation_artifact_digest_order: observations
                .iter()
                .flat_map(|observation| observation.artifact_order.iter().cloned())
                .collect(),
            simulation_only_order: observations
                .iter()
                .filter(|observation| observation.simulation_only)
                .map(|observation| observation.action_id.clone())
                .collect(),
            budget_before_units: budget_before,
            budget_after_units: budget_remaining,
        });
        if !made_progress {
            stop_reason = GliomaReplicationCampaignStopReason::NoProgress;
            break;
        }
        if budget_remaining == 0 {
            stop_reason = GliomaReplicationCampaignStopReason::BudgetExhausted;
            break;
        }
    }
    let (final_assessment, final_meta_analysis, final_transportability) =
        analyze_current(request, &studies, &transport_studies)?;
    build_campaign(
        request,
        rounds,
        studies,
        transport_studies,
        completed_action_order,
        failed_action_order,
        retry_count,
        budget_remaining,
        uncertainty,
        negative_evidence,
        stop_reason,
        final_assessment,
        final_meta_analysis,
        final_transportability,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_campaign(
    request: &GliomaReplicationCampaignRequest,
    mut rounds: Vec<GliomaReplicationCampaignRound>,
    mut studies: Vec<ReplicationStudy>,
    mut transport_studies: Vec<TransportStudy>,
    mut completed_action_order: Vec<String>,
    mut failed_action_order: Vec<String>,
    retry_count: u32,
    remaining_budget_units: u32,
    uncertainty: BTreeSet<String>,
    negative_evidence: BTreeSet<String>,
    stop_reason: GliomaReplicationCampaignStopReason,
    final_assessment: ReplicationAssessment,
    final_meta_analysis: ReplicationMetaAnalysis,
    final_transportability: Option<TransportabilityAnalysis>,
    execution_error: Option<String>,
) -> Result<GliomaReplicationCampaign, GliomaReplicationCampaignError> {
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    transport_studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    completed_action_order.sort();
    completed_action_order.dedup();
    failed_action_order.sort();
    failed_action_order.dedup();
    if let Some(error) = execution_error {
        let mut values = negative_evidence.into_iter().collect::<Vec<_>>();
        values.push(format!("executor-error:{error}"));
        values.sort();
        let output = GliomaReplicationCampaign {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            objective: request.objective.clone(),
            model_system: request.model_system,
            target_model_system: request.target_model_system,
            replay_identity: request.replay_identity.clone(),
            rounds,
            studies,
            transport_studies,
            completed_action_order,
            failed_action_order,
            retry_count,
            budget_used_units: request.budget_units.saturating_sub(remaining_budget_units),
            remaining_budget_units,
            final_assessment,
            final_meta_analysis,
            final_transportability,
            uncertainty: uncertainty.into_iter().collect(),
            negative_evidence: values,
            disposition: GliomaReplicationCampaignDisposition::Failed,
            stop_reason,
            digest: ContentHash::of_bytes(b"unsealed-glioma-replication-campaign"),
        };
        return seal_campaign(output);
    }
    let disposition = if qualified(
        request,
        &final_assessment,
        &final_meta_analysis,
        final_transportability.as_ref(),
    ) {
        GliomaReplicationCampaignDisposition::Qualified
    } else if negative(&final_assessment, &final_meta_analysis) {
        GliomaReplicationCampaignDisposition::Negative
    } else if rounds.is_empty() {
        GliomaReplicationCampaignDisposition::Blocked
    } else if matches!(stop_reason, GliomaReplicationCampaignStopReason::NoActions) {
        GliomaReplicationCampaignDisposition::Unresolved
    } else {
        GliomaReplicationCampaignDisposition::Partial
    };
    let output = GliomaReplicationCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        target_model_system: request.target_model_system,
        replay_identity: request.replay_identity.clone(),
        rounds: std::mem::take(&mut rounds),
        studies,
        transport_studies,
        completed_action_order,
        failed_action_order,
        retry_count,
        budget_used_units: request.budget_units.saturating_sub(remaining_budget_units),
        remaining_budget_units,
        final_assessment,
        final_meta_analysis,
        final_transportability,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-campaign"),
    };
    seal_campaign(output)
}

fn seal_campaign(
    mut output: GliomaReplicationCampaign,
) -> Result<GliomaReplicationCampaign, GliomaReplicationCampaignError> {
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaReplicationCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn study(id: &str, site: &str, effect: i64) -> ReplicationStudy {
        ReplicationStudy {
            study_id: id.into(),
            site_id: site.into(),
            model_system: GliomaModelSystem::Organoid,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact:{id}"),
                content_hash: hash(id),
                content_type: "application/vnd.aurora.glioma-study+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            effect_milli: effect,
            uncertainty_milli: 100,
            replicate_count: 3,
        }
    }

    fn request(studies: Vec<ReplicationStudy>) -> GliomaReplicationCampaignRequest {
        GliomaReplicationCampaignRequest {
            objective: "replicate a preclinical glioma invasion effect".into(),
            model_system: GliomaModelSystem::Organoid,
            target_model_system: GliomaModelSystem::Organoid,
            target_signature: vec![1, 2],
            min_sites: 3,
            min_replicates_per_site: 2,
            min_studies: 3,
            min_replicates_per_study: 2,
            effect_threshold_milli: 10,
            max_heterogeneity_milli: 500,
            max_i2_milli: 500,
            min_signal_to_noise_milli: 10,
            max_leave_one_out_shift_milli: 1_000,
            min_quality_milli: 500,
            distance_scale_milli: 1_000,
            max_transport_gap_milli: 500,
            max_transport_heterogeneity_milli: 500,
            budget_units: 16,
            max_rounds: 2,
            max_actions_per_round: 1,
            max_retries: 1,
            initial_studies: studies,
            initial_transport_studies: Vec::new(),
            replay_identity: hash("replay"),
        }
    }

    #[derive(Debug, Default)]
    struct ReturningExecutor {
        sequence: u64,
    }

    impl GliomaReplicationCampaignExecutor for ReturningExecutor {
        fn execute_action(
            &mut self,
            action: &GliomaReplicationAction,
            _round: u16,
        ) -> Result<GliomaReplicationCampaignObservation, GliomaReplicationExecutionFailure>
        {
            self.sequence = self.sequence.saturating_add(1);
            let study_id = format!("returned-study-{}", self.sequence);
            let artifact_hash = hash(&study_id);
            Ok(GliomaReplicationCampaignObservation {
                action_id: action.action_id.clone(),
                simulation_only: false,
                studies: vec![ReplicationStudy {
                    study_id: study_id.clone(),
                    site_id: format!("returned-site-{}", self.sequence),
                    model_system: action.model_system,
                    artifact: LocalArtifactRef {
                        artifact_id: format!("artifact:{study_id}"),
                        content_hash: artifact_hash.clone(),
                        content_type: "application/vnd.aurora.glioma-study+json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                    effect_milli: 260,
                    uncertainty_milli: 100,
                    replicate_count: 3,
                }],
                transport_studies: Vec::new(),
                artifact_order: vec![artifact_hash],
            })
        }
    }

    #[test]
    fn campaign_ranks_missing_replication_without_inventing_results() {
        let mut executor = DryRunGliomaReplicationCampaignExecutor::default();
        let output = execute_glioma_replication_campaign(
            &request(vec![study("s1", "site-1", 250)]),
            &mut executor,
        )
        .unwrap();
        assert!(!output.rounds.is_empty());
        assert_eq!(
            output.rounds[0].candidate_actions[0].kind,
            GliomaReplicationActionKind::ReplicateStudy
        );
        assert!(output.studies.iter().all(|study| study.study_id == "s1"));
        assert!(output
            .negative_evidence
            .iter()
            .any(|value| value.contains("minimum")));
        output.validate().unwrap();
    }

    #[test]
    fn qualified_existing_replication_stops_without_dispatch() {
        let studies = vec![
            study("s1", "site-1", 250),
            study("s2", "site-2", 260),
            study("s3", "site-3", 255),
        ];
        let mut executor = DryRunGliomaReplicationCampaignExecutor::default();
        let request = request(studies);
        let output = execute_glioma_replication_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            GliomaReplicationCampaignStopReason::Qualified
        );
        assert_eq!(
            output.disposition,
            GliomaReplicationCampaignDisposition::Qualified
        );
        assert!(output.rounds.is_empty());
    }

    #[test]
    fn returned_study_artifact_is_consumed_by_the_next_analysis_round() {
        let mut request = request(vec![study("s1", "site-1", 250)]);
        request.min_sites = 2;
        request.min_studies = 2;
        let mut executor = ReturningExecutor::default();
        let output = execute_glioma_replication_campaign(&request, &mut executor).unwrap();
        assert_eq!(
            output.stop_reason,
            GliomaReplicationCampaignStopReason::Qualified
        );
        assert_eq!(
            output.disposition,
            GliomaReplicationCampaignDisposition::Qualified
        );
        assert_eq!(output.studies.len(), 2);
        assert_eq!(output.rounds.len(), 1);
        assert_eq!(output.final_meta_analysis.included_order.len(), 2);
    }
}
