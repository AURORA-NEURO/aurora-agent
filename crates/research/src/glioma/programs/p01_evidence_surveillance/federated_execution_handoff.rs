//! Policy-bounded handoff of glioma acquisition actions to institution-local adapters.
//!
//! This is the execution boundary for P01: a selected action becomes a typed local request only
//! when an unexpired site approval grants that exact action.  The compiler creates no network
//! session, moves no raw data, and cannot authorize clinical or human-subject work.  A site-local
//! adapter may consume the handoff and later return an outcome to `acquisition_feedback`.

use super::federated_acquisition_policy::{
    FederatedAcquisitionAction, FederatedEvidenceAcquisitionPolicy,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedExecutionHandoff1@1";
pub const MAX_APPROVALS: usize = 512;
pub const MAX_HANDOFFS: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffEffect {
    ReadSiteLocalInputs,
    RunPreclinicalAcquisition,
    WriteSiteLocalArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffDisposition {
    Ready,
    HeldApproval,
    HeldCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedHandoffApproval {
    pub grant_id: String,
    pub actor: String,
    pub site_id: String,
    pub expires_epoch: u32,
    pub max_actions: usize,
    pub permitted_action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedExecutionHandoffRequest {
    pub objective: String,
    pub policy: FederatedEvidenceAcquisitionPolicy,
    pub approvals: Vec<FederatedHandoffApproval>,
    pub execution_epoch: u32,
    pub max_handoffs: usize,
    pub require_explicit_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedExecutionHandoff {
    pub handoff_id: String,
    pub action_id: String,
    pub need_id: String,
    pub site_id: String,
    pub approval_grant_id: Option<String>,
    pub local_request_digest: ContentHash,
    pub effect_order: Vec<HandoffEffect>,
    pub autonomy_tier: String,
    pub expires_epoch: u32,
    pub idempotency_key: String,
    pub compensation: String,
    pub disposition: HandoffDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedExecutionHandoffReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub policy_digest: ContentHash,
    pub handoff_order: Vec<String>,
    pub handoffs: Vec<FederatedExecutionHandoff>,
    pub ready_order: Vec<String>,
    pub held_approval_order: Vec<String>,
    pub held_capacity_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_routes: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedExecutionHandoffError {
    #[error("federated handoff request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated handoff output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated handoff digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn request_digest(action: &FederatedAcquisitionAction) -> ContentHash {
    ContentHash::of_value(&serde_json::json!({
        "action_id": action.action_id,
        "need_id": action.need_id,
        "site_id": action.site_id,
        "independence_group": action.independence_group,
        "local_raw_data_required": action.local_raw_data_required,
        "effects": ["read_site_local_inputs", "run_preclinical_acquisition", "write_site_local_artifact"],
    }))
    .expect("JSON request digest is infallible")
}

fn digest_input(output: &FederatedExecutionHandoffReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "policy_digest": output.policy_digest,
        "handoff_order": output.handoff_order,
        "handoffs": output.handoffs,
        "ready_order": output.ready_order,
        "held_approval_order": output.held_approval_order,
        "held_capacity_order": output.held_capacity_order,
        "uncertainty": output.uncertainty,
        "next_routes": output.next_routes,
    })
}

impl FederatedExecutionHandoffReport {
    pub fn validate(&self) -> Result<(), FederatedExecutionHandoffError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.policy_digest.as_str().len() != 64
            || !canonical(&self.handoff_order)
            || !canonical(&self.ready_order)
            || !canonical(&self.held_approval_order)
            || !canonical(&self.held_capacity_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.next_routes)
            || !canonical(
                &self
                    .handoffs
                    .iter()
                    .map(|handoff| handoff.handoff_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.handoffs.iter().any(|handoff| {
                handoff.handoff_id.trim().is_empty()
                    || handoff.action_id.trim().is_empty()
                    || handoff.need_id.trim().is_empty()
                    || handoff.site_id.trim().is_empty()
                    || handoff.local_request_digest.as_str().len() != 64
                    || !canonical(&handoff.effect_order)
                    || handoff.effect_order.is_empty()
                    || !matches!(
                        handoff.autonomy_tier.as_str(),
                        "A1_policy_bounded_local" | "A2_policy_approved_local"
                    )
                    || handoff.expires_epoch == 0
                    || handoff.idempotency_key.trim().is_empty()
                    || handoff.compensation.trim().is_empty()
            })
            || self.digest.as_str().len() != 64
        {
            return Err(FederatedExecutionHandoffError::InvalidOutput(
                "identity, canonical partitions, effect contract, locality, or digest is invalid"
                    .into(),
            ));
        }
        let handoff_ids = self
            .handoffs
            .iter()
            .map(|handoff| handoff.handoff_id.clone())
            .collect::<BTreeSet<_>>();
        let handoff_order = self.handoff_order.iter().cloned().collect::<BTreeSet<_>>();
        let action_ids = self
            .handoffs
            .iter()
            .map(|handoff| handoff.action_id.clone())
            .collect::<BTreeSet<_>>();
        let classified = self
            .ready_order
            .iter()
            .chain(self.held_approval_order.iter())
            .chain(self.held_capacity_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if handoff_order != handoff_ids
            || handoff_order.len() != self.handoff_order.len()
            || classified != action_ids
            || classified.len()
                != self.ready_order.len()
                    + self.held_approval_order.len()
                    + self.held_capacity_order.len()
            || self.ready_order.iter().any(|id| {
                !self
                    .handoffs
                    .iter()
                    .any(|h| h.action_id == *id && h.disposition == HandoffDisposition::Ready)
            })
            || self.held_approval_order.iter().any(|id| {
                !self.handoffs.iter().any(|h| {
                    h.action_id == *id && h.disposition == HandoffDisposition::HeldApproval
                })
            })
            || self.held_capacity_order.iter().any(|id| {
                !self.handoffs.iter().any(|h| {
                    h.action_id == *id && h.disposition == HandoffDisposition::HeldCapacity
                })
            })
        {
            return Err(FederatedExecutionHandoffError::InvalidOutput(
                "handoff identity or disposition partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedExecutionHandoffError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedExecutionHandoffError::Digest(
                "handoff digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Compile local adapter requests for selected acquisition actions. Exact action approvals are
/// required when configured; held actions remain visible and are not silently dispatched.
pub fn compile_federated_glioma_execution_handoff(
    request: &FederatedExecutionHandoffRequest,
) -> Result<FederatedExecutionHandoffReport, FederatedExecutionHandoffError> {
    if request.objective.trim().is_empty()
        || request.execution_epoch == 0
        || request.max_handoffs == 0
        || request.max_handoffs > MAX_HANDOFFS
        || request.approvals.len() > MAX_APPROVALS
        || request.policy.objective != request.objective
        || !request.policy.raw_data_local_only
    {
        return Err(FederatedExecutionHandoffError::InvalidRequest(
            "objective, bounds, policy binding, or local-data policy is invalid".into(),
        ));
    }
    request
        .policy
        .validate()
        .map_err(|error| FederatedExecutionHandoffError::InvalidRequest(error.to_string()))?;
    let mut approvals = BTreeMap::new();
    for approval in &request.approvals {
        if approval.grant_id.trim().is_empty()
            || approval.actor.trim().is_empty()
            || approval.site_id.trim().is_empty()
            || approval.expires_epoch <= request.execution_epoch
            || approval.max_actions == 0
            || !canonical(&approval.permitted_action_ids)
            || approvals
                .insert(approval.grant_id.clone(), approval)
                .is_some()
        {
            return Err(FederatedExecutionHandoffError::InvalidRequest(
                "approvals must be unique, unexpired, bounded, and canonical".into(),
            ));
        }
    }
    let mut handoffs = Vec::new();
    let mut ready_order = BTreeSet::new();
    let mut held_approval_order = BTreeSet::new();
    let mut held_capacity_order = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut approval_use = BTreeMap::<String, usize>::new();
    for action in request.policy.actions.iter().take(request.max_handoffs) {
        let matching = approvals.values().find(|approval| {
            approval.site_id == action.site_id
                && approval
                    .permitted_action_ids
                    .binary_search(&action.action_id)
                    .is_ok()
                && approval.expires_epoch > request.execution_epoch
        });
        let (disposition, grant_id) = if !request.require_explicit_approval && matching.is_none() {
            (HandoffDisposition::Ready, None)
        } else if let Some(approval) = matching {
            let used = approval_use.get(&approval.grant_id).copied().unwrap_or(0);
            if used >= approval.max_actions {
                held_capacity_order.insert(action.action_id.clone());
                uncertainty.insert(format!(
                    "{}: approval grant capacity is exhausted",
                    action.action_id
                ));
                (
                    HandoffDisposition::HeldCapacity,
                    Some(approval.grant_id.clone()),
                )
            } else {
                approval_use.insert(approval.grant_id.clone(), used + 1);
                ready_order.insert(action.action_id.clone());
                (HandoffDisposition::Ready, Some(approval.grant_id.clone()))
            }
        } else {
            held_approval_order.insert(action.action_id.clone());
            uncertainty.insert(format!(
                "{}: exact site/action approval is missing",
                action.action_id
            ));
            (HandoffDisposition::HeldApproval, None)
        };
        let effects = vec![
            HandoffEffect::ReadSiteLocalInputs,
            HandoffEffect::RunPreclinicalAcquisition,
            HandoffEffect::WriteSiteLocalArtifact,
        ];
        let local_request_digest = request_digest(action);
        let approved = grant_id.is_some();
        handoffs.push(FederatedExecutionHandoff {
            handoff_id: format!("handoff::{}", action.action_id),
            action_id: action.action_id.clone(),
            need_id: action.need_id.clone(),
            site_id: action.site_id.clone(),
            approval_grant_id: grant_id,
            local_request_digest,
            effect_order: effects,
            autonomy_tier: if approved {
                "A2_policy_approved_local".into()
            } else {
                "A1_policy_bounded_local".into()
            },
            expires_epoch: request.execution_epoch.saturating_add(1),
            idempotency_key: format!("glioma:{}:{}", request.execution_epoch, action.action_id),
            compensation:
                "on partial failure, retain local artifacts and return an explicit failure outcome"
                    .into(),
            disposition,
        });
    }
    if request.policy.actions.len() > request.max_handoffs {
        uncertainty.insert("handoff output truncated at max_handoffs".into());
    }
    handoffs.sort_by(|left, right| left.handoff_id.cmp(&right.handoff_id));
    let handoff_order = handoffs
        .iter()
        .map(|handoff| handoff.handoff_id.clone())
        .collect::<Vec<_>>();
    let next_routes = vec![
        "glioma_evidence_acquisition_feedback".into(),
        "glioma_evidence_frontier_join".into(),
    ];
    let mut output = FederatedExecutionHandoffReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        policy_digest: request.policy.digest.clone(),
        handoff_order,
        handoffs,
        ready_order: ready_order.into_iter().collect(),
        held_approval_order: held_approval_order.into_iter().collect(),
        held_capacity_order: held_capacity_order.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        next_routes,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| FederatedExecutionHandoffError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedExecutionHandoffError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::federated_acquisition_policy::{
        plan_federated_glioma_evidence_acquisition, FederatedAcquisitionKind,
        FederatedAcquisitionPolicyRequest, FederatedAcquisitionSite, FederatedEvidenceNeed,
    };
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn policy() -> FederatedEvidenceAcquisitionPolicy {
        plan_federated_glioma_evidence_acquisition(&FederatedAcquisitionPolicyRequest {
            objective: "close glioma claim".into(),
            needs: vec![FederatedEvidenceNeed {
                need_id: "need-1".into(),
                claim_key: "claim-1".into(),
                modality: GliomaModality::FunctionalPerturbation,
                model_system: Some(GliomaModelSystem::Organoid),
                action_kind: FederatedAcquisitionKind::AssayReplication,
                priority_milli: 900,
                expected_information_milli: 850,
                estimated_cost_units: 10,
                required_sites: 1,
            }],
            sites: vec![FederatedAcquisitionSite {
                site_id: "site-a".into(),
                independence_group: "north".into(),
                supported_modalities: [GliomaModality::FunctionalPerturbation]
                    .into_iter()
                    .collect(),
                supported_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
                capability_milli: 900,
                independence_milli: 900,
                privacy_risk_milli: 50,
                cost_multiplier_milli: 1_000,
                local_only: true,
                revoked: false,
            }],
            budget_units: 20,
            max_actions: 4,
            max_site_privacy_risk_milli: 200,
            privacy_budget_milli: 500,
            min_site_capability_milli: 700,
            min_independence_milli: 500,
            require_local_raw_data: true,
        })
        .unwrap()
    }

    fn request(approvals: Vec<FederatedHandoffApproval>) -> FederatedExecutionHandoffRequest {
        FederatedExecutionHandoffRequest {
            objective: "close glioma claim".into(),
            policy: policy(),
            approvals,
            execution_epoch: 4,
            max_handoffs: 4,
            require_explicit_approval: true,
        }
    }

    #[test]
    fn approved_action_becomes_a_local_handoff() {
        let output =
            compile_federated_glioma_execution_handoff(&request(vec![FederatedHandoffApproval {
                grant_id: "grant-1".into(),
                actor: "site-admin".into(),
                site_id: "site-a".into(),
                expires_epoch: 10,
                max_actions: 1,
                permitted_action_ids: vec!["need-1::site-a".into()],
            }]))
            .unwrap();
        assert_eq!(output.ready_order, vec!["need-1::site-a"]);
        assert!(output.held_approval_order.is_empty());
        assert_eq!(output.handoffs[0].disposition, HandoffDisposition::Ready);
        output.validate().unwrap();
    }

    #[test]
    fn missing_approval_holds_without_dispatch() {
        let output = compile_federated_glioma_execution_handoff(&request(Vec::new())).unwrap();
        assert!(output.ready_order.is_empty());
        assert_eq!(output.held_approval_order, vec!["need-1::site-a"]);
    }

    #[test]
    fn expired_approval_is_rejected_before_handoff() {
        let input = request(vec![FederatedHandoffApproval {
            grant_id: "grant-1".into(),
            actor: "site-admin".into(),
            site_id: "site-a".into(),
            expires_epoch: 4,
            max_actions: 1,
            permitted_action_ids: vec!["need-1::site-a".into()],
        }]);
        assert!(compile_federated_glioma_execution_handoff(&input).is_err());
    }
}
