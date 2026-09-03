//! Bounded autonomous portfolio orchestration over the real six-lane PubMed snapshot.
//!
//! The portfolio is the next layer above the workbench: it runs one deterministic, lane-scoped
//! metadata query and one reviewer queue for every selected specialty, preserving the exact
//! workbench lane beside those results. It is an offline research handoff, not a model loop or a
//! clinical decision engine. No lane is ranked, no missing field is repaired, and no population
//! citation is promoted to patient evidence.

use crate::{
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureQuery, PublicLiteratureQueryResult,
    PublicLiteratureReviewQueueQuery, PublicLiteratureReviewQueueReport,
    PublicLiteratureWorkbenchLane, PublicLiteratureWorkbenchQuery, RealDataFreshnessQuery,
    RealDataFreshnessReport, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PUBLIC_LITERATURE_PORTFOLIO_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-portfolio/0.1";
const MAX_SPECIALTIES: usize = 6;
const MAX_HITS_PER_LANE: usize = 128;
const DEFAULT_HITS_PER_LANE: usize = 16;
const MAX_REVIEW_ITEMS_PER_LANE: usize = 128;
const DEFAULT_REVIEW_ITEMS_PER_LANE: usize = 32;
const MAX_ISSUES_PER_LANE: usize = 256;
const DEFAULT_ISSUES_PER_LANE: usize = 128;

fn default_hits_per_lane() -> usize {
    DEFAULT_HITS_PER_LANE
}

fn default_review_items_per_lane() -> usize {
    DEFAULT_REVIEW_ITEMS_PER_LANE
}

fn default_issues_per_lane() -> usize {
    DEFAULT_ISSUES_PER_LANE
}

/// Flat, bounded controls for a deterministic six-lane research pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteraturePortfolioQuery {
    /// `None` includes all six supported lanes; a list is an explicit lane filter.
    #[serde(default)]
    pub specialties: Option<Vec<Specialty>>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub publication_type: Option<String>,
    #[serde(default)]
    pub mesh_term: Option<String>,
    #[serde(default)]
    pub from_date: Option<String>,
    #[serde(default)]
    pub to_date: Option<String>,
    #[serde(default = "default_hits_per_lane")]
    pub max_hits_per_lane: usize,
    #[serde(default = "default_review_items_per_lane")]
    pub max_review_items_per_lane: usize,
    #[serde(default = "default_issues_per_lane")]
    pub max_issues_per_lane: usize,
    /// Optional caller-owned UTC source-age policy for the selected portfolio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessQuery>,
}

impl Default for PublicLiteraturePortfolioQuery {
    fn default() -> Self {
        Self {
            specialties: None,
            text: None,
            publication_type: None,
            mesh_term: None,
            from_date: None,
            to_date: None,
            max_hits_per_lane: default_hits_per_lane(),
            max_review_items_per_lane: default_review_items_per_lane(),
            max_issues_per_lane: default_issues_per_lane(),
            freshness: None,
        }
    }
}

/// One specialty's exact query result, profile/coverage lane, and reviewer queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteraturePortfolioLane {
    pub specialty: Specialty,
    pub workbench: PublicLiteratureWorkbenchLane,
    pub query_result: PublicLiteratureQueryResult,
    pub review_queue: PublicLiteratureReviewQueueReport,
}

