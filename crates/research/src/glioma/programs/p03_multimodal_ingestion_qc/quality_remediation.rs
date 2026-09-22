//! Budgeted remediation planning for multimodal glioma QC incidents.
//!
//! This feature is the action seam after process-cause attribution. It selects deterministic,
//! approval-aware remediation steps from local QC evidence; it does not execute assays, move raw
//! data, or infer biology. Resource limits and blocked prerequisites remain visible to the caller.

use super::quality_root_cause::{
    QualityRootCause, QualityRootCauseAttribution, QualityRootCauseDisposition,
};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQualityRemediation1@1";
pub const MAX_STEPS: usize = 64;
pub const MAX_CANDIDATES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRemediationActionKind {
    InspectInstrument,
    ReharmonizeBatch,
    ReprepareSamples,
    ReregisterAlignment,
    CalibrateTransport,
    QuarantineConnector,
    CollectOrthogonalQc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRemediationDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRemediationCandidate {
    pub action_id: String,
    pub cause: QualityRootCause,
    pub action_kind: QualityRemediationActionKind,
    pub modalities: Vec<GliomaModality>,
    pub cost_units: u32,
    pub duration_units: u32,
    pub risk_milli: u16,
    pub expected_recovery_milli: u16,
    pub prerequisites: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub available: bool,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRemediationRequest {
    pub objective: String,
    pub incident_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub attributions: Vec<QualityRootCauseAttribution>,
    pub candidates: Vec<QualityRemediationCandidate>,
    pub max_budget_units: u32,
    pub max_duration_units: u32,
    pub max_risk_milli: u16,
    pub min_recovery_milli: u16,
    pub require_approval_for_external: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRemediationStep {
    pub rank: u16,
    pub action_id: String,
    pub cause: QualityRootCause,
    pub action_kind: QualityRemediationActionKind,
    pub modality_order: Vec<GliomaModality>,
    pub cost_units: u32,
    pub duration_units: u32,
    pub risk_milli: u16,
    pub expected_recovery_milli: u16,
    pub prerequisites: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRemediationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub incident_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub selected_steps: Vec<QualityRemediationStep>,
    pub rejected_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub total_cost_units: u32,
    pub total_duration_units: u32,
    pub total_risk_milli: u16,
    pub expected_recovery_milli: u16,
    pub disposition: QualityRemediationDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityRemediationError {
    #[error("quality remediation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("quality remediation candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("quality remediation plan is invalid: {0}")]
    InvalidOutput(String),
    #[error("quality remediation digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &QualityRemediationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "incident_id": plan.incident_id,
        "study_id": plan.study_id,
        "model_system": plan.model_system,
        "selected_steps": plan.selected_steps,
        "rejected_action_order": plan.rejected_action_order,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "total_cost_units": plan.total_cost_units,
        "total_duration_units": plan.total_duration_units,
        "total_risk_milli": plan.total_risk_milli,
        "expected_recovery_milli": plan.expected_recovery_milli,
        "disposition": plan.disposition,
        "next_action": plan.next_action,
    })
}

fn validate_request(request: &QualityRemediationRequest) -> Result<(), QualityRemediationError> {
    if request.objective.trim().is_empty()
        || request.incident_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.attributions.is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.max_budget_units == 0
        || request.max_duration_units == 0
        || request.max_risk_milli > 1_000
        || request.min_recovery_milli > 1_000
    {
        return Err(QualityRemediationError::InvalidRequest(
            "incident binding, attributions, bounded candidates, and positive resource budgets are required".into(),
        ));
    }
    let mut causes = BTreeSet::new();
    for attribution in &request.attributions {
        if !causes.insert(attribution.cause) || attribution.signal_count == 0 {
            return Err(QualityRemediationError::InvalidRequest(
                "attributions must contain unique causes with observed signals".into(),
            ));
        }
    }
    let mut action_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.action_id.trim().is_empty()
            || !action_ids.insert(candidate.action_id.clone())
            || !causes.contains(&candidate.cause)
            || candidate.modalities.is_empty()
            || !canonical(&candidate.modalities)
            || candidate.cost_units == 0
            || candidate.duration_units == 0
            || candidate.risk_milli > 1_000
            || candidate.expected_recovery_milli > 1_000
            || candidate.evidence_ids.iter().any(|id| id.trim().is_empty())
            || !canonical(&candidate.evidence_ids)
            || candidate
                .prerequisites
                .iter()
                .any(|id| id.trim().is_empty())
            || !canonical(&candidate.prerequisites)
        {
            return Err(QualityRemediationError::InvalidCandidate(
                "candidates require unique IDs, attributed causes, canonical modalities/evidence, positive costs, and bounded risk/recovery".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(plan: &QualityRemediationPlan) -> Result<(), QualityRemediationError> {
    if plan.feature_id != FEATURE_ID
        || plan.output_schema != OUTPUT_SCHEMA
        || plan.objective.trim().is_empty()
        || plan.incident_id.trim().is_empty()
        || plan.study_id.trim().is_empty()
        || plan.selected_steps.len() > MAX_STEPS
        || !canonical(&plan.rejected_action_order)
        || !canonical(&plan.negative_evidence)
        || !canonical(&plan.uncertainty)
        || plan.total_risk_milli > 1_000
        || plan.expected_recovery_milli > 1_000
        || plan
            .selected_steps
            .windows(2)
            .any(|pair| pair[0].rank >= pair[1].rank)
        || plan.selected_steps.iter().any(|step| {
            step.action_id.trim().is_empty()
                || step.rank == 0
                || !canonical(&step.modality_order)
                || !canonical(&step.prerequisites)
                || !canonical(&step.evidence_ids)
                || step.risk_milli > 1_000
                || step.expected_recovery_milli > 1_000
        })
    {
        return Err(QualityRemediationError::InvalidOutput(
            "identity, canonical ordering, step ranks, or bounded aggregate invariants are invalid"
                .into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(plan))
        .map_err(|error| QualityRemediationError::Digest(error.to_string()))?;
    if expected != plan.digest {
        return Err(QualityRemediationError::InvalidOutput(
            "quality remediation digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl QualityRemediationPlan {
    pub fn validate(&self) -> Result<(), QualityRemediationError> {
        validate_output(self)
    }
}

fn priority(candidate: &QualityRemediationCandidate) -> u128 {
    let denominator = u128::from(candidate.cost_units)
        .saturating_mul(1_000)
        .saturating_add(u128::from(candidate.duration_units).saturating_mul(500))
        .saturating_add(u128::from(candidate.risk_milli))
        .saturating_add(1);
    u128::from(candidate.expected_recovery_milli).saturating_mul(1_000_000) / denominator
}

/// Select a bounded, approval-aware remediation sequence from QC root-cause evidence.
pub fn plan_glioma_multimodal_quality_remediation(
    request: &QualityRemediationRequest,
) -> Result<QualityRemediationPlan, QualityRemediationError> {
    validate_request(request)?;
    let attribution_by_cause = request
        .attributions
        .iter()
        .map(|attribution| (attribution.cause, attribution))
        .collect::<BTreeMap<_, _>>();
    let primary = request
        .attributions
        .iter()
        .find(|attribution| attribution.disposition == QualityRootCauseDisposition::Qualified)
        .map(|attribution| attribution.cause);
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        primary
            .map(|cause| (right.cause == cause).cmp(&(left.cause == cause)))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| priority(right).cmp(&priority(left)))
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let mut selected_steps = Vec::new();
    let mut rejected = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut total_cost = 0_u32;
    let mut total_duration = 0_u32;
    let mut total_risk = 0_u32;
    let mut total_recovery = 0_u32;
    for candidate in candidates {
        let Some(attribution) = attribution_by_cause.get(&candidate.cause) else {
            rejected.insert(candidate.action_id);
            continue;
        };
        if matches!(
            attribution.disposition,
            QualityRootCauseDisposition::Unresolved | QualityRootCauseDisposition::Blocked
        ) {
            rejected.insert(candidate.action_id.clone());
            uncertainty.insert(format!(
                "unresolved-cause:{}",
                format_args!("{:?}", candidate.cause)
            ));
            continue;
        }
        if !candidate.available {
            rejected.insert(candidate.action_id.clone());
            negative.insert(format!("unavailable-action:{}", candidate.action_id));
            continue;
        }
        if candidate.expected_recovery_milli < request.min_recovery_milli {
            rejected.insert(candidate.action_id.clone());
            negative.insert(format!("below-recovery-floor:{}", candidate.action_id));
            continue;
        }
        if request.require_approval_for_external && candidate.requires_approval {
            rejected.insert(candidate.action_id.clone());
            negative.insert(format!("approval-required:{}", candidate.action_id));
            continue;
        }
        let next_risk = total_risk.saturating_add(u32::from(candidate.risk_milli));
        if total_cost.saturating_add(candidate.cost_units) > request.max_budget_units
            || total_duration.saturating_add(candidate.duration_units) > request.max_duration_units
            || next_risk > u32::from(request.max_risk_milli)
        {
            rejected.insert(candidate.action_id.clone());
            negative.insert(format!("resource-bound:{}", candidate.action_id));
            continue;
        }
        total_cost = total_cost.saturating_add(candidate.cost_units);
        total_duration = total_duration.saturating_add(candidate.duration_units);
        total_risk = next_risk;
        total_recovery = total_recovery
            .saturating_add(u32::from(candidate.expected_recovery_milli))
            .min(1_000);
        selected_steps.push(QualityRemediationStep {
            rank: (selected_steps.len() + 1) as u16,
            action_id: candidate.action_id,
            cause: candidate.cause,
            action_kind: candidate.action_kind,
            modality_order: candidate.modalities,
            cost_units: candidate.cost_units,
            duration_units: candidate.duration_units,
            risk_milli: candidate.risk_milli,
            expected_recovery_milli: candidate.expected_recovery_milli,
            prerequisites: candidate.prerequisites,
            evidence_ids: candidate.evidence_ids,
            requires_approval: candidate.requires_approval,
        });
    }
    if selected_steps.is_empty() && !negative.is_empty() {
        uncertainty.insert("no-remediation-step-cleared-all-gates".into());
    }
    let selected_primary =
        primary.is_some_and(|cause| selected_steps.iter().any(|step| step.cause == cause));
    let disposition = if selected_steps.is_empty() {
        if negative
            .iter()
            .any(|entry| entry.starts_with("approval-required:"))
        {
            QualityRemediationDisposition::Blocked
        } else {
            QualityRemediationDisposition::Unresolved
        }
    } else if selected_primary && negative.is_empty() {
        QualityRemediationDisposition::Ready
    } else {
        QualityRemediationDisposition::Conditional
    };
    let next_action = match disposition {
        QualityRemediationDisposition::Ready => {
            "submit the selected local remediation sequence to the approval-bound QC campaign"
        }
        QualityRemediationDisposition::Conditional => {
            "review rejected actions and execute only the selected bounded remediation steps"
        }
        QualityRemediationDisposition::Blocked => {
            "obtain required approval or local capability before scheduling remediation"
        }
        QualityRemediationDisposition::Unresolved => {
            "collect orthogonal QC evidence before scheduling remediation"
        }
    }
    .into();
    let mut plan = QualityRemediationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        incident_id: request.incident_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        selected_steps,
        rejected_action_order: rejected.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        total_cost_units: total_cost,
        total_duration_units: total_duration,
        total_risk_milli: total_risk.min(1_000) as u16,
        expected_recovery_milli: total_recovery as u16,
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-quality-remediation"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| QualityRemediationError::Digest(error.to_string()))?;
    validate_output(&plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attribution(
        cause: QualityRootCause,
        disposition: QualityRootCauseDisposition,
    ) -> QualityRootCauseAttribution {
        QualityRootCauseAttribution {
            cause,
            support_milli: 800,
            contradiction_milli: 0,
            net_score_milli: 800,
            confidence_milli: 900,
            signal_count: 2,
            modality_order: vec![GliomaModality::Genomics],
            disposition,
            remediation_action: "local remediation".into(),
        }
    }

    fn candidate(
        action_id: &str,
        cause: QualityRootCause,
        requires_approval: bool,
    ) -> QualityRemediationCandidate {
        QualityRemediationCandidate {
            action_id: action_id.into(),
            cause,
            action_kind: QualityRemediationActionKind::ReharmonizeBatch,
            modalities: vec![GliomaModality::Genomics],
            cost_units: 3,
            duration_units: 2,
            risk_milli: 100,
            expected_recovery_milli: 800,
            prerequisites: vec!["qc-evidence".into()],
            evidence_ids: vec!["evidence-1".into()],
            available: true,
            requires_approval,
        }
    }

    fn request(candidates: Vec<QualityRemediationCandidate>) -> QualityRemediationRequest {
        QualityRemediationRequest {
            objective: "recover a glioma multimodal QC incident".into(),
            incident_id: "incident-1".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            attributions: vec![attribution(
                QualityRootCause::Batch,
                QualityRootCauseDisposition::Qualified,
            )],
            candidates,
            max_budget_units: 10,
            max_duration_units: 10,
            max_risk_milli: 500,
            min_recovery_milli: 700,
            require_approval_for_external: false,
        }
    }

    #[test]
    fn selects_qualified_remediation_as_ready() {
        let plan = plan_glioma_multimodal_quality_remediation(&request(vec![candidate(
            "reharmonize-batch",
            QualityRootCause::Batch,
            false,
        )]))
        .expect("plan");
        assert_eq!(plan.disposition, QualityRemediationDisposition::Ready);
        assert_eq!(plan.selected_steps.len(), 1);
        plan.validate().expect("valid digest");
    }

    #[test]
    fn retains_resource_rejection_as_negative_evidence() {
        let mut request = request(vec![candidate("step-a", QualityRootCause::Batch, false)]);
        request.max_budget_units = 1;
        let plan = plan_glioma_multimodal_quality_remediation(&request).expect("plan");
        assert_eq!(plan.disposition, QualityRemediationDisposition::Unresolved);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|entry| entry.starts_with("resource-bound:")));
    }

    #[test]
    fn approval_gate_blocks_external_remediation() {
        let mut request = request(vec![candidate(
            "external-step",
            QualityRootCause::Batch,
            true,
        )]);
        request.require_approval_for_external = true;
        let plan = plan_glioma_multimodal_quality_remediation(&request).expect("plan");
        assert_eq!(plan.disposition, QualityRemediationDisposition::Blocked);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|entry| entry.starts_with("approval-required:")));
    }
}
