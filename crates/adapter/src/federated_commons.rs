//! Multimodal federated-commons contribution gateway.
//!
//! Atlas feature: `AFA-adapter-P31-F22`.
//! Admits purpose-bound aggregate contributions without moving raw institution-local data.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P31-F22";
pub const CONTRACT_VERSION: &str = "adapter-federated-commons/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommonsContribution {
    pub institution_id: String,
    pub artifact_digest: ContentHash,
    pub semantic_profile: String,
    pub allowed_purposes: Vec<String>,
    pub aggregate_only: bool,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCommonsRequest {
    pub request_id: String,
    pub federation_id: String,
    pub objective_id: String,
    pub required_purpose: String,
    pub contributions: Vec<CommonsContribution>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommonsDisposition {
    Shared,
    Partial,
    Unknown,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCommonsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: FederatedCommonsRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub federation_id: String,
    pub objective_id: String,
    pub required_purpose: String,
    pub disposition: CommonsDisposition,
    pub institution_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub semantic_profile_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}
impl FederatedCommonsReceipt {
    pub fn validate(&self) -> Result<(), FederatedCommonsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.required_purpose.trim().is_empty()
            || self.institution_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedCommonsError::Invalid("commons identity, institutions, purpose, checks, effects, locality, or boundary are incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("objective_id", &self.objective_id)?;
        validate_text("required_purpose", &self.required_purpose)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("institution_order", &self.institution_order)?;
        validate_sorted_strings("admitted_order", &self.admitted_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("semantic_profile_order", &self.semantic_profile_order)?;
        validate_sorted_strings("checks", &self.checks)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        let institutions = self.institution_order.iter().collect::<BTreeSet<_>>();
        let admitted = self.admitted_order.iter().collect::<BTreeSet<_>>();
        let denied = self.denied_order.iter().collect::<BTreeSet<_>>();
        let partition = admitted.union(&denied).copied().collect::<BTreeSet<_>>();
        if admitted.intersection(&denied).next().is_some() || partition != institutions {
            return Err(FederatedCommonsError::Invalid(
                "commons institution partition is incomplete or overlapping".into(),
            ));
        }
        if self.artifact_order.len() != self.admitted_order.len()
            || self.artifact_order.windows(2).any(|p| p[0] >= p[1])
            || self
                .artifact_order
                .iter()
                .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(FederatedCommonsError::Invalid(
                "commons artifact ordering or admitted-artifact closure is invalid".into(),
            ));
        }
        let expected_effect = if matches!(
            self.disposition,
            CommonsDisposition::Shared | CommonsDisposition::Partial
        ) {
            "exchange:permitted-purpose-bound-aggregate-digests-only".to_string()
        } else {
            format!("block:federated-commons:{:?}", self.disposition).to_lowercase()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(FederatedCommonsError::Invalid(
                "commons effect receipt does not match disposition".into(),
            ));
        }
        if self.disposition == CommonsDisposition::Shared
            && (!self.denied_order.is_empty()
                || self.semantic_profile_order.len() > 1
                || self.admitted_order.is_empty())
        {
            return Err(FederatedCommonsError::Invalid(
                "shared commons disposition does not have complete compatible admission".into(),
            ));
        }
        if matches!(
            self.disposition,
            CommonsDisposition::Unknown | CommonsDisposition::Blocked
        ) && (!self.admitted_order.is_empty() || !self.artifact_order.is_empty())
        {
            return Err(FederatedCommonsError::Invalid(
                "unknown or blocked commons cannot carry admitted artifacts".into(),
            ));
        }
        if self.artifact.artifact_id != format!("adapter-federated-commons:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.adapter-federated-commons+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedCommonsError::Artifact(
                "commons artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| FederatedCommonsError::Artifact(e.to_string()))?;
        self.artifact
            .verify_payload(&commons_payload(self))
            .map_err(|e| FederatedCommonsError::Artifact(e.to_string()))?;
        if self.input_digest != commons_input_digest(&self.input)? {
            return Err(FederatedCommonsError::Invalid(
                "commons retained input digest mismatch".into(),
            ));
        }
        validate_request(&self.input)?;
        let expected = build_federated_commons_receipt(&self.input)?;
        if self != &expected {
            return Err(FederatedCommonsError::Invalid(
                "commons receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedCommonsError> {
        self.validate()?;
        let v = serde_json::to_value(self)
            .map_err(|e| FederatedCommonsError::Serialization(e.to_string()))?;
        ContentHash::of_value(&v).map_err(|e| FederatedCommonsError::Serialization(e.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), FederatedCommonsError> {
    if value.is_empty() || value.trim() != value {
        return Err(FederatedCommonsError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(FederatedCommonsError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), FederatedCommonsError> {
    if values.len() > MAX_ITEMS {
        return Err(FederatedCommonsError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(FederatedCommonsError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), FederatedCommonsError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedCommonsError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn commons_payload(receipt: &FederatedCommonsReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "federation_id": receipt.federation_id,
        "objective_id": receipt.objective_id,
        "required_purpose": receipt.required_purpose,
        "disposition": receipt.disposition,
        "institution_order": receipt.institution_order,
        "admitted_order": receipt.admitted_order,
        "denied_order": receipt.denied_order,
        "semantic_profile_order": receipt.semantic_profile_order,
        "artifact_order": receipt.artifact_order,
        "checks": receipt.checks,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn commons_input_digest(
    request: &FederatedCommonsRequest,
) -> Result<ContentHash, FederatedCommonsError> {
    let value = serde_json::to_value(&canonical_federated_commons_request(request))
        .map_err(|e| FederatedCommonsError::Serialization(e.to_string()))?;
    ContentHash::of_value(&value).map_err(|e| FederatedCommonsError::Serialization(e.to_string()))
}

fn canonical_federated_commons_request(
    request: &FederatedCommonsRequest,
) -> FederatedCommonsRequest {
    let mut canonical = request.clone();
    for contribution in &mut canonical.contributions {
        contribution.allowed_purposes.sort();
    }
    canonical
        .contributions
        .sort_by(|left, right| left.institution_id.cmp(&right.institution_id));
    canonical
}

#[derive(Debug, Error)]
pub enum FederatedCommonsError {
    #[error("invalid federated commons request: {0}")]
    Invalid(String),
    #[error("federated commons artifact error: {0}")]
    Artifact(String),
    #[error("federated commons serialization error: {0}")]
    Serialization(String),
}
pub fn admit_federated_commons(
    request: &FederatedCommonsRequest,
) -> Result<FederatedCommonsReceipt, FederatedCommonsError> {
    validate_request(request)?;
    let receipt = build_federated_commons_receipt(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_federated_commons_receipt(
    request: &FederatedCommonsRequest,
) -> Result<FederatedCommonsReceipt, FederatedCommonsError> {
    let mut c = request.contributions.clone();
    c.sort_by(|a, b| a.institution_id.cmp(&b.institution_id));
    let institution_order = c
        .iter()
        .map(|x| x.institution_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut profiles = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for x in &c {
        let purpose = x
            .allowed_purposes
            .iter()
            .any(|p| p == &request.required_purpose);
        if !x.aggregate_only || !x.policy_allow || !x.raw_data_local || !purpose {
            denied.insert(x.institution_id.clone());
            if !x.aggregate_only {
                omissions.insert(format!(
                    "institution:{}:raw-or-nonaggregate-exchange-denied",
                    x.institution_id
                ));
            }
            if !x.policy_allow {
                omissions.insert(format!("institution:{}:policy-denied", x.institution_id));
            }
            if !x.raw_data_local {
                omissions.insert(format!(
                    "institution:{}:locality-unproven",
                    x.institution_id
                ));
            }
            if !purpose {
                negative.insert(format!(
                    "institution:{}:purpose-not-authorized",
                    x.institution_id
                ));
            }
        } else {
            admitted.insert(x.institution_id.clone());
            profiles.insert(x.semantic_profile.clone());
            artifacts.insert(x.artifact_digest.clone());
        }
    }
    if profiles.len() > 1 {
        uncertainty.insert("semantic profiles disagree across admitted contributions".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !request.policy_allow || !request.protected_closure {
        for institution in &institution_order {
            denied.insert(institution.clone());
        }
        admitted.clear();
        profiles.clear();
        artifacts.clear();
    }
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let denied_order = denied.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow {
        CommonsDisposition::Blocked
    } else if !request.protected_closure || admitted_order.is_empty() {
        CommonsDisposition::Unknown
    } else if denied_order.is_empty() && profiles.len() <= 1 {
        CommonsDisposition::Shared
    } else {
        CommonsDisposition::Partial
    };
    let mut checks = vec![
        "institutions are ordered by stable id".into(),
        "purpose, aggregate-only, policy, locality, and semantic-profile gates are explicit".into(),
        "raw institution data remains local and only permitted artifact digests are exchanged"
            .into(),
    ];
    checks.sort();
    let semantic_profile_order = profiles.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if matches!(
        disposition,
        CommonsDisposition::Shared | CommonsDisposition::Partial
    ) {
        vec!["exchange:permitted-purpose-bound-aggregate-digests-only".into()]
    } else {
        vec![format!("block:federated-commons:{disposition:?}").to_lowercase()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"objective_id":request.objective_id,"required_purpose":request.required_purpose,"disposition":disposition,"institution_order":institution_order,"admitted_order":admitted_order,"denied_order":denied_order,"semantic_profile_order":semantic_profile_order,"artifact_order":artifact_order,"checks":checks,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative_evidence,"effect_receipts":effect_receipts,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-federated-commons:{}", request.request_id),
        "application/vnd.aurora.adapter-federated-commons+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| FederatedCommonsError::Artifact(e.to_string()))?;
    let receipt = FederatedCommonsReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_federated_commons_request(request),
        input_digest: commons_input_digest(request)?,
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        objective_id: request.objective_id.clone(),
        required_purpose: request.required_purpose.clone(),
        disposition,
        institution_order,
        admitted_order,
        denied_order,
        semantic_profile_order,
        artifact_order,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}
fn validate_request(request: &FederatedCommonsRequest) -> Result<(), FederatedCommonsError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.required_purpose.trim().is_empty()
        || request.contributions.len() < 2
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedCommonsError::Invalid(
            "commons identity, purpose, contributions, locality, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("federation_id", &request.federation_id)?;
    validate_text("objective_id", &request.objective_id)?;
    validate_text("required_purpose", &request.required_purpose)?;
    validate_text("boundary", &request.boundary)?;
    if request.contributions.len() > MAX_ITEMS {
        return Err(FederatedCommonsError::Invalid(
            "commons contribution count exceeds its bound".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for c in &request.contributions {
        if c.institution_id.trim().is_empty()
            || !ids.insert(c.institution_id.clone())
            || c.semantic_profile.trim().is_empty()
            || c.allowed_purposes.is_empty()
            || c.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(FederatedCommonsError::Invalid(format!(
                "contribution {} is invalid or duplicated",
                c.institution_id
            )));
        }
        validate_text("institution_id", &c.institution_id)?;
        validate_text("semantic_profile", &c.semantic_profile)?;
        validate_text("boundary", &c.boundary)?;
        validate_unique_strings("allowed_purposes", &c.allowed_purposes)?;
        if c.artifact_digest == ContentHash::of_bytes(b"") {
            return Err(FederatedCommonsError::Invalid(
                "contribution artifact digest must be non-empty".into(),
            ));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn c(id: &str, purpose: bool) -> CommonsContribution {
        CommonsContribution {
            institution_id: id.into(),
            artifact_digest: ContentHash::of_bytes(id.as_bytes()),
            semantic_profile: "ome-ngff+anndata:v1".into(),
            allowed_purposes: if purpose {
                vec!["benchmark".into()]
            } else {
                vec!["other".into()]
            },
            aggregate_only: true,
            policy_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn q() -> FederatedCommonsRequest {
        FederatedCommonsRequest {
            request_id: "commons:adapter".into(),
            federation_id: "federation:preclinical".into(),
            objective_id: "objective:benchmark".into(),
            required_purpose: "benchmark".into(),
            contributions: vec![c("site:b", true), c("site:a", false)],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn partial_commons_retains_denial() {
        let r = admit_federated_commons(&q()).unwrap();
        assert_eq!(r.disposition, CommonsDisposition::Partial);
        assert!(!r.denied_order.is_empty());
    }
    #[test]
    fn all_purpose_bound_shares() {
        let mut q = q();
        q.contributions[1].allowed_purposes = vec!["benchmark".into()];
        let r = admit_federated_commons(&q).unwrap();
        assert_eq!(r.disposition, CommonsDisposition::Shared);
    }
    #[test]
    fn protected_gap_unknown() {
        let mut q = q();
        q.protected_closure = false;
        assert_eq!(
            admit_federated_commons(&q).unwrap().disposition,
            CommonsDisposition::Unknown
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = q();
        q.policy_allow = false;
        assert_eq!(
            admit_federated_commons(&q).unwrap().disposition,
            CommonsDisposition::Blocked
        );
    }
    #[test]
    fn nonaggregate_exchange_denied() {
        let mut q = q();
        q.contributions[0].aggregate_only = false;
        let r = admit_federated_commons(&q).unwrap();
        assert!(r.omissions.iter().any(|v| v.contains("nonaggregate")));
    }

    #[test]
    fn global_policy_denial_carries_no_admitted_artifacts() {
        let mut q = q();
        q.contributions[1].allowed_purposes = vec!["benchmark".into()];
        q.policy_allow = false;
        let receipt = admit_federated_commons(&q).unwrap();
        assert_eq!(receipt.disposition, CommonsDisposition::Blocked);
        assert!(receipt.admitted_order.is_empty());
        assert!(receipt.artifact_order.is_empty());
    }

    #[test]
    fn protected_closure_gap_carries_no_admitted_artifacts() {
        let mut q = q();
        q.contributions[1].allowed_purposes = vec!["benchmark".into()];
        q.protected_closure = false;
        let receipt = admit_federated_commons(&q).unwrap();
        assert_eq!(receipt.disposition, CommonsDisposition::Unknown);
        assert!(receipt.admitted_order.is_empty());
        assert!(receipt.artifact_order.is_empty());
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload_binding() {
        let mut receipt = admit_federated_commons(&q()).unwrap();
        receipt.required_purpose = "tampered-purpose".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn receipt_rejects_tampered_retained_request() {
        let mut receipt = admit_federated_commons(&q()).unwrap();
        receipt.input.required_purpose = "tampered-purpose".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
