//! Marker-level grounding for typed glioma molecular evidence.
//!
//! This module searches only caller-supplied, validated public snapshots. A marker match is a
//! source-addressable retrieval observation, never a molecular diagnosis, tumour class, grade,
//! prognosis, treatment, or operative recommendation. Missing or conflicting caller calls stay
//! explicit and a zero-hit search is never reported as negative evidence.
//! Reports are self-validating and can be rebuilt against the exact request and local snapshots
//! before a caller persists or hands them to another worker.

use crate::evidence_synthesis::{EvidenceSynthesisPlane, EvidenceSynthesisReference};
use crate::{
    CaseRequest, GliomaEvidenceState, GliomaMarker, GliomaMolecularPanel, GliomaMolecularSummary,
    NeurosurgeryError, PublicLiteratureBundle, PublicLiteratureQuery, RealDataFreshnessQuery,
    RealDataFreshnessReport, RealDataQuery, RealDataQueryHit, RealGliomaBundle, Specialty,
    ToolCapability,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const GLIOMA_MOLECULAR_MAP_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-glioma-molecular-map/0.1";
pub const MAX_GLIOMA_MOLECULAR_MAP_MARKERS: usize = GliomaMarker::ALL.len();
pub const MAX_GLIOMA_MOLECULAR_MAP_HITS_PER_MARKER: usize = 32;
pub const MAX_GLIOMA_MOLECULAR_MAP_REFERENCES: usize = 256;
const DEFAULT_HITS_PER_MARKER: usize = 8;
const DEFAULT_REFERENCES: usize = 128;

fn default_hits_per_marker() -> usize {
    DEFAULT_HITS_PER_MARKER
}

fn default_references() -> usize {
    DEFAULT_REFERENCES
}

/// Bounded marker grounding controls. Text filters on the nested bundle queries are replaced by
/// each marker's controlled search term; all non-text facets remain caller-owned and intact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMolecularMapQuery {
    #[serde(default)]
    pub markers: Option<Vec<GliomaMarker>>,
    #[serde(default)]
    pub real_data_query: Option<RealDataQuery>,
    #[serde(default)]
    pub public_literature_query: Option<PublicLiteratureQuery>,
    #[serde(default)]
    pub freshness: Option<RealDataFreshnessQuery>,
    #[serde(default = "default_hits_per_marker")]
    pub max_hits_per_marker: usize,
    #[serde(default = "default_references")]
    pub max_references: usize,
    #[serde(default)]
    pub include_source_text: bool,
}

impl Default for GliomaMolecularMapQuery {
    fn default() -> Self {
        Self {
            markers: None,
            real_data_query: None,
            public_literature_query: None,
            freshness: None,
            max_hits_per_marker: default_hits_per_marker(),
            max_references: default_references(),
            include_source_text: false,
        }
    }
}

/// One marker's typed caller state plus exact source identifiers found in the local bundles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMolecularMarkerEvidence {
    pub marker: GliomaMarker,
    pub state: GliomaEvidenceState,
    pub assay_present: bool,
    pub specimen_present: bool,
    pub provenance_present: bool,
    pub provenance_complete: bool,
    pub observed_at_present: bool,
    pub search_terms: Vec<String>,
    pub real_total_matches: usize,
    pub real_returned_matches: usize,
    pub real_truncated: bool,
    pub public_total_matches: usize,
    pub public_returned_matches: usize,
    pub public_truncated: bool,
    pub reference_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_reasons: Vec<String>,
}

/// A deterministic review obligation for marker missingness, source absence, or bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMolecularMapReviewItem {
    pub code: String,
    pub marker: Option<GliomaMarker>,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_ids: Vec<String>,
}

