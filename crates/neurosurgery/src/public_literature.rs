//! Provenance-bound, cross-specialty public literature for neurosurgical research.
//!
//! The glioma bundle in [`crate::real_data`] intentionally carries registry, genomic and
//! cBioPortal metadata. This module supplies the missing breadth for the other specialty routes:
//! a compact PubMed-only corpus tagged to the specialty lane it was retrieved for. It is still
//! population-level citation metadata, never a patient record or a generated medical conclusion.
//! The Rust core validates an already downloaded snapshot and never performs network access.

use crate::{EvidenceRecord, EvidenceTier, NeurosurgeryError, Specialty, ToolCapability};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Version of the cross-specialty public-literature contract.
pub const PUBLIC_LITERATURE_SCHEMA_VERSION: &str = "bioprism-neurosurgery-public-literature/0.1";
const MAX_SOURCES: usize = 32;
const MAX_RECORDS: usize = 4_096;
const MAX_QUERY_TEXT_BYTES: usize = 512;
const MAX_QUERY_HITS: usize = 128;
pub(crate) const MAX_QUERY_HITS_PUBLIC: usize = MAX_QUERY_HITS;
const MAX_ABSTRACT_BYTES: usize = 12_000;
const MAX_ABSTRACT_EXCERPT_CHARS: usize = 4_000;
const MAX_TAGS: usize = 64;

/// A PubMed source endpoint and its canonical record digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureSource {
    pub source_id: String,
    pub authority: String,
    pub uri: String,
    pub retrieved_at: String,
    pub content_sha256: String,
    pub record_count: usize,
}

/// A compact, specialty-tagged PubMed citation. The abstract and tags are source text and
/// indexing metadata; the `specialty` field records the retrieval lane, not a diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureRecord {
    pub source_id: String,
    pub specialty: Specialty,
    pub pmid: String,
    pub title: String,
    pub journal: String,
    #[serde(default)]
    pub publication_date: Option<String>,
    #[serde(default)]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstract_text: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub abstract_truncated: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publication_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mesh_terms: Vec<String>,
}

/// A validated, caller-supplied public literature snapshot covering any subset of the six
/// neurosurgical specialty lanes. Coverage is explicit in [`PublicLiteratureSummary`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureBundle {
    pub schema_version: String,
    pub generated_at: String,
    /// Must remain false. Synthetic fixtures belong under `fixtures/`, not here.
    pub synthetic_data: bool,
    pub sources: Vec<PublicLiteratureSource>,
    pub records: Vec<PublicLiteratureRecord>,
}

/// Bounded local query over an already validated public-literature bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureQuery {
    #[serde(default)]
    pub specialty: Option<Specialty>,
    #[serde(default)]
    pub text: Option<String>,
    /// Optional case-insensitive exact phrase match over PubMed publication-type labels.
    #[serde(default)]
    pub publication_type: Option<String>,
    /// Optional case-insensitive exact phrase match over MeSH descriptor labels.
    #[serde(default)]
    pub mesh_term: Option<String>,
    /// Inclusive publication-date lower bound.
    #[serde(default)]
    pub from_date: Option<String>,
    /// Inclusive publication-date upper bound.
    #[serde(default)]
    pub to_date: Option<String>,
    #[serde(default = "default_query_limit")]
    pub limit: usize,
}

fn default_query_limit() -> usize {
    32
}

