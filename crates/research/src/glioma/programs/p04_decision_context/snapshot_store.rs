//! Local immutable decision-context snapshot indexing for long-running glioma programs.
//!
//! The reducer models the durable part of a site-local context store without exporting the
//! context payload. Every accepted snapshot is content-bound to a validated [`DecisionContext`]
//! digest and an event lineage. Parent chains are checked before retention, so a crash can only
//! restore a complete, verified decision frontier. Retention is conservative: pinned, referenced,
//! and ancestor snapshots are never evicted, and corruption remains visible as negative evidence.

use super::context_compiler::DecisionContext;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionContextSnapshotIndex1@1";
pub const MAX_SNAPSHOTS: usize = 512;
pub const MAX_EVENTS_PER_SNAPSHOT: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextSnapshotInput {
    pub snapshot_id: String,
    pub study_id: String,
    pub epoch: u32,
    pub parent_snapshot_id: Option<String>,
    pub context: DecisionContext,
    pub event_order: Vec<String>,
    pub pinned: bool,
    pub referenced: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextSnapshotStoreRequest {
    pub study_id: String,
    pub max_retained_snapshots: usize,
    pub snapshots: Vec<DecisionContextSnapshotInput>,
    pub restore_snapshot_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextSnapshotRecord {
    pub snapshot_id: String,
    pub study_id: String,
    pub epoch: u32,
    pub parent_snapshot_id: Option<String>,
    pub context_digest: ContentHash,
    pub event_order: Vec<String>,
    pub pinned: bool,
    pub referenced: bool,
    pub record_digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStoreDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionContextSnapshotIndex {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub records: Vec<DecisionContextSnapshotRecord>,
    pub retained_order: Vec<String>,
    pub evicted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub recovery_snapshot_id: Option<String>,
    pub integrity_status: String,
    pub disposition: SnapshotStoreDisposition,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionContextSnapshotStoreError {
    #[error("snapshot-store request is invalid: {0}")]
    InvalidRequest(String),
    #[error("snapshot-store index is invalid: {0}")]
    InvalidOutput(String),
    #[error("snapshot-store digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn record_body(record: &DecisionContextSnapshotRecord) -> serde_json::Value {
    serde_json::json!({
        "snapshot_id": record.snapshot_id,
        "study_id": record.study_id,
        "epoch": record.epoch,
        "parent_snapshot_id": record.parent_snapshot_id,
        "context_digest": record.context_digest,
        "event_order": record.event_order,
        "pinned": record.pinned,
        "referenced": record.referenced,
    })
}

fn index_body(index: &DecisionContextSnapshotIndex) -> serde_json::Value {
    serde_json::json!({
        "feature_id": index.feature_id,
        "output_schema": index.output_schema,
        "study_id": index.study_id,
        "records": index.records,
        "retained_order": index.retained_order,
        "evicted_order": index.evicted_order,
        "rejected_order": index.rejected_order,
        "recovery_snapshot_id": index.recovery_snapshot_id,
        "integrity_status": index.integrity_status,
        "disposition": index.disposition,
        "omissions": index.omissions,
        "negative_evidence": index.negative_evidence,
    })
}

fn validate_request(
    request: &DecisionContextSnapshotStoreRequest,
) -> Result<(), DecisionContextSnapshotStoreError> {
    if !safe_text(&request.study_id)
        || request.max_retained_snapshots == 0
        || request.max_retained_snapshots > MAX_SNAPSHOTS
        || request.snapshots.is_empty()
        || request.snapshots.len() > MAX_SNAPSHOTS
    {
        return Err(DecisionContextSnapshotStoreError::InvalidRequest(
            "study, bounded retention, and at least one snapshot are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for snapshot in &request.snapshots {
        if !safe_text(&snapshot.snapshot_id)
            || !ids.insert(snapshot.snapshot_id.clone())
            || snapshot.study_id != request.study_id
            || snapshot.epoch == 0
            || snapshot
                .parent_snapshot_id
                .as_deref()
                .is_some_and(|parent| !safe_text(parent) || parent == snapshot.snapshot_id)
            || snapshot.event_order.len() > MAX_EVENTS_PER_SNAPSHOT
            || snapshot.event_order.iter().any(|event| !safe_text(event))
            || !canonical(&snapshot.event_order)
        {
            return Err(DecisionContextSnapshotStoreError::InvalidRequest(format!(
                "snapshot {} is malformed, duplicated, cross-study, or non-canonical",
                snapshot.snapshot_id
            )));
        }
    }
    Ok(())
}

fn build_record(
    snapshot: &DecisionContextSnapshotInput,
) -> Result<DecisionContextSnapshotRecord, DecisionContextSnapshotStoreError> {
    snapshot
        .context
        .validate()
        .map_err(|error| DecisionContextSnapshotStoreError::InvalidRequest(error.to_string()))?;
    let mut record = DecisionContextSnapshotRecord {
        snapshot_id: snapshot.snapshot_id.clone(),
        study_id: snapshot.study_id.clone(),
        epoch: snapshot.epoch,
        parent_snapshot_id: snapshot.parent_snapshot_id.clone(),
        context_digest: snapshot.context.digest.clone(),
        event_order: snapshot.event_order.clone(),
        pinned: snapshot.pinned,
        referenced: snapshot.referenced,
        record_digest: ContentHash::of_bytes(b"unsealed-glioma-context-snapshot"),
    };
    record.record_digest = ContentHash::of_value(&record_body(&record))
        .map_err(|error| DecisionContextSnapshotStoreError::Digest(error.to_string()))?;
    Ok(record)
}

fn has_complete_parent_chain(
    snapshot_id: &str,
    records: &BTreeMap<String, DecisionContextSnapshotRecord>,
) -> bool {
    let mut seen = BTreeSet::new();
    let mut current = Some(snapshot_id.to_string());
    while let Some(id) = current {
        if !seen.insert(id.clone()) {
            return false;
        }
        let Some(record) = records.get(&id) else {
            return false;
        };
        current = record.parent_snapshot_id.clone();
    }
    true
}

fn add_ancestors(
    snapshot_id: &str,
    records: &BTreeMap<String, DecisionContextSnapshotRecord>,
    retained: &mut BTreeSet<String>,
) {
    let mut current = Some(snapshot_id.to_string());
    while let Some(id) = current {
        if !retained.insert(id.clone()) {
            break;
        }
        current = records
            .get(&id)
            .and_then(|record| record.parent_snapshot_id.clone());
    }
}

impl DecisionContextSnapshotIndex {
    pub fn validate(&self) -> Result<(), DecisionContextSnapshotStoreError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.study_id)
            || !canonical(&self.retained_order)
            || !canonical(&self.evicted_order)
            || !canonical(&self.rejected_order)
            || self.records.iter().any(|record| {
                record.study_id != self.study_id
                    || !safe_text(&record.snapshot_id)
                    || record.epoch == 0
                    || !canonical(&record.event_order)
                    || ContentHash::of_value(&record_body(record))
                        .map(|digest| digest != record.record_digest)
                        .unwrap_or(true)
            })
            || self
                .retained_order
                .iter()
                .any(|id| !self.records.iter().any(|record| &record.snapshot_id == id))
            || !self
                .recovery_snapshot_id
                .as_ref()
                .is_none_or(|id| self.retained_order.binary_search(id).is_ok())
            || self.digest.as_str().len() != 64
        {
            return Err(DecisionContextSnapshotStoreError::InvalidOutput(
                "snapshot identity, ordering, record digest, retention, recovery, or bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&index_body(self))
            .map_err(|error| DecisionContextSnapshotStoreError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionContextSnapshotStoreError::InvalidOutput(
                "snapshot index digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Reduce local immutable context snapshots into a crash-recoverable index.
pub fn store_glioma_decision_context_snapshots(
    request: &DecisionContextSnapshotStoreRequest,
) -> Result<DecisionContextSnapshotIndex, DecisionContextSnapshotStoreError> {
    validate_request(request)?;
    let mut records = BTreeMap::new();
    let mut rejected_order = Vec::new();
    let mut negative_evidence = Vec::new();
    for snapshot in &request.snapshots {
        match build_record(snapshot) {
            Ok(record) => {
                records.insert(record.snapshot_id.clone(), record);
            }
            Err(error) => {
                rejected_order.push(snapshot.snapshot_id.clone());
                negative_evidence.push(format!(
                    "{} rejected because its context failed validation: {error}",
                    snapshot.snapshot_id
                ));
            }
        }
    }
    let all_ids = records.keys().cloned().collect::<Vec<_>>();
    for id in &all_ids {
        let Some(record) = records.get(id) else {
            continue;
        };
        if let Some(parent) = &record.parent_snapshot_id {
            let invalid_parent = records
                .get(parent)
                .map(|parent_record| parent_record.epoch >= record.epoch)
                .unwrap_or(true);
            if invalid_parent {
                records.remove(id);
                rejected_order.push(id.clone());
                negative_evidence.push(format!(
                    "{id} rejected because its parent chain is missing or not epoch-prior"
                ));
            }
        }
    }
    let mut omissions = Vec::new();
    let mut retained = BTreeSet::new();
    for record in records
        .values()
        .filter(|record| record.pinned || record.referenced)
    {
        add_ancestors(&record.snapshot_id, &records, &mut retained);
    }
    let mut epoch_order = records.values().collect::<Vec<_>>();
    epoch_order.sort_by(|left, right| {
        left.epoch
            .cmp(&right.epoch)
            .then(left.snapshot_id.cmp(&right.snapshot_id))
    });
    if let Some(latest) = epoch_order.last() {
        add_ancestors(&latest.snapshot_id, &records, &mut retained);
    }
    if retained.len() > request.max_retained_snapshots {
        omissions.push(format!(
            "retention budget {} is below the protected snapshot/ancestor set {}",
            request.max_retained_snapshots,
            retained.len()
        ));
    } else {
        for record in epoch_order.iter().rev() {
            if retained.len() >= request.max_retained_snapshots {
                break;
            }
            add_ancestors(&record.snapshot_id, &records, &mut retained);
        }
    }
    let mut evicted_order = records
        .keys()
        .filter(|id| !retained.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    evicted_order.sort();
    rejected_order.sort();
    rejected_order.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    let retained_order = retained.iter().cloned().collect::<Vec<_>>();
    let recovery_snapshot_id = request
        .restore_snapshot_id
        .as_ref()
        .filter(|id| retained.contains(*id) && has_complete_parent_chain(id, &records))
        .cloned()
        .or_else(|| {
            epoch_order
                .iter()
                .rev()
                .find(|record| {
                    retained.contains(&record.snapshot_id)
                        && has_complete_parent_chain(&record.snapshot_id, &records)
                })
                .map(|record| record.snapshot_id.clone())
        });
    if let Some(requested) = &request.restore_snapshot_id {
        if recovery_snapshot_id.as_deref() != Some(requested.as_str()) {
            omissions.push(format!(
                "requested restore point {requested} was not retained or did not have a complete parent chain"
            ));
        }
    }
    if recovery_snapshot_id.is_none() {
        omissions.push("no complete verified recovery point is available".into());
    }
    let integrity_status = if rejected_order.is_empty() {
        "verified"
    } else if recovery_snapshot_id.is_some() {
        "partial_with_rejections"
    } else {
        "blocked_by_integrity_failures"
    };
    let disposition = if recovery_snapshot_id.is_none() {
        SnapshotStoreDisposition::Blocked
    } else if !rejected_order.is_empty() || !omissions.is_empty() {
        SnapshotStoreDisposition::Partial
    } else {
        SnapshotStoreDisposition::Ready
    };
    let mut index = DecisionContextSnapshotIndex {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        records: retained_order
            .iter()
            .filter_map(|id| records.get(id).cloned())
            .collect(),
        retained_order,
        evicted_order,
        rejected_order,
        recovery_snapshot_id,
        integrity_status: integrity_status.into(),
        disposition,
        omissions,
        negative_evidence,
        digest: ContentHash::of_bytes(b"unsealed-glioma-context-snapshot-index"),
    };
    index.digest = ContentHash::of_value(&index_body(&index))
        .map_err(|error| DecisionContextSnapshotStoreError::Digest(error.to_string()))?;
    index.validate()?;
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(objective: &str, epoch: u32) -> DecisionContext {
        let mut context = DecisionContext {
            feature_id: "GAF-GLIOMA-P04-F01".into(),
            output_schema: "GliomaDecisionContext1@2".into(),
            objective: objective.into(),
            claim_order: Vec::new(),
            actions: Vec::new(),
            action_order: Vec::new(),
            deferred_action_order: Vec::new(),
            omission_order: vec![format!("omission-{epoch}")],
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: super::super::context_compiler::DecisionContextDisposition::Unresolved,
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let body = serde_json::json!({
            "feature_id": context.feature_id,
            "output_schema": context.output_schema,
            "objective": context.objective,
            "claim_order": context.claim_order,
            "actions": context.actions,
            "action_order": context.action_order,
            "deferred_action_order": context.deferred_action_order,
            "omission_order": context.omission_order,
            "negative_evidence_order": context.negative_evidence_order,
            "uncertainty_order": context.uncertainty_order,
            "disposition": context.disposition,
        });
        context.digest = ContentHash::of_value(&body).unwrap();
        context
    }

    fn snapshot(id: &str, epoch: u32, parent: Option<&str>) -> DecisionContextSnapshotInput {
        DecisionContextSnapshotInput {
            snapshot_id: id.into(),
            study_id: "study-a".into(),
            epoch,
            parent_snapshot_id: parent.map(str::to_string),
            context: context("map glioma invasion frontier", epoch),
            event_order: vec![format!("event-{epoch:03}")],
            pinned: false,
            referenced: false,
        }
    }

    fn request(
        snapshots: Vec<DecisionContextSnapshotInput>,
    ) -> DecisionContextSnapshotStoreRequest {
        DecisionContextSnapshotStoreRequest {
            study_id: "study-a".into(),
            max_retained_snapshots: 2,
            snapshots,
            restore_snapshot_id: None,
        }
    }

    #[test]
    fn retains_latest_with_complete_parent_chain() {
        let mut req = request(vec![
            snapshot("s1", 1, None),
            snapshot("s2", 2, Some("s1")),
            snapshot("s3", 3, Some("s2")),
        ]);
        req.max_retained_snapshots = 3;
        let index = store_glioma_decision_context_snapshots(&req).unwrap();
        assert_eq!(index.recovery_snapshot_id.as_deref(), Some("s3"));
        assert_eq!(index.retained_order, vec!["s1", "s2", "s3"]);
        assert!(index.evicted_order.is_empty());
        assert!(index.validate().is_ok());
    }

    #[test]
    fn pinned_ancestor_blocks_eviction_and_is_reported() {
        let mut first = snapshot("s1", 1, None);
        first.pinned = true;
        let mut req = request(vec![
            first,
            snapshot("s2", 2, Some("s1")),
            snapshot("s3", 3, Some("s2")),
        ]);
        req.max_retained_snapshots = 1;
        let index = store_glioma_decision_context_snapshots(&req).unwrap();
        assert_eq!(index.disposition, SnapshotStoreDisposition::Partial);
        assert!(index
            .omissions
            .iter()
            .any(|item| item.contains("retention budget")));
        assert!(index.retained_order.contains(&"s1".into()));
    }

    #[test]
    fn corrupt_context_is_rejected_without_poisoning_recovery() {
        let mut corrupt = snapshot("s2", 2, Some("s1"));
        corrupt.context.digest = ContentHash::of_bytes(b"corrupt");
        let index = store_glioma_decision_context_snapshots(&request(vec![
            snapshot("s1", 1, None),
            corrupt,
        ]))
        .unwrap();
        assert!(index.rejected_order.contains(&"s2".into()));
        assert_eq!(index.recovery_snapshot_id.as_deref(), Some("s1"));
        assert!(index
            .negative_evidence
            .iter()
            .any(|item| item.contains("s2")));
    }

    #[test]
    fn requested_evicted_restore_point_is_an_explicit_omission() {
        let mut req = request(vec![
            snapshot("s1", 1, None),
            snapshot("s2", 2, None),
            snapshot("s3", 3, Some("s2")),
        ]);
        req.restore_snapshot_id = Some("s1".into());
        let index = store_glioma_decision_context_snapshots(&req).unwrap();
        assert_eq!(index.recovery_snapshot_id.as_deref(), Some("s3"));
        assert!(index.omissions.iter().any(|item| item.contains("s1")));
    }
}