/// Marker-level, source-addressable grounding report. It contains retrieval metadata only; it
/// intentionally has no evidence score or patient-level interpretation field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMolecularEvidenceMapReport {
    pub schema_version: String,
    pub map_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub generated_at: String,
    pub query: GliomaMolecularMapQuery,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<GliomaMolecularSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_freshness: Option<RealDataFreshnessReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_freshness: Option<RealDataFreshnessReport>,
    pub markers: Vec<GliomaMolecularMarkerEvidence>,
    pub references: Vec<EvidenceSynthesisReference>,
    pub review_items: Vec<GliomaMolecularMapReviewItem>,
    pub reviewer_roles: Vec<String>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl GliomaMolecularEvidenceMapReport {
    /// Validate the self-contained integrity contract before a report crosses a tool or mission
    /// boundary.  The digest is deliberately reproducible rather than secret-signed: callers can
    /// independently recompute it and detect accidental or adversarial mutation of the report.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != GLIOMA_MOLECULAR_MAP_SCHEMA_VERSION {
            return Err(NeurosurgeryError::UnsupportedSchema {
                found: self.schema_version.clone(),
                expected: GLIOMA_MOLECULAR_MAP_SCHEMA_VERSION,
            });
        }
        if !is_sha256_hex(&self.map_digest) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map digest must be a 64-character SHA-256 hex value"
                    .to_string(),
            });
        }
        if !is_sha256_hex(&self.request_digest) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "glioma molecular map request digest must be a 64-character SHA-256 hex value"
                        .to_string(),
            });
        }
        if self.specialty != Specialty::Glioma {
            return Err(NeurosurgeryError::RealDataSpecialtyUnsupported {
                specialty: self.specialty,
            });
        }
        if self.generated_at.trim().is_empty() {
            return Err(NeurosurgeryError::EmptyField {
                field: "glioma_molecular_map.generated_at",
            });
        }
        validate_query(&self.query)?;
        let expected_markers = self
            .query
            .markers
            .clone()
            .unwrap_or_else(|| GliomaMarker::ALL.to_vec());
        if self.markers.len() != expected_markers.len()
            || self
                .markers
                .iter()
                .map(|row| row.marker)
                .ne(expected_markers.iter().copied())
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map marker rows do not match the requested marker set"
                    .to_string(),
            });
        }
        for row in &self.markers {
            let expected_terms = marker_search_terms(row.marker)
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            if row.search_terms != expected_terms {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "glioma molecular map search terms are not canonical for {}",
                        row.marker.label()
                    ),
                });
            }
            if row.real_returned_matches > row.real_total_matches
                || row.public_returned_matches > row.public_total_matches
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "glioma molecular map returned-match counts exceed totals for {}",
                        row.marker.label()
                    ),
                });
            }
            if !is_sorted_unique(&row.reference_ids) || !is_sorted_unique(&row.review_reasons) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "glioma molecular map marker metadata is not canonical for {}",
                        row.marker.label()
                    ),
                });
            }
        }
        if self.references.len() > MAX_GLIOMA_MOLECULAR_MAP_REFERENCES
            || self.references.len() > crate::evidence_synthesis::MAX_EVIDENCE_SYNTHESIS_REFERENCES
        {
            return Err(NeurosurgeryError::TooMany {
                field: "glioma_molecular_map.references",
                found: self.references.len(),
                max: MAX_GLIOMA_MOLECULAR_MAP_REFERENCES
                    .min(crate::evidence_synthesis::MAX_EVIDENCE_SYNTHESIS_REFERENCES),
            });
        }
        let mut previous_reference = None;
        for reference in &self.references {
            if reference.record_id.trim().is_empty()
                || reference.record_kind.trim().is_empty()
                || reference.title.trim().is_empty()
                || reference.citation.trim().is_empty()
                || reference.source_id.as_deref().is_none_or(str::is_empty)
                || reference.source_uri.as_deref().is_none_or(str::is_empty)
                || !reference
                    .supports
                    .contains(&ToolCapability::MolecularContext)
            {
                return Err(NeurosurgeryError::InvalidEvidence {
                    id: reference.record_id.clone(),
                });
            }
            if !matches!(
                reference.plane,
                EvidenceSynthesisPlane::RealGliomaPopulation
                    | EvidenceSynthesisPlane::PublicLiterature
            ) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "glioma molecular map references must come from a public evidence plane"
                            .to_string(),
                });
            }
            if reference.plane == EvidenceSynthesisPlane::PublicLiterature
                && reference.record_uri.as_deref().is_none_or(str::is_empty)
            {
                return Err(NeurosurgeryError::InvalidEvidence {
                    id: reference.record_id.clone(),
                });
            }
            let key = (
                reference.plane,
                reference.record_kind.as_str(),
                reference.record_id.as_str(),
            );
            if previous_reference.is_some_and(|previous| previous >= key) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "glioma molecular map references must be sorted and unique".to_string(),
                });
            }
            previous_reference = Some(key);
        }
        if self.review_items.len() > crate::evidence_synthesis::MAX_EVIDENCE_SYNTHESIS_REVIEW_ITEMS
        {
            return Err(NeurosurgeryError::TooMany {
                field: "glioma_molecular_map.review_items",
                found: self.review_items.len(),
                max: crate::evidence_synthesis::MAX_EVIDENCE_SYNTHESIS_REVIEW_ITEMS,
            });
        }
        if self.review_items.iter().any(|item| {
            item.code.trim().is_empty()
                || item.detail.trim().is_empty()
                || !is_sorted_unique(&item.reference_ids)
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map review items are not canonical".to_string(),
            });
        }
        if self.reviewer_roles.is_empty()
            || !self.provenance_bound
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map provider-free provenance contract is invalid"
                    .to_string(),
            });
        }
        for digest in [&self.real_data_digest, &self.public_literature_digest] {
            if digest.as_deref().is_some_and(|value| !is_sha256_hex(value)) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "glioma molecular map bundle digests must be SHA-256 hex values"
                        .to_string(),
                });
            }
        }
        if self.real_data_freshness.as_ref().is_some_and(|freshness| {
            self.real_data_digest.as_deref() != Some(freshness.bundle_digest.as_str())
        }) || self
            .public_literature_freshness
            .as_ref()
            .is_some_and(|freshness| {
                self.public_literature_digest.as_deref() != Some(freshness.bundle_digest.as_str())
            })
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map freshness reports must bind their bundle digest"
                    .to_string(),
            });
        }
        let expected_digest = digest_report(self)?;
        if self.map_digest != expected_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map digest does not match its report contents"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the map from the exact request and caller-supplied snapshots and compare every
    /// field. This protects mission audits from a validly-shaped report rebound to another case
    /// or another real-data snapshot.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = map_molecular_evidence(request, real_data, public_literature, &self.query)?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map is not bound to the supplied request and snapshots"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// Search the supplied snapshots for each requested marker and preserve every evidence plane.
