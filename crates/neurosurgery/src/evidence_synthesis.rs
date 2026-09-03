//! Digest-bound evidence alignment for the provider-free neurosurgical agent.
//!
//! This module is the missing join between a de-identified case, the real glioma population
//! snapshot, and the six-lane PubMed snapshot.  It deliberately performs *alignment*, not
//! medical interpretation: observations remain case-plane metadata, public records remain
//! population/citation metadata, and exact identifiers are reported as links rather than cohort
//! or biological conclusions.  A local model or qualified reviewer can use the resulting ledger
//! as a bounded, source-addressable context without an API key.

use crate::evidence_audit::audit as audit_evidence;
use crate::glioma_molecular_map::{GliomaMolecularEvidenceMapReport, GliomaMolecularMapQuery};
use crate::{
    CaseAssetKind, CaseRequest, EvidenceAuditReport, EvidenceRecord, EvidenceState, EvidenceTier,
    LiteratureLinkAuditQuery, LiteratureLinkAuditReport, NeurosurgeryError, PublicLiteratureBundle,
    PublicLiteratureQuery, PublicLiteratureQueryHit, PublicLiteratureSummary,
    RealDataFreshnessQuery, RealDataFreshnessReport, RealDataQuery, RealDataQueryHit,
    RealDataRecordKind, RealDataSummary, RealGliomaBundle, Specialty, ToolCapability,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const EVIDENCE_SYNTHESIS_SCHEMA_VERSION: &str = "bioprism-neurosurgery-evidence-synthesis/0.1";
pub const MAX_EVIDENCE_SYNTHESIS_REFERENCES: usize = 256;
pub const MAX_EVIDENCE_SYNTHESIS_REVIEW_ITEMS: usize = 256;
const DEFAULT_EVIDENCE_SYNTHESIS_REFERENCES: usize = 64;

fn default_reference_limit() -> usize {
    DEFAULT_EVIDENCE_SYNTHESIS_REFERENCES
}

/// Which evidence plane supplied an item.  The planes are never silently merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSynthesisPlane {
    CaseObservation,
    CallerEvidence,
    RealGliomaPopulation,
    PublicLiterature,
}

/// Bounded controls for evidence alignment.  Source queries narrow already validated local
/// bundles; they never fetch a URL or infer a query from patient text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisQuery {
    #[serde(default)]
    pub real_data_query: Option<RealDataQuery>,
    #[serde(default)]
    pub public_literature_query: Option<PublicLiteratureQuery>,
    /// Optional explicit caller-owned retrieval-age policy applied to supplied bundles.
    #[serde(default)]
    pub freshness: Option<RealDataFreshnessQuery>,
    #[serde(default = "default_reference_limit")]
    pub max_references: usize,
    /// Include bounded source-text excerpts in public reference rows. Excerpts are untrusted
    /// source text and are never rewritten or interpreted here.
    #[serde(default)]
    pub include_source_text: bool,
}

/// Digest-bound multimodal coverage summary included in synthesis when a caller supplies a
/// validated real de-identified asset projection. The summary exposes coverage and review state
/// without importing asset bytes or raw identifiers into the synthesis ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisCaseAssetSummary {
    pub report_digest: String,
    pub asset_count: usize,
    pub observed_asset_count: usize,
    pub non_observed_asset_count: usize,
    pub provenance_complete_asset_count: usize,
    pub missing_requested_kinds: Vec<CaseAssetKind>,
    pub review_item_count: usize,
    pub omitted_review_item_count: usize,
    pub truncated: bool,
}

impl Default for EvidenceSynthesisQuery {
    fn default() -> Self {
        Self {
            real_data_query: None,
            public_literature_query: None,
            freshness: None,
            max_references: default_reference_limit(),
            include_source_text: false,
        }
    }
}

/// A redacted observation index.  The value and label stay in the caller-owned case request;
/// this row lets a reviewer trace coverage without echoing sensitive case text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisObservation {
    pub observation_digest: String,
    pub kind: crate::ObservationKind,
    pub status: crate::ObservationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timepoint: Option<String>,
}

/// One source-addressable population/citation or caller evidence row.  `supports` is the
/// caller/source-declared capability map; it is not a statement that the source proves a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisReference {
    pub plane: EvidenceSynthesisPlane,
    pub record_kind: String,
    pub record_id: String,
    pub title: String,
    pub citation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<EvidenceTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_record_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supports: Vec<ToolCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_text_excerpt: Option<String>,
}

/// A capability-specific alignment row. Counts describe the bounded input ledger only; there is
/// intentionally no evidence score, priority, diagnosis, prognosis, or treatment field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisLane {
    pub capability: ToolCapability,
    pub case_observation_count: usize,
    pub caller_evidence_count: usize,
    pub population_reference_count: usize,
    pub verified_reference_count: usize,
    pub unverified_reference_count: usize,
    pub reference_ids: Vec<String>,
    pub evidence_state: EvidenceState,
    pub reviewer_questions: Vec<String>,
}

/// A deterministic review obligation generated by missingness, truncation, or source separation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisReviewItem {
    pub code: String,
    pub scope: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_ids: Vec<String>,
}

/// One cross-bundle exact-identifier correspondence, retained as a compact digest-bound row.
/// This is intentionally not a scientific or cohort-overlap assertion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisLink {
    pub real_pmid: String,
    pub public_pmid: String,
    pub public_specialty: Specialty,
    pub match_kinds: Vec<crate::LiteratureLinkKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mismatched_fields: Vec<String>,
}

