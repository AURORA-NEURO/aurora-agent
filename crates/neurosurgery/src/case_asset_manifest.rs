//! De-identified, multimodal case-asset provenance for reviewer-owned neurosurgical research.
//! This extends the repository's documented bounded multimodal boundary (`docs/AUTONOMOUS_BRAIN.md`,
//! “Bounded multimodal provider input”) with a real-data provenance handoff while preserving its
//! explicit non-interpretation limit.
//!
//! The manifest is deliberately metadata-only. A caller can point it at a real DICOM export,
//! pathology report, molecular assay, operative note, functional assessment, or longitudinal
//! outcome after an external de-identification step, but this crate never opens the asset or
//! interprets its contents. It emits stable digests and explicit missingness so a local model or
//! human reviewer can decide what to inspect next without turning an asset inventory into a
//! diagnosis, treatment, or operative instruction.

use crate::{CaseRequest, NeurosurgeryError, ObservationStatus, Specialty};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const CASE_ASSET_MANIFEST_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-case-asset-manifest/0.1";
const MAX_ASSETS: usize = 256;
const MAX_REVIEW_ITEMS: usize = 512;
const DEFAULT_MAX_REVIEW_ITEMS: usize = 128;
const MAX_ASSET_ID_BYTES: usize = 128;
const MAX_SOURCE_ID_BYTES: usize = 128;
const MAX_MODALITY_BYTES: usize = 128;
const MAX_BODY_REGION_BYTES: usize = 128;
const MAX_TIMEPOINT_BYTES: usize = 128;

fn default_max_review_items() -> usize {
    DEFAULT_MAX_REVIEW_ITEMS
}

/// Typed asset classes that a de-identification/export pipeline may register. These labels are
/// inventory vocabulary only; they do not assert that an asset establishes a clinical finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseAssetKind {
    ImagingSeries,
    PathologyReport,
    MolecularAssay,
    OperativeNote,
    NeurofunctionalAssessment,
    DevelopmentalAssessment,
    LongitudinalOutcome,
    AnatomicalModel,
}

impl CaseAssetKind {
    pub const ALL: [Self; 8] = [
        Self::ImagingSeries,
        Self::PathologyReport,
        Self::MolecularAssay,
        Self::OperativeNote,
        Self::NeurofunctionalAssessment,
        Self::DevelopmentalAssessment,
        Self::LongitudinalOutcome,
        Self::AnatomicalModel,
    ];
}

/// The external source family that produced the asset metadata. It is provenance vocabulary,
/// not a claim that the source was reviewed or that its contents are correct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseAssetSourceKind {
    DicomArchive,
    PathologyLaboratory,
    MolecularLaboratory,
    OperativeRecord,
    FunctionalAssessment,
    ResearchRepository,
    CallerExport,
    Other,
}

/// Caller-supplied metadata for one real, de-identified asset. `content_sha256` is the digest of
/// the asset bytes or canonical export produced by the caller; no asset bytes enter this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAsset {
    /// A caller-local de-identified label. The report returns a digest of this value instead of
    /// echoing it, so an identifier cannot accidentally become part of a reviewer handoff.
    pub asset_id: String,
    pub kind: CaseAssetKind,
    pub status: ObservationStatus,
    pub source_kind: CaseAssetSourceKind,
    #[serde(default)]
    pub source_id: Option<String>,
    /// Lowercase SHA-256 over the real asset or canonical export, when the caller has one.
    #[serde(default)]
    pub content_sha256: Option<String>,
    /// DICOM modality, stain/assay name, note type, or caller-owned modality label.
    #[serde(default)]
    pub modality: Option<String>,
    /// Caller-owned anatomic region label; it is not inferred from bytes or free text.
    #[serde(default)]
    pub body_region: Option<String>,
    /// Explicit caller-supplied UTC acquisition/assessment time.
    #[serde(default)]
    pub observed_at: Option<String>,
    /// De-identified timepoint label such as `baseline` or `post_intervention`.
    #[serde(default)]
    pub timepoint: Option<String>,
}

