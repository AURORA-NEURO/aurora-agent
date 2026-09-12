//! Deterministic diffs between two validated public glioma snapshots.
//!
//! Snapshot refreshes are an important part of a real-data workflow. This module makes changes
//! explicit without pretending that a changed registry field is a clinical change: it reports
//! stable-record additions, removals, metadata changes, and provenance-source changes only.
//! Values such as abstracts are never copied into the diff; changed field names and source
//! identities are sufficient for a reviewer to locate the underlying snapshot records.

use crate::{NeurosurgeryError, RealDataRecordKind, RealGliomaBundle, RealSourceKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const REAL_DATA_DIFF_SCHEMA_VERSION: &str = "bioprism-neurosurgery-real-data-diff/0.1";
pub const MAX_REAL_DATA_DIFF_CHANGES: usize = 1024;

fn default_max_changes() -> usize {
    256
}

/// Bounded facets over the records and sources in two already validated snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDiffQuery {
    #[serde(default)]
    pub record_kind: Option<RealDataRecordKind>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default = "default_max_changes")]
    pub max_changes: usize,
}

impl Default for RealDataDiffQuery {
    fn default() -> Self {
        Self {
            record_kind: None,
            source_id: None,
            max_changes: default_max_changes(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataDiffChangeKind {
    Added,
    Removed,
    Changed,
}

/// Counts are descriptive change classes, not quality, recency, or clinical scores.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDiffCounts {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
}

impl RealDataDiffCounts {
    fn total(&self) -> usize {
        self.added
            .saturating_add(self.removed)
            .saturating_add(self.changed)
    }
}

/// One stable-record change. Added and removed rows populate only the corresponding before or
/// after fields; changed rows populate both and list the changed JSON field names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDiffRecordChange {
    pub record_kind: RealDataRecordKind,
    pub record_id: String,
    /// Molecular profiles use their study id as an explicit scope because profile identifiers are
    /// only unique within a study in the upstream cBioPortal contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<String>,
    pub change: RealDataDiffChangeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
}

/// One source-metadata change. Content hashes are included in `changed_fields`, allowing a
/// reviewer to distinguish a source refresh from a record-level update without exposing payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDiffSourceChange {
    pub source_id: String,
    pub change: RealDataDiffChangeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_kind: Option<RealSourceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_kind: Option<RealSourceKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed_fields: Vec<String>,
}

/// Digest-addressed refresh projection for caller-owned monitoring and human review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataDiffReport {
    pub schema_version: String,
    pub before_bundle_digest: String,
    pub after_bundle_digest: String,
    pub diff_digest: String,
    pub before_generated_at: String,
    pub after_generated_at: String,
    pub query: RealDataDiffQuery,
    pub before_record_count: usize,
    pub after_record_count: usize,
    pub record_counts: RealDataDiffCounts,
    pub source_counts: RealDataDiffCounts,
    pub total_change_count: usize,
    pub returned_change_count: usize,
    pub omitted_record_change_count: usize,
    pub omitted_source_change_count: usize,
    pub truncated: bool,
    pub record_changes: Vec<RealDataDiffRecordChange>,
    pub source_changes: Vec<RealDataDiffSourceChange>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataDiffReport {
    /// Validate a persisted snapshot diff without fetching or accepting a refresh.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        validate_query(&self.query)?;
        if self.schema_version != REAL_DATA_DIFF_SCHEMA_VERSION
            || !is_sha256_hex(&self.before_bundle_digest)
            || !is_sha256_hex(&self.after_bundle_digest)
            || !is_sha256_hex(&self.diff_digest)
            || !crate::temporal::is_utc_timestamp(&self.before_generated_at)
            || !crate::temporal::is_utc_timestamp(&self.after_generated_at)
            || self
                .record_counts
                .total()
                .saturating_add(self.source_counts.total())
                != self.total_change_count
            || self.returned_change_count
                != self
                    .record_changes
                    .len()
                    .saturating_add(self.source_changes.len())
            || self.record_changes.len() > self.query.max_changes
            || self.source_changes.len() > self.query.max_changes
            || self.truncated
                != (self.omitted_record_change_count > 0 || self.omitted_source_change_count > 0)
            || self.record_changes.iter().any(|change| {
                change.record_id.trim().is_empty()
                    || self
                        .query
                        .record_kind
                        .is_some_and(|kind| kind != change.record_kind)
                    || matches!(
                        change.change,
                        RealDataDiffChangeKind::Added
                            if change.before_source_id.is_some()
                                || change.before_title.is_some()
                                || change.after_source_id.is_none()
                                || change.after_title.is_none()
                                || !change.changed_fields.is_empty()
                    )
                    || matches!(
                        change.change,
                        RealDataDiffChangeKind::Removed
                            if change.after_source_id.is_some()
                                || change.after_title.is_some()
                                || change.before_source_id.is_none()
                                || change.before_title.is_none()
                                || !change.changed_fields.is_empty()
                    )
                    || matches!(
                        change.change,
                        RealDataDiffChangeKind::Changed
                            if change.before_source_id.is_none()
                                || change.after_source_id.is_none()
                                || change.before_title.is_none()
                                || change.after_title.is_none()
                                || change.changed_fields.is_empty()
                    )
                    || change
                        .changed_fields
                        .iter()
                        .any(|field| field.trim().is_empty())
                    || change
                        .changed_fields
                        .windows(2)
                        .any(|window| window[0] >= window[1])
            })
            || self
                .record_changes
                .windows(2)
                .any(|window| record_change_key(&window[0]) >= record_change_key(&window[1]))
            || self.source_changes.iter().any(|change| {
                change.source_id.trim().is_empty()
                    || self
                        .query
                        .source_id
                        .as_deref()
                        .is_some_and(|source_id| source_id != change.source_id)
                    || matches!(
                        change.change,
                        RealDataDiffChangeKind::Added
                            if change.before_kind.is_some()
                                || change.after_kind.is_none()
                                || !change.changed_fields.is_empty()
                    )
                    || matches!(
                        change.change,
                        RealDataDiffChangeKind::Removed
                            if change.after_kind.is_some()
                                || change.before_kind.is_none()
                                || !change.changed_fields.is_empty()
                    )
                    || matches!(
                        change.change,
                        RealDataDiffChangeKind::Changed
                            if change.before_kind.is_none()
                                || change.after_kind.is_none()
                                || change.changed_fields.is_empty()
                    )
                    || change
                        .changed_fields
                        .iter()
                        .any(|field| field.trim().is_empty())
                    || change
                        .changed_fields
                        .windows(2)
                        .any(|window| window[0] >= window[1])
            })
            || self
                .source_changes
                .windows(2)
                .any(|window| window[0].source_id >= window[1].source_id)
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data diff envelope is invalid".to_string(),
            });
        }
        if self.diff_digest
            != digest_diff(
                &self.before_bundle_digest,
                &self.after_bundle_digest,
                &self.query,
                &self.record_counts,
                &self.source_counts,
                &self.record_changes,
                &self.source_changes,
            )?
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data diff digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild a diff from the exact before/after validated snapshots and persisted query.
    pub fn validate_for_inputs(
        &self,
        before: &RealGliomaBundle,
        after: &RealGliomaBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = before.diff(after, &self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data diff does not replay to the exact supplied snapshots"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RecordKey {
    record_kind: RealDataRecordKind,
    record_id: String,
    scope_id: Option<String>,
}

#[derive(Debug, Clone)]
struct DiffRecord {
    key: RecordKey,
    source_id: String,
    title: String,
    value: Value,
}

#[derive(Debug, Clone)]
struct DiffSource {
    source_id: String,
    kind: RealSourceKind,
    value: Value,
}

impl RealGliomaBundle {
    /// Compare two validated snapshots without fetching, merging, scoring, or exposing source
    /// text. `self` is the before snapshot and `after` is the newer/candidate snapshot.
    pub fn diff(
        &self,
        after: &RealGliomaBundle,
        query: &RealDataDiffQuery,
    ) -> Result<RealDataDiffReport, NeurosurgeryError> {
        self.validate()?;
        after.validate()?;
        validate_query(query)?;
        let before_digest = self.summary()?.bundle_digest;
        let after_digest = after.summary()?.bundle_digest;
        let before_records = collect_records(self)?;
        let after_records = collect_records(after)?;
        let (record_changes, record_counts, omitted_record_change_count) =
            diff_records(&before_records, &after_records, query);
        let before_sources = collect_sources(self)?;
        let after_sources = collect_sources(after)?;
        let (source_changes, source_counts, omitted_source_change_count) =
            diff_sources(&before_sources, &after_sources, query);
        let total_change_count = record_counts.total() + source_counts.total();
        let returned_change_count = record_changes.len() + source_changes.len();
        let diff_digest = digest_diff(
            &before_digest,
            &after_digest,
            query,
            &record_counts,
            &source_counts,
            &record_changes,
            &source_changes,
        )?;
        let report = RealDataDiffReport {
            schema_version: REAL_DATA_DIFF_SCHEMA_VERSION.to_string(),
            before_bundle_digest: before_digest,
            after_bundle_digest: after_digest,
            diff_digest,
            before_generated_at: self.generated_at.clone(),
            after_generated_at: after.generated_at.clone(),
            query: query.clone(),
            before_record_count: record_count(self),
            after_record_count: record_count(after),
            record_counts,
            source_counts,
            total_change_count,
            returned_change_count,
            omitted_record_change_count,
            omitted_source_change_count,
            truncated: omitted_record_change_count > 0 || omitted_source_change_count > 0,
            record_changes,
            source_changes,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "changes are structural and metadata-level; they do not establish study quality, freshness, applicability, causality, or a clinical change".to_string(),
                "record IDs are stable public identifiers; changed fields are reported by name and source text is never copied into the diff".to_string(),
                "record_kind filters record changes; source changes remain a bundle-level projection unless source_id is selected".to_string(),
                "a removed record means it is absent from the after snapshot, not that the upstream authority deleted it".to_string(),
                "the comparison never fetches URLs, invokes a model, opens credentials, exposes patient/sample values, or writes durable state".to_string(),
            ],
        };
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &RealDataDiffQuery) -> Result<(), NeurosurgeryError> {
    if query.max_changes == 0 || query.max_changes > MAX_REAL_DATA_DIFF_CHANGES {
        return Err(NeurosurgeryError::TooMany {
            field: "real_data_diff.max_changes",
            found: query.max_changes,
            max: MAX_REAL_DATA_DIFF_CHANGES,
        });
    }
    if let Some(source_id) = query.source_id.as_deref() {
        if source_id.is_empty() || source_id.len() > 512 || source_id.chars().any(char::is_control)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "real-data diff source_id is empty, too long, or contains a control character"
                        .to_string(),
            });
        }
    }
    Ok(())
}