/// One source-aligned research handoff.  All fields are either copied public metadata, typed
/// case coverage, or deterministic linkage/audit output; no generated medical answer is present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisReport {
    pub schema_version: String,
    pub synthesis_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub generated_at: String,
    pub query: EvidenceSynthesisQuery,
    pub case_observations: Vec<EvidenceSynthesisObservation>,
    pub case_audit: EvidenceAuditReport,
    /// Digest of the optional real de-identified asset projection carried by the mission. This
    /// links provenance metadata into the ledger without importing or interpreting asset bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_report_digest: Option<String>,
    /// Bounded coverage/review projection for the optional real de-identified asset manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_summary: Option<EvidenceSynthesisCaseAssetSummary>,
    /// Digest-only asset review obligations copied from the validated projection. Asset
    /// references are hashes; no caller asset/source identifier or byte content is imported.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub case_asset_review_items: Vec<crate::CaseAssetReviewItem>,
    /// Digest of the reviewer-owned case-asset disposition ledger, when one was supplied.
    /// Counts are workflow state only; they do not establish that an asset was clinically
    /// interpreted or that a reviewer verified its contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_disposition_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_pending_item_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_resolved_decision_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_unresolved_decision_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glioma_molecular_map: Option<GliomaMolecularEvidenceMapReport>,
    pub references: Vec<EvidenceSynthesisReference>,
    pub lanes: Vec<EvidenceSynthesisLane>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_summary: Option<RealDataSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_freshness: Option<RealDataFreshnessReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_summary: Option<PublicLiteratureSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_freshness: Option<RealDataFreshnessReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literature_link_audit: Option<LiteratureLinkAuditReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<EvidenceSynthesisLink>,
    pub review_items: Vec<EvidenceSynthesisReviewItem>,
    pub reviewer_roles: Vec<String>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl EvidenceSynthesisReport {
    /// Validate a persisted synthesis envelope without needing the original source bundles.
    ///
    /// This is deliberately a structural check: it verifies digest shape, plane separation,
    /// count projections, and provider boundaries. `validate_for_inputs` below performs the
    /// stronger exact replay against the request and snapshots whenever those inputs are
    /// available.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != EVIDENCE_SYNTHESIS_SCHEMA_VERSION
            || !is_sha256_hex(&self.synthesis_digest)
            || !is_sha256_hex(&self.request_digest)
            || self.generated_at.trim().is_empty()
            || self.specialty != self.case_audit.specialty
            || self.request_digest != self.case_audit.request_digest
            || self.case_observations.len() != self.case_audit.temporal_alignment.observation_count
            || self.references.len() > self.query.max_references
            || !self.provenance_bound
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
        {
            return Err(invalid_report("evidence synthesis envelope is invalid"));
        }
        validate_query(&self.query)?;
        validate_case_audit(&self.case_audit, self.specialty)?;
        validate_observations(&self.case_observations)?;
        validate_asset_projection(self)?;
        validate_references(&self.references, self.query.max_references)?;
        validate_lanes(self)?;
        validate_source_summaries(self)?;
        validate_links(self)?;
        if self.review_items.len() > MAX_EVIDENCE_SYNTHESIS_REVIEW_ITEMS
            || self.review_items.iter().any(|item| {
                item.code.trim().is_empty()
                    || item.scope.trim().is_empty()
                    || item.detail.trim().is_empty()
                    || {
                        let mut seen = BTreeSet::new();
                        item.reference_ids
                            .iter()
                            .any(|reference_id| !seen.insert(reference_id))
                    }
            })
        {
            return Err(invalid_report(
                "evidence synthesis review projection is invalid",
            ));
        }
        let reference_ids = self
            .references
            .iter()
            .map(|reference| reference.record_id.as_str())
            .collect::<BTreeSet<_>>();
        if self.review_items.iter().any(|item| {
            item.reference_ids
                .iter()
                .any(|reference_id| !reference_ids.contains(reference_id.as_str()))
        }) {
            return Err(invalid_report(
                "evidence synthesis review item references an un-emitted row",
            ));
        }
        if self.synthesis_digest != digest_report(self)? {
            return Err(invalid_report(
                "evidence synthesis digest does not match its report contents",
            ));
        }
        Ok(())
    }

    /// Rebuild this ledger from the exact request, snapshots, asset projection, and disposition
    /// ledger. A report with a valid shape but rebound inputs therefore fails closed.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_assets: Option<&crate::CaseAssetManifestReport>,
        dispositions: Option<&crate::CaseAssetReviewDispositionReport>,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = synthesize_with_case_assets_and_dispositions(
            request,
            real_data,
            public_literature,
            &self.query,
            case_assets,
            dispositions,
        )?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence synthesis is not bound to the supplied request, snapshots, or asset review state".to_string(),
            });
        }
        Ok(())
    }
}

