//! Bounded, de-identification-first FHIR metadata intake for real neurosurgical cases.
//!
//! This module intentionally does not parse imaging, pathology, genomics, operative notes, or
//! patient narratives. It accepts a caller-sanitised FHIR `Bundle`, inspects only resource
//! identity/type metadata and an explicit asset-kind hint, and projects the result into the
//! existing digest-only [`CaseAssetManifestReport`]. The original JSON is never returned and no
//! clinical meaning is inferred from codes or observations.

use crate::case_asset_manifest::{
    CaseAssetKind, CaseAssetManifest, CaseAssetManifestQuery, CaseAssetManifestReport,
};
use crate::{CaseRequest, NeurosurgeryError, ObservationStatus, Specialty};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const CASE_FHIR_IMPORT_SCHEMA_VERSION: &str = "bioprism-neurosurgery-case-fhir-import/0.1";
pub const CASE_FHIR_ASSET_KIND_EXTENSION_URL: &str =
    "https://aurora-neuro.dev/fhir/StructureDefinition/case-asset-kind";

const MAX_BUNDLE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RESOURCES: usize = 256;
const MAX_HINTS: usize = 256;
const MAX_RESOURCE_ID_BYTES: usize = 128;
const MAX_SOURCE_ID_BYTES: usize = 128;
const MAX_REVIEW_ITEMS: usize = 512;
const DEFAULT_MAX_REVIEW_ITEMS: usize = 128;

fn default_max_review_items() -> usize {
    DEFAULT_MAX_REVIEW_ITEMS
}

/// A caller-owned, sanitized FHIR Bundle plus explicit metadata mapping hints.
///
/// `bundle` is accepted only as an in-memory JSON value. The importer never follows references,
/// opens URLs, reads files, or returns the value. `resource_hints` are the safe seam for mapping a
/// FHIR resource to one of the inventory classes; absent hints are reported as review obligations
/// rather than guessed from clinical codes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FhirCaseImport {
    pub schema_version: String,
    pub specialty: Specialty,
    pub deidentified: bool,
    pub synthetic_data: bool,
    pub source_id: String,
    pub bundle: Value,
    #[serde(default)]
    pub resource_hints: Vec<FhirResourceHint>,
    #[serde(default)]
    pub query: FhirCaseImportQuery,
}

/// Explicit mapping and provenance metadata for one FHIR resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FhirResourceHint {
    pub resource_id: String,
    pub asset_kind: CaseAssetKind,
    pub status: ObservationStatus,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub content_sha256: Option<String>,
    #[serde(default)]
    pub modality: Option<String>,
    #[serde(default)]
    pub body_region: Option<String>,
    #[serde(default)]
    pub observed_at: Option<String>,
    #[serde(default)]
    pub timepoint: Option<String>,
}

/// Bounded projection controls for FHIR import. The default reports every safe resource and
/// returns at most 128 review rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FhirCaseImportQuery {
    #[serde(default)]
    pub requested_kinds: Option<Vec<CaseAssetKind>>,
    #[serde(default = "default_max_review_items")]
    pub max_review_items: usize,
}

impl Default for FhirCaseImportQuery {
    fn default() -> Self {
        Self {
            requested_kinds: None,
            max_review_items: default_max_review_items(),
        }
    }
}

/// A bounded reviewer obligation emitted when a FHIR resource cannot be safely projected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FhirCaseImportReviewItem {
    pub sequence: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    pub code: String,
    pub reason: String,
}

/// Digest-bound result of a FHIR metadata import. It contains no source JSON or raw patient text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FhirCaseImportReport {
    pub schema_version: String,
    pub request_digest: String,
    pub bundle_digest: String,
    pub hints_digest: String,
    pub report_digest: String,
    pub specialty: Specialty,
    pub resource_count: usize,
    pub projected_asset_count: usize,
    pub unclassified_resource_count: usize,
    pub manifest_report: CaseAssetManifestReport,
    pub review_items: Vec<FhirCaseImportReviewItem>,
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