impl Default for PublicLiteratureQuery {
    fn default() -> Self {
        Self {
            specialty: None,
            text: None,
            publication_type: None,
            mesh_term: None,
            from_date: None,
            to_date: None,
            limit: default_query_limit(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureQueryHit {
    pub specialty: Specialty,
    pub pmid: String,
    pub title: String,
    pub journal: String,
    pub publication_date: Option<String>,
    pub doi: Option<String>,
    pub source_id: String,
    pub source_uri: String,
    /// Direct human-review link for the exact PubMed record.
    pub record_uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstract_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publication_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mesh_terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureSpecialtyCount {
    pub specialty: Specialty,
    pub count: usize,
}

/// A compact summary suitable for attaching to a research report or audit receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureSummary {
    pub schema_version: String,
    pub bundle_digest: String,
    pub source_count: usize,
    pub record_count: usize,
    pub abstract_count: usize,
    pub abstract_truncated_count: usize,
    pub specialty_counts: Vec<PublicLiteratureSpecialtyCount>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureQueryResult {
    pub schema_version: String,
    pub bundle_digest: String,
    pub query: PublicLiteratureQuery,
    pub total_matches: usize,
    pub returned_matches: usize,
    pub truncated: bool,
    pub hits: Vec<PublicLiteratureQueryHit>,
    pub abstract_count: usize,
    pub abstract_truncated_count: usize,
    pub specialty_counts: Vec<PublicLiteratureSpecialtyCount>,
}

impl PublicLiteratureQueryResult {
    /// Validate a persisted citation-query result without reopening the source bundle. This is a
    /// structural gate only; `validate_for_inputs` performs the exact local replay.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != PUBLIC_LITERATURE_SCHEMA_VERSION
            || !is_sha256(&self.bundle_digest)
            || self.query.limit == 0
            || self.query.limit > MAX_QUERY_HITS
            || self.returned_matches > self.total_matches
            || self.hits.len() != self.returned_matches
            || self.returned_matches > self.query.limit
            || self.truncated != (self.returned_matches < self.total_matches)
            || self.hits.windows(2).any(|window| {
                (window[0].specialty, window[0].pmid.as_str())
                    > (window[1].specialty, window[1].pmid.as_str())
            })
            || self.hits.iter().any(|hit| {
                hit.pmid.trim().is_empty()
                    || !hit.pmid.bytes().all(|byte| byte.is_ascii_digit())
                    || hit.title.trim().is_empty()
                    || hit.journal.trim().is_empty()
                    || hit.source_id.trim().is_empty()
                    || !is_allow_listed_uri(&hit.source_uri)
                    || !hit
                        .record_uri
                        .starts_with("https://pubmed.ncbi.nlm.nih.gov/")
                    || hit
                        .abstract_excerpt
                        .as_deref()
                        .is_some_and(|excerpt| excerpt.len() > MAX_ABSTRACT_EXCERPT_CHARS)
                    || hit
                        .publication_date
                        .as_deref()
                        .is_some_and(|date| !is_calendar_date(date))
            })
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature query result envelope is invalid".to_string(),
            });
        }
        validate_query_shape(&self.query)?;
        Ok(())
    }

    /// Replay this result against the exact validated PubMed snapshot and refuse any changed
    /// query, source digest, hit ordering, or count projection.
    pub fn validate_for_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.query(&self.query)?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature query result does not replay to the supplied bundle"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct SourceContent {
    records: Vec<PublicLiteratureRecord>,
}

impl PublicLiteratureBundle {
    /// Validate source allow-lists, record linkage, date/identifier bounds, and canonical hashes.
    pub fn validate(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != PUBLIC_LITERATURE_SCHEMA_VERSION {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "unsupported public-literature schema {:?}; expected {:?}",
                    self.schema_version, PUBLIC_LITERATURE_SCHEMA_VERSION
                ),
            });
        }
        if self.synthetic_data {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "synthetic_data=true is never accepted for public literature".to_string(),
            });
        }
        validate_text(&self.generated_at, "public_literature.generated_at")?;
        if !is_utc_timestamp(&self.generated_at) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature generated_at must be a UTC RFC3339 timestamp"
                    .to_string(),
            });
        }
        if self.sources.is_empty() || self.records.is_empty() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature bundle must contain sources and records".to_string(),
            });
        }
        if self.sources.len() > MAX_SOURCES || self.records.len() > MAX_RECORDS {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "public-literature bundle exceeds safety bounds ({} sources, {} records)",
                    self.sources.len(),
                    self.records.len()
                ),
            });
        }

        let mut source_ids = BTreeSet::new();
        for source in &self.sources {
            validate_text(&source.source_id, "public_literature.source_id")?;
            validate_text(&source.authority, "public_literature.authority")?;
            validate_text(&source.uri, "public_literature.source_uri")?;
            validate_text(&source.retrieved_at, "public_literature.retrieved_at")?;
            if [
                source.source_id.as_str(),
                source.authority.as_str(),
                source.uri.as_str(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "synthetic marker found in public-literature source {}",
                        source.source_id
                    ),
                });
            }
            if !source.uri.starts_with("https://")
                || !is_allow_listed_uri(&source.uri)
                || !is_utc_timestamp(&source.retrieved_at)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "public-literature source {} is not an allow-listed UTC PubMed source",
                        source.source_id
                    ),
                });
            }
            if source.retrieved_at > self.generated_at {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "public-literature source {} was retrieved after bundle generation",
                        source.source_id
                    ),
                });
            }
            if source.record_count == 0 || !is_sha256(&source.content_sha256) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "public-literature source {} has an invalid count or content hash",
                        source.source_id
                    ),
                });
            }
            if !source_ids.insert(source.source_id.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "duplicate public-literature source_id".to_string(),
                });
            }
        }

        let mut pmids = BTreeSet::new();
        for record in &self.records {
            if !source_ids.contains(&record.source_id) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "public-literature record {} references an unknown source_id",
                        record.pmid
                    ),
                });
            }
            for (value, field) in [
                (&record.source_id, "public_literature.record.source_id"),
                (&record.pmid, "public_literature.record.pmid"),
                (&record.title, "public_literature.record.title"),
                (&record.journal, "public_literature.record.journal"),
            ] {
                validate_text(value, field)?;
            }
            if [
                record.source_id.as_str(),
                record.pmid.as_str(),
                record.title.as_str(),
                record.journal.as_str(),
                record.doi.as_deref().unwrap_or_default(),
                record.abstract_text.as_deref().unwrap_or_default(),
            ]
            .into_iter()
            .any(contains_synthetic_marker)
                || record
                    .publication_types
                    .iter()
                    .chain(record.mesh_terms.iter())
                    .any(|value| contains_synthetic_marker(value))
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "synthetic marker found in public-literature PMID {} metadata",
                        record.pmid
                    ),
                });
            }
            if record.pmid.len() > 32 || !record.pmid.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("public-literature PMID {} is invalid", record.pmid),
                });
            }
            if !pmids.insert(record.pmid.clone()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("duplicate public-literature PMID {}", record.pmid),
                });
            }
            if let Some(date) = &record.publication_date {
                validate_text(date, "public_literature.record.publication_date")?;
                if !is_calendar_date(date) {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "public-literature PMID {} has an invalid publication date",
                            record.pmid
                        ),
                    });
                }
            }
            if let Some(doi) = &record.doi {
                validate_text(doi, "public_literature.record.doi")?;
                if doi.len() > 512 || !doi.starts_with("10.") {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "public-literature PMID {} has an invalid DOI",
                            record.pmid
                        ),
                    });
                }
            }
            if let Some(abstract_text) = &record.abstract_text {
                validate_text(abstract_text, "public_literature.record.abstract_text")?;
                if abstract_text.len() > MAX_ABSTRACT_BYTES {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "public-literature PMID {} abstract exceeds {} bytes",
                            record.pmid, MAX_ABSTRACT_BYTES
                        ),
                    });
                }
            } else if record.abstract_truncated {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "public-literature PMID {} marks a missing abstract as truncated",
                        record.pmid
                    ),
                });
            }
            validate_tags(&record.publication_types, "publication type", &record.pmid)?;
            validate_tags(&record.mesh_terms, "MeSH term", &record.pmid)?;
        }

        let by_source = self.canonical_source_content();
        for source in &self.sources {
            let content = by_source.get(&source.source_id).ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "public-literature source {} has no linked records",
                        source.source_id
                    ),
                }
            })?;
            let bytes = serde_json::to_vec(content)
                .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
            if sha256_hex(&bytes) != source.content_sha256.to_ascii_lowercase() {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "content hash mismatch for public-literature source {}",
                        source.source_id
                    ),
                });
            }
            if content.records.len() != source.record_count {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "record count mismatch for public-literature source {}",
                        source.source_id
                    ),
                });
            }
        }
        Ok(())
    }

    /// Return a deterministic summary after validating the snapshot.
    pub fn summary(&self) -> Result<PublicLiteratureSummary, NeurosurgeryError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
        Ok(PublicLiteratureSummary {
            schema_version: self.schema_version.clone(),
            bundle_digest: sha256_hex(&bytes),
            source_count: self.sources.len(),
            record_count: self.records.len(),
            abstract_count: self
                .records
                .iter()
                .filter(|record| record.abstract_text.is_some())
                .count(),
            abstract_truncated_count: self
                .records
                .iter()
                .filter(|record| record.abstract_truncated)
                .count(),
            specialty_counts: self.specialty_counts(),
            provenance_bound: true,
            synthetic_data: false,
        })
    }

    /// Query the validated local corpus by specialty and source text.
    pub fn query(
        &self,
        query: &PublicLiteratureQuery,
    ) -> Result<PublicLiteratureQueryResult, NeurosurgeryError> {
        self.validate()?;
        validate_query_shape(query)?;
        let text = query.text.as_deref().map(str::to_ascii_lowercase);
        let publication_type = query
            .publication_type
            .as_deref()
            .map(str::to_ascii_lowercase);
        let mesh_term = query.mesh_term.as_deref().map(str::to_ascii_lowercase);
        let mut records = self
            .records
            .iter()
            .filter(|record| {
                query
                    .specialty
                    .is_none_or(|specialty| record.specialty == specialty)
            })
            .filter(|record| {
                text.as_deref().is_none_or(|needle| {
                    [
                        record.pmid.as_str(),
                        record.title.as_str(),
                        record.journal.as_str(),
                        record.doi.as_deref().unwrap_or_default(),
                        record.abstract_text.as_deref().unwrap_or_default(),
                    ]
                    .into_iter()
                    .chain(record.publication_types.iter().map(String::as_str))
                    .chain(record.mesh_terms.iter().map(String::as_str))
                    .any(|field| field.to_ascii_lowercase().contains(needle))
                })
            })
            .filter(|record| {
                publication_type.as_deref().is_none_or(|needle| {
                    record
                        .publication_types
                        .iter()
                        .any(|value| value.to_ascii_lowercase().contains(needle))
                })
            })
            .filter(|record| {
                mesh_term.as_deref().is_none_or(|needle| {
                    record
                        .mesh_terms
                        .iter()
                        .any(|value| value.to_ascii_lowercase().contains(needle))
                })
            })
            .filter(|record| {
                query.from_date.as_deref().is_none_or(|from_date| {
                    record
                        .publication_date
                        .as_deref()
                        .is_some_and(|date| date >= from_date)
                })
            })
            .filter(|record| {
                query.to_date.as_deref().is_none_or(|to_date| {
                    record
                        .publication_date
                        .as_deref()
                        .is_some_and(|date| date <= to_date)
                })
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.specialty
                .cmp(&right.specialty)
                .then_with(|| left.pmid.cmp(&right.pmid))
        });
        let total_matches = records.len();
        records.truncate(query.limit);
        let hits = records
            .into_iter()
            .map(|record| {
                let source_uri = self
                    .sources
                    .iter()
                    .find(|source| source.source_id == record.source_id)
                    .map(|source| source.uri.clone())
                    .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "public-literature query hit {} references an unknown source",
                            record.pmid
                        ),
                    })?;
                Ok(PublicLiteratureQueryHit {
                    specialty: record.specialty,
                    pmid: record.pmid.clone(),
                    title: record.title.clone(),
                    journal: record.journal.clone(),
                    publication_date: record.publication_date.clone(),
                    doi: record.doi.clone(),
                    source_id: record.source_id.clone(),
                    source_uri,
                    record_uri: format!("https://pubmed.ncbi.nlm.nih.gov/{}/", record.pmid),
                    abstract_excerpt: record
                        .abstract_text
                        .as_deref()
                        .map(bounded_abstract_excerpt),
                    publication_types: record.publication_types.clone(),
                    mesh_terms: record.mesh_terms.clone(),
                })
            })
            .collect::<Result<Vec<_>, NeurosurgeryError>>()?;
        let summary = self.summary()?;
        Ok(PublicLiteratureQueryResult {
            schema_version: PUBLIC_LITERATURE_SCHEMA_VERSION.to_string(),
            bundle_digest: summary.bundle_digest,
            query: query.clone(),
            total_matches,
            returned_matches: hits.len(),
            truncated: total_matches > hits.len(),
            hits,
            abstract_count: summary.abstract_count,
            abstract_truncated_count: summary.abstract_truncated_count,
            specialty_counts: summary.specialty_counts,
        })
    }

    /// Convert citation metadata into unverified, provenance-bearing evidence records.
    pub fn evidence_records(&self) -> Vec<EvidenceRecord> {
        self.evidence_records_for_specialty(None)
    }

    /// Convert only the requested specialty lane into evidence records. Keeping this scope
    /// explicit prevents a broad snapshot from silently leaking unrelated citations into a
    /// single-specialty research route.
    pub fn evidence_records_for_specialty(
        &self,
        specialty: Option<Specialty>,
    ) -> Vec<EvidenceRecord> {
        self.records
            .iter()
            .filter(|record| specialty.is_none_or(|requested| record.specialty == requested))
            .map(|record| EvidenceRecord {
                id: format!("PMID-{}", record.pmid),
                title: record.title.clone(),
                citation: format!(
                    "PubMed:{}; specialty={}; source_id={}",
                    record.pmid,
                    record.specialty.slug(),
                    record.source_id
                ),
                tier: EvidenceTier::Unverified,
                population: Some(format!(
                    "PubMed indexed citation metadata; specialty lane={}",
                    record.specialty.slug()
                )),
                year: record
                    .publication_date
                    .as_deref()
                    .and_then(|date| date.get(..4))
                    .and_then(|year| year.parse::<u16>().ok()),
                supports: vec![ToolCapability::EvidenceSynthesis],
            })
            .collect()
    }

    /// Whether at least one validated record is tagged for the requested specialty.
    pub fn has_specialty(&self, specialty: Specialty) -> bool {
        self.records
            .iter()
            .any(|record| record.specialty == specialty)
    }

    /// Return canonical source payloads for an ingestion job or audit tool.
    pub fn canonical_source_payloads(&self) -> Result<BTreeMap<String, Value>, NeurosurgeryError> {
        self.canonical_source_content()
            .into_iter()
            .map(|(source_id, content)| {
                serde_json::to_value(content)
                    .map(|value| (source_id, value))
                    .map_err(|error| NeurosurgeryError::Json(error.to_string()))
            })
            .collect()
    }

    /// Compute the hashes that should be placed into `sources[*].content_sha256`.
    pub fn canonical_source_hashes(&self) -> Result<BTreeMap<String, String>, NeurosurgeryError> {
        self.canonical_source_content()
            .into_iter()
            .map(|(source_id, content)| {
                serde_json::to_vec(&content)
                    .map(|bytes| (source_id, sha256_hex(&bytes)))
                    .map_err(|error| NeurosurgeryError::Digest(error.to_string()))
            })
            .collect()
    }

    fn specialty_counts(&self) -> Vec<PublicLiteratureSpecialtyCount> {
        let mut counts = BTreeMap::<Specialty, usize>::new();
        for record in &self.records {
            *counts.entry(record.specialty).or_default() += 1;
        }
        counts
            .into_iter()
            .map(|(specialty, count)| PublicLiteratureSpecialtyCount { specialty, count })
            .collect()
    }

    fn canonical_source_content(&self) -> BTreeMap<String, SourceContent> {
        self.sources
            .iter()
            .map(|source| {
                let mut records = self
                    .records
                    .iter()
                    .filter(|record| record.source_id == source.source_id)
                    .cloned()
                    .collect::<Vec<_>>();
                records.sort_by(|left, right| {
                    left.specialty
                        .cmp(&right.specialty)
                        .then_with(|| left.pmid.cmp(&right.pmid))
                });
                (source.source_id.clone(), SourceContent { records })
            })
            .collect()
    }
}

