//! Partition-resilient multi-study decision-context checkpoint reconciliation.
//!
//! Each institution keeps the context payload locally and contributes only typed field digests
//! plus signed epoch metadata. This reducer is deliberately conservative: retries are
//! idempotent, stale/future epochs are visible, conflicting values are never last-writer-wins,
//! and an active partition cannot be promoted as a converged checkpoint.

use super::context_compiler::DecisionContextDisposition;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaPartitionResilientContextCheckpoint1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_DELTAS: usize = 1_024;
pub const MAX_FIELDS_PER_DELTA: usize = 64;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointNetworkState {
    Connected,
    Partitioned,
    Reconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointConflictPolicy {
    PreserveConflicts,
    RequireConsensus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCheckpointField {
    Objective,
    ClaimOrder,
    ActionOrder,
    DeferredActionOrder,
    OmissionOrder,
    NegativeEvidenceOrder,
    UncertaintyOrder,
    Disposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ContextFieldValue {
    Text(String),
    Items(Vec<String>),
    Disposition(DecisionContextDisposition),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFieldDelta {
    pub field: ContextCheckpointField,
    pub value: ContextFieldValue,
    pub value_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCheckpointDelta {
    pub site_id: String,
    pub study_id: String,
    pub epoch: u32,
    pub parent_checkpoint_digest: ContentHash,
    pub observed_tick: u64,
    pub signer_digest: ContentHash,
    pub fields: Vec<ContextFieldDelta>,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub delta_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionResilientContextCheckpointRequest {
    pub objective: String,
    pub study_id: String,
    pub expected_epoch: u32,
    pub checkpoint_anchor_digest: ContentHash,
    pub minimum_sites: usize,
    pub max_sites: usize,
    pub max_fields_per_delta: usize,
    pub current_tick: u64,
    pub max_staleness_ticks: u64,
    pub network_state: CheckpointNetworkState,
    pub conflict_policy: CheckpointConflictPolicy,
    pub deltas: Vec<ContextCheckpointDelta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointSiteDisposition {
    Accepted,
    DuplicateRetry,
    Stale,
    Partitioned,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointSiteStatus {
    pub site_id: String,
    pub epoch: u32,
    pub delta_digest: ContentHash,
    pub field_order: Vec<ContextCheckpointField>,
    pub disposition: CheckpointSiteDisposition,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointFieldConflict {
    pub field: ContextCheckpointField,
    pub site_order: Vec<String>,
    pub value_digest_order: Vec<ContentHash>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCheckpointDisposition {
    Converged,
    Partial,
    Partitioned,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionResilientContextCheckpoint {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub expected_epoch: u32,
    pub network_state: CheckpointNetworkState,
    pub site_order: Vec<String>,
    pub accepted_site_order: Vec<String>,
    pub rejected_site_order: Vec<String>,
    pub acknowledged_delta_order: Vec<String>,
    pub site_statuses: Vec<CheckpointSiteStatus>,
    pub converged_field_order: Vec<ContextCheckpointField>,
    pub conflicts: Vec<CheckpointFieldConflict>,
    pub non_converged_field_order: Vec<ContextCheckpointField>,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ContextCheckpointDisposition,
    pub checkpoint_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PartitionCheckpointError {
    #[error("partition checkpoint request is invalid: {0}")]
    InvalidRequest(String),
    #[error("partition checkpoint output is invalid: {0}")]
    InvalidOutput(String),
    #[error("partition checkpoint digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn field_body(field: ContextCheckpointField, value: &ContextFieldValue) -> serde_json::Value {
    serde_json::json!({"field": field, "value": value})
}

fn delta_body(delta: &ContextCheckpointDelta) -> serde_json::Value {
    serde_json::json!({
        "site_id": delta.site_id,
        "study_id": delta.study_id,
        "epoch": delta.epoch,
        "parent_checkpoint_digest": delta.parent_checkpoint_digest,
        "observed_tick": delta.observed_tick,
        "signer_digest": delta.signer_digest,
        "fields": delta.fields,
        "local_only": delta.local_only,
        "contains_human_data": delta.contains_human_data,
        "contains_direct_identifiers": delta.contains_direct_identifiers,
    })
}

fn output_body(output: &PartitionResilientContextCheckpoint) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "study_id": output.study_id,
        "expected_epoch": output.expected_epoch,
        "network_state": output.network_state,
        "site_order": output.site_order,
        "accepted_site_order": output.accepted_site_order,
        "rejected_site_order": output.rejected_site_order,
        "acknowledged_delta_order": output.acknowledged_delta_order,
        "site_statuses": output.site_statuses,
        "converged_field_order": output.converged_field_order,
        "conflicts": output.conflicts,
        "non_converged_field_order": output.non_converged_field_order,
        "omissions": output.omissions,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
    })
}

fn validate_field(field: &ContextFieldDelta) -> Result<(), PartitionCheckpointError> {
    let valid_shape = match &field.value {
        ContextFieldValue::Text(value) => safe_text(value),
        ContextFieldValue::Items(values) => {
            !values.is_empty()
                && values.len() <= MAX_TEXT_LEN
                && values.iter().all(|value| safe_text(value))
                && canonical(values)
        }
        ContextFieldValue::Disposition(_) => true,
    };
    if !valid_shape || !valid_hash(&field.value_digest) {
        return Err(PartitionCheckpointError::InvalidRequest(
            "field value must be bounded, canonical, and content-addressed".into(),
        ));
    }
    let expected = ContentHash::of_value(&field_body(field.field, &field.value))
        .map_err(|error| PartitionCheckpointError::Digest(error.to_string()))?;
    if expected != field.value_digest {
        return Err(PartitionCheckpointError::InvalidRequest(
            "field value digest does not match its typed value".into(),
        ));
    }
    Ok(())
}

fn validate_delta(
    delta: &ContextCheckpointDelta,
    request: &PartitionResilientContextCheckpointRequest,
) -> Result<(), PartitionCheckpointError> {
    if !safe_text(&delta.site_id)
        || delta.study_id != request.study_id
        || delta.epoch == 0
        || delta.observed_tick == 0
        || !valid_hash(&delta.parent_checkpoint_digest)
        || !valid_hash(&delta.signer_digest)
        || delta.fields.is_empty()
        || delta.fields.len() > request.max_fields_per_delta
        || delta.fields.len() > MAX_FIELDS_PER_DELTA
        || !delta.local_only
        || delta.contains_human_data
        || delta.contains_direct_identifiers
        || !valid_hash(&delta.delta_digest)
        || delta
            .fields
            .windows(2)
            .any(|pair| pair[0].field >= pair[1].field)
    {
        return Err(PartitionCheckpointError::InvalidRequest(format!(
            "site {} delta is stale, protected, non-local, out of epoch, unordered, or malformed",
            delta.site_id
        )));
    }
    for field in &delta.fields {
        validate_field(field)?;
    }
    let expected = ContentHash::of_value(&delta_body(delta))
        .map_err(|error| PartitionCheckpointError::Digest(error.to_string()))?;
    if expected != delta.delta_digest {
        return Err(PartitionCheckpointError::InvalidRequest(format!(
            "site {} delta digest does not match its signed body",
            delta.site_id
        )));
    }
    Ok(())
}

fn validate_request(
    request: &PartitionResilientContextCheckpointRequest,
) -> Result<(), PartitionCheckpointError> {
    if !safe_text(&request.objective)
        || !safe_text(&request.study_id)
        || request.expected_epoch == 0
        || !valid_hash(&request.checkpoint_anchor_digest)
        || request.minimum_sites == 0
        || request.max_sites < request.minimum_sites
        || request.max_sites > MAX_SITES
        || request.max_fields_per_delta == 0
        || request.max_fields_per_delta > MAX_FIELDS_PER_DELTA
        || request.current_tick == 0
        || request.deltas.is_empty()
        || request.deltas.len() > MAX_DELTAS
        || request.deltas.len() > request.max_sites.saturating_mul(4)
    {
        return Err(PartitionCheckpointError::InvalidRequest(
            "objective, study, anchor, epoch, bounded site/field limits, tick, and deltas are required".into(),
        ));
    }
    for delta in &request.deltas {
        validate_delta(delta, request)?;
    }
    Ok(())
}

impl PartitionResilientContextCheckpoint {
    pub fn validate(&self) -> Result<(), PartitionCheckpointError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !safe_text(&self.study_id)
            || self.expected_epoch == 0
            || !canonical(&self.site_order)
            || !canonical(&self.accepted_site_order)
            || !canonical(&self.rejected_site_order)
            || !canonical(&self.acknowledged_delta_order)
            || !canonical(&self.converged_field_order)
            || !canonical(&self.non_converged_field_order)
            || self.site_statuses.iter().any(|status| {
                !safe_text(&status.site_id)
                    || !safe_text(&status.reason)
                    || !valid_hash(&status.delta_digest)
            })
            || self.conflicts.iter().any(|conflict| {
                !canonical(&conflict.site_order)
                    || !canonical(&conflict.value_digest_order)
                    || conflict.site_order.is_empty()
                    || conflict.value_digest_order.len() < 2
                    || !safe_text(&conflict.reason)
            })
            || !self.recovery_field_partition_is_disjoint()
            || self.checkpoint_digest.as_str().len() != 64
        {
            return Err(PartitionCheckpointError::InvalidOutput(
                "checkpoint identity, ordering, field partitions, conflict, site, or digest invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| PartitionCheckpointError::Digest(error.to_string()))?;
        if expected != self.checkpoint_digest {
            return Err(PartitionCheckpointError::InvalidOutput(
                "checkpoint digest is not content-bound".into(),
            ));
        }
        Ok(())
    }

    fn recovery_field_partition_is_disjoint(&self) -> bool {
        self.converged_field_order
            .iter()
            .all(|field| !self.non_converged_field_order.contains(field))
            && self
                .conflicts
                .iter()
                .all(|conflict| self.non_converged_field_order.contains(&conflict.field))
    }
}

/// Reconcile site-local context deltas without last-writer-wins or raw-payload movement.
pub fn reconcile_partition_resilient_context_checkpoint(
    request: &PartitionResilientContextCheckpointRequest,
) -> Result<PartitionResilientContextCheckpoint, PartitionCheckpointError> {
    validate_request(request)?;
    let mut statuses = Vec::new();
    let mut accepted = BTreeMap::<String, &ContextCheckpointDelta>::new();
    let mut acknowledged = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut omissions = Vec::new();
    let mut negative = Vec::new();
    for delta in &request.deltas {
        if delta.parent_checkpoint_digest != request.checkpoint_anchor_digest {
            rejected.insert(delta.site_id.clone());
            negative.push(format!(
                "site {} delta is anchored to a different checkpoint; payload was not merged",
                delta.site_id
            ));
            statuses.push(CheckpointSiteStatus {
                site_id: delta.site_id.clone(),
                epoch: delta.epoch,
                delta_digest: delta.delta_digest.clone(),
                field_order: delta.fields.iter().map(|field| field.field).collect(),
                disposition: CheckpointSiteDisposition::Rejected,
                reason: "checkpoint anchor mismatch".into(),
            });
            continue;
        }
        if delta.epoch != request.expected_epoch {
            rejected.insert(delta.site_id.clone());
            let disposition = if delta.epoch < request.expected_epoch {
                CheckpointSiteDisposition::Stale
            } else {
                CheckpointSiteDisposition::Rejected
            };
            let reason = if disposition == CheckpointSiteDisposition::Stale {
                "delta epoch is older than the requested checkpoint epoch"
            } else {
                "future delta epoch cannot be merged into this checkpoint"
            };
            negative.push(format!("site {}: {reason}", delta.site_id));
            statuses.push(CheckpointSiteStatus {
                site_id: delta.site_id.clone(),
                epoch: delta.epoch,
                delta_digest: delta.delta_digest.clone(),
                field_order: delta.fields.iter().map(|field| field.field).collect(),
                disposition,
                reason: reason.into(),
            });
            continue;
        }
        if delta.observed_tick > request.current_tick
            || request.current_tick.saturating_sub(delta.observed_tick)
                > request.max_staleness_ticks
        {
            rejected.insert(delta.site_id.clone());
            negative.push(format!(
                "site {} delta is outside the freshness window and was not merged",
                delta.site_id
            ));
            statuses.push(CheckpointSiteStatus {
                site_id: delta.site_id.clone(),
                epoch: delta.epoch,
                delta_digest: delta.delta_digest.clone(),
                field_order: delta.fields.iter().map(|field| field.field).collect(),
                disposition: CheckpointSiteDisposition::Stale,
                reason: "delta observed tick is outside the freshness window".into(),
            });
            continue;
        }
        if let Some(previous) = accepted.get(&delta.site_id) {
            if previous.delta_digest == delta.delta_digest {
                acknowledged.insert(delta.delta_digest.as_str().to_string());
                statuses.push(CheckpointSiteStatus {
                    site_id: delta.site_id.clone(),
                    epoch: delta.epoch,
                    delta_digest: delta.delta_digest.clone(),
                    field_order: delta.fields.iter().map(|field| field.field).collect(),
                    disposition: CheckpointSiteDisposition::DuplicateRetry,
                    reason: "duplicate retry acknowledged idempotently".into(),
                });
                continue;
            }
            rejected.insert(delta.site_id.clone());
            negative.push(format!(
                "site {} submitted conflicting delta digests for one epoch; neither was selected",
                delta.site_id
            ));
            statuses.push(CheckpointSiteStatus {
                site_id: delta.site_id.clone(),
                epoch: delta.epoch,
                delta_digest: delta.delta_digest.clone(),
                field_order: delta.fields.iter().map(|field| field.field).collect(),
                disposition: CheckpointSiteDisposition::Rejected,
                reason: "conflicting same-site retry is not last-writer-wins".into(),
            });
            continue;
        }
        accepted.insert(delta.site_id.clone(), delta);
        let disposition = if request.network_state == CheckpointNetworkState::Partitioned {
            CheckpointSiteDisposition::Partitioned
        } else {
            CheckpointSiteDisposition::Accepted
        };
        if disposition == CheckpointSiteDisposition::Partitioned {
            omissions.push(format!(
                "site {} is acknowledged locally but cannot promote while the federation is partitioned",
                delta.site_id
            ));
        }
        statuses.push(CheckpointSiteStatus {
            site_id: delta.site_id.clone(),
            epoch: delta.epoch,
            delta_digest: delta.delta_digest.clone(),
            field_order: delta.fields.iter().map(|field| field.field).collect(),
            disposition,
            reason: if disposition == CheckpointSiteDisposition::Partitioned {
                "delta retained locally; network partition prevents promotion".into()
            } else {
                "signed current-epoch delta accepted".into()
            },
        });
        acknowledged.insert(delta.delta_digest.as_str().to_string());
    }
    let mut field_values =
        BTreeMap::<ContextCheckpointField, BTreeMap<String, BTreeSet<String>>>::new();
    for (site_id, delta) in &accepted {
        for field in &delta.fields {
            field_values
                .entry(field.field)
                .or_default()
                .entry(field.value_digest.as_str().to_string())
                .or_default()
                .insert(site_id.clone());
        }
    }
    let mut converged = Vec::new();
    let mut conflicts = Vec::new();
    let mut non_converged = Vec::new();
    for (field, values) in field_values {
        let value_digest_order = values.keys().cloned().collect::<Vec<_>>();
        if value_digest_order.len() == 1 {
            converged.push(field);
        } else {
            let site_order = values
                .values()
                .flat_map(|sites| sites.iter().cloned())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            non_converged.push(field);
            conflicts.push(CheckpointFieldConflict {
                field,
                site_order,
                value_digest_order: value_digest_order
                    .into_iter()
                    .map(|value| {
                        ContentHash::parse(value).expect("field value digest is validated")
                    })
                    .collect(),
                reason: "field values disagree; no site value was overwritten".into(),
            });
        }
    }
    converged.sort();
    non_converged.sort();
    conflicts.sort_by_key(|conflict| conflict.field);
    let site_order = accepted
        .keys()
        .chain(rejected.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let accepted_site_order = accepted.keys().cloned().collect::<Vec<_>>();
    let rejected_site_order = rejected.into_iter().collect::<Vec<_>>();
    statuses.sort_by(|left, right| {
        left.site_id
            .cmp(&right.site_id)
            .then(left.delta_digest.as_str().cmp(right.delta_digest.as_str()))
    });
    let mut acknowledged_delta_order = acknowledged.into_iter().collect::<Vec<_>>();
    acknowledged_delta_order.sort();
    if accepted_site_order.len() < request.minimum_sites {
        omissions.push(format!(
            "only {} sites were accepted; minimum {} is required",
            accepted_site_order.len(),
            request.minimum_sites
        ));
    }
    if accepted.is_empty() {
        negative.push("no site-local delta reached reconciliation".into());
    }
    if !conflicts.is_empty() {
        negative.push(format!(
            "{} context fields remain non-converged and were not overwritten",
            conflicts.len()
        ));
    }
    omissions.sort();
    omissions.dedup();
    negative.sort();
    negative.dedup();
    let enough_sites = accepted_site_order.len() >= request.minimum_sites;
    let disposition = if !enough_sites || accepted.is_empty() {
        ContextCheckpointDisposition::Blocked
    } else if request.network_state == CheckpointNetworkState::Partitioned {
        ContextCheckpointDisposition::Partitioned
    } else if request.conflict_policy == CheckpointConflictPolicy::RequireConsensus
        && !conflicts.is_empty()
    {
        ContextCheckpointDisposition::Blocked
    } else if !conflicts.is_empty() || !rejected_site_order.is_empty() {
        ContextCheckpointDisposition::Partial
    } else {
        ContextCheckpointDisposition::Converged
    };
    let mut output = PartitionResilientContextCheckpoint {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        expected_epoch: request.expected_epoch,
        network_state: request.network_state,
        site_order,
        accepted_site_order,
        rejected_site_order,
        acknowledged_delta_order,
        site_statuses: statuses,
        converged_field_order: converged,
        conflicts,
        non_converged_field_order: non_converged,
        omissions,
        negative_evidence: negative,
        disposition,
        checkpoint_digest: ContentHash::of_bytes(b"unsealed-glioma-partition-checkpoint"),
    };
    output.checkpoint_digest = ContentHash::of_value(&output_body(&output))
        .map_err(|error| PartitionCheckpointError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &serde_json::Value) -> ContentHash {
        ContentHash::of_value(value).unwrap()
    }

    fn field(field: ContextCheckpointField, value: ContextFieldValue) -> ContextFieldDelta {
        ContextFieldDelta {
            field,
            value_digest: hash(&field_body(field, &value)),
            value,
        }
    }

    fn delta(site_id: &str, fields: Vec<ContextFieldDelta>) -> ContextCheckpointDelta {
        let mut output = ContextCheckpointDelta {
            site_id: site_id.into(),
            study_id: "study-a".into(),
            epoch: 4,
            parent_checkpoint_digest: ContentHash::of_bytes(b"anchor"),
            observed_tick: 20,
            signer_digest: ContentHash::of_bytes(format!("signer-{site_id}").as_bytes()),
            fields,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            delta_digest: ContentHash::of_bytes(b"unsealed"),
        };
        output.delta_digest = hash(&delta_body(&output));
        output
    }

    fn request(deltas: Vec<ContextCheckpointDelta>) -> PartitionResilientContextCheckpointRequest {
        PartitionResilientContextCheckpointRequest {
            objective: "reconcile glioma invasion context".into(),
            study_id: "study-a".into(),
            expected_epoch: 4,
            checkpoint_anchor_digest: ContentHash::of_bytes(b"anchor"),
            minimum_sites: 2,
            max_sites: 4,
            max_fields_per_delta: 8,
            current_tick: 20,
            max_staleness_ticks: 5,
            network_state: CheckpointNetworkState::Reconnected,
            conflict_policy: CheckpointConflictPolicy::PreserveConflicts,
            deltas,
        }
    }

    #[test]
    fn converges_only_when_current_site_fields_agree() {
        let fields = vec![field(
            ContextCheckpointField::OmissionOrder,
            ContextFieldValue::Items(vec!["missing-imaging".into()]),
        )];
        let output = reconcile_partition_resilient_context_checkpoint(&request(vec![
            delta("site-a", fields.clone()),
            delta("site-b", fields),
        ]))
        .unwrap();
        assert_eq!(output.disposition, ContextCheckpointDisposition::Converged);
        assert_eq!(
            output.converged_field_order,
            vec![ContextCheckpointField::OmissionOrder]
        );
        assert!(output.conflicts.is_empty());
        assert!(output.validate().is_ok());
    }

    #[test]
    fn conflicting_fields_are_preserved_not_last_writer_wins() {
        let mut req = request(vec![
            delta(
                "site-a",
                vec![field(
                    ContextCheckpointField::Disposition,
                    ContextFieldValue::Disposition(DecisionContextDisposition::Qualified),
                )],
            ),
            delta(
                "site-b",
                vec![field(
                    ContextCheckpointField::Disposition,
                    ContextFieldValue::Disposition(DecisionContextDisposition::Unresolved),
                )],
            ),
        ]);
        req.minimum_sites = 1;
        let output = reconcile_partition_resilient_context_checkpoint(&req).unwrap();
        assert_eq!(output.disposition, ContextCheckpointDisposition::Partial);
        assert_eq!(
            output.conflicts[0].field,
            ContextCheckpointField::Disposition
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("not overwritten")));
    }

    #[test]
    fn partition_blocks_promotion_even_with_matching_deltas() {
        let mut req = request(vec![
            delta(
                "site-a",
                vec![field(
                    ContextCheckpointField::Objective,
                    ContextFieldValue::Text("same".into()),
                )],
            ),
            delta(
                "site-b",
                vec![field(
                    ContextCheckpointField::Objective,
                    ContextFieldValue::Text("same".into()),
                )],
            ),
        ]);
        req.network_state = CheckpointNetworkState::Partitioned;
        let output = reconcile_partition_resilient_context_checkpoint(&req).unwrap();
        assert_eq!(
            output.disposition,
            ContextCheckpointDisposition::Partitioned
        );
        assert!(output
            .omissions
            .iter()
            .any(|item| item.contains("partition")));
    }

    #[test]
    fn identical_retry_is_acknowledged_and_conflicting_retry_is_rejected() {
        let first = delta(
            "site-a",
            vec![field(
                ContextCheckpointField::Objective,
                ContextFieldValue::Text("same".into()),
            )],
        );
        let mut conflicting = first.clone();
        conflicting.fields[0] = field(
            ContextCheckpointField::Objective,
            ContextFieldValue::Text("different".into()),
        );
        conflicting.delta_digest = hash(&delta_body(&conflicting));
        let mut req = request(vec![
            first.clone(),
            first,
            conflicting,
            delta(
                "site-b",
                vec![field(
                    ContextCheckpointField::Objective,
                    ContextFieldValue::Text("same".into()),
                )],
            ),
        ]);
        req.minimum_sites = 1;
        let output = reconcile_partition_resilient_context_checkpoint(&req).unwrap();
        assert!(output
            .site_statuses
            .iter()
            .any(|status| status.disposition == CheckpointSiteDisposition::DuplicateRetry));
        assert!(output
            .site_statuses
            .iter()
            .any(|status| status.disposition == CheckpointSiteDisposition::Rejected));
    }

    #[test]
    fn protected_or_unanchored_delta_is_refused() {
        let mut protected = delta(
            "site-a",
            vec![field(
                ContextCheckpointField::Objective,
                ContextFieldValue::Text("same".into()),
            )],
        );
        protected.contains_human_data = true;
        protected.delta_digest = hash(&delta_body(&protected));
        assert!(
            reconcile_partition_resilient_context_checkpoint(&request(vec![
                protected,
                delta(
                    "site-b",
                    vec![field(
                        ContextCheckpointField::Objective,
                        ContextFieldValue::Text("same".into())
                    )]
                )
            ]))
            .is_err()
        );
    }
}