/// Digest-bound, provider-free multi-specialty research handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteraturePortfolioReport {
    pub schema_version: String,
    pub portfolio_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: PublicLiteraturePortfolioQuery,
    pub lanes: Vec<PublicLiteraturePortfolioLane>,
    pub specialty_count: usize,
    pub non_empty_lane_count: usize,
    pub empty_lane_specialties: Vec<Specialty>,
    pub total_match_count: usize,
    pub total_returned_count: usize,
    pub total_review_issue_count: usize,
    pub total_review_item_count: usize,
    pub omitted_review_item_count: usize,
    pub truncated_lane_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessReport>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl PublicLiteraturePortfolioReport {
    /// Validate a persisted multi-lane portfolio without fetching sources or ranking evidence.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_query(&self.query)?;
        let specialties = selected_specialties(&self.query)?;
        if self.schema_version != PUBLIC_LITERATURE_PORTFOLIO_SCHEMA_VERSION
            || !is_sha256_hex(&self.portfolio_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || self.generated_at.trim().is_empty()
            || self.specialty_count != specialties.len()
            || self.lanes.len() != specialties.len()
            || self
                .lanes
                .iter()
                .map(|lane| lane.specialty)
                .collect::<Vec<_>>()
                != specialties
            || self.lanes.iter().any(|lane| {
                lane.workbench.specialty != lane.specialty
                    || lane.workbench.profile.specialty != lane.specialty
                    || lane.query_result.schema_version
                        != crate::public_literature::PUBLIC_LITERATURE_SCHEMA_VERSION
                    || lane.query_result.bundle_digest != self.bundle_digest
                    || lane.query_result.query.specialty != Some(lane.specialty)
                    || lane.query_result.query.text != self.query.text
                    || lane.query_result.query.publication_type != self.query.publication_type
                    || lane.query_result.query.mesh_term != self.query.mesh_term
                    || lane.query_result.query.from_date != self.query.from_date
                    || lane.query_result.query.to_date != self.query.to_date
                    || lane.query_result.query.limit != self.query.max_hits_per_lane
                    || lane.query_result.returned_matches != lane.query_result.hits.len()
                    || lane.query_result.total_matches < lane.query_result.returned_matches
                    || lane.query_result.truncated
                        != (lane.query_result.total_matches > lane.query_result.returned_matches)
                    || lane.review_queue.bundle_digest != self.bundle_digest
                    || lane.review_queue.query.specialties != Some(vec![lane.specialty])
                    || lane.review_queue.query.max_items != self.query.max_review_items_per_lane
                    || lane.review_queue.returned_item_count != lane.review_queue.items.len()
                    || lane.review_queue.candidate_item_count
                        < lane.review_queue.returned_item_count
                    || lane.review_queue.omitted_item_count
                        != lane
                            .review_queue
                            .candidate_item_count
                            .saturating_sub(lane.review_queue.returned_item_count)
                    || lane.review_queue.truncated != (lane.review_queue.omitted_item_count > 0)
            })
            || self.non_empty_lane_count
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.workbench.record_count > 0)
                    .count()
            || self.empty_lane_specialties
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.workbench.record_count == 0)
                    .map(|lane| lane.specialty)
                    .collect::<Vec<_>>()
            || self.total_match_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.query_result.total_matches)
                    .fold(0usize, usize::saturating_add)
            || self.total_returned_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.query_result.returned_matches)
                    .fold(0usize, usize::saturating_add)
            || self.total_review_issue_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.workbench.review_issue_count)
                    .fold(0usize, usize::saturating_add)
            || self.total_review_item_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.review_queue.candidate_item_count)
                    .fold(0usize, usize::saturating_add)
            || self.omitted_review_item_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.review_queue.omitted_item_count)
                    .fold(0usize, usize::saturating_add)
            || self.truncated_lane_count
                != self
                    .lanes
                    .iter()
                    .filter(|lane| {
                        lane.workbench.truncated
                            || lane.query_result.truncated
                            || lane.review_queue.truncated
                    })
                    .count()
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature portfolio envelope is invalid".to_string(),
            });
        }
        if let Some(freshness) = self.freshness.as_ref() {
            if freshness.bundle_digest != self.bundle_digest
                || !is_sha256_hex(&freshness.freshness_digest)
                || !freshness.provenance_bound
                || freshness.synthetic_data
                || !freshness.human_review_required
                || freshness.provider != "none"
                || freshness.network
                || freshness.effect != "read_only"
                || self.query.freshness.as_ref() != Some(&freshness.query)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "public-literature portfolio freshness binding is invalid".to_string(),
                });
            }
        } else if self.query.freshness.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature portfolio freshness query is missing its report"
                    .to_string(),
            });
        }
        if self.portfolio_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature portfolio digest does not match its contents"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the portfolio from the exact validated public-literature snapshot and query.
    pub fn validate_for_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.literature_portfolio(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "public-literature portfolio does not replay to the exact supplied snapshot"
                        .to_string(),
            });
        }
        Ok(())
    }
}

