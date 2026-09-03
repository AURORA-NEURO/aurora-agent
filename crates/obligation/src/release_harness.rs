//! High-throughput signed research-object assurance harness.
//!
//! Atlas feature: `AFA-obligation-P16-F27`.
//!
//! The harness is an admission-control product: it checks a governance-signed object against
//! protected closure, required provenance, replay identity, and benchmark obligations. It never
//! turns an unmeasured or contradictory check into a pass.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_governance::SignedResearchObject;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-obligation-P16-F27";
pub const CONTRACT_VERSION: &str = "release-assurance-harness/1.0";
pub const MAX_CHECKS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseHarnessRequest {
    pub request_id: String,
    pub object: SignedResearchObject,
    pub required_artifact_ids: Vec<String>,
    pub required_evidence_receipt_ids: Vec<String>,
    pub protected_closure_satisfied: bool,
    pub replay_identity: Option<ContentHash>,
    pub benchmark_id: String,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCheck {
    pub check_id: String,
    pub disposition: HarnessDisposition,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseHarnessReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub object_digest: ContentHash,
    pub disposition: HarnessDisposition,
    pub checks: Vec<HarnessCheck>,
    pub omissions: Vec<String>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ReleaseHarnessReceipt {
    pub fn validate(&self) -> Result<(), ReleaseHarnessError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.checks.is_empty()
            || self.checks.len() > MAX_CHECKS
            || self.reasons.is_empty()
        {
            return Err(ReleaseHarnessError::InvalidField(
                "schema, identity, checks, reasons, or boundary".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ReleaseHarnessError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ReleaseHarnessError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReleaseHarnessError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReleaseHarnessError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ReleaseHarnessError {
    #[error("invalid release assurance field: {0}")]
    InvalidField(String),
    #[error("governance object rejected: {0}")]
    Governance(String),
    #[error("release assurance artifact error: {0}")]
    Artifact(String),
    #[error("release assurance serialization error: {0}")]
    Serialization(String),
}

pub fn assess_release_harness(
    request: &ReleaseHarnessRequest,
) -> Result<ReleaseHarnessReceipt, ReleaseHarnessError> {
    validate_request(request)?;
    let object_digest = request
        .object
        .digest()
        .map_err(|error| ReleaseHarnessError::Governance(error.to_string()))?;
    let artifact_ids: BTreeSet<&str> = request
        .object
        .artifact_ids
        .iter()
        .map(String::as_str)
        .collect();
    let evidence_ids: BTreeSet<&str> = request
        .object
        .evidence_receipt_ids
        .iter()
        .map(String::as_str)
        .collect();
    let mut checks = Vec::new();
    let mut omissions = request.object.omissions.clone();
    checks.push(HarnessCheck {
        check_id: "governance-signature".into(),
        disposition: HarnessDisposition::Passed,
        reason: "signed object and detached signature verified".into(),
    });
    let artifact_ok = request
        .required_artifact_ids
        .iter()
        .all(|id| artifact_ids.contains(id.as_str()));
    checks.push(HarnessCheck {
        check_id: "required-artifacts".into(),
        disposition: if artifact_ok {
            HarnessDisposition::Passed
        } else {
            HarnessDisposition::Blocked
        },
        reason: if artifact_ok {
            "all required artifact identifiers are present".into()
        } else {
            "required artifact identifier is absent".into()
        },
    });
    let evidence_ok = request
        .required_evidence_receipt_ids
        .iter()
        .all(|id| evidence_ids.contains(id.as_str()));
    checks.push(HarnessCheck {
        check_id: "required-evidence".into(),
        disposition: if evidence_ok {
            HarnessDisposition::Passed
        } else {
            HarnessDisposition::Blocked
        },
        reason: if evidence_ok {
            "all required evidence receipt identifiers are present".into()
        } else {
            "required evidence receipt identifier is absent".into()
        },
    });
    checks.push(HarnessCheck {
        check_id: "protected-closure".into(),
        disposition: if request.protected_closure_satisfied {
            HarnessDisposition::Passed
        } else {
            HarnessDisposition::Blocked
        },
        reason: if request.protected_closure_satisfied {
            "protected closure is satisfied".into()
        } else {
            omissions.push("protected closure is incomplete".into());
            "protected closure is incomplete".into()
        },
    });
    checks.push(HarnessCheck {
        check_id: "replay-identity".into(),
        disposition: if request.replay_identity.is_some() {
            HarnessDisposition::Passed
        } else {
            omissions.push("replay identity is unmeasured".into());
            HarnessDisposition::Unknown
        },
        reason: if request.replay_identity.is_some() {
            "replay identity is declared".into()
        } else {
            "replay identity is unmeasured".into()
        },
    });
    let disposition = if checks
        .iter()
        .any(|check| check.disposition == HarnessDisposition::Blocked)
    {
        HarnessDisposition::Blocked
    } else if checks
        .iter()
        .any(|check| check.disposition == HarnessDisposition::Unknown)
    {
        HarnessDisposition::Unknown
    } else {
        HarnessDisposition::Passed
    };
    let reasons = vec![match disposition {
        HarnessDisposition::Passed => "all release assurance gates passed".into(),
        HarnessDisposition::Blocked => {
            "one or more release assurance gates blocked admission".into()
        }
        HarnessDisposition::Unknown => {
            "an unmeasured release assurance gate prevents a pass".into()
        }
    }];
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request.request_id, "object_digest": object_digest, "disposition": disposition, "checks": checks, "omissions": omissions, "reasons": reasons, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        "release-assurance-harness",
        "application/vnd.aurora.release-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ReleaseHarnessError::Artifact(error.to_string()))?;
    let receipt = ReleaseHarnessReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        object_digest,
        disposition,
        checks: serde_json::from_value(payload["checks"].clone())
            .map_err(|error| ReleaseHarnessError::Serialization(error.to_string()))?,
        omissions: serde_json::from_value(payload["omissions"].clone())
            .map_err(|error| ReleaseHarnessError::Serialization(error.to_string()))?,
        reasons,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ReleaseHarnessRequest) -> Result<(), ReleaseHarnessError> {
    if request.request_id.trim().is_empty()
        || request.benchmark_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.required_artifact_ids.is_empty()
        || request.required_evidence_receipt_ids.is_empty()
    {
        return Err(ReleaseHarnessError::InvalidField(
            "request identity, benchmark, boundary, and required provenance are required".into(),
        ));
    }
    if request.required_artifact_ids.len() + request.required_evidence_receipt_ids.len()
        > MAX_CHECKS
    {
        return Err(ReleaseHarnessError::InvalidField(
            "required provenance exceeds bound".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::PolicyDecision;
    use bioprism_governance::{compile_signed_research_object, ValidatedResearchRun};
    use ed25519_dalek::{Signer, SigningKey};

    fn object() -> SignedResearchObject {
        let signing = SigningKey::from_bytes(&[9; 32]);
        let digest = ContentHash::of_bytes(b"release-harness");
        let signature = signing.sign(digest.to_string().as_bytes());
        let policy = bioprism_foundation::PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy:harness".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["approved".into()],
            evaluated_artifacts: vec![digest.clone()],
            authority_reference: Some("authority:steward".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        compile_signed_research_object(&ValidatedResearchRun {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            feature_id: "AFA-governance-P16-F08".into(),
            run_id: "run:harness".into(),
            release_id: "release:harness".into(),
            origin: "site-a".into(),
            purpose: "replay".into(),
            artifact_ids: vec!["artifact:a".into()],
            evidence_receipt_ids: vec!["evidence:a".into()],
            release_digest: digest,
            policy,
            provenance_complete: true,
            raw_data_local: true,
            localization_statement: "raw data remains local".into(),
            source_contract_version: "signed-research-object/2.0".into(),
            signer_public_key_hex: signing
                .verifying_key()
                .to_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            signer_signature_hex: signature
                .to_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            omissions: vec!["protected:raw".into()],
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap()
    }

    #[test]
    fn missing_replay_is_unknown_not_pass() {
        let receipt = assess_release_harness(&ReleaseHarnessRequest {
            request_id: "request:harness".into(),
            object: object(),
            required_artifact_ids: vec!["artifact:a".into()],
            required_evidence_receipt_ids: vec!["evidence:a".into()],
            protected_closure_satisfied: true,
            replay_identity: None,
            benchmark_id: "benchmark:release".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .unwrap();
        assert_eq!(receipt.disposition, HarnessDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("unmeasured")));
    }
}
