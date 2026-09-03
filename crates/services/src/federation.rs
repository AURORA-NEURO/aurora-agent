//! Signed, policy-gated federation of typed research artifacts.
//!
//! Raw experimental bytes stay at the origin. The envelope signs only the content-addressed
//! artifact metadata, policy constraints and localization declaration; a receiving institution
//! must obtain payload bytes through its own policy admission and verify the artifact hash.

use bioprism_foundation::{
    FederationEnvelope, PolicyDecision, PolicyReceipt, TypedResearchArtifact,
};
use bioprism_ids::to_canonical_bytes;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Atlas feature implemented by this module.
pub const FEATURE_ID: &str = "AFA-services-P16-F01";

#[derive(Debug, Error)]
pub enum FederationError {
    #[error("federation only accepts an allow policy, found {0:?}")]
    PolicyBlocked(PolicyDecision),
    #[error("artifact payload failed verification: {0}")]
    Artifact(String),
    #[error("federation contract rejected the envelope: {0}")]
    Contract(String),
    #[error("invalid federation signature encoding")]
    SignatureEncoding,
    #[error("federation signature verification failed")]
    SignatureInvalid,
    #[error("canonical federation serialization failed: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedFederationArtifact {
    pub envelope: FederationEnvelope,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct FederationSigner {
    key_id: String,
    signing_key: SigningKey,
}

impl FederationSigner {
    pub fn new(
        key_id: impl Into<String>,
        signing_key: SigningKey,
    ) -> Result<Self, FederationError> {
        let key_id = key_id.into();
        if key_id.trim().is_empty() {
            return Err(FederationError::Serialization("empty key id".into()));
        }
        Ok(Self {
            key_id,
            signing_key,
        })
    }

    pub fn sign(
        &self,
        origin: impl Into<String>,
        purpose: impl Into<String>,
        artifact: TypedResearchArtifact,
        payload: Value,
        policy: &PolicyReceipt,
    ) -> Result<SignedFederationArtifact, FederationError> {
        policy
            .validate()
            .map_err(|error| FederationError::Contract(error.to_string()))?;
        if policy.decision != PolicyDecision::Allow {
            return Err(FederationError::PolicyBlocked(policy.decision));
        }
        artifact
            .verify_payload(&payload)
            .map_err(|error| FederationError::Artifact(error.to_string()))?;
        let content_hash = artifact.content_hash.clone();
        let mut envelope = FederationEnvelope {
            schema_version: bioprism_foundation::RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            envelope_id: format!("federation:{}", artifact.artifact_id),
            origin: origin.into(),
            purpose: purpose.into(),
            export: artifact,
            policy_constraints: vec![
                "raw_data_local=true".into(),
                "recipient_must_verify_payload=true".into(),
            ],
            integrity_evidence: vec![content_hash],
            localization_statement: "raw experimental data remains local at origin".into(),
            raw_data_local: true,
            signature: None,
            boundary: bioprism_foundation::PRECLINICAL_BOUNDARY.into(),
        };
        let bytes = signing_bytes(&envelope)?;
        let signature = self.signing_key.sign(&bytes);
        envelope.signature = Some(format!(
            "ed25519:{}:{}",
            self.key_id,
            hex(&signature.to_bytes())
        ));
        envelope
            .validate()
            .map_err(|error| FederationError::Contract(error.to_string()))?;
        Ok(SignedFederationArtifact { envelope, payload })
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }
}

pub fn verify_signed_federation(
    signed: &SignedFederationArtifact,
    key: &VerifyingKey,
) -> Result<(), FederationError> {
    signed
        .envelope
        .validate()
        .map_err(|error| FederationError::Contract(error.to_string()))?;
    signed
        .envelope
        .export
        .verify_payload(&signed.payload)
        .map_err(|error| FederationError::Artifact(error.to_string()))?;
    let raw = signed
        .envelope
        .signature
        .as_deref()
        .ok_or(FederationError::SignatureInvalid)?;
    let encoded = raw
        .strip_prefix("ed25519:")
        .and_then(|value| value.rsplit_once(':'))
        .map(|(_, value)| value)
        .ok_or(FederationError::SignatureEncoding)?;
    let bytes = decode_hex(encoded).ok_or(FederationError::SignatureEncoding)?;
    let signature =
        Signature::from_slice(&bytes).map_err(|_| FederationError::SignatureEncoding)?;
    let unsigned = FederationEnvelope {
        signature: None,
        ..signed.envelope.clone()
    };
    key.verify(&signing_bytes(&unsigned)?, &signature)
        .map_err(|_| FederationError::SignatureInvalid)
}

fn signing_bytes(envelope: &FederationEnvelope) -> Result<Vec<u8>, FederationError> {
    let value = serde_json::to_value(envelope)
        .map_err(|error| FederationError::Serialization(error.to_string()))?;
    to_canonical_bytes(&value).map_err(|error| FederationError::Serialization(error.to_string()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{
        PolicyReceipt, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
    };
    use serde_json::json;

    fn fixture() -> (
        FederationSigner,
        TypedResearchArtifact,
        Value,
        PolicyReceipt,
    ) {
        let signer =
            FederationSigner::new("fixture-key", SigningKey::from_bytes(&[7u8; 32])).unwrap();
        let payload = json!({"result": "unknown", "omissions": ["protected"]});
        let artifact = TypedResearchArtifact::from_payload(
            "artifact-federation-1",
            "application/json",
            &payload,
            vec![],
            vec![],
        )
        .unwrap();
        let policy = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy:federation".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["consortium agreement".into()],
            evaluated_artifacts: vec![artifact.content_hash.clone()],
            authority_reference: Some("approval:consortium-1".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        (signer, artifact, payload, policy)
    }

    #[test]
    fn signed_federation_round_trips_and_verifies_payload_and_signature() {
        let (signer, artifact, payload, policy) = fixture();
        let signed = signer
            .sign(
                "institution-a",
                "benchmark aggregate",
                artifact,
                payload,
                &policy,
            )
            .unwrap();
        verify_signed_federation(&signed, &signer.verifying_key()).unwrap();
    }

    #[test]
    fn local_only_policy_cannot_be_exported() {
        let (signer, artifact, payload, mut policy) = fixture();
        policy.decision = PolicyDecision::LocalOnly;
        let error = signer
            .sign("institution-a", "blocked", artifact, payload, &policy)
            .unwrap_err();
        assert!(matches!(
            error,
            FederationError::PolicyBlocked(PolicyDecision::LocalOnly)
        ));
    }
}