fn record_count(bundle: &RealGliomaBundle) -> usize {
    bundle.clinical_trials.len()
        + bundle.genomic_projects.len()
        + bundle.portal_studies.len()
        + bundle.portal_molecular_profiles.len()
        + bundle.references.len()
        + bundle.literature.len()
}

fn collect_records(bundle: &RealGliomaBundle) -> Result<Vec<DiffRecord>, NeurosurgeryError> {
    let mut records = Vec::with_capacity(record_count(bundle));
    for record in &bundle.clinical_trials {
        records.push(diff_record(
            RealDataRecordKind::ClinicalTrial,
            &record.nct_id,
            None,
            &record.source_id,
            &record.title,
            record,
        )?);
    }
    for record in &bundle.genomic_projects {
        records.push(diff_record(
            RealDataRecordKind::GenomicProject,
            &record.project_id,
            None,
            &record.source_id,
            &record.name,
            record,
        )?);
    }
    for record in &bundle.portal_studies {
        records.push(diff_record(
            RealDataRecordKind::PortalStudy,
            &record.study_id,
            None,
            &record.source_id,
            &record.name,
            record,
        )?);
    }
    for record in &bundle.portal_molecular_profiles {
        records.push(diff_record(
            RealDataRecordKind::PortalMolecularProfile,
            &record.profile_id,
            Some(&record.study_id),
            &record.source_id,
            &record.name,
            record,
        )?);
    }
    for record in &bundle.references {
        records.push(diff_record(
            RealDataRecordKind::GuidelineReference,
            &record.reference_id,
            None,
            &record.source_id,
            &record.title,
            record,
        )?);
    }
    for record in &bundle.literature {
        records.push(diff_record(
            RealDataRecordKind::LiteratureArticle,
            &record.pmid,
            None,
            &record.source_id,
            &record.title,
            record,
        )?);
    }
    records.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(records)
}

