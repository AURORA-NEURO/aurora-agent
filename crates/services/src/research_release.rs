//! Signed research-object publication with local-first federation boundaries.
//!
//! Atlas feature: `AFA-services-P16-F02`.
//!
//! The release compiler packages only content-addressed metadata, evidence receipts, policy
//! decisions, and localization statements. Raw experimental bytes never enter the federation
//! payload. A signing authority is supplied by the caller, while verification is independently
//! replayable with a public key at the receiving institution.

use crate::federation::{
    verify_signed_federation, FederationError, FederationSigner, SignedFederationArtifact,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReceipt,
    EvidenceReference, EvidenceState, PolicyDecision, PolicyReceipt, ProvenanceLink,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

/// Stable atlas identity for signed research-object publication.
pub const FEATURE_ID: &str = "AFA-services-P16-F02";
pub const FEATURE_CONTRACT_VERSION: &str = "0.1.0";

/// Capability manifest for the local-first signed publication compiler.
pub fn research_release_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_CONTRACT_VERSION.into(),
        owner_crate: "services".into(),
        consumers: ["research workflow operator".into(), "federation verifier".into()].into(),
        behavior: "packages typed artifact metadata and evidence receipts into a deterministic signed research object while keeping raw data local".into(),
        value: "enables portable reproducibility bundles with policy, provenance, omission, and localization evidence".into(),
        inputs: vec![TypedPort {
            name: "research_release_request".into(),
            schema: "ResearchReleaseRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "research_release_receipt".into(),
            schema: "ResearchReleaseReceipt@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport]
            .into(),
        permissions: [
            "read:local-research-artifacts".into(),
            "export:policy-approved-research-object".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "ro-crate-1.3".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "consortium release approver".into(),
            reason: "federation export is permitted only after an explicit allow policy and authority reference".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReleaseRequest {
    pub release_id: String,
    pub origin: String,
    pub purpose: String,
    pub artifacts: Vec<TypedResearchArtifact>,
    pub evidence_receipts: Vec<EvidenceReceipt>,
    pub policy: PolicyReceipt,
    pub localization_statement: String,
}

/// The portable object contains metadata and an integrity-bound manifest, never raw source bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject {
    pub release_id: String,
    pub federation: SignedFederationArtifact,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReleaseReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub release_id: String,
    pub research_object: SignedResearchObject,
    pub release_digest: ContentHash,
    pub omissions: Vec<String>,
    pub reasons: Vec<String>,
    pub boundary: String,
}

impl ResearchReleaseReceipt {
    pub fn validate(&self) -> Result<(), ResearchReleaseError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchReleaseError::Contract(format!(
                "expected schema {RESEARCH_CONTRACT_SCHEMA_VERSION}, found {}",
                self.schema_version
            )));
        }
        if self.feature_id != FEATURE_ID {
            return Err(ResearchReleaseError::InvalidRequest(
                "research-release feature id mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || self.research_object.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ResearchReleaseError::Contract(
                "research release crossed the preclinical boundary".into(),
            ));
        }
        if self.release_id.trim().is_empty()
            || self.research_object.release_id != self.release_id
            || self.research_object.artifact_ids.is_empty()
            || self.research_object.evidence_receipt_ids.is_empty()
            || self.reasons.is_empty()
        {
            return Err(ResearchReleaseError::InvalidRequest(
                "release identity, artifact ids, evidence ids, and reasons are required".into(),
            ));
        }
        unique_ids(&self.research_object.artifact_ids, "artifact")?;
        unique_ids(
            &self.research_object.evidence_receipt_ids,
            "evidence receipt",
        )?;
        self.research_object
            .federation
            .envelope
            .validate()
            .map_err(|error| ResearchReleaseError::Contract(error.to_string()))?;
        if !self.research_object.federation.envelope.raw_data_local {
            return Err(ResearchReleaseError::RawDataEgress);
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ResearchReleaseError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ResearchReleaseError {
    #[error("invalid research-release request: {0}")]
    InvalidRequest(String),
    #[error("research-release contract rejected: {0}")]
    Contract(String),
    #[error("research release policy is not allow: {0:?}")]
    PolicyBlocked(PolicyDecision),
    #[error("research release would export raw data")]
    RawDataEgress,
    #[error("duplicate {kind} id {id}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("federation signing or verification failed: {0}")]
    Federation(#[from] FederationError),
    #[error("cannot serialize research release: {0}")]
    Serialization(String),
}

/// Compile and sign a deterministic release bundle. The signer is an explicit authority boundary;
/// no private key is accepted in the transport request.
pub fn build_research_release(
    request: &ResearchReleaseRequest,
    signer: &FederationSigner,
) -> Result<ResearchReleaseReceipt, ResearchReleaseError> {
    validate_request(request)?;
    let mut artifacts = request.artifacts.clone();
    artifacts.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    let mut evidence = request.evidence_receipts.clone();
    evidence.sort_by(|left, right| left.receipt_id.cmp(&right.receipt_id));

    let mut artifact_provenance = Vec::with_capacity(artifacts.len() + evidence.len());
    for artifact in &artifacts {
        artifact_provenance.push(ProvenanceLink {
            source_id: artifact.artifact_id.clone(),
            relation: "release-includes".into(),
            digest: artifact.content_hash.clone(),
        });
    }
    let mut evidence_json = Vec::with_capacity(evidence.len());
    for receipt in &evidence {
        let value = serde_json::to_value(receipt)
            .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))?;
        let digest = ContentHash::of_value(&value)
            .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))?;
        artifact_provenance.push(ProvenanceLink {
            source_id: receipt.receipt_id.clone(),
            relation: "release-evidence".into(),
            digest,
        });
        evidence_json.push(value);
    }
    let omissions = evidence
        .iter()
        .flat_map(|receipt| {
            receipt.omissions.iter().map(|omission| {
                format!(
                    "{}:{}:{}",
                    receipt.receipt_id, omission.item, omission.reason
                )
            })
        })
        .collect::<Vec<_>>();
    let artifact_ids = artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<Vec<_>>();
    let evidence_receipt_ids = evidence
        .iter()
        .map(|receipt| receipt.receipt_id.clone())
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "release_id": request.release_id,
        "artifacts": artifacts,
        "evidence_receipts": evidence_json,
        "policy": request.policy,
        "omissions": omissions,
        "raw_data_local": true,
        "localization_statement": request.localization_statement,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let release_artifact = TypedResearchArtifact::from_payload(
        format!("research-release:{}", request.release_id),
        "application/vnd.aurora.signed-research-object+json",
        &payload,
        Vec::new(),
        artifact_provenance,
    )
    .map_err(|error| ResearchReleaseError::Contract(error.to_string()))?;
    let federation = signer.sign(
        request.origin.clone(),
        request.purpose.clone(),
        release_artifact,
        payload,
        &request.policy,
    )?;
    let research_object = SignedResearchObject {
        release_id: request.release_id.clone(),
        federation,
        artifact_ids,
        evidence_receipt_ids,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let reasons = if omissions.is_empty() {
        vec!["all evidence receipts were included; raw data remains local".into()]
    } else {
        vec![format!(
            "{} evidence omissions are preserved in the signed release manifest",
            omissions.len()
        )]
    };
    let mut receipt = ResearchReleaseReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        release_id: request.release_id.clone(),
        research_object,
        release_digest: ContentHash::of_bytes(b"placeholder"),
        omissions,
        reasons,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let digest_value = serde_json::to_value(&receipt.research_object)
        .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))?;
    receipt.release_digest = ContentHash::of_value(&digest_value)
        .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

