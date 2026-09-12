//! De-identification-first DICOM JSON metadata intake for neurosurgical research.
//!
//! This module accepts the standard DICOM JSON model (for example, metadata exported by a
//! caller's `dcm2json`/DICOMweb pipeline) and projects only series-level metadata into the
//! existing digest-only case-asset boundary. Pixel data, private tags, patient identifiers, and
//! free-text clinical interpretation never enter the report. A dataset with incomplete metadata
//! remains a reviewer obligation rather than a guessed imaging finding.

use crate::case_asset_manifest::{
    CaseAssetKind, CaseAssetManifest, CaseAssetManifestQuery, CaseAssetManifestReport,
};
use crate::{CaseRequest, NeurosurgeryError, ObservationStatus, Specialty};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const CASE_DICOM_IMPORT_SCHEMA_VERSION: &str = "bioprism-neurosurgery-case-dicom-import/0.1";

const MAX_METADATA_BYTES: usize = 4 * 1024 * 1024;
const MAX_DATASETS: usize = 512;
const MAX_REVIEW_ITEMS: usize = 1024;
const MAX_TEXT_BYTES: usize = 256;

// Standard DICOM JSON tags. Values are read only when their VR is compatible with a textual
// representation; unrecognised/private tags are ignored for projection but still included in the
// metadata digest. Patient-identifying tags are rejected before any projection.
const TAG_MODALITY: &str = "00080060";
const TAG_BODY_PART: &str = "00180015";
const TAG_STUDY_UID: &str = "0020000D";
const TAG_SERIES_UID: &str = "0020000E";
const TAG_SOP_UID: &str = "00080018";
const TAG_STUDY_DATE: &str = "00080020";
const TAG_SERIES_DATE: &str = "00080021";
const TAG_STUDY_DESCRIPTION: &str = "00081030";
const TAG_SERIES_DESCRIPTION: &str = "0008103E";
const TAG_SERIES_NUMBER: &str = "00200011";
const TAG_PIXEL_DATA: &str = "7FE00010";

const FORBIDDEN_IDENTIFIER_TAGS: &[&str] = &[
    "00100010", // PatientName
    "00100020", // PatientID
    "00100030", // PatientBirthDate
    "00100040", // PatientSex
    "00080050", // AccessionNumber
    "00080090", // ReferringPhysicianName
    "00081050", // PerformingPhysicianName
    "00081070", // OperatorsName
    "00101000", // OtherPatientIDs
    "00101001", // OtherPatientNames
    "00101002", // OtherPatientIDsSequence
    "00101005", // PatientBirthName
    "00101010", // PatientAge
    "00101040", // PatientAddress
    "00101060", // PatientMotherBirthName
    "00101080", // MilitaryRank
    "00101090", // MedicalRecordLocator
    "00102154", // PatientTelephoneNumbers
    "00102160", // EthnicGroup
];

fn default_max_review_items() -> usize {
    128
}

/// Caller-owned DICOM JSON metadata. `datasets` accepts either a single DICOM JSON dataset or an
/// array of datasets, which keeps the seam usable with both dcm2json and DICOMweb exports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DicomCaseImport {
    pub schema_version: String,
    pub specialty: Specialty,
    pub deidentified: bool,
    pub synthetic_data: bool,
    pub source_id: String,
    pub datasets: Value,
    #[serde(default)]
    pub query: DicomCaseImportQuery,
}

/// Bounded projection controls for DICOM JSON metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DicomCaseImportQuery {
    #[serde(default)]
    pub requested_kinds: Option<Vec<CaseAssetKind>>,
    #[serde(default = "default_max_review_items")]
    pub max_review_items: usize,
    /// When true, datasets without a SeriesInstanceUID receive a stable index-based reference.
    /// This does not create a DICOM identity and always emits a missing-UID review obligation.
    #[serde(default)]
    pub allow_missing_series_uid: bool,
}

impl Default for DicomCaseImportQuery {
    fn default() -> Self {
        Self {
            requested_kinds: None,
            max_review_items: default_max_review_items(),
            allow_missing_series_uid: false,
        }
    }
}

/// A digest-only projection of one DICOM dataset/series.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DicomSeriesMetadata {
    pub dataset_index: usize,
    pub series_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sop_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_number: Option<String>,
    pub metadata_digest: String,
}

