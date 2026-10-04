//! Federated aggregate research-object release for preclinical glioma consortia.
//!
//! This compiler joins already-approved site-local aggregate contributions. It carries each
//! site's localization and limitations, enforces quorum/freshness/schema/policy gates, and emits
//! a deterministic aggregate digest. Raw experimental data never enters this object.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedResearchObject1@1";
pub const MAX_SITES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedSiteAggregate {
    pub site_id: String,
    pub contribution_id: String,
    pub aggregate_digest: ContentHash,
    pub schema_version: String,
    pub policy_digest: ContentHash,
    pub issued_epoch: u64,
    pub expires_epoch: u64,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub contains_human_data: bool,
    pub permitted: bool,
    pub localization_statement: String,
    pub uncertainty_milli: u16,
    pub heterogeneity_milli: u16,
    pub source_count: u32,
    pub limitation_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleasePolicy {
    pub required_schema_version: String,
    pub required_policy_digest: ContentHash,
    pub now_epoch: u64,
    pub required_quorum: usize,
    pub max_uncertainty_milli: u16,
    pub max_heterogeneity_milli: u16,
    pub require_localization_statement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseRequest {
    pub research_id: String,
    pub study_id: String,
    pub object_version: String,
    pub contributions: Vec<FederatedSiteAggregate>,
    pub policy: FederatedReleasePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedReleaseDisposition {
    ReadyForReview,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedResearchObject {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub object_version: String,
    pub site_order: Vec<String>,
    pub contribution_digest_order: Vec<ContentHash>,
    pub omitted_site_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub source_count: u32,
    pub aggregate_uncertainty_milli: u16,
    pub heterogeneity_milli: u16,
    pub localization_statement: String,
    pub disposition: FederatedReleaseDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedReleaseError {
    #[error("federated release request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated release object is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated release digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedResearchObject) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "research_id": output.research_id,
        "study_id": output.study_id,
        "object_version": output.object_version,
        "site_order": output.site_order,
        "contribution_digest_order": output.contribution_digest_order,
        "omitted_site_order": output.omitted_site_order,
        "omission_order": output.omission_order,
        "source_count": output.source_count,
        "aggregate_uncertainty_milli": output.aggregate_uncertainty_milli,
        "heterogeneity_milli": output.heterogeneity_milli,
        "localization_statement": output.localization_statement,
        "disposition": output.disposition,
    })
}

impl FederatedResearchObject {
    pub fn validate(&self) -> Result<(), FederatedReleaseError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.research_id)
            || !identifier(&self.study_id)
            || !identifier(&self.object_version)
            || !canonical(&self.site_order)
            || !canonical(&self.omitted_site_order)
            || !canonical(&self.omission_order)
            || self
                .site_order
                .iter()
                .any(|site| self.omitted_site_order.binary_search(site).is_ok())
            || self.aggregate_uncertainty_milli > 1_000
            || self.heterogeneity_milli > 1_000
            || self.localization_statement.trim().is_empty()
            || self.digest.as_str().len() != 64
        {
            return Err(FederatedReleaseError::InvalidOutput(
                "federated identity, partitions, bounds, localization, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedReleaseError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedReleaseError::InvalidOutput(
                "federated object digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &FederatedReleaseRequest) -> Result<(), FederatedReleaseError> {
    if !identifier(&request.research_id)
        || !identifier(&request.study_id)
        || !identifier(&request.object_version)
        || request.contributions.is_empty()
        || request.contributions.len() > MAX_SITES
        || request.policy.required_schema_version.trim().is_empty()
        || request.policy.required_policy_digest.as_str().len() != 64
        || request.policy.now_epoch == 0
        || request.policy.required_quorum == 0
        || request.policy.required_quorum > request.contributions.len()
        || request.policy.max_uncertainty_milli > 1_000
        || request.policy.max_heterogeneity_milli > 1_000
    {
        return Err(FederatedReleaseError::InvalidRequest(
            "bounded research identity, contributions, schema/policy, epoch, quorum, and thresholds are required".into(),
        ));
    }
    let mut sites = BTreeSet::new();
    let mut contributions = BTreeSet::new();
    for contribution in &request.contributions {
        if !identifier(&contribution.site_id)
            || !sites.insert(contribution.site_id.clone())
            || !identifier(&contribution.contribution_id)
            || !contributions.insert(contribution.contribution_id.clone())
            || contribution.aggregate_digest.as_str().len() != 64
            || contribution.policy_digest.as_str().len() != 64
            || contribution.schema_version.trim().is_empty()
            || contribution.issued_epoch == 0
            || contribution.expires_epoch <= contribution.issued_epoch
            || contribution.uncertainty_milli > 1_000
            || contribution.heterogeneity_milli > 1_000
            || contribution.source_count == 0
            || contribution.localization_statement.trim().is_empty()
            || !canonical(&contribution.limitation_order)
            || contribution
                .limitation_order
                .iter()
                .any(|item| item.trim().is_empty())
        {
            return Err(FederatedReleaseError::InvalidRequest(
                "site contributions require unique identity, bounded evidence, localization, and limitations".into(),
            ));
        }
    }
    Ok(())
}

/// Compile policy-approved aggregate contributions into a federated research object.
pub fn compile_glioma_federated_release(
    request: &FederatedReleaseRequest,
) -> Result<FederatedResearchObject, FederatedReleaseError> {
    validate_request(request)?;
    let mut accepted = Vec::new();
    let mut omitted = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    for contribution in &request.contributions {
        let mut reasons = Vec::new();
        if !contribution.permitted {
            reasons.push("site-policy-denied".to_string());
        }
        if contribution.schema_version != request.policy.required_schema_version {
            reasons.push("schema-mismatch".to_string());
        }
        if contribution.policy_digest != request.policy.required_policy_digest {
            reasons.push("policy-mismatch".to_string());
        }
        if contribution.expires_epoch < request.policy.now_epoch {
            reasons.push("stale-contribution".to_string());
        }
        if !contribution.aggregate_only || !contribution.raw_data_local {
            reasons.push("raw-data-locality-gate".to_string());
        }
        if contribution.contains_human_data {
            reasons.push("human-data-excluded".to_string());
        }
        if contribution.uncertainty_milli > request.policy.max_uncertainty_milli {
            reasons.push("uncertainty-threshold".to_string());
        }
        if contribution.heterogeneity_milli > request.policy.max_heterogeneity_milli {
            reasons.push("heterogeneity-threshold".to_string());
        }
        if request.policy.require_localization_statement
            && contribution.localization_statement.trim().is_empty()
        {
            reasons.push("missing-localization-statement".to_string());
        }
        if reasons.is_empty() {
            accepted.push(contribution);
        } else {
            omitted.insert(contribution.site_id.clone());
            for reason in reasons {
                omissions.insert(format!("{}:{reason}", contribution.site_id));
            }
        }
    }
    accepted.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let site_order = accepted
        .iter()
        .map(|item| item.site_id.clone())
        .collect::<Vec<_>>();
    let contribution_digest_order = accepted
        .iter()
        .map(|item| item.aggregate_digest.clone())
        .collect::<Vec<_>>();
    let source_count = accepted.iter().map(|item| item.source_count).sum::<u32>();
    let aggregate_uncertainty_milli = accepted
        .iter()
        .map(|item| item.uncertainty_milli as u32)
        .max()
        .unwrap_or(1_000) as u16;
    let heterogeneity_milli = accepted
        .iter()
        .map(|item| item.heterogeneity_milli as u32)
        .max()
        .unwrap_or(1_000) as u16;
    let localization_statement = if accepted.is_empty() {
        "no-site-contribution-admitted; raw data remains local".into()
    } else {
        format!(
            "aggregate-only; raw data remains at origin sites [{}]",
            site_order.join(",")
        )
    };
    let disposition = if accepted.len() < request.policy.required_quorum {
        FederatedReleaseDisposition::Blocked
    } else if accepted.len() < request.contributions.len() {
        FederatedReleaseDisposition::Partial
    } else {
        FederatedReleaseDisposition::ReadyForReview
    };
    let mut output = FederatedResearchObject {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.research_id.clone(),
        study_id: request.study_id.clone(),
        object_version: request.object_version.clone(),
        site_order,
        contribution_digest_order,
        omitted_site_order: omitted.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        source_count,
        aggregate_uncertainty_milli,
        heterogeneity_milli,
        localization_statement,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-release"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedReleaseError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn contribution(site: &str) -> FederatedSiteAggregate {
        FederatedSiteAggregate {
            site_id: site.into(),
            contribution_id: format!("contribution-{site}"),
            aggregate_digest: hash(site),
            schema_version: "aggregate-1".into(),
            policy_digest: hash("policy"),
            issued_epoch: 90,
            expires_epoch: 110,
            aggregate_only: true,
            raw_data_local: true,
            contains_human_data: false,
            permitted: true,
            localization_statement: format!("raw data local to {site}"),
            uncertainty_milli: 200,
            heterogeneity_milli: 300,
            source_count: 4,
            limitation_order: vec!["preclinical".into()],
        }
    }

    fn request() -> FederatedReleaseRequest {
        FederatedReleaseRequest {
            research_id: "research-a".into(),
            study_id: "study-a".into(),
            object_version: "v1".into(),
            contributions: vec![contribution("site-a"), contribution("site-b")],
            policy: FederatedReleasePolicy {
                required_schema_version: "aggregate-1".into(),
                required_policy_digest: hash("policy"),
                now_epoch: 100,
                required_quorum: 2,
                max_uncertainty_milli: 500,
                max_heterogeneity_milli: 500,
                require_localization_statement: true,
            },
        }
    }

    #[test]
    fn quorum_approved_aggregate_is_ready_for_review() {
        let output = compile_glioma_federated_release(&request()).unwrap();
        assert_eq!(
            output.disposition,
            FederatedReleaseDisposition::ReadyForReview
        );
        assert_eq!(output.source_count, 8);
        output.validate().unwrap();
    }

    #[test]
    fn denied_site_is_omitted_without_moving_raw_data() {
        let mut request = request();
        request.contributions[1].permitted = false;
        let output = compile_glioma_federated_release(&request).unwrap();
        assert_eq!(output.disposition, FederatedReleaseDisposition::Blocked);
        assert_eq!(output.omitted_site_order, vec!["site-b"]);
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason.contains("site-policy-denied")));
    }

    #[test]
    fn failed_quorum_blocks_release() {
        let mut request = request();
        request.policy.required_quorum = 2;
        request.contributions[0].expires_epoch = 99;
        let output = compile_glioma_federated_release(&request).unwrap();
        assert_eq!(output.disposition, FederatedReleaseDisposition::Blocked);
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason.contains("stale-contribution")));
    }

    #[test]
    fn human_data_is_never_admitted() {
        let mut request = request();
        request.contributions[0].contains_human_data = true;
        let output = compile_glioma_federated_release(&request).unwrap();
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason.contains("human-data-excluded")));
    }

    #[test]
    fn mutation_breaks_content_digest() {
        let mut output = compile_glioma_federated_release(&request()).unwrap();
        output.site_order.reverse();
        assert!(matches!(
            output.validate(),
            Err(FederatedReleaseError::InvalidOutput(_))
        ));
    }
}
