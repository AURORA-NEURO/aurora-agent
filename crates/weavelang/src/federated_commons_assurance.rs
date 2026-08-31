//! Prospective high-throughput WeaveLang federated-commons assurance (`AFA-weavelang-P31-F27`).
//!
//! This is a local verifier for capability declarations. It produces a deterministic, digest-bound
//! envelope and never opens a federation connection, exports raw observations, or executes a weave.
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-weavelang-P31-F27";
pub const CONTRACT_VERSION: &str =
    "weavelang-prospective-high-throughput-federated-commons-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "WeavelangFederationRequest5@1";
pub const OUTPUT_SCHEMA: &str = "WeavelangFederationEnvelope8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.weavelang-federation-envelope-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeaveCapability5 {
    pub capability_id: String,
    pub provider_id: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeavelangFederationRequest5 {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_capability_order: Vec<String>,
    pub required_provider_order: Vec<String>,
    pub capabilities: Vec<WeaveCapability5>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_authorized: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeavelangFederationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeavelangFederationArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeavelangFederationEnvelope8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: WeavelangFederationDisposition,
    pub capability_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub unresolved_capability_order: Vec<String>,
    pub blocked_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub provider_order: Vec<String>,
    pub selected_provider_order: Vec<String>,
    pub missing_provider_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub federation_digest: ContentHash,
    pub artifact: WeavelangFederationArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WeavelangFederationError {
    #[error("invalid WeaveLang federated commons request: {0}")]
    Invalid(String),
    #[error("WeaveLang federated commons artifact failed: {0}")]
    Artifact(String),
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn weavelang_federated_commons_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"weavelang".into(),consumers:["WeaveLang compiler steward".into(),"federation verifier".into(),"research automation operator".into()].into(),behavior:"verify typed WeaveLang capability declarations and federation closure at prospective high-throughput scale without opening connections".into(),value:"prevents incomplete or unsafe weave capability declarations from becoming cross-institution release evidence".into(),inputs:vec![TypedPort{name:"weavelang_federation_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"weavelang_federation_envelope".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(),permissions:["evaluate:weavelang-capabilities".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"slsa-provenance-1.2".into(),state:EvidenceState::Supported,locator:Some("https://slsa.dev/spec/v1.2/provenance".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

impl WeavelangFederationEnvelope8 {
    pub fn validate(&self) -> Result<(), WeavelangFederationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.capability_order.is_empty()
            || self.provider_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(WeavelangFederationError::Invalid(
                "federation identity, axes, locality, or effects are incomplete".into(),
            ));
        }
        for v in [
            &self.capability_order,
            &self.selected_capability_order,
            &self.unresolved_capability_order,
            &self.blocked_capability_order,
            &self.missing_capability_order,
            &self.provider_order,
            &self.selected_provider_order,
            &self.missing_provider_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(v) {
                return Err(WeavelangFederationError::Invalid(
                    "federation envelope ordering is not canonical".into(),
                ));
            }
        }
        let all = self
            .capability_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .selected_capability_order
            .iter()
            .chain(&self.unresolved_capability_order)
            .chain(&self.blocked_capability_order)
            .chain(&self.missing_capability_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if all.len() != self.capability_order.len() || parts != all {
            return Err(WeavelangFederationError::Invalid(
                "capability states do not form a complete partition".into(),
            ));
        }
        if !self
            .missing_capability_order
            .iter()
            .all(|id| self.capability_order.contains(id))
            || !self
                .missing_provider_order
                .iter()
                .all(|id| self.provider_order.contains(id))
        {
            return Err(WeavelangFederationError::Invalid(
                "missing state is outside declared axes".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.federation_digest)
            || self.artifact.content_hash != self.federation_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(WeavelangFederationError::Artifact(
                "federation replay or digest is invalid".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(WeavelangFederationError::Artifact(
                "federation artifact content type is invalid".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| !e.starts_with("verify:weavelang-federation:") && e != "block:unsafe-release")
        {
            return Err(WeavelangFederationError::Invalid(
                "effect is outside WeaveLang assurance gate".into(),
            ));
        }
        if self.disposition == WeavelangFederationDisposition::Qualified
            && self.effect_receipts != [format!("verify:weavelang-federation:{}", self.request_id)]
        {
            return Err(WeavelangFederationError::Invalid(
                "qualified federation effect is invalid".into(),
            ));
        }
        if self.disposition != WeavelangFederationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(WeavelangFederationError::Invalid(
                "non-qualified federation must block".into(),
            ));
        }
        Ok(())
    }
}

pub fn assure_weavelang_federated_commons(
    request: &WeavelangFederationRequest5,
) -> Result<WeavelangFederationEnvelope8, WeavelangFederationError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_capability_order.is_empty()
        || request.required_provider_order.is_empty()
        || request.capabilities.is_empty()
        || !canonical(&request.required_capability_order)
        || !canonical(&request.required_provider_order)
        || !canonical(&request.adversarial_events)
        || !digest(&request.replay_identity)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(WeavelangFederationError::Invalid(
            "federation request identity, closure, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut rows = request.capabilities.clone();
    rows.sort_by(|a, b| {
        a.provider_id
            .cmp(&b.provider_id)
            .then(a.capability_id.cmp(&b.capability_id))
    });
    let capability_order = request
        .required_capability_order
        .iter()
        .cloned()
        .chain(rows.iter().map(|c| c.capability_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut ids = BTreeSet::new();
    for c in &rows {
        if c.capability_id.trim().is_empty()
            || c.provider_id.trim().is_empty()
            || c.semantic_profile.trim().is_empty()
            || !ids.insert(c.capability_id.clone())
            || !digest(&c.artifact_digest)
            || !digest(&c.evidence_digest)
            || !digest(&c.provenance_digest)
            || !digest(&c.replay_identity)
            || !canonical(&c.omission_order)
        {
            return Err(WeavelangFederationError::Invalid(
                "capability identity, digests, or ordering are invalid".into(),
            ));
        }
    }
    let providers = request
        .required_provider_order
        .iter()
        .cloned()
        .chain(rows.iter().map(|c| c.provider_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for c in &rows {
        omission.extend(
            c.omission_order
                .iter()
                .map(|x| format!("{}:{x}", c.capability_id)),
        );
        if c.negative_result {
            negative.insert(format!("{}:negative-result", c.capability_id));
        }
        if c.semantic_profile != request.semantic_profile {
            unresolved.insert(c.capability_id.clone());
            uncertainty.insert(format!("{}:semantic-profile", c.capability_id));
        } else if !c.local_only || !c.aggregate_only || !c.policy_allow {
            blocked.insert(c.capability_id.clone());
            omission.insert(format!("{}:locality-or-policy", c.capability_id));
        } else if c.replay_identity != request.replay_identity
            || !c.signed
            || !c.protected_closure
            || !matches!(
                c.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(c.capability_id.clone());
            if c.replay_identity != request.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", c.capability_id));
            }
            if !c.signed {
                uncertainty.insert(format!("{}:signature-missing", c.capability_id));
            }
            if !c.protected_closure {
                uncertainty.insert(format!("{}:protected-closure", c.capability_id));
            }
        } else {
            selected.insert(c.capability_id.clone());
        }
    }
    for id in &request.required_capability_order {
        if !rows.iter().any(|c| &c.capability_id == id) {
            missing.insert(id.clone());
            omission.insert(format!("capability:{id}:missing"));
        }
    }
    let missing_provider = request
        .required_provider_order
        .iter()
        .filter(|id| !rows.iter().any(|c| &c.provider_id == *id))
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing_provider {
        omission.insert(format!("provider:{id}:missing"));
    }
    uncertainty.extend(
        request
            .adversarial_events
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_authorized
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global {
        blocked.extend(
            capability_order
                .iter()
                .filter(|id| rows.iter().any(|c| &c.capability_id == *id))
                .cloned(),
        );
        selected.clear();
        unresolved.clear();
        omission.insert("request:federation-release-gate-blocked".into());
    }
    let disposition = if global {
        WeavelangFederationDisposition::Blocked
    } else if selected.is_empty() || !missing.is_empty() || !missing_provider.is_empty() {
        WeavelangFederationDisposition::Unresolved
    } else {
        WeavelangFederationDisposition::Qualified
    };
    if disposition != WeavelangFederationDisposition::Qualified {
        omission.insert("request:federation-not-release-ready".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_order = missing.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == WeavelangFederationDisposition::Qualified {
        vec![format!(
            "verify:weavelang-federation:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let selected_providers = selected_order
        .iter()
        .filter_map(|id| {
            rows.iter()
                .find(|c| &c.capability_id == id)
                .map(|c| c.provider_id.clone())
        })
        .collect::<BTreeSet<_>>();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"capability_order":capability_order,"selected_capability_order":selected_order,"unresolved_capability_order":unresolved_order,"blocked_capability_order":blocked_order,"missing_capability_order":missing_order,"provider_order":providers,"selected_provider_order":selected_providers,"missing_provider_order":missing_provider,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"effect_receipts":effect_receipts,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let federation_digest = ContentHash::of_value(&payload)
        .map_err(|e| WeavelangFederationError::Artifact(e.to_string()))?;
    let artifact = WeavelangFederationArtifact8 {
        artifact_id: format!("weavelang-federation:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: federation_digest.clone(),
        semantic_loss: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        provenance_digests: rows.iter().map(|c| c.provenance_digest.clone()).collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let r = WeavelangFederationEnvelope8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        capability_order: payload["capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_capability_order: payload["selected_capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_capability_order: payload["unresolved_capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_capability_order: payload["blocked_capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_capability_order: payload["missing_capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        provider_order: payload["provider_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_provider_order: payload["selected_provider_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_provider_order: payload["missing_provider_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        federation_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    r.validate()?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn c(id: &str) -> WeaveCapability5 {
        WeaveCapability5 {
            capability_id: id.into(),
            provider_id: format!("provider:{id}"),
            semantic_profile: "weave:v1".into(),
            artifact_digest: h(id),
            evidence_digest: h("evidence"),
            provenance_digest: h("provenance"),
            replay_identity: h("replay"),
            evidence_state: EvidenceState::Supported,
            signed: true,
            policy_allow: true,
            protected_closure: true,
            local_only: true,
            aggregate_only: true,
            omission_order: Vec::new(),
            negative_result: false,
        }
    }
    fn q() -> WeavelangFederationRequest5 {
        WeavelangFederationRequest5 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "request".into(),
            federation_id: "commons".into(),
            requester: "compiler-steward".into(),
            purpose: "capability assurance".into(),
            semantic_profile: "weave:v1".into(),
            required_capability_order: vec!["cap:a".into(), "cap:b".into()],
            required_provider_order: vec!["provider:cap:a".into(), "provider:cap:b".into()],
            capabilities: vec![c("cap:a"), c("cap:b")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_authorized: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            weavelang_federated_commons_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified() {
        assert_eq!(
            assure_weavelang_federated_commons(&q())
                .unwrap()
                .disposition,
            WeavelangFederationDisposition::Qualified
        )
    }
    #[test]
    fn missing_is_unresolved() {
        let mut r = q();
        r.capabilities.pop();
        assert_eq!(
            assure_weavelang_federated_commons(&r).unwrap().disposition,
            WeavelangFederationDisposition::Unresolved
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = q();
        r.policy_allow = false;
        assert_eq!(
            assure_weavelang_federated_commons(&r).unwrap().disposition,
            WeavelangFederationDisposition::Blocked
        )
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            assure_weavelang_federated_commons(&q()).unwrap(),
            assure_weavelang_federated_commons(&q()).unwrap()
        )
    }
}