impl FhirCaseImport {
    /// Project a real, caller-sanitized FHIR bundle into the existing metadata-only asset route.
    pub fn project(
        &self,
        request: &CaseRequest,
    ) -> Result<FhirCaseImportReport, NeurosurgeryError> {
        validate_request_and_import(self, request)?;
        let bundle_bytes = serde_json::to_vec(&self.bundle)
            .map_err(|error| NeurosurgeryError::Json(error.to_string()))?;
        if bundle_bytes.len() > MAX_BUNDLE_BYTES {
            return Err(NeurosurgeryError::TooMany {
                field: "fhir_case_import.bundle_bytes",
                found: bundle_bytes.len(),
                max: MAX_BUNDLE_BYTES,
            });
        }
        let bundle_digest = digest_bytes(&bundle_bytes);
        let hints_digest = digest_value(&self.resource_hints)?;
        let request_digest = digest_value(request)?;
        let entries = bundle_entries(&self.bundle)?;
        if entries.len() > MAX_RESOURCES {
            return Err(NeurosurgeryError::TooMany {
                field: "fhir_case_import.bundle.entry",
                found: entries.len(),
                max: MAX_RESOURCES,
            });
        }

        let mut hints = BTreeMap::<String, &FhirResourceHint>::new();
        for hint in &self.resource_hints {
            if hints.insert(hint.resource_id.clone(), hint).is_some() {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("duplicate FHIR resource hint {:?}", hint.resource_id),
                });
            }
        }

        let mut seen_resource_ids = BTreeSet::new();
        let mut assets = Vec::new();
        let mut review_candidates = Vec::new();
        let mut resource_types = BTreeMap::<String, String>::new();
        for entry in entries {
            let resource =
                entry
                    .get("resource")
                    .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                        reason: "FHIR Bundle entries must contain a resource object".to_string(),
                    })?;
            let resource =
                resource
                    .as_object()
                    .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                        reason: "FHIR Bundle entry.resource must be an object".to_string(),
                    })?;
            let resource_type = resource
                .get("resourceType")
                .and_then(Value::as_str)
                .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                    reason: "FHIR resourceType is required for every Bundle resource".to_string(),
                })?;
            validate_text(resource_type, "fhir.resourceType", MAX_RESOURCE_ID_BYTES)?;
            if is_identity_resource_type(resource_type) {
                return Err(NeurosurgeryError::DirectIdentifiers {
                    fields: vec![format!("resourceType:{resource_type}")],
                });
            }
            if let Some(forbidden) = forbidden_key(resource) {
                return Err(NeurosurgeryError::DirectIdentifiers {
                    fields: vec![format!("fhir.{forbidden}")],
                });
            }
            let resource_id = resource.get("id").and_then(Value::as_str).ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: format!("FHIR {resource_type} resource is missing a de-identified id"),
                }
            })?;
            validate_text(resource_id, "fhir.resource.id", MAX_RESOURCE_ID_BYTES)?;
            if !seen_resource_ids.insert(resource_id.to_string()) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("duplicate FHIR resource id {resource_id:?}"),
                });
            }
            resource_types.insert(resource_id.to_string(), resource_type.to_string());

            let extension_kind = explicit_extension_kind(resource)?;
            let hint = hints.get(resource_id).copied();
            let kind = match (hint.map(|value| value.asset_kind), extension_kind) {
                (Some(hint_kind), Some(extension_kind)) if hint_kind != extension_kind => {
                    add_review(
                        &mut review_candidates,
                        Some(resource_id),
                        Some(resource_type),
                        "asset_kind_conflict",
                        "the explicit hint and FHIR asset-kind extension disagree; no asset was projected",
                    );
                    None
                }
                (Some(hint_kind), _) => Some(hint_kind),
                (None, Some(extension_kind)) => Some(extension_kind),
                (None, None) => {
                    add_review(
                        &mut review_candidates,
                        Some(resource_id),
                        Some(resource_type),
                        "asset_kind_missing",
                        "resource metadata has no explicit caller-owned asset-kind hint; no clinical class was inferred",
                    );
                    None
                }
            };
            let Some(kind) = kind else { continue };
            let Some(hint) = hint else {
                add_review(
                    &mut review_candidates,
                    Some(resource_id),
                    Some(resource_type),
                    "asset_metadata_missing",
                    "an asset-kind extension was present but no caller hint supplied an explicit status and provenance metadata",
                );
                continue;
            };
            let source_id = hint
                .source_id
                .clone()
                .or_else(|| Some(self.source_id.clone()));
            assets.push(crate::CaseAsset {
                asset_id: resource_id.to_string(),
                kind,
                status: hint.status,
                source_kind: crate::CaseAssetSourceKind::CallerExport,
                source_id,
                content_sha256: hint.content_sha256.clone(),
                modality: hint.modality.clone(),
                body_region: hint.body_region.clone(),
                observed_at: hint.observed_at.clone(),
                timepoint: hint.timepoint.clone(),
            });
        }
        for hint in &self.resource_hints {
            if !resource_types.contains_key(&hint.resource_id) {
                add_review(
                    &mut review_candidates,
                    Some(&hint.resource_id),
                    None,
                    "hint_resource_missing",
                    "the caller supplied a resource hint that does not match any Bundle resource",
                );
            }
        }

        let manifest = CaseAssetManifest {
            schema_version: crate::CASE_ASSET_MANIFEST_SCHEMA_VERSION.to_string(),
            specialty: self.specialty,
            synthetic_data: false,
            direct_identifier_fields: Vec::new(),
            assets,
        };
        let manifest_query = CaseAssetManifestQuery {
            requested_kinds: self.query.requested_kinds.clone(),
            max_review_items: self.query.max_review_items,
        };
        let manifest_report = manifest.project(request, &manifest_query)?;
        review_candidates.sort_by(|left, right| {
            left.resource_ref
                .cmp(&right.resource_ref)
                .then(left.resource_type.cmp(&right.resource_type))
                .then(left.code.cmp(&right.code))
        });
        let omitted_review_item_count = review_candidates
            .len()
            .saturating_sub(self.query.max_review_items);
        let truncated = omitted_review_item_count > 0;
        let review_items = review_candidates
            .into_iter()
            .take(self.query.max_review_items)
            .enumerate()
            .map(|(index, mut item)| {
                item.sequence = (index + 1) as u16;
                item
            })
            .collect::<Vec<_>>();
        let projected_asset_count = manifest_report.asset_count;
        let mut report = FhirCaseImportReport {
            schema_version: CASE_FHIR_IMPORT_SCHEMA_VERSION.to_string(),
            request_digest,
            bundle_digest,
            hints_digest,
            report_digest: String::new(),
            specialty: self.specialty,
            resource_count: resource_types.len(),
            projected_asset_count,
            unclassified_resource_count: resource_types.len().saturating_sub(projected_asset_count),
            manifest_report,
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
                "only FHIR resourceType/id and explicit asset-kind metadata are inspected; clinical codes, measurements, narratives, and references are not interpreted".to_string(),
                "the source Bundle and all raw values are discarded after digesting; resource and source identifiers appear only as SHA-256 references in the nested manifest".to_string(),
                "the caller must de-identify the export and provide content digests; an import is not evidence that an asset is clinically valid or complete".to_string(),
                "unclassified, missing, uninterpretable, and conflicting resources remain reviewer obligations rather than negative findings".to_string(),
                "this report does not diagnose, prognosticate, triage, recommend treatment, or provide operative instructions".to_string(),
            ],
        };
        report.report_digest = digest_value(&report_without_digest(&report))?;
        Ok(report)
    }
}

