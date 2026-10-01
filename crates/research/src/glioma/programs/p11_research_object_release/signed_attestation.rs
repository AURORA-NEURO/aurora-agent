//! Fail-closed signed attestation planning for preclinical glioma research-object releases.
//!
//! The feature binds a release manifest, build provenance, release-gate evidence, verification
//! results, signer scope, and key status into one portable attestation.  The implementation uses
//! a deterministic signature seam: an institution-owned signer can replace the derived signature
//! with a real cryptographic provider, but every verifier still receives the exact payload digest,
//! authority identity, revocation state, and evidence required to reject post-sign mutation.  This
//! module never creates a private key, publishes bytes, moves raw data, or makes a clinical claim.

use super::release_gate::ReleaseGateStatus;
use crate::glioma::release::{ReleaseStatus, ResearchObjectManifest};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaSignedReleaseAttestation1@1";
pub const MAX_VERIFICATIONS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseSigningAuthority {
    pub authority_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub active: bool,
    pub revoked: bool,
    pub allowed_policy_scope: String,
    pub revocation_epoch: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseVerificationResult {
    pub verifier_id: String,
    pub verification_schema: String,
    pub passed: bool,
    pub evidence_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedReleaseAttestationRequest {
    pub manifest: ResearchObjectManifest,
    pub build_provenance_digest: ContentHash,
    pub release_gate_digest: ContentHash,
    pub release_gate_status: ReleaseGateStatus,
    pub signer_id: String,
    pub policy_scope: String,
    pub authority: ReleaseSigningAuthority,
    pub verification_results: Vec<ReleaseVerificationResult>,
    pub issued_at_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedReleaseAttestation {
    pub feature_id: String,
    pub output_schema: String,
    pub subject_digest: ContentHash,
    pub manifest_digest: ContentHash,
    pub build_provenance_digest: ContentHash,
    pub release_gate_digest: ContentHash,
    pub issuer: String,
    pub signer_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub policy_scope: String,
    pub issued_at_epoch: u64,
    pub revocation_epoch: Option<u64>,
    pub release_status: ReleaseStatus,
    pub release_gate_status: ReleaseGateStatus,
    pub verification_order: Vec<String>,
    pub failed_verification_order: Vec<String>,
    pub signature_digest: ContentHash,
    pub attestation_digest: ContentHash,
    pub status: AttestationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationStatus {
    Signed,
    Blocked,
    Revoked,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SignedAttestationError {
    #[error("signed release attestation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("signed release attestation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("signed release attestation digest failed: {0}")]
    Digest(String),
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.contains('\0')
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

struct SignaturePayload<'a> {
    manifest_digest: &'a ContentHash,
    build_provenance_digest: &'a ContentHash,
    release_gate_digest: &'a ContentHash,
    issuer: &'a str,
    signer_id: &'a str,
    key_id: &'a str,
    algorithm: &'a str,
    policy_scope: &'a str,
    issued_at_epoch: u64,
    release_gate_status: ReleaseGateStatus,
    verification_order: &'a [String],
}

fn signature_payload(payload: SignaturePayload<'_>) -> serde_json::Value {
    serde_json::json!({
        "manifest_digest": payload.manifest_digest,
        "build_provenance_digest": payload.build_provenance_digest,
        "release_gate_digest": payload.release_gate_digest,
        "issuer": payload.issuer,
        "signer_id": payload.signer_id,
        "key_id": payload.key_id,
        "algorithm": payload.algorithm,
        "policy_scope": payload.policy_scope,
        "issued_at_epoch": payload.issued_at_epoch,
        "release_gate_status": payload.release_gate_status,
        "verification_order": payload.verification_order,
    })
}

fn digest_input(attestation: &SignedReleaseAttestation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": attestation.feature_id,
        "output_schema": attestation.output_schema,
        "subject_digest": attestation.subject_digest,
        "manifest_digest": attestation.manifest_digest,
        "build_provenance_digest": attestation.build_provenance_digest,
        "release_gate_digest": attestation.release_gate_digest,
        "issuer": attestation.issuer,
        "signer_id": attestation.signer_id,
        "key_id": attestation.key_id,
        "algorithm": attestation.algorithm,
        "policy_scope": attestation.policy_scope,
        "issued_at_epoch": attestation.issued_at_epoch,
        "revocation_epoch": attestation.revocation_epoch,
        "release_status": attestation.release_status,
        "release_gate_status": attestation.release_gate_status,
        "verification_order": attestation.verification_order,
        "failed_verification_order": attestation.failed_verification_order,
        "signature_digest": attestation.signature_digest,
        "status": attestation.status,
    })
}

impl SignedReleaseAttestation {
    pub fn validate(&self) -> Result<(), SignedAttestationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.issuer)
            || !valid_identifier(&self.signer_id)
            || !valid_identifier(&self.key_id)
            || !valid_text(&self.algorithm)
            || !valid_text(&self.policy_scope)
            || !canonical(&self.verification_order)
            || !canonical(&self.failed_verification_order)
            || self
                .verification_order
                .iter()
                .any(|id| self.failed_verification_order.binary_search(id).is_ok())
        {
            return Err(SignedAttestationError::InvalidOutput(
                "attestation identity, scope, verification partition, or ordering is invalid"
                    .into(),
            ));
        }
        for digest in [
            &self.subject_digest,
            &self.manifest_digest,
            &self.build_provenance_digest,
            &self.release_gate_digest,
            &self.signature_digest,
        ] {
            if digest.as_str().len() != 64 {
                return Err(SignedAttestationError::InvalidOutput(
                    "attestation digests must be SHA-256 values".into(),
                ));
            }
        }
        if self.status == AttestationStatus::Signed
            && self.release_status != ReleaseStatus::ReadyForSigning
        {
            return Err(SignedAttestationError::InvalidOutput(
                "blocked manifests cannot carry a signed status".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SignedAttestationError::Digest(error.to_string()))?;
        if expected != self.attestation_digest {
            return Err(SignedAttestationError::InvalidOutput(
                "attestation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &SignedReleaseAttestationRequest,
) -> Result<(), SignedAttestationError> {
    request
        .manifest
        .validate()
        .map_err(|error| SignedAttestationError::InvalidRequest(error.to_string()))?;
    if request.build_provenance_digest.as_str().len() != 64
        || request.release_gate_digest.as_str().len() != 64
        || !valid_identifier(&request.signer_id)
        || !valid_identifier(&request.authority.authority_id)
        || !valid_identifier(&request.authority.key_id)
        || !valid_text(&request.authority.algorithm)
        || !valid_text(&request.policy_scope)
        || request.authority.allowed_policy_scope != request.policy_scope
        || request.verification_results.is_empty()
        || request.verification_results.len() > MAX_VERIFICATIONS
    {
        return Err(SignedAttestationError::InvalidRequest(
            "manifest, evidence digests, signer scope, authority, and bounded verification evidence are required".into(),
        ));
    }
    if request.authority.authority_id != request.signer_id {
        return Err(SignedAttestationError::InvalidRequest(
            "signer must be the declared signing authority".into(),
        ));
    }
    let mut verifier_ids = BTreeSet::new();
    for verification in &request.verification_results {
        if !valid_identifier(&verification.verifier_id)
            || !valid_text(&verification.verification_schema)
            || verification.evidence_digest.as_str().len() != 64
            || !verifier_ids.insert(verification.verifier_id.clone())
        {
            return Err(SignedAttestationError::InvalidRequest(
                "verification identities, schemas, evidence digests, and uniqueness are required"
                    .into(),
            ));
        }
    }
    Ok(())
}

/// Build a deterministic attestation envelope.  The returned signature is a content-bound seam
/// for an institution-owned signer; an active external cryptographic provider can replace it while
/// retaining the same payload and verifier contract.
pub fn attest_glioma_release(
    request: &SignedReleaseAttestationRequest,
) -> Result<SignedReleaseAttestation, SignedAttestationError> {
    validate_request(request)?;
    let mut verification_order = request
        .verification_results
        .iter()
        .filter(|verification| verification.passed)
        .map(|verification| verification.verifier_id.clone())
        .collect::<Vec<_>>();
    verification_order.sort();
    let mut failed_verification_order = request
        .verification_results
        .iter()
        .filter(|verification| !verification.passed)
        .map(|verification| verification.verifier_id.clone())
        .collect::<Vec<_>>();
    failed_verification_order.sort();
    let manifest_digest = request.manifest.manifest_digest.clone();
    let subject_digest = ContentHash::of_value(&serde_json::json!({
        "research_id": request.manifest.research_id,
        "study_id": request.manifest.study_id,
        "manifest_digest": manifest_digest,
    }))
    .map_err(|error| SignedAttestationError::Digest(error.to_string()))?;
    let signature_digest = ContentHash::of_value(&signature_payload(SignaturePayload {
        manifest_digest: &manifest_digest,
        build_provenance_digest: &request.build_provenance_digest,
        release_gate_digest: &request.release_gate_digest,
        issuer: &request.authority.authority_id,
        signer_id: &request.signer_id,
        key_id: &request.authority.key_id,
        algorithm: &request.authority.algorithm,
        policy_scope: &request.policy_scope,
        issued_at_epoch: request.issued_at_epoch,
        release_gate_status: request.release_gate_status,
        verification_order: &verification_order,
    }))
    .map_err(|error| SignedAttestationError::Digest(error.to_string()))?;
    let status = if request.authority.revoked {
        AttestationStatus::Revoked
    } else if !request.authority.active
        || request.manifest.release_status != ReleaseStatus::ReadyForSigning
        || request.release_gate_status != ReleaseGateStatus::Publishable
        || !failed_verification_order.is_empty()
        || request.authority.revocation_epoch.is_some()
    {
        AttestationStatus::Blocked
    } else {
        AttestationStatus::Signed
    };
    let mut output = SignedReleaseAttestation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        subject_digest,
        manifest_digest,
        build_provenance_digest: request.build_provenance_digest.clone(),
        release_gate_digest: request.release_gate_digest.clone(),
        issuer: request.authority.authority_id.clone(),
        signer_id: request.signer_id.clone(),
        key_id: request.authority.key_id.clone(),
        algorithm: request.authority.algorithm.clone(),
        policy_scope: request.policy_scope.clone(),
        issued_at_epoch: request.issued_at_epoch,
        revocation_epoch: request.authority.revocation_epoch,
        release_status: request.manifest.release_status,
        release_gate_status: request.release_gate_status,
        verification_order,
        failed_verification_order,
        signature_digest,
        attestation_digest: ContentHash::of_bytes(b"unsealed-glioma-release-attestation"),
        status,
    };
    output.attestation_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SignedAttestationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::release::{build_research_object_manifest, ResearchObjectRequest};
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> SignedReleaseAttestationRequest {
        let manifest = build_research_object_manifest(&ResearchObjectRequest {
            research_id: "attestation-research".into(),
            study_id: "attestation-study".into(),
            objective: "release a preclinical glioma result".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p05-mechanism".into(), "p10-analysis".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact".into(),
                content_hash: hash("artifact"),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec!["null-result-preserved".into()],
            limitations: vec!["single-model-system".into()],
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap();
        SignedReleaseAttestationRequest {
            manifest,
            build_provenance_digest: hash("build"),
            release_gate_digest: hash("gate"),
            release_gate_status: ReleaseGateStatus::Publishable,
            signer_id: "institute-release-authority".into(),
            policy_scope: "preclinical-research-release".into(),
            authority: ReleaseSigningAuthority {
                authority_id: "institute-release-authority".into(),
                key_id: "key-2026".into(),
                algorithm: "institution-signature-seam-v1".into(),
                active: true,
                revoked: false,
                allowed_policy_scope: "preclinical-research-release".into(),
                revocation_epoch: None,
            },
            verification_results: vec![ReleaseVerificationResult {
                verifier_id: "independent-replay".into(),
                verification_schema: "replay-verifier-1".into(),
                passed: true,
                evidence_digest: hash("verification"),
            }],
            issued_at_epoch: 20260923,
        }
    }

    #[test]
    fn valid_release_is_attested_with_content_bound_signature() {
        let output = attest_glioma_release(&request()).unwrap();
        assert_eq!(output.status, AttestationStatus::Signed);
        assert_eq!(output.issuer, "institute-release-authority");
        assert_ne!(output.signature_digest, hash("placeholder"));
        output.validate().unwrap();
    }

    #[test]
    fn failed_verification_blocks_without_erasing_failure() {
        let mut request = request();
        request.verification_results[0].passed = false;
        let output = attest_glioma_release(&request).unwrap();
        assert_eq!(output.status, AttestationStatus::Blocked);
        assert_eq!(output.failed_verification_order, vec!["independent-replay"]);
    }

    #[test]
    fn revoked_authority_is_explicitly_revoked() {
        let mut request = request();
        request.authority.revoked = true;
        let output = attest_glioma_release(&request).unwrap();
        assert_eq!(output.status, AttestationStatus::Revoked);
    }

    #[test]
    fn blocked_manifest_cannot_be_signed() {
        let mut request = request();
        request.manifest.limitations.clear();
        request.manifest = build_research_object_manifest(&ResearchObjectRequest {
            research_id: "attestation-research".into(),
            study_id: "attestation-study".into(),
            objective: "blocked release".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p05-mechanism".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact".into(),
                content_hash: hash("artifact"),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: Vec::new(),
            limitations: Vec::new(),
            raw_data_local: true,
            aggregate_only: true,
        })
        .unwrap();
        let output = attest_glioma_release(&request).unwrap();
        assert_eq!(output.status, AttestationStatus::Blocked);
    }

    #[test]
    fn post_sign_mutation_breaks_attestation_digest() {
        let mut output = attest_glioma_release(&request()).unwrap();
        output.policy_scope = "mutated-scope".into();
        assert!(matches!(
            output.validate(),
            Err(SignedAttestationError::InvalidOutput(_))
        ));
    }
}