fn invalid_report(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

fn validate_case_audit(
    audit: &EvidenceAuditReport,
    specialty: Specialty,
) -> Result<(), NeurosurgeryError> {
    if audit.schema_version != crate::evidence_audit::EVIDENCE_AUDIT_SCHEMA_VERSION
        || audit.specialty != specialty
        || audit.required_observation_kinds
            != crate::evidence_audit::required_observation_kinds(specialty)
        || audit.items.len() != audit.required_observation_kinds.len()
        || audit
            .items
            .iter()
            .zip(audit.required_observation_kinds.iter())
            .any(|(item, kind)| {
                item.observation_kind != *kind
                    || !item.required_for_review
                    || item.provenance_complete_count > item.observed_count
                    || item.reviewer_note.trim().is_empty()
                    || item.state != expected_evidence_state(item)
            })
        || {
            let mut seen = BTreeSet::new();
            audit
                .missing_required_kinds
                .iter()
                .any(|kind| !audit.required_observation_kinds.contains(kind) || !seen.insert(kind))
        }
        || audit.provenance_gap_count
            != audit
                .items
                .iter()
                .map(|item| {
                    item.observed_count
                        .saturating_sub(item.provenance_complete_count)
                })
                .sum::<usize>()
        || audit.verified_evidence_count + audit.unverified_evidence_count
            > audit.evidence_record_count
        || audit.evidence_supporting_synthesis_count > audit.evidence_record_count
        || audit.coverage_complete
            != (audit.missing_required_kinds.is_empty() && audit.provenance_gap_count == 0)
        || !audit.human_review_required
        || audit.provider != "none"
        || audit.network
        || audit.effect != "read_only"
    {
        return Err(invalid_report(
            "evidence synthesis case audit is inconsistent",
        ));
    }
    let temporal = &audit.temporal_alignment;
    if temporal.schema_version != crate::temporal::TEMPORAL_ALIGNMENT_SCHEMA_VERSION
        || temporal.request_digest != audit.request_digest
        || temporal.specialty != specialty
        || temporal.observation_count != temporal.observations.len()
        || temporal.timestamped_observation_count + temporal.untimestamped_observation_count
            != temporal.observation_count
        || temporal
            .observations
            .iter()
            .enumerate()
            .any(|(index, observation)| {
                observation.observation_index != index
                    || observation.label != format!("observation-{index}")
            })
        || !temporal.human_review_required
        || temporal.provider != "none"
        || temporal.network
        || temporal.effect != "read_only"
    {
        return Err(invalid_report(
            "evidence synthesis temporal audit is inconsistent",
        ));
    }
    Ok(())
}

fn expected_evidence_state(item: &crate::EvidenceAuditItem) -> EvidenceState {
    if item.conflicting_count > 0 {
        EvidenceState::Conflicting
    } else if item.uninterpretable_count > 0 {
        EvidenceState::Uninterpretable
    } else if item.observed_count > 0 {
        EvidenceState::Measured
    } else {
        EvidenceState::Unmeasured
    }
}

fn validate_observations(
    observations: &[EvidenceSynthesisObservation],
) -> Result<(), NeurosurgeryError> {
    let mut digests = BTreeSet::new();
    for observation in observations {
        if !is_sha256_hex(&observation.observation_digest)
            || !digests.insert(observation.observation_digest.as_str())
            || observation
                .source_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || observation
                .observed_at
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || observation
                .timepoint
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(invalid_report(
                "evidence synthesis observation index is invalid",
            ));
        }
    }
    Ok(())
}

fn validate_asset_projection(report: &EvidenceSynthesisReport) -> Result<(), NeurosurgeryError> {
    let has_digest = report.case_asset_report_digest.is_some();
    if has_digest != report.case_asset_summary.is_some()
        || report
            .case_asset_report_digest
            .as_deref()
            .is_some_and(|digest| !is_sha256_hex(digest))
    {
        return Err(invalid_report(
            "evidence synthesis case-asset projection presence is inconsistent",
        ));
    }
    if let Some(summary) = report.case_asset_summary.as_ref() {
        if !is_sha256_hex(&summary.report_digest)
            || report.case_asset_report_digest.as_deref() != Some(summary.report_digest.as_str())
            || summary.observed_asset_count + summary.non_observed_asset_count
                != summary.asset_count
            || summary.provenance_complete_asset_count > summary.observed_asset_count
            || summary.review_item_count != report.case_asset_review_items.len()
            || summary.truncated != (summary.omitted_review_item_count > 0)
            || {
                let mut seen = BTreeSet::new();
                summary
                    .missing_requested_kinds
                    .iter()
                    .any(|kind| !seen.insert(kind))
            }
        {
            return Err(invalid_report(
                "evidence synthesis case-asset summary is inconsistent",
            ));
        }
        let mut sequences = BTreeSet::new();
        for item in &report.case_asset_review_items {
            if item.sequence == 0
                || !sequences.insert(item.sequence)
                || item.sequence as usize != sequences.len()
                || item
                    .asset_ref
                    .as_deref()
                    .is_some_and(|value| !is_sha256_hex(value))
                || item.code.trim().is_empty()
                || item.reason.trim().is_empty()
            {
                return Err(invalid_report(
                    "evidence synthesis case-asset review item is invalid",
                ));
            }
        }
    } else if !report.case_asset_review_items.is_empty() {
        return Err(invalid_report(
            "evidence synthesis has asset review items without an asset summary",
        ));
    }
    let disposition_present = report.case_asset_review_disposition_digest.is_some();
    let disposition_counts_present = report.case_asset_review_pending_item_count.is_some()
        && report.case_asset_review_resolved_decision_count.is_some()
        && report.case_asset_review_unresolved_decision_count.is_some();
    if disposition_present != disposition_counts_present
        || (disposition_present && !has_digest)
        || report
            .case_asset_review_disposition_digest
            .as_deref()
            .is_some_and(|digest| !is_sha256_hex(digest))
    {
        return Err(invalid_report(
            "evidence synthesis case-asset disposition projection is inconsistent",
        ));
    }
    if let (Some(summary), Some(pending), Some(resolved), Some(unresolved)) = (
        report.case_asset_summary.as_ref(),
        report.case_asset_review_pending_item_count,
        report.case_asset_review_resolved_decision_count,
        report.case_asset_review_unresolved_decision_count,
    ) {
        let candidate_count = summary
            .review_item_count
            .saturating_add(summary.omitted_review_item_count);
        if resolved + unresolved > candidate_count || pending > candidate_count {
            return Err(invalid_report(
                "evidence synthesis case-asset disposition counts exceed the projection",
            ));
        }
    }
    Ok(())
}