/// A bounded projection query. Requested kinds are caller-owned review scope; an omitted list
/// means “report observed coverage without inventing expected asset classes.”
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetManifestQuery {
    #[serde(default)]
    pub requested_kinds: Option<Vec<CaseAssetKind>>,
    #[serde(default = "default_max_review_items")]
    pub max_review_items: usize,
}

impl Default for CaseAssetManifestQuery {
    fn default() -> Self {
        Self {
            requested_kinds: None,
            max_review_items: default_max_review_items(),
        }
    }
}

/// A digest-only asset row suitable for a local reviewer or model handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetSummary {
    pub asset_ref: String,
    pub kind: CaseAssetKind,
    pub status: ObservationStatus,
    pub source_kind: CaseAssetSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timepoint: Option<String>,
}

/// Counts for one asset class. Counts describe supplied metadata, never patient status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetCoverage {
    pub kind: CaseAssetKind,
    pub total_count: usize,
    pub observed_count: usize,
    pub not_collected_count: usize,
    pub uninterpretable_count: usize,
    pub conflicting_count: usize,
    pub provenance_complete_count: usize,
}

/// Explicit reviewer obligation for one asset or requested class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetReviewItem {
    pub sequence: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CaseAssetKind>,
    pub code: String,
    pub reason: String,
}

