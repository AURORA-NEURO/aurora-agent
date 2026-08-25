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
        for values in [
            &self.institution_order,
            &self.admitted_order,
            &self.denied_order,
            &self.semantic_profile_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|p| p[0] >= p[1]) {
                return Err(FederatedCommonsError::Invalid(
                    "commons ordering is not canonical".into(),
                ));
            }
        }
        if self.artifact_order.windows(2).any(|p| p[0] >= p[1]) {
            return Err(FederatedCommonsError::Invalid(
                "commons artifact ordering is not canonical".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| FederatedCommonsError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedCommonsError> {
        self.validate()?;
        let v = serde_json::to_value(self)
            .map_err(|e| FederatedCommonsError::Serialization(e.to_string()))?;
        ContentHash::of_value(&v).map_err(|e| FederatedCommonsError::Serialization(e.to_string()))
    }
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
    }
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let denied_order = denied.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow {
        CommonsDisposition::Blocked
    } else if !request.protected_closure {
        CommonsDisposition::Unknown
    } else if admitted_order.is_empty() {
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
    receipt.validate()?;
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
}
