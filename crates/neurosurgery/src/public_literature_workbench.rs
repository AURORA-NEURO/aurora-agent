//! Six-lane public-literature workbench for reviewer-owned neurosurgical research.
//!
//! This report joins the closed specialty profile (what a reviewer should interrogate) to
//! explicit coverage and metadata obligations in the validated PubMed snapshot. It is a
//! navigation and completeness surface, not a readiness score: no lane is ranked, and no
//! missing field is converted into a biological or clinical conclusion.

use crate::{
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureIntegrityAuditQuery,
    PublicLiteratureIntegrityReviewReason, PublicLiteratureRecord, RealDataFreshnessQuery,
    RealDataFreshnessReport, Specialty, SpecialtyProfile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const PUBLIC_LITERATURE_WORKBENCH_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-public-literature-workbench/0.1";
const MAX_SPECIALTIES: usize = 6;
const MAX_ISSUES_PER_LANE: usize = 256;
const DEFAULT_MAX_ISSUES_PER_LANE: usize = 128;

fn default_max_issues_per_lane() -> usize {
    DEFAULT_MAX_ISSUES_PER_LANE
}

/// Non-exclusive labels derived only from PubMed publication types, MeSH terms, and source text.
/// They describe corpus design context for a reviewer; they are not evidence-quality grades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicLiteratureDesignStratum {
    HumanIndexed,
    AnimalPreclinical,
    InVitroOrCellLine,
    ReviewOrSynthesis,
    ImagingOrDiagnostic,
    SurgicalOrProcedural,
    DevelopmentalOrGenetic,
    OutcomeOrFollowUp,
    InterventionalStudy,
}

/// Source-addressable counts for one design stratum. A record may occur in several strata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureDesignStratumCount {
    pub stratum: PublicLiteratureDesignStratum,
    pub record_count: usize,
    pub pmids: Vec<String>,
}

/// Bounded lane selection and per-lane issue projection controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureWorkbenchQuery {
    /// `None` includes all six supported lanes; a list is an explicit lane filter.
    #[serde(default)]
    pub specialties: Option<Vec<Specialty>>,
    #[serde(default = "default_max_issues_per_lane")]
    pub max_issues_per_lane: usize,
    /// Optional caller-owned source-age policy for the selected bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessQuery>,
}

impl Default for PublicLiteratureWorkbenchQuery {
    fn default() -> Self {
        Self {
            specialties: None,
            max_issues_per_lane: default_max_issues_per_lane(),
            freshness: None,
        }
    }
}

/// Coverage and reviewer obligations for one specialty lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureWorkbenchLane {
    pub specialty: Specialty,
    pub profile: SpecialtyProfile,
    pub source_ids: Vec<String>,
    pub record_count: usize,
    pub abstract_count: usize,
    pub abstract_truncated_count: usize,
    pub missing_doi_count: usize,
    pub missing_abstract_count: usize,
    pub empty_publication_type_count: usize,
    pub empty_mesh_term_count: usize,
    pub review_issue_count: usize,
    pub omitted_review_issue_count: usize,
    pub truncated: bool,
    pub integrity_audit_digest: String,
    pub review_reasons: Vec<PublicLiteratureIntegrityReviewReason>,
    /// Non-exclusive metadata-derived design context; no row is a quality or clinical score.
    pub design_strata: Vec<PublicLiteratureDesignStratumCount>,
    pub unclassified_design_count: usize,
    pub overlapping_design_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub design_review_pmids: Vec<String>,
}

/// Digest-bound, lane-complete navigation and completeness report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicLiteratureWorkbenchReport {
    pub schema_version: String,
    pub workbench_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: PublicLiteratureWorkbenchQuery,
    pub lanes: Vec<PublicLiteratureWorkbenchLane>,
    pub specialty_count: usize,
    pub non_empty_lane_count: usize,
    pub empty_lane_specialties: Vec<Specialty>,
    pub total_record_count: usize,
    pub total_review_issue_count: usize,
    pub omitted_review_issue_count: usize,
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

