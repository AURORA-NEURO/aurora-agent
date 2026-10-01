//! Offline signature, provenance, and revocation verification for glioma research objects.
//!
//! The verifier is deliberately stricter than a cryptographic "signature valid" check.  It
//! binds the attestation payload to the expected manifest/build/release-gate digests, checks the
//! authority at the verification time, and reports every missing or contradictory qualification.
//! It never fetches trust material, imports raw data, or turns an unverifiable object into a
//! research conclusion.

use super::release_gate::ReleaseGateStatus;
use super::signed_attestation::{AttestationStatus, SignedReleaseAttestation};
use crate::glioma::release::ReleaseStatus;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectVerification1@1";
pub const MAX_TRUST_ROOTS: usize = 128;
pub const MAX_FINDINGS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierTrustRoot {
    pub authority_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub active: bool,
    pub revoked: bool,
    pub revocation_epoch: Option<u64>,
    pub policy_scope_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseSignatureVerificationRequest {
    pub attestation: SignedReleaseAttestation,
    pub expected_manifest_digest: ContentHash,
    pub expected_build_provenance_digest: ContentHash,
    pub expected_release_gate_digest: ContentHash,
    pub expected_policy_scope: String,
    pub verification_epoch: u64,
    pub max_attestation_age_epochs: Option<u64>,
    pub trust_roots: Vec<VerifierTrustRoot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationCheckStatus {
    Passed,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub check_id: String,
    pub status: VerificationCheckStatus,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseVerificationDisposition {
    Verified,
    Blocked,
    Unverifiable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseVerificationReport {
    pub feature_id: String,
    pub output_schema: String,
    pub attestation_digest: ContentHash,
    pub subject_digest: ContentHash,
    pub issuer: String,
    pub key_id: String,
    pub verification_epoch: u64,
    pub checks: Vec<VerificationCheck>,
    pub passed_check_order: Vec<String>,
    pub failed_check_order: Vec<String>,
    pub unresolved_check_order: Vec<String>,
    pub limitations: Vec<String>,
    pub provenance_complete: bool,
    pub cryptographic_valid: bool,
    pub disposition: ReleaseVerificationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReleaseSignatureVerificationError {
    #[error("release signature verification request is invalid: {0}")]
    InvalidRequest(String),
    #[error("release signature verification output is invalid: {0}")]
    InvalidOutput(String),
    #[error("release signature verification digest failed: {0}")]
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

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn signature_payload(attestation: &SignedReleaseAttestation) -> serde_json::Value {
    serde_json::json!({
        "manifest_digest": attestation.manifest_digest,
        "build_provenance_digest": attestation.build_provenance_digest,
        "release_gate_digest": attestation.release_gate_digest,
        "issuer": attestation.issuer,
        "signer_id": attestation.signer_id,
        "key_id": attestation.key_id,
        "algorithm": attestation.algorithm,
        "policy_scope": attestation.policy_scope,
        "issued_at_epoch": attestation.issued_at_epoch,
        "release_gate_status": attestation.release_gate_status,
        "verification_order": attestation.verification_order,
    })
}

fn digest_input(report: &ReleaseVerificationReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "attestation_digest": report.attestation_digest,
        "subject_digest": report.subject_digest,
        "issuer": report.issuer,
        "key_id": report.key_id,
        "verification_epoch": report.verification_epoch,
        "checks": report.checks,
        "passed_check_order": report.passed_check_order,
        "failed_check_order": report.failed_check_order,
        "unresolved_check_order": report.unresolved_check_order,
        "limitations": report.limitations,
        "provenance_complete": report.provenance_complete,
        "cryptographic_valid": report.cryptographic_valid,
        "disposition": report.disposition,
    })
}

fn trust_root_valid(root: &VerifierTrustRoot) -> bool {
    valid_identifier(&root.authority_id)
        && valid_identifier(&root.key_id)
        && valid_text(&root.algorithm)
        && canonical(&root.policy_scope_order)
        && root
            .policy_scope_order
            .iter()
            .all(|scope| valid_text(scope))
}

fn validate_request(
    request: &ReleaseSignatureVerificationRequest,
) -> Result<(), ReleaseSignatureVerificationError> {
    request
        .attestation
        .validate()
        .map_err(|error| ReleaseSignatureVerificationError::InvalidRequest(error.to_string()))?;
    if !valid_digest(&request.expected_manifest_digest)
        || !valid_digest(&request.expected_build_provenance_digest)
        || !valid_digest(&request.expected_release_gate_digest)
        || !valid_text(&request.expected_policy_scope)
        || request.trust_roots.len() > MAX_TRUST_ROOTS
        || request
            .trust_roots
            .iter()
            .any(|root| !trust_root_valid(root))
    {
        return Err(ReleaseSignatureVerificationError::InvalidRequest(
            "expected digests, policy scope, and bounded valid trust roots are required".into(),
        ));
    }
    let mut identities = BTreeSet::new();
    for root in &request.trust_roots {
        if !identities.insert((root.authority_id.clone(), root.key_id.clone())) {
            return Err(ReleaseSignatureVerificationError::InvalidRequest(
                "trust root authority/key identities must be unique".into(),
            ));
        }
    }
    if request
        .max_attestation_age_epochs
        .is_some_and(|age| age > request.verification_epoch)
    {
        return Err(ReleaseSignatureVerificationError::InvalidRequest(
            "attestation age bound must fit the verification epoch".into(),
        ));
    }
    Ok(())
}

impl ReleaseVerificationReport {
    pub fn validate(&self) -> Result<(), ReleaseSignatureVerificationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_digest(&self.attestation_digest)
            || !valid_digest(&self.subject_digest)
            || !valid_identifier(&self.issuer)
            || !valid_identifier(&self.key_id)
            || !canonical(&self.passed_check_order)
            || !canonical(&self.failed_check_order)
            || !canonical(&self.unresolved_check_order)
            || !canonical(&self.limitations)
            || self.checks.len() > MAX_FINDINGS
            || self
                .checks
                .windows(2)
                .any(|pair| pair[0].check_id >= pair[1].check_id)
            || self
                .checks
                .iter()
                .any(|check| !valid_identifier(&check.check_id) || !valid_text(&check.rationale))
        {
            return Err(ReleaseSignatureVerificationError::InvalidOutput(
                "verification identity, bounds, ordering, or checks are invalid".into(),
            ));
        }
        let check_ids = self
            .checks
            .iter()
            .map(|check| check.check_id.as_str())
            .collect::<BTreeSet<_>>();
        if self
            .passed_check_order
            .iter()
            .chain(self.failed_check_order.iter())
            .chain(self.unresolved_check_order.iter())
            .any(|check| !check_ids.contains(check.as_str()))
        {
            return Err(ReleaseSignatureVerificationError::InvalidOutput(
                "verification partitions reference unknown checks".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReleaseSignatureVerificationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReleaseSignatureVerificationError::InvalidOutput(
                "verification report digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Verify a signed release object using only caller-provided trust roots and expected digests.
pub fn verify_glioma_release_signature(
    request: &ReleaseSignatureVerificationRequest,
) -> Result<ReleaseVerificationReport, ReleaseSignatureVerificationError> {
    validate_request(request)?;
    let attestation = &request.attestation;
    let mut checks = Vec::new();
    let mut limitations = BTreeSet::new();
    let mut add = |check_id: &str, status: VerificationCheckStatus, rationale: &str| {
        checks.push(VerificationCheck {
            check_id: check_id.into(),
            status,
            rationale: rationale.into(),
        });
    };
    let digest_match = attestation.manifest_digest == request.expected_manifest_digest;
    add(
        "manifest-digest",
        if digest_match {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if digest_match {
            "manifest digest matches the expected object"
        } else {
            "manifest digest differs from the expected object"
        },
    );
    let build_match =
        attestation.build_provenance_digest == request.expected_build_provenance_digest;
    add(
        "build-provenance-digest",
        if build_match {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if build_match {
            "build provenance digest matches the expected build"
        } else {
            "build provenance digest differs from the expected build"
        },
    );
    let gate_match = attestation.release_gate_digest == request.expected_release_gate_digest;
    add(
        "release-gate-digest",
        if gate_match {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if gate_match {
            "release-gate evidence digest matches the expected gate"
        } else {
            "release-gate evidence digest differs from the expected gate"
        },
    );
    let scope_match = attestation.policy_scope == request.expected_policy_scope;
    add(
        "policy-scope",
        if scope_match {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if scope_match {
            "attestation policy scope matches the requested scope"
        } else {
            "attestation policy scope differs from the requested scope"
        },
    );
    let Some(root) = request.trust_roots.iter().find(|root| {
        root.authority_id == attestation.issuer
            && root.key_id == attestation.key_id
            && root.algorithm == attestation.algorithm
    }) else {
        add(
            "trust-root",
            VerificationCheckStatus::Unresolved,
            "no caller-provided trust root matches the issuer, key, and algorithm",
        );
        limitations
            .insert("trust root was not available in the caller-provided offline set".into());
        let mut report = build_report(
            request,
            checks,
            limitations,
            false,
            false,
            ReleaseVerificationDisposition::Unverifiable,
        )?;
        report.digest = ContentHash::of_value(&digest_input(&report))
            .map_err(|error| ReleaseSignatureVerificationError::Digest(error.to_string()))?;
        report.validate()?;
        return Ok(report);
    };
    let root_scope = root
        .policy_scope_order
        .binary_search(&attestation.policy_scope)
        .is_ok();
    add(
        "trust-root",
        if root_scope {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if root_scope {
            "matching trust root authorizes the attestation policy scope"
        } else {
            "matching trust root does not authorize the attestation policy scope"
        },
    );
    let key_valid = root.active
        && !root.revoked
        && root
            .revocation_epoch
            .is_none_or(|epoch| request.verification_epoch < epoch);
    add(
        "key-validity",
        if key_valid {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if key_valid {
            "trust root is active and not revoked at verification time"
        } else {
            "trust root is inactive or revoked at verification time"
        },
    );
    let payload_digest = ContentHash::of_value(&signature_payload(attestation))
        .map_err(|error| ReleaseSignatureVerificationError::Digest(error.to_string()))?;
    let cryptographic_valid = payload_digest == attestation.signature_digest;
    add(
        "signature-payload",
        if cryptographic_valid {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if cryptographic_valid {
            "signature seam digest matches the canonical attestation payload"
        } else {
            "signature seam digest does not match the canonical attestation payload"
        },
    );
    let age_valid = attestation.issued_at_epoch <= request.verification_epoch
        && request.max_attestation_age_epochs.is_none_or(|max_age| {
            request
                .verification_epoch
                .saturating_sub(attestation.issued_at_epoch)
                <= max_age
        });
    add(
        "attestation-freshness",
        if age_valid {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if age_valid {
            "attestation age is within the caller-provided bound"
        } else {
            "attestation is older than the caller-provided bound"
        },
    );
    let release_ready = attestation.status == AttestationStatus::Signed
        && attestation.release_status == ReleaseStatus::ReadyForSigning
        && attestation.release_gate_status == ReleaseGateStatus::Publishable;
    add(
        "release-readiness",
        if release_ready {
            VerificationCheckStatus::Passed
        } else {
            VerificationCheckStatus::Failed
        },
        if release_ready {
            "attestation, manifest, and release gate all declare a signing-ready state"
        } else {
            "attestation or release gate is not signing-ready"
        },
    );
    let provenance_complete = build_match && gate_match && release_ready;
    let mut passed = Vec::new();
    let mut failed = Vec::new();
    let mut unresolved = Vec::new();
    checks.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    for check in &checks {
        match check.status {
            VerificationCheckStatus::Passed => passed.push(check.check_id.clone()),
            VerificationCheckStatus::Failed => failed.push(check.check_id.clone()),
            VerificationCheckStatus::Unresolved => unresolved.push(check.check_id.clone()),
        }
    }
    let disposition = if !failed.is_empty() {
        ReleaseVerificationDisposition::Blocked
    } else if !unresolved.is_empty() {
        ReleaseVerificationDisposition::Unverifiable
    } else {
        ReleaseVerificationDisposition::Verified
    };
    let mut report = ReleaseVerificationReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        attestation_digest: attestation.attestation_digest.clone(),
        subject_digest: attestation.subject_digest.clone(),
        issuer: attestation.issuer.clone(),
        key_id: attestation.key_id.clone(),
        verification_epoch: request.verification_epoch,
        checks,
        passed_check_order: passed,
        failed_check_order: failed,
        unresolved_check_order: unresolved,
        limitations: limitations.into_iter().collect(),
        provenance_complete,
        cryptographic_valid,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-verification"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| ReleaseSignatureVerificationError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

fn build_report(
    request: &ReleaseSignatureVerificationRequest,
    mut checks: Vec<VerificationCheck>,
    limitations: BTreeSet<String>,
    provenance_complete: bool,
    cryptographic_valid: bool,
    disposition: ReleaseVerificationDisposition,
) -> Result<ReleaseVerificationReport, ReleaseSignatureVerificationError> {
    checks.sort_by(|left, right| left.check_id.cmp(&right.check_id));
    let passed_check_order = checks
        .iter()
        .filter(|check| check.status == VerificationCheckStatus::Passed)
        .map(|check| check.check_id.clone())
        .collect();
    let failed_check_order = checks
        .iter()
        .filter(|check| check.status == VerificationCheckStatus::Failed)
        .map(|check| check.check_id.clone())
        .collect();
    let unresolved_check_order = checks
        .iter()
        .filter(|check| check.status == VerificationCheckStatus::Unresolved)
        .map(|check| check.check_id.clone())
        .collect();
    let mut report = ReleaseVerificationReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        attestation_digest: request.attestation.attestation_digest.clone(),
        subject_digest: request.attestation.subject_digest.clone(),
        issuer: request.attestation.issuer.clone(),
        key_id: request.attestation.key_id.clone(),
        verification_epoch: request.verification_epoch,
        checks,
        passed_check_order,
        failed_check_order,
        unresolved_check_order,
        limitations: limitations.into_iter().collect(),
        provenance_complete,
        cryptographic_valid,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-verification"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| ReleaseSignatureVerificationError::Digest(error.to_string()))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::super::release_gate::ReleaseGateStatus;
    use super::super::signed_attestation::{
        attest_glioma_release, ReleaseSigningAuthority, ReleaseVerificationResult,
        SignedReleaseAttestationRequest,
    };
    use super::*;
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn attestation() -> SignedReleaseAttestation {
        let manifest =
            crate::glioma::release::build_research_object_manifest(&ResearchObjectRequest {
                research_id: "verify-research".into(),
                study_id: "verify-study".into(),
                objective: "verify glioma release".into(),
                plan_digest: hash("plan"),
                execution_digest: hash("execution"),
                replay_identity: hash("replay"),
                program_order: vec!["p05".into()],
                artifacts: vec![LocalArtifactRef {
                    artifact_id: "result".into(),
                    content_hash: hash("result"),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                }],
                negative_evidence: vec!["null".into()],
                limitations: vec!["preclinical".into()],
                raw_data_local: true,
                aggregate_only: true,
            })
            .unwrap();
        let gate_digest = hash("gate");
        attest_glioma_release(&SignedReleaseAttestationRequest {
            manifest,
            build_provenance_digest: hash("build"),
            release_gate_digest: gate_digest,
            release_gate_status: ReleaseGateStatus::Publishable,
            signer_id: "authority".into(),
            policy_scope: "consortium".into(),
            authority: ReleaseSigningAuthority {
                authority_id: "authority".into(),
                key_id: "key-1".into(),
                algorithm: "content-bound-sha256".into(),
                active: true,
                revoked: false,
                allowed_policy_scope: "consortium".into(),
                revocation_epoch: None,
            },
            verification_results: vec![ReleaseVerificationResult {
                verifier_id: "replay".into(),
                verification_schema: "ReplayReport1@1".into(),
                passed: true,
                evidence_digest: hash("verification"),
            }],
            issued_at_epoch: 100,
        })
        .unwrap()
    }

    fn request() -> ReleaseSignatureVerificationRequest {
        let attestation = attestation();
        ReleaseSignatureVerificationRequest {
            expected_manifest_digest: attestation.manifest_digest.clone(),
            expected_build_provenance_digest: attestation.build_provenance_digest.clone(),
            expected_release_gate_digest: attestation.release_gate_digest.clone(),
            expected_policy_scope: "consortium".into(),
            verification_epoch: 101,
            max_attestation_age_epochs: Some(10),
            attestation,
            trust_roots: vec![VerifierTrustRoot {
                authority_id: "authority".into(),
                key_id: "key-1".into(),
                algorithm: "content-bound-sha256".into(),
                active: true,
                revoked: false,
                revocation_epoch: None,
                policy_scope_order: vec!["consortium".into()],
            }],
        }
    }

    #[test]
    fn verifier_accepts_matching_signed_attestation() {
        let report = verify_glioma_release_signature(&request()).unwrap();
        assert_eq!(report.disposition, ReleaseVerificationDisposition::Verified);
        assert!(report.cryptographic_valid);
        report.validate().unwrap();
    }

    #[test]
    fn verifier_distinguishes_tamper_from_missing_trust_root() {
        let mut tampered = request();
        tampered.expected_manifest_digest = hash("different");
        assert_eq!(
            verify_glioma_release_signature(&tampered)
                .unwrap()
                .disposition,
            ReleaseVerificationDisposition::Blocked
        );
        let mut unresolved = request();
        unresolved.trust_roots.clear();
        assert_eq!(
            verify_glioma_release_signature(&unresolved)
                .unwrap()
                .disposition,
            ReleaseVerificationDisposition::Unverifiable
        );
    }

    #[test]
    fn verifier_blocks_revoked_key_and_stale_attestation() {
        let mut revoked = request();
        revoked.trust_roots[0].revoked = true;
        assert_eq!(
            verify_glioma_release_signature(&revoked)
                .unwrap()
                .disposition,
            ReleaseVerificationDisposition::Blocked
        );
        let mut stale = request();
        stale.max_attestation_age_epochs = Some(0);
        stale.verification_epoch = 101;
        assert!(verify_glioma_release_signature(&stale)
            .unwrap()
            .failed_check_order
            .contains(&"attestation-freshness".into()));
    }
}