/// Digest-bound multimodal asset inventory. This is a reviewer handoff, not a clinical report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetManifestReport {
    pub schema_version: String,
    pub request_digest: String,
    pub manifest_digest: String,
    pub report_digest: String,
    pub specialty: Specialty,
    pub asset_count: usize,
    pub observed_asset_count: usize,
    pub non_observed_asset_count: usize,
    pub provenance_complete_asset_count: usize,
    pub coverage: Vec<CaseAssetCoverage>,
    pub requested_kinds: Vec<CaseAssetKind>,
    pub missing_requested_kinds: Vec<CaseAssetKind>,
    pub assets: Vec<CaseAssetSummary>,
    pub review_items: Vec<CaseAssetReviewItem>,
    pub omitted_review_item_count: usize,
    pub truncated: bool,
    pub deidentified: bool,
    pub raw_values_retained: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl CaseAssetManifest {
    /// Validate the manifest against a de-identified research request without projecting any
    /// report fields. This is used by intake to reject synthetic, identifier-bearing, malformed,
    /// or specialty-drifted metadata even when the required population snapshot is still absent.
    pub fn validate_for_request(
        &self,
        request: &CaseRequest,
        query: &CaseAssetManifestQuery,
    ) -> Result<(), NeurosurgeryError> {
        validate_request_and_query(self, request, query)
    }

    pub fn project(
        &self,
        request: &CaseRequest,
        query: &CaseAssetManifestQuery,
    ) -> Result<CaseAssetManifestReport, NeurosurgeryError> {
        self.validate_for_request(request, query)?;
        let request_digest = digest_value(request)?;
        let manifest_digest = digest_value(self)?;
        let requested_kinds = query.requested_kinds.clone().unwrap_or_default();
        let mut coverage = BTreeMap::<CaseAssetKind, CoverageAccumulator>::new();
        let mut summaries = Vec::with_capacity(self.assets.len());
        let mut review_candidates = Vec::<CaseAssetReviewItem>::new();
        let mut provenance_complete_asset_count = 0;

        for asset in &self.assets {
            let asset_ref = digest_text(&asset.asset_id);
            let source_ref = asset.source_id.as_deref().map(digest_text);
            let provenance_complete = asset.status == ObservationStatus::Observed
                && asset.source_id.is_some()
                && asset.content_sha256.is_some();
            let entry = coverage.entry(asset.kind).or_default();
            entry.total_count += 1;
            match asset.status {
                ObservationStatus::Observed => entry.observed_count += 1,
                ObservationStatus::NotCollected => entry.not_collected_count += 1,
                ObservationStatus::Uninterpretable => entry.uninterpretable_count += 1,
                ObservationStatus::Conflicting => entry.conflicting_count += 1,
            }
            if provenance_complete {
                entry.provenance_complete_count += 1;
                provenance_complete_asset_count += 1;
            }
            summaries.push(CaseAssetSummary {
                asset_ref: asset_ref.clone(),
                kind: asset.kind,
                status: asset.status,
                source_kind: asset.source_kind,
                source_ref,
                content_sha256: asset.content_sha256.clone(),
                modality: asset.modality.clone(),
                body_region: asset.body_region.clone(),
                observed_at: asset.observed_at.clone(),
                timepoint: asset.timepoint.clone(),
            });
            add_asset_review_items(asset, &asset_ref, &mut review_candidates);
        }

        let coverage = CaseAssetKind::ALL
            .iter()
            .copied()
            .map(|kind| {
                let counts = coverage.remove(&kind).unwrap_or_default();
                CaseAssetCoverage {
                    kind,
                    total_count: counts.total_count,
                    observed_count: counts.observed_count,
                    not_collected_count: counts.not_collected_count,
                    uninterpretable_count: counts.uninterpretable_count,
                    conflicting_count: counts.conflicting_count,
                    provenance_complete_count: counts.provenance_complete_count,
                }
            })
            .collect::<Vec<_>>();
        let observed_kinds = coverage
            .iter()
            .filter(|entry| entry.total_count > 0)
            .map(|entry| entry.kind)
            .collect::<BTreeSet<_>>();
        let missing_requested_kinds = requested_kinds
            .iter()
            .copied()
            .filter(|kind| !observed_kinds.contains(kind))
            .collect::<Vec<_>>();
        for kind in &missing_requested_kinds {
            review_candidates.push(CaseAssetReviewItem {
                sequence: 0,
                asset_ref: None,
                kind: Some(*kind),
                code: "requested_kind_missing".to_string(),
                reason: format!(
                    "the caller requested {kind:?} coverage, but no asset metadata was supplied"
                ),
            });
        }
        review_candidates.sort_by(|left, right| {
            left.asset_ref
                .cmp(&right.asset_ref)
                .then(left.kind.cmp(&right.kind))
                .then(left.code.cmp(&right.code))
        });
        let omitted_review_item_count = review_candidates
            .len()
            .saturating_sub(query.max_review_items);
        let truncated = omitted_review_item_count > 0;
        let review_items = review_candidates
            .into_iter()
            .take(query.max_review_items)
            .enumerate()
            .map(|(index, mut item)| {
                item.sequence = (index + 1) as u16;
                item
            })
            .collect::<Vec<_>>();
        let observed_asset_count = self
            .assets
            .iter()
            .filter(|asset| asset.status == ObservationStatus::Observed)
            .count();
        let mut report = CaseAssetManifestReport {
            schema_version: CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            request_digest,
            manifest_digest,
            report_digest: String::new(),
            specialty: request.specialty,
            asset_count: self.assets.len(),
            observed_asset_count,
            non_observed_asset_count: self.assets.len().saturating_sub(observed_asset_count),
            provenance_complete_asset_count,
            coverage,
            requested_kinds,
            missing_requested_kinds,
            assets: summaries,
            review_items,
            omitted_review_item_count,
            truncated,
            deidentified: true,
            raw_values_retained: false,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the manifest records caller-supplied asset metadata only; it never opens, parses, or interprets asset bytes".to_string(),
                "asset and source identifiers are returned only as SHA-256 references; the caller owns the local mapping".to_string(),
                "missing, uninterpretable, and conflicting statuses are review obligations, not negative findings".to_string(),
                "modality, anatomy, timing, and provenance labels are caller-declared and are not clinically validated".to_string(),
                "the report does not diagnose, prognosticate, triage, recommend treatment, or provide operative instructions".to_string(),
            ],
        };
        report.report_digest = digest_value(&report_without_digest(&report))?;
        Ok(report)
    }
}

