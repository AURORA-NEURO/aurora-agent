//! Local site participation review for aggregate-only federated glioma research.
//!
//! The workbench is the human-facing consent boundary for an institution. It renders the exact
//! purpose, requested aggregate fields, protocol fit, workload, privacy cost, approval state, and
//! withdrawal path before any benchmark scheduler can consider the site. It does not dispatch a
//! query, move raw data, or infer biological or clinical conclusions.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F17";
pub const OUTPUT_SCHEMA: &str = "GliomaSiteParticipationReview1@1";
pub const MAX_FIELDS: usize = 256;
pub const MAX_MODELS: usize = 64;
pub const MAX_REASON_LENGTH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipationDisposition {
    ReadyForApproval,
    Approved,
    ApprovalRequired,
    Incompatible,
    Withdrawn,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteParticipationRequest {
    pub benchmark_id: String,
    pub site_id: String,
    pub research_purpose: String,
    pub policy_allows_purpose: bool,
    pub requested_field_order: Vec<String>,
    pub requested_model_order: Vec<String>,
    pub requested_assay: String,
    pub required_protocol_version: String,
    pub local_protocol_version: String,
    pub local_field_order: Vec<String>,
    pub local_model_order: Vec<String>,
    pub local_assay_order: Vec<String>,
    pub proposed_workload_units: u64,
    pub workload_capacity_units: u64,
    pub proposed_privacy_cost_milli: u64,
    pub privacy_budget_milli: u64,
    pub approval_recorded: bool,
    pub withdrawal_requested: bool,
    pub withdrawal_reason: Option<String>,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteParticipationReview {
    pub feature_id: String,
    pub output_schema: String,
    pub benchmark_id: String,
    pub site_id: String,
    pub research_purpose: String,
    pub requested_field_order: Vec<String>,
    pub requested_model_order: Vec<String>,
    pub requested_assay: String,
    pub missing_field_order: Vec<String>,
    pub missing_model_order: Vec<String>,
    pub assay_supported: bool,
    pub protocol_compatible: bool,
    pub policy_permitted: bool,
    pub workload_within_capacity: bool,
    pub privacy_within_budget: bool,
    pub approval_recorded: bool,
    pub withdrawal_available: bool,
    pub withdrawal_requested: bool,
    pub withdrawal_reason: Option<String>,
    pub proposed_workload_units: u64,
    pub workload_capacity_units: u64,
    pub proposed_privacy_cost_milli: u64,
    pub privacy_budget_milli: u64,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub disposition: ParticipationDisposition,
    pub participation_permitted: bool,
    pub query_dispatch_permitted: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SiteParticipationError {
    #[error("site participation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("site participation review is invalid: {0}")]
    InvalidOutput(String),
    #[error("site participation digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_REASON_LENGTH
        && !value.chars().any(char::is_control)
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_body(review: &SiteParticipationReview) -> serde_json::Value {
    serde_json::json!({
        "feature_id": review.feature_id,
        "output_schema": review.output_schema,
        "benchmark_id": review.benchmark_id,
        "site_id": review.site_id,
        "research_purpose": review.research_purpose,
        "requested_field_order": review.requested_field_order,
        "requested_model_order": review.requested_model_order,
        "requested_assay": review.requested_assay,
        "missing_field_order": review.missing_field_order,
        "missing_model_order": review.missing_model_order,
        "assay_supported": review.assay_supported,
        "protocol_compatible": review.protocol_compatible,
        "policy_permitted": review.policy_permitted,
        "workload_within_capacity": review.workload_within_capacity,
        "privacy_within_budget": review.privacy_within_budget,
        "approval_recorded": review.approval_recorded,
        "withdrawal_available": review.withdrawal_available,
        "withdrawal_requested": review.withdrawal_requested,
        "withdrawal_reason": review.withdrawal_reason,
        "proposed_workload_units": review.proposed_workload_units,
        "workload_capacity_units": review.workload_capacity_units,
        "proposed_privacy_cost_milli": review.proposed_privacy_cost_milli,
        "privacy_budget_milli": review.privacy_budget_milli,
        "blockers": review.blockers,
        "warnings": review.warnings,
        "disposition": review.disposition,
        "participation_permitted": review.participation_permitted,
        "query_dispatch_permitted": review.query_dispatch_permitted,
    })
}

fn validate_request(request: &SiteParticipationRequest) -> Result<(), SiteParticipationError> {
    if !safe_text(&request.benchmark_id)
        || !safe_text(&request.site_id)
        || !safe_text(&request.research_purpose)
        || !safe_text(&request.requested_assay)
        || !safe_text(&request.required_protocol_version)
        || !safe_text(&request.local_protocol_version)
        || request.requested_field_order.is_empty()
        || request.requested_field_order.len() > MAX_FIELDS
        || !canonical(&request.requested_field_order)
        || request.requested_model_order.is_empty()
        || request.requested_model_order.len() > MAX_MODELS
        || !canonical(&request.requested_model_order)
        || request
            .local_field_order
            .iter()
            .any(|field| !safe_text(field))
        || request
            .local_model_order
            .iter()
            .any(|model| !safe_text(model))
        || request
            .local_assay_order
            .iter()
            .any(|assay| !safe_text(assay))
        || !canonical(&request.local_field_order)
        || !canonical(&request.local_model_order)
        || !canonical(&request.local_assay_order)
        || request.proposed_privacy_cost_milli > 1_000_000
        || request.privacy_budget_milli == 0
        || request.withdrawal_requested
            && request
                .withdrawal_reason
                .as_deref()
                .is_none_or(|reason| !safe_text(reason))
    {
        return Err(SiteParticipationError::InvalidRequest(
            "bounded identities, canonical requested/local capabilities, protocol, privacy, and withdrawal fields are required".into(),
        ));
    }
    Ok(())
}

impl SiteParticipationReview {
    pub fn validate(&self) -> Result<(), SiteParticipationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.benchmark_id)
            || !safe_text(&self.site_id)
            || !safe_text(&self.research_purpose)
            || !safe_text(&self.requested_assay)
            || !canonical(&self.requested_field_order)
            || !canonical(&self.requested_model_order)
            || !canonical(&self.missing_field_order)
            || !canonical(&self.missing_model_order)
            || !canonical(&self.blockers)
            || !canonical(&self.warnings)
            || self.privacy_budget_milli == 0
            || self.withdrawal_requested && self.withdrawal_reason.is_none()
            || self.digest.as_str().len() != 64
        {
            return Err(SiteParticipationError::InvalidOutput(
                "site identity, canonical capability, budget, withdrawal, blocker, or digest invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| SiteParticipationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SiteParticipationError::InvalidOutput(
                "site participation review digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Render the local participation contract and decide whether the site may opt in.
pub fn review_glioma_site_participation(
    request: &SiteParticipationRequest,
) -> Result<SiteParticipationReview, SiteParticipationError> {
    validate_request(request)?;
    let local_fields = request
        .local_field_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let local_models = request
        .local_model_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let local_assays = request
        .local_assay_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_fields = request
        .requested_field_order
        .iter()
        .filter(|field| !local_fields.contains(*field))
        .cloned()
        .collect::<Vec<_>>();
    let missing_models = request
        .requested_model_order
        .iter()
        .filter(|model| !local_models.contains(*model))
        .cloned()
        .collect::<Vec<_>>();
    let assay_supported = local_assays.contains(&request.requested_assay);
    let protocol_compatible = request.required_protocol_version == request.local_protocol_version;
    let policy_permitted = request.policy_allows_purpose;
    let workload_within_capacity =
        request.proposed_workload_units <= request.workload_capacity_units;
    let privacy_within_budget = request.proposed_privacy_cost_milli <= request.privacy_budget_milli;
    let mut blockers = Vec::new();
    if !policy_permitted {
        blockers.push("purpose is not permitted by the local research policy".into());
    }
    if !missing_fields.is_empty() {
        blockers.push(format!(
            "requested fields are unavailable: {}",
            missing_fields.join(",")
        ));
    }
    if !missing_models.is_empty() {
        blockers.push(format!(
            "requested models are unavailable: {}",
            missing_models.join(",")
        ));
    }
    if !assay_supported {
        blockers.push("requested assay is not locally supported".into());
    }
    if !protocol_compatible {
        blockers.push("local protocol version is incompatible with the benchmark".into());
    }
    if !workload_within_capacity {
        blockers.push("requested workload exceeds declared local capacity".into());
    }
    if !privacy_within_budget {
        blockers.push("requested privacy cost exceeds the local privacy budget".into());
    }
    if !request.aggregate_only {
        blockers.push("participation must be aggregate-only".into());
    }
    if !request.raw_data_local {
        blockers.push("raw experimental data must remain local".into());
    }
    if request.contains_human_data {
        blockers.push("human-subject data is outside the glioma engine boundary".into());
    }
    if request.contains_direct_identifiers {
        blockers.push("direct identifiers are outside the federation boundary".into());
    }
    if request.withdrawal_requested {
        blockers.push("local site withdrawal has been requested".into());
    }
    blockers.sort();
    let mut warnings = Vec::new();
    if !request.approval_recorded && blockers.is_empty() {
        warnings.push("local PI/data-steward approval is still required before admission".into());
    }
    warnings.sort();
    let disposition = if request.withdrawal_requested {
        ParticipationDisposition::Withdrawn
    } else if !blockers.is_empty() {
        ParticipationDisposition::Blocked
    } else if request.approval_recorded {
        ParticipationDisposition::Approved
    } else {
        ParticipationDisposition::ApprovalRequired
    };
    let participation_permitted = disposition == ParticipationDisposition::Approved;
    let mut review = SiteParticipationReview {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        benchmark_id: request.benchmark_id.clone(),
        site_id: request.site_id.clone(),
        research_purpose: request.research_purpose.clone(),
        requested_field_order: request.requested_field_order.clone(),
        requested_model_order: request.requested_model_order.clone(),
        requested_assay: request.requested_assay.clone(),
        missing_field_order: missing_fields,
        missing_model_order: missing_models,
        assay_supported,
        protocol_compatible,
        policy_permitted,
        workload_within_capacity,
        privacy_within_budget,
        approval_recorded: request.approval_recorded,
        withdrawal_available: true,
        withdrawal_requested: request.withdrawal_requested,
        withdrawal_reason: request.withdrawal_reason.clone(),
        proposed_workload_units: request.proposed_workload_units,
        workload_capacity_units: request.workload_capacity_units,
        proposed_privacy_cost_milli: request.proposed_privacy_cost_milli,
        privacy_budget_milli: request.privacy_budget_milli,
        blockers,
        warnings,
        disposition,
        participation_permitted,
        query_dispatch_permitted: false,
        digest: ContentHash::of_bytes(b"unsealed-glioma-site-participation"),
    };
    review.digest = ContentHash::of_value(&digest_body(&review))
        .map_err(|error| SiteParticipationError::Digest(error.to_string()))?;
    review.validate()?;
    Ok(review)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> SiteParticipationRequest {
        SiteParticipationRequest {
            benchmark_id: "benchmark-1".into(),
            site_id: "site-a".into(),
            research_purpose: "compare invasion phenotypes across preclinical models".into(),
            policy_allows_purpose: true,
            requested_field_order: vec!["effect".into(), "uncertainty".into()],
            requested_model_order: vec!["organoid".into(), "xenograft".into()],
            requested_assay: "organoid-imaging".into(),
            required_protocol_version: "protocol-v1".into(),
            local_protocol_version: "protocol-v1".into(),
            local_field_order: vec!["effect".into(), "uncertainty".into()],
            local_model_order: vec!["organoid".into(), "xenograft".into()],
            local_assay_order: vec!["organoid-imaging".into()],
            proposed_workload_units: 20,
            workload_capacity_units: 100,
            proposed_privacy_cost_milli: 50,
            privacy_budget_milli: 500,
            approval_recorded: true,
            withdrawal_requested: false,
            withdrawal_reason: None,
            aggregate_only: true,
            raw_data_local: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    #[test]
    fn approved_review_is_explicitly_participation_ready_but_never_dispatches() {
        let review = review_glioma_site_participation(&request()).unwrap();
        assert_eq!(review.disposition, ParticipationDisposition::Approved);
        assert!(review.participation_permitted);
        assert!(!review.query_dispatch_permitted);
        assert!(review.withdrawal_available);
    }

    #[test]
    fn missing_approval_never_becomes_admission() {
        let mut req = request();
        req.approval_recorded = false;
        let review = review_glioma_site_participation(&req).unwrap();
        assert_eq!(
            review.disposition,
            ParticipationDisposition::ApprovalRequired
        );
        assert!(!review.participation_permitted);
        assert_eq!(review.blockers, Vec::<String>::new());
        assert!(!review.warnings.is_empty());
    }

    #[test]
    fn protocol_capability_and_budget_failures_are_visible() {
        let mut req = request();
        req.requested_field_order = vec![
            "effect".into(),
            "spatial_state".into(),
            "uncertainty".into(),
        ];
        req.requested_model_order = vec!["organoid".into(), "slice".into(), "xenograft".into()];
        req.local_protocol_version = "old".into();
        req.proposed_workload_units = 101;
        req.proposed_privacy_cost_milli = 501;
        let review = review_glioma_site_participation(&req).unwrap();
        assert_eq!(review.disposition, ParticipationDisposition::Blocked);
        assert!(review.missing_field_order.contains(&"spatial_state".into()));
        assert!(review.missing_model_order.contains(&"slice".into()));
        assert!(!review.protocol_compatible);
        assert!(!review.workload_within_capacity);
        assert!(!review.privacy_within_budget);
    }

    #[test]
    fn withdrawal_supersedes_prior_approval() {
        let mut req = request();
        req.withdrawal_requested = true;
        req.withdrawal_reason = Some("local maintenance window".into());
        let review = review_glioma_site_participation(&req).unwrap();
        assert_eq!(review.disposition, ParticipationDisposition::Withdrawn);
        assert!(!review.participation_permitted);
        assert!(review
            .blockers
            .iter()
            .any(|reason| reason.contains("withdrawal")));
    }

    #[test]
    fn replay_is_content_deterministic() {
        let first = review_glioma_site_participation(&request()).unwrap();
        let second = review_glioma_site_participation(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}
