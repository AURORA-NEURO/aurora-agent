//! One-call, provenance-bound evidence handoff for the public glioma snapshot.
//!
//! This packet composes the existing descriptive projections—summary, coverage, explicit
//! crosswalk, bounded record query, comparative genomic-cohort landscape, identifier
//! reconciliation, and metadata-review queue—without
//! creating a second inference engine. It is intended for a local model or human reviewer that
//! needs one digest-addressed envelope before doing domain reasoning. All subreports remain
//! individually bounded and retain their own missingness and human-review posture.

use crate::real_data_freshness::{RealDataFreshnessQuery, RealDataFreshnessReport};
use crate::{
    EvidenceGraphQuery, EvidenceGraphReport, NeurosurgeryError, RealDataCohortLandscapeQuery,
    RealDataCohortLandscapeReport, RealDataCoverageQuery, RealDataCoverageReport,
    RealDataMolecularCoverageQuery, RealDataMolecularCoverageReport, RealDataQuery,
    RealDataQueryResult, RealDataReconciliationQuery, RealDataReconciliationReport,
    RealDataReviewQueueQuery, RealDataReviewQueueReport, RealDataSummary,
    RealDataTrialLandscapeQuery, RealDataTrialLandscapeReport, RealGliomaBundle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REAL_DATA_EVIDENCE_PACKET_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-evidence-packet/0.4";

fn canonical_molecular_coverage_query() -> RealDataMolecularCoverageQuery {
    let mut query = RealDataMolecularCoverageQuery::default();
    // The packet is the complete metadata handoff; keep the standalone query's smaller default
    // bound available while making packet-level assay coverage deterministic and inventory-wide.
    query.query.limit = 128;
    query
}

/// Nested bounded projections to include in one evidence handoff.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataEvidencePacketQuery {
    #[serde(default)]
    pub query: RealDataQuery,
    #[serde(default)]
    pub coverage: RealDataCoverageQuery,
    #[serde(default)]
    pub graph: EvidenceGraphQuery,
    #[serde(default)]
    pub review_queue: RealDataReviewQueueQuery,
    /// Optional explicit caller-owned source-age policy. When omitted, the packet does not
    /// invent a clock or silently claim freshness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessQuery>,
}