/// A bounded reviewer obligation for missing or unsafe DICOM metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DicomCaseImportReviewItem {
    pub sequence: u16,
    pub dataset_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_ref: Option<String>,
    pub code: String,
    pub reason: String,
}

/// Digest-bound DICOM metadata import report. It contains no dataset JSON, pixel data, or raw
/// patient identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DicomCaseImportReport {
    pub schema_version: String,
    pub request_digest: String,
    pub datasets_digest: String,
    pub report_digest: String,
    pub specialty: Specialty,
    pub dataset_count: usize,
    pub projected_series_count: usize,
    pub unclassified_dataset_count: usize,
    pub series: Vec<DicomSeriesMetadata>,
    pub manifest_report: CaseAssetManifestReport,
    pub review_items: Vec<DicomCaseImportReviewItem>,
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

impl DicomCaseImport {
    /// Project DICOM JSON metadata into the existing case-asset manifest without opening bytes or
    /// interpreting a scan. The caller remains responsible for de-identification and content
    /// digests of the original DICOM objects.
    pub fn project(
        &self,
        request: &CaseRequest,
    ) -> Result<DicomCaseImportReport, NeurosurgeryError> {
        validate_request_and_import(self, request)?;
        let datasets_bytes = serde_json::to_vec(&self.datasets)
            .map_err(|error| NeurosurgeryError::Json(error.to_string()))?;
        if datasets_bytes.len() > MAX_METADATA_BYTES {
            return Err(NeurosurgeryError::TooMany {
                field: "dicom_case_import.datasets_bytes",
                found: datasets_bytes.len(),
                max: MAX_METADATA_BYTES,
            });
        }
        let datasets_digest = digest_bytes(&datasets_bytes);
        let request_digest = digest_value(request)?;
        let datasets = dataset_values(&self.datasets)?;
        if datasets.len() > MAX_DATASETS {
            return Err(NeurosurgeryError::TooMany {
                field: "dicom_case_import.datasets",
                found: datasets.len(),
                max: MAX_DATASETS,
            });
        }

        let mut series = Vec::new();
        let mut assets = Vec::new();
        let mut review_candidates = Vec::new();
        let mut seen_series_uids = BTreeSet::new();
        for (index, dataset) in datasets.iter().enumerate() {
            let object =
                dataset
                    .as_object()
                    .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                        reason: format!("DICOM dataset {index} must be a JSON object"),
                    })?;
            validate_dataset_tags(object, index)?;
            let series_uid = text_tag(object, TAG_SERIES_UID)?;
            let study_uid = text_tag(object, TAG_STUDY_UID)?;
            let sop_uid = text_tag(object, TAG_SOP_UID)?;
            let series_ref = match series_uid.as_deref() {
                Some(uid) => {
                    if !seen_series_uids.insert(uid.to_string()) {
                        add_review(
                            &mut review_candidates,
                            index,
                            Some(digest_text(uid)),
                            "duplicate_series_uid",
                            "multiple datasets carry the same SeriesInstanceUID; review whether they represent one series",
                        );
                        continue;
                    }
                    digest_text(uid)
                }
                None if self.query.allow_missing_series_uid => {
                    let reference = format!("dataset-{index}");
                    add_review(
                        &mut review_candidates,
                        index,
                        Some(reference.clone()),
                        "series_uid_missing",
                        "SeriesInstanceUID is absent; an index reference was used only because allow_missing_series_uid=true",
                    );
                    reference
                }
                None => {
                    add_review(
                        &mut review_candidates,
                        index,
                        None,
                        "series_uid_missing",
                        "SeriesInstanceUID is absent; no imaging asset was projected",
                    );
                    continue;
                }
            };
            let modality = text_tag(object, TAG_MODALITY)?;
            let body_region = text_tag(object, TAG_BODY_PART)?;
            let study_date = date_tag(object, TAG_STUDY_DATE)?;
            let series_date = date_tag(object, TAG_SERIES_DATE)?;
            let study_description = text_tag(object, TAG_STUDY_DESCRIPTION)?;
            let series_description = text_tag(object, TAG_SERIES_DESCRIPTION)?;
            let series_number = text_tag(object, TAG_SERIES_NUMBER)?;
            let metadata_digest = digest_value(dataset)?;
            if modality.is_none() {
                add_review(
                    &mut review_candidates,
                    index,
                    Some(series_ref.clone()),
                    "modality_missing",
                    "Modality is absent from the DICOM dataset; the series remains metadata-only",
                );
            }
            if body_region.is_none() {
                add_review(
                    &mut review_candidates,
                    index,
                    Some(series_ref.clone()),
                    "body_region_missing",
                    "BodyPartExamined is absent; an anatomic region was not inferred",
                );
            }
            if study_date.is_none() && series_date.is_none() {
                add_review(
                    &mut review_candidates,
                    index,
                    Some(series_ref.clone()),
                    "acquisition_date_missing",
                    "StudyDate and SeriesDate are absent; temporal ordering remains unknown",
                );
            }
            add_review(
                &mut review_candidates,
                index,
                Some(series_ref.clone()),
                "content_digest_missing",
                "the caller supplied metadata but no DICOM object-byte digest; provenance is incomplete until the caller binds one",
            );
            series.push(DicomSeriesMetadata {
                dataset_index: index,
                series_ref: series_ref.clone(),
                study_ref: study_uid.as_deref().map(digest_text),
                sop_ref: sop_uid.as_deref().map(digest_text),
                modality: modality.clone(),
                body_region: body_region.clone(),
                study_date: study_date.clone(),
                series_date: series_date.clone(),
                study_description,
                series_description,
                series_number,
                metadata_digest,
            });
            assets.push(crate::CaseAsset {
                asset_id: series_ref,
                kind: CaseAssetKind::ImagingSeries,
                status: ObservationStatus::Observed,
                source_kind: crate::CaseAssetSourceKind::DicomArchive,
                source_id: Some(self.source_id.clone()),
                content_sha256: None,
                modality,
                body_region,
                observed_at: None,
                timepoint: None,
            });
        }
        series.sort_by(|left, right| {
            left.series_ref
                .cmp(&right.series_ref)
                .then(left.dataset_index.cmp(&right.dataset_index))
        });
        assets.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
        review_candidates.sort_by(|left, right| {
            left.dataset_index
                .cmp(&right.dataset_index)
                .then(left.series_ref.cmp(&right.series_ref))
                .then(left.code.cmp(&right.code))
        });
        let omitted_review_item_count = review_candidates
            .len()
            .saturating_sub(self.query.max_review_items);
        let review_items = review_candidates
            .into_iter()
            .take(self.query.max_review_items)
            .enumerate()
            .map(|(index, item)| DicomCaseImportReviewItem {
                sequence: (index + 1) as u16,
                dataset_index: item.dataset_index,
                series_ref: item.series_ref,
                code: item.code,
                reason: item.reason,
            })
            .collect::<Vec<_>>();
        let manifest = CaseAssetManifest {
            schema_version: crate::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            specialty: self.specialty,
            synthetic_data: false,
            direct_identifier_fields: Vec::new(),
            assets,
        };
        let manifest_report = manifest.project(
            request,
            &CaseAssetManifestQuery {
                requested_kinds: self.query.requested_kinds.clone(),
                max_review_items: self.query.max_review_items.min(512),
            },
        )?;
        let mut report = DicomCaseImportReport {
            schema_version: CASE_DICOM_IMPORT_SCHEMA_VERSION.to_string(),
            request_digest,
            datasets_digest,
            report_digest: String::new(),
            specialty: self.specialty,
            dataset_count: datasets.len(),
            projected_series_count: series.len(),
            unclassified_dataset_count: datasets.len().saturating_sub(series.len()),
            series,
            manifest_report,
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
                "only standard DICOM JSON metadata tags are inspected; pixel data and clinical content are never opened or interpreted".to_string(),
                "patient-identifying tags and private tags are rejected or ignored; the caller remains responsible for complete de-identification".to_string(),
                "DICOM dates are retained as date-only values because timezone and acquisition-time semantics are not inferred".to_string(),
                "series metadata is projected into a digest-only imaging asset inventory; missing object-byte digests remain reviewer obligations".to_string(),
                "this report does not diagnose, prognosticate, triage, recommend treatment, or provide operative instructions".to_string(),
            ],
        };
        report.report_digest = digest_value(&report_without_digest(&report))?;
        report.validate_integrity()?;
        Ok(report)
    }
}