pub fn map_molecular_evidence(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &GliomaMolecularMapQuery,
) -> Result<GliomaMolecularEvidenceMapReport, NeurosurgeryError> {
    if request.specialty != Specialty::Glioma {
        return Err(NeurosurgeryError::RealDataSpecialtyUnsupported {
            specialty: request.specialty,
        });
    }
    validate_query(query)?;
    if request.request_use == crate::RequestUse::SyntheticCaseSimulation
        && (real_data.is_some() || public_literature.is_some())
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "synthetic_case_simulation cannot be combined with public evidence".to_string(),
        });
    }
    if query.real_data_query.is_some() && real_data.is_none() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "real_data_query requires a validated real glioma bundle".to_string(),
        });
    }
    if query.public_literature_query.is_some() && public_literature.is_none() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "public_literature_query requires a validated public-literature bundle"
                .to_string(),
        });
    }
    if let Some(public_query) = &query.public_literature_query {
        if public_query
            .specialty
            .is_some_and(|specialty| specialty != Specialty::Glioma)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map public query must select the glioma lane".to_string(),
            });
        }
    }

    let panel = request
        .glioma_molecular
        .as_ref()
        .map(GliomaMolecularPanel::summary)
        .transpose()?;
    let panel_status = panel
        .as_ref()
        .map(|summary| {
            summary
                .markers
                .iter()
                .map(|status| (status.marker, status))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    if let Some(data) = real_data {
        data.validate()?;
    }
    if let Some(literature) = public_literature {
        literature.validate()?;
        if !literature.has_specialty(Specialty::Glioma) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public-literature bundle has no glioma records".to_string(),
            });
        }
    }

    let real_data_freshness = match (real_data, query.freshness.as_ref()) {
        (Some(data), Some(freshness)) => Some(data.freshness_report(freshness)?),
        _ => None,
    };
    let public_literature_freshness = match (public_literature, query.freshness.as_ref()) {
        (Some(literature), Some(freshness)) => Some(literature.freshness_report(freshness)?),
        _ => None,
    };

    let markers = query
        .markers
        .clone()
        .unwrap_or_else(|| GliomaMarker::ALL.to_vec());
    let mut references = Vec::new();
    let mut reference_keys = BTreeSet::new();
    let mut review_items = Vec::new();
    let mut marker_reports = Vec::with_capacity(markers.len());

    for marker in markers {
        let search_terms = marker_search_terms(marker)
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let status = panel_status.get(&marker).copied();
        let state = status.map_or(GliomaEvidenceState::NotCollected, |status| status.state);
        let mut real_hits = Vec::new();
        let mut public_hits = Vec::new();
        let mut real_total_matches: usize = 0;
        let mut public_total_matches: usize = 0;
        let mut real_truncated = false;
        let mut public_truncated = false;

        if let Some(data) = real_data {
            for term in &search_terms {
                let mut marker_query = query.real_data_query.clone().unwrap_or_default();
                marker_query.text = Some(term.clone());
                marker_query.limit = query.max_hits_per_marker;
                let result = data.query(&marker_query)?;
                real_total_matches = real_total_matches.saturating_add(result.total_matches);
                real_truncated |= result.truncated;
                real_hits.extend(result.hits);
            }
        }
        if let Some(literature) = public_literature {
            for term in &search_terms {
                let mut marker_query = query.public_literature_query.clone().unwrap_or_default();
                marker_query.specialty = Some(Specialty::Glioma);
                marker_query.text = Some(term.clone());
                marker_query.limit = query.max_hits_per_marker;
                let result = literature.query(&marker_query)?;
                public_total_matches = public_total_matches.saturating_add(result.total_matches);
                public_truncated |= result.truncated;
                public_hits.extend(result.hits);
            }
        }

        let mut source_reference_ids = Vec::new();
        for hit in real_hits {
            let key = (
                EvidenceSynthesisPlane::RealGliomaPopulation,
                hit.record_id.clone(),
            );
            source_reference_ids.push(hit.record_id.clone());
            if reference_keys.insert(key) {
                references.push(molecular_real_reference(&hit));
            }
        }
        for hit in public_hits {
            let record_id = format!("PMID-{}", hit.pmid);
            let key = (EvidenceSynthesisPlane::PublicLiterature, record_id.clone());
            source_reference_ids.push(record_id.clone());
            if reference_keys.insert(key) {
                references.push(molecular_public_reference(&hit, query.include_source_text));
            }
        }
        source_reference_ids.sort();
        source_reference_ids.dedup();

        let mut reasons = Vec::new();
        if request.glioma_molecular.is_none() {
            reasons.push("caller molecular panel was not supplied".to_string());
            review_items.push(GliomaMolecularMapReviewItem {
                code: "molecular_panel_unattached".to_string(),
                marker: Some(marker),
                detail: format!(
                    "{} has no caller panel state; the local search is population/citation context only",
                    marker.label()
                ),
                reference_ids: source_reference_ids.clone(),
            });
        } else if state == GliomaEvidenceState::NotCollected {
            reasons.push("marker_not_collected".to_string());
            review_items.push(GliomaMolecularMapReviewItem {
                code: "marker_not_collected".to_string(),
                marker: Some(marker),
                detail: format!(
                    "{} was not collected in the caller panel; source hits do not impute a case value",
                    marker.label()
                ),
                reference_ids: source_reference_ids.clone(),
            });
        } else if matches!(
            state,
            GliomaEvidenceState::Uninterpretable | GliomaEvidenceState::Conflicting
        ) {
            reasons.push("marker_state_requires_review".to_string());
            review_items.push(GliomaMolecularMapReviewItem {
                code: "marker_state_requires_review".to_string(),
                marker: Some(marker),
                detail: format!(
                    "{} remains {:?} in the caller panel; the map does not resolve the state",
                    marker.label(),
                    state
                ),
                reference_ids: source_reference_ids.clone(),
            });
        }
        if status.is_some_and(|status| !status.provenance_complete) {
            reasons.push("marker_provenance_incomplete".to_string());
            review_items.push(GliomaMolecularMapReviewItem {
                code: "marker_provenance_incomplete".to_string(),
                marker: Some(marker),
                detail: format!(
                    "{} has a caller state but is missing assay, specimen, or source provenance",
                    marker.label()
                ),
                reference_ids: source_reference_ids.clone(),
            });
        }
        if real_data.is_none() {
            reasons.push("real_population_unattached".to_string());
        }
        if public_literature.is_none() {
            reasons.push("public_literature_unattached".to_string());
        }
        if real_data.is_some() && real_total_matches == 0 {
            reasons.push("no_real_local_marker_match".to_string());
            review_items.push(GliomaMolecularMapReviewItem {
                code: "no_real_local_marker_match".to_string(),
                marker: Some(marker),
                detail: format!(
                    "no real snapshot record matched the controlled search terms for {}; this is not negative evidence",
                    marker.label()
                ),
                reference_ids: source_reference_ids.clone(),
            });
        }
        if public_literature.is_some() && public_total_matches == 0 {
            reasons.push("no_public_local_marker_match".to_string());
            review_items.push(GliomaMolecularMapReviewItem {
                code: "no_public_local_marker_match".to_string(),
                marker: Some(marker),
                detail: format!(
                    "no PubMed snapshot record matched the controlled search terms for {}; this is not negative evidence",
                    marker.label()
                ),
                reference_ids: source_reference_ids.clone(),
            });
        }
        if real_truncated || public_truncated {
            reasons.push("marker_query_truncated".to_string());
            review_items.push(GliomaMolecularMapReviewItem {
                code: "marker_query_truncated".to_string(),
                marker: Some(marker),
                detail: format!(
                    "one or more bounded local searches for {} matched more records than were returned",
                    marker.label()
                ),
                reference_ids: source_reference_ids.clone(),
            });
        }
        reasons.sort();
        reasons.dedup();
        marker_reports.push(GliomaMolecularMarkerEvidence {
            marker,
            state,
            assay_present: status.is_some_and(|status| status.assay_present),
            specimen_present: status.is_some_and(|status| status.specimen_present),
            provenance_present: status.is_some_and(|status| status.provenance_present),
            provenance_complete: status.is_some_and(|status| status.provenance_complete),
            observed_at_present: status.is_some_and(|status| status.observed_at_present),
            search_terms,
            real_total_matches,
            real_returned_matches: source_reference_ids
                .iter()
                .filter(|id| !id.starts_with("PMID-"))
                .count(),
            real_truncated,
            public_total_matches,
            public_returned_matches: source_reference_ids
                .iter()
                .filter(|id| id.starts_with("PMID-"))
                .count(),
            public_truncated,
            reference_ids: source_reference_ids,
            review_reasons: reasons,
        });
    }

    references.sort_by(|left, right| {
        left.plane
            .cmp(&right.plane)
            .then(left.record_kind.cmp(&right.record_kind))
            .then(left.record_id.cmp(&right.record_id))
    });
    if references.len() > query.max_references {
        review_items.push(GliomaMolecularMapReviewItem {
            code: "reference_projection_truncated".to_string(),
            marker: None,
            detail: format!(
                "{} unique marker reference rows were available, but only {} are emitted",
                references.len(),
                query.max_references
            ),
            reference_ids: Vec::new(),
        });
        references.truncate(query.max_references);
    }
    review_items.truncate(crate::evidence_synthesis::MAX_EVIDENCE_SYNTHESIS_REVIEW_ITEMS);

    let request_digest = digest_request(request)?;
    let generated_at = real_data
        .map(|data| data.generated_at.clone())
        .or_else(|| public_literature.map(|literature| literature.generated_at.clone()))
        .unwrap_or_else(|| "case-only-no-public-snapshot".to_string());
    let mut report = GliomaMolecularEvidenceMapReport {
        schema_version: GLIOMA_MOLECULAR_MAP_SCHEMA_VERSION.to_string(),
        map_digest: String::new(),
        request_digest,
        specialty: Specialty::Glioma,
        generated_at,
        query: query.clone(),
        panel,
        real_data_digest: real_data
            .map(|data| data.summary())
            .transpose()?
            .map(|summary| summary.bundle_digest),
        public_literature_digest: public_literature
            .map(|literature| literature.summary())
            .transpose()?
            .map(|summary| summary.bundle_digest),
        real_data_freshness,
        public_literature_freshness,
        markers: marker_reports,
        references,
        review_items,
        reviewer_roles: Specialty::Glioma.profile().human_review_roles,
        provenance_bound: true,
        synthetic_data: request.request_use == crate::RequestUse::SyntheticCaseSimulation,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "controlled marker searches return source metadata only; a match does not establish a case mutation, diagnosis, grade, prognosis, treatment response, or operative action".to_string(),
            "a zero-hit search is not negative evidence and does not prove that a marker is absent from the wider literature or population".to_string(),
            "real and PubMed bundles remain independent evidence planes; exact identifiers do not establish cohort identity, applicability, causality, or evidence quality".to_string(),
            "caller panel states, assay/specimen provenance, and conflicts remain explicit review work; no marker value is imputed".to_string(),
            "the map never fetches URLs, invokes a provider, opens credentials, accesses patient files, or performs an external effect".to_string(),
        ],
    };
    report.map_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn validate_query(query: &GliomaMolecularMapQuery) -> Result<(), NeurosurgeryError> {
    if let Some(markers) = &query.markers {
        if markers.is_empty() || markers.len() > MAX_GLIOMA_MOLECULAR_MAP_MARKERS {
            return Err(NeurosurgeryError::TooMany {
                field: "glioma_molecular_map.markers",
                found: markers.len(),
                max: MAX_GLIOMA_MOLECULAR_MAP_MARKERS,
            });
        }
        let mut unique = BTreeSet::new();
        if markers.iter().any(|marker| !unique.insert(*marker)) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "glioma molecular map markers must be unique".to_string(),
            });
        }
    }
    if !(1..=MAX_GLIOMA_MOLECULAR_MAP_HITS_PER_MARKER).contains(&query.max_hits_per_marker) {
        return Err(NeurosurgeryError::TooMany {
            field: "glioma_molecular_map.max_hits_per_marker",
            found: query.max_hits_per_marker,
            max: MAX_GLIOMA_MOLECULAR_MAP_HITS_PER_MARKER,
        });
    }
    if !(1..=MAX_GLIOMA_MOLECULAR_MAP_REFERENCES).contains(&query.max_references) {
        return Err(NeurosurgeryError::TooMany {
            field: "glioma_molecular_map.max_references",
            found: query.max_references,
            max: MAX_GLIOMA_MOLECULAR_MAP_REFERENCES,
        });
    }
    Ok(())
}