/// One digest-addressed packet for local-model or human review. It carries only the same
/// public metadata and bounded text excerpts already exposed by the underlying projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataEvidencePacketReport {
    pub schema_version: String,
    pub packet_digest: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: RealDataEvidencePacketQuery,
    pub summary: RealDataSummary,
    pub coverage: RealDataCoverageReport,
    pub graph: EvidenceGraphReport,
    pub data_query: RealDataQueryResult,
    /// Canonical bounded registry reconnaissance over all trial metadata in the validated
    /// snapshot. This is included automatically so a real-data handoff cannot omit the trial
    /// landscape while still remaining descriptive and human-review gated.
    pub trial_landscape: RealDataTrialLandscapeReport,
    /// Canonical bounded inventory of cBioPortal assay/profile metadata. This is included
    /// automatically so molecular availability cannot be confused with an absent projection.
    pub molecular_coverage: RealDataMolecularCoverageReport,
    /// Optional comparative inventory of public genomic projects. This field is absent only in
    /// packets persisted before cohort-landscape support; newly generated packets always include
    /// it. Rows contain aggregate project/file metadata only and never patient-level values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohort_landscape: Option<RealDataCohortLandscapeReport>,
    /// Canonical cross-source PMID/DOI reconciliation over the validated snapshot. This is
    /// included automatically so a local model cannot mistake an unresolved identifier
    /// crosswalk for a clean provenance graph.
    pub reconciliation: RealDataReconciliationReport,
    pub review_queue: RealDataReviewQueueReport,
    /// Optional digest-bound source-age posture requested by the caller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<RealDataFreshnessReport>,
    pub source_count: usize,
    pub record_count: usize,
    pub query_match_count: usize,
    pub open_review_obligation_count: usize,
    pub explicit_crosswalk_edge_count: usize,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataEvidencePacketReport {
    /// Validate a persisted packet without fetching sources or opening any asset bytes.
    ///
    /// This checks nested report closure, one-snapshot binding, bounded result counters, and the
    /// packet digest. It does not assess evidence quality, cohort applicability, or clinical truth.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != REAL_DATA_EVIDENCE_PACKET_SCHEMA_VERSION
            || !is_sha256_hex(&self.packet_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || self.bundle_digest != self.summary.bundle_digest
            || self.bundle_digest != self.coverage.bundle_digest
            || self.bundle_digest != self.graph.bundle_digest
            || self.bundle_digest != self.data_query.bundle_digest
            || self.bundle_digest != self.trial_landscape.bundle_digest
            || self.bundle_digest != self.molecular_coverage.bundle_digest
            || self
                .cohort_landscape
                .as_ref()
                .is_some_and(|report| self.bundle_digest != report.bundle_digest)
            || self.bundle_digest != self.reconciliation.bundle_digest
            || self.bundle_digest != self.review_queue.bundle_digest
            || self.generated_at.trim().is_empty()
            || self.query.coverage != self.coverage.query
            || self.query.graph != self.graph.query
            || self.query.query != self.data_query.query
            || self.query.review_queue != self.review_queue.query
            || self.source_count != self.summary.source_count
            || self.source_count != self.coverage.source_count
            || self.source_count != self.review_queue.source_count
            || self.record_count != self.summary.record_count
            || self.record_count != self.coverage.total_record_count
            || self.query_match_count != self.data_query.total_matches
            || self.open_review_obligation_count != self.review_queue.candidate_item_count
            || self.explicit_crosswalk_edge_count != self.graph.total_edge_count
            || self.trial_landscape.query != RealDataTrialLandscapeQuery::default()
            || self.molecular_coverage.query != canonical_molecular_coverage_query()
            || self
                .cohort_landscape
                .as_ref()
                .is_some_and(|report| report.query != RealDataCohortLandscapeQuery::default())
            || self.reconciliation.query != RealDataReconciliationQuery::default()
            || self.data_query.returned_matches != self.data_query.hits.len()
            || self.data_query.total_matches < self.data_query.returned_matches
            || self.data_query.truncated
                != (self.data_query.total_matches > self.data_query.returned_matches)
            || self.review_queue.returned_item_count != self.review_queue.items.len()
            || self.review_queue.candidate_item_count < self.review_queue.returned_item_count
            || self.review_queue.omitted_item_count
                != self
                    .review_queue
                    .candidate_item_count
                    .saturating_sub(self.review_queue.returned_item_count)
            || self.review_queue.truncated != (self.review_queue.omitted_item_count > 0)
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(packet_rejected(
                "real-data evidence packet envelope is invalid",
            ));
        }
        if self.summary.bundle_schema_version != crate::real_data::REAL_DATA_SCHEMA_VERSION
            || !self.summary.provenance_bound
            || self.summary.synthetic_data
            // Every nested projection is independently persisted and must pass its own
            // structural gate before the packet can be treated as a trustworthy handoff. A
            // packet digest alone is not sufficient: a caller could otherwise rewrite a nested
            // report and recompute only the outer digest while bypassing its local invariants.
            || self.coverage.validate_integrity().is_err()
            || self.graph.validate_integrity().is_err()
            || self.data_query.validate_integrity().is_err()
            || self.coverage.schema_version != crate::REAL_DATA_COVERAGE_SCHEMA_VERSION
            || !is_sha256_hex(&self.coverage.coverage_digest)
            || self.trial_landscape.validate_integrity().is_err()
            || self.molecular_coverage.validate_integrity().is_err()
            || self
                .cohort_landscape
                .as_ref()
                .is_some_and(|report| report.validate_integrity().is_err())
            || self.reconciliation.validate_integrity().is_err()
            || self.review_queue.validate_integrity().is_err()
            || self.data_query.schema_version != crate::real_data::REAL_DATA_SCHEMA_VERSION
        {
            return Err(packet_rejected(
                "real-data evidence packet nested reports are invalid",
            ));
        }
        if let Some(freshness) = self.freshness.as_ref() {
            if freshness.bundle_digest != self.bundle_digest
                || !is_sha256_hex(&freshness.freshness_digest)
                || freshness.validate_integrity().is_err()
                || !freshness.provenance_bound
                || freshness.synthetic_data
                || !freshness.human_review_required
                || freshness.provider != "none"
                || freshness.network
                || freshness.effect != "read_only"
                || self.query.freshness.as_ref() != Some(&freshness.query)
            {
                return Err(packet_rejected(
                    "real-data evidence packet freshness binding is invalid",
                ));
            }
        } else if self.query.freshness.is_some() {
            return Err(packet_rejected(
                "real-data evidence packet freshness query is missing its report",
            ));
        }
        let freshness_digest = self
            .freshness
            .as_ref()
            .map(|report| report.freshness_digest.as_str());
        let cohort_landscape_digest = self
            .cohort_landscape
            .as_ref()
            .map(|report| report.landscape_digest.as_str());
        if self.packet_digest
            != digest_packet(
                &self.bundle_digest,
                &self.query,
                &self.coverage.coverage_digest,
                &self.graph.graph_digest,
                &self.data_query.bundle_digest,
                &self.trial_landscape.landscape_digest,
                &self.molecular_coverage.coverage_digest,
                cohort_landscape_digest,
                &self.reconciliation.reconciliation_digest,
                &self.review_queue.queue_digest,
                freshness_digest,
            )?
        {
            return Err(packet_rejected(
                "real-data evidence packet digest does not match its contents",
            ));
        }
        Ok(())
    }

    /// Rebuild the packet from the exact validated snapshot and persisted nested query bounds.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.evidence_packet(&self.query)?;
        let expected = if self.cohort_landscape.is_none() {
            // Packets created before the cohort field was introduced remain replayable after
            // deserialization. Rebuild the legacy digest shape while preserving all other
            // canonical projections.
            let mut legacy = expected;
            legacy.cohort_landscape = None;
            legacy.packet_digest = digest_packet(
                &legacy.bundle_digest,
                &legacy.query,
                &legacy.coverage.coverage_digest,
                &legacy.graph.graph_digest,
                &legacy.data_query.bundle_digest,
                &legacy.trial_landscape.landscape_digest,
                &legacy.molecular_coverage.coverage_digest,
                None,
                &legacy.reconciliation.reconciliation_digest,
                &legacy.review_queue.queue_digest,
                legacy
                    .freshness
                    .as_ref()
                    .map(|report| report.freshness_digest.as_str()),
            )?;
            legacy
        } else {
            expected
        };
        if expected != *self {
            return Err(packet_rejected(
                "real-data evidence packet does not replay to the exact supplied snapshot",
            ));
        }
        Ok(())
    }
}