fn validate_references(
    references: &[EvidenceSynthesisReference],
    max_references: usize,
) -> Result<(), NeurosurgeryError> {
    if references.len() > max_references
        || references.windows(2).any(|window| {
            (
                window[0].plane,
                window[0].record_kind.as_str(),
                window[0].record_id.as_str(),
            ) >= (
                window[1].plane,
                window[1].record_kind.as_str(),
                window[1].record_id.as_str(),
            )
        })
    {
        return Err(invalid_report(
            "evidence synthesis references are not canonical",
        ));
    }
    for reference in references {
        if reference.record_kind.trim().is_empty()
            || reference.record_id.trim().is_empty()
            || reference.title.trim().is_empty()
            || reference.citation.trim().is_empty()
            || reference.tier.is_none()
            || reference
                .supports
                .windows(2)
                .any(|window| window[0] >= window[1])
            || reference
                .source_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || reference
                .source_uri
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || reference
                .record_uri
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(invalid_report(
                "evidence synthesis reference row is invalid",
            ));
        }
        match reference.plane {
            EvidenceSynthesisPlane::CallerEvidence => {
                if reference.record_kind != "caller_evidence"
                    || reference.source_id.is_some()
                    || reference.source_uri.is_some()
                    || reference.record_uri.is_some()
                {
                    return Err(invalid_report(
                        "caller evidence reference crosses its provenance plane",
                    ));
                }
            }
            EvidenceSynthesisPlane::RealGliomaPopulation => {
                if reference.source_id.is_none()
                    || reference.source_uri.is_none()
                    || reference.record_uri.is_some()
                {
                    return Err(invalid_report(
                        "real-data reference is missing source provenance",
                    ));
                }
            }
            EvidenceSynthesisPlane::PublicLiterature => {
                if !reference.record_id.starts_with("PMID-")
                    || reference.source_id.is_none()
                    || reference.source_uri.is_none()
                    || reference.record_uri.is_none()
                {
                    return Err(invalid_report(
                        "public-literature reference is missing PMID provenance",
                    ));
                }
            }
            EvidenceSynthesisPlane::CaseObservation => {
                if reference.source_id.is_some()
                    || reference.source_uri.is_some()
                    || reference.record_uri.is_some()
                {
                    return Err(invalid_report(
                        "case observation reference carries external provenance",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_lanes(report: &EvidenceSynthesisReport) -> Result<(), NeurosurgeryError> {
    let route = crate::required_capabilities(report.specialty);
    if report.lanes.len() != route.len() {
        return Err(invalid_report(
            "evidence synthesis lane count is not canonical",
        ));
    }
    for (lane, capability) in report.lanes.iter().zip(route) {
        if lane.capability != capability
            || lane
                .reference_ids
                .windows(2)
                .any(|window| window[0] >= window[1])
            || lane
                .reviewer_questions
                .windows(2)
                .any(|window| window[0] > window[1])
        {
            return Err(invalid_report(
                "evidence synthesis lane ordering is invalid",
            ));
        }
        let required_inputs = crate::tool_catalogue()
            .into_iter()
            .find(|spec| spec.capability == capability)
            .map(|spec| spec.required_inputs)
            .unwrap_or_default();
        let relevant_items = report
            .case_audit
            .items
            .iter()
            .filter(|item| required_inputs.contains(&item.observation_kind))
            .collect::<Vec<_>>();
        let caller_refs = report
            .references
            .iter()
            .filter(|reference| {
                reference.plane == EvidenceSynthesisPlane::CallerEvidence
                    && reference.supports.contains(&capability)
            })
            .collect::<Vec<_>>();
        let population_refs = report
            .references
            .iter()
            .filter(|reference| {
                matches!(
                    reference.plane,
                    EvidenceSynthesisPlane::RealGliomaPopulation
                        | EvidenceSynthesisPlane::PublicLiterature
                ) && reference.supports.contains(&capability)
            })
            .collect::<Vec<_>>();
        let mut reference_ids = caller_refs
            .iter()
            .chain(population_refs.iter())
            .map(|reference| reference.record_id.clone())
            .collect::<Vec<_>>();
        reference_ids.sort();
        reference_ids.dedup();
        let expected_state = relevant_items.iter().map(|item| item.state).fold(
            if capability == ToolCapability::EvidenceSynthesis
                && report.case_audit.evidence_supporting_synthesis_count == 0
            {
                EvidenceState::Unmeasured
            } else {
                EvidenceState::Measured
            },
            worst_evidence_state,
        );
        let mut reviewer_questions = report.specialty.profile().evidence_questions;
        for item in relevant_items {
            reviewer_questions.push(item.reviewer_note.clone());
        }
        reviewer_questions.sort();
        reviewer_questions.dedup();
        let verified_reference_count = caller_refs
            .iter()
            .chain(population_refs.iter())
            .filter(|reference| reference.tier.is_some_and(EvidenceTier::is_verified))
            .count();
        let unverified_reference_count = caller_refs
            .iter()
            .chain(population_refs.iter())
            .filter(|reference| reference.tier == Some(EvidenceTier::Unverified))
            .count();
        if lane.case_observation_count
            != report
                .case_audit
                .items
                .iter()
                .filter(|item| required_inputs.contains(&item.observation_kind))
                .map(|item| item.observed_count)
                .sum::<usize>()
            || lane.caller_evidence_count != caller_refs.len()
            || lane.population_reference_count != population_refs.len()
            || lane.verified_reference_count != verified_reference_count
            || lane.unverified_reference_count != unverified_reference_count
            || lane.reference_ids != reference_ids
            || lane.evidence_state != expected_state
            || lane.reviewer_questions != reviewer_questions
        {
            return Err(invalid_report(
                "evidence synthesis lane projection is inconsistent",
            ));
        }
    }
    Ok(())
}

fn validate_source_summaries(report: &EvidenceSynthesisReport) -> Result<(), NeurosurgeryError> {
    if let Some(summary) = report.real_data_summary.as_ref() {
        if summary.bundle_schema_version != crate::real_data::REAL_DATA_SCHEMA_VERSION
            || !is_sha256_hex(&summary.bundle_digest)
            || !summary.provenance_bound
            || summary.synthetic_data
        {
            return Err(invalid_report("real-data synthesis summary is invalid"));
        }
    }
    if let Some(summary) = report.public_literature_summary.as_ref() {
        if summary.schema_version != crate::public_literature::PUBLIC_LITERATURE_SCHEMA_VERSION
            || !is_sha256_hex(&summary.bundle_digest)
            || !summary.provenance_bound
            || summary.synthetic_data
        {
            return Err(invalid_report(
                "public-literature synthesis summary is invalid",
            ));
        }
    }
    for (summary_digest, freshness) in [
        (
            report
                .real_data_summary
                .as_ref()
                .map(|summary| summary.bundle_digest.as_str()),
            report.real_data_freshness.as_ref(),
        ),
        (
            report
                .public_literature_summary
                .as_ref()
                .map(|summary| summary.bundle_digest.as_str()),
            report.public_literature_freshness.as_ref(),
        ),
    ] {
        if freshness.is_some_and(|freshness| {
            summary_digest.is_none()
                || freshness.schema_version
                    != crate::real_data_freshness::REAL_DATA_FRESHNESS_SCHEMA_VERSION
                || !is_sha256_hex(&freshness.bundle_digest)
                || summary_digest != Some(freshness.bundle_digest.as_str())
                || !is_sha256_hex(&freshness.freshness_digest)
                || !freshness.provenance_bound
                || freshness.synthetic_data
                || !freshness.human_review_required
                || freshness.provider != "none"
                || freshness.network
                || freshness.effect != "read_only"
        }) {
            return Err(invalid_report(
                "evidence synthesis freshness projection is invalid",
            ));
        }
    }
    Ok(())
}

fn validate_links(report: &EvidenceSynthesisReport) -> Result<(), NeurosurgeryError> {
    if let Some(audit) = report.literature_link_audit.as_ref() {
        if audit.schema_version != crate::literature_link::LITERATURE_LINK_AUDIT_SCHEMA_VERSION
            || !is_sha256_hex(&audit.audit_digest)
            || !is_sha256_hex(&audit.real_data_bundle_digest)
            || !is_sha256_hex(&audit.public_literature_bundle_digest)
            || report
                .real_data_summary
                .as_ref()
                .map(|summary| summary.bundle_digest.as_str())
                != Some(audit.real_data_bundle_digest.as_str())
            || report
                .public_literature_summary
                .as_ref()
                .map(|summary| summary.bundle_digest.as_str())
                != Some(audit.public_literature_bundle_digest.as_str())
            || audit.links.len() != report.links.len()
            || audit.links.len() > report.query.max_references.min(128)
            || !audit.provenance_bound
            || audit.synthetic_data
            || !audit.human_review_required
            || audit.provider != "none"
            || audit.network
            || audit.effect != "read_only"
        {
            return Err(invalid_report(
                "evidence synthesis literature-link audit is invalid",
            ));
        }
        for (link, projected) in audit.links.iter().zip(report.links.iter()) {
            if link.real_pmid != projected.real_pmid
                || link.public_pmid != projected.public_pmid
                || link.public_specialty != projected.public_specialty
                || link.match_kinds != projected.match_kinds
                || link.mismatched_fields != projected.mismatched_fields
            {
                return Err(invalid_report(
                    "evidence synthesis literature links drifted from their audit",
                ));
            }
        }
    } else if !report.links.is_empty() {
        return Err(invalid_report(
            "evidence synthesis has links without a literature-link audit",
        ));
    }
    Ok(())
}

/// Align the case request with one or both validated public evidence planes.
#[allow(clippy::too_many_arguments)]
pub fn synthesize(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceSynthesisQuery,
) -> Result<EvidenceSynthesisReport, NeurosurgeryError> {
    validate_query(query)?;
    if real_data.is_some() && request.specialty != Specialty::Glioma {
        return Err(NeurosurgeryError::RealDataSpecialtyUnsupported {
            specialty: request.specialty,
        });
    }
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
    if let Some(literature_query) = &query.public_literature_query {
        if literature_query.specialty != Some(request.specialty) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "public_literature_query specialty must match the request specialty"
                    .to_string(),
            });
        }
    }

    let mut case_audit = audit_evidence(request)?;
    redact_temporal_labels(&mut case_audit);
    let request_digest = case_audit.request_digest.clone();
    let case_observations = request
        .observations
        .iter()
        .map(observation_index)
        .collect::<Result<Vec<_>, _>>()?;

    let real_data_summary = real_data.map(RealGliomaBundle::summary).transpose()?;
    let public_literature_summary = public_literature
        .map(PublicLiteratureBundle::summary)
        .transpose()?;
    let real_data_freshness = match (real_data, query.freshness.as_ref()) {
        (Some(data), Some(freshness)) => Some(data.freshness_report(freshness)?),
        _ => None,
    };
    let public_literature_freshness = match (public_literature, query.freshness.as_ref()) {
        (Some(literature), Some(freshness)) => Some(literature.freshness_report(freshness)?),
        _ => None,
    };
    let mut references = Vec::new();
    let mut review_items = Vec::new();
    let mut source_query_truncated = false;

    // Caller evidence is kept as a separate plane.  Stable IDs are copied; raw case text is not.
    for evidence in &request.evidence {
        references.push(caller_reference(evidence));
    }

    if let Some(data) = real_data {
        data.validate()?;
        let source_query = query
            .real_data_query
            .clone()
            .unwrap_or_else(|| RealDataQuery {
                limit: crate::real_data::MAX_QUERY_HITS_PUBLIC,
                ..RealDataQuery::default()
            });
        let result = data.query(&source_query)?;
        source_query_truncated |= result.truncated;
        references.extend(result.hits.iter().map(real_reference));
    }

    if let Some(literature) = public_literature {
        literature.validate()?;
        if !literature.has_specialty(request.specialty) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "public-literature bundle has no records tagged for {}",
                    request.specialty.slug()
                ),
            });
        }
        let source_query =
            query
                .public_literature_query
                .clone()
                .unwrap_or_else(|| PublicLiteratureQuery {
                    specialty: Some(request.specialty),
                    limit: crate::public_literature::MAX_QUERY_HITS_PUBLIC,
                    ..PublicLiteratureQuery::default()
                });
        let result = literature.query(&source_query)?;
        source_query_truncated |= result.truncated;
        references.extend(
            result
                .hits
                .iter()
                .map(|hit| public_reference(hit, query.include_source_text)),
        );
    }

    if real_data.is_none() && request.specialty == Specialty::Glioma {
        review_items.push(EvidenceSynthesisReviewItem {
            code: "real_glioma_population_unattached".to_string(),
            scope: "real_glioma_population".to_string(),
            detail: "no validated real glioma population snapshot was supplied; case and citation planes remain separate and no population inventory is asserted".to_string(),
            reference_ids: Vec::new(),
        });
    }
    if public_literature.is_none() {
        review_items.push(EvidenceSynthesisReviewItem {
            code: "public_literature_unattached".to_string(),
            scope: "public_literature".to_string(),
            detail: "no validated PubMed snapshot was supplied; the ledger does not infer that the wider literature is empty".to_string(),
            reference_ids: Vec::new(),
        });
    }
    for item in &case_audit.items {
        if item.state != EvidenceState::Measured
            || item.provenance_complete_count < item.observed_count
        {
            review_items.push(EvidenceSynthesisReviewItem {
                code: "case_coverage_review".to_string(),
                scope: format!("case_observation::{:?}", item.observation_kind)
                    .to_ascii_lowercase(),
                detail: item.reviewer_note.clone(),
                reference_ids: Vec::new(),
            });
        }
    }

    // Only typed glioma panels trigger marker-level grounding. This keeps non-glioma lanes from
    // inheriting molecular vocabulary and makes an absent panel an explicit caller decision.
    let glioma_molecular_map =
        if request.specialty == Specialty::Glioma && request.glioma_molecular.is_some() {
            let map = crate::glioma_molecular_map::map_molecular_evidence(
                request,
                real_data,
                public_literature,
                &GliomaMolecularMapQuery {
                    real_data_query: query.real_data_query.clone(),
                    public_literature_query: query.public_literature_query.clone(),
                    freshness: query.freshness.clone(),
                    max_hits_per_marker: 8,
                    max_references: query.max_references,
                    include_source_text: query.include_source_text,
                    ..GliomaMolecularMapQuery::default()
                },
            )?;
            references.extend(map.references.clone());
            Some(map)
        } else {
            None
        };

    // Canonicalize and bound only after every source plane, including the optional molecular
    // map, has contributed its rows. This prevents nested reports from bypassing the caller's
    // reference cap or introducing duplicate PMID/source entries.
    references.sort_by(|left, right| {
        left.plane
            .cmp(&right.plane)
            .then(left.record_kind.cmp(&right.record_kind))
            .then(left.record_id.cmp(&right.record_id))
    });
    references.dedup_by(|left, right| {
        left.plane == right.plane
            && left.record_kind == right.record_kind
            && left.record_id == right.record_id
    });
    if references.len() > query.max_references {
        review_items.push(EvidenceSynthesisReviewItem {
            code: "reference_projection_truncated".to_string(),
            scope: "references".to_string(),
            detail: format!(
                "{} reference rows were available, but only {} are emitted by the caller bound",
                references.len(),
                query.max_references
            ),
            reference_ids: Vec::new(),
        });
        references.truncate(query.max_references);
    }
    if source_query_truncated {
        review_items.push(EvidenceSynthesisReviewItem {
            code: "source_query_truncated".to_string(),
            scope: "public_snapshot_query".to_string(),
            detail: "a bounded local source query matched more records than it returned; emitted rows are a lower-bounded projection, not an exhaustive corpus".to_string(),
            reference_ids: references.iter().map(|reference| reference.record_id.clone()).collect(),
        });
    }
    review_items.truncate(MAX_EVIDENCE_SYNTHESIS_REVIEW_ITEMS);

    let lanes = build_lanes(request, &references, &case_audit);
    let (literature_link_audit, links) = match (real_data, public_literature) {
        (Some(real_data), Some(public_literature)) => {
            let audit = real_data.literature_link_audit(
                public_literature,
                &LiteratureLinkAuditQuery {
                    public_specialty: Some(Specialty::Glioma),
                    max_links: query.max_references.min(128),
                    max_unmatched_ids: query.max_references.min(128),
                },
            )?;
            let links = audit
                .links
                .iter()
                .map(|link| EvidenceSynthesisLink {
                    real_pmid: link.real_pmid.clone(),
                    public_pmid: link.public_pmid.clone(),
                    public_specialty: link.public_specialty,
                    match_kinds: link.match_kinds.clone(),
                    mismatched_fields: link.mismatched_fields.clone(),
                })
                .collect();
            (Some(audit), links)
        }
        _ => (None, Vec::new()),
    };

    let generated_at = real_data
        .map(|data| data.generated_at.clone())
        .or_else(|| public_literature.map(|literature| literature.generated_at.clone()))
        .unwrap_or_else(|| "case-only-no-public-snapshot".to_string());
    let mut report = EvidenceSynthesisReport {
        schema_version: EVIDENCE_SYNTHESIS_SCHEMA_VERSION.to_string(),
        synthesis_digest: String::new(),
        request_digest,
        specialty: request.specialty,
        generated_at,
        query: query.clone(),
        case_observations,
        case_audit,
        case_asset_report_digest: None,
        case_asset_summary: None,
        case_asset_review_items: Vec::new(),
        case_asset_review_disposition_digest: None,
        case_asset_review_pending_item_count: None,
        case_asset_review_resolved_decision_count: None,
        case_asset_review_unresolved_decision_count: None,
        glioma_molecular_map,
        references,
        lanes,
        real_data_summary,
        real_data_freshness,
        public_literature_summary,
        public_literature_freshness,
        literature_link_audit,
        links,
        review_items,
        reviewer_roles: request.specialty.profile().human_review_roles,
        provenance_bound: true,
        // The attached public bundles are required to be real, but the caller may explicitly
        // label the case as a synthetic educational scenario. Preserve that distinction instead
        // of claiming the entire ledger is real-world case data.
        synthetic_data: request.request_use == crate::RequestUse::SyntheticCaseSimulation,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "the report aligns typed case coverage with public source metadata; it does not generate or fact-check a medical conclusion".to_string(),
            "case observations, caller evidence, real population records, and PubMed citations remain separate evidence planes".to_string(),
            "exact PMID/DOI links do not establish cohort identity, independence, evidence quality, applicability, causality, or patient-level meaning".to_string(),
            "missing, unverified, conflicting, and truncated inputs remain explicit review obligations; no value is imputed and no absence is treated as negative evidence".to_string(),
            "the ledger never fetches URLs, invokes a model/provider, opens credentials, accesses patient files, sends notifications, or performs an external effect".to_string(),
        ],
    };
    report.synthesis_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