impl PublicLiteratureWorkbenchReport {
    /// Validate a persisted reviewer workbench without fetching or interpreting records.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        let specialties = selected_specialties(&self.query)?;
        if self.schema_version != PUBLIC_LITERATURE_WORKBENCH_SCHEMA_VERSION
            || !is_sha256_hex(&self.workbench_digest)
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
                lane.profile.specialty != lane.specialty
                    || lane.source_ids.is_empty() && lane.record_count > 0
                    || lane
                        .source_ids
                        .windows(2)
                        .any(|window| window[0] >= window[1])
                    || lane.abstract_count > lane.record_count
                    || lane.abstract_truncated_count > lane.abstract_count
                    || lane.missing_doi_count > lane.record_count
                    || lane.missing_abstract_count > lane.record_count
                    || lane.empty_publication_type_count > lane.record_count
                    || lane.empty_mesh_term_count > lane.record_count
                    || lane.review_issue_count < lane.omitted_review_issue_count
                    || lane.truncated != (lane.omitted_review_issue_count > 0)
                    || !is_sha256_hex(&lane.integrity_audit_digest)
                    || lane.design_strata.iter().any(|stratum| {
                        stratum.record_count != stratum.pmids.len()
                            || stratum
                                .pmids
                                .windows(2)
                                .any(|window| window[0] >= window[1])
                    })
            })
            || self.non_empty_lane_count
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.record_count > 0)
                    .count()
            || self.empty_lane_specialties
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.record_count == 0)
                    .map(|lane| lane.specialty)
                    .collect::<Vec<_>>()
            || self.total_record_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.record_count)
                    .fold(0usize, usize::saturating_add)
            || self.total_review_issue_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.review_issue_count)
                    .fold(0usize, usize::saturating_add)
            || self.omitted_review_issue_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.omitted_review_issue_count)
                    .fold(0usize, usize::saturating_add)
            || self.truncated_lane_count != self.lanes.iter().filter(|lane| lane.truncated).count()
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature workbench envelope is invalid".to_string(),
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
                    reason: "public-literature workbench freshness binding is invalid".to_string(),
                });
            }
        } else if self.query.freshness.is_some() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature workbench freshness query is missing its report"
                    .to_string(),
            });
        }
        if self.workbench_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature workbench digest does not match its contents"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the workbench from the exact validated public-literature snapshot and query.
    pub fn validate_for_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.specialty_workbench(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "public-literature workbench does not replay to the exact supplied snapshot"
                        .to_string(),
            });
        }
        Ok(())
    }
}

