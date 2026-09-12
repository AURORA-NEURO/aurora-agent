//! Lane-complete public-literature workbench for the neurosurgical corpus.
//!
//! A matrix is a deterministic fan-out over the validated PubMed snapshot. It does not infer
//! relationships between specialties or merge cohorts: every lane retains its own bounded packet,
//! query, source digest, and PMID identities. This makes an autonomous caller useful for corpus
//! reconnaissance while keeping the evidence population/citation-only and human-review-bound.

use crate::public_literature_draft_audit::{
    PublicLiteratureEvidencePacketQuery, PublicLiteratureEvidencePacketReport,
};
use crate::{NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureQuery, Specialty};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PUBLIC_LITERATURE_MATRIX_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-matrix/0.1";
pub const MAX_PUBLIC_LITERATURE_MATRIX_SPECIALTIES: usize = 6;

/// Bounded fan-out query. `specialties` is empty to scan all six supported lanes; the nested
/// query contributes text, publication-type, MeSH, date, and per-lane limit bounds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureMatrixQuery {
    #[serde(default)]
    pub specialties: Vec<Specialty>,
    #[serde(default)]
    pub query: PublicLiteratureQuery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureMatrixLane {
    pub specialty: Specialty,
    pub packet: PublicLiteratureEvidencePacketReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureMatrixReport {
    pub schema_version: String,
    pub matrix_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: PublicLiteratureMatrixQuery,
    pub lanes: Vec<PublicLiteratureMatrixLane>,
    pub specialty_count: usize,
    pub non_empty_lane_count: usize,
    pub empty_lane_specialties: Vec<Specialty>,
    pub total_match_count: usize,
    pub total_returned_count: usize,
    pub truncated_lane_count: usize,
    pub returned_abstract_count: usize,
    pub returned_without_abstract_count: usize,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl PublicLiteratureMatrixReport {
    /// Validate a persisted lane matrix without fetching sources or merging specialties.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        let specialties = selected_specialties(&self.query)?;
        if self.schema_version != PUBLIC_LITERATURE_MATRIX_SCHEMA_VERSION
            || !is_sha256_hex(&self.matrix_digest)
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
                lane.packet.validate_integrity().is_err()
                    || lane.packet.bundle_digest != self.bundle_digest
                    || lane.packet.query.query.specialty != Some(lane.specialty)
                    || lane.packet.query.query.text != self.query.query.text
                    || lane.packet.query.query.publication_type != self.query.query.publication_type
                    || lane.packet.query.query.mesh_term != self.query.query.mesh_term
                    || lane.packet.query.query.from_date != self.query.query.from_date
                    || lane.packet.query.query.to_date != self.query.query.to_date
                    || lane.packet.query.query.limit != self.query.query.limit
            })
            || self.non_empty_lane_count
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.packet.query_result.total_matches > 0)
                    .count()
            || self.empty_lane_specialties
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.packet.query_result.total_matches == 0)
                    .map(|lane| lane.specialty)
                    .collect::<Vec<_>>()
            || self.total_match_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.packet.query_result.total_matches)
                    .fold(0usize, usize::saturating_add)
            || self.total_returned_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.packet.query_result.returned_matches)
                    .fold(0usize, usize::saturating_add)
            || self.truncated_lane_count
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.packet.query_result.truncated)
                    .count()
            || self.returned_abstract_count
                != self
                    .lanes
                    .iter()
                    .flat_map(|lane| lane.packet.query_result.hits.iter())
                    .filter(|hit| hit.abstract_excerpt.is_some())
                    .count()
            || self.returned_without_abstract_count
                != self
                    .lanes
                    .iter()
                    .flat_map(|lane| lane.packet.query_result.hits.iter())
                    .filter(|hit| hit.abstract_excerpt.is_none())
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
                reason: "public-literature matrix envelope is invalid".to_string(),
            });
        }
        let digest_input = serde_json::to_vec(&(&self.bundle_digest, &self.query, &self.lanes))
            .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
        if self.matrix_digest != sha256_hex(&digest_input) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature matrix digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the matrix from the exact validated public-literature snapshot and query.
    pub fn validate_for_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.literature_matrix(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature matrix does not replay to the exact supplied snapshot"
                    .to_string(),
            });
        }
        Ok(())
    }
}