impl CaseAssetManifestReport {
    /// Validate a persisted digest-only projection before it is joined to another report.
    ///
    /// The check proves that the projection is internally consistent and has not drifted since
    /// it was emitted. It cannot prove that the caller's upstream asset metadata is true; that
    /// remains a human-owned provenance question.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != CASE_ASSET_MANIFEST_SCHEMA_VERSION
            || !is_lower_hex_digest(&self.request_digest)
            || !is_lower_hex_digest(&self.manifest_digest)
            || !is_lower_hex_digest(&self.report_digest)
            || self.asset_count != self.assets.len()
            || self.observed_asset_count + self.non_observed_asset_count != self.asset_count
            || self.provenance_complete_asset_count > self.observed_asset_count
            || !self.deidentified
            || self.raw_values_retained
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.coverage.len() != CaseAssetKind::ALL.len()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report envelope is invalid".to_string(),
            });
        }

        let mut coverage_kinds = BTreeSet::new();
        let mut coverage_total = 0usize;
        let mut coverage_observed = 0usize;
        let mut coverage_provenance = 0usize;
        for coverage in &self.coverage {
            if !coverage_kinds.insert(coverage.kind)
                || coverage.observed_count
                    + coverage.not_collected_count
                    + coverage.uninterpretable_count
                    + coverage.conflicting_count
                    != coverage.total_count
                || coverage.provenance_complete_count > coverage.observed_count
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "case-asset manifest report coverage is inconsistent".to_string(),
                });
            }
            coverage_total = coverage_total.saturating_add(coverage.total_count);
            coverage_observed = coverage_observed.saturating_add(coverage.observed_count);
            coverage_provenance =
                coverage_provenance.saturating_add(coverage.provenance_complete_count);
        }
        if coverage_kinds.len() != CaseAssetKind::ALL.len()
            || coverage_total != self.asset_count
            || coverage_observed != self.observed_asset_count
            || coverage_provenance != self.provenance_complete_asset_count
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report coverage totals do not match assets"
                    .to_string(),
            });
        }

        validate_unique_kinds(&self.requested_kinds, "requested asset kinds")?;
        validate_unique_kinds(
            &self.missing_requested_kinds,
            "missing requested asset kinds",
        )?;
        if self
            .missing_requested_kinds
            .iter()
            .any(|kind| !self.requested_kinds.contains(kind))
            || self.missing_requested_kinds.iter().any(|kind| {
                self.coverage
                    .iter()
                    .find(|coverage| coverage.kind == *kind)
                    .is_some_and(|coverage| coverage.total_count != 0)
            })
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report requested-kind coverage is inconsistent"
                    .to_string(),
            });
        }

        let mut asset_refs = BTreeSet::new();
        let mut observed_count = 0usize;
        let mut provenance_count = 0usize;
        for asset in &self.assets {
            if !is_lower_hex_digest(&asset.asset_ref)
                || !asset_refs.insert(asset.asset_ref.as_str())
                || asset
                    .source_ref
                    .as_deref()
                    .is_some_and(|value| !is_lower_hex_digest(value))
                || asset
                    .content_sha256
                    .as_deref()
                    .is_some_and(|value| !is_lower_hex_digest(value))
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "case-asset manifest report contains an invalid or duplicate asset reference"
                        .to_string(),
                });
            }
            if asset.status == ObservationStatus::Observed {
                observed_count += 1;
                if asset.source_ref.is_some() && asset.content_sha256.is_some() {
                    provenance_count += 1;
                }
            }
            validate_optional_report_text(&asset.modality, MAX_MODALITY_BYTES)?;
            validate_optional_report_text(&asset.body_region, MAX_BODY_REGION_BYTES)?;
            validate_optional_report_text(&asset.observed_at, 32)?;
            validate_optional_report_text(&asset.timepoint, MAX_TIMEPOINT_BYTES)?;
        }
        if observed_count != self.observed_asset_count
            || provenance_count != self.provenance_complete_asset_count
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report asset counts do not match summaries"
                    .to_string(),
            });
        }

        // The exact candidate count is not retained in the report, so the persisted projection
        // can prove only that its returned items are bounded and that truncation agrees with the
        // omitted count.
        if self.review_items.len() > MAX_REVIEW_ITEMS
            || self.truncated != (self.omitted_review_item_count > 0)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report review bounds are invalid".to_string(),
            });
        }
        for (index, item) in self.review_items.iter().enumerate() {
            if item.sequence as usize != index + 1
                || item
                    .asset_ref
                    .as_deref()
                    .is_some_and(|value| !asset_refs.contains(value))
                || item.reason.trim().is_empty()
                || item.code.trim().is_empty()
                || item.code.len() > 256
                || item.reason.len() > 4096
                || item.code.chars().any(char::is_control)
                || item.reason.chars().any(char::is_control)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "case-asset manifest report review item is invalid".to_string(),
                });
            }
        }

        let mut unsigned = self.clone();
        unsigned.report_digest.clear();
        if digest_value(&unsigned)? != self.report_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Validate the projection against the exact request it claims to describe.
    pub fn validate_for_request(&self, request: &CaseRequest) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        if self.specialty != request.specialty || self.request_digest != digest_value(request)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report is bound to a different request".to_string(),
            });
        }
        Ok(())
    }

    /// Compose multiple independently projected, digest-only manifests for one request. This is
    /// used when a caller has more than one de-identified export (for example, DICOM metadata and
    /// FHIR metadata) describing the same case. The child projections are validated first, asset
    /// rows are unioned without identifier recovery, and duplicate digest references fail closed.
    /// The resulting `manifest_digest` binds the request, child manifest digests, and sorted asset
    /// references; no source payload or asset byte is opened.
    pub fn compose_for_request(
        request: &CaseRequest,
        reports: &[&CaseAssetManifestReport],
    ) -> Result<CaseAssetManifestReport, NeurosurgeryError> {
        if reports.is_empty() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "composing case-asset manifests requires at least one child report"
                    .to_string(),
            });
        }
        let request_digest = digest_value(request)?;
        let specialty = request.specialty;
        let mut assets = Vec::new();
        let mut requested_kinds = BTreeSet::new();
        let mut review_candidates = Vec::new();
        let mut omitted_review_item_count = 0usize;
        let mut child_manifest_digests = Vec::with_capacity(reports.len());

        for report in reports {
            report.validate_for_request(request)?;
            child_manifest_digests.push(report.manifest_digest.clone());
            requested_kinds.extend(report.requested_kinds.iter().copied());
            omitted_review_item_count =
                omitted_review_item_count.saturating_add(report.omitted_review_item_count);
            assets.extend(report.assets.iter().cloned());
            review_candidates.extend(report.review_items.iter().cloned());
        }
        if assets.len() > MAX_ASSETS {
            return Err(NeurosurgeryError::TooMany {
                field: "case_asset_manifest.composed_assets",
                found: assets.len(),
                max: MAX_ASSETS,
            });
        }
        assets.sort_by(|left, right| {
            left.asset_ref
                .cmp(&right.asset_ref)
                .then(left.kind.cmp(&right.kind))
        });
        if assets
            .windows(2)
            .any(|window| window[0].asset_ref == window[1].asset_ref)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "composed case-asset manifests contain duplicate asset references"
                    .to_string(),
            });
        }

        let mut coverage = Vec::with_capacity(CaseAssetKind::ALL.len());
        let mut observed_asset_count = 0usize;
        let mut provenance_complete_asset_count = 0usize;
        for kind in CaseAssetKind::ALL {
            let class = assets.iter().filter(|asset| asset.kind == kind);
            let mut row = CaseAssetCoverage {
                kind,
                total_count: 0,
                observed_count: 0,
                not_collected_count: 0,
                uninterpretable_count: 0,
                conflicting_count: 0,
                provenance_complete_count: 0,
            };
            for asset in class {
                row.total_count += 1;
                match asset.status {
                    ObservationStatus::Observed => {
                        row.observed_count += 1;
                        observed_asset_count += 1;
                        if asset.source_ref.is_some() && asset.content_sha256.is_some() {
                            row.provenance_complete_count += 1;
                            provenance_complete_asset_count += 1;
                        }
                    }
                    ObservationStatus::NotCollected => row.not_collected_count += 1,
                    ObservationStatus::Uninterpretable => row.uninterpretable_count += 1,
                    ObservationStatus::Conflicting => row.conflicting_count += 1,
                }
            }
            coverage.push(row);
        }
        let requested_kinds = requested_kinds.into_iter().collect::<Vec<_>>();
        let missing_requested_kinds = requested_kinds
            .iter()
            .copied()
            .filter(|kind| {
                coverage
                    .iter()
                    .all(|row| row.kind != *kind || row.total_count == 0)
            })
            .collect::<Vec<_>>();

        review_candidates.sort_by(|left, right| {
            left.asset_ref
                .cmp(&right.asset_ref)
                .then(left.kind.cmp(&right.kind))
                .then(left.code.cmp(&right.code))
        });
        let omitted_from_bound = review_candidates.len().saturating_sub(MAX_REVIEW_ITEMS);
        omitted_review_item_count = omitted_review_item_count.saturating_add(omitted_from_bound);
        let review_items = review_candidates
            .into_iter()
            .take(MAX_REVIEW_ITEMS)
            .enumerate()
            .map(|(index, mut item)| {
                item.sequence = (index + 1) as u16;
                item
            })
            .collect::<Vec<_>>();
        let asset_refs = assets
            .iter()
            .map(|asset| asset.asset_ref.as_str())
            .collect::<Vec<_>>();
        let manifest_identity = (request_digest.as_str(), child_manifest_digests, asset_refs);
        let manifest_digest = digest_value(&manifest_identity)?;
        let mut report = CaseAssetManifestReport {
            schema_version: CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            request_digest,
            manifest_digest,
            report_digest: String::new(),
            specialty,
            asset_count: assets.len(),
            observed_asset_count,
            non_observed_asset_count: assets.len().saturating_sub(observed_asset_count),
            provenance_complete_asset_count,
            coverage,
            requested_kinds,
            missing_requested_kinds,
            assets,
            review_items,
            omitted_review_item_count,
            truncated: omitted_review_item_count > 0,
            deidentified: true,
            raw_values_retained: false,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "this is a deterministic composition of independently validated digest-only asset projections; source payloads and asset bytes are never opened".to_string(),
                "asset and source identifiers remain SHA-256 references and are never recovered or joined by raw labels".to_string(),
                "missing, uninterpretable, and conflicting statuses remain reviewer obligations rather than negative findings".to_string(),
                "the report does not diagnose, prognosticate, triage, recommend treatment, or provide operative instructions".to_string(),
            ],
        };
        report.report_digest = digest_value(&report_without_digest(&report))?;
        report.validate_integrity()?;
        Ok(report)
    }

    /// Return whether this projection contains every digest-only asset row from `child` for the
    /// same request and specialty. This is a structural subset check for mission audits; it does
    /// not authenticate the caller's upstream export or establish clinical meaning.
    pub fn contains_projection(&self, child: &CaseAssetManifestReport) -> bool {
        self.validate_integrity().is_ok()
            && child.validate_integrity().is_ok()
            && self.request_digest == child.request_digest
            && self.specialty == child.specialty
            && child
                .assets
                .iter()
                .all(|asset| self.assets.iter().any(|candidate| candidate == asset))
    }
}

