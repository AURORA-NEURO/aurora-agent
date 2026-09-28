//! Federated compute capacity and cost exchange for preclinical glioma research.
//!
//! Sites publish signed, aggregate capacity summaries rather than credentials, raw inputs, or
//! workflow outputs. The coordinator ranks only policy-permitted local execution options and
//! keeps stale, revoked, incompatible, and reconstruction-risky summaries visible.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedComputeCapacityEnvelope1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_ITEMS: usize = 512;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCapacitySiteSummary {
    pub site_id: String,
    pub membership_version: String,
    pub workflow_envelope_digest: ContentHash,
    pub policy_digest: ContentHash,
    pub sampled_tick: u64,
    pub expires_at_tick: u64,
    pub available_concurrency: usize,
    pub resource_capacity_units: u64,
    pub memory_capacity_mb: u32,
    pub accelerator_capacity_count: u16,
    pub cost_per_resource_milli: u64,
    pub cost_per_tick_milli: u64,
    pub cost_uncertainty_milli: u16,
    pub capability_order: Vec<String>,
    pub localization_scope_order: Vec<String>,
    pub aggregate_only: bool,
    pub local_only: bool,
    pub contains_raw_data: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub contains_clinical_decision: bool,
    pub reconstruction_risk_milli: u16,
    pub signer_id: String,
    pub signature_digest: ContentHash,
    pub attestation_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedComputeExchangePolicy {
    pub federation_policy_version: String,
    pub allowed_site_order: Vec<String>,
    pub revoked_site_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub required_localization_scope_order: Vec<String>,
    pub max_age_ticks: u64,
    pub expires_at_tick: u64,
    pub require_signed_summaries: bool,
    pub require_aggregate_only: bool,
    pub require_local_only: bool,
    pub max_reconstruction_risk_milli: u16,
    pub max_estimated_cost_milli: u64,
    pub policy_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedComputeCostExchangeRequest {
    pub workflow_id: String,
    pub workflow_envelope_digest: ContentHash,
    pub resource_units: u64,
    pub estimated_duration_ticks: u64,
    pub permitted_site_order: Vec<String>,
    pub summaries: Vec<FederatedCapacitySiteSummary>,
    pub policy: FederatedComputeExchangePolicy,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedCapacitySiteDisposition {
    Accepted,
    Rejected,
    Stale,
    Revoked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCapacitySiteOption {
    pub site_id: String,
    pub disposition: FederatedCapacitySiteDisposition,
    pub available_concurrency: usize,
    pub resource_capacity_units: u64,
    pub memory_capacity_mb: u32,
    pub accelerator_capacity_count: u16,
    pub estimated_cost_lower_milli: u64,
    pub estimated_cost_upper_milli: u64,
    pub freshness_age_ticks: Option<u64>,
    pub localization_scope_order: Vec<String>,
    pub reason_order: Vec<String>,
    pub evidence_digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedComputeEnvelopeDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedComputeCapacityEnvelope {
    pub feature_id: String,
    pub output_schema: String,
    pub workflow_id: String,
    pub workflow_envelope_digest: ContentHash,
    pub placement_order: Vec<String>,
    pub options: Vec<FederatedCapacitySiteOption>,
    pub excluded_site_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedComputeEnvelopeDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedComputeCapacityError {
    #[error("federated compute exchange request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated compute capacity envelope is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated compute exchange digest failed: {0}")]
    Digest(String),
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values
            .iter()
            .all(|value| safe_text(value) && seen.insert(value.clone()))
}

fn summary_body(summary: &FederatedCapacitySiteSummary) -> serde_json::Value {
    serde_json::json!({
        "site_id": summary.site_id,
        "membership_version": summary.membership_version,
        "workflow_envelope_digest": summary.workflow_envelope_digest,
        "policy_digest": summary.policy_digest,
        "sampled_tick": summary.sampled_tick,
        "expires_at_tick": summary.expires_at_tick,
        "available_concurrency": summary.available_concurrency,
        "resource_capacity_units": summary.resource_capacity_units,
        "memory_capacity_mb": summary.memory_capacity_mb,
        "accelerator_capacity_count": summary.accelerator_capacity_count,
        "cost_per_resource_milli": summary.cost_per_resource_milli,
        "cost_per_tick_milli": summary.cost_per_tick_milli,
        "cost_uncertainty_milli": summary.cost_uncertainty_milli,
        "capability_order": summary.capability_order,
        "localization_scope_order": summary.localization_scope_order,
        "aggregate_only": summary.aggregate_only,
        "local_only": summary.local_only,
        "contains_raw_data": summary.contains_raw_data,
        "contains_human_data": summary.contains_human_data,
        "contains_direct_identifiers": summary.contains_direct_identifiers,
        "contains_clinical_decision": summary.contains_clinical_decision,
        "reconstruction_risk_milli": summary.reconstruction_risk_milli,
        "signer_id": summary.signer_id,
    })
}

fn attestation_body(summary: &FederatedCapacitySiteSummary) -> serde_json::Value {
    serde_json::json!({
        "summary": summary_body(summary),
        "signature_digest": summary.signature_digest,
    })
}

fn digest_input(envelope: &FederatedComputeCapacityEnvelope) -> serde_json::Value {
    serde_json::json!({
        "feature_id": envelope.feature_id,
        "output_schema": envelope.output_schema,
        "workflow_id": envelope.workflow_id,
        "workflow_envelope_digest": envelope.workflow_envelope_digest,
        "placement_order": envelope.placement_order,
        "options": envelope.options,
        "excluded_site_order": envelope.excluded_site_order,
        "negative_evidence": envelope.negative_evidence,
        "uncertainty": envelope.uncertainty,
        "disposition": envelope.disposition,
    })
}

fn validate_request(
    request: &FederatedComputeCostExchangeRequest,
) -> Result<(), FederatedComputeCapacityError> {
    let policy = &request.policy;
    if !safe_text(&request.workflow_id)
        || !valid_hash(&request.workflow_envelope_digest)
        || request.resource_units == 0
        || request.estimated_duration_ticks == 0
        || request.current_tick == 0
        || request.summaries.len() > MAX_SITES
        || !unique_bounded(&request.permitted_site_order, MAX_SITES)
        || !canonical(&request.permitted_site_order)
        || !safe_text(&policy.federation_policy_version)
        || !unique_bounded(&policy.allowed_site_order, MAX_SITES)
        || !canonical(&policy.allowed_site_order)
        || !unique_bounded(&policy.revoked_site_order, MAX_SITES)
        || !canonical(&policy.revoked_site_order)
        || !unique_bounded(&policy.required_capability_order, MAX_ITEMS)
        || !canonical(&policy.required_capability_order)
        || !unique_bounded(&policy.required_localization_scope_order, MAX_ITEMS)
        || !canonical(&policy.required_localization_scope_order)
        || policy.max_age_ticks == 0
        || policy.expires_at_tick < request.current_tick
        || policy.max_estimated_cost_milli == 0
        || !valid_hash(&policy.policy_digest)
    {
        return Err(FederatedComputeCapacityError::InvalidRequest(
            "bounded workflow, site membership, capability, freshness, cost, and policy identities are required".into(),
        ));
    }
    if request
        .permitted_site_order
        .iter()
        .any(|site| !policy.allowed_site_order.binary_search(site).is_ok())
    {
        return Err(FederatedComputeCapacityError::InvalidRequest(
            "requested sites must be in the allowed federation membership".into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    for summary in &request.summaries {
        if !safe_text(&summary.site_id)
            || !site_ids.insert(summary.site_id.clone())
            || !safe_text(&summary.membership_version)
            || !valid_hash(&summary.workflow_envelope_digest)
            || !valid_hash(&summary.policy_digest)
            || summary.sampled_tick == 0
            || summary.expires_at_tick < summary.sampled_tick
            || summary.available_concurrency == 0
            || summary.resource_capacity_units == 0
            || summary.memory_capacity_mb == 0
            || summary.cost_per_resource_milli == 0
            || summary.cost_per_tick_milli == 0
            || !unique_bounded(&summary.capability_order, MAX_ITEMS)
            || !canonical(&summary.capability_order)
            || !unique_bounded(&summary.localization_scope_order, MAX_ITEMS)
            || !canonical(&summary.localization_scope_order)
            || !summary.aggregate_only
            || !summary.local_only
            || summary.contains_raw_data
            || summary.contains_human_data
            || summary.contains_direct_identifiers
            || summary.contains_clinical_decision
            || summary.reconstruction_risk_milli > 1_000
            || !safe_text(&summary.signer_id)
            || !valid_hash(&summary.signature_digest)
            || !valid_hash(&summary.attestation_digest)
        {
            return Err(FederatedComputeCapacityError::InvalidRequest(
                "site summaries must be unique, aggregate-only, local, signed, bounded, and free of protected or clinical content".into(),
            ));
        }
    }
    Ok(())
}

impl FederatedComputeCapacityEnvelope {
    pub fn validate(&self) -> Result<(), FederatedComputeCapacityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.workflow_id)
            || !valid_hash(&self.workflow_envelope_digest)
            || !unique_bounded(&self.placement_order, MAX_SITES)
            || !unique_bounded(&self.excluded_site_order, MAX_SITES)
            || !canonical(&self.excluded_site_order)
            || !unique_bounded(&self.negative_evidence, MAX_ITEMS)
            || !canonical(&self.negative_evidence)
            || !unique_bounded(&self.uncertainty, MAX_ITEMS)
            || !canonical(&self.uncertainty)
            || self.options.len() > MAX_SITES
            || !valid_hash(&self.digest)
        {
            return Err(FederatedComputeCapacityError::InvalidOutput(
                "capacity-envelope identity, sorted site evidence, bounds, or digest is invalid"
                    .into(),
            ));
        }
        let option_ids = self
            .options
            .iter()
            .map(|option| option.site_id.clone())
            .collect::<Vec<_>>();
        if !unique_bounded(&option_ids, MAX_SITES) || !canonical(&option_ids) {
            return Err(FederatedComputeCapacityError::InvalidOutput(
                "site option identities must be unique and canonical".into(),
            ));
        }
        for option in &self.options {
            if !safe_text(&option.site_id)
                || !valid_hash(&option.evidence_digest)
                || !unique_bounded(&option.localization_scope_order, MAX_ITEMS)
                || !canonical(&option.localization_scope_order)
                || !unique_bounded(&option.reason_order, MAX_ITEMS)
                || !canonical(&option.reason_order)
                || option.estimated_cost_upper_milli < option.estimated_cost_lower_milli
            {
                return Err(FederatedComputeCapacityError::InvalidOutput(
                    "site option bounds, evidence, and reason ordering are invalid".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedComputeCapacityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedComputeCapacityError::InvalidOutput(
                "capacity envelope digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn evidence_digest(
    site_id: &str,
    disposition: FederatedCapacitySiteDisposition,
    reasons: &[String],
) -> Result<ContentHash, FederatedComputeCapacityError> {
    ContentHash::of_value(&serde_json::json!({
        "site_id": site_id,
        "disposition": disposition,
        "reasons": reasons,
    }))
    .map_err(|error| FederatedComputeCapacityError::Digest(error.to_string()))
}

fn cost_interval(
    summary: &FederatedCapacitySiteSummary,
    request: &FederatedComputeCostExchangeRequest,
) -> (u64, u64) {
    let lower = summary
        .cost_per_resource_milli
        .saturating_mul(request.resource_units)
        .saturating_add(
            summary
                .cost_per_tick_milli
                .saturating_mul(request.estimated_duration_ticks),
        );
    let upper = lower
        .saturating_add(lower.saturating_mul(u64::from(summary.cost_uncertainty_milli)) / 1_000);
    (lower, upper.max(lower))
}

fn has_all(required: &[String], available: &[String]) -> bool {
    required
        .iter()
        .all(|value| available.binary_search(value).is_ok())
}

fn missing_values(required: &[String], available: &[String]) -> Vec<String> {
    required
        .iter()
        .filter(|value| available.binary_search(value).is_err())
        .cloned()
        .collect()
}

/// Exchange signed aggregate capacity classes and cost intervals without moving local data.
pub fn exchange_glioma_compute_capacity(
    request: &FederatedComputeCostExchangeRequest,
) -> Result<FederatedComputeCapacityEnvelope, FederatedComputeCapacityError> {
    validate_request(request)?;
    let policy = &request.policy;
    let mut summaries = request
        .summaries
        .iter()
        .map(|summary| (summary.site_id.clone(), summary))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut options = Vec::new();
    let mut placement = Vec::new();
    let mut excluded = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();

    for site_id in &request.permitted_site_order {
        let Some(summary) = summaries.remove(site_id) else {
            let reasons = vec!["summary-missing".to_string()];
            let option = FederatedCapacitySiteOption {
                site_id: site_id.clone(),
                disposition: FederatedCapacitySiteDisposition::Unresolved,
                available_concurrency: 0,
                resource_capacity_units: 0,
                memory_capacity_mb: 0,
                accelerator_capacity_count: 0,
                estimated_cost_lower_milli: 0,
                estimated_cost_upper_milli: 0,
                freshness_age_ticks: None,
                localization_scope_order: Vec::new(),
                reason_order: reasons.clone(),
                evidence_digest: evidence_digest(
                    site_id,
                    option_disposition("summary-missing"),
                    &reasons,
                )?,
            };
            excluded.push(site_id.clone());
            negative_evidence.push(format!("{}:summary-missing", site_id));
            uncertainty.push(format!("{}:capacity-summary-unresolved", site_id));
            options.push(option);
            continue;
        };
        let age = request.current_tick.saturating_sub(summary.sampled_tick);
        let (lower, upper) = cost_interval(summary, request);
        let mut reasons = Vec::new();
        let mut disposition = FederatedCapacitySiteDisposition::Accepted;
        if policy.revoked_site_order.binary_search(site_id).is_ok()
            || summary.expires_at_tick < request.current_tick
        {
            disposition = if policy.revoked_site_order.binary_search(site_id).is_ok() {
                FederatedCapacitySiteDisposition::Revoked
            } else {
                FederatedCapacitySiteDisposition::Stale
            };
            reasons.push(
                if matches!(disposition, FederatedCapacitySiteDisposition::Revoked) {
                    "site-revoked".to_string()
                } else {
                    "summary-expired".to_string()
                },
            );
        }
        if age > policy.max_age_ticks {
            disposition = FederatedCapacitySiteDisposition::Stale;
            reasons.push("summary-stale".into());
        }
        if policy.require_signed_summaries {
            let signature = ContentHash::of_value(&summary_body(summary))
                .map_err(|error| FederatedComputeCapacityError::Digest(error.to_string()))?;
            let attestation = ContentHash::of_value(&attestation_body(summary))
                .map_err(|error| FederatedComputeCapacityError::Digest(error.to_string()))?;
            if signature != summary.signature_digest || attestation != summary.attestation_digest {
                disposition = FederatedCapacitySiteDisposition::Rejected;
                reasons.push("signature-invalid".into());
            }
        }
        if summary.workflow_envelope_digest != request.workflow_envelope_digest {
            disposition = FederatedCapacitySiteDisposition::Rejected;
            reasons.push("workflow-envelope-mismatch".into());
        }
        if !has_all(&policy.required_capability_order, &summary.capability_order) {
            disposition = FederatedCapacitySiteDisposition::Rejected;
            let missing =
                missing_values(&policy.required_capability_order, &summary.capability_order);
            reasons.push(format!("capability-missing:{}", missing.join(",")));
        }
        if !has_all(
            &policy.required_localization_scope_order,
            &summary.localization_scope_order,
        ) {
            disposition = FederatedCapacitySiteDisposition::Rejected;
            reasons.push("localization-scope-missing".into());
        }
        if policy.require_aggregate_only && !summary.aggregate_only {
            disposition = FederatedCapacitySiteDisposition::Rejected;
            reasons.push("aggregate-only-required".into());
        }
        if policy.require_local_only && !summary.local_only {
            disposition = FederatedCapacitySiteDisposition::Rejected;
            reasons.push("local-only-required".into());
        }
        if summary.reconstruction_risk_milli > policy.max_reconstruction_risk_milli {
            disposition = FederatedCapacitySiteDisposition::Rejected;
            reasons.push("reconstruction-risk-too-high".into());
        }
        if upper > policy.max_estimated_cost_milli {
            disposition = FederatedCapacitySiteDisposition::Rejected;
            reasons.push("estimated-cost-exceeds-policy".into());
        }
        reasons.sort();
        reasons.dedup();
        let evidence = evidence_digest(site_id, disposition, &reasons)?;
        if matches!(disposition, FederatedCapacitySiteDisposition::Accepted) {
            placement.push((lower, site_id.clone()));
        } else {
            excluded.push(site_id.clone());
            for reason in &reasons {
                negative_evidence.push(format!("{}:{}", site_id, reason));
            }
        }
        if age > policy.max_age_ticks || summary.cost_uncertainty_milli > 0 {
            uncertainty.push(format!("{}:cost-or-freshness-interval", site_id));
        }
        options.push(FederatedCapacitySiteOption {
            site_id: site_id.clone(),
            disposition,
            available_concurrency: summary.available_concurrency,
            resource_capacity_units: summary.resource_capacity_units,
            memory_capacity_mb: summary.memory_capacity_mb,
            accelerator_capacity_count: summary.accelerator_capacity_count,
            estimated_cost_lower_milli: lower,
            estimated_cost_upper_milli: upper,
            freshness_age_ticks: Some(age),
            localization_scope_order: summary.localization_scope_order.clone(),
            reason_order: reasons,
            evidence_digest: evidence,
        });
    }
    placement.sort();
    let placement_order = placement
        .into_iter()
        .map(|(_, site)| site)
        .collect::<Vec<_>>();
    options.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    excluded.sort();
    excluded.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if placement_order.is_empty() {
        FederatedComputeEnvelopeDisposition::Blocked
    } else if excluded.is_empty() {
        FederatedComputeEnvelopeDisposition::Ready
    } else {
        FederatedComputeEnvelopeDisposition::Partial
    };
    let mut envelope = FederatedComputeCapacityEnvelope {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        workflow_id: request.workflow_id.clone(),
        workflow_envelope_digest: request.workflow_envelope_digest.clone(),
        placement_order,
        options,
        excluded_site_order: excluded,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-capacity-envelope"),
    };
    envelope.digest = ContentHash::of_value(&digest_input(&envelope))
        .map_err(|error| FederatedComputeCapacityError::Digest(error.to_string()))?;
    envelope.validate()?;
    Ok(envelope)
}

fn option_disposition(reason: &str) -> FederatedCapacitySiteDisposition {
    if reason == "summary-missing" {
        FederatedCapacitySiteDisposition::Unresolved
    } else {
        FederatedCapacitySiteDisposition::Rejected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn summary(site_id: &str, cost: u64, sampled_tick: u64) -> FederatedCapacitySiteSummary {
        let mut summary = FederatedCapacitySiteSummary {
            site_id: site_id.into(),
            membership_version: "members-1".into(),
            workflow_envelope_digest: hash("workflow"),
            policy_digest: hash("policy"),
            sampled_tick,
            expires_at_tick: 100,
            available_concurrency: 4,
            resource_capacity_units: 32,
            memory_capacity_mb: 16_384,
            accelerator_capacity_count: 2,
            cost_per_resource_milli: cost,
            cost_per_tick_milli: 1,
            cost_uncertainty_milli: 50,
            capability_order: vec!["cuda".into(), "imaging".into()],
            localization_scope_order: vec!["organoid".into()],
            aggregate_only: true,
            local_only: true,
            contains_raw_data: false,
            contains_human_data: false,
            contains_direct_identifiers: false,
            contains_clinical_decision: false,
            reconstruction_risk_milli: 10,
            signer_id: format!("signer-{site_id}"),
            signature_digest: hash("unsealed-signature"),
            attestation_digest: hash("unsealed-attestation"),
        };
        summary.signature_digest =
            ContentHash::of_value(&summary_body(&summary)).expect("signature");
        summary.attestation_digest =
            ContentHash::of_value(&attestation_body(&summary)).expect("attestation");
        summary
    }

    fn request(
        summaries: Vec<FederatedCapacitySiteSummary>,
    ) -> FederatedComputeCostExchangeRequest {
        FederatedComputeCostExchangeRequest {
            workflow_id: "glioma-imaging".into(),
            workflow_envelope_digest: hash("workflow"),
            resource_units: 4,
            estimated_duration_ticks: 10,
            permitted_site_order: vec!["site-a".into(), "site-b".into()],
            summaries,
            policy: FederatedComputeExchangePolicy {
                federation_policy_version: "federation-1".into(),
                allowed_site_order: vec!["site-a".into(), "site-b".into()],
                revoked_site_order: Vec::new(),
                required_capability_order: vec!["cuda".into(), "imaging".into()],
                required_localization_scope_order: vec!["organoid".into()],
                max_age_ticks: 10,
                expires_at_tick: 100,
                require_signed_summaries: true,
                require_aggregate_only: true,
                require_local_only: true,
                max_reconstruction_risk_milli: 100,
                max_estimated_cost_milli: 10_000,
                policy_digest: hash("policy"),
            },
            current_tick: 5,
        }
    }

    #[test]
    fn accepted_sites_are_ranked_by_cost_without_exporting_payloads() {
        let envelope = exchange_glioma_compute_capacity(&request(vec![
            summary("site-a", 30, 4),
            summary("site-b", 10, 4),
        ]))
        .expect("envelope");
        assert_eq!(
            envelope.disposition,
            FederatedComputeEnvelopeDisposition::Ready
        );
        assert_eq!(envelope.placement_order, vec!["site-b", "site-a"]);
        assert!(envelope
            .options
            .iter()
            .all(|option| { option.disposition == FederatedCapacitySiteDisposition::Accepted }));
    }

    #[test]
    fn stale_and_revoked_sites_are_excluded_explicitly() {
        let mut request = request(vec![summary("site-a", 10, 1), summary("site-b", 10, 4)]);
        request.policy.revoked_site_order = vec!["site-b".into()];
        request.policy.max_age_ticks = 2;
        let envelope = exchange_glioma_compute_capacity(&request).expect("envelope");
        assert_eq!(
            envelope.disposition,
            FederatedComputeEnvelopeDisposition::Blocked
        );
        assert!(envelope.placement_order.is_empty());
        assert!(envelope
            .negative_evidence
            .iter()
            .any(|item| item.contains("site-b:site-revoked")));
        assert!(envelope
            .negative_evidence
            .iter()
            .any(|item| item.contains("site-a:summary-stale")));
    }

    #[test]
    fn missing_required_site_remains_unresolved() {
        let envelope = exchange_glioma_compute_capacity(&request(vec![summary("site-a", 10, 4)]))
            .expect("envelope");
        assert_eq!(
            envelope.disposition,
            FederatedComputeEnvelopeDisposition::Partial
        );
        let missing = envelope
            .options
            .iter()
            .find(|option| option.site_id == "site-b")
            .expect("site-b");
        assert_eq!(
            missing.disposition,
            FederatedCapacitySiteDisposition::Unresolved
        );
        assert_eq!(missing.reason_order, vec!["summary-missing"]);
    }

    #[test]
    fn tampered_signature_never_becomes_a_placement_option() {
        let mut tampered = summary("site-a", 10, 4);
        tampered.signer_id = "attacker".into();
        let envelope =
            exchange_glioma_compute_capacity(&request(vec![tampered, summary("site-b", 20, 4)]))
                .expect("envelope");
        assert_eq!(envelope.placement_order, vec!["site-b"]);
        assert!(envelope
            .negative_evidence
            .iter()
            .any(|item| item.contains("site-a:signature-invalid")));
    }

    #[test]
    fn reconstruction_risk_and_cost_limits_are_policy_visible() {
        let mut risky = summary("site-a", 10, 4);
        risky.reconstruction_risk_milli = 900;
        let mut request = request(vec![risky, summary("site-b", 10_000, 4)]);
        request.policy.max_reconstruction_risk_milli = 100;
        request.policy.max_estimated_cost_milli = 100;
        let envelope = exchange_glioma_compute_capacity(&request).expect("envelope");
        assert_eq!(
            envelope.disposition,
            FederatedComputeEnvelopeDisposition::Blocked
        );
        assert!(envelope
            .negative_evidence
            .iter()
            .any(|item| item.contains("reconstruction-risk")));
        assert!(envelope
            .negative_evidence
            .iter()
            .any(|item| item.contains("estimated-cost")));
    }
}
