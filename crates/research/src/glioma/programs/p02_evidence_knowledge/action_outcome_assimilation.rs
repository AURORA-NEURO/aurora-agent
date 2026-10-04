//! Idempotent assimilation of P02 knowledge-action outcomes across retries and restarts.
//!
//! Dispatchers can retry an action, return a duplicate callback, or observe a result that
//! conflicts with an earlier attempt.  This feature reconciles those value-only envelopes and
//! evidence records without silently choosing a scientific winner.  Conflicts remain explicit and
//! route back to the knowledge frontier for adjudication.

use crate::glioma::evidence::EvidenceRecord;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

use super::dispatch::KnowledgeActionResultDisposition;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeActionOutcomeAssimilation1@1";
pub const MAX_OUTCOMES: usize = 4_096;
pub const MAX_RECORDS: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeAssimilationDecision {
    AcceptedIncoming,
    RetainedPrior,
    Duplicate,
    Conflict,
    FailedRetained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeAssimilationDisposition {
    Advanced,
    Idempotent,
    Conflict,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionOutcomeSnapshot {
    pub action_id: String,
    pub claim_id: String,
    pub attempt: u8,
    pub cost_units: u64,
    pub evidence_order: Vec<String>,
    pub disposition: KnowledgeActionResultDisposition,
    pub failure: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionOutcomeAssimilationRequest {
    pub objective: String,
    pub plan_digest: ContentHash,
    pub selection_digest: ContentHash,
    pub prior_outcomes: Vec<ActionOutcomeSnapshot>,
    pub incoming_outcomes: Vec<ActionOutcomeSnapshot>,
    pub prior_records: Vec<EvidenceRecord>,
    pub incoming_records: Vec<EvidenceRecord>,
    pub max_outcomes: usize,
    pub max_records: usize,
    pub retain_prior_on_incoming_failure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionOutcomeAssimilationItem {
    pub action_id: String,
    pub claim_id: String,
    pub decision: OutcomeAssimilationDecision,
    pub selected_attempt: u8,
    pub selected_evidence_order: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionOutcomeAssimilation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub plan_digest: ContentHash,
    pub selection_digest: ContentHash,
    pub action_order: Vec<String>,
    pub items: Vec<ActionOutcomeAssimilationItem>,
    pub records: Vec<EvidenceRecord>,
    pub accepted_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub duplicate_order: Vec<String>,
    pub conflict_order: Vec<String>,
    pub conflict_evidence_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: OutcomeAssimilationDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionOutcomeAssimilationError {
    #[error("action outcome assimilation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("action outcome assimilation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("action outcome assimilation digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &KnowledgeActionOutcomeAssimilation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "plan_digest": output.plan_digest,
        "selection_digest": output.selection_digest,
        "action_order": output.action_order,
        "items": output.items,
        "records": output.records,
        "accepted_order": output.accepted_order,
        "retained_order": output.retained_order,
        "duplicate_order": output.duplicate_order,
        "conflict_order": output.conflict_order,
        "conflict_evidence_order": output.conflict_evidence_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl KnowledgeActionOutcomeAssimilation {
    pub fn validate(&self) -> Result<(), ActionOutcomeAssimilationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.plan_digest.as_str().len() != 64
            || self.selection_digest.as_str().len() != 64
            || !canonical(&self.action_order)
            || !canonical(&self.accepted_order)
            || !canonical(&self.retained_order)
            || !canonical(&self.duplicate_order)
            || !canonical(&self.conflict_order)
            || !canonical(&self.conflict_evidence_order)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .items
                    .iter()
                    .map(|item| item.action_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(
                &self
                    .records
                    .iter()
                    .map(|record| record.evidence_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.items.iter().any(|item| {
                item.action_id.trim().is_empty()
                    || item.claim_id.trim().is_empty()
                    || item.selected_attempt == 0
                    || !canonical(&item.selected_evidence_order)
                    || item.rationale.trim().is_empty()
            })
            || self.digest.as_str().len() != 64
        {
            return Err(ActionOutcomeAssimilationError::InvalidOutput(
                "identity, canonical partitions, item contracts, or digest is invalid".into(),
            ));
        }
        let action_ids = self
            .items
            .iter()
            .map(|item| item.action_id.clone())
            .collect::<BTreeSet<_>>();
        let action_order = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let classified = self
            .accepted_order
            .iter()
            .chain(self.retained_order.iter())
            .chain(self.duplicate_order.iter())
            .chain(self.conflict_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let record_ids = self
            .records
            .iter()
            .map(|record| record.evidence_id.clone())
            .collect::<BTreeSet<_>>();
        if action_ids != action_order
            || action_order.len() != self.action_order.len()
            || classified != action_ids
            || classified.len()
                != self.accepted_order.len()
                    + self.retained_order.len()
                    + self.duplicate_order.len()
                    + self.conflict_order.len()
            || record_ids.len() != self.records.len()
            || self
                .conflict_evidence_order
                .iter()
                .any(|id| record_ids.contains(id))
        {
            return Err(ActionOutcomeAssimilationError::InvalidOutput(
                "action/evidence identity or assimilation partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ActionOutcomeAssimilationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ActionOutcomeAssimilationError::Digest(
                "assimilation digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn outcome_equal(left: &ActionOutcomeSnapshot, right: &ActionOutcomeSnapshot) -> bool {
    left.claim_id == right.claim_id
        && left.attempt == right.attempt
        && left.cost_units == right.cost_units
        && left.evidence_order == right.evidence_order
        && left.disposition == right.disposition
        && left.failure == right.failure
}

fn evidence_equal(left: &EvidenceRecord, right: &EvidenceRecord) -> bool {
    left == right
}

/// Reconcile prior and incoming action results. Incoming results advance only when their action
/// identity and evidence are unambiguous; duplicates are idempotent and conflicts remain held.
pub fn assimilate_glioma_knowledge_action_outcomes(
    request: &KnowledgeActionOutcomeAssimilationRequest,
) -> Result<KnowledgeActionOutcomeAssimilation, ActionOutcomeAssimilationError> {
    if request.objective.trim().is_empty()
        || request.plan_digest.as_str().len() != 64
        || request.selection_digest.as_str().len() != 64
        || request.max_outcomes == 0
        || request.max_outcomes > MAX_OUTCOMES
        || request.max_records == 0
        || request.max_records > MAX_RECORDS
        || request.prior_outcomes.len() > request.max_outcomes
        || request.incoming_outcomes.len() > request.max_outcomes
        || request.prior_records.len() > request.max_records
        || request.incoming_records.len() > request.max_records
    {
        return Err(ActionOutcomeAssimilationError::InvalidRequest(
            "objective, digests, bounds, or input sizes are invalid".into(),
        ));
    }
    let mut prior = BTreeMap::new();
    let mut incoming = BTreeMap::new();
    for outcome in &request.prior_outcomes {
        if outcome.action_id.trim().is_empty()
            || outcome.claim_id.trim().is_empty()
            || outcome.attempt == 0
            || !canonical(&outcome.evidence_order)
            || prior.insert(outcome.action_id.clone(), outcome).is_some()
        {
            return Err(ActionOutcomeAssimilationError::InvalidRequest(
                "prior outcomes must have unique ids, attempts, and canonical evidence order"
                    .into(),
            ));
        }
    }
    for outcome in &request.incoming_outcomes {
        if outcome.action_id.trim().is_empty()
            || outcome.claim_id.trim().is_empty()
            || outcome.attempt == 0
            || !canonical(&outcome.evidence_order)
            || incoming
                .insert(outcome.action_id.clone(), outcome)
                .is_some()
        {
            return Err(ActionOutcomeAssimilationError::InvalidRequest(
                "incoming outcomes must have unique ids, attempts, and canonical evidence order"
                    .into(),
            ));
        }
    }
    let action_ids = prior
        .keys()
        .chain(incoming.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut items = Vec::new();
    let mut accepted = BTreeSet::new();
    let mut retained = BTreeSet::new();
    let mut duplicate = BTreeSet::new();
    let mut conflicts = BTreeSet::new();
    let mut conflict_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for action_id in &action_ids {
        let item = match (prior.get(action_id), incoming.get(action_id)) {
            (None, Some(next)) => {
                accepted.insert(action_id.clone());
                (
                    next.claim_id.clone(),
                    OutcomeAssimilationDecision::AcceptedIncoming,
                    next,
                )
            }
            (Some(old), None) => {
                retained.insert(action_id.clone());
                (
                    old.claim_id.clone(),
                    OutcomeAssimilationDecision::RetainedPrior,
                    old,
                )
            }
            (Some(old), Some(next)) if outcome_equal(old, next) => {
                duplicate.insert(action_id.clone());
                (
                    next.claim_id.clone(),
                    OutcomeAssimilationDecision::Duplicate,
                    next,
                )
            }
            (Some(old), Some(next))
                if request.retain_prior_on_incoming_failure
                    && next.disposition == KnowledgeActionResultDisposition::Failed
                    && old.disposition != KnowledgeActionResultDisposition::Failed =>
            {
                retained.insert(action_id.clone());
                uncertainty.insert(format!(
                    "{action_id}: incoming failure retained prior result"
                ));
                (
                    old.claim_id.clone(),
                    OutcomeAssimilationDecision::FailedRetained,
                    old,
                )
            }
            (Some(old), Some(next)) => {
                conflicts.insert(action_id.clone());
                next.evidence_order
                    .iter()
                    .filter(|evidence_id| !old.evidence_order.contains(evidence_id))
                    .for_each(|evidence_id| {
                        conflict_evidence.insert(evidence_id.clone());
                    });
                uncertainty.insert(format!("{action_id}: prior and incoming outcomes disagree"));
                (
                    next.claim_id.clone(),
                    OutcomeAssimilationDecision::Conflict,
                    next,
                )
            }
            (None, None) => unreachable!(),
        };
        let (claim_id, decision, selected) = item;
        items.push(ActionOutcomeAssimilationItem {
            action_id: action_id.clone(),
            claim_id,
            decision,
            selected_attempt: selected.attempt,
            selected_evidence_order: selected.evidence_order.clone(),
            rationale: match decision {
                OutcomeAssimilationDecision::AcceptedIncoming => {
                    "incoming result advances the action state".into()
                }
                OutcomeAssimilationDecision::RetainedPrior => {
                    "no incoming result exists; prior state is retained".into()
                }
                OutcomeAssimilationDecision::Duplicate => {
                    "incoming result is byte-identical to prior state".into()
                }
                OutcomeAssimilationDecision::Conflict => {
                    "outcomes disagree and require frontier adjudication".into()
                }
                OutcomeAssimilationDecision::FailedRetained => {
                    "incoming failure cannot erase a prior non-failed result".into()
                }
            },
        });
    }
    items.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let mut record_map = BTreeMap::<String, EvidenceRecord>::new();
    for record in request
        .prior_records
        .iter()
        .chain(request.incoming_records.iter())
    {
        record
            .source_artifact
            .validate()
            .map_err(|error| ActionOutcomeAssimilationError::InvalidRequest(error.to_string()))?;
        if let Some(existing) = record_map.get(&record.evidence_id) {
            if !evidence_equal(existing, record) {
                conflict_evidence.insert(record.evidence_id.clone());
                record_map.remove(&record.evidence_id);
            }
        } else if !conflict_evidence.contains(&record.evidence_id) {
            record_map.insert(record.evidence_id.clone(), record.clone());
        }
    }
    let records = record_map.into_values().collect::<Vec<_>>();
    let action_order = items
        .iter()
        .map(|item| item.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = if !conflicts.is_empty() || !conflict_evidence.is_empty() {
        OutcomeAssimilationDisposition::Conflict
    } else if accepted.is_empty() && retained.is_empty() && !duplicate.is_empty() {
        OutcomeAssimilationDisposition::Idempotent
    } else if accepted.is_empty() && retained.is_empty() {
        OutcomeAssimilationDisposition::Blocked
    } else {
        OutcomeAssimilationDisposition::Advanced
    };
    let next_step = match disposition {
        OutcomeAssimilationDisposition::Advanced => "recompile typed knowledge and the claim frontier from the reconciled evidence".into(),
        OutcomeAssimilationDisposition::Idempotent => "reuse the existing knowledge state; no new evidence was observed".into(),
        OutcomeAssimilationDisposition::Conflict => "route conflicting action/evidence identities to explicit adjudication before compilation".into(),
        OutcomeAssimilationDisposition::Blocked => "obtain a valid action outcome before advancing the knowledge frontier".into(),
    };
    let mut output = KnowledgeActionOutcomeAssimilation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        plan_digest: request.plan_digest.clone(),
        selection_digest: request.selection_digest.clone(),
        action_order,
        items,
        records,
        accepted_order: accepted.into_iter().collect(),
        retained_order: retained.into_iter().collect(),
        duplicate_order: duplicate.into_iter().collect(),
        conflict_order: conflicts.into_iter().collect(),
        conflict_evidence_order: conflict_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| ActionOutcomeAssimilationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ActionOutcomeAssimilationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
    use crate::glioma_engine::LocalArtifactRef;

    fn outcome(
        action: &str,
        disposition: KnowledgeActionResultDisposition,
        evidence: &str,
    ) -> ActionOutcomeSnapshot {
        ActionOutcomeSnapshot {
            action_id: action.into(),
            claim_id: format!("claim-{action}"),
            attempt: 1,
            cost_units: 1,
            evidence_order: vec![evidence.into()],
            disposition,
            failure: None,
        }
    }

    fn record(id: &str) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Computation,
            claim: "egfr resistance".into(),
            scope: "organoid".into(),
            modality: crate::glioma_engine::GliomaModality::Computational,
            model_system: Some(crate::glioma_engine::GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 800,
            quality_milli: 800,
            reproducibility_milli: 800,
            release_epoch: 1,
        }
    }

    fn request(
        prior: Vec<ActionOutcomeSnapshot>,
        incoming: Vec<ActionOutcomeSnapshot>,
    ) -> KnowledgeActionOutcomeAssimilationRequest {
        KnowledgeActionOutcomeAssimilationRequest {
            objective: "reconcile knowledge actions".into(),
            plan_digest: ContentHash::of_bytes(b"plan"),
            selection_digest: ContentHash::of_bytes(b"selection"),
            prior_outcomes: prior,
            incoming_outcomes: incoming,
            prior_records: vec![record("evidence-a")],
            incoming_records: vec![record("evidence-a")],
            max_outcomes: 10,
            max_records: 10,
            retain_prior_on_incoming_failure: true,
        }
    }

    #[test]
    fn identical_retry_is_idempotent() {
        let result = assimilate_glioma_knowledge_action_outcomes(&request(
            vec![outcome(
                "action-a",
                KnowledgeActionResultDisposition::Completed,
                "evidence-a",
            )],
            vec![outcome(
                "action-a",
                KnowledgeActionResultDisposition::Completed,
                "evidence-a",
            )],
        ))
        .unwrap();
        assert_eq!(
            result.disposition,
            OutcomeAssimilationDisposition::Idempotent
        );
        assert_eq!(result.duplicate_order, vec!["action-a"]);
        result.validate().unwrap();
    }

    #[test]
    fn incoming_result_advances_the_frontier() {
        let result = assimilate_glioma_knowledge_action_outcomes(&request(
            vec![],
            vec![outcome(
                "action-a",
                KnowledgeActionResultDisposition::Completed,
                "evidence-a",
            )],
        ))
        .unwrap();
        assert_eq!(result.disposition, OutcomeAssimilationDisposition::Advanced);
        assert_eq!(result.accepted_order, vec!["action-a"]);
    }

    #[test]
    fn conflicting_retries_are_not_silently_chosen() {
        let result = assimilate_glioma_knowledge_action_outcomes(&request(
            vec![outcome(
                "action-a",
                KnowledgeActionResultDisposition::Completed,
                "evidence-a",
            )],
            vec![outcome(
                "action-a",
                KnowledgeActionResultDisposition::Negative,
                "evidence-b",
            )],
        ))
        .unwrap();
        assert_eq!(result.disposition, OutcomeAssimilationDisposition::Conflict);
        assert_eq!(result.conflict_order, vec!["action-a"]);
        assert_eq!(result.conflict_evidence_order, vec!["evidence-b"]);
    }
}
