//! Federated policy/autonomy interoperability (`AFA-ids-P19-F24`).
//!
//! This module turns signed, institution-local autonomy declarations into a
//! deterministic admission receipt. It does not grant authority, execute
//! actions, move raw data, or make clinical decisions.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P19-F24";
pub const CONTRACT_VERSION: &str = "ids-federated-policy-autonomy-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "AutonomyPolicyRequest7@1";
pub const OUTPUT_SCHEMA: &str = "AutonomyPolicyReceipt9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.autonomy-policy-receipt-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_ACTORS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyActor8 {
    pub actor_id: String,
    pub role: String,
    pub authority: String,
    pub scope: String,
    pub autonomy_tier: u8,
    pub permitted_actions: Vec<String>,
    pub approval_digest: ContentHash,
    pub revoked: bool,
    pub budget_units: u64,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyPolicyRequest7 {
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub scope: String,
    pub policy_version: String,
    pub required_tier: u8,
    pub requested_actions: Vec<String>,
    pub actors: Vec<AutonomyActor8>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyPolicyReceipt9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyPolicyReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub scope: String,
    pub policy_version: String,
    pub required_tier: u8,
    pub disposition: String,
    pub actor_order: Vec<String>,
    pub admitted_actor_order: Vec<String>,
    pub approval_required_actor_order: Vec<String>,
    pub denied_actor_order: Vec<String>,
    pub revoked_actor_order: Vec<String>,
    pub over_budget_actor_order: Vec<String>,
    pub scope_mismatch_order: Vec<String>,
    pub missing_authority_order: Vec<String>,
    pub requested_action_order: Vec<String>,
    pub permitted_action_order: Vec<String>,
    pub denied_action_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub budget_remaining: u64,
    pub replay_identity: ContentHash,
    pub policy_digest: ContentHash,
    pub artifact: AutonomyPolicyReceipt9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyAutonomyError {
    #[error("invalid policy/autonomy request: {0}")]
    Invalid(String),
    #[error("policy/autonomy receipt failed validation: {0}")]
    Receipt(String),
}

pub fn policy_autonomy_interoperability_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["research workflow operator", "institutional policy steward", "federation administrator", "autonomy auditor"],
        "behavior": "admit typed actor scopes and action budgets under risk-tiered federated policy",
        "value": "prevents revoked, over-scoped, over-budget, or under-authorized autonomy from crossing a research boundary",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:policy-receipts", "manage:autonomy-grant"],
        "permissions": ["read:local-authority-manifests", "request:autonomy-admission"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

impl AutonomyPolicyReceipt9 {
    pub fn validate(&self) -> Result<(), PolicyAutonomyError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.policy_version.trim().is_empty()
            || self.actor_order.is_empty()
            || self.requested_action_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(PolicyAutonomyError::Receipt(
                "policy identity, locality, actors, actions, disposition, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.actor_order,
            &self.admitted_actor_order,
            &self.approval_required_actor_order,
            &self.denied_actor_order,
            &self.revoked_actor_order,
            &self.over_budget_actor_order,
            &self.scope_mismatch_order,
            &self.missing_authority_order,
            &self.requested_action_order,
            &self.permitted_action_order,
            &self.denied_action_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(PolicyAutonomyError::Receipt(
                    "policy/autonomy ordering is not canonical".into(),
                ));
            }
        }
        let actors = BTreeSet::from_iter(self.actor_order.iter().cloned());
        let states = self
            .admitted_actor_order
            .iter()
            .chain(&self.approval_required_actor_order)
            .chain(&self.denied_actor_order)
            .cloned()
            .collect::<Vec<_>>();
        if actors.len() != self.actor_order.len()
            || states.len() != actors.len()
            || BTreeSet::from_iter(states.iter().cloned()) != actors
        {
            return Err(PolicyAutonomyError::Receipt(
                "actor states do not partition actor order".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.policy_digest)
            || self.artifact.content_hash != self.policy_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(PolicyAutonomyError::Receipt(
                "policy digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:policy-receipts:")
                && !effect.starts_with("manage:autonomy-grant:")
                && effect != "block:unsafe-release"
        }) {
            return Err(PolicyAutonomyError::Receipt(
                "effect is outside the governed autonomy gate".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &AutonomyPolicyRequest7) -> Result<(), PolicyAutonomyError> {
    if request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.policy_version.trim().is_empty()
        || request.required_tier > 4
        || request.requested_actions.is_empty()
        || request.actors.is_empty()
        || request.actors.len() > MAX_ACTORS
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(PolicyAutonomyError::Invalid(
            "policy identity, scope, tier, actions, actor bound, replay, or locality is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for actor in &request.actors {
        if actor.actor_id.trim().is_empty()
            || actor.role.trim().is_empty()
            || actor.authority.trim().is_empty()
            || actor.scope.trim().is_empty()
            || actor.autonomy_tier > 4
            || actor.permitted_actions.is_empty()
            || !valid_digest(&actor.approval_digest)
            || !ids.insert(actor.actor_id.clone())
        {
            return Err(PolicyAutonomyError::Invalid(
                "actor identity, authority, tier, actions, approval, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

pub fn admit_policy_autonomy(
    request: &AutonomyPolicyRequest7,
) -> Result<AutonomyPolicyReceipt9, PolicyAutonomyError> {
    validate_request(request)?;
    let mut actors = request.actors.clone();
    actors.sort_by(|left, right| left.actor_id.cmp(&right.actor_id));
    let actor_order = actors
        .iter()
        .map(|actor| actor.actor_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = BTreeSet::new();
    let mut approval_required = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut revoked = BTreeSet::new();
    let mut over_budget = BTreeSet::new();
    let mut scope_mismatch = BTreeSet::new();
    let mut missing_authority = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut permitted_actions = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut budget_remaining = 0_u64;
    for actor in &actors {
        let id = actor.actor_id.clone();
        provenance.insert(actor.approval_digest.clone());
        if actor.revoked {
            revoked.insert(id.clone());
            denied.insert(id.clone());
            negative.insert(format!("{id}:revoked"));
        } else if actor.autonomy_tier > request.required_tier {
            denied.insert(id.clone());
            omissions.insert(format!("{id}:autonomy-tier-exceeds-request"));
        } else if actor.scope != request.scope {
            scope_mismatch.insert(id.clone());
            denied.insert(id.clone());
            omissions.insert(format!("{id}:scope-mismatch"));
        } else if actor.authority == "" {
            missing_authority.insert(id.clone());
            approval_required.insert(id.clone());
            uncertainty.insert(format!("{id}:authority-missing"));
        } else if actor.approval_digest.as_str().len() != 64 {
            missing_authority.insert(id.clone());
            approval_required.insert(id.clone());
            uncertainty.insert(format!("{id}:approval-attestation-missing"));
        } else if actor.budget_units == 0 {
            over_budget.insert(id.clone());
            denied.insert(id.clone());
            omissions.insert(format!("{id}:budget-exhausted"));
        } else {
            admitted.insert(id.clone());
            budget_remaining = budget_remaining.saturating_add(actor.budget_units);
            permitted_actions.extend(
                actor
                    .permitted_actions
                    .iter()
                    .filter(|action| request.requested_actions.contains(action))
                    .cloned(),
            );
        }
    }
    let mut denied_actions = request
        .requested_actions
        .iter()
        .filter(|action| !permitted_actions.contains(*action))
        .cloned()
        .collect::<BTreeSet<_>>();
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        denied.extend(actor_order.iter().cloned());
        admitted.clear();
        permitted_actions.clear();
        denied_actions.extend(request.requested_actions.iter().cloned());
        omissions.insert("request:policy-protected-closure-or-federation-denied".into());
    }
    let admitted_order = admitted.iter().cloned().collect::<Vec<_>>();
    let approval_order = approval_required.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let action_order = request.requested_actions.clone();
    let permitted_order = permitted_actions.iter().cloned().collect::<Vec<_>>();
    let denied_action_order = denied_actions.iter().cloned().collect::<Vec<_>>();
    let disposition = if global_block || (permitted_order.is_empty() && approval_order.is_empty()) {
        "blocked"
    } else if !approval_order.is_empty()
        || !denied_order.is_empty()
        || denied_action_order.len() > 0
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:autonomy-admission-not-closed".into());
    }
    let mut payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "scope": request.scope,
        "policy_version": request.policy_version,
        "required_tier": request.required_tier,
        "disposition": disposition,
        "actor_order": actor_order,
        "admitted_actor_order": admitted_order,
        "approval_required_actor_order": approval_order,
        "denied_actor_order": denied_order,
        "revoked_actor_order": revoked.iter().cloned().collect::<Vec<_>>(),
        "over_budget_actor_order": over_budget.iter().cloned().collect::<Vec<_>>(),
        "scope_mismatch_order": scope_mismatch.iter().cloned().collect::<Vec<_>>(),
        "missing_authority_order": missing_authority.iter().cloned().collect::<Vec<_>>(),
        "requested_action_order": action_order,
        "permitted_action_order": permitted_order,
        "denied_action_order": denied_action_order,
        "omission_order": omissions.iter().cloned().collect::<Vec<_>>(),
        "uncertainty_order": uncertainty.iter().cloned().collect::<Vec<_>>(),
        "negative_evidence_order": negative.iter().cloned().collect::<Vec<_>>(),
        "budget_remaining": budget_remaining,
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let policy_digest = ContentHash::of_value(&payload)
        .map_err(|error| PolicyAutonomyError::Receipt(error.to_string()))?;
    payload["policy_digest"] = json!(policy_digest);
    payload["artifact"] = json!({
        "artifact_id": format!("autonomy-policy-receipt-9:{}", request.request_id),
        "content_type": CONTENT_TYPE,
        "content_hash": policy_digest,
        "semantic_loss": omissions.iter().cloned().collect::<Vec<_>>(),
        "provenance_digests": provenance.iter().cloned().collect::<Vec<_>>(),
        "boundary": PRECLINICAL_BOUNDARY,
    });
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("exchange:policy-receipts:{}", request.request_id),
            format!("manage:autonomy-grant:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let receipt: AutonomyPolicyReceipt9 = serde_json::from_value(payload)
        .map_err(|error| PolicyAutonomyError::Receipt(error.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"policy")
    }
    fn actor(id: &str) -> AutonomyActor8 {
        AutonomyActor8 {
            actor_id: id.into(),
            role: "operator".into(),
            authority: "signed-authority".into(),
            scope: "study:s1".into(),
            autonomy_tier: 1,
            permitted_actions: vec!["read:manifest".into()],
            approval_digest: hash(),
            revoked: false,
            budget_units: 10,
            local: true,
            aggregate_only: true,
        }
    }
    fn request(actors: Vec<AutonomyActor8>) -> AutonomyPolicyRequest7 {
        AutonomyPolicyRequest7 {
            request_id: "policy:req".into(),
            purpose: "research".into(),
            semantic_profile: "ome-ngff".into(),
            scope: "study:s1".into(),
            policy_version: "2026.1".into(),
            required_tier: 1,
            requested_actions: vec!["read:manifest".into()],
            actors,
            replay_identity: hash(),
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            policy_autonomy_interoperability_manifest()["autonomy_tier"],
            "A2"
        );
    }
    #[test]
    fn nominal_admission_is_qualified() {
        let r = admit_policy_autonomy(&request(vec![actor("a")])).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.permitted_action_order, vec!["read:manifest"]);
    }
    #[test]
    fn revoked_actor_is_denied() {
        let mut a = actor("a");
        a.revoked = true;
        let r = admit_policy_autonomy(&request(vec![a])).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn scope_mismatch_is_blocked() {
        let mut a = actor("a");
        a.scope = "study:s2".into();
        let r = admit_policy_autonomy(&request(vec![a])).unwrap();
        assert_eq!(r.scope_mismatch_order, vec!["a"]);
    }
    #[test]
    fn tier_overflow_is_denied() {
        let mut a = actor("a");
        a.autonomy_tier = 2;
        let r = admit_policy_autonomy(&request(vec![a])).unwrap();
        assert_eq!(r.denied_actor_order, vec!["a"]);
    }
    #[test]
    fn budget_exhaustion_is_denied() {
        let mut a = actor("a");
        a.budget_units = 0;
        let r = admit_policy_autonomy(&request(vec![a])).unwrap();
        assert_eq!(r.over_budget_actor_order, vec!["a"]);
    }
    #[test]
    fn actors_are_canonicalized() {
        let r = admit_policy_autonomy(&request(vec![actor("z"), actor("a")])).unwrap();
        assert_eq!(r.actor_order, vec!["a", "z"]);
    }
}