impl DicomCaseImportReport {
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != CASE_DICOM_IMPORT_SCHEMA_VERSION
            || !is_digest(&self.request_digest)
            || !is_digest(&self.datasets_digest)
            || !is_digest(&self.report_digest)
            || self.dataset_count < self.projected_series_count
            || self.unclassified_dataset_count
                != self
                    .dataset_count
                    .saturating_sub(self.projected_series_count)
            || self.series.len() != self.projected_series_count
            || self.review_items.len() > MAX_REVIEW_ITEMS
            || self.truncated != (self.omitted_review_item_count > 0)
            || !self.deidentified
            || self.raw_values_retained
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM import report envelope is invalid".to_string(),
            });
        }
        self.manifest_report.validate_integrity()?;
        if self.manifest_report.specialty != self.specialty
            || self.manifest_report.request_digest != self.request_digest
            || self.manifest_report.asset_count != self.projected_series_count
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM import manifest binding is invalid".to_string(),
            });
        }
        if self.series.windows(2).any(|window| {
            window[0].series_ref > window[1].series_ref
                || (window[0].series_ref == window[1].series_ref
                    && window[0].dataset_index >= window[1].dataset_index)
        }) || self.series.iter().any(|row| {
            row.series_ref.trim().is_empty()
                || (!row.series_ref.starts_with("dataset-") && !is_digest(&row.series_ref))
                || (row.series_ref.starts_with("dataset-")
                    && row.series_ref != format!("dataset-{}", row.dataset_index))
                || row.dataset_index >= self.dataset_count
                || row
                    .study_ref
                    .as_deref()
                    .is_some_and(|value| !is_digest(value))
                || row
                    .sop_ref
                    .as_deref()
                    .is_some_and(|value| !is_digest(value))
                || !is_digest(&row.metadata_digest)
                || row
                    .modality
                    .as_deref()
                    .is_some_and(|value| value.len() > MAX_TEXT_BYTES)
                || row
                    .body_region
                    .as_deref()
                    .is_some_and(|value| value.len() > MAX_TEXT_BYTES)
                || row
                    .study_description
                    .as_deref()
                    .is_some_and(|value| value.len() > MAX_TEXT_BYTES)
                || row
                    .series_description
                    .as_deref()
                    .is_some_and(|value| value.len() > MAX_TEXT_BYTES)
                || row
                    .series_number
                    .as_deref()
                    .is_some_and(|value| value.len() > MAX_TEXT_BYTES)
                || row
                    .study_date
                    .as_deref()
                    .is_some_and(|value| !is_dicom_date(value))
                || row
                    .series_date
                    .as_deref()
                    .is_some_and(|value| !is_dicom_date(value))
        }) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM series projection is not canonical".to_string(),
            });
        }
        for (index, item) in self.review_items.iter().enumerate() {
            if item.sequence as usize != index + 1
                || item.dataset_index >= self.dataset_count
                || item.code.trim().is_empty()
                || item.reason.trim().is_empty()
                || item.reason.chars().any(char::is_control)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "DICOM import review item is invalid".to_string(),
                });
            }
        }
        if digest_value(&report_without_digest(self))? != self.report_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM import report digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        import: &DicomCaseImport,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let replay = import.project(request)?;
        if &replay != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "DICOM import report does not replay from the supplied request and metadata"
                        .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ReviewCandidate {
    dataset_index: usize,
    series_ref: Option<String>,
    code: String,
    reason: String,
}

