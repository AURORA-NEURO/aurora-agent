//! Exact-identifier linkage between the real glioma snapshot and the wider PubMed snapshot.
//!
//! The two public bundles deliberately have different scopes: `RealGliomaBundle` carries a
//! bounded glioblastoma literature index alongside registry/genomic metadata, while
//! `PublicLiteratureBundle` carries six specialty-tagged PubMed lanes. This module joins only
//! exact PMID/DOI identifiers, names metadata drift by field, and leaves unmatched records
//! visible. It does not infer cohort identity, evidence quality, biological relationships, or
//! clinical meaning, and it never fetches or mutates either bundle.

use crate::{
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureRecord, PublicLiteratureSummary,
    RealDataSummary, RealGliomaBundle, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const LITERATURE_LINK_AUDIT_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-literature-link-audit/0.1";
const MAX_LINKS: usize = 256;
const MAX_UNMATCHED_IDS: usize = 256;
const DEFAULT_MAX_LINKS: usize = 128;
const DEFAULT_MAX_UNMATCHED_IDS: usize = 64;

fn default_public_specialty() -> Option<Specialty> {
    Some(Specialty::Glioma)
}

fn default_max_links() -> usize {
    DEFAULT_MAX_LINKS
}

fn default_max_unmatched_ids() -> usize {
    DEFAULT_MAX_UNMATCHED_IDS
}

/// Bounded exact-identifier linkage controls. `null` `public_specialty` intentionally widens
/// the public side to all six lanes so a caller can inspect cross-lane identifier collisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureLinkAuditQuery {
    #[serde(default = "default_public_specialty")]
    pub public_specialty: Option<Specialty>,
    #[serde(default = "default_max_links")]
    pub max_links: usize,
    #[serde(default = "default_max_unmatched_ids")]
    pub max_unmatched_ids: usize,
}

impl Default for LiteratureLinkAuditQuery {
    fn default() -> Self {
        Self {
            public_specialty: default_public_specialty(),
            max_links: default_max_links(),
            max_unmatched_ids: default_max_unmatched_ids(),
        }
    }
}

/// Identifier used for one exact cross-bundle link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiteratureLinkKind {
    Pmid,
    Doi,
}

/// One exact PMID/DOI correspondence. Metadata is represented by field names only; source text
/// is intentionally not copied into the linkage report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureBundleLink {
    pub real_pmid: String,
    pub public_pmid: String,
    pub public_specialty: Specialty,
    pub real_source_id: String,
    pub public_source_id: String,
    pub match_kinds: Vec<LiteratureLinkKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mismatched_fields: Vec<String>,
}

/// Aggregate exact-link counts. Counts do not imply cohort overlap or scientific comparability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureLinkAuditCounts {
    pub real_literature_records: usize,
    pub selected_public_literature_records: usize,
    pub linked_real_records: usize,
    pub linked_public_records: usize,
    pub unmatched_real_records: usize,
    pub unmatched_public_records: usize,
    pub pmid_match_count: usize,
    pub doi_match_count: usize,
    pub metadata_mismatch_count: usize,
    pub identifier_conflict_count: usize,
}

/// A reviewer reason emitted when exact linkage is incomplete or internally discordant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureLinkReviewReason {
    pub code: String,
    pub count: usize,
    pub detail: String,
}

