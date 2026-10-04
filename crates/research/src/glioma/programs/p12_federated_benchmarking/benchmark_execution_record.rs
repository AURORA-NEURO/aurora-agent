//! Immutable, replayable federated benchmark execution records for glioma research.
//!
//! This boundary records the exact benchmark/policy/executor binding, site attestations admitted
//! to analysis, explicit site omissions, aggregate metric inputs, uncertainty, and replay identity.
//! It is an execution artifact rather than a receipt: recomputing its content digest is sufficient
//! to detect input or benchmark-version tampering.  Only aggregate metadata is represented.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkRunRecord1@1";
pub const MAX_SITES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkSiteContribution {
    pub site_id: String,
    pub benchmark_id: String,
    pub benchmark_version: String,
    pub policy_scope: String,
    pub attestation_digest: ContentHash,
    pub aggregate_digest: ContentHash,
    pub metric_digest: ContentHash,
    pub uncertainty_milli: u16,
    pub observed_epoch: u64,
    pub valid_until_epoch: u64,
    pub signed: bool,
    pub signer_revoked: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkExecutionRequest {
    pub benchmark_id: String,
    pub benchmark_version: String,
    pub policy_scope: String,
    pub executor_version: String,
    pub replay_identity: ContentHash,
    pub current_epoch: u64,
    pub max_staleness_epochs: u64,
    pub required_quorum: u16,
    pub analysis_digest: ContentHash,
    pub uncertainty_digest: ContentHash,
    pub contributions: Vec<FederatedBenchmarkSiteContribution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBenchmarkRunStatus {
    Completed,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkRunRecord {
    pub feature_id: String,
    pub output_schema: String,
    pub benchmark_id: String,
    pub benchmark_version: String,
    pub policy_scope: String,
    pub executor_version: String,
    pub replay_identity: ContentHash,
    pub required_quorum: u16,
    pub included_site_order: Vec<String>,
    pub omitted_site_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub attestation_digest_order: Vec<ContentHash>,
    pub aggregate_input_digest_order: Vec<ContentHash>,
    pub metric_digest_order: Vec<ContentHash>,
    pub uncertainty_order_milli: Vec<u16>,
    pub analysis_digest: ContentHash,
    pub uncertainty_digest: ContentHash,
    pub pooled_metric_digest: ContentHash,
    pub status: FederatedBenchmarkRunStatus,
    pub run_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBenchmarkExecutionError {
    #[error("federated benchmark execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated benchmark execution record is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated benchmark execution digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn digest(digest: &ContentHash) -> bool {
    digest.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedBenchmarkRunRecord) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "benchmark_id": output.benchmark_id,
        "benchmark_version": output.benchmark_version,
        "policy_scope": output.policy_scope,
        "executor_version": output.executor_version,
        "replay_identity": output.replay_identity,
        "required_quorum": output.required_quorum,
        "included_site_order": output.included_site_order,
        "omitted_site_order": output.omitted_site_order,
        "omission_order": output.omission_order,
        "attestation_digest_order": output.attestation_digest_order,
        "aggregate_input_digest_order": output.aggregate_input_digest_order,
        "metric_digest_order": output.metric_digest_order,
        "uncertainty_order_milli": output.uncertainty_order_milli,
        "analysis_digest": output.analysis_digest,
        "uncertainty_digest": output.uncertainty_digest,
        "pooled_metric_digest": output.pooled_metric_digest,
        "status": output.status,
    })
}

fn validate_request(
    request: &FederatedBenchmarkExecutionRequest,
) -> Result<(), FederatedBenchmarkExecutionError> {
    if !identifier(&request.benchmark_id)
        || !identifier(&request.benchmark_version)
        || !identifier(&request.policy_scope)
        || !identifier(&request.executor_version)
        || !digest(&request.replay_identity)
        || !digest(&request.analysis_digest)
        || !digest(&request.uncertainty_digest)
        || request.current_epoch == 0
        || request.max_staleness_epochs == 0
        || request.required_quorum == 0
        || request.contributions.is_empty()
        || request.contributions.len() > MAX_SITES
    {
        return Err(FederatedBenchmarkExecutionError::InvalidRequest(
            "benchmark/policy/executor identity, replay and analysis digests, epoch bounds, quorum, and bounded contributions are required".into(),
        ));
    }
    let mut sites = BTreeSet::new();
    for contribution in &request.contributions {
        if !identifier(&contribution.site_id)
            || !identifier(&contribution.benchmark_id)
            || !identifier(&contribution.benchmark_version)
            || !identifier(&contribution.policy_scope)
            || !digest(&contribution.attestation_digest)
            || !digest(&contribution.aggregate_digest)
            || !digest(&contribution.metric_digest)
            || contribution.uncertainty_milli > 1_000
            || contribution.observed_epoch == 0
            || contribution.valid_until_epoch < contribution.observed_epoch
            || !sites.insert(contribution.site_id.clone())
        {
            return Err(FederatedBenchmarkExecutionError::InvalidRequest(
                "site contributions require unique bounded identities, hashes, uncertainty, and validity windows".into(),
            ));
        }
    }
    Ok(())
}

impl FederatedBenchmarkRunRecord {
    pub fn validate(&self) -> Result<(), FederatedBenchmarkExecutionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.benchmark_id)
            || !identifier(&self.benchmark_version)
            || !identifier(&self.policy_scope)
            || !identifier(&self.executor_version)
            || !digest(&self.replay_identity)
            || self.required_quorum == 0
            || !canonical(&self.included_site_order)
            || !canonical(&self.omitted_site_order)
            || !canonical(&self.omission_order)
            || self.included_site_order.len() != self.attestation_digest_order.len()
            || self.included_site_order.len() != self.aggregate_input_digest_order.len()
            || self.included_site_order.len() != self.metric_digest_order.len()
            || self.included_site_order.len() != self.uncertainty_order_milli.len()
            || self
                .included_site_order
                .iter()
                .any(|site| self.omitted_site_order.binary_search(site).is_ok())
            || self
                .uncertainty_order_milli
                .iter()
                .any(|value| *value > 1_000)
            || !digest(&self.analysis_digest)
            || !digest(&self.uncertainty_digest)
            || !digest(&self.pooled_metric_digest)
            || !digest(&self.run_digest)
        {
            return Err(FederatedBenchmarkExecutionError::InvalidOutput(
                "run identity, canonical inclusion/omission partitions, aligned inputs, bounds, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBenchmarkExecutionError::Digest(error.to_string()))?;
        if expected != self.run_digest {
            return Err(FederatedBenchmarkExecutionError::InvalidOutput(
                "run digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile an immutable, replayable aggregate-only federated benchmark execution record.
pub fn execute_glioma_federated_benchmark_record(
    request: &FederatedBenchmarkExecutionRequest,
) -> Result<FederatedBenchmarkRunRecord, FederatedBenchmarkExecutionError> {
    validate_request(request)?;
    let mut accepted = Vec::new();
    let mut omitted = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    for contribution in &request.contributions {
        let mut reasons = Vec::new();
        if contribution.benchmark_id != request.benchmark_id {
            reasons.push("benchmark-mismatch");
        }
        if contribution.benchmark_version != request.benchmark_version {
            reasons.push("benchmark-version-mismatch");
        }
        if contribution.policy_scope != request.policy_scope {
            reasons.push("policy-scope-mismatch");
        }
        if !contribution.signed {
            reasons.push("unsigned-attestation");
        }
        if contribution.signer_revoked {
            reasons.push("signer-revoked");
        }
        if contribution.valid_until_epoch < request.current_epoch {
            reasons.push("attestation-expired");
        }
        if request
            .current_epoch
            .saturating_sub(contribution.observed_epoch)
            > request.max_staleness_epochs
        {
            reasons.push("contribution-stale");
        }
        if !contribution.aggregate_only {
            reasons.push("not-aggregate-only");
        }
        if !contribution.raw_data_local {
            reasons.push("raw-data-not-local");
        }
        if contribution.contains_human_data || contribution.contains_direct_identifiers {
            reasons.push("protected-data-present");
        }
        if reasons.is_empty() {
            accepted.push(contribution);
        } else {
            omitted.insert(contribution.site_id.clone());
            reasons.sort_unstable();
            for reason in reasons {
                omissions.insert(format!("{}:{reason}", contribution.site_id));
            }
        }
    }
    accepted.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let included_site_order = accepted
        .iter()
        .map(|contribution| contribution.site_id.clone())
        .collect::<Vec<_>>();
    let attestation_digest_order = accepted
        .iter()
        .map(|contribution| contribution.attestation_digest.clone())
        .collect::<Vec<_>>();
    let aggregate_input_digest_order = accepted
        .iter()
        .map(|contribution| contribution.aggregate_digest.clone())
        .collect::<Vec<_>>();
    let metric_digest_order = accepted
        .iter()
        .map(|contribution| contribution.metric_digest.clone())
        .collect::<Vec<_>>();
    let uncertainty_order_milli = accepted
        .iter()
        .map(|contribution| contribution.uncertainty_milli)
        .collect::<Vec<_>>();
    let pooled_metric_digest = ContentHash::of_value(&serde_json::json!({
        "benchmark_id": request.benchmark_id.clone(),
        "benchmark_version": request.benchmark_version.clone(),
        "metric_digest_order": metric_digest_order.clone(),
        "uncertainty_order_milli": uncertainty_order_milli.clone(),
    }))
    .map_err(|error| FederatedBenchmarkExecutionError::Digest(error.to_string()))?;
    let status = if accepted.len() < request.required_quorum as usize {
        FederatedBenchmarkRunStatus::Blocked
    } else if omitted.is_empty() {
        FederatedBenchmarkRunStatus::Completed
    } else {
        FederatedBenchmarkRunStatus::Partial
    };
    let mut output = FederatedBenchmarkRunRecord {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        benchmark_id: request.benchmark_id.clone(),
        benchmark_version: request.benchmark_version.clone(),
        policy_scope: request.policy_scope.clone(),
        executor_version: request.executor_version.clone(),
        replay_identity: request.replay_identity.clone(),
        required_quorum: request.required_quorum,
        included_site_order,
        omitted_site_order: omitted.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        attestation_digest_order,
        aggregate_input_digest_order,
        metric_digest_order,
        uncertainty_order_milli,
        analysis_digest: request.analysis_digest.clone(),
        uncertainty_digest: request.uncertainty_digest.clone(),
        pooled_metric_digest,
        status,
        run_digest: ContentHash::of_bytes(b"unsealed-glioma-federated-benchmark-run"),
    };
    output.run_digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBenchmarkExecutionError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn contribution(site: &str) -> FederatedBenchmarkSiteContribution {
        FederatedBenchmarkSiteContribution {
            site_id: site.into(),
            benchmark_id: "benchmark-1".into(),
            benchmark_version: "2026.1".into(),
            policy_scope: "glioma-v1".into(),
            attestation_digest: hash(&format!("{site}-attestation")),
            aggregate_digest: hash(&format!("{site}-aggregate")),
            metric_digest: hash(&format!("{site}-metric")),
            uncertainty_milli: 120,
            observed_epoch: 95,
            valid_until_epoch: 110,
            signed: true,
            signer_revoked: false,
            aggregate_only: true,
            raw_data_local: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> FederatedBenchmarkExecutionRequest {
        FederatedBenchmarkExecutionRequest {
            benchmark_id: "benchmark-1".into(),
            benchmark_version: "2026.1".into(),
            policy_scope: "glioma-v1".into(),
            executor_version: "executor-2".into(),
            replay_identity: hash("replay"),
            current_epoch: 100,
            max_staleness_epochs: 10,
            required_quorum: 2,
            analysis_digest: hash("analysis"),
            uncertainty_digest: hash("uncertainty"),
            contributions: vec![contribution("site-b"), contribution("site-a")],
        }
    }

    #[test]
    fn valid_sites_are_sorted_and_recorded_for_replay() {
        let output = execute_glioma_federated_benchmark_record(&request()).unwrap();
        assert_eq!(output.status, FederatedBenchmarkRunStatus::Completed);
        assert_eq!(output.included_site_order, vec!["site-a", "site-b"]);
        assert!(output.omission_order.is_empty());
        output.validate().unwrap();
    }

    #[test]
    fn omitted_sites_keep_reason_codes_and_partial_status() {
        let mut request = request();
        request.required_quorum = 1;
        request.contributions[0].signed = false;
        let output = execute_glioma_federated_benchmark_record(&request).unwrap();
        assert_eq!(output.status, FederatedBenchmarkRunStatus::Partial);
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason == "site-b:unsigned-attestation"));
    }

    #[test]
    fn quorum_failure_is_blocked_without_inventing_metrics() {
        let mut request = request();
        request.required_quorum = 3;
        let output = execute_glioma_federated_benchmark_record(&request).unwrap();
        assert_eq!(output.status, FederatedBenchmarkRunStatus::Blocked);
        assert_eq!(output.metric_digest_order.len(), 2);
    }

    #[test]
    fn stale_or_protected_sites_are_explicitly_omitted() {
        let mut request = request();
        request.contributions[0].observed_epoch = 80;
        request.contributions[1].contains_direct_identifiers = true;
        let output = execute_glioma_federated_benchmark_record(&request).unwrap();
        assert_eq!(output.status, FederatedBenchmarkRunStatus::Blocked);
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason == "site-b:contribution-stale"));
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason == "site-a:protected-data-present"));
    }

    #[test]
    fn input_or_benchmark_tampering_breaks_record_digest() {
        let mut output = execute_glioma_federated_benchmark_record(&request()).unwrap();
        output.benchmark_version = "tampered".into();
        assert!(matches!(
            output.validate(),
            Err(FederatedBenchmarkExecutionError::InvalidOutput(_))
        ));
    }
}