fn add_review(
    target: &mut Vec<ReviewCandidate>,
    dataset_index: usize,
    series_ref: Option<String>,
    code: &str,
    reason: &str,
) {
    target.push(ReviewCandidate {
        dataset_index,
        series_ref,
        code: code.to_string(),
        reason: reason.to_string(),
    });
}

fn validate_request_and_import(
    import: &DicomCaseImport,
    request: &CaseRequest,
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
    if import.schema_version != CASE_DICOM_IMPORT_SCHEMA_VERSION {
        return Err(NeurosurgeryError::UnsupportedSchema {
            found: import.schema_version.clone(),
            expected: CASE_DICOM_IMPORT_SCHEMA_VERSION,
        });
    }
    if import.specialty != request.specialty {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "DICOM import specialty does not match the CaseRequest specialty".to_string(),
        });
    }
    if !import.deidentified {
        return Err(NeurosurgeryError::DirectIdentifiers {
            fields: vec!["dicom.deidentified=false".to_string()],
        });
    }
    if import.synthetic_data {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "DICOM import declares synthetic_data=true; only real de-identified metadata is accepted".to_string(),
        });
    }
    validate_text(&import.source_id, "dicom_case_import.source_id")?;
    if import.query.max_review_items == 0 || import.query.max_review_items > MAX_REVIEW_ITEMS {
        return Err(NeurosurgeryError::TooMany {
            field: "dicom_case_import.query.max_review_items",
            found: import.query.max_review_items,
            max: MAX_REVIEW_ITEMS,
        });
    }
    if let Some(kinds) = &import.query.requested_kinds {
        let mut seen = BTreeSet::new();
        if kinds.is_empty() || kinds.iter().any(|kind| !seen.insert(*kind)) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM requested_kinds must be non-empty and unique when supplied"
                    .to_string(),
            });
        }
        if kinds
            .iter()
            .any(|kind| *kind != CaseAssetKind::ImagingSeries)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "DICOM import can project only imaging_series assets".to_string(),
            });
        }
    }
    Ok(())
}