impl PublicLiteratureBundle {
    /// Fan out one bounded query across explicit lanes and preserve each lane's packet intact.
    pub fn literature_matrix(
        &self,
        query: &PublicLiteratureMatrixQuery,
    ) -> Result<PublicLiteratureMatrixReport, NeurosurgeryError> {
        self.validate()?;
        let specialties = selected_specialties(query)?;
        if query.query.specialty.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature matrix query.specialty is controlled by specialties"
                    .to_string(),
            });
        }

        let summary = self.summary()?;
        let mut lanes = Vec::with_capacity(specialties.len());
        for specialty in specialties.iter().copied() {
            let lane_query = PublicLiteratureQuery {
                specialty: Some(specialty),
                text: query.query.text.clone(),
                publication_type: query.query.publication_type.clone(),
                mesh_term: query.query.mesh_term.clone(),
                from_date: query.query.from_date.clone(),
                to_date: query.query.to_date.clone(),
                limit: query.query.limit,
            };
            let packet = self.evidence_packet(&PublicLiteratureEvidencePacketQuery {
                query: lane_query,
                freshness: None,
            })?;
            lanes.push(PublicLiteratureMatrixLane { specialty, packet });
        }

        let non_empty_lane_count = lanes
            .iter()
            .filter(|lane| lane.packet.query_result.total_matches > 0)
            .count();
        let empty_lane_specialties = lanes
            .iter()
            .filter(|lane| lane.packet.query_result.total_matches == 0)
            .map(|lane| lane.specialty)
            .collect::<Vec<_>>();
        let total_match_count = lanes
            .iter()
            .map(|lane| lane.packet.query_result.total_matches)
            .sum();
        let total_returned_count = lanes
            .iter()
            .map(|lane| lane.packet.query_result.returned_matches)
            .sum();
        let truncated_lane_count = lanes
            .iter()
            .filter(|lane| lane.packet.query_result.truncated)
            .count();
        let returned_abstract_count = lanes
            .iter()
            .map(|lane| {
                lane.packet
                    .query_result
                    .hits
                    .iter()
                    .filter(|hit| hit.abstract_excerpt.is_some())
                    .count()
            })
            .sum();
        let returned_without_abstract_count = lanes
            .iter()
            .map(|lane| {
                lane.packet
                    .query_result
                    .hits
                    .iter()
                    .filter(|hit| hit.abstract_excerpt.is_none())
                    .count()
            })
            .sum();

        let digest_input = serde_json::to_vec(&(&summary.bundle_digest, query, &lanes))
            .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
        let report = PublicLiteratureMatrixReport {
            schema_version: PUBLIC_LITERATURE_MATRIX_SCHEMA_VERSION.to_string(),
            matrix_digest: sha256_hex(&digest_input),
            bundle_digest: summary.bundle_digest,
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            lanes,
            specialty_count: specialties.len(),
            non_empty_lane_count,
            empty_lane_specialties,
            total_match_count,
            total_returned_count,
            truncated_lane_count,
            returned_abstract_count,
            returned_without_abstract_count,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "each lane is an independent citation packet; the matrix does not merge cohorts or infer cross-specialty biology".to_string(),
                "PMID metadata and bounded abstract excerpts require human source and study-quality review".to_string(),
                "empty or truncated lanes are reported explicitly and are not silently broadened".to_string(),
                "the matrix never fetches URLs, invokes a model, retains patient files, or emits diagnosis, prognosis, treatment, triage, or procedure".to_string(),
            ],
        };
        report.validate_integrity()?;
        Ok(report)
    }
}

fn selected_specialties(
    query: &PublicLiteratureMatrixQuery,
) -> Result<Vec<Specialty>, NeurosurgeryError> {
    let specialties = if query.specialties.is_empty() {
        Specialty::ALL.to_vec()
    } else {
        query.specialties.clone()
    };
    if specialties.is_empty() || specialties.len() > MAX_PUBLIC_LITERATURE_MATRIX_SPECIALTIES {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!(
                "public-literature matrix specialties must contain 1..={MAX_PUBLIC_LITERATURE_MATRIX_SPECIALTIES} lanes"
            ),
        });
    }
    let mut seen = BTreeSet::new();
    if specialties.iter().any(|specialty| !seen.insert(*specialty)) {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "public-literature matrix specialties must be unique".to_string(),
        });
    }
    let mut specialties = specialties;
    specialties.sort_unstable();
    Ok(specialties)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