impl RealGliomaBundle {
    /// Compose bounded, source-linked projections without fetching, ranking, or interpreting the
    /// underlying records. Each nested projection revalidates the same bundle independently.
    pub fn evidence_packet(
        &self,
        query: &RealDataEvidencePacketQuery,
    ) -> Result<RealDataEvidencePacketReport, NeurosurgeryError> {
        self.validate()?;
        let summary = self.summary()?;
        let coverage = self.coverage_report(&query.coverage)?;
        let graph = self.evidence_graph(&query.graph)?;
        let data_query = self.query(&query.query)?;
        let trial_landscape = self.trial_landscape(&RealDataTrialLandscapeQuery::default())?;
        let molecular_coverage = self.molecular_coverage(&canonical_molecular_coverage_query())?;
        let cohort_landscape = self.cohort_landscape(&RealDataCohortLandscapeQuery::default())?;
        let reconciliation = self.reconcile(&RealDataReconciliationQuery::default())?;
        let review_queue = self.review_queue(&query.review_queue)?;
        let freshness = query
            .freshness
            .as_ref()
            .map(|freshness_query| self.freshness_report(freshness_query))
            .transpose()?;
        let packet_digest = digest_packet(
            &summary.bundle_digest,
            query,
            &coverage.coverage_digest,
            &graph.graph_digest,
            &data_query.bundle_digest,
            &trial_landscape.landscape_digest,
            &molecular_coverage.coverage_digest,
            Some(&cohort_landscape.landscape_digest),
            &reconciliation.reconciliation_digest,
            &review_queue.queue_digest,
            freshness
                .as_ref()
                .map(|report| report.freshness_digest.as_str()),
        )?;
        let report = RealDataEvidencePacketReport {
            schema_version: REAL_DATA_EVIDENCE_PACKET_SCHEMA_VERSION.to_string(),
            packet_digest,
            bundle_digest: summary.bundle_digest.clone(),
            generated_at: self.generated_at.clone(),
            query: query.clone(),
            source_count: summary.source_count,
            record_count: summary.record_count,
            query_match_count: data_query.total_matches,
            open_review_obligation_count: review_queue.candidate_item_count,
            explicit_crosswalk_edge_count: graph.total_edge_count,
            summary,
            coverage,
            graph,
            data_query,
            trial_landscape,
            molecular_coverage,
            cohort_landscape: Some(cohort_landscape),
            reconciliation,
            review_queue,
            freshness,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the packet composes descriptive public metadata and bounded literature excerpts; it is not a diagnosis, prognosis, treatment recommendation, triage decision, or procedural plan".to_string(),
                "coverage gaps, graph isolates, query omissions, and review obligations remain explicit; no unknown value, cohort identity, or biological relationship is inferred".to_string(),
                "the packet is a caller-owned handoff for a local model or qualified human reviewer; it never fetches URLs, invokes a provider, opens credentials, stores patient files, or performs an external effect".to_string(),
                "the included trial landscape is canonical descriptive registry metadata over the validated snapshot; it does not rank trials or infer eligibility, efficacy, safety, outcomes, or patient-level meaning".to_string(),
                "the included molecular coverage ledger is canonical cBioPortal assay metadata; it does not expose patient values or infer that an assay was run for a specimen".to_string(),
                "the included cohort landscape is canonical aggregate GDC project/file metadata; it does not establish cohort comparability, patient overlap, assay equivalence, eligibility, or a clinical conclusion".to_string(),
                "the included reconciliation ledger is canonical PMID/DOI identifier metadata; unresolved or shared identifiers remain review obligations and are never repaired, merged, or treated as evidence conclusions".to_string(),
            ],
        };
        report.validate_integrity()?;
        Ok(report)
    }
}