fn validate_tags(values: &[String], label: &str, pmid: &str) -> Result<(), NeurosurgeryError> {
    if values.len() > MAX_TAGS {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("public-literature PMID {pmid} has too many {label}s"),
        });
    }
    for value in values {
        validate_text(value, "public_literature.record.tag")?;
    }
    Ok(())
}

fn validate_query_shape(query: &PublicLiteratureQuery) -> Result<(), NeurosurgeryError> {
    if query.limit == 0 || query.limit > MAX_QUERY_HITS {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("public-literature query limit must be between 1 and {MAX_QUERY_HITS}"),
        });
    }
    for (value, field) in [
        (&query.text, "public-literature query text"),
        (
            &query.publication_type,
            "public-literature publication_type",
        ),
        (&query.mesh_term, "public-literature mesh_term"),
    ] {
        if let Some(value) = value {
            validate_query_filter(value, field)?;
        }
    }
    for (date, field) in [
        (&query.from_date, "public-literature query from_date"),
        (&query.to_date, "public-literature query to_date"),
    ] {
        if let Some(date) = date {
            if !is_calendar_date(date) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("{field} must be an ISO calendar date"),
                });
            }
        }
    }
    if let (Some(from_date), Some(to_date)) = (&query.from_date, &query.to_date) {
        if from_date > to_date {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature query from_date must not follow to_date".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_query_filter(value: &str, field: &'static str) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty()
        || value.len() > MAX_QUERY_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("{field} is invalid"),
        });
    }
    Ok(())
}