fn marker_search_terms(marker: GliomaMarker) -> Vec<&'static str> {
    match marker {
        GliomaMarker::Idh1Mutation => vec!["IDH1", "isocitrate dehydrogenase 1"],
        GliomaMarker::Idh2Mutation => vec!["IDH2", "isocitrate dehydrogenase 2"],
        GliomaMarker::Codeletion1p19q => vec!["1p/19q", "1p 19q", "codeletion"],
        GliomaMarker::H3K27Alteration => vec!["H3 K27", "H3K27", "histone H3 K27"],
        GliomaMarker::H3G34Mutation => vec!["H3 G34", "H3G34", "histone H3 G34"],
        GliomaMarker::MgmtPromoterMethylation => vec!["MGMT", "O6-methylguanine"],
        GliomaMarker::TertPromoterMutation => vec!["TERT", "telomerase reverse transcriptase"],
        GliomaMarker::EgfrAmplification => vec!["EGFR", "epidermal growth factor receptor"],
        GliomaMarker::Chromosome7Gain10Loss => vec!["chromosome 7", "chromosome 10"],
        GliomaMarker::Cdkna2bHomozygousDeletion => vec!["CDKN2A", "CDKN2B"],
        GliomaMarker::AtrxLoss => vec!["ATRX"],
        GliomaMarker::Tp53Mutation => vec!["TP53", "p53"],
        GliomaMarker::PtenLoss => vec!["PTEN"],
        GliomaMarker::BrafV600e => vec!["BRAF V600E", "BRAF"],
        GliomaMarker::NtrkFusion => vec!["NTRK", "neurotrophic receptor tyrosine kinase"],
        GliomaMarker::MismatchRepairDeficiency => vec!["mismatch repair", "MMR"],
        GliomaMarker::MethylationClassifier => vec!["methylation classifier", "methylome"],
        GliomaMarker::TumourMutationalBurden => {
            vec!["tumor mutational burden", "tumour mutational burden", "TMB"]
        }
    }
}

fn molecular_real_reference(hit: &RealDataQueryHit) -> EvidenceSynthesisReference {
    let mut reference = crate::evidence_synthesis::real_reference(hit);
    reference.supports.push(ToolCapability::MolecularContext);
    reference.supports.sort();
    reference.supports.dedup();
    reference
}

fn molecular_public_reference(
    hit: &crate::PublicLiteratureQueryHit,
    include_source_text: bool,
) -> EvidenceSynthesisReference {
    let mut reference = crate::evidence_synthesis::public_reference(hit, include_source_text);
    reference.supports.push(ToolCapability::MolecularContext);
    reference.supports.sort();
    reference.supports.dedup();
    reference
}

fn digest_request(request: &CaseRequest) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn digest_report(report: &GliomaMolecularEvidenceMapReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.map_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}
