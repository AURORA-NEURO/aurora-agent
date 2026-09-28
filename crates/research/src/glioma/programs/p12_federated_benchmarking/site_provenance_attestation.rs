//! Site-local provenance attestations for aggregate-only glioma federation.
//!
//! The attestation binds a permitted aggregate contribution to its local lineage, analysis
//! version, calibration, environment, policy decision, and signer chain.  It is deliberately a
//! deterministic signing seam: an institution may replace the derived payload digest with a
//! cryptographic provider without changing the verifier contract.  No raw source identifiers,
//! human data, credentials, or clinical decisions cross this boundary.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaSiteContributionAttestation1@1";
pub const MAX_SOURCE_DIGESTS: usize = 512;
pub const MAX_CHAIN_ENTRIES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SitePolicyDecision {
    Approved,
    Denied,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteAttestationStatus {
    Signed,
    Blocked,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteSignerChain {
    pub authority_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub active: bool,
    pub revoked: bool,
    pub chain_valid: bool,
    pub chain_order: Vec<String>,
    pub revocation_epoch: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteProvenanceAttestationRequest {
    pub site_id: String,
    pub contribution_id: String,
    pub benchmark_id: String,
    pub aggregate_digest: ContentHash,
    pub source_digest_order: Vec<ContentHash>,
    pub lineage_digest: ContentHash,
    pub analysis_version: String,
    pub calibration_digest: ContentHash,
    pub policy_scope: String,
    pub policy_decision: SitePolicyDecision,
    pub environment_lock_digest: ContentHash,
    pub signer: SiteSignerChain,
    pub issued_at_epoch: u64,
    pub now_epoch: u64,
    pub valid_until_epoch: u64,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteContributionAttestation {
    pub feature_id: String,
    pub output_schema: String,
    pub site_id: String,
    pub contribution_id: String,
    pub benchmark_id: String,
    pub aggregate_digest: ContentHash,
    pub source_digest_order: Vec<ContentHash>,
    pub lineage_digest: ContentHash,
    pub analysis_version: String,
    pub calibration_digest: ContentHash,
    pub environment_lock_digest: ContentHash,
    pub policy_scope: String,
    pub policy_decision: SitePolicyDecision,
    pub signer_id: String,
    pub key_id: String,
    pub algorithm: String,
    pub issued_at_epoch: u64,
    pub valid_until_epoch: u64,
    pub revocation_epoch: Option<u64>,
    pub freshness_valid: bool,
    pub failure_order: Vec<String>,
    pub signature_digest: ContentHash,
    pub attestation_digest: ContentHash,
    pub status: SiteAttestationStatus,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SiteProvenanceAttestationError {
    #[error("site provenance attestation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("site provenance attestation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("site provenance attestation digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &SiteContributionAttestation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "site_id": output.site_id,
        "contribution_id": output.contribution_id,
        "benchmark_id": output.benchmark_id,
        "aggregate_digest": output.aggregate_digest,
        "source_digest_order": output.source_digest_order,
        "lineage_digest": output.lineage_digest,
        "analysis_version": output.analysis_version,
        "calibration_digest": output.calibration_digest,
        "environment_lock_digest": output.environment_lock_digest,
        "policy_scope": output.policy_scope,
        "policy_decision": output.policy_decision,
        "signer_id": output.signer_id,
        "key_id": output.key_id,
        "algorithm": output.algorithm,
        "issued_at_epoch": output.issued_at_epoch,
        "valid_until_epoch": output.valid_until_epoch,
        "revocation_epoch": output.revocation_epoch,
        "freshness_valid": output.freshness_valid,
        "failure_order": output.failure_order,
        "signature_digest": output.signature_digest,
        "status": output.status,
    })
}

fn signature_payload(
    request: &SiteProvenanceAttestationRequest,
    freshness_valid: bool,
) -> serde_json::Value {
    serde_json::json!({
        "site_id": request.site_id,
        "contribution_id": request.contribution_id,
        "benchmark_id": request.benchmark_id,
        "aggregate_digest": request.aggregate_digest,
        "source_digest_order": request.source_digest_order,
        "lineage_digest": request.lineage_digest,
        "analysis_version": request.analysis_version,
        "calibration_digest": request.calibration_digest,
        "environment_lock_digest": request.environment_lock_digest,
        "policy_scope": request.policy_scope,
        "policy_decision": request.policy_decision,
        "authority_id": request.signer.authority_id,
        "key_id": request.signer.key_id,
        "algorithm": request.signer.algorithm,
        "issued_at_epoch": request.issued_at_epoch,
        "valid_until_epoch": request.valid_until_epoch,
        "freshness_valid": freshness_valid,
    })
}

fn validate_digest(
    digest: &ContentHash,
    field: &str,
) -> Result<(), SiteProvenanceAttestationError> {
    if digest.as_str().len() != 64 {
        return Err(SiteProvenanceAttestationError::InvalidRequest(format!(
            "{field} must be a SHA-256 content hash"
        )));
    }
    Ok(())
}

fn validate_request(
    request: &SiteProvenanceAttestationRequest,
) -> Result<(), SiteProvenanceAttestationError> {
    if !identifier(&request.site_id)
        || !identifier(&request.contribution_id)
        || !identifier(&request.benchmark_id)
        || !text(&request.analysis_version)
        || !text(&request.policy_scope)
        || request.source_digest_order.is_empty()
        || request.source_digest_order.len() > MAX_SOURCE_DIGESTS
        || !canonical(&request.source_digest_order)
        || request.issued_at_epoch == 0
        || request.now_epoch == 0
        || request.valid_until_epoch == 0
        || request.issued_at_epoch > request.valid_until_epoch
        || !identifier(&request.signer.authority_id)
        || !identifier(&request.signer.key_id)
        || !text(&request.signer.algorithm)
        || request.signer.chain_order.is_empty()
        || request.signer.chain_order.len() > MAX_CHAIN_ENTRIES
        || request.signer.chain_order[0] != request.signer.authority_id
    {
        return Err(SiteProvenanceAttestationError::InvalidRequest(
            "bounded identities, canonical source lineage, freshness window, signer chain, and typed policy are required".into(),
        ));
    }
    for (digest, field) in [
        (&request.aggregate_digest, "aggregate_digest"),
        (&request.lineage_digest, "lineage_digest"),
        (&request.calibration_digest, "calibration_digest"),
        (&request.environment_lock_digest, "environment_lock_digest"),
    ] {
        validate_digest(digest, field)?;
    }
    for digest in &request.source_digest_order {
        validate_digest(digest, "source_digest_order")?;
    }
    let mut chain_ids = BTreeSet::new();
    if request
        .signer
        .chain_order
        .iter()
        .any(|entry| !identifier(entry) || !chain_ids.insert(entry.clone()))
    {
        return Err(SiteProvenanceAttestationError::InvalidRequest(
            "signer chain entries must be unique bounded identifiers".into(),
        ));
    }
    Ok(())
}

impl SiteContributionAttestation {
    pub fn validate(&self) -> Result<(), SiteProvenanceAttestationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.site_id)
            || !identifier(&self.contribution_id)
            || !identifier(&self.benchmark_id)
            || !text(&self.analysis_version)
            || !text(&self.policy_scope)
            || self.source_digest_order.is_empty()
            || !canonical(&self.source_digest_order)
            || !identifier(&self.signer_id)
            || !identifier(&self.key_id)
            || !text(&self.algorithm)
            || !canonical(&self.failure_order)
            || self.issued_at_epoch == 0
            || self.valid_until_epoch < self.issued_at_epoch
            || self.signature_digest.as_str().len() != 64
            || self.attestation_digest.as_str().len() != 64
        {
            return Err(SiteProvenanceAttestationError::InvalidOutput(
                "attestation identity, lineage, signer, freshness, ordering, or digest is invalid"
                    .into(),
            ));
        }
        for digest in self.source_digest_order.iter().chain([
            &self.aggregate_digest,
            &self.lineage_digest,
            &self.calibration_digest,
            &self.environment_lock_digest,
        ]) {
            if digest.as_str().len() != 64 {
                return Err(SiteProvenanceAttestationError::InvalidOutput(
                    "attestation evidence digests must be SHA-256 values".into(),
                ));
            }
        }
        if self.status == SiteAttestationStatus::Signed
            && (!self.freshness_valid || !self.failure_order.is_empty())
        {
            return Err(SiteProvenanceAttestationError::InvalidOutput(
                "signed attestation cannot contain stale or failed gates".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SiteProvenanceAttestationError::Digest(error.to_string()))?;
        if expected != self.attestation_digest {
            return Err(SiteProvenanceAttestationError::InvalidOutput(
                "attestation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a deterministic site contribution attestation.  The output is a verifier-ready
/// envelope; a governed institution-owned signer may replace the derived signature digest.
pub fn attest_glioma_site_contribution(
    request: &SiteProvenanceAttestationRequest,
) -> Result<SiteContributionAttestation, SiteProvenanceAttestationError> {
    validate_request(request)?;
    let freshness_valid = request.now_epoch <= request.valid_until_epoch;
    let mut failures = Vec::new();
    if request.policy_decision != SitePolicyDecision::Approved {
        failures.push("policy-not-approved".to_string());
    }
    if !request.signer.active {
        failures.push("signer-inactive".to_string());
    }
    if !request.signer.chain_valid {
        failures.push("signer-chain-invalid".to_string());
    }
    if !request.aggregate_only {
        failures.push("contribution-not-aggregate-only".to_string());
    }
    if !request.raw_data_local {
        failures.push("raw-data-not-local".to_string());
    }
    if request.contains_human_data || request.contains_direct_identifiers {
        failures.push("protected-data-present".to_string());
    }
    if !freshness_valid {
        failures.push("attestation-expired".to_string());
    }
    failures.sort();
    let status = if request.signer.revoked {
        SiteAttestationStatus::Revoked
    } else if !freshness_valid {
        SiteAttestationStatus::Expired
    } else if failures.is_empty() {
        SiteAttestationStatus::Signed
    } else {
        SiteAttestationStatus::Blocked
    };
    let signature_digest = ContentHash::of_value(&signature_payload(request, freshness_valid))
        .map_err(|error| SiteProvenanceAttestationError::Digest(error.to_string()))?;
    let mut output = SiteContributionAttestation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        site_id: request.site_id.clone(),
        contribution_id: request.contribution_id.clone(),
        benchmark_id: request.benchmark_id.clone(),
        aggregate_digest: request.aggregate_digest.clone(),
        source_digest_order: request.source_digest_order.clone(),
        lineage_digest: request.lineage_digest.clone(),
        analysis_version: request.analysis_version.clone(),
        calibration_digest: request.calibration_digest.clone(),
        environment_lock_digest: request.environment_lock_digest.clone(),
        policy_scope: request.policy_scope.clone(),
        policy_decision: request.policy_decision,
        signer_id: request.signer.authority_id.clone(),
        key_id: request.signer.key_id.clone(),
        algorithm: request.signer.algorithm.clone(),
        issued_at_epoch: request.issued_at_epoch,
        valid_until_epoch: request.valid_until_epoch,
        revocation_epoch: request.signer.revocation_epoch,
        freshness_valid,
        failure_order: failures,
        signature_digest,
        attestation_digest: ContentHash::of_bytes(b"unsealed-glioma-site-contribution-attestation"),
        status,
    };
    output.attestation_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SiteProvenanceAttestationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> SiteProvenanceAttestationRequest {
        let mut source_digest_order = vec![hash("source-a"), hash("source-b")];
        source_digest_order.sort();
        SiteProvenanceAttestationRequest {
            site_id: "site-a".into(),
            contribution_id: "contribution-a".into(),
            benchmark_id: "benchmark-1".into(),
            aggregate_digest: hash("aggregate"),
            source_digest_order,
            lineage_digest: hash("lineage"),
            analysis_version: "glioma-analysis-2026.1".into(),
            calibration_digest: hash("calibration"),
            policy_scope: "glioma-federation-v1".into(),
            policy_decision: SitePolicyDecision::Approved,
            environment_lock_digest: hash("environment"),
            signer: SiteSignerChain {
                authority_id: "site-a-authority".into(),
                key_id: "site-a-key-1".into(),
                algorithm: "institution-signature-seam-v1".into(),
                active: true,
                revoked: false,
                chain_valid: true,
                chain_order: vec!["site-a-authority".into(), "consortium-root".into()],
                revocation_epoch: None,
            },
            issued_at_epoch: 100,
            now_epoch: 105,
            valid_until_epoch: 110,
            aggregate_only: true,
            raw_data_local: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    #[test]
    fn approved_site_contribution_is_signed_and_fresh() {
        let output = attest_glioma_site_contribution(&request()).unwrap();
        assert_eq!(output.status, SiteAttestationStatus::Signed);
        assert!(output.freshness_valid);
        output.validate().unwrap();
    }

    #[test]
    fn invalid_signer_chain_blocks_without_erasing_reason() {
        let mut request = request();
        request.signer.chain_valid = false;
        let output = attest_glioma_site_contribution(&request).unwrap();
        assert_eq!(output.status, SiteAttestationStatus::Blocked);
        assert!(output
            .failure_order
            .iter()
            .any(|reason| reason == "signer-chain-invalid"));
    }

    #[test]
    fn revoked_signer_is_never_admitted() {
        let mut request = request();
        request.signer.revoked = true;
        request.signer.revocation_epoch = Some(106);
        let output = attest_glioma_site_contribution(&request).unwrap();
        assert_eq!(output.status, SiteAttestationStatus::Revoked);
        assert_eq!(output.revocation_epoch, Some(106));
    }

    #[test]
    fn stale_attestation_is_explicitly_expired() {
        let mut request = request();
        request.now_epoch = 111;
        let output = attest_glioma_site_contribution(&request).unwrap();
        assert_eq!(output.status, SiteAttestationStatus::Expired);
        assert!(output
            .failure_order
            .iter()
            .any(|reason| reason == "attestation-expired"));
    }

    #[test]
    fn protected_or_tampered_output_cannot_validate() {
        let mut request = request();
        request.contains_direct_identifiers = true;
        let output = attest_glioma_site_contribution(&request).unwrap();
        assert_eq!(output.status, SiteAttestationStatus::Blocked);
        let mut mutated = output;
        mutated.aggregate_digest = hash("tampered");
        assert!(matches!(
            mutated.validate(),
            Err(SiteProvenanceAttestationError::InvalidOutput(_))
        ));
    }
}