impl PublicLiteratureBundle {
    /// Join specialty protocols to exact real-bundle coverage and explicit integrity obligations.
    pub fn specialty_workbench(
        &self,
        query: &PublicLiteratureWorkbenchQuery,
    ) -> Result<PublicLiteratureWorkbenchReport, NeurosurgeryError> {
        validate_query(query)?;
        self.validate()?;
        let specialties = selected_specialties(query)?;
        let summary = self.summary()?;
        let selected_source_ids = self
            .records
            .iter()
            .filter(|record| specialties.contains(&record.specialty))
            .map(|record| record.source_id.clone())
            .collect::<BTreeSet<_>>();
        if let Some(source_id) = query
            .freshness
            .as_ref()
            .and_then(|freshness| freshness.source_id.as_deref())
        {
            if !selected_source_ids.contains(source_id) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "workbench freshness source_id {source_id:?} is outside the selected specialty lanes"
                    ),
                });
            }
        }

        let mut lanes = Vec::with_capacity(specialties.len());
        for specialty in specialties.iter().copied() {
            let integrity = self.integrity_audit(&PublicLiteratureIntegrityAuditQuery {
                specialties: Some(vec![specialty]),
                max_issues: query.max_issues_per_lane,
            })?;
            let records = self
                .records
                .iter()
                .filter(|record| record.specialty == specialty)
                .collect::<Vec<_>>();
            let (
                design_strata,
                unclassified_design_count,
                overlapping_design_count,
                design_review_pmids,
            ) = design_projection(&records);
            let source_ids = records
                .iter()
                .map(|record| record.source_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let review_issue_count = integrity.issues.len() + integrity.omitted_issue_count;
            lanes.push(PublicLiteratureWorkbenchLane {
                specialty,
                profile: specialty.profile(),
                source_ids,
                record_count: records.len(),
                abstract_count: records
                    .iter()
                    .filter(|record| record.abstract_text.is_some())
                    .count(),
                abstract_truncated_count: records
                    .iter()
                    .filter(|record| record.abstract_truncated)
                    .count(),
                missing_doi_count: integrity.counts.missing_doi_count,
                missing_abstract_count: integrity.counts.missing_abstract_count,
                empty_publication_type_count: integrity.counts.empty_publication_type_count,
                empty_mesh_term_count: integrity.counts.empty_mesh_term_count,
                review_issue_count,
                omitted_review_issue_count: integrity.omitted_issue_count,
                truncated: integrity.truncated,
                integrity_audit_digest: integrity.audit_digest,
                review_reasons: integrity.review_reasons,
                design_strata,
                unclassified_design_count,
                overlapping_design_count,
                design_review_pmids,
            });
        }

        let freshness = query
            .freshness
            .as_ref()
            .map(|freshness| self.freshness_report(freshness))
            .transpose()?;
        let empty_lane_specialties = lanes
            .iter()
            .filter(|lane| lane.record_count == 0)
            .map(|lane| lane.specialty)
            .collect::<Vec<_>>();
        let non_empty_lane_count = lanes.len() - empty_lane_specialties.len();
        let total_record_count = lanes.iter().map(|lane| lane.record_count).sum();
        let total_review_issue_count = lanes.iter().map(|lane| lane.review_issue_count).sum();
        let omitted_review_issue_count = lanes
            .iter()
            .map(|lane| lane.omitted_review_issue_count)
            .sum();
        let truncated_lane_count = lanes.iter().filter(|lane| lane.truncated).count();
        let mut report = PublicLiteratureWorkbenchReport {
            schema_version: PUBLIC_LITERATURE_WORKBENCH_SCHEMA_VERSION.to_string(),
            workbench_digest: String::new(),
            bundle_digest: summary.bundle_digest.clone(),
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            lanes,
            specialty_count: specialties.len(),
            non_empty_lane_count,
            empty_lane_specialties,
            total_record_count,
            total_review_issue_count,
            omitted_review_issue_count,
            truncated_lane_count,
            freshness,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the workbench is a coverage and reviewer-navigation projection; it does not rank specialties, score evidence, infer biology, or make a clinical conclusion".to_string(),
                "profile axes and evidence questions describe what a qualified reviewer should inspect, not diagnostic criteria, treatment thresholds, or operative instructions".to_string(),
                "record counts and metadata gaps are properties of this validated snapshot and must not be generalized to an unseen population or patient".to_string(),
                "missing or truncated metadata remains unknown; no field is imputed, repaired, deduplicated, or treated as negative evidence".to_string(),
                "the report never fetches URLs, invokes a provider, opens credentials, stores patient files, or writes durable state".to_string(),
            ],
        };
        report.workbench_digest = digest_report(&report)?;
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &PublicLiteratureWorkbenchQuery) -> Result<(), NeurosurgeryError> {
    if query.max_issues_per_lane == 0 || query.max_issues_per_lane > MAX_ISSUES_PER_LANE {
        return Err(NeurosurgeryError::TooMany {
            field: "public_literature_workbench.max_issues_per_lane",
            found: query.max_issues_per_lane,
            max: MAX_ISSUES_PER_LANE,
        });
    }
    if let Some(specialties) = &query.specialties {
        if specialties.is_empty() || specialties.len() > MAX_SPECIALTIES {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "public-literature workbench specialties must contain 1..={MAX_SPECIALTIES} lanes"
                ),
            });
        }
        let mut seen = BTreeSet::new();
        if specialties.iter().any(|specialty| !seen.insert(*specialty)) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature workbench specialties must be unique".to_string(),
            });
        }
    }
    Ok(())
}

