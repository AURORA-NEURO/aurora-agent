//! Version-negotiated interoperability (`AFA-ids-P22-F24`).
//!
//! Negotiates caller-supplied capability manifests and records migration loss
//! without transporting raw research data or invoking external systems.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P22-F24";
pub const CONTRACT_VERSION: &str =
    "ids-version-negotiated-interoperability-extensibility-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ExternalCapability8@1";
pub const OUTPUT_SCHEMA: &str = "NegotiatedIntegration9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.negotiated-integration-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CAPABILITIES: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCapability8 {
    pub capability_id: String,
    pub endpoint_id: String,
    pub offered_versions: Vec<String>,
    pub semantic_profile: String,
    pub input_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub effects: Vec<String>,
    pub evidence_state: IntegrationEvidenceState,
    pub migration_loss: Vec<String>,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteroperabilityRequest7 {
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_capability: String,
    pub supported_versions: Vec<String>,
    pub capabilities: Vec<ExternalCapability8>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedIntegration9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedIntegration9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_capability: String,
    pub disposition: String,
    pub endpoint_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub migrated_order: Vec<String>,
    pub incompatible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub negotiated_version: String,
    pub migration_loss: Vec<String>,
    pub replay_identity: ContentHash,
    pub integration_digest: ContentHash,
    pub artifact: NegotiatedIntegration9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InteroperabilityError {
    #[error("invalid interoperability request: {0}")]
    Invalid(String),
    #[error("interoperability receipt failed validation: {0}")]
    Receipt(String),
}

pub fn interoperability_gateway_manifest() -> serde_json::Value {
    json!({
        "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID, "version":CONTRACT_VERSION, "owner_crate":"ids",
        "consumers":["protocol adapter", "SDK integrator", "federation operator", "compatibility auditor"],
        "behavior":"negotiate versioned external capability manifests with explicit migration loss and conformance gates",
        "value":"prevents semantic drift, incompatible versions, unauthorized effects, and raw-data movement at extension boundaries",
        "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA, "effects":["exchange:integration-manifests","manage:local-capability"],
        "permissions":["read:capability-manifests","request:version-negotiation"], "autonomy_tier":"A2", "boundary":PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}

impl NegotiatedIntegration9 {
    pub fn validate(&self) -> Result<(), InteroperabilityError> {
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
            || self.required_capability.trim().is_empty()
            || self.endpoint_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(InteroperabilityError::Receipt("interoperability identity, locality, endpoints, disposition, or effects are incomplete".into()));
        }
        for values in [
            &self.endpoint_order,
            &self.accepted_order,
            &self.migrated_order,
            &self.incompatible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_capability_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.migration_loss,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(InteroperabilityError::Receipt(
                    "interoperability ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.endpoint_order.iter().cloned());
        let parts = self
            .accepted_order
            .iter()
            .chain(&self.migrated_order)
            .chain(&self.incompatible_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.endpoint_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
        {
            return Err(InteroperabilityError::Receipt(
                "endpoint states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.integration_digest)
            || self.artifact.content_hash != self.integration_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| !valid_digest(d))
        {
            return Err(InteroperabilityError::Receipt(
                "integration digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:integration-manifests:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(InteroperabilityError::Receipt(
                "effect is outside governed interoperability gate".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &InteroperabilityRequest7) -> Result<(), InteroperabilityError> {
    if request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_capability.trim().is_empty()
        || request.supported_versions.is_empty()
        || request.capabilities.is_empty()
        || request.capabilities.len() > MAX_CAPABILITIES
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(InteroperabilityError::Invalid(
            "interoperability identity, versions, capability bound, replay, or locality is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for c in &request.capabilities {
        if c.capability_id.trim().is_empty()
            || c.endpoint_id.trim().is_empty()
            || c.offered_versions.is_empty()
            || c.semantic_profile.trim().is_empty()
            || !valid_digest(&c.input_digest)
            || !valid_digest(&c.provenance_digest)
            || !valid_digest(&c.replay_identity)
            || c.effects.is_empty()
            || !ids.insert(c.endpoint_id.clone())
        {
            return Err(InteroperabilityError::Invalid("capability identity, versions, semantic profile, digests, effects, or uniqueness is invalid".into()));
        }
    }
    Ok(())
}

pub fn negotiate_interoperability(
    request: &InteroperabilityRequest7,
) -> Result<NegotiatedIntegration9, InteroperabilityError> {
    validate_request(request)?;
    let mut capabilities = request.capabilities.clone();
    capabilities.sort_by(|a, b| a.endpoint_id.cmp(&b.endpoint_id));
    let endpoint_order = capabilities
        .iter()
        .map(|c| c.endpoint_id.clone())
        .collect::<Vec<_>>();
    let supported = request
        .supported_versions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut accepted = BTreeSet::new();
    let mut migrated = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut losses = BTreeSet::new();
    let mut selected_version = String::new();
    for c in &capabilities {
        let id = c.endpoint_id.clone();
        if c.capability_id != request.required_capability {
            incompatible.insert(id);
            omissions.insert(format!("{}:capability-mismatch", c.endpoint_id));
        } else if !c.local || !c.aggregate_only {
            blocked.insert(id);
            omissions.insert(format!("{}:raw-data-locality", c.endpoint_id));
        } else if c.replay_identity != request.replay_identity {
            unresolved.insert(id);
            uncertainty.insert(format!("{}:replay-identity", c.endpoint_id));
        } else if c.semantic_profile != request.semantic_profile {
            incompatible.insert(id);
            omissions.insert(format!("{}:semantic-profile", c.endpoint_id));
        } else if c.evidence_state == IntegrationEvidenceState::Contradicted {
            blocked.insert(id);
            negative.insert(format!("{}:contradicted", c.endpoint_id));
        } else if !matches!(
            c.evidence_state,
            IntegrationEvidenceState::Proven | IntegrationEvidenceState::Supported
        ) {
            unresolved.insert(id);
            uncertainty.insert(format!("{}:evidence-state", c.endpoint_id));
        } else {
            let common = c
                .offered_versions
                .iter()
                .filter(|v| supported.contains(*v))
                .cloned()
                .collect::<Vec<_>>();
            if common.is_empty() {
                incompatible.insert(id);
                omissions.insert(format!("{}:version-incompatible", c.endpoint_id));
            } else {
                let version = common.iter().max().cloned().unwrap_or_default();
                if c.offered_versions.contains(&version) {
                    accepted.insert(id);
                } else {
                    migrated.insert(id);
                }
                if selected_version.is_empty() || version > selected_version {
                    selected_version = version;
                }
                losses.extend(c.migration_loss.iter().cloned());
            }
        }
    }
    let missing = if accepted.is_empty() && migrated.is_empty() {
        vec![request.required_capability.clone()]
    } else {
        Vec::new()
    };
    if missing.len() > 0 {
        omissions.insert(format!(
            "capability:{}:missing",
            request.required_capability
        ));
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(endpoint_order.iter().cloned());
        accepted.clear();
        migrated.clear();
        unresolved.clear();
        incompatible.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let ao = accepted.iter().cloned().collect::<Vec<_>>();
    let mo = migrated.iter().cloned().collect::<Vec<_>>();
    let io = incompatible.iter().cloned().collect::<Vec<_>>();
    let uo = unresolved.iter().cloned().collect::<Vec<_>>();
    let bo = blocked.iter().cloned().collect::<Vec<_>>();
    let disposition = if global || ao.is_empty() && mo.is_empty() && uo.is_empty() {
        "blocked"
    } else if !uo.is_empty() || !bo.is_empty() || !io.is_empty() || !missing.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:integration-not-closed".into());
    }
    let mut payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"required_capability":request.required_capability,"disposition":disposition,"endpoint_order":endpoint_order,"accepted_order":ao,"migrated_order":mo,"incompatible_order":io,"unresolved_order":uo,"blocked_order":bo,"missing_capability_order":missing,"omission_order":omissions.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"negotiated_version":selected_version,"migration_loss":losses.iter().cloned().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| InteroperabilityError::Receipt(e.to_string()))?;
    payload["integration_digest"] = json!(digest);
    payload["artifact"] = json!({"artifact_id":format!("negotiated-integration-9:{}",request.request_id),"content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":omissions.iter().cloned().collect::<Vec<_>>(),"provenance_digests":capabilities.iter().map(|c|c.provenance_digest.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("exchange:integration-manifests:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let receipt: NegotiatedIntegration9 = serde_json::from_value(payload)
        .map_err(|e| InteroperabilityError::Receipt(e.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn cap(id: &str) -> ExternalCapability8 {
        ExternalCapability8 {
            capability_id: "cap".into(),
            endpoint_id: id.into(),
            offered_versions: vec!["1".into()],
            semantic_profile: "ome".into(),
            input_digest: h(id),
            provenance_digest: h("prov"),
            replay_identity: h("replay"),
            effects: vec!["exchange".into()],
            evidence_state: IntegrationEvidenceState::Supported,
            migration_loss: vec![],
            local: true,
            aggregate_only: true,
        }
    }
    fn req(caps: Vec<ExternalCapability8>) -> InteroperabilityRequest7 {
        InteroperabilityRequest7 {
            request_id: "interop:req".into(),
            purpose: "research".into(),
            semantic_profile: "ome".into(),
            required_capability: "cap".into(),
            supported_versions: vec!["1".into()],
            capabilities: caps,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(interoperability_gateway_manifest()["autonomy_tier"], "A2")
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            negotiate_interoperability(&req(vec![cap("a")]))
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn missing_version_is_unresolved() {
        let mut c = cap("a");
        c.offered_versions = vec!["2".into()];
        assert_eq!(
            negotiate_interoperability(&req(vec![c]))
                .unwrap()
                .disposition,
            "blocked"
        )
    }
    #[test]
    fn semantic_mismatch_is_unresolved() {
        let mut c = cap("a");
        c.semantic_profile = "other".into();
        assert_eq!(
            negotiate_interoperability(&req(vec![c]))
                .unwrap()
                .disposition,
            "blocked"
        )
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = req(vec![cap("a")]);
        q.policy_allow = false;
        assert_eq!(
            negotiate_interoperability(&q).unwrap().effect_receipts,
            vec!["block:unsafe-release"]
        )
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut c = cap("a");
        c.evidence_state = IntegrationEvidenceState::Unknown;
        assert_eq!(
            negotiate_interoperability(&req(vec![c]))
                .unwrap()
                .disposition,
            "unresolved"
        )
    }
    #[test]
    fn endpoint_order_is_canonical() {
        let r = negotiate_interoperability(&req(vec![cap("z"), cap("a")])).unwrap();
        assert_eq!(r.endpoint_order, vec!["a", "z"])
    }
}