/// Align the case and public evidence planes while binding an already validated, metadata-only
/// asset projection into the resulting ledger. The projection is intentionally supplied as a
/// report rather than raw assets: callers must pass through `CaseAssetManifest::project` first,
/// and this function never opens or interprets bytes.
pub fn synthesize_with_case_assets(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceSynthesisQuery,
    case_asset_report: Option<&crate::CaseAssetManifestReport>,
) -> Result<EvidenceSynthesisReport, NeurosurgeryError> {
    synthesize_with_case_assets_and_dispositions(
        request,
        real_data,
        public_literature,
        query,
        case_asset_report,
        None,
    )
}

/// Align the evidence planes while binding both a metadata-only case-asset projection and its
/// reviewer-owned disposition ledger. The disposition report must be produced from the exact
/// projection supplied here; digest/count mismatches fail closed before the synthesis digest is
/// emitted. No asset bytes or clinical interpretation enter this function.
pub fn synthesize_with_case_assets_and_dispositions(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceSynthesisQuery,
    case_asset_report: Option<&crate::CaseAssetManifestReport>,
    disposition_report: Option<&crate::CaseAssetReviewDispositionReport>,
) -> Result<EvidenceSynthesisReport, NeurosurgeryError> {
    if let Some(asset_report) = case_asset_report {
        asset_report.validate_for_request(request)?;
    }
    if disposition_report.is_some() && case_asset_report.is_none() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "case-asset review disposition requires a case-asset report".to_string(),
        });
    }
    if let (Some(asset_report), Some(disposition)) = (case_asset_report, disposition_report) {
        disposition.validate_integrity()?;
        if disposition.report_digest != asset_report.report_digest
            || disposition.returned_item_count != asset_report.review_items.len()
            || disposition.omitted_item_count != asset_report.omitted_review_item_count
            || disposition.candidate_item_count
                != asset_report
                    .review_items
                    .len()
                    .checked_add(asset_report.omitted_review_item_count)
                    .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                        reason: "case-asset review candidate count overflows its bound".to_string(),
                    })?
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset review disposition does not match the supplied report"
                    .to_string(),
            });
        }
    }
    let mut report = synthesize(request, real_data, public_literature, query)?;
    report.case_asset_report_digest = case_asset_report.map(|asset| asset.report_digest.clone());
    report.case_asset_summary = case_asset_report.map(|asset| EvidenceSynthesisCaseAssetSummary {
        report_digest: asset.report_digest.clone(),
        asset_count: asset.asset_count,
        observed_asset_count: asset.observed_asset_count,
        non_observed_asset_count: asset.non_observed_asset_count,
        provenance_complete_asset_count: asset.provenance_complete_asset_count,
        missing_requested_kinds: asset.missing_requested_kinds.clone(),
        review_item_count: asset.review_items.len(),
        omitted_review_item_count: asset.omitted_review_item_count,
        truncated: asset.truncated,
    });
    report.case_asset_review_items = case_asset_report
        .map(|asset| asset.review_items.clone())
        .unwrap_or_default();
    report.case_asset_review_disposition_digest =
        disposition_report.map(|disposition| disposition.disposition_digest.clone());
    report.case_asset_review_pending_item_count =
        disposition_report.map(|disposition| disposition.pending_item_count);
    report.case_asset_review_resolved_decision_count =
        disposition_report.map(|disposition| disposition.resolved_decision_count);
    report.case_asset_review_unresolved_decision_count =
        disposition_report.map(|disposition| disposition.unresolved_decision_count);
    report.synthesis_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn validate_query(query: &EvidenceSynthesisQuery) -> Result<(), NeurosurgeryError> {
    if !(1..=MAX_EVIDENCE_SYNTHESIS_REFERENCES).contains(&query.max_references) {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_synthesis.max_references",
            found: query.max_references,
            max: MAX_EVIDENCE_SYNTHESIS_REFERENCES,
        });
    }
    if let Some(public_query) = &query.public_literature_query {
        if public_query.specialty.is_none() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence synthesis public_literature_query must name one specialty"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn observation_index(
    observation: &crate::Observation,
) -> Result<EvidenceSynthesisObservation, NeurosurgeryError> {
    let bytes = serde_json::to_vec(observation)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(EvidenceSynthesisObservation {
        observation_digest: sha256_hex(&bytes),
        kind: observation.kind,
        status: observation.status,
        source_id: observation.source_id.clone(),
        observed_at: observation.observed_at.clone(),
        timepoint: observation.timepoint.clone(),
    })
}

