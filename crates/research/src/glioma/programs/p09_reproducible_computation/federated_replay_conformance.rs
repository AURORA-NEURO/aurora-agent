//! Federated replay conformance verification for preclinical glioma workflows.
//!
//! Sites exchange signed aggregate metrics, not protected inputs or raw outputs. This verifier
//! binds every metric to one workflow digest, one versioned tolerance profile, and one freshness
//! window. Missing, stale, revoked, or tampered attestations are explicit evidence gaps and can
//! never be mistaken for a passing replication.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedReplayConformanceReport1@1";
pub const MAX_ITEMS: usize = 512;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayMetricTolerance {
    pub metric: String,
    pub reference_milli: i64,
    pub max_absolute_milli: u64,
    pub max_relative_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayConformanceReference {
    pub reference_id: String,
    pub workflow_digest: ContentHash,
    pub tolerance_profile_version: String,
    pub metric_order: Vec<ReplayMetricTolerance>,
    pub required_site_order: Vec<String>,
    pub generated_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaySiteAttestation {
    pub site_id: String,
    pub workflow_digest: ContentHash,
    pub run_digest: ContentHash,
    pub metric_order: Vec<(String, i64)>,
    pub generated_tick: u64,
    pub aggregate_only: bool,
    pub signed: bool,
    pub revoked: bool,
    pub attestation_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayConformancePolicy {
    pub allowed_site_order: Vec<String>,
    pub federation_policy_version: String,
    pub max_age_ticks: u64,
    pub require_signed_attestations: bool,
    pub require_aggregate_only: bool,
    pub expires_at_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReplayConformanceRequest {
    pub reference: ReplayConformanceReference,
    pub attestations: Vec<ReplaySiteAttestation>,
    pub policy: ReplayConformancePolicy,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplaySiteConformance {
    Pass,
    MetricFailure,
    Missing,
    Stale,
    Rejected,
    Tampered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayEvidenceStrength {
    Strong,
    Moderate,
    Weak,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayMetricDeviation {
    pub metric: String,
    pub reference_milli: i64,
    pub observed_milli: Option<i64>,
    pub absolute_error_milli: Option<u64>,
    pub relative_error_milli: Option<u64>,
    pub within_tolerance: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaySiteConformanceResult {
    pub site_id: String,
    pub conformance: ReplaySiteConformance,
    pub run_digest: Option<ContentHash>,
    pub metric_deviation_order: Vec<ReplayMetricDeviation>,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedReplayConformanceDisposition {
    Conformant,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReplayConformanceReport {
    pub feature_id: String,
    pub output_schema: String,
    pub reference_id: String,
    pub workflow_digest: ContentHash,
    pub tolerance_profile_version: String,
    pub site_result_order: Vec<ReplaySiteConformanceResult>,
    pub missing_site_order: Vec<String>,
    pub portability_claim: bool,
    pub evidence_strength: ReplayEvidenceStrength,
    pub disposition: FederatedReplayConformanceDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedReplayConformanceError {
    #[error("federated replay conformance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated replay conformance report is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated replay conformance digest failed: {0}")]
    Digest(String),
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= MAX_ITEMS
        && values.iter().all(|value| {
            !value.trim().is_empty() && value.len() <= MAX_TEXT_LEN && seen.insert(value)
        })
}

fn absolute_error(reference: i64, observed: i64) -> u64 {
    reference.abs_diff(observed)
}

fn relative_error_milli(reference: i64, absolute: u64) -> u64 {
    let denominator = reference.unsigned_abs().max(1);
    absolute.saturating_mul(1_000).saturating_div(denominator)
}

fn attestation_body(attestation: &ReplaySiteAttestation) -> serde_json::Value {
    serde_json::json!({
        "site_id": attestation.site_id,
        "workflow_digest": attestation.workflow_digest,
        "run_digest": attestation.run_digest,
        "metric_order": attestation.metric_order,
        "generated_tick": attestation.generated_tick,
        "aggregate_only": attestation.aggregate_only,
        "signed": attestation.signed,
        "revoked": attestation.revoked,
    })
}

fn digest_input(report: &FederatedReplayConformanceReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "reference_id": report.reference_id,
        "workflow_digest": report.workflow_digest,
        "tolerance_profile_version": report.tolerance_profile_version,
        "site_result_order": report.site_result_order,
        "missing_site_order": report.missing_site_order,
        "portability_claim": report.portability_claim,
        "evidence_strength": report.evidence_strength,
        "disposition": report.disposition,
        "negative_evidence": report.negative_evidence,
        "uncertainty": report.uncertainty,
    })
}

fn validate_request(
    request: &FederatedReplayConformanceRequest,
) -> Result<(), FederatedReplayConformanceError> {
    let reference = &request.reference;
    if reference.reference_id.trim().is_empty()
        || reference.reference_id.len() > MAX_TEXT_LEN
        || !valid_hash(&reference.workflow_digest)
        || reference.tolerance_profile_version.trim().is_empty()
        || reference.tolerance_profile_version.len() > MAX_TEXT_LEN
        || reference.metric_order.is_empty()
        || reference.metric_order.len() > MAX_ITEMS
        || reference
            .metric_order
            .windows(2)
            .any(|pair| pair[0].metric >= pair[1].metric)
        || reference.metric_order.iter().any(|metric| {
            metric.metric.trim().is_empty()
                || metric.metric.len() > MAX_TEXT_LEN
                || metric.max_relative_milli > 1_000_000
        })
        || !unique_bounded(&reference.required_site_order)
        || !canonical(&reference.required_site_order)
        || reference.generated_tick == 0
        || request.attestations.len() > MAX_ITEMS
        || !unique_bounded(&request.policy.allowed_site_order)
        || !canonical(&request.policy.allowed_site_order)
        || request.policy.federation_policy_version.trim().is_empty()
        || request.policy.federation_policy_version.len() > MAX_TEXT_LEN
        || request.policy.max_age_ticks == 0
        || request.policy.expires_at_tick == 0
        || request.current_tick > request.policy.expires_at_tick
    {
        return Err(FederatedReplayConformanceError::InvalidRequest(
            "versioned workflow/tolerance identity, canonical metrics and sites, live policy, and bounded attestations are required".into(),
        ));
    }
    for attestation in &request.attestations {
        if attestation.site_id.trim().is_empty()
            || attestation.site_id.len() > MAX_TEXT_LEN
            || !valid_hash(&attestation.workflow_digest)
            || !valid_hash(&attestation.run_digest)
            || attestation.generated_tick == 0
            || attestation
                .metric_order
                .windows(2)
                .any(|pair| pair[0].0 >= pair[1].0)
            || attestation
                .metric_order
                .iter()
                .any(|(metric, _)| metric.trim().is_empty() || metric.len() > MAX_TEXT_LEN)
            || !valid_hash(&attestation.attestation_digest)
        {
            return Err(FederatedReplayConformanceError::InvalidRequest(
                "site attestations require bounded canonical metric identities and content digests"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&attestation_body(attestation))
            .map_err(|error| FederatedReplayConformanceError::Digest(error.to_string()))?;
        if expected != attestation.attestation_digest {
            return Err(FederatedReplayConformanceError::InvalidRequest(
                "site attestation digest does not verify".into(),
            ));
        }
    }
    Ok(())
}

impl FederatedReplayConformanceReport {
    pub fn validate(&self) -> Result<(), FederatedReplayConformanceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.reference_id.trim().is_empty()
            || !valid_hash(&self.workflow_digest)
            || self.tolerance_profile_version.trim().is_empty()
            || self
                .site_result_order
                .windows(2)
                .any(|pair| pair[0].site_id >= pair[1].site_id)
            || !unique_bounded(&self.missing_site_order)
            || !canonical(&self.missing_site_order)
            || !unique_bounded(&self.negative_evidence)
            || !canonical(&self.negative_evidence)
            || !unique_bounded(&self.uncertainty)
            || !canonical(&self.uncertainty)
            || !valid_hash(&self.digest)
        {
            return Err(FederatedReplayConformanceError::InvalidOutput(
                "report identity, ordered site results, coverage gaps, evidence, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedReplayConformanceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedReplayConformanceError::InvalidOutput(
                "conformance report digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Verify site-local replay summaries against a shared, versioned metric/tolerance contract.
pub fn verify_glioma_federated_replay_conformance(
    request: &FederatedReplayConformanceRequest,
) -> Result<FederatedReplayConformanceReport, FederatedReplayConformanceError> {
    validate_request(request)?;
    let allowed = request
        .policy
        .allowed_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required = request
        .reference
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let tolerance = request
        .reference
        .metric_order
        .iter()
        .map(|metric| (metric.metric.clone(), metric))
        .collect::<BTreeMap<_, _>>();
    let mut site_results = Vec::new();
    for attestation in &request.attestations {
        if !seen.insert(attestation.site_id.clone()) {
            return Err(FederatedReplayConformanceError::InvalidRequest(
                "duplicate site attestations are ambiguous".into(),
            ));
        }
        let mut reasons = Vec::new();
        let mut deviations = Vec::new();
        let mut status = ReplaySiteConformance::Pass;
        if !allowed.contains(&attestation.site_id) || attestation.revoked {
            status = ReplaySiteConformance::Rejected;
            reasons.push("site-not-permitted-or-revoked".into());
        } else if attestation.workflow_digest != request.reference.workflow_digest {
            status = ReplaySiteConformance::Tampered;
            reasons.push("workflow-digest-mismatch".into());
        } else if request.policy.require_signed_attestations && !attestation.signed {
            status = ReplaySiteConformance::Rejected;
            reasons.push("signature-required-by-policy".into());
        } else if request.policy.require_aggregate_only && !attestation.aggregate_only {
            status = ReplaySiteConformance::Rejected;
            reasons.push("aggregate-only-attestation-required".into());
        } else if request
            .current_tick
            .saturating_sub(attestation.generated_tick)
            > request.policy.max_age_ticks
        {
            status = ReplaySiteConformance::Stale;
            reasons.push("attestation-exceeds-freshness-window".into());
        } else {
            let observed = attestation
                .metric_order
                .iter()
                .cloned()
                .collect::<BTreeMap<_, _>>();
            for (name, expected) in &tolerance {
                let value = observed.get(name).copied();
                let (absolute, relative, within) = if let Some(value) = value {
                    let absolute = absolute_error(expected.reference_milli, value);
                    let relative = relative_error_milli(expected.reference_milli, absolute);
                    (
                        Some(absolute),
                        Some(relative),
                        absolute <= expected.max_absolute_milli
                            && relative <= expected.max_relative_milli,
                    )
                } else {
                    (None, None, false)
                };
                if !within {
                    status = ReplaySiteConformance::MetricFailure;
                }
                deviations.push(ReplayMetricDeviation {
                    metric: name.clone(),
                    reference_milli: expected.reference_milli,
                    observed_milli: value,
                    absolute_error_milli: absolute,
                    relative_error_milli: relative,
                    within_tolerance: within,
                });
            }
            if status == ReplaySiteConformance::MetricFailure {
                reasons.push("one-or-more-metrics-outside-versioned-tolerance".into());
            }
        }
        reasons.sort();
        site_results.push(ReplaySiteConformanceResult {
            site_id: attestation.site_id.clone(),
            conformance: status,
            run_digest: Some(attestation.run_digest.clone()),
            metric_deviation_order: deviations,
            reason_order: reasons,
        });
    }
    for site in required.difference(&seen) {
        site_results.push(ReplaySiteConformanceResult {
            site_id: site.clone(),
            conformance: ReplaySiteConformance::Missing,
            run_digest: None,
            metric_deviation_order: Vec::new(),
            reason_order: vec!["required-site-attestation-missing".into()],
        });
    }
    site_results.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let missing = site_results
        .iter()
        .filter(|result| result.conformance == ReplaySiteConformance::Missing)
        .map(|result| result.site_id.clone())
        .collect::<Vec<_>>();
    let pass_count = site_results
        .iter()
        .filter(|result| result.conformance == ReplaySiteConformance::Pass)
        .count();
    let failure_count = site_results.len().saturating_sub(pass_count);
    let portability_claim = !site_results.is_empty() && failure_count == 0 && missing.is_empty();
    let evidence_strength = if portability_claim && pass_count >= 3 {
        ReplayEvidenceStrength::Strong
    } else if portability_claim {
        ReplayEvidenceStrength::Moderate
    } else if pass_count > 0 {
        ReplayEvidenceStrength::Weak
    } else {
        ReplayEvidenceStrength::None
    };
    let disposition = if portability_claim {
        FederatedReplayConformanceDisposition::Conformant
    } else if site_results.is_empty() || !missing.is_empty() {
        FederatedReplayConformanceDisposition::Unresolved
    } else if pass_count > 0 {
        FederatedReplayConformanceDisposition::Partial
    } else {
        FederatedReplayConformanceDisposition::Blocked
    };
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !missing.is_empty() {
        negative_evidence.push("required-site-attestation-missing".into());
        uncertainty.push("missing-sites-prevent-federated-portability-claim".into());
    }
    if failure_count > 0 {
        negative_evidence.push("one-or-more-sites-failed-replay-conformance".into());
    }
    if request.attestations.is_empty() {
        uncertainty.push("no-site-attestations-were-provided".into());
    }
    negative_evidence.sort();
    uncertainty.sort();
    let mut report = FederatedReplayConformanceReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        reference_id: request.reference.reference_id.clone(),
        workflow_digest: request.reference.workflow_digest.clone(),
        tolerance_profile_version: request.reference.tolerance_profile_version.clone(),
        site_result_order: site_results,
        missing_site_order: missing,
        portability_claim,
        evidence_strength,
        disposition,
        negative_evidence,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-replay-conformance"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| FederatedReplayConformanceError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn reference() -> ReplayConformanceReference {
        ReplayConformanceReference {
            reference_id: "ref-1".into(),
            workflow_digest: hash("workflow"),
            tolerance_profile_version: "tol-1".into(),
            metric_order: vec![ReplayMetricTolerance {
                metric: "replay_rate".into(),
                reference_milli: 950,
                max_absolute_milli: 20,
                max_relative_milli: 30,
            }],
            required_site_order: vec!["site-a".into(), "site-b".into()],
            generated_tick: 1,
        }
    }

    fn attestation(site_id: &str, value: i64) -> ReplaySiteAttestation {
        let mut attestation = ReplaySiteAttestation {
            site_id: site_id.into(),
            workflow_digest: hash("workflow"),
            run_digest: hash(&format!("run-{site_id}")),
            metric_order: vec![("replay_rate".into(), value)],
            generated_tick: 2,
            aggregate_only: true,
            signed: true,
            revoked: false,
            attestation_digest: hash("placeholder"),
        };
        attestation.attestation_digest =
            ContentHash::of_value(&attestation_body(&attestation)).expect("attestation");
        attestation
    }

    fn request(attestations: Vec<ReplaySiteAttestation>) -> FederatedReplayConformanceRequest {
        FederatedReplayConformanceRequest {
            reference: reference(),
            attestations,
            policy: ReplayConformancePolicy {
                allowed_site_order: vec!["site-a".into(), "site-b".into(), "site-c".into()],
                federation_policy_version: "policy-1".into(),
                max_age_ticks: 10,
                require_signed_attestations: true,
                require_aggregate_only: true,
                expires_at_tick: 100,
            },
            current_tick: 3,
        }
    }

    #[test]
    fn all_required_sites_within_tolerance_are_conformant() {
        let report = verify_glioma_federated_replay_conformance(&request(vec![
            attestation("site-a", 950),
            attestation("site-b", 960),
        ]))
        .expect("report");
        assert_eq!(
            report.disposition,
            FederatedReplayConformanceDisposition::Conformant
        );
        assert!(report.portability_claim);
        assert_eq!(report.evidence_strength, ReplayEvidenceStrength::Moderate);
    }

    #[test]
    fn missing_required_site_never_passes() {
        let report =
            verify_glioma_federated_replay_conformance(&request(vec![attestation("site-a", 950)]))
                .expect("report");
        assert_eq!(
            report.disposition,
            FederatedReplayConformanceDisposition::Unresolved
        );
        assert!(!report.portability_claim);
        assert_eq!(report.missing_site_order, vec!["site-b"]);
    }

    #[test]
    fn tolerance_failure_is_partial_and_retains_metric_deviation() {
        let report = verify_glioma_federated_replay_conformance(&request(vec![
            attestation("site-a", 950),
            attestation("site-b", 1_200),
        ]))
        .expect("report");
        assert_eq!(
            report.disposition,
            FederatedReplayConformanceDisposition::Partial
        );
        assert!(!report.portability_claim);
        let site_b = report
            .site_result_order
            .iter()
            .find(|site| site.site_id == "site-b")
            .expect("site b");
        assert_eq!(site_b.conformance, ReplaySiteConformance::MetricFailure);
        assert!(!site_b.metric_deviation_order[0].within_tolerance);
    }

    #[test]
    fn tampered_attestation_digest_fails_closed() {
        let mut item = attestation("site-a", 950);
        item.metric_order[0].1 = 951;
        assert!(matches!(
            verify_glioma_federated_replay_conformance(&request(vec![item])),
            Err(FederatedReplayConformanceError::InvalidRequest(_))
        ));
    }
}