/// Top-level manifest accepted by the provider-free agent. `synthetic_data` is explicit so a
/// synthetic fixture cannot be mistaken for a real asset inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseAssetManifest {
    pub schema_version: String,
    pub specialty: Specialty,
    pub synthetic_data: bool,
    #[serde(default)]
    pub direct_identifier_fields: Vec<String>,
    pub assets: Vec<CaseAsset>,
}

#[derive(Debug, Clone, Default)]
struct CoverageAccumulator {
    total_count: usize,
    observed_count: usize,
    not_collected_count: usize,
    uninterpretable_count: usize,
    conflicting_count: usize,
    provenance_complete_count: usize,
}

fn validate_request_and_query(
    manifest: &CaseAssetManifest,
    request: &CaseRequest,
    query: &CaseAssetManifestQuery,
) -> Result<(), NeurosurgeryError> {
    if request.schema_version != crate::NEUROSURGERY_SCHEMA_VERSION {
        return Err(NeurosurgeryError::UnsupportedSchema {
            found: request.schema_version.clone(),
            expected: crate::NEUROSURGERY_SCHEMA_VERSION,
        });
    }
    if request.request_use.is_clinical() {
        return Err(NeurosurgeryError::ClinicalUseRefused {
            use_case: request.request_use,
            description: request.request_use.description(),
        });
    }
    if !request.direct_identifier_fields.is_empty() {
        return Err(NeurosurgeryError::DirectIdentifiers {
            fields: request.direct_identifier_fields.clone(),
        });
    }
    if manifest.schema_version != CASE_ASSET_MANIFEST_SCHEMA_VERSION {
        return Err(NeurosurgeryError::UnsupportedSchema {
            found: manifest.schema_version.clone(),
            expected: CASE_ASSET_MANIFEST_SCHEMA_VERSION,
        });
    }
    if manifest.specialty != request.specialty {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!(
                "case asset manifest specialty {:?} does not match request specialty {:?}",
                manifest.specialty, request.specialty
            ),
        });
    }
    if manifest.synthetic_data {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "case asset manifest declares synthetic_data=true; only real de-identified assets are accepted".to_string(),
        });
    }
    if !manifest.direct_identifier_fields.is_empty() {
        return Err(NeurosurgeryError::DirectIdentifiers {
            fields: manifest.direct_identifier_fields.clone(),
        });
    }
    if manifest.assets.len() > MAX_ASSETS {
        return Err(NeurosurgeryError::TooMany {
            field: "case_asset_manifest.assets",
            found: manifest.assets.len(),
            max: MAX_ASSETS,
        });
    }
    if query.max_review_items == 0 || query.max_review_items > MAX_REVIEW_ITEMS {
        return Err(NeurosurgeryError::TooMany {
            field: "case_asset_manifest.max_review_items",
            found: query.max_review_items,
            max: MAX_REVIEW_ITEMS,
        });
    }
    if let Some(requested_kinds) = &query.requested_kinds {
        if requested_kinds.is_empty() {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case asset requested_kinds must be non-empty when supplied".to_string(),
            });
        }
        let mut seen = BTreeSet::new();
        if requested_kinds.iter().any(|kind| !seen.insert(*kind)) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case asset requested_kinds must be unique".to_string(),
            });
        }
    }
    let mut seen_asset_ids = BTreeSet::new();
    for asset in &manifest.assets {
        validate_text(&asset.asset_id, "case_asset.asset_id", MAX_ASSET_ID_BYTES)?;
        if !seen_asset_ids.insert(&asset.asset_id) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!("duplicate case asset id {:?}", asset.asset_id),
            });
        }
        validate_optional_text(
            &asset.source_id,
            "case_asset.source_id",
            MAX_SOURCE_ID_BYTES,
        )?;
        validate_optional_text(&asset.modality, "case_asset.modality", MAX_MODALITY_BYTES)?;
        validate_optional_text(
            &asset.body_region,
            "case_asset.body_region",
            MAX_BODY_REGION_BYTES,
        )?;
        validate_optional_text(
            &asset.timepoint,
            "case_asset.timepoint",
            MAX_TIMEPOINT_BYTES,
        )?;
        if let Some(observed_at) = &asset.observed_at {
            validate_text(observed_at, "case_asset.observed_at", 32)?;
            if !crate::temporal::is_utc_timestamp(observed_at) {
                return Err(NeurosurgeryError::TemporalRejected {
                    reason: "case_asset.observed_at must be a UTC RFC3339 timestamp".to_string(),
                });
            }
        }
        if let Some(content_sha256) = &asset.content_sha256 {
            if content_sha256.len() != 64
                || !content_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "case asset {:?} content_sha256 must be 64 lowercase hexadecimal bytes",
                        asset.asset_id
                    ),
                });
            }
        }
    }
    Ok(())
}