/// Digest-bound, provider-free cross-bundle literature linkage audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureLinkAuditReport {
    pub schema_version: String,
    pub audit_digest: String,
    pub real_data_bundle_digest: String,
    pub public_literature_bundle_digest: String,
    pub real_data_generated_at: String,
    pub public_literature_generated_at: String,
    pub query: LiteratureLinkAuditQuery,
    pub real_data_summary: RealDataSummary,
    pub public_literature_summary: PublicLiteratureSummary,
    pub counts: LiteratureLinkAuditCounts,
    pub links: Vec<LiteratureBundleLink>,
    pub unmatched_real_pmids: Vec<String>,
    pub unmatched_public_pmids: Vec<String>,
    pub omitted_link_count: usize,
    pub omitted_unmatched_real_count: usize,
    pub omitted_unmatched_public_count: usize,
    pub truncated: bool,
    pub requires_link_review: bool,
    pub review_reasons: Vec<LiteratureLinkReviewReason>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealGliomaBundle {
    /// Link this real glioma bundle to a validated public-literature bundle by exact identifiers.
    pub fn literature_link_audit(
        &self,
        public_literature: &PublicLiteratureBundle,
        query: &LiteratureLinkAuditQuery,
    ) -> Result<LiteratureLinkAuditReport, NeurosurgeryError> {
        validate_query(query)?;
        self.validate()?;
        public_literature.validate()?;
        let real_data_summary = self.summary()?;
        let public_literature_summary = public_literature.summary()?;
        let selected_public = public_literature
            .records
            .iter()
            .filter(|record| {
                query
                    .public_specialty
                    .is_none_or(|specialty| record.specialty == specialty)
            })
            .collect::<Vec<_>>();

        let public_by_pmid = selected_public
            .iter()
            .map(|record| (record.pmid.as_str(), *record))
            .collect::<BTreeMap<_, _>>();
        let mut public_by_doi: BTreeMap<String, Vec<&PublicLiteratureRecord>> = BTreeMap::new();
        for record in &selected_public {
            if let Some(doi) = record.doi.as_deref() {
                public_by_doi
                    .entry(normalize_doi(doi))
                    .or_default()
                    .push(*record);
            }
        }

        let mut links = Vec::new();
        let mut matched_real_pmids = BTreeSet::new();
        let mut matched_public_pmids = BTreeSet::new();
        let mut pmid_match_count = 0;
        let mut doi_match_count = 0;
        let mut metadata_mismatch_count = 0;
        let mut identifier_conflict_count = 0;

        for real_record in &self.literature {
            let mut candidates =
                BTreeMap::<&str, (&PublicLiteratureRecord, Vec<LiteratureLinkKind>)>::new();
            if let Some(public_record) = public_by_pmid.get(real_record.pmid.as_str()) {
                candidates
                    .entry(public_record.pmid.as_str())
                    .or_insert_with(|| (*public_record, Vec::new()))
                    .1
                    .push(LiteratureLinkKind::Pmid);
            }
            if let Some(doi) = real_record.doi.as_deref() {
                if let Some(public_records) = public_by_doi.get(&normalize_doi(doi)) {
                    for public_record in public_records {
                        candidates
                            .entry(public_record.pmid.as_str())
                            .or_insert_with(|| (*public_record, Vec::new()))
                            .1
                            .push(LiteratureLinkKind::Doi);
                    }
                }
            }

            for (_, (public_record, mut match_kinds)) in candidates {
                match_kinds.sort();
                match_kinds.dedup();
                let mismatched_fields = metadata_mismatch_fields(real_record, public_record);
                let has_pmid = match_kinds.contains(&LiteratureLinkKind::Pmid);
                let has_doi = match_kinds.contains(&LiteratureLinkKind::Doi);
                if has_pmid {
                    pmid_match_count += 1;
                }
                if has_doi {
                    doi_match_count += 1;
                }
                if !mismatched_fields.is_empty() {
                    metadata_mismatch_count += 1;
                }
                if (has_pmid
                    && real_record.doi.is_some()
                    && public_record.doi.is_some()
                    && normalize_doi(real_record.doi.as_deref().unwrap_or_default())
                        != normalize_doi(public_record.doi.as_deref().unwrap_or_default()))
                    || (has_doi && real_record.pmid != public_record.pmid)
                {
                    identifier_conflict_count += 1;
                }
                matched_real_pmids.insert(real_record.pmid.clone());
                matched_public_pmids.insert(public_record.pmid.clone());
                links.push(LiteratureBundleLink {
                    real_pmid: real_record.pmid.clone(),
                    public_pmid: public_record.pmid.clone(),
                    public_specialty: public_record.specialty,
                    real_source_id: real_record.source_id.clone(),
                    public_source_id: public_record.source_id.clone(),
                    match_kinds,
                    mismatched_fields,
                });
            }
        }
        links.sort_by(|left, right| {
            left.real_pmid
                .cmp(&right.real_pmid)
                .then(left.public_pmid.cmp(&right.public_pmid))
        });

        let unmatched_real_pmids = self
            .literature
            .iter()
            .filter(|record| !matched_real_pmids.contains(&record.pmid))
            .map(|record| record.pmid.clone())
            .collect::<Vec<_>>();
        let unmatched_public_pmids = selected_public
            .iter()
            .filter(|record| !matched_public_pmids.contains(&record.pmid))
            .map(|record| record.pmid.clone())
            .collect::<Vec<_>>();
        let counts = LiteratureLinkAuditCounts {
            real_literature_records: self.literature.len(),
            selected_public_literature_records: selected_public.len(),
            linked_real_records: matched_real_pmids.len(),
            linked_public_records: matched_public_pmids.len(),
            unmatched_real_records: unmatched_real_pmids.len(),
            unmatched_public_records: unmatched_public_pmids.len(),
            pmid_match_count,
            doi_match_count,
            metadata_mismatch_count,
            identifier_conflict_count,
        };

        let omitted_link_count = links.len().saturating_sub(query.max_links);
        let omitted_unmatched_real_count = unmatched_real_pmids
            .len()
            .saturating_sub(query.max_unmatched_ids);
        let omitted_unmatched_public_count = unmatched_public_pmids
            .len()
            .saturating_sub(query.max_unmatched_ids);
        links.truncate(query.max_links);
        let mut unmatched_real_pmids = unmatched_real_pmids;
        unmatched_real_pmids.truncate(query.max_unmatched_ids);
        let mut unmatched_public_pmids = unmatched_public_pmids;
        unmatched_public_pmids.truncate(query.max_unmatched_ids);

        let mut review_reasons = Vec::new();
        if counts.unmatched_real_records > 0 {
            review_reasons.push(LiteratureLinkReviewReason {
                code: "unmatched_real_literature".to_string(),
                count: counts.unmatched_real_records,
                detail: "real-bundle literature PMIDs without an exact match in the selected public lane remain unresolved; absence is not evidence of absence".to_string(),
            });
        }
        if counts.unmatched_public_records > 0 {
            review_reasons.push(LiteratureLinkReviewReason {
                code: "unmatched_public_literature".to_string(),
                count: counts.unmatched_public_records,
                detail: "selected public-lane PMIDs are outside the real bundle's bounded literature window; the windows must not be treated as equivalent corpora".to_string(),
            });
        }
        if counts.metadata_mismatch_count > 0 {
            review_reasons.push(LiteratureLinkReviewReason {
                code: "metadata_mismatch".to_string(),
                count: counts.metadata_mismatch_count,
                detail: "exact identifiers matched but one or more source metadata fields differ; inspect the field names before relying on the correspondence".to_string(),
            });
        }
        if counts.identifier_conflict_count > 0 {
            review_reasons.push(LiteratureLinkReviewReason {
                code: "identifier_conflict".to_string(),
                count: counts.identifier_conflict_count,
                detail: "PMID and DOI evidence disagree for one or more candidate links; treat the correspondence as unresolved until a reviewer checks the source records".to_string(),
            });
        }
        let truncated = omitted_link_count > 0
            || omitted_unmatched_real_count > 0
            || omitted_unmatched_public_count > 0;
        if truncated {
            review_reasons.push(LiteratureLinkReviewReason {
                code: "projection_truncated".to_string(),
                count: omitted_link_count
                    + omitted_unmatched_real_count
                    + omitted_unmatched_public_count,
                detail: "caller bounds omitted exact-link or unmatched PMID rows; the returned projection is not exhaustive".to_string(),
            });
        }
        if selected_public.is_empty() {
            review_reasons.push(LiteratureLinkReviewReason {
                code: "empty_public_lane".to_string(),
                count: 1,
                detail: "the selected public specialty lane contains no records, so no correspondence can be established".to_string(),
            });
        }

        let mut report = LiteratureLinkAuditReport {
            schema_version: LITERATURE_LINK_AUDIT_SCHEMA_VERSION.to_string(),
            audit_digest: String::new(),
            real_data_bundle_digest: real_data_summary.bundle_digest.clone(),
            public_literature_bundle_digest: public_literature_summary.bundle_digest.clone(),
            real_data_generated_at: self.generated_at.clone(),
            public_literature_generated_at: public_literature.generated_at.clone(),
            query: query.clone(),
            real_data_summary,
            public_literature_summary,
            counts,
            links,
            unmatched_real_pmids,
            unmatched_public_pmids,
            omitted_link_count,
            omitted_unmatched_real_count,
            omitted_unmatched_public_count,
            truncated,
            requires_link_review: !review_reasons.is_empty(),
            review_reasons,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the audit links only exact PMIDs and normalized DOIs already present in two validated public snapshots; it never fetches or mutates either bundle".to_string(),
                "an exact identifier does not establish cohort identity, independence, evidence quality, applicability, causality, or a patient-level finding".to_string(),
                "unmatched and metadata-mismatch rows describe bounded acquisition windows and source metadata, not biological absence or clinical relevance".to_string(),
                "the report is a caller-owned research handoff and cannot produce diagnosis, prognosis, treatment, triage, or procedural action".to_string(),
            ],
        };
        report.audit_digest = digest_report(&report)?;
        Ok(report)
    }
}