impl FhirCaseImportReport {
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != CASE_FHIR_IMPORT_SCHEMA_VERSION
            || !is_lower_hex_digest(&self.request_digest)
            || !is_lower_hex_digest(&self.bundle_digest)
            || !is_lower_hex_digest(&self.hints_digest)
            || !is_lower_hex_digest(&self.report_digest)
            || self.resource_count < self.projected_asset_count
            || self.unclassified_resource_count
                != self
                    .resource_count
                    .saturating_sub(self.projected_asset_count)
            || !self.deidentified
            || self.raw_values_retained
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.review_items.len() > MAX_REVIEW_ITEMS
            || self.truncated != (self.omitted_review_item_count > 0)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "FHIR import report envelope is invalid".to_string(),
            });
        }
        self.manifest_report.validate_integrity()?;
        if self.manifest_report.asset_count != self.projected_asset_count
            || self.manifest_report.specialty != self.specialty
            || self.manifest_report.request_digest != self.request_digest
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "FHIR import report manifest count or specialty is inconsistent"
                    .to_string(),
            });
        }
        for (index, item) in self.review_items.iter().enumerate() {
            if item.sequence as usize != index + 1
                || item
                    .resource_ref
                    .as_deref()
                    .is_some_and(|value| !is_lower_hex_digest(value))
                || item.resource_type.as_ref().is_some_and(|value| {
                    value.trim().is_empty()
                        || value.len() > MAX_RESOURCE_ID_BYTES
                        || value.chars().any(char::is_control)
                })
                || item.code.trim().is_empty()
                || item.reason.trim().is_empty()
                || item.code.len() > 256
                || item.reason.len() > 4096
                || item.code.chars().any(char::is_control)
                || item.reason.chars().any(char::is_control)
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "FHIR import report review item is invalid".to_string(),
                });
            }
        }
        let mut unsigned = self.clone();
        unsigned.report_digest.clear();
        if digest_value(&unsigned)? != self.report_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "FHIR import report digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Replay the exact local import against the same request and FHIR envelope.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        import: &FhirCaseImport,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let replay = import.project(request)?;
        if replay.report_digest != self.report_digest {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "FHIR import report does not replay from the supplied request and Bundle"
                    .to_string(),
            });
        }
        Ok(())
    }
}

