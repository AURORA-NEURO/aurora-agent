//! Federated multi-site evaluation consensus with contradiction preservation.
//!
//! Atlas feature: `AFA-evalengine-P23-F02`.
//!
//! This product combines independently produced EvaluationCards without averaging away site
//! disagreement. A site is accepted only when its card is valid and release-eligible; blocked
//! cards and contradictory digests remain explicit entries in the returned receipt.

use crate::evaluation_observability::EvaluationCardReceipt;
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-evalengine-P23-F02";
pub const FEATURE_VERSION: &str = "0.1.0";
pub const MAX_FEDERATED_SITES: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederatedEvaluationSite {
    pub site_id: String,
    pub card: EvaluationCardReceipt,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederatedEvaluationRequest {
    pub capability_id: String,
    pub benchmark_world: String,
    pub minimum_sites: usize,
    pub sites: Vec<FederatedEvaluationSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEvaluationDisposition {
    Consensus,
    Partial,
    Contradicted,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEvaluationSiteDisposition {
    Accepted,
    Contradictory,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvaluationSiteEntry {
    pub site_id: String,
    pub disposition: FederatedEvaluationSiteDisposition,
    pub card_digest: Option<ContentHash>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvaluationReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub minimum_sites: usize,
    pub total_sites: usize,
    pub agreeing_sites: usize,
    pub contradictory_sites: usize,
    pub blocked_sites: usize,
    pub disposition: FederatedEvaluationDisposition,
    pub entries: Vec<FederatedEvaluationSiteEntry>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl FederatedEvaluationReceipt {
    pub fn validate(&self) -> Result<(), FederatedEvaluationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.capability_id.trim().is_empty()
            || self.benchmark_world.trim().is_empty()
        {
            return Err(FederatedEvaluationError::InvalidField(
                "schema, identity, or boundary".into(),
            ));
        }
        if self.minimum_sites == 0
            || self.total_sites == 0
            || self.total_sites != self.entries.len()
            || self.agreeing_sites + self.contradictory_sites + self.blocked_sites
                != self.total_sites
            || self.entries.iter().any(|entry| {
                entry.site_id.trim().is_empty()
                    || entry.reasons.is_empty()
                    || (entry.disposition == FederatedEvaluationSiteDisposition::Accepted
                        && entry.card_digest.is_none())
            })
        {
            return Err(FederatedEvaluationError::InvalidField(
                "site counts, identity, digest, or reasons".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedEvaluationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedEvaluationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedEvaluationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedEvaluationError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum FederatedEvaluationError {
    #[error("invalid federated evaluation field: {0}")]
    InvalidField(String),
    #[error("duplicate federated site {0}")]
    DuplicateSite(String),
    #[error("federated evaluation has too many sites: {0} > {MAX_FEDERATED_SITES}")]
    TooLarge(usize),
    #[error("federated evaluation artifact error: {0}")]
    Artifact(String),
    #[error("federated evaluation serialization error: {0}")]
    Serialization(String),
}

pub fn federated_evaluation_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "evalengine".into(),
        consumers: ["research program lead".into(), "consortium benchmark curator".into()].into(),
        behavior: "compares independently produced EvaluationCards by capability, benchmark world, and content digest while preserving accepted, contradictory, and blocked site entries".into(),
        value: "turns multi-site evaluation into an omission-aware consensus product without erasing disagreement or treating missing sites as success".into(),
        inputs: vec![TypedPort { name: "federated_evaluation_request".into(), schema: "FederatedEvaluationRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federated_evaluation_receipt".into(), schema: "FederatedEvaluationReceipt@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["read:site-evaluation-cards".into(), "write:local-research-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "otel-evaluation-plane".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn evaluate_federated_evaluation(
    request: &FederatedEvaluationRequest,
) -> Result<FederatedEvaluationReceipt, FederatedEvaluationError> {
    validate_request(request)?;
    let mut sites = request.sites.clone();
    sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let mut valid_digests = BTreeMap::<String, Vec<String>>::new();
    let mut preliminary = Vec::with_capacity(sites.len());
    for site in sites {
        let site_id = site.site_id.clone();
        let result = site.card.validate().and_then(|_| {
            if site.card.card.capability_id != request.capability_id
                || site.card.card.benchmark_world != request.benchmark_world
            {
                return Err(
                    crate::evaluation_observability::EvaluationObservabilityError::InvalidRequest(
                        "capability or benchmark world mismatch".into(),
                    ),
                );
            }
            if !site.card.omissions.is_empty()
                || matches!(
                    site.card.card.release_verdict,
                    bioprism_foundation::ReleaseVerdict::Blocked
                        | bioprism_foundation::ReleaseVerdict::NotEvaluated
                )
            {
                return Err(
                    crate::evaluation_observability::EvaluationObservabilityError::InvalidRequest(
                        "site card is blocked or retains protected omissions".into(),
                    ),
                );
            }
            Ok(())
        });
        match result {
            Ok(()) => {
                let digest = site.card.card_digest.clone();
                valid_digests
                    .entry(digest.to_string())
                    .or_default()
                    .push(site_id.clone());
                preliminary.push((site_id, Some(digest), None));
            }
            Err(error) => preliminary.push((site_id, None, Some(error.to_string()))),
        }
    }
    let winning_digest = valid_digests
        .iter()
        .max_by(|left, right| {
            left.1
                .len()
                .cmp(&right.1.len())
                .then_with(|| right.0.cmp(left.0))
        })
        .map(|(digest, _)| digest.clone());
    let mut entries = Vec::with_capacity(preliminary.len());
    for (site_id, digest, error) in preliminary {
        match (digest, error, winning_digest.as_deref()) {
            (Some(digest), None, Some(winner)) if digest.to_string() == winner => {
                entries.push(FederatedEvaluationSiteEntry {
                    site_id,
                    disposition: FederatedEvaluationSiteDisposition::Accepted,
                    card_digest: Some(digest),
                    reasons: vec!["card matches the winning canonical evaluation digest".into()],
                })
            }
            (Some(digest), None, Some(_)) => entries.push(FederatedEvaluationSiteEntry {
                site_id,
                disposition: FederatedEvaluationSiteDisposition::Contradictory,
                card_digest: Some(digest),
                reasons: vec!["card digest contradicts the winning site consensus".into()],
            }),
            (Some(digest), None, None) => entries.push(FederatedEvaluationSiteEntry {
                site_id,
                disposition: FederatedEvaluationSiteDisposition::Blocked,
                card_digest: Some(digest),
                reasons: vec!["no eligible site consensus exists".into()],
            }),
            (Some(digest), Some(error), _) => entries.push(FederatedEvaluationSiteEntry {
                site_id,
                disposition: FederatedEvaluationSiteDisposition::Blocked,
                card_digest: Some(digest),
                reasons: vec![error],
            }),
            (None, Some(error), _) => entries.push(FederatedEvaluationSiteEntry {
                site_id,
                disposition: FederatedEvaluationSiteDisposition::Blocked,
                card_digest: None,
                reasons: vec![error],
            }),
            (None, None, _) => entries.push(FederatedEvaluationSiteEntry {
                site_id,
                disposition: FederatedEvaluationSiteDisposition::Blocked,
                card_digest: None,
                reasons: vec!["site produced neither a valid card digest nor a refusal".into()],
            }),
        }
    }
    let agreeing_sites = entries
        .iter()
        .filter(|entry| entry.disposition == FederatedEvaluationSiteDisposition::Accepted)
        .count();
    let contradictory_sites = entries
        .iter()
        .filter(|entry| entry.disposition == FederatedEvaluationSiteDisposition::Contradictory)
        .count();
    let blocked_sites = entries
        .iter()
        .filter(|entry| entry.disposition == FederatedEvaluationSiteDisposition::Blocked)
        .count();
    let disposition = if agreeing_sites < request.minimum_sites {
        FederatedEvaluationDisposition::Blocked
    } else if contradictory_sites > 0 {
        FederatedEvaluationDisposition::Contradicted
    } else if blocked_sites > 0 {
        FederatedEvaluationDisposition::Partial
    } else {
        FederatedEvaluationDisposition::Consensus
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "capability_id": request.capability_id,
        "benchmark_world": request.benchmark_world,
        "minimum_sites": request.minimum_sites,
        "total_sites": entries.len(),
        "agreeing_sites": agreeing_sites,
        "contradictory_sites": contradictory_sites,
        "blocked_sites": blocked_sites,
        "disposition": disposition,
        "entries": entries,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        "federated-evaluation",
        "application/vnd.aurora.federated-evaluation+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedEvaluationError::Artifact(error.to_string()))?;
    let receipt = FederatedEvaluationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        minimum_sites: request.minimum_sites,
        total_sites: entries.len(),
        agreeing_sites,
        contradictory_sites,
        blocked_sites,
        disposition,
        entries: serde_json::from_value(payload["entries"].clone())
            .map_err(|error| FederatedEvaluationError::Serialization(error.to_string()))?,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedEvaluationRequest) -> Result<(), FederatedEvaluationError> {
    if request.capability_id.trim().is_empty()
        || request.benchmark_world.trim().is_empty()
        || request.minimum_sites == 0
        || request.sites.is_empty()
    {
        return Err(FederatedEvaluationError::InvalidField(
            "capability, benchmark, minimum sites, and sites are required".into(),
        ));
    }
    if request.sites.len() > MAX_FEDERATED_SITES {
        return Err(FederatedEvaluationError::TooLarge(request.sites.len()));
    }
    let mut ids = BTreeSet::new();
    for site in &request.sites {
        if site.site_id.trim().is_empty() || !ids.insert(site.site_id.clone()) {
            return Err(FederatedEvaluationError::DuplicateSite(
                site.site_id.clone(),
            ));
        }
    }
    if request.minimum_sites > request.sites.len() {
        return Err(FederatedEvaluationError::InvalidField(
            "minimum sites exceeds supplied sites".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{
        EvaluationCard, EvaluationMetric, ReleaseVerdict, UncertaintyStatement,
    };

    fn valid_card() -> EvaluationCardReceipt {
        let card = EvaluationCard {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            capability_id: "capability:test".into(),
            benchmark_world: "world:test".into(),
            baselines: vec!["baseline:test".into()],
            metrics: vec![EvaluationMetric {
                name: "metric".into(),
                value: "1".into(),
                uncertainty: "bounded".into(),
            }],
            uncertainty: vec![UncertaintyStatement {
                kind: "fixture".into(),
                statement: "bounded".into(),
            }],
            limitations: vec!["fixture".into()],
            release_verdict: ReleaseVerdict::Pass,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let card_digest = ContentHash::of_value(&serde_json::to_value(&card).unwrap()).unwrap();
        let artifact = TypedResearchArtifact::from_payload(
            "evaluation-fixture",
            "application/json",
            &json!({"fixture": true}),
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        EvaluationCardReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            feature_id: crate::evaluation_observability::FEATURE_ID.into(),
            card,
            card_digest,
            observations_digest: ContentHash::of_value(&json!([])).unwrap(),
            baseline_counts: [("baseline:test".into(), 1)].into_iter().collect(),
            omissions: Vec::new(),
            reasons: vec!["fixture".into()],
            artifact,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn duplicate_sites_are_rejected_before_card_evaluation() {
        let request = FederatedEvaluationRequest {
            capability_id: "capability:test".into(),
            benchmark_world: "world:test".into(),
            minimum_sites: 1,
            sites: vec![
                FederatedEvaluationSite {
                    site_id: "site:a".into(),
                    card: valid_card(),
                },
                FederatedEvaluationSite {
                    site_id: "site:a".into(),
                    card: valid_card(),
                },
            ],
        };
        assert!(matches!(
            validate_request(&request),
            Err(FederatedEvaluationError::DuplicateSite(_))
        ));
    }

    #[test]
    fn empty_site_set_is_blocked() {
        let request = FederatedEvaluationRequest {
            capability_id: "capability:test".into(),
            benchmark_world: "world:test".into(),
            minimum_sites: 1,
            sites: vec![],
        };
        assert!(evaluate_federated_evaluation(&request).is_err());
    }
}
