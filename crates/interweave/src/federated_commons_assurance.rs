//! Federated-commons assurance harness for `AFA-interweave-P31-F27`.
//!
//! The harness verifies signed, institution-local capability declarations at prospective
//! high-throughput scale.  It creates no federation connection and never treats a declaration as
//! proof of scientific validity; the durable product is a deterministic envelope of gates,
//! omissions, uncertainty, negative evidence, and digest-bound release evidence.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-interweave-P31-F27";
pub const CONTRACT_VERSION: &str =
    "interweave-prospective-high-throughput-federated-commons-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "InterweaveFederationRequest3@1";
pub const OUTPUT_SCHEMA: &str = "InterweaveFederationEnvelope7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.interweave-federation-envelope-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweaveCapability5 {
    pub capability_id: String,
    pub provider_id: String,
    pub surface: String,
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
pub struct InterweaveFederationRequest3 {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_capability_order: Vec<String>,
    pub required_provider_order: Vec<String>,
    pub capabilities: Vec<InterweaveCapability5>,
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
pub enum InterweaveFederationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweaveFederationEnvelope7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: InterweaveFederationDisposition,
    pub capability_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub unresolved_capability_order: Vec<String>,
    pub blocked_capability_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub provider_order: Vec<String>,
    pub selected_provider_order: Vec<String>,
    pub missing_provider_order: Vec<String>,
    pub surface_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub federation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InterweaveFederationError {
    #[error("invalid interweave federation request: {0}")]
    Invalid(String),
    #[error("interweave federation artifact failed: {0}")]
    Artifact(String),
}
fn invalid(message: impl Into<String>) -> InterweaveFederationError {
    InterweaveFederationError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

impl InterweaveFederationEnvelope7 {
    pub fn validate(&self) -> Result<(), InterweaveFederationError> {
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
            || self.surface_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "federation identity, axes, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.capability_order,
            &self.selected_capability_order,
            &self.unresolved_capability_order,
            &self.blocked_capability_order,
            &self.missing_capability_order,
            &self.provider_order,
            &self.selected_provider_order,
            &self.missing_provider_order,
            &self.surface_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("federation envelope ordering is not canonical"));
            }
        }
        let all = self
            .capability_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let observed_parts = self
            .selected_capability_order
            .iter()
            .chain(self.unresolved_capability_order.iter())
            .chain(self.blocked_capability_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let missing = self
            .missing_capability_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if all.len() != self.capability_order.len()
            || observed_parts != all
            || missing.len() != self.missing_capability_order.len()
            || !missing.is_disjoint(&all)
        {
            return Err(invalid(
                "capability states do not form a complete observed partition",
            ));
        }
        let missing_providers = self
            .missing_provider_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if missing_providers.len() != self.missing_provider_order.len()
            || !missing_providers.is_disjoint(
                &self.provider_order.iter().cloned().collect::<BTreeSet<_>>(),
            )
        {
            return Err(invalid(
                "missing federation providers overlap observed providers",
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.federation_digest)
            || self.artifact.content_hash != self.federation_digest
        {
            return Err(invalid("federation replay or digest is invalid"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| InterweaveFederationError::Artifact(e.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("federation artifact content type is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("verify:federated-capability:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside federated assurance gate"));
        }
        if self.disposition == InterweaveFederationDisposition::Qualified
            && self.effect_receipts != [format!("verify:federated-capability:{}", self.request_id)]
        {
            return Err(invalid("qualified federation effect is invalid"));
        }
        if self.disposition != InterweaveFederationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified federation must block"));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, InterweaveFederationError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| InterweaveFederationError::Artifact(e.to_string()))?,
        )
        .map_err(|e| InterweaveFederationError::Artifact(e.to_string()))
    }
}

pub fn federated_commons_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "interweave".into(), consumers: ["laboratory automation engineer".into(), "federation verifier".into(), "release governance operator".into()].into(), behavior: "verifies signed institution-local interweave capability declarations at prospective high-throughput scale without opening federation connections".into(), value: "prevents incomplete or unsafe capability declarations from becoming cross-institution release evidence while retaining omissions and negative findings".into(), inputs: vec![TypedPort { name: "interweave_federation_request".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "interweave_federation_envelope".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_federated_commons(
    request: &InterweaveFederationRequest3,
) -> Result<InterweaveFederationEnvelope7, InterweaveFederationError> {
    validate_request(request)?;
    let mut capabilities = request.capabilities.clone();
    capabilities.sort_by(|a, b| {
        a.provider_id
            .cmp(&b.provider_id)
            .then(a.capability_id.cmp(&b.capability_id))
    });
    let capability_order = capabilities
        .iter()
        .map(|c| c.capability_id.clone())
        .collect::<Vec<_>>();
    let provider_order = capabilities
        .iter()
        .map(|c| c.provider_id.clone())
        .collect::<BTreeSet<_>>();
    let surface_order = capabilities
        .iter()
        .map(|c| c.surface.clone())
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut selected_providers = BTreeSet::new();
    for c in &capabilities {
        if !c.local_only || !c.aggregate_only || !c.policy_allow {
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
            if !matches!(
                c.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            ) {
                uncertainty.insert(format!("{}:evidence-state", c.capability_id));
            }
        } else {
            selected.insert(c.capability_id.clone());
            selected_providers.insert(c.provider_id.clone());
        }
        omission.extend(
            c.omission_order
                .iter()
                .map(|e| format!("{}:{e}", c.capability_id)),
        );
        if c.negative_result {
            negative.insert(format!("{}:negative-result", c.capability_id));
        }
    }
    let providers = provider_order.into_iter().collect::<Vec<_>>();
    let surfaces = surface_order.into_iter().collect::<Vec<_>>();
    let missing_c = request
        .required_capability_order
        .iter()
        .filter(|id| !capability_order.contains(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_p = request
        .required_provider_order
        .iter()
        .filter(|id| !providers.contains(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    omission.extend(
        missing_c
            .iter()
            .map(|id| format!("capability:{id}:missing")),
    );
    omission.extend(missing_p.iter().map(|id| format!("provider:{id}:missing")));
    uncertainty.extend(
        request
            .adversarial_events
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_authorized
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(capability_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omission.insert("request:federation-release-gate-blocked".into());
    }
    let disposition = if global_block {
        InterweaveFederationDisposition::Blocked
    } else if selected.is_empty() || !missing_c.is_empty() || !missing_p.is_empty() {
        InterweaveFederationDisposition::Unresolved
    } else {
        InterweaveFederationDisposition::Qualified
    };
    if disposition != InterweaveFederationDisposition::Qualified {
        omission.insert("request:federation-not-release-ready".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let effects = if disposition == InterweaveFederationDisposition::Qualified {
        vec![format!(
            "verify:federated-capability:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"capability_order":capability_order,"selected_capability_order":selected_order,"unresolved_capability_order":unresolved_order,"blocked_capability_order":blocked_order,"missing_capability_order":missing_c,"provider_order":providers,"selected_provider_order":selected_providers,"missing_provider_order":missing_p,"surface_order":surfaces,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let federation_digest = ContentHash::of_value(&payload)
        .map_err(|e| InterweaveFederationError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("interweave-federation:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        vec![SemanticLoss {
            field: "omission_order".into(),
            reason: "unresolved federation evidence remains explicit".into(),
            severity: LossSeverity::DecisionRelevant,
        }],
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "assurance-over-capability-attestations".into(),
            digest: federation_digest.clone(),
        }],
    )
    .map_err(|e| InterweaveFederationError::Artifact(e.to_string()))?;
    let r = InterweaveFederationEnvelope7 {
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
        surface_order: payload["surface_order"]
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

fn validate_request(
    request: &InterweaveFederationRequest3,
) -> Result<(), InterweaveFederationError> {
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
        return Err(invalid(
            "federation request identity, closure, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for c in &request.capabilities {
        if c.capability_id.trim().is_empty()
            || c.provider_id.trim().is_empty()
            || c.surface.trim().is_empty()
            || !ids.insert(c.capability_id.clone())
            || !digest(&c.artifact_digest)
            || !digest(&c.evidence_digest)
            || !digest(&c.provenance_digest)
            || !digest(&c.replay_identity)
            || !canonical(&c.omission_order)
        {
            return Err(invalid(
                "capability identity, digests, or ordering are invalid",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> InterweaveFederationRequest3 {
        let c = |id: &str| InterweaveCapability5 {
            capability_id: id.into(),
            provider_id: format!("provider:{id}"),
            surface: "mcp".into(),
            artifact_digest: hash(id),
            evidence_digest: hash("evidence"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            evidence_state: EvidenceState::Supported,
            signed: true,
            policy_allow: true,
            protected_closure: true,
            local_only: true,
            aggregate_only: true,
            omission_order: Vec::new(),
            negative_result: false,
        };
        InterweaveFederationRequest3 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "federation-request".into(),
            federation_id: "commons".into(),
            requester: "automation-engineer".into(),
            purpose: "capability assurance".into(),
            semantic_profile: "interweave:v1".into(),
            required_capability_order: vec!["cap:a".into(), "cap:b".into()],
            required_provider_order: vec!["provider:cap:a".into(), "provider:cap:b".into()],
            capabilities: vec![c("cap:a"), c("cap:b")],
            replay_identity: hash("replay"),
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
            federated_commons_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_is_deterministic() {
        let r = assure_federated_commons(&request()).unwrap();
        assert_eq!(r.disposition, InterweaveFederationDisposition::Qualified);
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn missing_is_unresolved() {
        let mut q = request();
        q.capabilities.pop();
        let r = assure_federated_commons(&q).unwrap();
        assert_eq!(r.disposition, InterweaveFederationDisposition::Unresolved);
        assert!(r.missing_capability_order.contains(&"cap:b".into()));
    }
    #[test]
    fn denied_is_blocked_per_item() {
        let mut q = request();
        q.capabilities[0].policy_allow = false;
        let r = assure_federated_commons(&q).unwrap();
        assert!(r.blocked_capability_order.contains(&"cap:a".into()));
    }
    #[test]
    fn adversarial_blocks_all() {
        let mut q = request();
        q.adversarial_events = vec!["poisoned-artifact".into()];
        let r = assure_federated_commons(&q).unwrap();
        assert_eq!(r.disposition, InterweaveFederationDisposition::Blocked);
        assert!(r.selected_capability_order.is_empty());
    }
    #[test]
    fn negative_is_preserved() {
        let mut q = request();
        q.capabilities[0].negative_result = true;
        let r = assure_federated_commons(&q).unwrap();
        assert!(r
            .negative_evidence_order
            .contains(&"cap:a:negative-result".into()));
    }
    #[test]
    fn replay_mismatch_is_unresolved() {
        let mut q = request();
        q.capabilities[0].replay_identity = hash("other");
        let r = assure_federated_commons(&q).unwrap();
        assert!(r.unresolved_capability_order.contains(&"cap:a".into()));
    }
}