impl PublicLiteratureBundle {
    /// Run the same bounded, source-linked review pass across every selected specialty lane.
    pub fn literature_portfolio(
        &self,
        query: &PublicLiteraturePortfolioQuery,
    ) -> Result<PublicLiteraturePortfolioReport, NeurosurgeryError> {
        validate_query(query)?;
        self.validate()?;
        let specialties = selected_specialties(query)?;
        let workbench = self.specialty_workbench(&PublicLiteratureWorkbenchQuery {
            specialties: Some(specialties.clone()),
            max_issues_per_lane: query.max_issues_per_lane,
            freshness: query.freshness.clone(),
        })?;

        let mut lanes = Vec::with_capacity(workbench.lanes.len());
        for workbench_lane in workbench.lanes.iter().cloned() {
            let query_result = self.query(&PublicLiteratureQuery {
                specialty: Some(workbench_lane.specialty),
                text: query.text.clone(),
                publication_type: query.publication_type.clone(),
                mesh_term: query.mesh_term.clone(),
                from_date: query.from_date.clone(),
                to_date: query.to_date.clone(),
                limit: query.max_hits_per_lane,
            })?;
            let review_queue = self.review_queue(&PublicLiteratureReviewQueueQuery {
                specialties: Some(vec![workbench_lane.specialty]),
                max_items: query.max_review_items_per_lane,
            })?;
            lanes.push(PublicLiteraturePortfolioLane {
                specialty: workbench_lane.specialty,
                workbench: workbench_lane,
                query_result,
                review_queue,
            });
        }

        let empty_lane_specialties = lanes
            .iter()
            .filter(|lane| lane.workbench.record_count == 0)
            .map(|lane| lane.specialty)
            .collect::<Vec<_>>();
        let non_empty_lane_count = lanes.len() - empty_lane_specialties.len();
        let total_match_count = lanes
            .iter()
            .map(|lane| lane.query_result.total_matches)
            .sum();
        let total_returned_count = lanes
            .iter()
            .map(|lane| lane.query_result.returned_matches)
            .sum();
        let total_review_issue_count = lanes
            .iter()
            .map(|lane| lane.workbench.review_issue_count)
            .sum();
        let total_review_item_count = lanes
            .iter()
            .map(|lane| lane.review_queue.candidate_item_count)
            .sum();
        let omitted_review_item_count = lanes
            .iter()
            .map(|lane| lane.review_queue.omitted_item_count)
            .sum();
        let truncated_lane_count = lanes
            .iter()
            .filter(|lane| {
                lane.workbench.truncated
                    || lane.query_result.truncated
                    || lane.review_queue.truncated
            })
            .count();
        let mut report = PublicLiteraturePortfolioReport {
            schema_version: PUBLIC_LITERATURE_PORTFOLIO_SCHEMA_VERSION.to_string(),
            portfolio_digest: String::new(),
            bundle_digest: workbench.bundle_digest.clone(),
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            lanes,
            specialty_count: specialties.len(),
            non_empty_lane_count,
            empty_lane_specialties,
            total_match_count,
            total_returned_count,
            total_review_issue_count,
            total_review_item_count,
            omitted_review_item_count,
            truncated_lane_count,
            freshness: workbench.freshness.clone(),
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the portfolio is a deterministic multi-lane research handoff; it does not rank specialties, score evidence, infer biology, or make a clinical conclusion".to_string(),
                "query matches are lexical metadata retrieval from this exact validated snapshot, not relevance, evidence-quality, diagnostic, prognostic, treatment, triage, or procedural judgment".to_string(),
                "workbench profiles and review queues describe reviewer-owned questions and metadata obligations; they do not prescribe tests, care, or operations".to_string(),
                "missing or truncated metadata remains unknown; no field is imputed, repaired, deduplicated, or treated as negative evidence".to_string(),
                "the report never fetches URLs, invokes a provider, opens credentials, stores patient files, sends notifications, or writes durable state".to_string(),
            ],
        };
        report.portfolio_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &PublicLiteraturePortfolioQuery) -> Result<(), NeurosurgeryError> {
    if query.max_hits_per_lane == 0 || query.max_hits_per_lane > MAX_HITS_PER_LANE {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_portfolio.max_hits_per_lane",
            found: query.max_hits_per_lane,
            max: MAX_HITS_PER_LANE,
        });
    }
    if query.max_review_items_per_lane == 0
        || query.max_review_items_per_lane > MAX_REVIEW_ITEMS_PER_LANE
    {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_portfolio.max_review_items_per_lane",
            found: query.max_review_items_per_lane,
            max: MAX_REVIEW_ITEMS_PER_LANE,
        });
    }
    if query.max_issues_per_lane == 0 || query.max_issues_per_lane > MAX_ISSUES_PER_LANE {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_portfolio.max_issues_per_lane",
            found: query.max_issues_per_lane,
            max: MAX_ISSUES_PER_LANE,
        });
    }
    if let Some(specialties) = &query.specialties {
        if specialties.is_empty() || specialties.len() > MAX_SPECIALTIES {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "public-literature portfolio specialties must contain 1..={MAX_SPECIALTIES} lanes"
                ),
            });
        }
        let mut seen = BTreeSet::new();
        if specialties.iter().any(|specialty| !seen.insert(*specialty)) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature portfolio specialties must be unique".to_string(),
            });
        }
    }
    Ok(())
}

fn selected_specialties(
    query: &PublicLiteraturePortfolioQuery,
) -> Result<Vec<Specialty>, NeurosurgeryError> {
    let mut specialties = query
        .specialties
        .clone()
        .unwrap_or_else(|| Specialty::ALL.to_vec());
    if specialties.is_empty() || specialties.len() > MAX_SPECIALTIES {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!(
                "public-literature portfolio specialties must contain 1..={MAX_SPECIALTIES} lanes"
            ),
        });
    }
    specialties.sort_unstable();
    Ok(specialties)
}

fn digest_report(report: &PublicLiteraturePortfolioReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.portfolio_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