/// Verify the signature, payload hash, boundary, and release identity at a receiving institution.
pub fn verify_research_release(
    receipt: &ResearchReleaseReceipt,
    verifying_key: &VerifyingKey,
) -> Result<(), ResearchReleaseError> {
    receipt.validate()?;
    let digest_value = serde_json::to_value(&receipt.research_object)
        .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))?;
    let expected_digest = ContentHash::of_value(&digest_value)
        .map_err(|error| ResearchReleaseError::Serialization(error.to_string()))?;
    if expected_digest != receipt.release_digest {
        return Err(ResearchReleaseError::Contract(
            "release digest does not match the signed research object".into(),
        ));
    }
    verify_signed_federation(&receipt.research_object.federation, verifying_key)?;
    let payload = &receipt.research_object.federation.payload;
    if payload.get("release_id").and_then(Value::as_str) != Some(receipt.release_id.as_str())
        || payload.get("raw_data_local").and_then(Value::as_bool) != Some(true)
    {
        return Err(ResearchReleaseError::Contract(
            "signed payload identity or localization claim is inconsistent".into(),
        ));
    }
    Ok(())
}

fn validate_request(request: &ResearchReleaseRequest) -> Result<(), ResearchReleaseError> {
    for (field, value) in [
        ("release_id", request.release_id.as_str()),
        ("origin", request.origin.as_str()),
        ("purpose", request.purpose.as_str()),
        (
            "localization_statement",
            request.localization_statement.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            return Err(ResearchReleaseError::InvalidRequest(format!(
                "{field} is required"
            )));
        }
    }
    if !request
        .localization_statement
        .to_ascii_lowercase()
        .contains("local")
    {
        return Err(ResearchReleaseError::RawDataEgress);
    }
    if request.artifacts.is_empty() || request.evidence_receipts.is_empty() {
        return Err(ResearchReleaseError::InvalidRequest(
            "at least one artifact and evidence receipt are required".into(),
        ));
    }
    let mut artifact_ids = BTreeSet::new();
    for artifact in &request.artifacts {
        artifact
            .validate_metadata()
            .map_err(|error| ResearchReleaseError::Contract(error.to_string()))?;
        if !artifact_ids.insert(artifact.artifact_id.clone()) {
            return Err(ResearchReleaseError::DuplicateId {
                kind: "artifact",
                id: artifact.artifact_id.clone(),
            });
        }
    }
    let mut evidence_ids = BTreeSet::new();
    for receipt in &request.evidence_receipts {
        receipt
            .validate()
            .map_err(|error| ResearchReleaseError::Contract(error.to_string()))?;
        if !evidence_ids.insert(receipt.receipt_id.clone()) {
            return Err(ResearchReleaseError::DuplicateId {
                kind: "evidence receipt",
                id: receipt.receipt_id.clone(),
            });
        }
    }
    request
        .policy
        .validate()
        .map_err(|error| ResearchReleaseError::Contract(error.to_string()))?;
    if request.policy.decision != PolicyDecision::Allow {
        return Err(ResearchReleaseError::PolicyBlocked(request.policy.decision));
    }
    let evaluated = request
        .policy
        .evaluated_artifacts
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request
        .artifacts
        .iter()
        .any(|artifact| !evaluated.contains(&artifact.content_hash))
    {
        return Err(ResearchReleaseError::InvalidRequest(
            "policy must evaluate every exported artifact".into(),
        ));
    }
    Ok(())
}