fn redact_temporal_labels(audit: &mut EvidenceAuditReport) {
    for observation in &mut audit.temporal_alignment.observations {
        observation.label = format!("observation-{}", observation.observation_index);
    }
    for timepoint in &mut audit.temporal_alignment.timepoints {
        timepoint.labels = timepoint
            .observation_indices
            .iter()
            .map(|index| format!("observation-{index}"))
            .collect();
    }
}

fn caller_reference(evidence: &EvidenceRecord) -> EvidenceSynthesisReference {
    EvidenceSynthesisReference {
        plane: EvidenceSynthesisPlane::CallerEvidence,
        record_kind: "caller_evidence".to_string(),
        record_id: evidence.id.clone(),
        title: evidence.title.clone(),
        citation: evidence.citation.clone(),
        source_id: None,
        source_uri: None,
        record_uri: None,
        tier: Some(evidence.tier),
        year: evidence.year,
        status: None,
        related_record_ids: Vec::new(),
        supports: evidence.supports.clone(),
        source_text_excerpt: None,
    }
}

pub(crate) fn real_reference(hit: &RealDataQueryHit) -> EvidenceSynthesisReference {
    let tier = match hit.record_kind {
        RealDataRecordKind::GuidelineReference => EvidenceTier::Guideline,
        _ => EvidenceTier::Unverified,
    };
    let mut supports = vec![ToolCapability::EvidenceSynthesis];
    if hit.record_kind == RealDataRecordKind::GuidelineReference {
        supports.extend([
            ToolCapability::ImagingReview,
            ToolCapability::MolecularContext,
            ToolCapability::DifferentialMatrix,
        ]);
    }
    supports.sort();
    supports.dedup();
    EvidenceSynthesisReference {
        plane: EvidenceSynthesisPlane::RealGliomaPopulation,
        record_kind: hit.record_kind.slug().to_string(),
        record_id: hit.record_id.clone(),
        title: hit.title.clone(),
        citation: hit.source_uri.clone(),
        source_id: Some(hit.source_id.clone()),
        source_uri: Some(hit.source_uri.clone()),
        record_uri: None,
        tier: Some(tier),
        year: None,
        status: hit.status.clone(),
        related_record_ids: hit
            .related_records
            .iter()
            .map(|record| record.record_id.clone())
            .collect(),
        supports,
        source_text_excerpt: hit.abstract_excerpt.clone(),
    }
}

