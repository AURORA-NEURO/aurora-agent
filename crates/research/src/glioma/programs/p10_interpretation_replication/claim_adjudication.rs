//! Causal-claim adjudication for preclinical glioma research.
//!
//! A mechanism result is not ready for a research claim merely because one estimator is positive.
//! This feature composes the existing difference-in-differences contrast, unmeasured-confounding
//! sensitivity analysis, independent-site replication assessment, and fixed/random-effects
//! meta-analysis.  It emits a gate-by-gate verdict and an ordered next-evidence portfolio.  The
//! engine deliberately keeps negative, heterogeneous, underpowered, and unresolved states
//! separate; it never turns a preclinical result into a patient-facing or clinical decision.

use super::causal_contrast::{
    analyze_glioma_causal_contrast, CausalContrastAnalysis, CausalContrastDisposition,
    CausalContrastError, CausalContrastRequest,
};
use super::meta_analysis::{
    analyze_replication_meta_analysis, MetaAnalysisDisposition, MetaAnalysisError,
    MetaAnalysisRequest, ReplicationMetaAnalysis,
};
use super::sensitivity::{
    analyze_causal_sensitivity, CausalSensitivityAnalysis, SensitivityDisposition,
    SensitivityError, SensitivityRequest,
};
use super::trajectory::TrajectoryObservation;
use crate::glioma::replication::{
    assess_replication, ReplicationAssessment, ReplicationDisposition, ReplicationError,
    ReplicationRequest, ReplicationStudy,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaCausalClaimAdjudication1@1";
pub const MAX_ACTIONS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaCausalClaimAdjudicationRequest {
    pub objective: String,
    pub hypothesis: String,
    pub model_system: GliomaModelSystem,
    pub contrast_request: CausalContrastRequest,
    pub contrast_observations: Vec<TrajectoryObservation>,
    pub sensitivity_request: SensitivityRequest,
    pub sensitivity_observations: Vec<super::sensitivity::SensitivityObservation>,
    pub replication_request: ReplicationRequest,
    pub meta_request: MetaAnalysisRequest,
    pub studies: Vec<ReplicationStudy>,
    pub min_robust_strength_milli: u64,
    pub max_i2_milli: u16,
    pub min_claim_confidence_milli: u16,
    pub max_actions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimGateDisposition {
    Pass,
    Negative,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimGate {
    pub gate_id: String,
    pub disposition: ClaimGateDisposition,
    pub metric_milli: u64,
    pub threshold_milli: u64,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimActionKind {
    CollectTimepoints,
    MeasureConfounders,
    ReplicateIndependentSite,
    ResolveHeterogeneity,
    PublishNegativeResult,
    ReleasePreclinicalClaim,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimNextAction {
    pub action_id: String,
    pub kind: ClaimActionKind,
    pub priority_milli: u64,
    pub consumer: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalClaimDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaCausalClaimAdjudication {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub hypothesis: String,
    pub model_system: GliomaModelSystem,
    pub contrast: CausalContrastAnalysis,
    pub sensitivity: CausalSensitivityAnalysis,
    pub replication: ReplicationAssessment,
    pub meta_analysis: ReplicationMetaAnalysis,
    pub gates: Vec<ClaimGate>,
    pub action_order: Vec<String>,
    pub actions: Vec<ClaimNextAction>,
    pub claim_effect_milli: i64,
    pub claim_uncertainty_milli: u64,
    pub confidence_milli: u16,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: CausalClaimDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaCausalClaimAdjudicationError {
    #[error("causal claim request is invalid: {0}")]
    InvalidRequest(String),
    #[error("causal claim analysis failed: {0}")]
    Analysis(String),
    #[error("causal claim output is invalid: {0}")]
    InvalidOutput(String),
    #[error("causal claim digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &GliomaCausalClaimAdjudication) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "hypothesis": output.hypothesis,
        "model_system": output.model_system,
        "contrast": output.contrast,
        "sensitivity": output.sensitivity,
        "replication": output.replication,
        "meta_analysis": output.meta_analysis,
        "gates": output.gates,
        "action_order": output.action_order,
        "actions": output.actions,
        "claim_effect_milli": output.claim_effect_milli,
        "claim_uncertainty_milli": output.claim_uncertainty_milli,
        "confidence_milli": output.confidence_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl GliomaCausalClaimAdjudication {
    pub fn validate(&self) -> Result<(), GliomaCausalClaimAdjudicationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.gates.len() != 4
            || self.gates.iter().any(|gate| {
                gate.gate_id.trim().is_empty()
                    || gate.rationale.trim().is_empty()
                    || gate.metric_milli > 1_000_000_000
                    || gate.threshold_milli > 1_000_000_000
            })
            || !canonical(&self.action_order)
            || self.actions.len() != self.action_order.len()
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.consumer.trim().is_empty()
                    || action.rationale.trim().is_empty()
                    || action.priority_milli > 1_000_000
            })
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
            || self.confidence_milli > 1_000
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
        {
            return Err(GliomaCausalClaimAdjudicationError::InvalidOutput(
                "claim identity, gate count, action ordering, confidence, or evidence bounds are invalid".into(),
            ));
        }
        let gate_ids = self
            .gates
            .iter()
            .map(|gate| gate.gate_id.clone())
            .collect::<BTreeSet<_>>();
        if gate_ids
            != BTreeSet::from([
                "causal_effect".into(),
                "confounding_robustness".into(),
                "independent_replication".into(),
                "meta_analysis".into(),
            ])
        {
            return Err(GliomaCausalClaimAdjudicationError::InvalidOutput(
                "claim gates must cover exactly the four adjudication dimensions".into(),
            ));
        }
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        if action_ids.len() != self.actions.len()
            || action_ids != self.action_order.iter().cloned().collect::<BTreeSet<_>>()
        {
            return Err(GliomaCausalClaimAdjudicationError::InvalidOutput(
                "claim action order does not reconcile with actions".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaCausalClaimAdjudicationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaCausalClaimAdjudicationError::InvalidOutput(
                "claim digest is not bound to adjudication inputs".into(),
            ));
        }
        Ok(())
    }
}

fn validate_binding(
    request: &GliomaCausalClaimAdjudicationRequest,
) -> Result<(), GliomaCausalClaimAdjudicationError> {
    if request.objective.trim().is_empty()
        || request.hypothesis.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.max_i2_milli > 1_000
        || request.min_claim_confidence_milli > 1_000
        || request.min_robust_strength_milli > 1_000_000_000
        || request.contrast_request.objective != request.objective
        || request.sensitivity_request.objective != request.objective
        || request.replication_request.objective != request.objective
        || request.meta_request.objective != request.objective
        || request.contrast_request.model_system != request.model_system
        || request.sensitivity_request.model_system != request.model_system
        || request.replication_request.model_system != request.model_system
        || request.meta_request.model_system != request.model_system
    {
        return Err(GliomaCausalClaimAdjudicationError::InvalidRequest(
            "objective, bounded action/confidence thresholds, and nested model/objective bindings are required".into(),
        ));
    }
    Ok(())
}

fn gate(
    gate_id: &str,
    disposition: ClaimGateDisposition,
    metric_milli: u64,
    threshold_milli: u64,
    rationale: impl Into<String>,
) -> ClaimGate {
    ClaimGate {
        gate_id: gate_id.into(),
        disposition,
        metric_milli,
        threshold_milli,
        rationale: rationale.into(),
    }
}

fn action(
    kind: ClaimActionKind,
    priority_milli: u64,
    consumer: &str,
    rationale: impl Into<String>,
) -> ClaimNextAction {
    let kind_label = serde_json::to_string(&kind)
        .unwrap_or_else(|_| "\"unknown\"".into())
        .trim_matches('"')
        .to_string();
    ClaimNextAction {
        action_id: kind_label.clone(),
        kind,
        priority_milli,
        consumer: consumer.into(),
        rationale: rationale.into(),
    }
}

fn confidence(
    contrast: &CausalContrastAnalysis,
    sensitivity: &CausalSensitivityAnalysis,
    replication: &ReplicationAssessment,
    meta: &ReplicationMetaAnalysis,
    min_robust_strength: u64,
) -> u16 {
    let effect_score: u16 = if contrast.difference_in_differences_milli.unsigned_abs() >= 1
        && contrast.interval_low_milli <= contrast.difference_in_differences_milli
        && contrast.interval_high_milli >= contrast.difference_in_differences_milli
    {
        1_000
    } else {
        0
    };
    let robustness_score = if min_robust_strength == 0 {
        1_000
    } else {
        (sensitivity
            .max_robust_strength_milli
            .saturating_mul(1_000)
            .checked_div(min_robust_strength)
            .unwrap_or(0)
            .min(1_000)) as u16
    };
    let replication_score: u16 = match replication.disposition {
        ReplicationDisposition::Replicated => 1_000,
        ReplicationDisposition::Mixed => 500,
        ReplicationDisposition::NotReplicated => 0,
        ReplicationDisposition::Unresolved => 250,
    };
    let meta_score = meta.random_signal_to_noise_milli.min(1_000) as u16;
    ((u32::from(effect_score)
        + u32::from(robustness_score)
        + u32::from(replication_score)
        + u32::from(meta_score))
        / 4) as u16
}

pub fn execute_glioma_causal_claim_adjudication(
    request: &GliomaCausalClaimAdjudicationRequest,
) -> Result<GliomaCausalClaimAdjudication, GliomaCausalClaimAdjudicationError> {
    validate_binding(request)?;
    let contrast =
        analyze_glioma_causal_contrast(&request.contrast_request, &request.contrast_observations)
            .map_err(|error: CausalContrastError| {
            GliomaCausalClaimAdjudicationError::Analysis(error.to_string())
        })?;
    let sensitivity = analyze_causal_sensitivity(
        &request.sensitivity_request,
        &request.sensitivity_observations,
    )
    .map_err(|error: SensitivityError| {
        GliomaCausalClaimAdjudicationError::Analysis(error.to_string())
    })?;
    let replication = assess_replication(&request.replication_request, &request.studies).map_err(
        |error: ReplicationError| GliomaCausalClaimAdjudicationError::Analysis(error.to_string()),
    )?;
    let meta_analysis = analyze_replication_meta_analysis(&request.meta_request, &request.studies)
        .map_err(|error: MetaAnalysisError| {
            GliomaCausalClaimAdjudicationError::Analysis(error.to_string())
        })?;

    let causal_gate = match contrast.disposition {
        CausalContrastDisposition::Qualified => gate(
            "causal_effect",
            ClaimGateDisposition::Pass,
            contrast.difference_in_differences_milli.unsigned_abs(),
            request.contrast_request.effect_threshold_milli,
            "difference-in-differences clears the declared effect and permutation gates",
        ),
        CausalContrastDisposition::Negative => gate(
            "causal_effect",
            ClaimGateDisposition::Negative,
            contrast.difference_in_differences_milli.unsigned_abs(),
            request.contrast_request.effect_threshold_milli,
            "the causal contrast is below threshold or fails its permutation gate",
        ),
        CausalContrastDisposition::Unresolved => gate(
            "causal_effect",
            ClaimGateDisposition::Hold,
            contrast.difference_in_differences_milli.unsigned_abs(),
            request.contrast_request.effect_threshold_milli,
            "missing or underpowered temporal units prevent causal adjudication",
        ),
    };
    let sensitivity_gate = match sensitivity.disposition {
        SensitivityDisposition::Qualified
            if sensitivity.max_robust_strength_milli >= request.min_robust_strength_milli =>
        {
            gate(
                "confounding_robustness",
                ClaimGateDisposition::Pass,
                sensitivity.max_robust_strength_milli,
                request.min_robust_strength_milli,
                "the declared effect survives the requested confounding-strength grid",
            )
        }
        SensitivityDisposition::Negative => gate(
            "confounding_robustness",
            ClaimGateDisposition::Negative,
            sensitivity.max_robust_strength_milli,
            request.min_robust_strength_milli,
            "the effect fails the observed or sign-robustness sensitivity gate",
        ),
        _ => gate(
            "confounding_robustness",
            ClaimGateDisposition::Hold,
            sensitivity.max_robust_strength_milli,
            request.min_robust_strength_milli,
            "confounding sensitivity remains partial or underpowered",
        ),
    };
    let replication_gate = match replication.disposition {
        ReplicationDisposition::Replicated => gate(
            "independent_replication",
            ClaimGateDisposition::Pass,
            replication.concordant_order.len() as u64,
            request.replication_request.min_sites as u64,
            "independent preclinical sites reproduce the declared direction within tolerance",
        ),
        ReplicationDisposition::NotReplicated => gate(
            "independent_replication",
            ClaimGateDisposition::Negative,
            replication.concordant_order.len() as u64,
            request.replication_request.min_sites as u64,
            "the independent-site replication floor or direction gate is not met",
        ),
        _ => gate(
            "independent_replication",
            ClaimGateDisposition::Hold,
            replication.concordant_order.len() as u64,
            request.replication_request.min_sites as u64,
            "site coverage or cross-site agreement remains unresolved",
        ),
    };
    let meta_gate = match meta_analysis.disposition {
        MetaAnalysisDisposition::Qualified if meta_analysis.i2_milli <= request.max_i2_milli => {
            gate(
                "meta_analysis",
                ClaimGateDisposition::Pass,
                1_000_u64.saturating_sub(u64::from(meta_analysis.i2_milli)),
                1_000_u64.saturating_sub(u64::from(request.max_i2_milli)),
                "fixed/random-effects estimates meet signal, influence, and heterogeneity gates",
            )
        }
        MetaAnalysisDisposition::Negative => gate(
            "meta_analysis",
            ClaimGateDisposition::Negative,
            meta_analysis.random_signal_to_noise_milli,
            request.meta_request.min_signal_to_noise_milli,
            "the pooled effect does not clear the declared signal or effect gate",
        ),
        _ => gate(
            "meta_analysis",
            ClaimGateDisposition::Hold,
            1_000_u64.saturating_sub(u64::from(meta_analysis.i2_milli)),
            1_000_u64.saturating_sub(u64::from(request.max_i2_milli)),
            "pooled evidence remains heterogeneous, underpowered, or unresolved",
        ),
    };
    let gates = vec![causal_gate, sensitivity_gate, replication_gate, meta_gate];
    let has_negative = gates
        .iter()
        .any(|gate| gate.disposition == ClaimGateDisposition::Negative);
    let has_hold = gates
        .iter()
        .any(|gate| gate.disposition == ClaimGateDisposition::Hold);
    let all_pass = gates
        .iter()
        .all(|gate| gate.disposition == ClaimGateDisposition::Pass);
    let claim_confidence = confidence(
        &contrast,
        &sensitivity,
        &replication,
        &meta_analysis,
        request.min_robust_strength_milli,
    );
    let disposition = if all_pass && claim_confidence >= request.min_claim_confidence_milli {
        CausalClaimDisposition::Qualified
    } else if has_negative && !has_hold {
        CausalClaimDisposition::Negative
    } else if has_negative {
        CausalClaimDisposition::Partial
    } else {
        CausalClaimDisposition::Unresolved
    };

    let mut actions = Vec::new();
    if !contrast.unresolved_unit_order.is_empty() {
        actions.push(action(
            ClaimActionKind::CollectTimepoints,
            950_000,
            "experimentalist",
            "collect missing baseline/post windows before interpreting the causal contrast",
        ));
    }
    if sensitivity.tipping_strength_milli.is_some()
        || sensitivity.disposition != SensitivityDisposition::Qualified
    {
        actions.push(action(
            ClaimActionKind::MeasureConfounders,
            900_000,
            "methods reviewer",
            "measure or balance the declared confounder dimensions before promoting the effect",
        ));
    }
    if replication.disposition != ReplicationDisposition::Replicated {
        actions.push(action(
            ClaimActionKind::ReplicateIndependentSite,
            850_000,
            "consortium administrator",
            "add an independent preclinical site or study with the same estimand and model binding",
        ));
    }
    if meta_analysis.i2_milli > request.max_i2_milli
        || meta_analysis.disposition == MetaAnalysisDisposition::Heterogeneous
    {
        actions.push(action(
            ClaimActionKind::ResolveHeterogeneity,
            800_000,
            "replication scientist",
            "stratify protocol, clone, modality, or model-system differences instead of averaging them away",
        ));
    }
    if matches!(disposition, CausalClaimDisposition::Negative) {
        actions.push(action(
            ClaimActionKind::PublishNegativeResult,
            700_000,
            "research-object steward",
            "release the negative or null result with its estimand, limitations, and failed gate evidence",
        ));
    }
    if matches!(disposition, CausalClaimDisposition::Qualified) {
        actions.push(action(
            ClaimActionKind::ReleasePreclinicalClaim,
            600_000,
            "methods reviewer",
            "release a bounded preclinical claim with all four passed gates and no clinical interpretation",
        ));
    }
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    actions.truncate(request.max_actions);
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = BTreeSet::new();
    negative_evidence.extend(
        contrast
            .negative_evidence
            .iter()
            .map(|item| format!("contrast:{item}")),
    );
    negative_evidence.extend(
        sensitivity
            .negative_evidence
            .iter()
            .map(|item| format!("sensitivity:{item}")),
    );
    negative_evidence.extend(
        replication
            .negative_evidence
            .iter()
            .map(|item| format!("replication:{item}")),
    );
    negative_evidence.extend(
        meta_analysis
            .negative_evidence
            .iter()
            .map(|item| format!("meta:{item}")),
    );
    let mut uncertainty = BTreeSet::new();
    uncertainty.extend(
        contrast
            .uncertainty
            .iter()
            .map(|item| format!("contrast:{item}")),
    );
    uncertainty.extend(
        sensitivity
            .uncertainty
            .iter()
            .map(|item| format!("sensitivity:{item}")),
    );
    uncertainty.extend(
        replication
            .uncertainty
            .iter()
            .map(|item| format!("replication:{item}")),
    );
    uncertainty.extend(
        meta_analysis
            .uncertainty
            .iter()
            .map(|item| format!("meta:{item}")),
    );
    let mut output = GliomaCausalClaimAdjudication {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        hypothesis: request.hypothesis.clone(),
        model_system: request.model_system,
        claim_effect_milli: contrast.difference_in_differences_milli,
        claim_uncertainty_milli: contrast
            .interval_high_milli
            .saturating_sub(contrast.interval_low_milli)
            .unsigned_abs()
            .saturating_add(sensitivity.leave_one_out_shift_milli)
            .saturating_add(meta_analysis.random_effect_uncertainty_milli),
        confidence_milli: claim_confidence,
        contrast,
        sensitivity,
        replication,
        meta_analysis,
        gates,
        action_order,
        actions,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({"placeholder": true}))
            .map_err(|error| GliomaCausalClaimAdjudicationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaCausalClaimAdjudicationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::sensitivity::SensitivityDirection;
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(id: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"id": id})).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> GliomaCausalClaimAdjudicationRequest {
        let objective = "test invasion mechanism claim".to_string();
        let contrast_request = CausalContrastRequest {
            objective: objective.clone(),
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            model_system: GliomaModelSystem::Organoid,
            intervention_timepoint: 2,
            min_units_per_arm: 2,
            effect_threshold_milli: 10,
            alpha_milli: 1_000,
        };
        let mut contrast_observations = Vec::new();
        for (unit, arm, baseline, post) in [
            ("c1", "control", 100, 110),
            ("c2", "control", 90, 100),
            ("t1", "perturbation", 100, 160),
            ("t2", "perturbation", 90, 150),
        ] {
            contrast_observations.push(TrajectoryObservation {
                observation_id: format!("{unit}-baseline"),
                unit_id: unit.into(),
                arm_id: arm.into(),
                model_system: GliomaModelSystem::Organoid,
                batch_id: "batch".into(),
                timepoint: 1,
                outcome_milli: baseline,
            });
            contrast_observations.push(TrajectoryObservation {
                observation_id: format!("{unit}-post"),
                unit_id: unit.into(),
                arm_id: arm.into(),
                model_system: GliomaModelSystem::Organoid,
                batch_id: "batch".into(),
                timepoint: 2,
                outcome_milli: post,
            });
        }
        let sensitivity_request = SensitivityRequest {
            objective: objective.clone(),
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            model_system: GliomaModelSystem::Organoid,
            expected_direction: SensitivityDirection::Positive,
            min_units_per_arm: 2,
            effect_threshold_milli: 10,
            max_confounder_strength_milli: 100,
            strength_step_milli: 100,
            max_leave_one_out_shift_milli: 1_000,
        };
        let sensitivity_observations = [
            ("c1", "control", 100),
            ("c2", "control", 90),
            ("c3", "control", 95),
            ("t1", "perturbation", 160),
            ("t2", "perturbation", 150),
            ("t3", "perturbation", 155),
        ]
        .into_iter()
        .map(
            |(unit, arm, outcome)| super::super::sensitivity::SensitivityObservation {
                observation_id: format!("s-{unit}"),
                unit_id: unit.into(),
                arm_id: arm.into(),
                model_system: GliomaModelSystem::Organoid,
                outcome_milli: outcome,
                confounder_score_milli: 0,
                artifact: artifact(&format!("a-{unit}")),
            },
        )
        .collect();
        let studies = vec![
            ReplicationStudy {
                study_id: "study-a".into(),
                site_id: "site-a".into(),
                model_system: GliomaModelSystem::Organoid,
                artifact: artifact("study-a"),
                effect_milli: 50,
                uncertainty_milli: 20,
                replicate_count: 3,
            },
            ReplicationStudy {
                study_id: "study-b".into(),
                site_id: "site-b".into(),
                model_system: GliomaModelSystem::Organoid,
                artifact: artifact("study-b"),
                effect_milli: 55,
                uncertainty_milli: 20,
                replicate_count: 3,
            },
        ];
        GliomaCausalClaimAdjudicationRequest {
            objective: objective.clone(),
            hypothesis: "perturbing invasion reduces the invasion endpoint".into(),
            model_system: GliomaModelSystem::Organoid,
            contrast_request,
            contrast_observations,
            sensitivity_request,
            sensitivity_observations,
            replication_request: ReplicationRequest {
                objective: objective.clone(),
                model_system: GliomaModelSystem::Organoid,
                min_sites: 2,
                min_replicates_per_site: 3,
                effect_threshold_milli: 10,
                heterogeneity_tolerance_milli: 100,
            },
            meta_request: MetaAnalysisRequest {
                objective,
                model_system: GliomaModelSystem::Organoid,
                min_studies: 2,
                min_replicates_per_study: 3,
                effect_threshold_milli: 10,
                max_i2_milli: 1_000,
                min_signal_to_noise_milli: 1,
                max_leave_one_out_shift_milli: 1_000,
            },
            studies,
            min_robust_strength_milli: 100,
            max_i2_milli: 1_000,
            min_claim_confidence_milli: 500,
            max_actions: 8,
        }
    }

    #[test]
    fn adjudication_requires_all_four_scientific_gates() {
        let output = execute_glioma_causal_claim_adjudication(&request()).unwrap();
        assert_eq!(output.disposition, CausalClaimDisposition::Qualified);
        assert_eq!(output.gates.len(), 4);
        assert!(output
            .actions
            .iter()
            .any(|action| action.kind == ClaimActionKind::ReleasePreclinicalClaim));
        output.validate().unwrap();
    }

    #[test]
    fn adjudication_preserves_unresolved_temporal_evidence_as_a_next_action() {
        let mut request = request();
        request
            .contrast_observations
            .retain(|observation| observation.observation_id != "t2-post");
        let output = execute_glioma_causal_claim_adjudication(&request).unwrap();
        assert_ne!(output.disposition, CausalClaimDisposition::Qualified);
        assert!(output
            .actions
            .iter()
            .any(|action| action.kind == ClaimActionKind::CollectTimepoints));
    }
}
