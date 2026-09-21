//! Aggregate-only federated typed-knowledge consensus for preclinical glioma research.
//!
//! Independent sites may compile the same evidence locally while retaining source text and
//! specimen data. This capability compares their typed claim summaries, quantifies disposition
//! consensus, disagreement, and leave-one-site-out influence, and emits bounded promotion or
//! adjudication actions. It is a knowledge-governance algorithm, not a source transport layer and
//! not a causal or clinical decision engine.

use super::knowledge_graph::KnowledgeClaimDisposition;
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedKnowledge1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledgeSiteClaim {
    pub site_id: String,
    pub study_id: String,
    pub claim_id: String,
    pub statement: String,
    pub scope: String,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub confidence_milli: u16,
    pub disposition: KnowledgeClaimDisposition,
    pub source_count: u16,
    pub artifact: LocalArtifactRef,
    pub preclinical_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledgeRequest {
    pub objective: String,
    pub snapshot_id: String,
    pub minimum_sites: usize,
    pub minimum_consensus_milli: u16,
    pub minimum_confidence_milli: u16,
    pub max_disagreement_milli: u16,
    pub max_leave_one_out_shift_milli: u16,
    pub max_actions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedKnowledgeKind {
    ConsensusSupported,
    ConsensusContested,
    ConsensusNegative,
    SiteSpecific,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledgeAction {
    pub action_id: String,
    pub claim_id: String,
    pub kind: FederatedKnowledgeKind,
    pub site_order: Vec<String>,
    pub pooled_support_milli: u16,
    pub pooled_contradiction_milli: u16,
    pub pooled_confidence_milli: u16,
    pub consensus_milli: u16,
    pub disagreement_milli: u16,
    pub max_leave_one_out_shift_milli: u16,
    pub priority_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedKnowledgeDisposition {
    Qualified,
    Underpowered,
    Disagreement,
    InfluenceBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedKnowledge {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub snapshot_id: String,
    pub claim_order: Vec<String>,
    pub action_order: Vec<String>,
    pub actions: Vec<FederatedKnowledgeAction>,
    pub unresolved_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedKnowledgeDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedKnowledgeError {
    #[error("federated knowledge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated knowledge site claim is invalid: {0}")]
    InvalidSite(String),
    #[error("federated knowledge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated knowledge digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone, Copy)]
struct PooledClaim {
    support_milli: u16,
    contradiction_milli: u16,
    confidence_milli: u16,
    consensus_milli: u16,
    disagreement_milli: u16,
    max_leave_one_out_shift_milli: u16,
    dominant_disposition: KnowledgeClaimDisposition,
    site_count: usize,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn weight(site: &FederatedKnowledgeSiteClaim) -> u128 {
    u128::from(site.source_count.max(1))
        .saturating_mul(u128::from(site.confidence_milli.max(1)))
        .saturating_mul(1_000)
}

fn weighted_mean(values: impl Iterator<Item = (u16, u128)>) -> u16 {
    let values = values.collect::<Vec<_>>();
    let total = values.iter().map(|(_, weight)| *weight).sum::<u128>();
    if total == 0 {
        return 0;
    }
    (values
        .iter()
        .map(|(value, weight)| u128::from(*value).saturating_mul(*weight))
        .sum::<u128>()
        / total)
        .min(1_000) as u16
}

fn pooled_confidence(sites: &[&FederatedKnowledgeSiteClaim]) -> u16 {
    weighted_mean(
        sites
            .iter()
            .map(|site| (site.confidence_milli, weight(site))),
    )
}

fn pooled_without(sites: &[&FederatedKnowledgeSiteClaim]) -> u16 {
    pooled_confidence(sites)
}

fn pooled_claim(sites: &[&FederatedKnowledgeSiteClaim]) -> PooledClaim {
    if sites.is_empty() {
        return PooledClaim {
            support_milli: 0,
            contradiction_milli: 0,
            confidence_milli: 0,
            consensus_milli: 0,
            disagreement_milli: 1_000,
            max_leave_one_out_shift_milli: 0,
            dominant_disposition: KnowledgeClaimDisposition::Unresolved,
            site_count: 0,
        };
    }
    let support_milli = weighted_mean(sites.iter().map(|site| (site.support_milli, weight(site))));
    let contradiction_milli = weighted_mean(
        sites
            .iter()
            .map(|site| (site.contradiction_milli, weight(site))),
    );
    let confidence_milli = pooled_confidence(sites);
    let mut disposition_mass = BTreeMap::<u8, (KnowledgeClaimDisposition, u128)>::new();
    for site in sites {
        let code = match site.disposition {
            KnowledgeClaimDisposition::Supported => 0,
            KnowledgeClaimDisposition::Contested => 1,
            KnowledgeClaimDisposition::Negative => 2,
            KnowledgeClaimDisposition::Unresolved => 3,
        };
        let entry = disposition_mass
            .entry(code)
            .or_insert((site.disposition, 0));
        entry.1 += weight(site);
    }
    let (dominant_disposition, dominant_mass) = disposition_mass
        .iter()
        .max_by(|left, right| left.1 .1.cmp(&right.1 .1).then_with(|| right.0.cmp(left.0)))
        .map(|(_, (disposition, mass))| (*disposition, *mass))
        .unwrap_or((KnowledgeClaimDisposition::Unresolved, 0));
    let total_mass = sites.iter().map(|site| weight(site)).sum::<u128>();
    let consensus_milli = if total_mass == 0 {
        0
    } else {
        (dominant_mass.saturating_mul(1_000) / total_mass).min(1_000) as u16
    };
    let max_deviation = sites
        .iter()
        .map(|site| site.confidence_milli.abs_diff(confidence_milli))
        .max()
        .unwrap_or(0);
    let disagreement_milli = max_deviation
        .saturating_mul(1_000)
        .checked_div(u16::max(confidence_milli, 1))
        .unwrap_or(1_000)
        .min(1_000);
    let max_leave_one_out_shift_milli = sites
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let without = sites
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, site)| (candidate_index != index).then_some(*site))
                .collect::<Vec<_>>();
            pooled_without(&without).abs_diff(confidence_milli)
        })
        .max()
        .unwrap_or(0);
    PooledClaim {
        support_milli,
        contradiction_milli,
        confidence_milli,
        consensus_milli,
        disagreement_milli,
        max_leave_one_out_shift_milli,
        dominant_disposition,
        site_count: sites.len(),
    }
}

fn priority(summary: PooledClaim) -> u16 {
    let pressure = match summary.dominant_disposition {
        KnowledgeClaimDisposition::Supported => summary.support_milli,
        KnowledgeClaimDisposition::Contested => summary.contradiction_milli,
        KnowledgeClaimDisposition::Negative => summary.contradiction_milli,
        KnowledgeClaimDisposition::Unresolved => 0,
    };
    (u32::from(pressure)
        .saturating_mul(u32::from(summary.consensus_milli))
        .saturating_mul(u32::from(summary.confidence_milli))
        .saturating_mul(summary.site_count.min(u16::MAX as usize) as u32)
        / 1_000_000)
        .min(1_000) as u16
}

fn digest_input(output: &FederatedKnowledge) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "snapshot_id": output.snapshot_id,
        "claim_order": output.claim_order,
        "action_order": output.action_order,
        "actions": output.actions,
        "unresolved_order": output.unresolved_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl FederatedKnowledge {
    pub fn validate(&self) -> Result<(), FederatedKnowledgeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.snapshot_id.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.actions.len() > MAX_CLAIMS
            || self.actions.iter().any(|action| {
                action.action_id != format!("federated-knowledge:{}", action.claim_id)
                    || action.claim_id.trim().is_empty()
                    || action.site_order.is_empty()
                    || !canonical(&action.site_order)
                    || action.pooled_support_milli > 1_000
                    || action.pooled_contradiction_milli > 1_000
                    || action.pooled_confidence_milli > 1_000
                    || action.consensus_milli > 1_000
                    || action.disagreement_milli > 1_000
                    || action.max_leave_one_out_shift_milli > 1_000
                    || action.priority_milli > 1_000
                    || action.rationale.trim().is_empty()
            })
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
        {
            return Err(FederatedKnowledgeError::InvalidOutput(
                "identity, ordering, bounds, or action state is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedKnowledgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedKnowledgeError::InvalidOutput(
                "federated knowledge digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn classify(summary: PooledClaim, request: &FederatedKnowledgeRequest) -> FederatedKnowledgeKind {
    if summary.site_count < request.minimum_sites
        || summary.confidence_milli < request.minimum_confidence_milli
    {
        FederatedKnowledgeKind::Unresolved
    } else if summary.consensus_milli < request.minimum_consensus_milli
        || summary.disagreement_milli > request.max_disagreement_milli
    {
        FederatedKnowledgeKind::SiteSpecific
    } else {
        match summary.dominant_disposition {
            KnowledgeClaimDisposition::Supported => FederatedKnowledgeKind::ConsensusSupported,
            KnowledgeClaimDisposition::Contested => FederatedKnowledgeKind::ConsensusContested,
            KnowledgeClaimDisposition::Negative => FederatedKnowledgeKind::ConsensusNegative,
            KnowledgeClaimDisposition::Unresolved => FederatedKnowledgeKind::Unresolved,
        }
    }
}

pub fn analyze_federated_knowledge(
    request: &FederatedKnowledgeRequest,
    sites: &[FederatedKnowledgeSiteClaim],
) -> Result<FederatedKnowledge, FederatedKnowledgeError> {
    if request.objective.trim().is_empty()
        || request.snapshot_id.trim().is_empty()
        || request.minimum_sites == 0
        || request.minimum_sites > MAX_SITES
        || request.minimum_consensus_milli > 1_000
        || request.minimum_confidence_milli > 1_000
        || request.max_disagreement_milli > 1_000
        || request.max_leave_one_out_shift_milli > 1_000
        || request.max_actions == 0
        || request.max_actions > MAX_CLAIMS
    {
        return Err(FederatedKnowledgeError::InvalidRequest(
            "objective, snapshot, site/consensus/confidence gates, and bounded action capacity are required".into(),
        ));
    }
    if sites.is_empty() || sites.len() > MAX_SITES.saturating_mul(MAX_CLAIMS) {
        return Err(FederatedKnowledgeError::InvalidSite(
            "site-claim count is outside the supported bound".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    let mut groups = BTreeMap::<String, Vec<&FederatedKnowledgeSiteClaim>>::new();
    let mut bindings = BTreeMap::<String, (String, String)>::new();
    for site in sites {
        site.artifact
            .validate()
            .map_err(|error| FederatedKnowledgeError::InvalidSite(error.to_string()))?;
        let binding = bindings
            .entry(site.claim_id.clone())
            .or_insert_with(|| (site.statement.clone(), site.scope.clone()));
        if site.site_id.trim().is_empty()
            || site.study_id.trim().is_empty()
            || site.claim_id.trim().is_empty()
            || site.statement.trim().is_empty()
            || site.scope.trim().is_empty()
            || site.support_milli > 1_000
            || site.contradiction_milli > 1_000
            || site.confidence_milli > 1_000
            || site.source_count == 0
            || !site.preclinical_only
            || site.artifact.contains_human_data
            || site.artifact.contains_direct_identifiers
            || binding != &(site.statement.clone(), site.scope.clone())
            || !keys.insert((site.site_id.clone(), site.claim_id.clone()))
        {
            return Err(FederatedKnowledgeError::InvalidSite(
                "identity, statement/scope binding, score, source, privacy, preclinical, or uniqueness declaration is invalid".into(),
            ));
        }
        groups.entry(site.claim_id.clone()).or_default().push(site);
    }
    if groups.len() > MAX_CLAIMS {
        return Err(FederatedKnowledgeError::InvalidSite(
            "claim count exceeds the supported bound".into(),
        ));
    }
    let claim_order = groups.keys().cloned().collect::<Vec<_>>();
    let mut actions = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for (claim_id, claim_sites) in groups {
        let summary = pooled_claim(&claim_sites);
        let kind = classify(summary, request);
        let site_order = claim_sites
            .iter()
            .map(|site| site.site_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let priority_milli = priority(summary);
        if matches!(kind, FederatedKnowledgeKind::Unresolved) {
            unresolved_order.push(claim_id.clone());
            uncertainty.push(format!(
                "{claim_id}: federated claim quorum or confidence is unresolved"
            ));
        }
        if matches!(kind, FederatedKnowledgeKind::SiteSpecific)
            || matches!(kind, FederatedKnowledgeKind::ConsensusNegative)
            || matches!(kind, FederatedKnowledgeKind::ConsensusContested)
        {
            negative_evidence.push(format!(
                "{claim_id}: federated knowledge is not unqualified positive support"
            ));
        }
        if summary.max_leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
            uncertainty.push(format!(
                "{claim_id}: claim confidence is influence-sensitive"
            ));
        }
        if !matches!(
            kind,
            FederatedKnowledgeKind::Unresolved | FederatedKnowledgeKind::SiteSpecific
        ) && priority_milli >= request.minimum_confidence_milli
            && summary.max_leave_one_out_shift_milli <= request.max_leave_one_out_shift_milli
            && actions.len() < request.max_actions
        {
            let rationale = match kind {
                FederatedKnowledgeKind::ConsensusSupported => {
                    "independent typed-knowledge compilers agree on a supported claim; permit downstream planning while retaining site provenance"
                }
                FederatedKnowledgeKind::ConsensusContested => {
                    "independent compilers agree that the claim remains contested; route contradiction adjudication rather than promotion"
                }
                FederatedKnowledgeKind::ConsensusNegative => {
                    "independent compilers agree on a negative disposition; preserve the null/negative result for replanning"
                }
                FederatedKnowledgeKind::SiteSpecific | FederatedKnowledgeKind::Unresolved => unreachable!(),
            };
            actions.push(FederatedKnowledgeAction {
                action_id: format!("federated-knowledge:{claim_id}"),
                claim_id,
                kind,
                site_order,
                pooled_support_milli: summary.support_milli,
                pooled_contradiction_milli: summary.contradiction_milli,
                pooled_confidence_milli: summary.confidence_milli,
                consensus_milli: summary.consensus_milli,
                disagreement_milli: summary.disagreement_milli,
                max_leave_one_out_shift_milli: summary.max_leave_one_out_shift_milli,
                priority_milli,
                rationale: rationale.into(),
            });
        }
    }
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    unresolved_order.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if claim_order.is_empty() {
        FederatedKnowledgeDisposition::Unresolved
    } else if !actions.is_empty() {
        FederatedKnowledgeDisposition::Qualified
    } else if !unresolved_order.is_empty() {
        FederatedKnowledgeDisposition::Underpowered
    } else if negative_evidence
        .iter()
        .any(|entry| entry.contains("not unqualified"))
    {
        FederatedKnowledgeDisposition::Disagreement
    } else if uncertainty
        .iter()
        .any(|entry| entry.contains("influence-sensitive"))
    {
        FederatedKnowledgeDisposition::InfluenceBlocked
    } else {
        FederatedKnowledgeDisposition::Unresolved
    };
    let next_step = match disposition {
        FederatedKnowledgeDisposition::Qualified => {
            "route qualified claim summaries to knowledge consistency and downstream mechanism planning"
        }
        FederatedKnowledgeDisposition::Underpowered => {
            "obtain additional independent typed-knowledge summaries before promotion"
        }
        FederatedKnowledgeDisposition::Disagreement => {
            "retain site disagreement and route the claim to contradiction adjudication"
        }
        FederatedKnowledgeDisposition::InfluenceBlocked => {
            "obtain additional sites because the consensus depends on one contributor"
        }
        FederatedKnowledgeDisposition::Unresolved => {
            "do not promote the claim until quorum, confidence, and disposition gates resolve"
        }
    };
    let mut output = FederatedKnowledge {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        snapshot_id: request.snapshot_id.clone(),
        claim_order,
        action_order,
        actions,
        unresolved_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| FederatedKnowledgeError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedKnowledgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(site_id: &str, disposition: KnowledgeClaimDisposition) -> FederatedKnowledgeSiteClaim {
        FederatedKnowledgeSiteClaim {
            site_id: site_id.into(),
            study_id: format!("study-{site_id}"),
            claim_id: "claim-egfr-invasion".into(),
            statement: "egfr activation increases invasion".into(),
            scope: "organoid".into(),
            support_milli: 900,
            contradiction_milli: 50,
            confidence_milli: 900,
            disposition,
            source_count: 4,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{site_id}"),
                content_hash: ContentHash::of_value(&serde_json::json!({"site": site_id})).unwrap(),
                content_type: "aggregate-typed-knowledge".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            preclinical_only: true,
        }
    }

    fn request() -> FederatedKnowledgeRequest {
        FederatedKnowledgeRequest {
            objective: "federated glioma knowledge".into(),
            snapshot_id: "snapshot-1".into(),
            minimum_sites: 3,
            minimum_consensus_milli: 800,
            minimum_confidence_milli: 100,
            max_disagreement_milli: 500,
            max_leave_one_out_shift_milli: 100,
            max_actions: 8,
        }
    }

    #[test]
    fn qualifies_consensus_supported_claim() {
        let result = analyze_federated_knowledge(
            &request(),
            &[
                site("site-a", KnowledgeClaimDisposition::Supported),
                site("site-b", KnowledgeClaimDisposition::Supported),
                site("site-c", KnowledgeClaimDisposition::Supported),
            ],
        )
        .expect("federated knowledge");
        assert_eq!(result.disposition, FederatedKnowledgeDisposition::Qualified);
        assert_eq!(
            result.actions[0].kind,
            FederatedKnowledgeKind::ConsensusSupported
        );
        assert_eq!(result.actions[0].consensus_milli, 1_000);
        result.validate().expect("digest validates");
    }

    #[test]
    fn retains_disagreement_between_site_compilers() {
        let result = analyze_federated_knowledge(
            &request(),
            &[
                site("site-a", KnowledgeClaimDisposition::Supported),
                site("site-b", KnowledgeClaimDisposition::Contested),
                site("site-c", KnowledgeClaimDisposition::Negative),
            ],
        )
        .expect("federated knowledge");
        assert_eq!(
            result.disposition,
            FederatedKnowledgeDisposition::Disagreement
        );
        assert!(result.actions.is_empty());
        assert!(!result.negative_evidence.is_empty());
    }
}