fn add_asset_review_items(
    asset: &CaseAsset,
    asset_ref: &str,
    items: &mut Vec<CaseAssetReviewItem>,
) {
    let mut add = |code: &str, reason: &str| {
        items.push(CaseAssetReviewItem {
            sequence: 0,
            asset_ref: Some(asset_ref.to_string()),
            kind: Some(asset.kind),
            code: code.to_string(),
            reason: reason.to_string(),
        });
    };
    if asset.source_id.is_none() {
        add(
            "source_missing",
            "the asset has no caller-supplied provenance source identifier",
        );
    }
    if asset.status == ObservationStatus::Observed && asset.content_sha256.is_none() {
        add(
            "content_digest_missing",
            "an observed asset has no caller-supplied SHA-256 content digest",
        );
    }
    if asset.observed_at.is_none() {
        add(
            "timestamp_missing",
            "acquisition or assessment time is not supplied; longitudinal alignment remains unknown",
        );
    }
    if asset.kind == CaseAssetKind::ImagingSeries && asset.modality.is_none() {
        add(
            "imaging_modality_missing",
            "an imaging asset has no caller-supplied modality label",
        );
    }
    match asset.status {
        ObservationStatus::NotCollected => add(
            "asset_not_collected",
            "the caller marked this asset class as not collected; this is not a negative finding",
        ),
        ObservationStatus::Uninterpretable => add(
            "asset_uninterpretable",
            "the caller marked this asset as uninterpretable; qualified review is required",
        ),
        ObservationStatus::Conflicting => add(
            "asset_conflicting",
            "the caller marked this asset as conflicting; source reconciliation is required",
        ),
        ObservationStatus::Observed => {}
    }
}

