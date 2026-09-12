//! Governance-owned typed release contract with migration and signature gates.
//!
//! Atlas feature: `AFA-governance-P16-F08`.
//!
//! This module is the governance boundary for portable research-object publication. It validates
//! policy and provenance before constructing a signed metadata object, keeps raw data local, and
//! verifies the detached Ed25519 signature over the caller's content digest before release.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, PolicyDecision,
    PolicyReceipt, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-governance-P16-F08";
pub const CONTRACT_VERSION: &str = "signed-research-object/2.0";
pub const MAX_ID_COUNT: usize = 4096;

pub fn research_release_contract_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "governance".into(),
        consumers: ["research data steward".into(), "federation verifier".into()].into(),
        behavior: "validates policy, provenance, schema migration, and detached signatures before constructing a local-first signed research-object envelope".into(),
        value: "makes publication compatibility and signing gates independently replayable without exporting raw preclinical data".into(),
        inputs: vec![TypedPort { name: "validated_research_run".into(), schema: "ValidatedResearchRun4@1".into(), required: true }],
        outputs: vec![TypedPort { name: "signed_research_object".into(), schema: "SignedResearchObject2@2".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into(), "export:policy-approved-research-object".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: Vec::new(),
        authority_requirements: vec![AuthorityRequirement { role: "consortium release approver".into(), reason: "policy authority and detached signature are required before release".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun {
    pub schema_version: String,
    pub feature_id: String,
    pub run_id: String,
    pub release_id: String,
    pub origin: String,
    pub purpose: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub policy: PolicyReceipt,
    pub provenance_complete: bool,
    pub raw_data_local: bool,
    pub localization_statement: String,
    pub source_contract_version: String,
    pub signer_public_key_hex: String,
    pub signer_signature_hex: String,
    pub omissions: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub run_id: String,
    pub release_id: String,
    pub origin: String,
    pub purpose: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub signer_public_key_hex: String,
    pub signer_signature_hex: String,
    pub migration_notes: Vec<String>,
    pub omissions: Vec<String>,
    pub raw_data_local: bool,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl SignedResearchObject {
    pub fn validate(&self) -> Result<(), GovernanceReleaseError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.run_id.trim().is_empty()
            || self.release_id.trim().is_empty()
            || self.origin.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.artifact_ids.is_empty()
            || self.evidence_receipt_ids.is_empty()
            || self.migration_notes.is_empty()
        {
            return Err(GovernanceReleaseError::InvalidField(
                "identity, provenance, migration, localization, or boundary".into(),
            ));
        }
        unique_ids(&self.artifact_ids, "artifact")?;
        unique_ids(&self.evidence_receipt_ids, "evidence")?;
        verify_detached_signature(
            &self.release_digest,
            &self.signer_public_key_hex,
            &self.signer_signature_hex,
        )?;
        self.artifact
            .validate_metadata()
            .map_err(|error| GovernanceReleaseError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, GovernanceReleaseError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| GovernanceReleaseError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| GovernanceReleaseError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum GovernanceReleaseError {
    #[error("invalid governance release field: {0}")]
    InvalidField(String),
    #[error("duplicate {kind} id {id}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("policy gate rejected release: {0}")]
    Policy(String),
    #[error("detached signature is invalid: {0}")]
    Signature(String),
    #[error("research release artifact error: {0}")]
    Artifact(String),
    #[error("research release serialization error: {0}")]
    Serialization(String),
}

pub fn compile_signed_research_object(
    run: &ValidatedResearchRun,
) -> Result<SignedResearchObject, GovernanceReleaseError> {
    validate_run(run)?;
    let mut artifact_ids = run.artifact_ids.clone();
    artifact_ids.sort();
    let mut evidence_receipt_ids = run.evidence_receipt_ids.clone();
    evidence_receipt_ids.sort();
    let migration_notes = if run.source_contract_version == CONTRACT_VERSION {
        vec!["source contract is current; no semantic migration applied".into()]
    } else {
        vec![format!("additive migration applied from {} to {CONTRACT_VERSION}; unknown fields remain explicit", run.source_contract_version)]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "run_id": run.run_id,
        "release_id": run.release_id,
        "origin": run.origin,
        "purpose": run.purpose,
        "artifact_ids": artifact_ids,
        "evidence_receipt_ids": evidence_receipt_ids,
        "release_digest": run.release_digest,
        "signer_public_key_hex": run.signer_public_key_hex,
        "signer_signature_hex": run.signer_signature_hex,
        "migration_notes": migration_notes,
        "omissions": run.omissions,
        "raw_data_local": true,
        "localization_statement": run.localization_statement,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("signed-research-object:{}", run.release_id),
        "application/vnd.aurora.signed-research-object+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| GovernanceReleaseError::Artifact(error.to_string()))?;
    let object = SignedResearchObject {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        run_id: run.run_id.clone(),
        release_id: run.release_id.clone(),
        origin: run.origin.clone(),
        purpose: run.purpose.clone(),
        artifact_ids,
        evidence_receipt_ids,
        release_digest: run.release_digest.clone(),
        signer_public_key_hex: run.signer_public_key_hex.clone(),
        signer_signature_hex: run.signer_signature_hex.clone(),
        migration_notes,
        omissions: run.omissions.clone(),
        raw_data_local: true,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    object.validate()?;
    Ok(object)
}

fn validate_run(run: &ValidatedResearchRun) -> Result<(), GovernanceReleaseError> {
    if run.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || run.feature_id != FEATURE_ID
        || run.boundary != PRECLINICAL_BOUNDARY
        || run.run_id.trim().is_empty()
        || run.release_id.trim().is_empty()
        || run.origin.trim().is_empty()
        || run.purpose.trim().is_empty()
        || run.artifact_ids.is_empty()
        || run.evidence_receipt_ids.is_empty()
        || run.source_contract_version.trim().is_empty()
        || !valid_localization_statement(&run.localization_statement)
        || !run.raw_data_local
        || !run.provenance_complete
        || run.artifact_ids.len() > MAX_ID_COUNT
        || run.evidence_receipt_ids.len() > MAX_ID_COUNT
    {
        return Err(GovernanceReleaseError::InvalidField(
            "validated run is incomplete, non-local, or exceeds bounds".into(),
        ));
    }
    unique_ids(&run.artifact_ids, "artifact")?;
    unique_ids(&run.evidence_receipt_ids, "evidence")?;
    run.policy
        .validate()
        .map_err(|error| GovernanceReleaseError::Policy(error.to_string()))?;
    if run.policy.decision != PolicyDecision::Allow {
        return Err(GovernanceReleaseError::Policy(
            "policy decision is not allow".into(),
        ));
    }
    if !run.policy.evaluated_artifacts.contains(&run.release_digest) {
        return Err(GovernanceReleaseError::Policy(
            "policy did not evaluate the release digest".into(),
        ));
    }
    verify_detached_signature(
        &run.release_digest,
        &run.signer_public_key_hex,
        &run.signer_signature_hex,
    )
}

fn valid_localization_statement(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    !lower.is_empty()
        && value == value.trim()
        && !value.chars().any(char::is_control)
        && lower.contains("local")
        && !lower.contains("not local")
        && !lower.contains("non-local")
}

fn verify_detached_signature(
    digest: &ContentHash,
    public_key_hex: &str,
    signature_hex: &str,
) -> Result<(), GovernanceReleaseError> {
    let public_key = decode_hex(public_key_hex, 32)?;
    let signature = decode_hex(signature_hex, 64)?;
    let key = VerifyingKey::from_bytes(
        public_key
            .as_slice()
            .try_into()
            .map_err(|_| GovernanceReleaseError::Signature("public key length".into()))?,
    )
    .map_err(|error| GovernanceReleaseError::Signature(error.to_string()))?;
    let signature = Signature::from_slice(&signature)
        .map_err(|error| GovernanceReleaseError::Signature(error.to_string()))?;
    key.verify(digest.to_string().as_bytes(), &signature)
        .map_err(|error| GovernanceReleaseError::Signature(error.to_string()))
}

fn decode_hex(value: &str, bytes: usize) -> Result<Vec<u8>, GovernanceReleaseError> {
    if value.len() != bytes * 2 || !value.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(GovernanceReleaseError::Signature(format!(
            "expected {bytes} bytes encoded as lowercase hex"
        )));
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| GovernanceReleaseError::Signature("invalid hexadecimal".into()))
        })
        .collect()
}

fn unique_ids(ids: &[String], kind: &'static str) -> Result<(), GovernanceReleaseError> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.trim().is_empty() || !seen.insert(id) {
            return Err(GovernanceReleaseError::DuplicateId {
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
    use ed25519_dalek::{Signer, SigningKey};

    fn run() -> ValidatedResearchRun {
        let signing = SigningKey::from_bytes(&[7; 32]);
        let digest = ContentHash::of_bytes(b"release-content");
        let signature = signing.sign(digest.to_string().as_bytes());
        let policy = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy:release".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["approved".into()],
            evaluated_artifacts: vec![digest.clone()],
            authority_reference: Some("authority:steward".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        ValidatedResearchRun {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            feature_id: FEATURE_ID.into(),
            run_id: "run:1".into(),
            release_id: "release:1".into(),
            origin: "site-a".into(),
            purpose: "federated preclinical reproduction".into(),
            artifact_ids: vec!["artifact:a".into()],
            evidence_receipt_ids: vec!["evidence:a".into()],
            release_digest: digest,
            policy,
            provenance_complete: true,
            raw_data_local: true,
            localization_statement: "raw data remains local to site-a".into(),
            source_contract_version: "signed-research-object/1.0".into(),
            signer_public_key_hex: hex(&signing.verifying_key().to_bytes()),
            signer_signature_hex: hex(&signature.to_bytes()),
            omissions: vec!["protected:raw-bytes".into()],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn release_migrates_and_verifies_signature() {
        let object = compile_signed_research_object(&run()).unwrap();
        assert_eq!(object.contract_version, CONTRACT_VERSION);
        assert!(object.migration_notes[0].contains("signed-research-object/1.0"));
        assert_eq!(object.digest().unwrap(), object.digest().unwrap());
    }

    #[test]
    fn non_local_run_is_rejected() {
        let mut run = run();
        run.raw_data_local = false;
        assert!(compile_signed_research_object(&run).is_err());
    }
}
