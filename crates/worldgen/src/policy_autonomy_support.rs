//! Worldgen policy/autonomy admission and evidence receipt engine (`AFA-worldgen-P19-F01`).
//!
//! Computes an auditable policy receipt for bounded research actions. It only classifies caller
//! supplied metadata; it never performs the action, moves raw data, or makes a clinical decision.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P19-F01";
pub const CONTRACT_VERSION: &str =
    "worldgen-local-policy-autonomy/1.0/1.0";
pub const INPUT_SCHEMA: &str = "ActionAndAuthority3@1";
pub const OUTPUT_SCHEMA: &str = "PolicyReceipt1@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen-policy-autonomy-receipt-1+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionAndAuthority3 {
    pub action_id: String,
    pub actor: String,
    pub autonomy_tier: String,
    pub requested_effect_order: Vec<String>,
    pub scope: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub policy_allowed: bool,
    pub authority_present: bool,
    pub approval_required: bool,
    pub local_only: bool,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyInferenceRequest3 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub required_scope: String,
    pub replay_identity: ContentHash,
    pub policy_epoch: String,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub federated_summary_only: bool,
    pub boundary: String,
    pub actions: Vec<ActionAndAuthority3>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReceiptArtifact1 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReceipt1 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub required_scope: String,
    pub policy_epoch: String,
    pub disposition: String,
    pub action_order: Vec<String>,
    pub allowed_order: Vec<String>,
    pub approval_required_order: Vec<String>,
    pub local_only_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub receipt_digest: ContentHash,
    pub artifact: PolicyReceiptArtifact1,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PolicyAutonomyInferenceError {
    #[error("invalid policy/autonomy request or receipt: {0}")]
    Invalid(String),
    #[error("policy receipt artifact failed: {0}")]
    Artifact(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn policy_autonomy_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"hub".into(),consumers:["consortium administrator".into(),"policy steward".into(),"workflow operator".into()].into(),behavior:"classify prospective high-throughput research actions into auditable allow, approval, local-only, deny, or unresolved policy receipts".into(),value:"prevents policy-bounded automation from mistaking missing authority or evidence for permission".into(),inputs:vec![TypedPort{name:"action_and_authority".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"policy_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:BTreeSet::new(),permissions:["read:local-research-artifacts".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())},EvidenceReference{source_id:"ro-crate-1.3".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
fn validate_request(r: &PolicyInferenceRequest3) -> Result<(), PolicyAutonomyInferenceError> {
    if r.schema_version != INPUT_SCHEMA
        || [
            &r.request_id,
            &r.consumer,
            &r.purpose,
            &r.required_scope,
            &r.policy_epoch,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
        || !digest(&r.replay_identity)
        || !r.federated_summary_only
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.actions.is_empty()
    {
        return Err(PolicyAutonomyInferenceError::Invalid(
            "policy identity, replay, epoch, boundary, or action closure is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for a in &r.actions {
        if a.action_id.trim().is_empty()
            || !ids.insert(a.action_id.clone())
            || a.actor.trim().is_empty()
            || a.autonomy_tier.trim().is_empty()
            || a.scope.trim().is_empty()
            || !ordered(&a.requested_effect_order)
            || !digest(&a.artifact_digest)
            || !digest(&a.provenance_digest)
            || a.replay_identity != r.replay_identity
        {
            return Err(PolicyAutonomyInferenceError::Invalid(
                "action identity, effect ordering, digest, or replay is invalid".into(),
            ));
        }
    }
    Ok(())
}
impl PolicyReceipt1 {
    pub fn validate(&self) -> Result<(), PolicyAutonomyInferenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.action_order.is_empty()
        {
            return Err(PolicyAutonomyInferenceError::Invalid(
                "policy receipt identity, locality, disposition, or actions are incomplete".into(),
            ));
        }
        for v in [
            &self.action_order,
            &self.allowed_order,
            &self.approval_required_order,
            &self.local_only_order,
            &self.denied_order,
            &self.unresolved_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
        ] {
            if !ordered(v) {
                return Err(PolicyAutonomyInferenceError::Invalid(
                    "policy receipt ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .allowed_order
            .iter()
            .chain(&self.approval_required_order)
            .chain(&self.local_only_order)
            .chain(&self.denied_order)
            .chain(&self.unresolved_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.action_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(PolicyAutonomyInferenceError::Invalid(
                "policy action states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.receipt_digest)
            || self.artifact.content_hash != self.receipt_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(PolicyAutonomyInferenceError::Artifact(
                "policy receipt digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}
pub fn infer_policy_receipt(
    r: &PolicyInferenceRequest3,
) -> Result<PolicyReceipt1, PolicyAutonomyInferenceError> {
    validate_request(r)?;
    let action_order = r
        .actions
        .iter()
        .map(|a| a.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut allowed = BTreeSet::new();
    let mut approval = BTreeSet::new();
    let mut local_only = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let provenance = r
        .actions
        .iter()
        .map(|a| a.provenance_digest.clone())
        .collect::<BTreeSet<_>>();
    for a in &r.actions {
        if a.negative_result {
            negative.insert(a.action_id.clone());
        }
        if a.scope != r.required_scope || !a.policy_allowed || !a.authority_present {
            denied.insert(a.action_id.clone());
            omissions.insert(format!("{}:scope-policy-or-authority", a.action_id));
        } else if a.approval_required {
            approval.insert(a.action_id.clone());
        } else if a.local_only {
            local_only.insert(a.action_id.clone());
        } else if matches!(
            a.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative | EvidenceState::Contradicted
        ) {
            unresolved.insert(a.action_id.clone());
            uncertainty.insert(format!("{}:evidence-state", a.action_id));
        } else {
            allowed.insert(a.action_id.clone());
        }
    }
    if !r.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !r.raw_data_local {
        omissions.insert("request:raw-data-not-local".into());
    }
    let global_block = !r.protected_closure || !r.raw_data_local;
    let disposition = if global_block || !denied.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || !approval.is_empty() || !local_only.is_empty() {
        "partial"
    } else if allowed.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        denied.extend(action_order.iter().cloned());
        allowed.clear();
        approval.clear();
        local_only.clear();
        unresolved.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:policy-closure-not-ready".into());
    }
    let payload = json!({"action_order":action_order,"allowed_order":allowed,"approval_required_order":approval,"local_only_order":local_only,"denied_order":denied,"unresolved_order":unresolved,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"replay_identity":r.replay_identity});
    let rd = ContentHash::of_value(&payload)
        .map_err(|e| PolicyAutonomyInferenceError::Artifact(e.to_string()))?;
    let strings = |k: &str| {
        payload[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let out = PolicyReceipt1 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        consumer: r.consumer.clone(),
        purpose: r.purpose.clone(),
        required_scope: r.required_scope.clone(),
        policy_epoch: r.policy_epoch.clone(),
        disposition: disposition.into(),
        action_order: strings("action_order"),
        allowed_order: strings("allowed_order"),
        approval_required_order: strings("approval_required_order"),
        local_only_order: strings("local_only_order"),
        denied_order: strings("denied_order"),
        unresolved_order: strings("unresolved_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: r.replay_identity.clone(),
        receipt_digest: rd.clone(),
        artifact: PolicyReceiptArtifact1 {
            artifact_id: format!("hub-policy-receipt:{}", r.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: rd,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["action-not-executed".into()]
            },
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

/// Feature-surface aliases keep the four operating-scale wrappers type-compatible while the
/// underlying contract remains the explicit action/authority policy model.
pub type ArtifactAndDerivation = PolicyInferenceRequest3;
pub type ArtifactCandidate = ActionAndAuthority3;
pub type PolicyAutonomyRequest = PolicyInferenceRequest3;
pub type PolicyAutonomyAction = ActionAndAuthority3;
pub type SignedPolicyAutonomyEnvelope1 = PolicyReceipt1;
pub type PolicyAutonomyEvidenceState = EvidenceState;
pub type PolicyAutonomyError = PolicyAutonomyInferenceError;

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "worldgen",
        "consumers": ["consortium administrator", "policy steward", "workflow operator"],
        "behavior": format!("classify bounded research actions into allow, approval-required, local-only, deny, or unresolved policy receipts at {scale}"),
        "value": "prevents missing evidence or authority from becoming permission and keeps autonomy tier decisions auditable",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["emit:policy-receipt", "block:unsafe-policy"],
        "permissions": ["read:local-research-artifacts"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

pub fn qualify(
    request: &PolicyInferenceRequest3,
    feature_id: &str,
    contract_version: &str,
) -> Result<PolicyReceipt1, PolicyAutonomyInferenceError> {
    let mut out = infer_policy_receipt(request)?;
    out.feature_id = feature_id.to_owned();
    out.contract_version = contract_version.to_owned();
    Ok(out)
}
pub fn infer_policy_receipt_json(v: &serde_json::Value) -> Result<serde_json::Value, String> {
    let r: PolicyInferenceRequest3 =
        serde_json::from_value(v.clone()).map_err(|e| format!("invalid policy request: {e}"))?;
    serde_json::to_value(infer_policy_receipt(&r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_policy_receipt_json(v: &serde_json::Value) -> Result<PolicyReceipt1, String> {
    let o: PolicyReceipt1 =
        serde_json::from_value(v.clone()).map_err(|e| format!("invalid policy receipt: {e}"))?;
    o.validate().map_err(|e| e.to_string())?;
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> PolicyInferenceRequest3 {
        PolicyInferenceRequest3 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "policy-1".into(),
            consumer: "admin".into(),
            purpose: "admit batch".into(),
            required_scope: "organoid".into(),
            replay_identity: h("r"),
            policy_epoch: "2026q3".into(),
            protected_closure: true,
            raw_data_local: true,
            federated_summary_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            actions: vec![ActionAndAuthority3 {
                action_id: "a".into(),
                actor: "agent".into(),
                autonomy_tier: "A1".into(),
                requested_effect_order: vec!["compute".into()],
                scope: "organoid".into(),
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: h("r"),
                evidence_state: EvidenceState::Supported,
                policy_allowed: true,
                authority_present: true,
                approval_required: false,
                local_only: true,
                negative_result: false,
            }],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            policy_autonomy_inference_engine_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn local_action_partial() {
        assert_eq!(infer_policy_receipt(&req()).unwrap().disposition, "partial")
    }
    #[test]
    fn denied_action_blocks() {
        let mut r = req();
        r.actions[0].policy_allowed = false;
        assert_eq!(infer_policy_receipt(&r).unwrap().disposition, "blocked")
    }
    #[test]
    fn approval_is_partial() {
        let mut r = req();
        r.actions[0].approval_required = true;
        assert_eq!(
            infer_policy_receipt(&r).unwrap().approval_required_order,
            vec!["a"]
        )
    }
}