fn diff_record<T: Serialize>(
    record_kind: RealDataRecordKind,
    record_id: &str,
    scope_id: Option<&str>,
    source_id: &str,
    title: &str,
    record: &T,
) -> Result<DiffRecord, NeurosurgeryError> {
    let value = serde_json::to_value(record)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    Ok(DiffRecord {
        key: RecordKey {
            record_kind,
            record_id: record_id.to_string(),
            scope_id: scope_id.map(str::to_string),
        },
        source_id: source_id.to_string(),
        title: title.to_string(),
        value,
    })
}

fn collect_sources(bundle: &RealGliomaBundle) -> Result<Vec<DiffSource>, NeurosurgeryError> {
    let mut sources = bundle
        .sources
        .iter()
        .map(|source| {
            Ok(DiffSource {
                source_id: source.source_id.clone(),
                kind: source.kind,
                value: serde_json::to_value(source)
                    .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?,
            })
        })
        .collect::<Result<Vec<_>, NeurosurgeryError>>()?;
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    Ok(sources)
}

fn record_matches(record: &DiffRecord, query: &RealDataDiffQuery) -> bool {
    query
        .record_kind
        .is_none_or(|kind| kind == record.key.record_kind)
        && query
            .source_id
            .as_deref()
            .is_none_or(|source_id| source_id == record.source_id)
}