fn validate_request_and_import(
    import: &FhirCaseImport,
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
    if import.schema_version != CASE_FHIR_IMPORT_SCHEMA_VERSION {
        return Err(NeurosurgeryError::UnsupportedSchema {
            found: import.schema_version.clone(),
            expected: CASE_FHIR_IMPORT_SCHEMA_VERSION,
        });
    }
    if import.specialty != request.specialty {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "FHIR import specialty does not match the CaseRequest specialty".to_string(),
        });
    }
    if !import.deidentified {
        return Err(NeurosurgeryError::DirectIdentifiers {
            fields: vec!["fhir_bundle.deidentified=false".to_string()],
        });
    }
    if import.synthetic_data {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "FHIR import declares synthetic_data=true; only real de-identified exports are accepted".to_string(),
        });
    }
    validate_text(
        &import.source_id,
        "fhir_case_import.source_id",
        MAX_SOURCE_ID_BYTES,
    )?;
    if import.resource_hints.len() > MAX_HINTS {
        return Err(NeurosurgeryError::TooMany {
            field: "fhir_case_import.resource_hints",
            found: import.resource_hints.len(),
            max: MAX_HINTS,
        });
    }
    if import.query.max_review_items == 0 || import.query.max_review_items > MAX_REVIEW_ITEMS {
        return Err(NeurosurgeryError::TooMany {
            field: "fhir_case_import.query.max_review_items",
            found: import.query.max_review_items,
            max: MAX_REVIEW_ITEMS,
        });
    }
    if let Some(kinds) = &import.query.requested_kinds {
        let mut seen_kinds = BTreeSet::new();
        if kinds.is_empty() || kinds.iter().any(|kind| !seen_kinds.insert(*kind)) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "FHIR import requested_kinds must be non-empty and unique when supplied"
                    .to_string(),
            });
        }
    }
    let mut seen = BTreeSet::new();
    for hint in &import.resource_hints {
        validate_text(
            &hint.resource_id,
            "fhir_resource_hint.resource_id",
            MAX_RESOURCE_ID_BYTES,
        )?;
        if !seen.insert(&hint.resource_id) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!("duplicate FHIR resource hint {:?}", hint.resource_id),
            });
        }
        validate_optional_text(
            &hint.source_id,
            "fhir_resource_hint.source_id",
            MAX_SOURCE_ID_BYTES,
        )?;
        validate_optional_text(
            &hint.modality,
            "fhir_resource_hint.modality",
            MAX_SOURCE_ID_BYTES,
        )?;
        validate_optional_text(
            &hint.body_region,
            "fhir_resource_hint.body_region",
            MAX_SOURCE_ID_BYTES,
        )?;
        validate_optional_text(
            &hint.timepoint,
            "fhir_resource_hint.timepoint",
            MAX_SOURCE_ID_BYTES,
        )?;
        if let Some(value) = &hint.observed_at {
            validate_text(value, "fhir_resource_hint.observed_at", 32)?;
            if !crate::temporal::is_utc_timestamp(value) {
                return Err(NeurosurgeryError::TemporalRejected {
                    reason: "FHIR resource hint observed_at must be a UTC RFC3339 timestamp"
                        .to_string(),
                });
            }
        }
        if let Some(value) = &hint.content_sha256 {
            if !is_lower_hex_digest(value) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "FHIR resource hint content_sha256 must be 64 lowercase hexadecimal bytes"
                            .to_string(),
                });
            }
        }
    }
    Ok(())
}