fn validate_query(query: &LiteratureLinkAuditQuery) -> Result<(), NeurosurgeryError> {
    if query.max_links == 0 || query.max_links > MAX_LINKS {
        return Err(NeurosurgeryError::TooMany {
            field: "literature_link_audit.max_links",
            found: query.max_links,
            max: MAX_LINKS,
        });
    }
    if query.max_unmatched_ids == 0 || query.max_unmatched_ids > MAX_UNMATCHED_IDS {
        return Err(NeurosurgeryError::TooMany {
            field: "literature_link_audit.max_unmatched_ids",
            found: query.max_unmatched_ids,
            max: MAX_UNMATCHED_IDS,
        });
    }
    Ok(())
}

fn normalize_doi(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    let normalized = normalized
        .strip_prefix("https://doi.org/")
        .or_else(|| normalized.strip_prefix("http://doi.org/"))
        .or_else(|| normalized.strip_prefix("doi:"))
        .unwrap_or(&normalized);
    normalized
        .trim_end_matches(['.', ',', ';', ')', ']', '}'])
        .to_string()
}

fn metadata_mismatch_fields(
    real_record: &crate::LiteratureRecord,
    public_record: &PublicLiteratureRecord,
) -> Vec<String> {
    let mut fields = Vec::new();
    if real_record.title != public_record.title {
        fields.push("title".to_string());
    }
    if real_record.journal != public_record.journal {
        fields.push("journal".to_string());
    }
    if real_record.publication_date != public_record.publication_date {
        fields.push("publication_date".to_string());
    }
    if real_record.doi.as_deref().map(normalize_doi)
        != public_record.doi.as_deref().map(normalize_doi)
    {
        fields.push("doi".to_string());
    }
    if real_record.abstract_text != public_record.abstract_text {
        fields.push("abstract_text".to_string());
    }
    if real_record.abstract_truncated != public_record.abstract_truncated {
        fields.push("abstract_truncated".to_string());
    }
    if real_record.publication_types != public_record.publication_types {
        fields.push("publication_types".to_string());
    }
    if real_record.mesh_terms != public_record.mesh_terms {
        fields.push("mesh_terms".to_string());
    }
    fields
}

fn digest_report(report: &LiteratureLinkAuditReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.audit_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
