//! Local interoperability workbench (`AFA-oracle-P22-F17`).
//!
//! Provides an A0, read-only compatibility surface for external research capabilities. It
//! negotiates schemas and evidence, preserving semantic loss and negative results; it never
//! invokes a provider or grants execution authority.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oracle-P22-F17";
pub const CONTRACT_VERSION: &str =
    "oracle-local-single-study-interoperability-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ExternalCapability1@1";
pub const OUTPUT_SCHEMA: &str = "NegotiatedIntegration5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.oracle-negotiated-integration-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCapability1 {
    pub capability_id: String,
    pub provider: String,
    pub version: String,
    pub schema_order: Vec<String>,
    pub standard_order: Vec<String>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub supported: bool,
    pub enabled: bool,
    pub local_only: bool,
    pub semantic_loss_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCapabilityRequest1 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_capability: String,
    pub required_schema_order: Vec<String>,
    pub required_standard_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub capabilities: Vec<ExternalCapability1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedIntegrationArtifact5 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegotiatedIntegration5 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_capability: String,
    pub disposition: String,
    pub capability_order: Vec<String>,
    pub compatible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub integration_digest: ContentHash,
    pub artifact: NegotiatedIntegrationArtifact5,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InteroperabilityWorkbenchError {
    #[error("invalid interoperability workbench request or receipt: {0}")]
    Invalid(String),
    #[error("interoperability workbench artifact failed: {0}")]
    Artifact(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn interoperability_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "oracle".into(), consumers: ["bioinformatician".into(), "extension steward".into(), "benchmark curator".into()].into(), behavior: "negotiate external research capability schemas and standards with deterministic compatibility, semantic-loss, and provenance witnesses".into(), value: "gives bioinformaticians a portable workbench view of compatible extensions without invoking untrusted providers or hiding limitations".into(), inputs: vec![TypedPort { name: "external_capability_request".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "negotiated_integration".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::new(), permissions: ["view:authorized-research-state".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(r: &ExternalCapabilityRequest1) -> Result<(), InteroperabilityWorkbenchError> {
    if r.schema_version != INPUT_SCHEMA
        || [&r.request_id, &r.consumer, &r.purpose, &r.target_capability]
            .iter()
            .any(|v| v.trim().is_empty())
        || !ordered(&r.required_schema_order)
        || !ordered(&r.required_standard_order)
        || !digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.capabilities.is_empty()
    {
        return Err(InteroperabilityWorkbenchError::Invalid(
            "request identity, ordering, replay, boundary, or capability closure is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for c in &r.capabilities {
        if c.capability_id.trim().is_empty()
            || !ids.insert(c.capability_id.clone())
            || c.provider.trim().is_empty()
            || c.version.trim().is_empty()
            || !ordered(&c.schema_order)
            || !ordered(&c.standard_order)
            || !ordered(&c.semantic_loss_order)
            || !digest(&c.artifact_digest)
            || !digest(&c.provenance_digest)
            || c.replay_identity != r.replay_identity
        {
            return Err(InteroperabilityWorkbenchError::Invalid(
                "capability identity, ordering, digest, or replay is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl NegotiatedIntegration5 {
    pub fn validate(&self) -> Result<(), InteroperabilityWorkbenchError> {
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
            || self.capability_order.is_empty()
        {
            return Err(InteroperabilityWorkbenchError::Invalid(
                "integration identity, locality, disposition, or capability closure is incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.capability_order,
            &self.compatible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.semantic_loss_order,
            &self.negative_evidence_order,
        ] {
            if !ordered(v) {
                return Err(InteroperabilityWorkbenchError::Invalid(
                    "integration ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .capability_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let states = self
            .compatible_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.capability_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(InteroperabilityWorkbenchError::Invalid(
                "capability states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.integration_digest)
            || self.artifact.content_hash != self.integration_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(InteroperabilityWorkbenchError::Artifact(
                "integration digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}

pub fn negotiate_integration(
    r: &ExternalCapabilityRequest1,
) -> Result<NegotiatedIntegration5, InteroperabilityWorkbenchError> {
    validate_request(r)?;
    let capability_order = r
        .capabilities
        .iter()
        .map(|c| c.capability_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut compatible = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let provenance = r
        .capabilities
        .iter()
        .map(|c| c.provenance_digest.clone())
        .collect::<BTreeSet<_>>();
    for c in &r.capabilities {
        if c.negative_result {
            negative.insert(c.capability_id.clone());
        }
        semantic_loss.extend(
            c.semantic_loss_order
                .iter()
                .map(|x| format!("{}:{}", c.capability_id, x)),
        );
        let schema_ok = r
            .required_schema_order
            .iter()
            .all(|x| c.schema_order.contains(x));
        let standard_ok = r
            .required_standard_order
            .iter()
            .all(|x| c.standard_order.contains(x));
        if !c.enabled || !c.local_only || !c.supported {
            blocked.insert(c.capability_id.clone());
            semantic_loss.insert(format!(
                "{}:disabled-unsupported-or-nonlocal",
                c.capability_id
            ));
        } else if c.capability_id != r.target_capability || !schema_ok || !standard_ok {
            unresolved.insert(c.capability_id.clone());
            semantic_loss.insert(format!("{}:schema-or-standard-mismatch", c.capability_id));
        } else if matches!(
            c.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            compatible.insert(c.capability_id.clone());
        } else {
            unresolved.insert(c.capability_id.clone());
            semantic_loss.insert(format!("{}:evidence-state", c.capability_id));
        }
    }
    let global_block = !r.policy_allowed || !r.protected_closure || !r.raw_data_local;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || compatible.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(capability_order.iter().cloned());
        compatible.clear();
        unresolved.clear();
        semantic_loss.insert("request:policy-protected-closure-or-locality-blocked".into());
    }
    if disposition != "qualified" {
        semantic_loss.insert("request:integration-closure-not-ready".into());
    }
    let payload = json!({"capability_order":capability_order,"compatible_order":compatible,"unresolved_order":unresolved,"blocked_order":blocked,"semantic_loss_order":semantic_loss,"negative_evidence_order":negative,"replay_identity":r.replay_identity});
    let integration_digest = ContentHash::of_value(&payload)
        .map_err(|e| InteroperabilityWorkbenchError::Artifact(e.to_string()))?;
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
    let out = NegotiatedIntegration5 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        consumer: r.consumer.clone(),
        purpose: r.purpose.clone(),
        target_capability: r.target_capability.clone(),
        disposition: disposition.into(),
        capability_order: strings("capability_order"),
        compatible_order: strings("compatible_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        semantic_loss_order: strings("semantic_loss_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: r.replay_identity.clone(),
        integration_digest: integration_digest.clone(),
        artifact: NegotiatedIntegrationArtifact5 {
            artifact_id: format!("oracle-negotiated-integration:{}", r.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: integration_digest,
            semantic_loss: vec!["provider-not-invoked".into()],
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}
pub fn negotiate_integration_json(v: &serde_json::Value) -> Result<serde_json::Value, String> {
    let r: ExternalCapabilityRequest1 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid interoperability request: {e}"))?;
    serde_json::to_value(negotiate_integration(&r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_interoperability_workbench_json(
    v: &serde_json::Value,
) -> Result<NegotiatedIntegration5, String> {
    let out: NegotiatedIntegration5 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid interoperability receipt: {e}"))?;
    out.validate().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> ExternalCapabilityRequest1 {
        ExternalCapabilityRequest1 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "cap-1".into(),
            consumer: "bioinformatician".into(),
            purpose: "inspect extension".into(),
            target_capability: "aligner".into(),
            required_schema_order: vec!["input:v1".into()],
            required_standard_order: vec!["ro-crate".into()],
            replay_identity: h("r"),
            policy_allowed: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            capabilities: vec![ExternalCapability1 {
                capability_id: "aligner".into(),
                provider: "local".into(),
                version: "1".into(),
                schema_order: vec!["input:v1".into()],
                standard_order: vec!["ro-crate".into()],
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: h("r"),
                evidence_state: EvidenceState::Supported,
                supported: true,
                enabled: true,
                local_only: true,
                semantic_loss_order: vec![],
                negative_result: false,
            }],
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            interoperability_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A0
        )
    }
    #[test]
    fn exact_capability_qualifies() {
        assert_eq!(
            negotiate_integration(&req()).unwrap().disposition,
            "qualified"
        )
    }
    #[test]
    fn mismatch_is_partial() {
        let mut r = req();
        r.capabilities[0].schema_order = vec!["other".into()];
        assert_eq!(negotiate_integration(&r).unwrap().disposition, "partial")
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allowed = false;
        assert_eq!(negotiate_integration(&r).unwrap().disposition, "blocked")
    }
}
