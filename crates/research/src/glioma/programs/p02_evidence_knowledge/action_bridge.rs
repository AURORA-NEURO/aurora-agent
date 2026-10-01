//! Bridge validated P02 knowledge actions into the glioma action selector.
//!
//! The knowledge-action compiler deliberately stops at a dependency-closed research plan. This
//! module gives that plan a typed hand-off into the existing autonomous selector, so knowledge
//! work can compete with other local research actions without inventing an assay, exporting data,
//! or bypassing autonomy policy. The bridge is deterministic and only emits A1 local-computation
//! candidates; instrument, federation, and clinical effects remain impossible at this boundary.

use super::action_compiler::{CompiledActionDisposition, KnowledgeActionPlan};
use super::claim_frontier::FrontierActionKind;
use crate::glioma_engine::{GliomaActionCandidate, GliomaStageKind};
use bioprism_foundation::{AutonomyTier, Effect};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeActionBridge1@1";
pub const MAX_CANDIDATES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionBridgeRequest {
    pub objective: String,
    pub plan_digest: ContentHash,
    pub max_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgedKnowledgeCandidate {
    pub source_action_id: String,
    pub claim_id: String,
    pub action_kind: FrontierActionKind,
    pub rationale: String,
    pub candidate: GliomaActionCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionBridge {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub candidate_order: Vec<String>,
    pub candidates: Vec<BridgedKnowledgeCandidate>,
    pub blocked_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeActionBridgeError {
    #[error("knowledge action bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge action bridge input is invalid: {0}")]
    InvalidInput(String),
    #[error("knowledge action bridge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge action bridge digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &KnowledgeActionBridge) -> serde_json::Value {
    json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "candidate_order": output.candidate_order,
        "candidates": output.candidates,
        "blocked_order": output.blocked_order,
        "omitted_order": output.omitted_order,
        "uncertainty_order": output.uncertainty_order,
    })
}

fn candidate_id(action_id: &str) -> String {
    format!("knowledge-action:{action_id}")
}

fn stage_for(kind: FrontierActionKind) -> GliomaStageKind {
    match kind {
        FrontierActionKind::CloseCoverage => GliomaStageKind::EvidenceSurveillance,
        FrontierActionKind::ResolveContradiction => GliomaStageKind::StatisticalInterpretation,
        FrontierActionKind::ResolveUncertainty => GliomaStageKind::MultimodalIngestionQc,
        FrontierActionKind::RevalidateNegative | FrontierActionKind::ValidateSupported => {
            GliomaStageKind::ReplicationRobustness
        }
    }
}

fn feasibility(risk_milli: u64) -> u16 {
    1_000_u64.saturating_sub(risk_milli.min(1_000)) as u16
}

fn cost_units(cost_milli: u64) -> u32 {
    cost_milli
        .saturating_add(999)
        .checked_div(1_000)
        .unwrap_or(u64::MAX)
        .clamp(1, u64::from(u32::MAX)) as u32
}

fn validate_request(
    request: &KnowledgeActionBridgeRequest,
    plan: &KnowledgeActionPlan,
) -> Result<(), KnowledgeActionBridgeError> {
    if request.objective.trim().is_empty()
        || request.objective != plan.objective
        || request.plan_digest != plan.digest
        || request.max_candidates == 0
        || request.max_candidates > MAX_CANDIDATES
    {
        return Err(KnowledgeActionBridgeError::InvalidRequest(
            "objective, plan digest, and bounded candidate cap must match the validated plan"
                .into(),
        ));
    }
    plan.validate()
        .map_err(|error| KnowledgeActionBridgeError::InvalidInput(error.to_string()))
}

impl KnowledgeActionBridge {
    pub fn validate(&self) -> Result<(), KnowledgeActionBridgeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.candidate_order.len() != self.candidates.len()
            || !canonical(&self.candidate_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.omitted_order)
            || !canonical(&self.uncertainty_order)
            || self.candidates.iter().any(|entry| {
                entry.source_action_id.trim().is_empty()
                    || entry.claim_id.trim().is_empty()
                    || entry.rationale.trim().is_empty()
                    || entry.candidate.action_id != candidate_id(&entry.source_action_id)
            })
        {
            return Err(KnowledgeActionBridgeError::InvalidOutput(
                "bridge identity, ordering, rationale, or candidate binding is invalid".into(),
            ));
        }
        let candidate_ids = self
            .candidates
            .iter()
            .map(|entry| entry.candidate.action_id.clone())
            .collect::<BTreeSet<_>>();
        if candidate_ids
            != self
                .candidate_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || candidate_ids.len() != self.candidates.len()
        {
            return Err(KnowledgeActionBridgeError::InvalidOutput(
                "candidate order does not reconcile with bridged candidates".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeActionBridgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeActionBridgeError::InvalidOutput(
                "bridge digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Convert a validated knowledge-action plan into local candidates consumable by
/// `select_glioma_actions`. Selection remains a separate policy step; this bridge never dispatches
/// work and never admits instrument, federation, or clinical effects.
pub fn bridge_glioma_knowledge_actions(
    request: &KnowledgeActionBridgeRequest,
    plan: &KnowledgeActionPlan,
) -> Result<KnowledgeActionBridge, KnowledgeActionBridgeError> {
    validate_request(request, plan)?;
    let actions = plan
        .actions
        .iter()
        .filter(|action| action.disposition != CompiledActionDisposition::Blocked)
        .map(|action| (action.action_id.clone(), action))
        .collect::<BTreeMap<_, _>>();
    let mut ranking = actions
        .values()
        .map(|action| {
            let score = u128::from(action.priority_milli)
                .saturating_mul(u128::from(action.information_gain_milli))
                .saturating_mul(1_000)
                .checked_div(u128::from(action.cost_milli).max(1))
                .unwrap_or_default();
            (score, action.action_id.clone())
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| right.cmp(left));

    let mut selected = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    for (_, action_id) in ranking {
        if selected.contains(&action_id) || omitted.contains(&action_id) {
            continue;
        }
        let mut closure = BTreeSet::new();
        let mut stack = vec![action_id.clone()];
        let mut valid = true;
        while let Some(current) = stack.pop() {
            if selected.contains(&current) || !closure.insert(current.clone()) {
                continue;
            }
            let Some(action) = actions.get(&current) else {
                valid = false;
                break;
            };
            for dependency in &action.dependency_order {
                stack.push(dependency.clone());
            }
        }
        if !valid || selected.len() + closure.len() > request.max_candidates {
            omitted.insert(action_id);
        } else {
            selected.extend(closure);
        }
    }

    let mut uncertainty = plan.uncertainty_order.clone();
    let mut candidates = Vec::new();
    for action_id in &selected {
        let action = actions.get(action_id).ok_or_else(|| {
            KnowledgeActionBridgeError::InvalidInput("selected action disappeared".into())
        })?;
        let Some(modality) = action.modality_order.first().copied() else {
            omitted.insert(action_id.clone());
            continue;
        };
        let Some(model_system) = action.model_system_order.first().copied() else {
            omitted.insert(action_id.clone());
            continue;
        };
        let id = candidate_id(&action.action_id);
        let depends_on = action
            .dependency_order
            .iter()
            .map(|dependency| candidate_id(dependency))
            .collect::<Vec<_>>();
        let candidate = GliomaActionCandidate {
            action_id: id,
            stage_kind: stage_for(action.action_kind),
            modality,
            model_system,
            depends_on,
            cost_units: cost_units(action.cost_milli),
            information_gain_milli: action.information_gain_milli,
            frontier_novelty_milli: action.priority_milli,
            workflow_leverage_milli: (650_u16
                .saturating_add((action.dependency_order.len() as u16).saturating_mul(50)))
            .min(1_000),
            cross_stage_unlock_milli: if action.dependency_order.is_empty() {
                500
            } else {
                850
            },
            reproducibility_safety_milli: if action.action_kind
                == FrontierActionKind::RevalidateNegative
            {
                950
            } else {
                800
            },
            federation_value_milli: 250,
            feasibility_milli: feasibility(action.risk_milli),
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        };
        candidates.push(BridgedKnowledgeCandidate {
            source_action_id: action.action_id.clone(),
            claim_id: action.claim_id.clone(),
            action_kind: action.action_kind,
            rationale: action.reason.clone(),
            candidate,
        });
    }
    candidates.sort_by(|left, right| left.candidate.action_id.cmp(&right.candidate.action_id));
    let candidate_order = candidates
        .iter()
        .map(|entry| entry.candidate.action_id.clone())
        .collect::<Vec<_>>();
    uncertainty.extend(omitted.iter().map(|id| format!("omitted-action:{id}")));
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = KnowledgeActionBridge {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        plan_digest: plan.digest.clone(),
        candidate_order,
        candidates,
        blocked_order: plan.blocked_order.clone(),
        omitted_order: omitted.into_iter().collect(),
        uncertainty_order: uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-knowledge-action-bridge"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeActionBridgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::action_compiler::{
        CompiledActionDisposition, CompiledResearchAction,
    };

    fn plan() -> KnowledgeActionPlan {
        let mut plan = KnowledgeActionPlan {
            feature_id: "GAF-GLIOMA-P02-F16".into(),
            output_schema: "GliomaKnowledgeActionCompiler1@1".into(),
            objective: "independent invasion validation".into(),
            knowledge_digest: ContentHash::of_bytes(b"knowledge"),
            frontier_digest: ContentHash::of_bytes(b"frontier"),
            action_order: vec!["validate-invasion".into()],
            selected_order: vec!["validate-invasion".into()],
            deferred_order: Vec::new(),
            blocked_order: Vec::new(),
            actions: vec![CompiledResearchAction {
                action_id: "validate-invasion".into(),
                claim_id: "claim-invasion".into(),
                action_kind: FrontierActionKind::ValidateSupported,
                modality_order: vec![crate::glioma_engine::GliomaModality::Imaging],
                model_system_order: vec![crate::glioma_engine::GliomaModelSystem::Organoid],
                information_gain_milli: 800,
                cost_milli: 100,
                risk_milli: 100,
                dependency_order: Vec::new(),
                priority_milli: 900,
                disposition: CompiledActionDisposition::Selected,
                reason: "independent validation admitted".into(),
            }],
            selected_cost_milli: 100,
            selected_risk_milli: 100,
            selected_information_milli: 800,
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: super::super::action_compiler::KnowledgeActionPlanDisposition::Qualified,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        plan.digest = ContentHash::of_value(&super::super::action_compiler::digest_input(&plan))
            .expect("plan digest");
        plan
    }

    #[test]
    fn bridges_validated_knowledge_action_into_selector_candidate() {
        let plan = plan();
        let request = KnowledgeActionBridgeRequest {
            objective: plan.objective.clone(),
            plan_digest: plan.digest.clone(),
            max_candidates: 4,
        };
        let bridge = bridge_glioma_knowledge_actions(&request, &plan).expect("bridge");
        assert_eq!(
            bridge.candidate_order,
            vec!["knowledge-action:validate-invasion"]
        );
        assert_eq!(
            bridge.candidates[0].candidate.stage_kind,
            GliomaStageKind::ReplicationRobustness
        );
        assert_eq!(
            bridge.candidates[0].candidate.autonomy_tier,
            AutonomyTier::A1
        );
        assert!(bridge.validate().is_ok());
    }
}
