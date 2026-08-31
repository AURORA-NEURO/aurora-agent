//! Local provenance/signing inference engine (`AFA-atlashub-P18-F01`).
//!
//! Compiles content-addressed artifact lineage into a deterministic, verifiable envelope. The
//! implementation models signature material as a digest-bound witness; a deployment may attach
//! a real Sigstore/Ed25519 signer at the transport boundary. No artifact payload leaves the site.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlashub-P18-F01";
pub const CONTRACT_VERSION: &str =
    "atlashub-local-single-study-provenance-signing-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "ArtifactAndDerivation1@1";
pub const OUTPUT_SCHEMA: &str = "SignedProvenanceEnvelope1@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.atlashub-signed-provenance-envelope-1+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAndDerivation1 {
    pub artifact_id: String,
    pub content_digest: ContentHash,
    pub derivation_order: Vec<String>,
    pub source_order: Vec<String>,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub local_only: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSigningRequest1 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub signer_id: String,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub artifacts: Vec<ArtifactAndDerivation1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceArtifact1 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceEnvelope1 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub signer_id: String,
    pub disposition: String,
    pub artifact_order: Vec<String>,
    pub signed_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub signature_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub artifact: SignedProvenanceArtifact1,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProvenanceSigningInferenceError {
    #[error("invalid provenance signing request or envelope: {0}")]
    Invalid(String),
    #[error("provenance signing artifact failed: {0}")]
    Artifact(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn provenance_signing_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id:FEATURE_ID.into(), version:CONTRACT_VERSION.into(), owner_crate:"atlashub".into(), consumers:["integration engineer".into(),"provenance steward".into(),"research-object publisher".into()].into(), behavior:"compile local preclinical artifact lineage into a deterministic signer-bound provenance envelope with omission and negative-evidence witnesses".into(), value:"lets integrations prove research-object lineage and replay identity without moving raw artifacts or treating missing provenance as valid".into(), inputs:vec![TypedPort{name:"artifact_and_derivation".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"signed_provenance_envelope".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:BTreeSet::new(),permissions:["read:local-research-artifacts".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())},EvidenceReference{source_id:"ro-crate-1.3".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())},EvidenceReference{source_id:"ga4gh-drs-1.3".into(),state:EvidenceState::Supported,locator:Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A0,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

fn validate_request(r: &ProvenanceSigningRequest1) -> Result<(), ProvenanceSigningInferenceError> {
    if r.schema_version != INPUT_SCHEMA
        || [&r.request_id, &r.consumer, &r.purpose, &r.signer_id]
            .iter()
            .any(|v| v.trim().is_empty())
        || !digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.artifacts.is_empty()
    {
        return Err(ProvenanceSigningInferenceError::Invalid(
            "request identity, replay, boundary, or artifact closure is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for a in &r.artifacts {
        if a.artifact_id.trim().is_empty()
            || !ids.insert(a.artifact_id.clone())
            || !ordered(&a.derivation_order)
            || !ordered(&a.source_order)
            || !digest(&a.content_digest)
            || !digest(&a.provenance_digest)
            || a.replay_identity != r.replay_identity
        {
            return Err(ProvenanceSigningInferenceError::Invalid(
                "artifact identity, ordering, digest, or replay is invalid".into(),
            ));
        }
    }
    Ok(())
}
impl SignedProvenanceEnvelope1 {
    pub fn validate(&self) -> Result<(), ProvenanceSigningInferenceError> {
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
            || self.artifact_order.is_empty()
            || self.signer_id.trim().is_empty()
        {
            return Err(ProvenanceSigningInferenceError::Invalid(
                "envelope identity, locality, disposition, or artifacts are incomplete".into(),
            ));
        }
        for v in [
            &self.artifact_order,
            &self.signed_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.omission_order,
            &self.negative_evidence_order,
        ] {
            if !ordered(v) {
                return Err(ProvenanceSigningInferenceError::Invalid(
                    "envelope ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.artifact_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .signed_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.artifact_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(ProvenanceSigningInferenceError::Invalid(
                "artifact states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.signature_digest)
            || !digest(&self.envelope_digest)
            || self.artifact.content_hash != self.envelope_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(ProvenanceSigningInferenceError::Artifact(
                "envelope digest is inconsistent".into(),
            ));
        }
        Ok(())
    }
}
pub fn infer_signed_provenance(
    r: &ProvenanceSigningRequest1,
) -> Result<SignedProvenanceEnvelope1, ProvenanceSigningInferenceError> {
    validate_request(r)?;
    let artifact_order = r
        .artifacts
        .iter()
        .map(|a| a.artifact_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut signed = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let provenance = r
        .artifacts
        .iter()
        .map(|a| a.provenance_digest.clone())
        .collect::<BTreeSet<_>>();
    for a in &r.artifacts {
        if a.negative_result {
            negative.insert(a.artifact_id.clone());
        }
        if !a.local_only {
            blocked.insert(a.artifact_id.clone());
            omissions.insert(format!("{}:raw-data-not-local", a.artifact_id));
        } else if !matches!(
            a.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            unresolved.insert(a.artifact_id.clone());
            omissions.insert(format!("{}:evidence-state", a.artifact_id));
        } else {
            signed.insert(a.artifact_id.clone());
        }
    }
    if !r.policy_allowed {
        omissions.insert("request:policy-denied".into());
    }
    if !r.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !r.raw_data_local {
        omissions.insert("request:raw-data-not-local".into());
    }
    let global_block = !r.policy_allowed || !r.protected_closure || !r.raw_data_local;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || signed.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(artifact_order.iter().cloned());
        signed.clear();
        unresolved.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:provenance-closure-not-ready".into());
    }
    let payload = json!({"artifact_order":artifact_order,"signed_order":signed,"unresolved_order":unresolved,"blocked_order":blocked,"omission_order":omissions,"negative_evidence_order":negative,"replay_identity":r.replay_identity});
    let envelope_digest = ContentHash::of_value(&payload)
        .map_err(|e| ProvenanceSigningInferenceError::Artifact(e.to_string()))?;
    let signature_digest =
        ContentHash::of_value(&json!({"signer_id":r.signer_id,"envelope_digest":envelope_digest}))
            .map_err(|e| ProvenanceSigningInferenceError::Artifact(e.to_string()))?;
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
    let out = SignedProvenanceEnvelope1 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        consumer: r.consumer.clone(),
        purpose: r.purpose.clone(),
        signer_id: r.signer_id.clone(),
        disposition: disposition.into(),
        artifact_order: strings("artifact_order"),
        signed_order: strings("signed_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        omission_order: strings("omission_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: r.replay_identity.clone(),
        signature_digest,
        envelope_digest: envelope_digest.clone(),
        artifact: SignedProvenanceArtifact1 {
            artifact_id: format!("atlashub-provenance:{}", r.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: envelope_digest,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["provenance-not-signed-for-release".into()]
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
pub fn infer_signed_provenance_json(v: &serde_json::Value) -> Result<serde_json::Value, String> {
    let r: ProvenanceSigningRequest1 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid provenance request: {e}"))?;
    serde_json::to_value(infer_signed_provenance(&r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_signed_provenance_json(
    v: &serde_json::Value,
) -> Result<SignedProvenanceEnvelope1, String> {
    let out: SignedProvenanceEnvelope1 = serde_json::from_value(v.clone())
        .map_err(|e| format!("invalid provenance envelope: {e}"))?;
    out.validate().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> ProvenanceSigningRequest1 {
        ProvenanceSigningRequest1 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "prov-1".into(),
            consumer: "integration".into(),
            purpose: "lineage".into(),
            signer_id: "site-a".into(),
            replay_identity: h("r"),
            policy_allowed: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            artifacts: vec![ArtifactAndDerivation1 {
                artifact_id: "a".into(),
                content_digest: h("a"),
                derivation_order: vec!["step".into()],
                source_order: vec!["source".into()],
                provenance_digest: h("p"),
                replay_identity: h("r"),
                evidence_state: EvidenceState::Supported,
                local_only: true,
                negative_result: false,
            }],
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            provenance_signing_inference_engine_manifest().autonomy_tier,
            AutonomyTier::A0
        )
    }
    #[test]
    fn signed_lineage_qualifies() {
        assert_eq!(
            infer_signed_provenance(&req()).unwrap().disposition,
            "qualified"
        )
    }
    #[test]
    fn unknown_is_partial() {
        let mut r = req();
        r.artifacts[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(infer_signed_provenance(&r).unwrap().disposition, "partial")
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allowed = false;
        assert_eq!(infer_signed_provenance(&r).unwrap().disposition, "blocked")
    }
}