fn dataset_values(value: &Value) -> Result<Vec<Value>, NeurosurgeryError> {
    match value {
        Value::Array(values) => Ok(values.clone()),
        Value::Object(_) => Ok(vec![value.clone()]),
        _ => Err(NeurosurgeryError::RealDataRejected {
            reason: "DICOM datasets must be a JSON object or array of objects".to_string(),
        }),
    }
}

fn validate_dataset_tags(
    object: &Map<String, Value>,
    index: usize,
) -> Result<(), NeurosurgeryError> {
    for tag in object.keys() {
        let normalized = tag.to_ascii_uppercase();
        if FORBIDDEN_IDENTIFIER_TAGS.contains(&normalized.as_str()) {
            return Err(NeurosurgeryError::DirectIdentifiers {
                fields: vec![format!("dicom.dataset[{index}].tag:{normalized}")],
            });
        }
        if normalized == TAG_PIXEL_DATA {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!("DICOM dataset {index} contains PixelData; metadata-only import refuses pixel bytes"),
            });
        }
        if tag.len() != 8 || !tag.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!("DICOM dataset {index} contains a non-canonical tag key"),
            });
        }
    }
    Ok(())
}

fn text_tag(object: &Map<String, Value>, tag: &str) -> Result<Option<String>, NeurosurgeryError> {
    let Some(element) = object
        .get(tag)
        .or_else(|| object.get(&tag.to_ascii_lowercase()))
    else {
        return Ok(None);
    };
    let Some(element) = element.as_object() else {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("DICOM tag {tag} must be a JSON element object"),
        });
    };
    let Some(value) = element.get("Value") else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("DICOM tag {tag} Value must be an array"),
        });
    };
    let Some(first) = values.first() else {
        return Ok(None);
    };
    let Some(text) = first.as_str() else {
        return Ok(None);
    };
    validate_text(text, "dicom.text_tag")?;
    if text.len() > MAX_TEXT_BYTES {
        return Err(NeurosurgeryError::FieldTooLong {
            field: "dicom.text_tag",
            max: MAX_TEXT_BYTES,
        });
    }
    Ok((!text.trim().is_empty()).then(|| text.trim().to_string()))
}

fn date_tag(object: &Map<String, Value>, tag: &str) -> Result<Option<String>, NeurosurgeryError> {
    let value = text_tag(object, tag)?;
    if let Some(value) = value.as_deref() {
        if !is_dicom_date(value) {
            return Err(NeurosurgeryError::TemporalRejected {
                reason: format!("DICOM date tag {tag} must be YYYYMMDD when present"),
            });
        }
    }
    Ok(value)
}

fn is_dicom_date(value: &str) -> bool {
    if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let Ok(year) = value[0..4].parse::<u16>() else {
        return false;
    };
    let Ok(month) = value[4..6].parse::<u8>() else {
        return false;
    };
    let Ok(day) = value[6..8].parse::<u8>() else {
        return false;
    };
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    day >= 1 && day <= days_in_month[(month - 1) as usize]
}

fn validate_text(value: &str, field: &'static str) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty() {
        return Err(NeurosurgeryError::EmptyField { field });
    }
    if value.chars().any(char::is_control) {
        return Err(NeurosurgeryError::ControlCharacter { field });
    }
    Ok(())
}

fn digest_value<T: Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(digest_bytes(&bytes))
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn digest_text(value: &str) -> String {
    digest_bytes(value.as_bytes())
}

fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn report_without_digest(report: &DicomCaseImportReport) -> DicomCaseImportReport {
    let mut clone = report.clone();
    clone.report_digest.clear();
    clone
}
