//! Conservative quorum admission for aggregate-only federated glioma benchmarks.
//!
//! This gate is deliberately independent from query execution. It verifies the typed claims a
//! site contributes before a benchmark can be admitted: signer identity, policy scope, schema and
//! benchmark conformance, freshness, privacy-safe locality, duplication, and independent-group
//! coverage. Correlated sites remain visible but never inflate quorum. The output contains
//! metadata and digests only; it does not contact sites, move raw data, or make a clinical decision.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaQuorumAdmissionDecision1@1";
pub const MAX_CONTRIBUTIONS: usize = 512;
pub const MAX_MODELS: usize = 64;
pub const MAX_REASON_LENGTH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuorumAdmissionDisposition {
    Admit,
    UnderQuorum,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuorumContributionStatus {
    Admitted,
    Correlated,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuorumExclusionReason {
    DuplicateContribution,
    DuplicateSiteArtifact,
    MissingSigner,
    InvalidSignature,
    RevokedSite,
    ApprovalMissing,
    PolicyScopeMismatch,
    SchemaMismatch,
    BenchmarkMismatch,
    ModelMismatch,
    AssayMismatch,
    StaleContribution,
    FutureContribution,
    PrivacyUnsafe,
    HumanDataDeclared,
    DirectIdentifiersDeclared,
    EmptyAggregate,
    CorrelatedSite,
    InvalidIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedQuorumContribution {
    pub contribution_id: String,
    pub site_id: String,
    pub study_id: String,
    pub independence_group: String,
    pub signer_id: Option<String>,
    pub signature_digest: Option<ContentHash>,
    pub policy_scope: String,
    pub schema_version: String,
    pub benchmark_world: String,
    pub metric_name: String,
    pub model_system: String,
    pub assay: String,
    pub artifact_digest: ContentHash,
    pub observed_tick: u64,
    pub aggregate_count: u32,
    pub privacy_cost_milli: u32,
    pub approved: bool,
    pub revoked: bool,
    pub signature_valid: bool,
    pub aggregate_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumAdmissionRequest {
    pub benchmark_id: String,
    pub benchmark_world: String,
    pub schema_version: String,
    pub metric_name: String,
    pub required_policy_scope: String,
    pub required_assay: String,
    pub required_model_order: Vec<String>,
    pub current_tick: u64,
    pub max_staleness_ticks: u64,
    pub minimum_independent_sites: usize,
    pub privacy_budget_milli: u64,
    pub require_signatures: bool,
    pub contributions: Vec<FederatedQuorumContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumContributionReview {
    pub contribution_id: String,
    pub site_id: String,
    pub independence_group: String,
    pub status: QuorumContributionStatus,
    pub exclusion_reason: Option<QuorumExclusionReason>,
    pub observed_tick: u64,
    pub privacy_cost_milli: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumAdmissionDecision {
    pub feature_id: String,
    pub output_schema: String,
    pub benchmark_id: String,
    pub eligible_contribution_order: Vec<String>,
    pub quorum_contribution_order: Vec<String>,
    pub correlated_contribution_order: Vec<String>,
    pub excluded_contribution_order: Vec<String>,
    pub reviews: Vec<QuorumContributionReview>,
    pub independent_group_order: Vec<String>,
    pub model_coverage_order: Vec<String>,
    pub required_model_order: Vec<String>,
    pub independent_site_count: usize,
    pub required_independent_sites: usize,
    pub aggregate_count: u64,
    pub privacy_spend_milli: u64,
    pub privacy_budget_milli: u64,
    pub blockers: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: QuorumAdmissionDisposition,
    pub query_admission_permitted: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuorumAdmissionError {
    #[error("quorum admission request is invalid: {0}")]
    InvalidRequest(String),
    #[error("quorum admission output is invalid: {0}")]
    InvalidOutput(String),
    #[error("quorum admission digest failed: {0}")]
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

fn digest_body(decision: &QuorumAdmissionDecision) -> serde_json::Value {
    serde_json::json!({
        "feature_id": decision.feature_id,
        "output_schema": decision.output_schema,
        "benchmark_id": decision.benchmark_id,
        "eligible_contribution_order": decision.eligible_contribution_order,
        "quorum_contribution_order": decision.quorum_contribution_order,
        "correlated_contribution_order": decision.correlated_contribution_order,
        "excluded_contribution_order": decision.excluded_contribution_order,
        "reviews": decision.reviews,
        "independent_group_order": decision.independent_group_order,
        "model_coverage_order": decision.model_coverage_order,
        "required_model_order": decision.required_model_order,
        "independent_site_count": decision.independent_site_count,
        "required_independent_sites": decision.required_independent_sites,
        "aggregate_count": decision.aggregate_count,
        "privacy_spend_milli": decision.privacy_spend_milli,
        "privacy_budget_milli": decision.privacy_budget_milli,
        "blockers": decision.blockers,
        "uncertainty": decision.uncertainty,
        "disposition": decision.disposition,
        "query_admission_permitted": decision.query_admission_permitted,
    })
}

fn validate_request(request: &QuorumAdmissionRequest) -> Result<(), QuorumAdmissionError> {
    if !safe_text(&request.benchmark_id)
        || !safe_text(&request.benchmark_world)
        || !safe_text(&request.schema_version)
        || !safe_text(&request.metric_name)
        || !safe_text(&request.required_policy_scope)
        || !safe_text(&request.required_assay)
        || request.required_model_order.is_empty()
        || request.required_model_order.len() > MAX_MODELS
        || !canonical(&request.required_model_order)
        || request.max_staleness_ticks == 0
        || request.minimum_independent_sites == 0
        || request.privacy_budget_milli == 0
        || request.contributions.is_empty()
        || request.contributions.len() > MAX_CONTRIBUTIONS
    {
        return Err(QuorumAdmissionError::InvalidRequest(
            "bounded benchmark identity, canonical model coverage, freshness, quorum, privacy, and contribution inputs are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for contribution in &request.contributions {
        if !safe_text(&contribution.contribution_id)
            || !safe_text(&contribution.site_id)
            || !safe_text(&contribution.study_id)
            || !safe_text(&contribution.independence_group)
            || !safe_text(&contribution.policy_scope)
            || !safe_text(&contribution.schema_version)
            || !safe_text(&contribution.benchmark_world)
            || !safe_text(&contribution.metric_name)
            || !safe_text(&contribution.model_system)
            || !safe_text(&contribution.assay)
            || !ids.insert(contribution.contribution_id.clone())
            || contribution.artifact_digest.as_str().len() != 64
            || contribution.aggregate_count == 0
            || contribution.privacy_cost_milli > 1_000_000
        {
            return Err(QuorumAdmissionError::InvalidRequest(format!(
                "contribution {} has invalid identity, digest, aggregate, privacy, or duplicate fields",
                contribution.contribution_id
            )));
        }
    }
    Ok(())
}

impl QuorumAdmissionDecision {
    pub fn validate(&self) -> Result<(), QuorumAdmissionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.benchmark_id)
            || !canonical(&self.eligible_contribution_order)
            || !canonical(&self.quorum_contribution_order)
            || !canonical(&self.correlated_contribution_order)
            || !canonical(&self.excluded_contribution_order)
            || !canonical(&self.independent_group_order)
            || !canonical(&self.model_coverage_order)
            || !canonical(&self.required_model_order)
            || !canonical(&self.blockers)
            || !canonical(&self.uncertainty)
            || self.independent_site_count != self.independent_group_order.len()
            || self.independent_site_count > self.eligible_contribution_order.len()
            || self.required_independent_sites == 0
            || self.privacy_budget_milli == 0
            || self.privacy_spend_milli > self.privacy_budget_milli
            || self.digest.as_str().len() != 64
            || self.reviews.iter().any(|review| {
                !safe_text(&review.contribution_id)
                    || !safe_text(&review.site_id)
                    || !safe_text(&review.independence_group)
                    || review.exclusion_reason.is_none()
                        && review.status == QuorumContributionStatus::Excluded
            })
        {
            return Err(QuorumAdmissionError::InvalidOutput(
                "quorum identity, canonical ordering, coverage, privacy, review, or digest invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| QuorumAdmissionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(QuorumAdmissionError::InvalidOutput(
                "quorum admission digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Verify whether aggregate-only site contributions can admit a federated benchmark query.
pub fn assess_glioma_quorum_admission(
    request: &QuorumAdmissionRequest,
) -> Result<QuorumAdmissionDecision, QuorumAdmissionError> {
    validate_request(request)?;
    let mut contributions = request.contributions.clone();
    contributions.sort_by(|left, right| left.contribution_id.cmp(&right.contribution_id));
    let mut reviews = Vec::with_capacity(contributions.len());
    let mut eligible = BTreeSet::new();
    let mut excluded = BTreeSet::new();
    let mut candidate_groups: BTreeMap<String, String> = BTreeMap::new();
    let mut contribution_keys = BTreeSet::new();
    let mut privacy_spend = 0_u64;
    let mut aggregate_count = 0_u64;
    let mut model_coverage = BTreeSet::new();

    for contribution in &contributions {
        let mut reason = None;
        if !contribution.approved {
            reason = Some(QuorumExclusionReason::ApprovalMissing);
        } else if contribution.revoked {
            reason = Some(QuorumExclusionReason::RevokedSite);
        } else if request.require_signatures
            && (contribution
                .signer_id
                .as_deref()
                .is_none_or(|id| !safe_text(id)))
        {
            reason = Some(QuorumExclusionReason::MissingSigner);
        } else if request.require_signatures && !contribution.signature_valid {
            reason = Some(QuorumExclusionReason::InvalidSignature);
        } else if contribution.policy_scope != request.required_policy_scope {
            reason = Some(QuorumExclusionReason::PolicyScopeMismatch);
        } else if contribution.schema_version != request.schema_version {
            reason = Some(QuorumExclusionReason::SchemaMismatch);
        } else if contribution.benchmark_world != request.benchmark_world
            || contribution.metric_name != request.metric_name
        {
            reason = Some(QuorumExclusionReason::BenchmarkMismatch);
        } else if !request
            .required_model_order
            .binary_search(&contribution.model_system)
            .is_ok()
        {
            reason = Some(QuorumExclusionReason::ModelMismatch);
        } else if contribution.assay != request.required_assay {
            reason = Some(QuorumExclusionReason::AssayMismatch);
        } else if contribution.observed_tick > request.current_tick {
            reason = Some(QuorumExclusionReason::FutureContribution);
        } else if request.current_tick - contribution.observed_tick > request.max_staleness_ticks {
            reason = Some(QuorumExclusionReason::StaleContribution);
        } else if !contribution.aggregate_only {
            reason = Some(QuorumExclusionReason::PrivacyUnsafe);
        } else if contribution.contains_human_data {
            reason = Some(QuorumExclusionReason::HumanDataDeclared);
        } else if contribution.contains_direct_identifiers {
            reason = Some(QuorumExclusionReason::DirectIdentifiersDeclared);
        } else if contribution.aggregate_count == 0 {
            reason = Some(QuorumExclusionReason::EmptyAggregate);
        } else if !contribution_keys.insert((
            contribution.site_id.clone(),
            contribution.study_id.clone(),
            contribution.artifact_digest.clone(),
        )) {
            reason = Some(QuorumExclusionReason::DuplicateSiteArtifact);
        } else if !eligible.is_empty() && eligible.contains(&contribution.contribution_id) {
            reason = Some(QuorumExclusionReason::DuplicateContribution);
        }

        if let Some(exclusion_reason) = reason {
            excluded.insert(contribution.contribution_id.clone());
            reviews.push(QuorumContributionReview {
                contribution_id: contribution.contribution_id.clone(),
                site_id: contribution.site_id.clone(),
                independence_group: contribution.independence_group.clone(),
                status: QuorumContributionStatus::Excluded,
                exclusion_reason: Some(exclusion_reason),
                observed_tick: contribution.observed_tick,
                privacy_cost_milli: contribution.privacy_cost_milli,
            });
            continue;
        }
        if privacy_spend.saturating_add(u64::from(contribution.privacy_cost_milli))
            > request.privacy_budget_milli
        {
            excluded.insert(contribution.contribution_id.clone());
            reviews.push(QuorumContributionReview {
                contribution_id: contribution.contribution_id.clone(),
                site_id: contribution.site_id.clone(),
                independence_group: contribution.independence_group.clone(),
                status: QuorumContributionStatus::Excluded,
                exclusion_reason: Some(QuorumExclusionReason::PrivacyUnsafe),
                observed_tick: contribution.observed_tick,
                privacy_cost_milli: contribution.privacy_cost_milli,
            });
            continue;
        }
        eligible.insert(contribution.contribution_id.clone());
        privacy_spend += u64::from(contribution.privacy_cost_milli);
        aggregate_count += u64::from(contribution.aggregate_count);
        model_coverage.insert(contribution.model_system.clone());
        if candidate_groups.contains_key(&contribution.independence_group) {
            reviews.push(QuorumContributionReview {
                contribution_id: contribution.contribution_id.clone(),
                site_id: contribution.site_id.clone(),
                independence_group: contribution.independence_group.clone(),
                status: QuorumContributionStatus::Correlated,
                exclusion_reason: Some(QuorumExclusionReason::CorrelatedSite),
                observed_tick: contribution.observed_tick,
                privacy_cost_milli: contribution.privacy_cost_milli,
            });
        } else {
            candidate_groups.insert(
                contribution.independence_group.clone(),
                contribution.contribution_id.clone(),
            );
            reviews.push(QuorumContributionReview {
                contribution_id: contribution.contribution_id.clone(),
                site_id: contribution.site_id.clone(),
                independence_group: contribution.independence_group.clone(),
                status: QuorumContributionStatus::Admitted,
                exclusion_reason: None,
                observed_tick: contribution.observed_tick,
                privacy_cost_milli: contribution.privacy_cost_milli,
            });
        }
    }

    let eligible_order = eligible.into_iter().collect::<Vec<_>>();
    let excluded_order = excluded.into_iter().collect::<Vec<_>>();
    let quorum_order = reviews
        .iter()
        .filter(|review| review.status == QuorumContributionStatus::Admitted)
        .map(|review| review.contribution_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let correlated_order = reviews
        .iter()
        .filter(|review| review.status == QuorumContributionStatus::Correlated)
        .map(|review| review.contribution_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let independent_group_order = candidate_groups.keys().cloned().collect::<Vec<_>>();
    let required_model_missing = request
        .required_model_order
        .iter()
        .filter(|model| !model_coverage.contains(*model))
        .cloned()
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    if independent_group_order.len() < request.minimum_independent_sites {
        blockers.push(format!(
            "independent quorum {} is below required {}",
            independent_group_order.len(),
            request.minimum_independent_sites
        ));
    }
    if !required_model_missing.is_empty() {
        blockers.push(format!(
            "required model coverage is missing: {}",
            required_model_missing.join(",")
        ));
    }
    if eligible_order.is_empty() {
        blockers.push("no policy-valid aggregate contribution remains".into());
    }
    let mut uncertainty = Vec::new();
    if !correlated_order.is_empty() {
        uncertainty.push(format!(
            "{} correlated contribution(s) remain visible but do not count toward quorum",
            correlated_order.len()
        ));
    }
    if !excluded_order.is_empty() {
        uncertainty.push(format!(
            "{} contribution(s) were excluded with stable reasons",
            excluded_order.len()
        ));
    }
    blockers.sort();
    uncertainty.sort();
    let disposition = if blockers.is_empty() {
        QuorumAdmissionDisposition::Admit
    } else if !eligible_order.is_empty() {
        QuorumAdmissionDisposition::UnderQuorum
    } else {
        QuorumAdmissionDisposition::Blocked
    };
    let mut decision = QuorumAdmissionDecision {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        benchmark_id: request.benchmark_id.clone(),
        eligible_contribution_order: eligible_order,
        quorum_contribution_order: quorum_order,
        correlated_contribution_order: correlated_order,
        excluded_contribution_order: excluded_order,
        reviews,
        independent_group_order,
        model_coverage_order: model_coverage.into_iter().collect(),
        required_model_order: request.required_model_order.clone(),
        independent_site_count: candidate_groups.len(),
        required_independent_sites: request.minimum_independent_sites,
        aggregate_count,
        privacy_spend_milli: privacy_spend,
        privacy_budget_milli: request.privacy_budget_milli,
        blockers,
        uncertainty,
        disposition,
        query_admission_permitted: disposition == QuorumAdmissionDisposition::Admit,
        digest: ContentHash::of_bytes(b"unsealed-glioma-quorum-admission"),
    };
    decision.digest = ContentHash::of_value(&digest_body(&decision))
        .map_err(|error| QuorumAdmissionError::Digest(error.to_string()))?;
    decision.validate()?;
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn contribution(id: &str, site: &str, group: &str, model: &str) -> FederatedQuorumContribution {
        FederatedQuorumContribution {
            contribution_id: id.into(),
            site_id: site.into(),
            study_id: format!("study-{site}"),
            independence_group: group.into(),
            signer_id: Some(format!("signer-{site}")),
            signature_digest: Some(digest(&format!("signature-{id}"))),
            policy_scope: "glioma-benchmark-v1".into(),
            schema_version: "aggregate-v1".into(),
            benchmark_world: "invasion-world".into(),
            metric_name: "invasion_score".into(),
            model_system: model.into(),
            assay: "organoid-imaging".into(),
            artifact_digest: digest(&format!("artifact-{id}")),
            observed_tick: 95,
            aggregate_count: 12,
            privacy_cost_milli: 50,
            approved: true,
            revoked: false,
            signature_valid: true,
            aggregate_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request(contributions: Vec<FederatedQuorumContribution>) -> QuorumAdmissionRequest {
        QuorumAdmissionRequest {
            benchmark_id: "benchmark-1".into(),
            benchmark_world: "invasion-world".into(),
            schema_version: "aggregate-v1".into(),
            metric_name: "invasion_score".into(),
            required_policy_scope: "glioma-benchmark-v1".into(),
            required_assay: "organoid-imaging".into(),
            required_model_order: vec!["organoid".into(), "xenograft".into()],
            current_tick: 100,
            max_staleness_ticks: 10,
            minimum_independent_sites: 2,
            privacy_budget_milli: 500,
            require_signatures: true,
            contributions,
        }
    }

    #[test]
    fn independent_model_complete_contributions_are_admitted() {
        let decision = assess_glioma_quorum_admission(&request(vec![
            contribution("a", "site-a", "group-a", "organoid"),
            contribution("b", "site-b", "group-b", "xenograft"),
        ]))
        .unwrap();
        assert_eq!(decision.disposition, QuorumAdmissionDisposition::Admit);
        assert!(decision.query_admission_permitted);
        assert_eq!(decision.independent_site_count, 2);
        assert_eq!(decision.quorum_contribution_order, vec!["a", "b"]);
    }

    #[test]
    fn correlated_sites_do_not_inflate_quorum() {
        let decision = assess_glioma_quorum_admission(&request(vec![
            contribution("a", "site-a", "group-a", "organoid"),
            contribution("b", "site-b", "group-a", "xenograft"),
        ]))
        .unwrap();
        assert_eq!(
            decision.disposition,
            QuorumAdmissionDisposition::UnderQuorum
        );
        assert!(!decision.query_admission_permitted);
        assert_eq!(decision.correlated_contribution_order, vec!["b"]);
        assert_eq!(decision.independent_site_count, 1);
    }

    #[test]
    fn stale_revoked_and_schema_invalid_inputs_are_excluded_with_reasons() {
        let mut stale = contribution("a", "site-a", "group-a", "organoid");
        stale.observed_tick = 70;
        let mut revoked = contribution("b", "site-b", "group-b", "xenograft");
        revoked.revoked = true;
        let mut schema = contribution("c", "site-c", "group-c", "organoid");
        schema.schema_version = "old".into();
        let decision =
            assess_glioma_quorum_admission(&request(vec![stale, revoked, schema])).unwrap();
        assert_eq!(decision.disposition, QuorumAdmissionDisposition::Blocked);
        assert_eq!(decision.excluded_contribution_order, vec!["a", "b", "c"]);
        assert!(decision.reviews.iter().any(|review| {
            review.exclusion_reason == Some(QuorumExclusionReason::StaleContribution)
        }));
    }

    #[test]
    fn privacy_budget_and_signature_gates_never_admit_unsafe_inputs() {
        let mut unsigned = contribution("a", "site-a", "group-a", "organoid");
        unsigned.signature_valid = false;
        let mut unsafe_data = contribution("b", "site-b", "group-b", "xenograft");
        unsafe_data.contains_direct_identifiers = true;
        let mut req = request(vec![unsigned, unsafe_data]);
        req.privacy_budget_milli = 40;
        let decision = assess_glioma_quorum_admission(&req).unwrap();
        assert_eq!(decision.disposition, QuorumAdmissionDisposition::Blocked);
        assert!(!decision.query_admission_permitted);
        assert!(decision
            .reviews
            .iter()
            .all(|review| { review.status == QuorumContributionStatus::Excluded }));
    }

    #[test]
    fn identical_replay_is_content_deterministic() {
        let req = request(vec![
            contribution("b", "site-b", "group-b", "xenograft"),
            contribution("a", "site-a", "group-a", "organoid"),
        ]);
        let first = assess_glioma_quorum_admission(&req).unwrap();
        let second = assess_glioma_quorum_admission(&req).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}
