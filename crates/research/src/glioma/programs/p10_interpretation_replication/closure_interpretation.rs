//! Convert replication-closure campaign results into uncertainty-aware interpretation.
//!
//! A closure campaign can establish that a replication workflow ran, but the research engine
//! still needs to combine that result with causal, trajectory, sensitivity, and other evidence
//! families.  This feature adds only the observed campaign-level replication summaries to the
//! existing cross-family synthesis algorithm.  It never turns a held, unresolved, or negative
//! round into positive evidence and never makes a clinical decision.

use super::campaign::GliomaReplicationCampaignDisposition;
use super::replication_closure_campaign::{
    ReplicationClosureCampaignError, ReplicationClosureCampaignRun,
};
use super::synthesis::{
    synthesize_glioma_interpretation, InterpretationEvidence, InterpretationEvidenceDirection,
    InterpretationEvidenceFamily, InterpretationSynthesis, InterpretationSynthesisError,
    InterpretationSynthesisRequest,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaClosureInterpretation1@1";
pub const MAX_CAMPAIGN_EVIDENCE: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureInterpretationRequest {
    pub campaign: ReplicationClosureCampaignRun,
    pub synthesis: InterpretationSynthesisRequest,
    pub minimum_campaign_evidence: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureInterpretationDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureInterpretationRun {
    pub feature_id: String,
    pub output_schema: String,
    pub campaign_id: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub campaign_digest: ContentHash,
    pub campaign_evidence_order: Vec<String>,
    pub synthesis: InterpretationSynthesis,
    pub disposition: ClosureInterpretationDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClosureInterpretationError {
    #[error("closure interpretation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("closure campaign is invalid: {0}")]
    Campaign(#[from] ReplicationClosureCampaignError),
    #[error("interpretation synthesis failed: {0}")]
    Synthesis(#[from] InterpretationSynthesisError),
    #[error("closure interpretation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("closure interpretation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn direction_for(
    disposition: GliomaReplicationCampaignDisposition,
    effect: i64,
) -> InterpretationEvidenceDirection {
    match disposition {
        GliomaReplicationCampaignDisposition::Qualified if effect > 0 => {
            InterpretationEvidenceDirection::Positive
        }
        GliomaReplicationCampaignDisposition::Qualified if effect < 0 => {
            InterpretationEvidenceDirection::Negative
        }
        GliomaReplicationCampaignDisposition::Negative if effect < 0 => {
            InterpretationEvidenceDirection::Negative
        }
        GliomaReplicationCampaignDisposition::Negative => InterpretationEvidenceDirection::Null,
        GliomaReplicationCampaignDisposition::Partial => InterpretationEvidenceDirection::Mixed,
        GliomaReplicationCampaignDisposition::Unresolved
        | GliomaReplicationCampaignDisposition::Failed
        | GliomaReplicationCampaignDisposition::Blocked => {
            InterpretationEvidenceDirection::Unresolved
        }
        GliomaReplicationCampaignDisposition::Qualified => InterpretationEvidenceDirection::Null,
    }
}

fn digest_input(output: &ClosureInterpretationRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "campaign_id": output.campaign_id,
        "objective": output.objective,
        "model_system": output.model_system,
        "campaign_digest": output.campaign_digest,
        "campaign_evidence_order": output.campaign_evidence_order,
        "synthesis": output.synthesis,
        "disposition": output.disposition,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn validate_request(
    request: &ClosureInterpretationRequest,
) -> Result<(), ClosureInterpretationError> {
    request.campaign.validate()?;
    if request.minimum_campaign_evidence > MAX_CAMPAIGN_EVIDENCE
        || request.synthesis.objective != request.campaign.objective
        || request.synthesis.model_system != request.campaign.model_system
        || request.synthesis.hypothesis.trim().is_empty()
    {
        return Err(ClosureInterpretationError::InvalidRequest(
            "synthesis objective/model/hypothesis and campaign-evidence limit must match the closure campaign".into(),
        ));
    }
    Ok(())
}

fn campaign_evidence(
    request: &ClosureInterpretationRequest,
) -> Result<Vec<InterpretationEvidence>, ClosureInterpretationError> {
    let mut evidence = Vec::new();
    for (index, round) in request.campaign.rounds.iter().enumerate() {
        let Some(campaign) = round.execution.campaign.as_ref() else {
            continue;
        };
        let effect = campaign.final_meta_analysis.random_effect_milli;
        let uncertainty = campaign
            .final_meta_analysis
            .random_effect_uncertainty_milli
            .max(1);
        let evidence_id = format!(
            "closure:{}:round:{}",
            request.campaign.campaign_id,
            index + 1
        );
        let artifact = LocalArtifactRef {
            artifact_id: format!("closure-interpretation:{}", evidence_id),
            content_hash: campaign.digest.clone(),
            content_type: "application/vnd.aurora.glioma.closure-interpretation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let sample_count = u32::try_from(campaign.studies.len())
            .unwrap_or(u32::MAX)
            .max(1);
        evidence.push(InterpretationEvidence {
            evidence_id,
            family: InterpretationEvidenceFamily::Replication,
            independent_group: format!("{}:round:{}", request.campaign.campaign_id, index + 1),
            model_system: campaign.model_system,
            direction: direction_for(campaign.disposition, effect),
            effect_milli: effect.unsigned_abs(),
            uncertainty_milli: uncertainty,
            quality_milli: if campaign.studies.len() >= 2 {
                800
            } else {
                500
            },
            sample_count,
            artifact,
            negative_evidence: campaign.negative_evidence.clone(),
        });
    }
    if evidence.len() > MAX_CAMPAIGN_EVIDENCE {
        return Err(ClosureInterpretationError::InvalidRequest(
            "closure campaign contains too many executable evidence rounds".into(),
        ));
    }
    Ok(evidence)
}

impl ClosureInterpretationRun {
    pub fn validate(&self) -> Result<(), ClosureInterpretationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.campaign_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || !canonical(&self.campaign_evidence_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.next_action.trim().is_empty()
            || self.synthesis.objective != self.objective
            || self.synthesis.model_system != self.model_system
            || self
                .campaign_evidence_order
                .iter()
                .any(|id| self.synthesis.evidence_order.binary_search(id).is_err())
        {
            return Err(ClosureInterpretationError::InvalidOutput(
                "identity, evidence ordering/binding, synthesis, boundary, or next-action invariant failed".into(),
            ));
        }
        self.synthesis
            .validate()
            .map_err(ClosureInterpretationError::Synthesis)?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClosureInterpretationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClosureInterpretationError::InvalidOutput(
                "closure interpretation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Append bounded closure-campaign summaries to the existing cross-family interpreter.
pub fn interpret_glioma_replication_closure(
    request: &ClosureInterpretationRequest,
) -> Result<ClosureInterpretationRun, ClosureInterpretationError> {
    validate_request(request)?;
    let generated = campaign_evidence(request)?;
    if generated.len() < request.minimum_campaign_evidence {
        return Err(ClosureInterpretationError::InvalidRequest(format!(
            "closure campaign produced {} evidence rounds but {} are required",
            generated.len(),
            request.minimum_campaign_evidence
        )));
    }
    let generated_ids = generated
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<BTreeSet<_>>();
    if request
        .synthesis
        .evidence
        .iter()
        .any(|item| generated_ids.contains(&item.evidence_id))
    {
        return Err(ClosureInterpretationError::InvalidRequest(
            "synthesis evidence already contains a generated closure evidence id".into(),
        ));
    }
    let mut synthesis_request = request.synthesis.clone();
    synthesis_request.evidence.extend(generated.clone());
    synthesis_request.require_replication_family = true;
    let synthesis = synthesize_glioma_interpretation(&synthesis_request)?;
    let disposition = match synthesis.disposition {
        super::synthesis::InterpretationSynthesisDisposition::Qualified => {
            ClosureInterpretationDisposition::Qualified
        }
        super::synthesis::InterpretationSynthesisDisposition::Negative => {
            ClosureInterpretationDisposition::Negative
        }
        super::synthesis::InterpretationSynthesisDisposition::Partial => {
            ClosureInterpretationDisposition::Partial
        }
        super::synthesis::InterpretationSynthesisDisposition::Unresolved => {
            ClosureInterpretationDisposition::Unresolved
        }
    };
    let mut negative_evidence = synthesis.negative_evidence.clone();
    negative_evidence.extend(
        request
            .campaign
            .rounds
            .iter()
            .flat_map(|round| round.execution.negative_evidence.clone()),
    );
    let mut uncertainty = synthesis.uncertainty.clone();
    uncertainty.extend(
        request
            .campaign
            .rounds
            .iter()
            .flat_map(|round| round.execution.uncertainty.clone()),
    );
    let next_action = match disposition {
        ClosureInterpretationDisposition::Qualified => {
            "hold the cross-family interpretation for independent methods review and release gating"
        }
        ClosureInterpretationDisposition::Negative => {
            "publish the negative interpretation with its estimand, model boundary, and failed alternatives"
        }
        ClosureInterpretationDisposition::Partial => {
            "open the next bounded evidence frontier for the unresolved mechanism or model gap"
        }
        ClosureInterpretationDisposition::Unresolved => {
            "acquire or adjudicate missing, contradictory, or underpowered evidence before concluding"
        }
    };
    let mut output = ClosureInterpretationRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        campaign_id: request.campaign.campaign_id.clone(),
        objective: request.campaign.objective.clone(),
        model_system: request.campaign.model_system,
        campaign_digest: request.campaign.digest.clone(),
        campaign_evidence_order: generated
            .iter()
            .map(|item| item.evidence_id.clone())
            .collect(),
        synthesis,
        disposition,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-closure-interpretation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClosureInterpretationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_objective_drift_before_synthesis() {
        let campaign = ReplicationClosureCampaignRun {
            feature_id: "GAF-GLIOMA-P10-F29".into(),
            output_schema: "GliomaReplicationClosureCampaign1@1".into(),
            campaign_id: "campaign".into(),
            objective: "closure objective".into(),
            model_system: GliomaModelSystem::Organoid,
            replay_identity: ContentHash::of_bytes(b"replay"),
            rounds: Vec::new(),
            completed_action_order: Vec::new(),
            held_round_order: Vec::new(),
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            budget_reserved_units: 0,
            budget_used_units: 0,
            remaining_budget_units: 0,
            disposition: super::super::replication_closure_campaign::ReplicationClosureCampaignDisposition::Held,
            stop_reason: super::super::replication_closure_campaign::ReplicationClosureCampaignStopReason::NoFrontiers,
            next_action: "supply a frontier".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let error = interpret_glioma_replication_closure(&ClosureInterpretationRequest {
            campaign,
            synthesis: InterpretationSynthesisRequest {
                objective: "different objective".into(),
                hypothesis: "hypothesis".into(),
                model_system: GliomaModelSystem::Organoid,
                min_evidence: 1,
                min_independent_groups: 1,
                min_families: 1,
                min_quality_milli: 1,
                effect_threshold_milli: 1,
                max_disagreement_milli: 1,
                max_leave_one_out_shift_milli: 1,
                require_replication_family: true,
                replay_identity: ContentHash::of_bytes(b"synthesis"),
                evidence: Vec::new(),
            },
            minimum_campaign_evidence: 0,
        })
        .unwrap_err();
        assert!(matches!(error, ClosureInterpretationError::Campaign(_)));
    }
}