fn validate_text(value: &str, field: &'static str) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("{field} is empty"),
        });
    }
    if value.len() > 16_000 || value.chars().any(char::is_control) {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("{field} exceeds the public-literature safety bound"),
        });
    }
    Ok(())
}

fn bounded_abstract_excerpt(value: &str) -> String {
    value.chars().take(MAX_ABSTRACT_EXCERPT_CHARS).collect()
}

fn contains_synthetic_marker(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "synthetic fixture",
        "synthetic case",
        "synthetic patient",
        "synthetic cohort",
        "generated fixture",
        "fake patient",
    ]
    .into_iter()
    .any(|marker| value.contains(marker))
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_allow_listed_uri(uri: &str) -> bool {
    uri.starts_with("https://eutils.ncbi.nlm.nih.gov/entrez/eutils/")
        || uri.starts_with("https://pubmed.ncbi.nlm.nih.gov/")
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || ![0usize, 1, 2, 3, 5, 6, 8, 9]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
    {
        return false;
    }
    let year = u16::from(bytes[0] - b'0') * 1_000
        + u16::from(bytes[1] - b'0') * 100
        + u16::from(bytes[2] - b'0') * 10
        + u16::from(bytes[3] - b'0');
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    day >= 1 && day <= days_in_month
}

fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if value.len() != 20
        || ![0usize, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return false;
    }
    let hour = (bytes[11] - b'0') * 10 + (bytes[12] - b'0');
    let minute = (bytes[14] - b'0') * 10 + (bytes[15] - b'0');
    let second = (bytes[17] - b'0') * 10 + (bytes[18] - b'0');
    is_calendar_date(&value[..10]) && hour < 24 && minute < 60 && second < 60
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