fn report_without_digest(report: &CaseAssetManifestReport) -> CaseAssetManifestReport {
    let mut unsigned = report.clone();
    unsigned.report_digest.clear();
    unsigned
}

fn validate_unique_kinds(
    kinds: &[CaseAssetKind],
    label: &'static str,
) -> Result<(), NeurosurgeryError> {
    let mut seen = BTreeSet::new();
    if kinds.iter().any(|kind| !seen.insert(*kind)) {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("case-asset manifest report contains duplicate {label}"),
        });
    }
    Ok(())
}

fn validate_optional_report_text(
    value: &Option<String>,
    max: usize,
) -> Result<(), NeurosurgeryError> {
    if let Some(value) = value {
        if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "case-asset manifest report contains invalid text metadata".to_string(),
            });
        }
    }
    Ok(())
}

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn digest_value<T: Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn validate_optional_text(
    value: &Option<String>,
    field: &'static str,
    max: usize,
) -> Result<(), NeurosurgeryError> {
    if let Some(value) = value {
        validate_text(value, field, max)?;
    }
    Ok(())
}

fn validate_text(value: &str, field: &'static str, max: usize) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty() {
        return Err(NeurosurgeryError::EmptyField { field });
    }
    if value.len() > max {
        return Err(NeurosurgeryError::FieldTooLong { field, max });
    }
    if value.chars().any(char::is_control) {
        return Err(NeurosurgeryError::ControlCharacter { field });
    }
    Ok(())
}
