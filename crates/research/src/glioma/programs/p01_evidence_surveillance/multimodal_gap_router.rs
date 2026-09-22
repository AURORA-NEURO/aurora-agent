//! Cross-modality gap routing for the glioma evidence frontier.
//!
//! A claim can look well supported while lacking the modality or model context needed for an
//! autonomous research workflow.  This feature computes the smallest deterministic set of
//! orthogonal modality/model actions that closes those gaps, preserving negative and contradictory
//! frontiers instead of treating them as missing positives.  It emits plans only; institution-
//! local adapters own retrieval, assay execution, and data movement.

use super::evidence_frontier_join::{EvidenceFrontierClaim, EvidenceFrontierVerdict};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F14";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalEvidenceGapPlan1@1";
pub const MAX_CLAIMS: usize = 2_048;
pub const MAX_ACTIONS: usize = 4_096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalGapActionKind {
    OrthogonalEvidence,
    MultimodalValidation,
    ContradictionResolution,
    NegativeReplication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalGapDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalGapRouterRequest {
    pub objective: String,
    pub claims: Vec<EvidenceFrontierClaim>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub max_actions: usize,
    pub min_priority_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalGapAction {
    pub action_id: String,
    pub frontier_id: String,
    pub claim: String,
    pub modality: GliomaModality,
    pub model_system: GliomaModelSystem,
    pub action_kind: MultimodalGapActionKind,
    pub priority_milli: u16,
    pub expected_information_milli: u16,
    pub dependency_order: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalGapClaim {
    pub frontier_id: String,
    pub claim: String,
    pub observed_modalities: Vec<GliomaModality>,
    pub missing_modalities: Vec<GliomaModality>,
    pub observed_model_systems: Vec<GliomaModelSystem>,
    pub missing_model_systems: Vec<GliomaModelSystem>,
    pub action_order: Vec<String>,
    pub disposition: MultimodalGapDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalGapRouterPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub claims: Vec<MultimodalGapClaim>,
    pub action_order: Vec<String>,
    pub actions: Vec<MultimodalGapAction>,
    pub covered_modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub blocked_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MultimodalGapDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalGapRouterError {
    #[error("multimodal gap request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal gap output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal gap digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MultimodalGapRouterPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "claims": output.claims,
        "action_order": output.action_order,
        "actions": output.actions,
        "covered_modality_order": output.covered_modality_order,
        "missing_modality_order": output.missing_modality_order,
        "blocked_order": output.blocked_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MultimodalGapRouterPlan {
    pub fn validate(&self) -> Result<(), MultimodalGapRouterError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.action_order)
            || !canonical(&self.covered_modality_order)
            || !canonical(&self.missing_modality_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .actions
                    .iter()
                    .map(|action| action.action_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(
                &self
                    .claims
                    .iter()
                    .map(|claim| claim.frontier_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.digest.as_str().len() != 64
            || self.claims.iter().any(|claim| {
                claim.frontier_id.trim().is_empty()
                    || claim.claim.trim().is_empty()
                    || !canonical(&claim.observed_modalities)
                    || !canonical(&claim.missing_modalities)
                    || !canonical(&claim.observed_model_systems)
                    || !canonical(&claim.missing_model_systems)
                    || !canonical(&claim.action_order)
                    || claim.rationale.trim().is_empty()
            })
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.frontier_id.trim().is_empty()
                    || action.claim.trim().is_empty()
                    || action.priority_milli > 1_000
                    || action.expected_information_milli > 1_000
                    || !canonical(&action.dependency_order)
                    || action.rationale.trim().is_empty()
            })
        {
            return Err(MultimodalGapRouterError::InvalidOutput(
                "identity, canonical partitions, action contracts, or digest is invalid".into(),
            ));
        }
        let claim_ids = self
            .claims
            .iter()
            .map(|claim| claim.frontier_id.clone())
            .collect::<BTreeSet<_>>();
        let action_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        let claim_order = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let action_order = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        if claim_order != claim_ids
            || self.claim_order.len() != claim_ids.len()
            || action_order != action_ids
            || self.action_order.len() != action_ids.len()
            || self
                .actions
                .iter()
                .any(|action| !claim_ids.contains(&action.frontier_id))
            || self
                .claims
                .iter()
                .any(|claim| claim.action_order.iter().any(|id| !action_ids.contains(id)))
        {
            return Err(MultimodalGapRouterError::InvalidOutput(
                "claim/action identity or ownership is inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultimodalGapRouterError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultimodalGapRouterError::Digest(
                "multimodal gap digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Route the minimum deterministic cross-modality/model action set for each evidence frontier
/// claim.  Actions are metadata-only and remain subject to downstream policy and approval gates.
pub fn route_glioma_multimodal_evidence_gaps(
    request: &MultimodalGapRouterRequest,
) -> Result<MultimodalGapRouterPlan, MultimodalGapRouterError> {
    if request.objective.trim().is_empty()
        || request.claims.is_empty()
        || request.claims.len() > MAX_CLAIMS
        || request.required_modalities.is_empty()
        || request.required_model_systems.is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.min_priority_milli > 1_000
    {
        return Err(MultimodalGapRouterError::InvalidRequest(
            "objective, claims, modality/model requirements, action bound, or priority floor is invalid"
                .into(),
        ));
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.frontier_id.cmp(&right.frontier_id));
    claims.dedup_by(|left, right| left.frontier_id == right.frontier_id);
    let mut actions = Vec::new();
    let mut routed_claims = Vec::new();
    let mut blocked_order = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut covered_modalities = BTreeSet::new();

    for claim in claims {
        if claim.priority_milli < request.min_priority_milli {
            blocked_order.insert(claim.frontier_id.clone());
            uncertainty.insert(format!(
                "{}: below configured priority floor",
                claim.frontier_id
            ));
            routed_claims.push(MultimodalGapClaim {
                frontier_id: claim.frontier_id,
                claim: claim.claim,
                observed_modalities: Vec::new(),
                missing_modalities: request.required_modalities.iter().copied().collect(),
                observed_model_systems: Vec::new(),
                missing_model_systems: request.required_model_systems.iter().copied().collect(),
                action_order: Vec::new(),
                disposition: MultimodalGapDisposition::Blocked,
                rationale: "priority floor prevents an autonomous acquisition route".into(),
            });
            continue;
        }
        let observed_modality = claim.modality;
        let observed_model = claim.model_system;
        covered_modalities.insert(observed_modality);
        let observed_modalities = vec![observed_modality];
        let mut missing_modalities = request
            .required_modalities
            .difference(&observed_modalities.iter().copied().collect())
            .copied()
            .collect::<Vec<_>>();
        missing_modalities.sort();
        let observed_model_systems = observed_model.into_iter().collect::<Vec<_>>();
        let mut missing_model_systems = request
            .required_model_systems
            .difference(&observed_model_systems.iter().copied().collect())
            .copied()
            .collect::<Vec<_>>();
        missing_model_systems.sort();
        let action_kind = match claim.verdict {
            EvidenceFrontierVerdict::Contradicted => {
                MultimodalGapActionKind::ContradictionResolution
            }
            EvidenceFrontierVerdict::Negative => MultimodalGapActionKind::NegativeReplication,
            EvidenceFrontierVerdict::Supported => MultimodalGapActionKind::MultimodalValidation,
            EvidenceFrontierVerdict::Sparse | EvidenceFrontierVerdict::Unresolved => {
                MultimodalGapActionKind::OrthogonalEvidence
            }
        };
        let mut claim_action_ids = Vec::new();
        for modality in &missing_modalities {
            for model_system in &missing_model_systems {
                if actions.len() >= request.max_actions {
                    break;
                }
                let action_id =
                    format!("{}::{:?}::{:?}", claim.frontier_id, modality, model_system);
                let priority = claim.priority_milli.saturating_add(50).min(1_000);
                actions.push(MultimodalGapAction {
                    action_id: action_id.clone(),
                    frontier_id: claim.frontier_id.clone(),
                    claim: claim.claim.clone(),
                    modality: *modality,
                    model_system: *model_system,
                    action_kind,
                    priority_milli: priority,
                    expected_information_milli: claim.unknown_milli.max(500),
                    dependency_order: vec![claim.frontier_id.clone()],
                    rationale: "orthogonal modality/model context is missing from the frontier"
                        .into(),
                });
                claim_action_ids.push(action_id);
            }
        }
        claim_action_ids.sort();
        let disposition = if missing_modalities.is_empty() && missing_model_systems.is_empty() {
            MultimodalGapDisposition::Ready
        } else if claim_action_ids.is_empty() {
            blocked_order.insert(claim.frontier_id.clone());
            uncertainty.insert(format!(
                "{}: multimodal gap exceeds action capacity",
                claim.frontier_id
            ));
            MultimodalGapDisposition::Blocked
        } else {
            MultimodalGapDisposition::Partial
        };
        routed_claims.push(MultimodalGapClaim {
            frontier_id: claim.frontier_id,
            claim: claim.claim,
            observed_modalities,
            missing_modalities,
            observed_model_systems,
            missing_model_systems,
            action_order: claim_action_ids,
            disposition,
            rationale: match disposition {
                MultimodalGapDisposition::Ready => {
                    "required modality and model coverage is present".into()
                }
                MultimodalGapDisposition::Partial => {
                    "bounded orthogonal actions cover the remaining gaps".into()
                }
                MultimodalGapDisposition::Blocked => {
                    "no bounded action can satisfy the remaining gap".into()
                }
            },
        });
    }
    routed_claims.sort_by(|left, right| left.frontier_id.cmp(&right.frontier_id));
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let claim_order = routed_claims
        .iter()
        .map(|claim| claim.frontier_id.clone())
        .collect::<Vec<_>>();
    let covered_modality_order = covered_modalities.into_iter().collect::<Vec<_>>();
    let all_modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<Vec<_>>();
    let missing_modality_order = all_modalities
        .iter()
        .copied()
        .filter(|modality| !covered_modality_order.contains(modality))
        .collect::<Vec<_>>();
    let disposition = if !blocked_order.is_empty() && actions.is_empty() {
        MultimodalGapDisposition::Blocked
    } else if !action_order.is_empty() {
        MultimodalGapDisposition::Partial
    } else {
        MultimodalGapDisposition::Ready
    };
    let mut output = MultimodalGapRouterPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        claims: routed_claims,
        action_order,
        actions,
        covered_modality_order,
        missing_modality_order,
        blocked_order: blocked_order.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| MultimodalGapRouterError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultimodalGapRouterError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::evidence_frontier_join::EvidenceFrontierAction;
    use super::*;

    fn claim(verdict: EvidenceFrontierVerdict, modality: GliomaModality) -> EvidenceFrontierClaim {
        EvidenceFrontierClaim {
            frontier_id: format!("frontier:{modality:?}"),
            claim: "egfr resistance".into(),
            scope: "organoid".into(),
            modality,
            model_system: Some(GliomaModelSystem::Organoid),
            evidence_order: vec!["evidence-1".into()],
            independent_source_count: 1,
            support_milli: 800,
            negative_milli: 0,
            contradiction_milli: 0,
            unknown_milli: 500,
            quality_milli: 800,
            reproducibility_milli: 800,
            verdict,
            next_action: EvidenceFrontierAction::AcquireIndependentEvidence,
            priority_milli: 900,
            rationale: "test frontier".into(),
        }
    }

    fn request(claims: Vec<EvidenceFrontierClaim>) -> MultimodalGapRouterRequest {
        MultimodalGapRouterRequest {
            objective: "close glioma modality gaps".into(),
            claims,
            required_modalities: [
                GliomaModality::FunctionalPerturbation,
                GliomaModality::Imaging,
            ]
            .into_iter()
            .collect(),
            required_model_systems: [GliomaModelSystem::Organoid, GliomaModelSystem::MouseModel]
                .into_iter()
                .collect(),
            max_actions: 8,
            min_priority_milli: 700,
        }
    }

    #[test]
    fn closes_orthogonal_modality_and_model_gaps() {
        let output = route_glioma_multimodal_evidence_gaps(&request(vec![claim(
            EvidenceFrontierVerdict::Unresolved,
            GliomaModality::FunctionalPerturbation,
        )]))
        .unwrap();
        assert_eq!(output.actions.len(), 1);
        assert_eq!(output.disposition, MultimodalGapDisposition::Partial);
        output.validate().unwrap();
    }

    #[test]
    fn routes_contradiction_to_resolution_actions() {
        let output = route_glioma_multimodal_evidence_gaps(&request(vec![claim(
            EvidenceFrontierVerdict::Contradicted,
            GliomaModality::Imaging,
        )]))
        .unwrap();
        assert!(output
            .actions
            .iter()
            .all(|action| action.action_kind == MultimodalGapActionKind::ContradictionResolution));
    }

    #[test]
    fn priority_floor_blocks_unsafe_autonomous_expansion() {
        let mut input = request(vec![claim(
            EvidenceFrontierVerdict::Sparse,
            GliomaModality::Imaging,
        )]);
        input.min_priority_milli = 950;
        let output = route_glioma_multimodal_evidence_gaps(&input).unwrap();
        assert_eq!(output.disposition, MultimodalGapDisposition::Blocked);
        assert_eq!(output.actions.len(), 0);
    }
}