fn selected_specialties(
    query: &PublicLiteratureWorkbenchQuery,
) -> Result<Vec<Specialty>, NeurosurgeryError> {
    let mut specialties = query
        .specialties
        .clone()
        .unwrap_or_else(|| Specialty::ALL.to_vec());
    if specialties.is_empty() || specialties.len() > MAX_SPECIALTIES {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!(
                "public-literature workbench specialties must contain 1..={MAX_SPECIALTIES} lanes"
            ),
        });
    }
    specialties.sort_unstable();
    Ok(specialties)
}

fn design_projection(
    records: &[&PublicLiteratureRecord],
) -> (
    Vec<PublicLiteratureDesignStratumCount>,
    usize,
    usize,
    Vec<String>,
) {
    let mut grouped = BTreeSet::<(PublicLiteratureDesignStratum, String)>::new();
    let mut unclassified = 0;
    let mut overlapping = 0;
    let mut review_pmids = BTreeSet::new();
    for record in records {
        let strata = classify_design(record);
        if strata.is_empty() {
            unclassified += 1;
            review_pmids.insert(record.pmid.clone());
        } else if strata.len() > 1 {
            overlapping += 1;
            review_pmids.insert(record.pmid.clone());
        }
        for stratum in strata {
            grouped.insert((stratum, record.pmid.clone()));
        }
    }
    let mut counts = BTreeMap::<PublicLiteratureDesignStratum, Vec<String>>::new();
    for (stratum, pmid) in grouped {
        counts.entry(stratum).or_default().push(pmid);
    }
    let strata = counts
        .into_iter()
        .map(|(stratum, mut pmids)| {
            pmids.sort();
            PublicLiteratureDesignStratumCount {
                stratum,
                record_count: pmids.len(),
                pmids,
            }
        })
        .collect();
    (
        strata,
        unclassified,
        overlapping,
        review_pmids.into_iter().collect(),
    )
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

fn classify_design(record: &PublicLiteratureRecord) -> BTreeSet<PublicLiteratureDesignStratum> {
    let searchable = format!(
        "{} {} {} {} {}",
        record.title,
        record.journal,
        record.abstract_text.as_deref().unwrap_or_default(),
        record.publication_types.join(" "),
        record.mesh_terms.join(" "),
    )
    .to_ascii_lowercase();
    let has = |terms: &[&str]| terms.iter().any(|term| searchable.contains(term));
    let mut strata = BTreeSet::new();
    if has(&["humans", "human", "patients", "patient", "adult", "child"]) {
        strata.insert(PublicLiteratureDesignStratum::HumanIndexed);
    }
    if has(&[
        "animals", "animal", "mice", "mouse", "rats", "murine", "canine",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::AnimalPreclinical);
    }
    if has(&[
        "in vitro",
        "cell line",
        "cultured",
        "spheroid",
        "organoid",
        "xenograft",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::InVitroOrCellLine);
    }
    if has(&[
        "review",
        "meta-analysis",
        "systematic review",
        "scoping review",
        "literature review",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::ReviewOrSynthesis);
    }
    if has(&[
        "mri",
        "magnetic resonance",
        "imaging",
        "radiolog",
        "diagnostic",
        "spectroscopy",
        "perfusion",
        "diffusion",
        "computed tomography",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::ImagingOrDiagnostic);
    }
    if has(&[
        "surgery",
        "surgical",
        "decompression",
        "repair",
        "reconstruction",
        "approach",
        "endoscopic",
        "craniotomy",
        "shunt",
        "correction",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::SurgicalOrProcedural);
    }
    if has(&[
        "genetic",
        "mutation",
        "syndrome",
        "developmental",
        "congenital",
        "suture",
        "craniosynostosis",
        "spina bifida",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::DevelopmentalOrGenetic);
    }
    if has(&[
        "survival",
        "outcome",
        "follow-up",
        "prognosis",
        "quality of life",
        "functional",
        "disability",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::OutcomeOrFollowUp);
    }
    if has(&[
        "clinical trial",
        "randomized",
        "prospective",
        "intervention",
        "treatment",
        "therapy",
    ]) {
        strata.insert(PublicLiteratureDesignStratum::InterventionalStudy);
    }
    strata
}

fn digest_report(report: &PublicLiteratureWorkbenchReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.workbench_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