fn bundle_entries(bundle: &Value) -> Result<&Vec<Value>, NeurosurgeryError> {
    let object = bundle
        .as_object()
        .ok_or_else(|| NeurosurgeryError::RealDataRejected {
            reason: "FHIR import bundle must be a JSON object".to_string(),
        })?;
    if object.get("resourceType").and_then(Value::as_str) != Some("Bundle") {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "FHIR import requires resourceType=Bundle".to_string(),
        });
    }
    if let Some(forbidden) = forbidden_key(object) {
        return Err(NeurosurgeryError::DirectIdentifiers {
            fields: vec![format!("fhir.{forbidden}")],
        });
    }
    object
        .get("entry")
        .and_then(Value::as_array)
        .ok_or_else(|| NeurosurgeryError::RealDataRejected {
            reason: "FHIR Bundle.entry must be an array".to_string(),
        })
}

fn explicit_extension_kind(
    resource: &serde_json::Map<String, Value>,
) -> Result<Option<CaseAssetKind>, NeurosurgeryError> {
    let Some(extensions) = resource.get("extension") else {
        return Ok(None);
    };
    let Some(extensions) = extensions.as_array() else {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "FHIR resource extension must be an array when supplied".to_string(),
        });
    };
    for extension in extensions {
        let Some(extension) = extension.as_object() else {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "FHIR resource extension entries must be objects".to_string(),
            });
        };
        if extension.get("url").and_then(Value::as_str) != Some(CASE_FHIR_ASSET_KIND_EXTENSION_URL)
        {
            continue;
        }
        let value = extension
            .get("valueCode")
            .and_then(Value::as_str)
            .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                reason: "FHIR asset-kind extension must carry valueCode".to_string(),
            })?;
        let kind = match value {
            "imaging_series" => CaseAssetKind::ImagingSeries,
            "pathology_report" => CaseAssetKind::PathologyReport,
            "molecular_assay" => CaseAssetKind::MolecularAssay,
            "operative_note" => CaseAssetKind::OperativeNote,
            "neurofunctional_assessment" => CaseAssetKind::NeurofunctionalAssessment,
            "developmental_assessment" => CaseAssetKind::DevelopmentalAssessment,
            "longitudinal_outcome" => CaseAssetKind::LongitudinalOutcome,
            "anatomical_model" => CaseAssetKind::AnatomicalModel,
            _ => {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!("unknown FHIR asset-kind extension code {value:?}"),
                })
            }
        };
        return Ok(Some(kind));
    }
    Ok(None)
}

fn forbidden_key(object: &serde_json::Map<String, Value>) -> Option<&'static str> {
    const FORBIDDEN: &[&str] = &[
        "name",
        "telecom",
        "address",
        "birthDate",
        "deceased",
        "photo",
        "contact",
        "link",
        "identifier",
        "subject",
        "patient",
        "encounter",
        "author",
        "performer",
        "recorder",
        "requester",
        "agent",
        "reference",
        "fullUrl",
        "display",
        "text",
        "div",
        "description",
        "note",
        "comment",
        "valueString",
        "valueMarkdown",
        "valueXhtml",
    ];
    for (key, value) in object {
        let normalized = key.to_ascii_lowercase();
        if FORBIDDEN.contains(&key.as_str())
            || [
                "identifier",
                "patient",
                "subject",
                "reference",
                "fullname",
                "birth",
                "address",
                "telecom",
                "narrative",
            ]
            .iter()
            .any(|fragment| normalized.contains(fragment))
        {
            if let Some(candidate) = FORBIDDEN.iter().find(|candidate| **candidate == key) {
                return Some(candidate);
            }
            return Some("sensitive_field");
        }
        if let Some(child) = value.as_object() {
            if let Some(found) = forbidden_key(child) {
                return Some(found);
            }
        }
        if let Some(children) = value.as_array() {
            for child in children {
                if let Some(child) = child.as_object() {
                    if let Some(found) = forbidden_key(child) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

fn is_identity_resource_type(resource_type: &str) -> bool {
    matches!(
        resource_type,
        "Patient"
            | "Person"
            | "RelatedPerson"
            | "Practitioner"
            | "PractitionerRole"
            | "Organization"
    )
}

fn add_review(
    items: &mut Vec<FhirCaseImportReviewItem>,
    resource_id: Option<&str>,
    resource_type: Option<&str>,
    code: &str,
    reason: &str,
) {
    items.push(FhirCaseImportReviewItem {
        sequence: 0,
        resource_ref: resource_id.map(digest_text),
        resource_type: resource_type.map(str::to_string),
        code: code.to_string(),
        reason: reason.to_string(),
    });
}

fn report_without_digest(report: &FhirCaseImportReport) -> FhirCaseImportReport {
    let mut unsigned = report.clone();
    unsigned.report_digest.clear();
    unsigned
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

fn is_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
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