fn diff_records(
    before: &[DiffRecord],
    after: &[DiffRecord],
    query: &RealDataDiffQuery,
) -> (Vec<RealDataDiffRecordChange>, RealDataDiffCounts, usize) {
    let before_map = before
        .iter()
        .map(|record| (record.key.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let after_map = after
        .iter()
        .map(|record| (record.key.clone(), record))
        .collect::<BTreeMap<_, _>>();
    let keys = before_map
        .keys()
        .chain(after_map.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut all = Vec::new();
    let mut counts = RealDataDiffCounts::default();
    for key in keys {
        let prior = before_map.get(&key).copied();
        let current = after_map.get(&key).copied();
        if !prior.is_some_and(|record| record_matches(record, query))
            && !current.is_some_and(|record| record_matches(record, query))
        {
            continue;
        }
        let change = match (prior, current) {
            (None, Some(current)) => {
                counts.added += 1;
                RealDataDiffRecordChange {
                    record_kind: current.key.record_kind,
                    record_id: current.key.record_id.clone(),
                    scope_id: current.key.scope_id.clone(),
                    change: RealDataDiffChangeKind::Added,
                    before_source_id: None,
                    after_source_id: Some(current.source_id.clone()),
                    before_title: None,
                    after_title: Some(current.title.clone()),
                    changed_fields: Vec::new(),
                }
            }
            (Some(prior), None) => {
                counts.removed += 1;
                RealDataDiffRecordChange {
                    record_kind: prior.key.record_kind,
                    record_id: prior.key.record_id.clone(),
                    scope_id: prior.key.scope_id.clone(),
                    change: RealDataDiffChangeKind::Removed,
                    before_source_id: Some(prior.source_id.clone()),
                    after_source_id: None,
                    before_title: Some(prior.title.clone()),
                    after_title: None,
                    changed_fields: Vec::new(),
                }
            }
            (Some(prior), Some(current)) if prior.value != current.value => {
                counts.changed += 1;
                RealDataDiffRecordChange {
                    record_kind: current.key.record_kind,
                    record_id: current.key.record_id.clone(),
                    scope_id: current.key.scope_id.clone(),
                    change: RealDataDiffChangeKind::Changed,
                    before_source_id: Some(prior.source_id.clone()),
                    after_source_id: Some(current.source_id.clone()),
                    before_title: Some(prior.title.clone()),
                    after_title: Some(current.title.clone()),
                    changed_fields: changed_fields(&prior.value, &current.value),
                }
            }
            (Some(_), Some(_)) => continue,
            (None, None) => continue,
        };
        all.push(change);
    }
    let omitted = all.len().saturating_sub(query.max_changes);
    all.truncate(query.max_changes);
    (all, counts, omitted)
}

fn diff_sources(
    before: &[DiffSource],
    after: &[DiffSource],
    query: &RealDataDiffQuery,
) -> (Vec<RealDataDiffSourceChange>, RealDataDiffCounts, usize) {
    let before_map = before
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let after_map = after
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let keys = before_map
        .keys()
        .chain(after_map.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut all = Vec::new();
    let mut counts = RealDataDiffCounts::default();
    for source_id in keys {
        if query
            .source_id
            .as_deref()
            .is_some_and(|selected| selected != source_id)
        {
            continue;
        }
        let prior = before_map.get(source_id).copied();
        let current = after_map.get(source_id).copied();
        let change = match (prior, current) {
            (None, Some(current)) => {
                counts.added += 1;
                RealDataDiffSourceChange {
                    source_id: source_id.to_string(),
                    change: RealDataDiffChangeKind::Added,
                    before_kind: None,
                    after_kind: Some(current.kind),
                    changed_fields: Vec::new(),
                }
            }
            (Some(prior), None) => {
                counts.removed += 1;
                RealDataDiffSourceChange {
                    source_id: source_id.to_string(),
                    change: RealDataDiffChangeKind::Removed,
                    before_kind: Some(prior.kind),
                    after_kind: None,
                    changed_fields: Vec::new(),
                }
            }
            (Some(prior), Some(current)) if prior.value != current.value => {
                counts.changed += 1;
                RealDataDiffSourceChange {
                    source_id: source_id.to_string(),
                    change: RealDataDiffChangeKind::Changed,
                    before_kind: Some(prior.kind),
                    after_kind: Some(current.kind),
                    changed_fields: changed_fields(&prior.value, &current.value),
                }
            }
            (Some(_), Some(_)) | (None, None) => continue,
        };
        all.push(change);
    }
    let omitted = all.len().saturating_sub(query.max_changes);
    all.truncate(query.max_changes);
    (all, counts, omitted)
}

fn changed_fields(before: &Value, after: &Value) -> Vec<String> {
    let (Some(before), Some(after)) = (before.as_object(), after.as_object()) else {
        return vec!["record".to_string()];
    };
    let keys = before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    keys.into_iter()
        .filter(|key| before.get(key) != after.get(key))
        .collect()
}

fn digest_diff(
    before_digest: &str,
    after_digest: &str,
    query: &RealDataDiffQuery,
    record_counts: &RealDataDiffCounts,
    source_counts: &RealDataDiffCounts,
    record_changes: &[RealDataDiffRecordChange],
    source_changes: &[RealDataDiffSourceChange],
) -> Result<String, NeurosurgeryError> {
    let payload = (
        before_digest,
        after_digest,
        query,
        record_counts,
        source_counts,
        record_changes,
        source_changes,
    );
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn record_change_key(
    change: &RealDataDiffRecordChange,
) -> (RealDataRecordKind, &str, Option<&str>) {
    (
        change.record_kind,
        change.record_id.as_str(),
        change.scope_id.as_deref(),
    )
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