#[allow(clippy::too_many_arguments)]
fn digest_packet(
    bundle_digest: &str,
    query: &RealDataEvidencePacketQuery,
    coverage_digest: &str,
    graph_digest: &str,
    data_query_bundle_digest: &str,
    trial_landscape_digest: &str,
    molecular_coverage_digest: &str,
    cohort_landscape_digest: Option<&str>,
    reconciliation_digest: &str,
    queue_digest: &str,
    freshness_digest: Option<&str>,
) -> Result<String, NeurosurgeryError> {
    // Preserve the pre-cohort tuple shape for legacy packets while binding the cohort digest
    // into every newly generated packet.
    let bytes = if let Some(cohort_landscape_digest) = cohort_landscape_digest {
        serde_json::to_vec(&(
            bundle_digest,
            query,
            coverage_digest,
            graph_digest,
            data_query_bundle_digest,
            trial_landscape_digest,
            molecular_coverage_digest,
            cohort_landscape_digest,
            reconciliation_digest,
            queue_digest,
            freshness_digest,
        ))
    } else {
        serde_json::to_vec(&(
            bundle_digest,
            query,
            coverage_digest,
            graph_digest,
            data_query_bundle_digest,
            trial_landscape_digest,
            molecular_coverage_digest,
            reconciliation_digest,
            queue_digest,
            freshness_digest,
        ))
    }
    .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn packet_rejected(reason: &str) -> NeurosurgeryError {
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
