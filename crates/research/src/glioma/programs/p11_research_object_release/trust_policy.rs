//! Verification of signed, project-scoped release-key trust policies.
//!
//! P11-F22 builds on the mathematical signature check from F21. An operator supplies the trust
//! root key and its expected content hash from local configuration; this module checks the pin,
//! verifies the signed policy, and checks whether the F21 signer has a current grant for the
//! release's research identity and purpose. The operator-supplied pin is still a trust boundary,
//! and a trusted signing key does not itself authorize publication or prevent replay.
//! The external blueprint distribution is not included in this checkout; the repository-local
//! acceptance contract is recorded as P11-F22 in `docs/glioma/PROGRAM_PLAN.md`.

use super::signature_protocol::{
    LocalReleaseSignatureProtocolError, LocalReleaseSignatureVerification, SIGNATURE_PURPOSE,
};
use bioprism_ids::{ContentHash, canonical::to_canonical_bytes_serializable};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = super::archive_migration_adapter::FEATURE_ID;
pub const INPUT_SCHEMA: &str = "GliomaReleaseTrustPolicy1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseSignerTrustDecision1@1";
pub const TRUST_POLICY_SCHEMA_VERSION: &str = "aurora-release-trust-policy/1.0";
pub const MAX_POLICY_ID_BYTES: usize = 128;
pub const MAX_GRANTS: usize = 128;
pub const MAX_POLICY_BYTES: usize = 64 * 1024;
pub const MAX_REASON_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseTrustGrant {
    pub signer_key_id: String,
    pub signer_public_key: [u8; 32],
    pub research_id: String,
    pub purpose: String,
    pub valid_from_unix_seconds: u64,
    pub valid_until_unix_seconds: u64,
    pub revoked_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseTrustPolicy {
    pub schema_version: String,
    pub policy_id: String,
    pub authority_key_id: String,
    /// Grants are sorted by `(signer_key_id, research_id)` and contain no duplicate pair.
    pub grants: Vec<ReleaseTrustGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedReleaseTrustPolicyPayload {
    pub feature_id: String,
    pub input_schema: String,
    pub policy: ReleaseTrustPolicy,
    /// Exact compact JSON bytes that the institutional policy authority must sign.
    pub canonical_policy_bytes: Vec<u8>,
    pub policy_digest: ContentHash,
    pub private_key_received: bool,
    pub policy_authority_authenticated: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseTrustPolicyEvaluationRequest {
    pub signature_verification: LocalReleaseSignatureVerification,
    pub policy: ReleaseTrustPolicy,
    /// Key material and ID loaded by the caller from the local institutional trust config.
    pub pinned_authority_key_id: String,
    pub pinned_authority_public_key: [u8; 32],
    pub pinned_authority_key_digest: ContentHash,
    pub policy_signature: Vec<u8>,
    /// Explicit clock input makes validity decisions deterministic and replayable.
    pub observed_at_unix_seconds: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseSignerTrustDisposition {
    TrustedForReleaseApproval,
    AuthorityPinMismatch,
    PolicySignatureInvalid,
    SignerGrantMissing,
    SignerGrantExpired,
    SignerGrantNotYetValid,
    SignerGrantRevoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseSignerTrustDecision {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    /// Proof inputs are retained so downstream consumers can independently revalidate the result.
    pub signature_verification: LocalReleaseSignatureVerification,
    pub policy: ReleaseTrustPolicy,
    pub pinned_authority_key_id: String,
    pub pinned_authority_public_key: [u8; 32],
    pub policy_signature: Vec<u8>,
    pub signature_verification_digest: ContentHash,
    pub policy_digest: ContentHash,
    pub policy_id: String,
    pub authority_key_id: String,
    pub authority_key_digest: ContentHash,
    pub configured_authority_pin_digest: ContentHash,
    pub policy_signature_digest: ContentHash,
    pub matched_grant_digest: Option<ContentHash>,
    pub observed_at_unix_seconds: u64,
    pub policy_authority_signature_verified: bool,
    pub configured_authority_pin_matched: bool,
    pub signer_grant_matches_release: bool,
    pub signer_grant_current: bool,
    pub signer_key_trusted_for_release_approval: bool,
    pub disposition: ReleaseSignerTrustDisposition,
    pub reason: String,
    pub release_authorized: bool,
    pub challenge_replay_checked: bool,
    pub published: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseTrustPolicyError {
    #[error("release signature verification is invalid: {0}")]
    SignatureVerification(#[from] LocalReleaseSignatureProtocolError),
    #[error("release trust policy is invalid: {0}")]
    InvalidPolicy(String),
    #[error("trust-policy payload or decision is invalid: {0}")]
    InvalidOutput(String),
    #[error("trust-policy digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, ReleaseTrustPolicyError> {
    ContentHash::of_serializable(value)
        .map_err(|error| ReleaseTrustPolicyError::Digest(error.to_string()))
}

fn valid_id(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@".contains(&byte))
}

fn policy_bytes(policy: &ReleaseTrustPolicy) -> Result<Vec<u8>, ReleaseTrustPolicyError> {
    to_canonical_bytes_serializable(policy)
        .map_err(|error| ReleaseTrustPolicyError::InvalidPolicy(error.to_string()))
}

fn validate_policy(policy: &ReleaseTrustPolicy) -> Result<(), ReleaseTrustPolicyError> {
    if policy.schema_version != TRUST_POLICY_SCHEMA_VERSION
        || !valid_id(&policy.policy_id, MAX_POLICY_ID_BYTES)
        || !valid_id(&policy.authority_key_id, MAX_POLICY_ID_BYTES)
        || policy.grants.is_empty()
        || policy.grants.len() > MAX_GRANTS
    {
        return Err(ReleaseTrustPolicyError::InvalidPolicy(
            "schema, policy/authority identity, or grant count is invalid".into(),
        ));
    }
    let mut previous_pair: Option<(&str, &str)> = None;
    let mut pairs = BTreeSet::new();
    for grant in &policy.grants {
        let pair = (grant.signer_key_id.as_str(), grant.research_id.as_str());
        if !valid_id(grant.signer_key_id.as_str(), MAX_POLICY_ID_BYTES)
            || !valid_id(grant.research_id.as_str(), MAX_POLICY_ID_BYTES)
            || grant.purpose != SIGNATURE_PURPOSE
            || grant.valid_from_unix_seconds >= grant.valid_until_unix_seconds
            || !pairs.insert(pair)
            || previous_pair.is_some_and(|previous| previous >= pair)
        {
            return Err(ReleaseTrustPolicyError::InvalidPolicy(
                "grants must be valid, unique, and canonically ordered by key ID and research ID"
                    .into(),
            ));
        }
        previous_pair = Some(pair);
    }
    let bytes = policy_bytes(policy)?;
    if bytes.len() > MAX_POLICY_BYTES {
        return Err(ReleaseTrustPolicyError::InvalidPolicy(
            "policy exceeds the bounded canonical payload size".into(),
        ));
    }
    Ok(())
}

fn prepared_digest(
    prepared: &PreparedReleaseTrustPolicyPayload,
) -> Result<ContentHash, ReleaseTrustPolicyError> {
    digest(&(
        &prepared.feature_id,
        &prepared.input_schema,
        &prepared.policy,
        &prepared.canonical_policy_bytes,
        &prepared.policy_digest,
        prepared.private_key_received,
        prepared.policy_authority_authenticated,
    ))
}

fn decision_digest(
    decision: &ReleaseSignerTrustDecision,
) -> Result<ContentHash, ReleaseTrustPolicyError> {
    digest(&(
        (
            &decision.feature_id,
            &decision.input_schema,
            &decision.output_schema,
            &decision.signature_verification,
            &decision.policy,
            &decision.pinned_authority_key_id,
            &decision.pinned_authority_public_key,
            &decision.policy_signature,
            &decision.signature_verification_digest,
            &decision.policy_digest,
            &decision.policy_id,
            &decision.authority_key_id,
            &decision.authority_key_digest,
            &decision.configured_authority_pin_digest,
            &decision.policy_signature_digest,
            &decision.matched_grant_digest,
        ),
        (
            decision.observed_at_unix_seconds,
            decision.policy_authority_signature_verified,
            decision.configured_authority_pin_matched,
            decision.signer_grant_matches_release,
            decision.signer_grant_current,
            decision.signer_key_trusted_for_release_approval,
            decision.disposition,
            &decision.reason,
            decision.release_authorized,
            decision.challenge_replay_checked,
            decision.published,
        ),
    ))
}

/// Prepare exact policy bytes for an institutional trust-root signer; no private key is accepted.
pub fn prepare_glioma_release_trust_policy_payload(
    policy: &ReleaseTrustPolicy,
) -> Result<PreparedReleaseTrustPolicyPayload, ReleaseTrustPolicyError> {
    validate_policy(policy)?;
    let canonical_policy_bytes = policy_bytes(policy)?;
    let mut prepared = PreparedReleaseTrustPolicyPayload {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        policy: policy.clone(),
        policy_digest: ContentHash::of_bytes(&canonical_policy_bytes),
        canonical_policy_bytes,
        private_key_received: false,
        policy_authority_authenticated: false,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-trust-policy-payload"),
    };
    prepared.digest = prepared_digest(&prepared)?;
    prepared.validate()?;
    Ok(prepared)
}

fn evaluate_request(
    request: &ReleaseTrustPolicyEvaluationRequest,
) -> Result<ReleaseSignerTrustDecision, ReleaseTrustPolicyError> {
    request.signature_verification.validate()?;
    validate_policy(&request.policy)?;
    if !valid_id(&request.pinned_authority_key_id, MAX_POLICY_ID_BYTES) {
        return Err(ReleaseTrustPolicyError::InvalidPolicy(
            "configured authority key ID is invalid".into(),
        ));
    }
    if request.policy_signature.len() > 64 {
        return Err(ReleaseTrustPolicyError::InvalidPolicy(
            "detached policy signature exceeds the Ed25519 size bound".into(),
        ));
    }

    let encoded_policy = policy_bytes(&request.policy)?;
    let policy_digest = ContentHash::of_bytes(&encoded_policy);
    let authority_key_digest = ContentHash::of_bytes(&request.pinned_authority_public_key);
    let configured_authority_pin_matched = authority_key_digest
        == request.pinned_authority_key_digest
        && request.policy.authority_key_id == request.pinned_authority_key_id;
    let policy_authority_signature_verified =
        if configured_authority_pin_matched && request.policy_signature.len() == 64 {
            let key = VerifyingKey::from_bytes(&request.pinned_authority_public_key);
            let signature = Signature::from_slice(&request.policy_signature);
            match (key, signature) {
                (Ok(key), Ok(signature)) => key.verify_strict(&encoded_policy, &signature).is_ok(),
                _ => false,
            }
        } else {
            false
        };

    let verification = &request.signature_verification;
    let matched_grant = request.policy.grants.iter().find(|grant| {
        grant.signer_key_id == verification.payload.signer_key_id
            && grant.research_id == verification.payload.research_id
            && grant.purpose == verification.payload.purpose
            && ContentHash::of_bytes(&grant.signer_public_key)
                == verification.payload.signer_public_key_digest
            && grant.signer_public_key == verification.signer_public_key
    });
    let signer_grant_matches_release = matched_grant.is_some();
    let grant_current = matched_grant.is_some_and(|grant| {
        request.observed_at_unix_seconds >= grant.valid_from_unix_seconds
            && request.observed_at_unix_seconds < grant.valid_until_unix_seconds
            && grant
                .revoked_at_unix_seconds
                .is_none_or(|revoked_at| request.observed_at_unix_seconds < revoked_at)
    });
    let signer_key_trusted_for_release_approval = policy_authority_signature_verified
        && configured_authority_pin_matched
        && signer_grant_matches_release
        && grant_current;

    let (disposition, reason) = if !configured_authority_pin_matched {
        (
            ReleaseSignerTrustDisposition::AuthorityPinMismatch,
            "policy authority identity or public-key digest does not match the configured local pin",
        )
    } else if !policy_authority_signature_verified {
        (
            ReleaseSignerTrustDisposition::PolicySignatureInvalid,
            "the trust policy's detached Ed25519 signature does not verify under the pinned authority key",
        )
    } else if let Some(grant) = matched_grant {
        if request.observed_at_unix_seconds < grant.valid_from_unix_seconds {
            (
                ReleaseSignerTrustDisposition::SignerGrantNotYetValid,
                "the matching signer grant is not yet valid at the supplied evaluation time",
            )
        } else if request.observed_at_unix_seconds >= grant.valid_until_unix_seconds {
            (
                ReleaseSignerTrustDisposition::SignerGrantExpired,
                "the matching signer grant has expired at the supplied evaluation time",
            )
        } else if grant
            .revoked_at_unix_seconds
            .is_some_and(|revoked_at| request.observed_at_unix_seconds >= revoked_at)
        {
            (
                ReleaseSignerTrustDisposition::SignerGrantRevoked,
                "the matching signer grant was revoked at or before the supplied evaluation time",
            )
        } else {
            (
                ReleaseSignerTrustDisposition::TrustedForReleaseApproval,
                "the F21 signer key matches a current project-scoped grant in a policy signed by the configured pinned root",
            )
        }
    } else {
        (
            ReleaseSignerTrustDisposition::SignerGrantMissing,
            "no signed policy grant matches this exact signer key, research identity, and approval purpose",
        )
    };
    let matched_grant_digest = matched_grant.map(digest).transpose()?;
    let mut decision = ReleaseSignerTrustDecision {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        signature_verification: request.signature_verification.clone(),
        policy: request.policy.clone(),
        pinned_authority_key_id: request.pinned_authority_key_id.clone(),
        pinned_authority_public_key: request.pinned_authority_public_key,
        policy_signature: request.policy_signature.clone(),
        signature_verification_digest: verification.digest.clone(),
        policy_digest,
        policy_id: request.policy.policy_id.clone(),
        authority_key_id: request.policy.authority_key_id.clone(),
        authority_key_digest,
        configured_authority_pin_digest: request.pinned_authority_key_digest.clone(),
        policy_signature_digest: ContentHash::of_bytes(&request.policy_signature),
        matched_grant_digest,
        observed_at_unix_seconds: request.observed_at_unix_seconds,
        policy_authority_signature_verified,
        configured_authority_pin_matched,
        signer_grant_matches_release,
        signer_grant_current: grant_current,
        signer_key_trusted_for_release_approval,
        disposition,
        reason: reason.into(),
        release_authorized: false,
        challenge_replay_checked: false,
        published: false,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-signer-trust-decision"),
    };
    decision.digest = decision_digest(&decision)?;
    Ok(decision)
}

/// Evaluate an F21 signer against a root-signed, project-scoped key policy.
pub fn evaluate_glioma_release_trust_policy(
    request: &ReleaseTrustPolicyEvaluationRequest,
) -> Result<ReleaseSignerTrustDecision, ReleaseTrustPolicyError> {
    let decision = evaluate_request(request)?;
    decision.validate()?;
    Ok(decision)
}

impl PreparedReleaseTrustPolicyPayload {
    pub fn validate(&self) -> Result<(), ReleaseTrustPolicyError> {
        validate_policy(&self.policy)?;
        let expected_bytes = policy_bytes(&self.policy)?;
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.canonical_policy_bytes != expected_bytes
            || self.policy_digest != ContentHash::of_bytes(&expected_bytes)
            || self.private_key_received
            || self.policy_authority_authenticated
            || self.digest != prepared_digest(self)?
        {
            return Err(ReleaseTrustPolicyError::InvalidOutput(
                "policy bytes, digest, or explicit signing boundary is invalid".into(),
            ));
        }
        Ok(())
    }
}

impl ReleaseSignerTrustDecision {
    pub fn validate(&self) -> Result<(), ReleaseTrustPolicyError> {
        if self.reason.trim().is_empty() || self.reason.len() > MAX_REASON_BYTES {
            return Err(ReleaseTrustPolicyError::InvalidOutput(
                "trust decision requires a bounded explanation".into(),
            ));
        }
        let request = ReleaseTrustPolicyEvaluationRequest {
            signature_verification: self.signature_verification.clone(),
            policy: self.policy.clone(),
            pinned_authority_key_id: self.pinned_authority_key_id.clone(),
            pinned_authority_public_key: self.pinned_authority_public_key,
            pinned_authority_key_digest: self.configured_authority_pin_digest.clone(),
            policy_signature: self.policy_signature.clone(),
            observed_at_unix_seconds: self.observed_at_unix_seconds,
        };
        let expected = evaluate_request(&request)?;
        if self != &expected
            || self.release_authorized
            || self.challenge_replay_checked
            || self.published
            || self.digest != decision_digest(self)?
        {
            return Err(ReleaseTrustPolicyError::InvalidOutput(
                "trust decision is inconsistent with verified policy, signer grant, or effect boundaries".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::signature_protocol::tests::signed_verification_fixture;
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    const ROOT_KEY_ID: &str = "institution-root-01";

    fn policy_for(
        verification: &LocalReleaseSignatureVerification,
        valid_from_unix_seconds: u64,
        valid_until_unix_seconds: u64,
        revoked_at_unix_seconds: Option<u64>,
        research_id: Option<&str>,
    ) -> ReleaseTrustPolicy {
        ReleaseTrustPolicy {
            schema_version: TRUST_POLICY_SCHEMA_VERSION.into(),
            policy_id: "release-policy-01".into(),
            authority_key_id: ROOT_KEY_ID.into(),
            grants: vec![ReleaseTrustGrant {
                signer_key_id: verification.payload.signer_key_id.clone(),
                signer_public_key: verification.signer_public_key,
                research_id: research_id
                    .unwrap_or(&verification.payload.research_id)
                    .into(),
                purpose: SIGNATURE_PURPOSE.into(),
                valid_from_unix_seconds,
                valid_until_unix_seconds,
                revoked_at_unix_seconds,
            }],
        }
    }

    fn signed_request(
        verification: LocalReleaseSignatureVerification,
        policy: ReleaseTrustPolicy,
        root_key: &SigningKey,
        observed_at_unix_seconds: u64,
    ) -> ReleaseTrustPolicyEvaluationRequest {
        let bytes = policy_bytes(&policy).unwrap();
        ReleaseTrustPolicyEvaluationRequest {
            signature_verification: verification,
            policy,
            pinned_authority_key_id: ROOT_KEY_ID.into(),
            pinned_authority_public_key: root_key.verifying_key().to_bytes(),
            pinned_authority_key_digest: ContentHash::of_bytes(
                &root_key.verifying_key().to_bytes(),
            ),
            policy_signature: root_key.sign(&bytes).to_bytes().to_vec(),
            observed_at_unix_seconds,
        }
    }

    #[test]
    fn exact_signed_project_grant_is_trusted_only_inside_its_interval_and_before_revocation() {
        let verification = signed_verification_fixture();
        let root_key = SigningKey::from_bytes(&[9; 32]);
        let policy = policy_for(&verification, 100, 200, Some(150), None);

        let not_yet_valid = evaluate_glioma_release_trust_policy(&signed_request(
            verification.clone(),
            policy.clone(),
            &root_key,
            99,
        ))
        .unwrap();
        assert_eq!(
            not_yet_valid.disposition,
            ReleaseSignerTrustDisposition::SignerGrantNotYetValid
        );
        assert!(!not_yet_valid.signer_key_trusted_for_release_approval);

        for observed_at in [100, 149] {
            let trusted = evaluate_glioma_release_trust_policy(&signed_request(
                verification.clone(),
                policy.clone(),
                &root_key,
                observed_at,
            ))
            .unwrap();
            assert_eq!(
                trusted.disposition,
                ReleaseSignerTrustDisposition::TrustedForReleaseApproval
            );
            assert!(trusted.policy_authority_signature_verified);
            assert!(trusted.configured_authority_pin_matched);
            assert!(trusted.signer_grant_matches_release);
            assert!(trusted.signer_grant_current);
            assert!(trusted.signer_key_trusted_for_release_approval);
            assert!(!trusted.release_authorized);
            assert!(!trusted.challenge_replay_checked);
            assert!(!trusted.published);
            trusted.validate().unwrap();
        }

        let revoked = evaluate_glioma_release_trust_policy(&signed_request(
            verification.clone(),
            policy.clone(),
            &root_key,
            150,
        ))
        .unwrap();
        assert_eq!(
            revoked.disposition,
            ReleaseSignerTrustDisposition::SignerGrantRevoked
        );
        assert!(!revoked.signer_grant_current);

        let expired = evaluate_glioma_release_trust_policy(&signed_request(
            verification,
            policy,
            &root_key,
            200,
        ))
        .unwrap();
        assert_eq!(
            expired.disposition,
            ReleaseSignerTrustDisposition::SignerGrantExpired
        );
        assert!(!expired.signer_key_trusted_for_release_approval);
    }

    #[test]
    fn authority_pin_signature_and_exact_research_scope_fail_closed() {
        let verification = signed_verification_fixture();
        let root_key = SigningKey::from_bytes(&[11; 32]);
        let policy = policy_for(&verification, 0, 500, None, None);

        let mut wrong_pin = signed_request(verification.clone(), policy.clone(), &root_key, 100);
        wrong_pin.pinned_authority_key_id = "unconfigured-root".into();
        let mismatch = evaluate_glioma_release_trust_policy(&wrong_pin).unwrap();
        assert_eq!(
            mismatch.disposition,
            ReleaseSignerTrustDisposition::AuthorityPinMismatch
        );
        assert!(!mismatch.policy_authority_signature_verified);
        assert!(!mismatch.signer_key_trusted_for_release_approval);

        let mut invalid_signature = signed_request(verification.clone(), policy, &root_key, 100);
        invalid_signature.policy_signature[0] ^= 0x01;
        let bad_signature = evaluate_glioma_release_trust_policy(&invalid_signature).unwrap();
        assert_eq!(
            bad_signature.disposition,
            ReleaseSignerTrustDisposition::PolicySignatureInvalid
        );
        assert!(!bad_signature.signer_key_trusted_for_release_approval);

        let wrong_scope = policy_for(&verification, 0, 500, None, Some("different-research"));
        let missing_grant = evaluate_glioma_release_trust_policy(&signed_request(
            verification,
            wrong_scope,
            &root_key,
            100,
        ))
        .unwrap();
        assert_eq!(
            missing_grant.disposition,
            ReleaseSignerTrustDisposition::SignerGrantMissing
        );
        assert!(!missing_grant.signer_grant_matches_release);
        assert!(!missing_grant.signer_key_trusted_for_release_approval);
    }

    #[test]
    fn policy_grants_must_be_sorted_unique_and_bounded() {
        let verification = signed_verification_fixture();
        let grant = policy_for(&verification, 0, 500, None, None)
            .grants
            .remove(0);
        let mut reversed = ReleaseTrustPolicy {
            schema_version: TRUST_POLICY_SCHEMA_VERSION.into(),
            policy_id: "release-policy-02".into(),
            authority_key_id: ROOT_KEY_ID.into(),
            grants: vec![
                ReleaseTrustGrant {
                    signer_key_id: "signer-z".into(),
                    ..grant.clone()
                },
                ReleaseTrustGrant {
                    signer_key_id: "signer-a".into(),
                    ..grant.clone()
                },
            ],
        };
        assert!(prepare_glioma_release_trust_policy_payload(&reversed).is_err());

        reversed.grants.reverse();
        assert!(prepare_glioma_release_trust_policy_payload(&reversed).is_ok());
        reversed.grants[1] = reversed.grants[0].clone();
        assert!(prepare_glioma_release_trust_policy_payload(&reversed).is_err());

        let mut too_many = policy_for(&verification, 0, 500, None, None);
        too_many.grants = vec![grant; MAX_GRANTS + 1];
        assert!(prepare_glioma_release_trust_policy_payload(&too_many).is_err());
    }
}