fn unique_ids(ids: &[String], kind: &'static str) -> Result<(), ResearchReleaseError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(ResearchReleaseError::DuplicateId {
                kind,
                id: id.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{
        EvidenceAvailability, EvidenceSource, EvidenceState, PRECLINICAL_BOUNDARY,
    };
    use ed25519_dalek::SigningKey;

    fn request() -> ResearchReleaseRequest {
        let source = EvidenceSource {
            source_id: "synthetic-study".into(),
            source_type: "fixture".into(),
            locator: "local://fixture".into(),
            digest: Some(ContentHash::of_bytes(b"fixture")),
            availability: EvidenceAvailability::Available,
        };
        let evidence = EvidenceReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "evidence:release-1".into(),
            intent: "publish a reproducibility bundle".into(),
            sources: vec![source],
            derivation: vec!["typed fixture aggregation".into()],
            uncertainty: vec![],
            omissions: vec![],
            competing_explanations: vec![],
            negative_evidence: vec![],
            conclusion_state: EvidenceState::Supported,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let payload = json!({"result": "supported", "raw_data_local": true});
        let artifact = TypedResearchArtifact::from_payload(
            "artifact:release-1",
            "application/json",
            &payload,
            vec![],
            vec![],
        )
        .unwrap();
        let policy = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy:release-1".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["consortium release approval".into()],
            evaluated_artifacts: vec![artifact.content_hash.clone()],
            authority_reference: Some("approval:release-1".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        ResearchReleaseRequest {
            release_id: "release-1".into(),
            origin: "institution-a".into(),
            purpose: "reproducibility benchmark".into(),
            artifacts: vec![artifact],
            evidence_receipts: vec![evidence],
            policy,
            localization_statement: "raw experimental data remains local at origin".into(),
        }
    }

    #[test]
    fn release_is_signed_and_replayable() {
        let signer =
            FederationSigner::new("release-key", SigningKey::from_bytes(&[9u8; 32])).unwrap();
        let receipt = build_research_release(&request(), &signer).unwrap();
        verify_research_release(&receipt, &signer.verifying_key()).unwrap();
        assert_eq!(
            receipt.research_object.artifact_ids,
            vec!["artifact:release-1"]
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn policy_must_cover_every_artifact() {
        let signer =
            FederationSigner::new("release-key", SigningKey::from_bytes(&[9u8; 32])).unwrap();
        let mut request = request();
        request.policy.evaluated_artifacts.clear();
        assert!(matches!(
            build_research_release(&request, &signer).unwrap_err(),
            ResearchReleaseError::InvalidRequest(_)
        ));
    }

    #[test]
    fn local_only_policy_cannot_be_signed() {
        let signer =
            FederationSigner::new("release-key", SigningKey::from_bytes(&[9u8; 32])).unwrap();
        let mut request = request();
        request.policy.decision = PolicyDecision::LocalOnly;
        assert!(matches!(
            build_research_release(&request, &signer).unwrap_err(),
            ResearchReleaseError::PolicyBlocked(PolicyDecision::LocalOnly)
        ));
    }
}