pub(crate) fn public_reference(
    hit: &PublicLiteratureQueryHit,
    include_source_text: bool,
) -> EvidenceSynthesisReference {
    EvidenceSynthesisReference {
        plane: EvidenceSynthesisPlane::PublicLiterature,
        record_kind: "literature_article".to_string(),
        record_id: format!("PMID-{}", hit.pmid),
        title: hit.title.clone(),
        citation: format!("PubMed:{}", hit.pmid),
        source_id: Some(hit.source_id.clone()),
        source_uri: Some(hit.source_uri.clone()),
        record_uri: Some(hit.record_uri.clone()),
        tier: Some(EvidenceTier::Unverified),
        year: hit
            .publication_date
            .as_deref()
            .and_then(|date| date.get(..4))
            .and_then(|year| year.parse::<u16>().ok()),
        status: None,
        related_record_ids: Vec::new(),
        supports: vec![ToolCapability::EvidenceSynthesis],
        source_text_excerpt: include_source_text
            .then(|| hit.abstract_excerpt.clone())
            .flatten(),
    }
}

fn build_lanes(
    request: &CaseRequest,
    references: &[EvidenceSynthesisReference],
    audit: &EvidenceAuditReport,
) -> Vec<EvidenceSynthesisLane> {
    let route = crate::required_capabilities(request.specialty);
    route
        .into_iter()
        .map(|capability| {
            let required_inputs = crate::tool_catalogue()
                .into_iter()
                .find(|spec| spec.capability == capability)
                .map(|spec| spec.required_inputs)
                .unwrap_or_default();
            let relevant_items = audit
                .items
                .iter()
                .filter(|item| required_inputs.contains(&item.observation_kind))
                .collect::<Vec<_>>();
            let case_observation_count =
                relevant_items.iter().map(|item| item.observed_count).sum();
            let caller_refs = references
                .iter()
                .filter(|reference| reference.plane == EvidenceSynthesisPlane::CallerEvidence)
                .filter(|reference| reference.supports.contains(&capability))
                .collect::<Vec<_>>();
            let population_refs = references
                .iter()
                .filter(|reference| {
                    matches!(
                        reference.plane,
                        EvidenceSynthesisPlane::RealGliomaPopulation
                            | EvidenceSynthesisPlane::PublicLiterature
                    )
                })
                .filter(|reference| reference.supports.contains(&capability))
                .collect::<Vec<_>>();
            let verified_reference_count = caller_refs
                .iter()
                .chain(population_refs.iter())
                .filter(|reference| reference.tier.is_some_and(EvidenceTier::is_verified))
                .count();
            let unverified_reference_count = caller_refs
                .iter()
                .chain(population_refs.iter())
                .filter(|reference| reference.tier == Some(EvidenceTier::Unverified))
                .count();
            let mut reference_ids = caller_refs
                .iter()
                .chain(population_refs.iter())
                .map(|reference| reference.record_id.clone())
                .collect::<Vec<_>>();
            reference_ids.sort();
            reference_ids.dedup();
            let evidence_state = relevant_items.iter().map(|item| item.state).fold(
                if capability == ToolCapability::EvidenceSynthesis
                    && audit.evidence_supporting_synthesis_count == 0
                {
                    EvidenceState::Unmeasured
                } else {
                    EvidenceState::Measured
                },
                worst_evidence_state,
            );
            let mut reviewer_questions = request.specialty.profile().evidence_questions;
            for item in relevant_items {
                reviewer_questions.push(item.reviewer_note.clone());
            }
            reviewer_questions.sort();
            reviewer_questions.dedup();
            EvidenceSynthesisLane {
                capability,
                case_observation_count,
                caller_evidence_count: caller_refs.len(),
                population_reference_count: population_refs.len(),
                verified_reference_count,
                unverified_reference_count,
                reference_ids,
                evidence_state,
                reviewer_questions,
            }
        })
        .collect()
}

fn worst_evidence_state(left: EvidenceState, right: EvidenceState) -> EvidenceState {
    match (left, right) {
        (EvidenceState::Conflicting, _) | (_, EvidenceState::Conflicting) => {
            EvidenceState::Conflicting
        }
        (EvidenceState::Uninterpretable, _) | (_, EvidenceState::Uninterpretable) => {
            EvidenceState::Uninterpretable
        }
        (EvidenceState::Unmeasured, _) | (_, EvidenceState::Unmeasured) => {
            EvidenceState::Unmeasured
        }
        _ => EvidenceState::Measured,
    }
}

fn digest_report(report: &EvidenceSynthesisReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.synthesis_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
