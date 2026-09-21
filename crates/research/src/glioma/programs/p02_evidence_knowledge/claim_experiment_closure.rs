//! Claim-to-experiment closure for autonomous preclinical glioma research.
//!
//! This capability joins completed or failed action outcomes to a typed knowledge claim and
//! determines whether the research loop actually closed.  It keeps null, contradictory, missing,
//! and failed outcomes visible, measures modality/model/artifact coverage, and returns a bounded
//! next step for the frontier.  It does not infer causality or make a clinical decision.

use super::action_outcome_assimilation::ActionOutcomeSnapshot;
use super::dispatch::KnowledgeActionResultDisposition;
use super::knowledge_graph::TypedKnowledge;
use crate::glioma::evidence::{EvidenceRecord, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaClaimExperimentClosure1@1";
pub const MAX_CLAIMS: usize = 16_384;
pub const MAX_OUTCOMES: usize = 65_536;
pub const MAX_RECORDS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimExperimentClosureRequest {
    pub objective: String,
    pub knowledge: TypedKnowledge,
    pub action_outcomes: Vec<ActionOutcomeSnapshot>,
    pub evidence: Vec<EvidenceRecord>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub min_support_milli: u16,
    pub min_independent_artifacts: usize,
    pub max_claims: usize,
    pub require_completed_outcome: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimExperimentDisposition {
    Closed,
    Partial,
    Unresolved,
    Negative,
    Contradicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimExperimentClosureDisposition {
    Qualified,
    Partial,
    Negative,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimExperimentResult {
    pub claim_id: String,
    pub action_order: Vec<String>,
    pub completed_action_order: Vec<String>,
    pub failed_action_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub supporting_evidence_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradictory_evidence_order: Vec<String>,
    pub unresolved_evidence_order: Vec<String>,
    pub independent_artifact_count: usize,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub closure_milli: u16,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub omission_order: Vec<String>,
    pub disposition: ClaimExperimentDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimExperimentClosure {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub closed_claim_order: Vec<String>,
    pub partial_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub negative_claim_order: Vec<String>,
    pub contradicted_claim_order: Vec<String>,
    pub claims: Vec<ClaimExperimentResult>,
    pub orphan_action_order: Vec<String>,
    pub orphan_evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: ClaimExperimentClosureDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ClaimExperimentClosureError {
    #[error("claim-experiment closure request is invalid: {0}")]
    InvalidRequest(String),
    #[error("claim-experiment closure output is invalid: {0}")]
    InvalidOutput(String),
    #[error("claim-experiment closure digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ClaimExperimentClosure) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "knowledge_digest": output.knowledge_digest,
        "claim_order": output.claim_order,
        "closed_claim_order": output.closed_claim_order,
        "partial_claim_order": output.partial_claim_order,
        "unresolved_claim_order": output.unresolved_claim_order,
        "negative_claim_order": output.negative_claim_order,
        "contradicted_claim_order": output.contradicted_claim_order,
        "claims": output.claims,
        "orphan_action_order": output.orphan_action_order,
        "orphan_evidence_order": output.orphan_evidence_order,
        "omission_order": output.omission_order,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl ClaimExperimentClosure {
    pub fn validate(&self) -> Result<(), ClaimExperimentClosureError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.knowledge_digest.as_str().len() != 64
            || !canonical(&self.claim_order)
            || !canonical(&self.closed_claim_order)
            || !canonical(&self.partial_claim_order)
            || !canonical(&self.unresolved_claim_order)
            || !canonical(&self.negative_claim_order)
            || !canonical(&self.contradicted_claim_order)
            || !canonical(&self.orphan_action_order)
            || !canonical(&self.orphan_evidence_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.claims.len() != self.claim_order.len()
            || self
                .claims
                .iter()
                .map(|claim| claim.claim_id.clone())
                .collect::<Vec<_>>()
                != self.claim_order
            || self.claims.iter().any(|claim| {
                claim.claim_id.trim().is_empty()
                    || !canonical(&claim.action_order)
                    || !canonical(&claim.completed_action_order)
                    || !canonical(&claim.failed_action_order)
                    || !canonical(&claim.evidence_order)
                    || !canonical(&claim.supporting_evidence_order)
                    || !canonical(&claim.negative_evidence_order)
                    || !canonical(&claim.contradictory_evidence_order)
                    || !canonical(&claim.unresolved_evidence_order)
                    || !canonical(&claim.missing_modality_order)
                    || !canonical(&claim.missing_model_system_order)
                    || !canonical(&claim.omission_order)
                    || claim.support_milli > 1_000
                    || claim.contradiction_milli > 1_000
                    || claim.closure_milli > 1_000
                    || claim
                        .completed_action_order
                        .iter()
                        .any(|id| !claim.action_order.contains(id))
                    || claim
                        .failed_action_order
                        .iter()
                        .any(|id| !claim.action_order.contains(id))
            })
        {
            return Err(ClaimExperimentClosureError::InvalidOutput(
                "identity, ordering, partitions, or score bounds are invalid".into(),
            ));
        }
        let closed = self
            .closed_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partial = self
            .partial_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let negative = self
            .negative_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let contradicted = self
            .contradicted_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let union = closed
            .union(&partial)
            .chain(unresolved.iter())
            .chain(negative.iter())
            .chain(contradicted.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if union.len() != self.claim_order.len()
            || self.claim_order.iter().collect::<BTreeSet<_>>() != union.iter().collect()
            || [
                closed.len(),
                partial.len(),
                unresolved.len(),
                negative.len(),
                contradicted.len(),
            ]
            .into_iter()
            .sum::<usize>()
                != union.len()
        {
            return Err(ClaimExperimentClosureError::InvalidOutput(
                "claim disposition partitions are inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ClaimExperimentClosureError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ClaimExperimentClosureError::Digest(
                "claim-experiment closure digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn evidence_score(record: &EvidenceRecord) -> u16 {
    ((45 * record.quality_milli as u32
        + 35 * record.relevance_milli as u32
        + 20 * record.reproducibility_milli as u32)
        / 100)
        .min(1_000) as u16
}

fn average_score(records: &[&EvidenceRecord]) -> u16 {
    if records.is_empty() {
        return 0;
    }
    (records
        .iter()
        .map(|record| evidence_score(record) as u32)
        .sum::<u32>()
        / records.len() as u32)
        .min(1_000) as u16
}

fn coverage_score<T: Ord>(required: &BTreeSet<T>, observed: &BTreeSet<T>) -> u16 {
    if required.is_empty() {
        1_000
    } else {
        ((required.intersection(observed).count() as u32 * 1_000) / required.len() as u32) as u16
    }
}

/// Join action results to claims and determine whether each research question actually closed.
pub fn close_glioma_claims_to_experiments(
    request: &ClaimExperimentClosureRequest,
) -> Result<ClaimExperimentClosure, ClaimExperimentClosureError> {
    if request.objective.trim().is_empty()
        || request.knowledge.objective.trim().is_empty()
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || request.knowledge.claims.len() > request.max_claims
        || request.action_outcomes.len() > MAX_OUTCOMES
        || request.evidence.len() > MAX_RECORDS
        || request.min_support_milli > 1_000
        || request.min_independent_artifacts > 1_000
    {
        return Err(ClaimExperimentClosureError::InvalidRequest(
            "objective, bounds, claim limits, or score thresholds are invalid".into(),
        ));
    }
    request
        .knowledge
        .validate()
        .map_err(|error| ClaimExperimentClosureError::InvalidRequest(error.to_string()))?;
    let mut outcomes = BTreeMap::<String, ActionOutcomeSnapshot>::new();
    for outcome in &request.action_outcomes {
        if outcome.action_id.trim().is_empty()
            || outcome.claim_id.trim().is_empty()
            || outcome.attempt == 0
            || !canonical(&outcome.evidence_order)
            || outcomes
                .insert(outcome.action_id.clone(), outcome.clone())
                .is_some()
        {
            return Err(ClaimExperimentClosureError::InvalidRequest(
                "action outcomes must have unique ids, positive attempts, and canonical evidence order"
                    .into(),
            ));
        }
    }
    let mut evidence = BTreeMap::<String, EvidenceRecord>::new();
    for record in &request.evidence {
        if record.evidence_id.trim().is_empty()
            || evidence
                .insert(record.evidence_id.clone(), record.clone())
                .is_some()
        {
            return Err(ClaimExperimentClosureError::InvalidRequest(
                "evidence must have unique non-empty ids".into(),
            ));
        }
        record
            .source_artifact
            .validate()
            .map_err(|error| ClaimExperimentClosureError::InvalidRequest(error.to_string()))?;
    }
    let claim_ids = request
        .knowledge
        .claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<BTreeSet<_>>();
    let mut results = Vec::with_capacity(request.knowledge.claims.len());
    let mut orphan_actions = BTreeSet::new();
    let mut used_evidence = BTreeSet::new();
    for outcome in outcomes.values() {
        if !claim_ids.contains(&outcome.claim_id) {
            orphan_actions.insert(outcome.action_id.clone());
        }
    }
    for claim in &request.knowledge.claims {
        let claim_outcomes = outcomes
            .values()
            .filter(|outcome| outcome.claim_id == claim.claim_id)
            .collect::<Vec<_>>();
        let action_order = claim_outcomes
            .iter()
            .map(|outcome| outcome.action_id.clone())
            .collect::<BTreeSet<_>>();
        let mut completed = BTreeSet::new();
        let mut failed = BTreeSet::new();
        let mut evidence_ids = BTreeSet::new();
        let mut omissions = BTreeSet::new();
        for outcome in &claim_outcomes {
            match outcome.disposition {
                KnowledgeActionResultDisposition::Completed
                | KnowledgeActionResultDisposition::Negative => {
                    completed.insert(outcome.action_id.clone());
                }
                KnowledgeActionResultDisposition::Uncertain => {
                    omissions.insert(format!("{}:uncertain-outcome", outcome.action_id));
                }
                KnowledgeActionResultDisposition::Failed => {
                    failed.insert(outcome.action_id.clone());
                    omissions.insert(format!("{}:failed-outcome", outcome.action_id));
                }
            }
            evidence_ids.extend(outcome.evidence_order.iter().cloned());
        }
        if request.require_completed_outcome && completed.is_empty() {
            omissions.insert("no-completed-action-outcome".into());
        }
        let mut records = Vec::new();
        for evidence_id in &evidence_ids {
            if let Some(record) = evidence.get(evidence_id) {
                used_evidence.insert(evidence_id.clone());
                records.push(record);
            } else {
                omissions.insert(format!("missing-evidence:{evidence_id}"));
            }
        }
        let supporting = records
            .iter()
            .filter(|record| record.state == EvidenceState::Supported)
            .copied()
            .collect::<Vec<_>>();
        let negative = records
            .iter()
            .filter(|record| record.state == EvidenceState::Negative)
            .copied()
            .collect::<Vec<_>>();
        let contradictory = records
            .iter()
            .filter(|record| record.state == EvidenceState::Contradicted)
            .copied()
            .collect::<Vec<_>>();
        let unresolved = records
            .iter()
            .filter(|record| {
                matches!(
                    record.state,
                    EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
                )
            })
            .copied()
            .collect::<Vec<_>>();
        let observed_modalities = records
            .iter()
            .map(|record| record.modality)
            .collect::<BTreeSet<_>>();
        let observed_models = records
            .iter()
            .filter_map(|record| record.model_system)
            .collect::<BTreeSet<_>>();
        let artifacts = supporting
            .iter()
            .map(|record| record.source_artifact.artifact_id.clone())
            .collect::<BTreeSet<_>>();
        let missing_modalities = request
            .required_modalities
            .difference(&observed_modalities)
            .copied()
            .collect::<Vec<_>>();
        let missing_models = request
            .required_model_systems
            .difference(&observed_models)
            .copied()
            .collect::<Vec<_>>();
        let support = average_score(&supporting);
        let contradiction = average_score(&contradictory);
        let modality_coverage = coverage_score(&request.required_modalities, &observed_modalities);
        let model_coverage = coverage_score(&request.required_model_systems, &observed_models);
        let artifact_coverage = if request.min_independent_artifacts == 0 {
            1_000
        } else {
            ((artifacts.len().min(request.min_independent_artifacts) * 1_000)
                / request.min_independent_artifacts) as u16
        };
        let closure = support
            .min(modality_coverage)
            .min(model_coverage)
            .min(artifact_coverage);
        let disposition =
            if supporting.is_empty() && !negative.is_empty() && contradictory.is_empty() {
                ClaimExperimentDisposition::Negative
            } else if contradiction >= request.min_support_milli && contradiction > support {
                ClaimExperimentDisposition::Contradicted
            } else if support >= request.min_support_milli
                && missing_modalities.is_empty()
                && missing_models.is_empty()
                && artifacts.len() >= request.min_independent_artifacts
                && contradictory.is_empty()
                && (!request.require_completed_outcome || !completed.is_empty())
            {
                ClaimExperimentDisposition::Closed
            } else if !supporting.is_empty()
                || !negative.is_empty()
                || !contradictory.is_empty()
                || !completed.is_empty()
            {
                ClaimExperimentDisposition::Partial
            } else {
                ClaimExperimentDisposition::Unresolved
            };
        if !unresolved.is_empty() {
            omissions.insert("unresolved-evidence-present".into());
        }
        if !missing_modalities.is_empty() {
            omissions.insert("required-modality-coverage-missing".into());
        }
        if !missing_models.is_empty() {
            omissions.insert("required-model-coverage-missing".into());
        }
        results.push(ClaimExperimentResult {
            claim_id: claim.claim_id.clone(),
            action_order: action_order.into_iter().collect(),
            completed_action_order: completed.into_iter().collect(),
            failed_action_order: failed.into_iter().collect(),
            evidence_order: evidence_ids.into_iter().collect(),
            supporting_evidence_order: supporting
                .iter()
                .map(|record| record.evidence_id.clone())
                .collect(),
            negative_evidence_order: negative
                .iter()
                .map(|record| record.evidence_id.clone())
                .collect(),
            contradictory_evidence_order: contradictory
                .iter()
                .map(|record| record.evidence_id.clone())
                .collect(),
            unresolved_evidence_order: unresolved
                .iter()
                .map(|record| record.evidence_id.clone())
                .collect(),
            independent_artifact_count: artifacts.len(),
            support_milli: support,
            contradiction_milli: contradiction,
            closure_milli: closure,
            missing_modality_order: missing_modalities,
            missing_model_system_order: missing_models,
            omission_order: omissions.into_iter().collect(),
            disposition,
        });
    }
    results.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let mut closed = BTreeSet::new();
    let mut partial = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for result in &results {
        match result.disposition {
            ClaimExperimentDisposition::Closed => {
                closed.insert(result.claim_id.clone());
            }
            ClaimExperimentDisposition::Partial => {
                partial.insert(result.claim_id.clone());
            }
            ClaimExperimentDisposition::Unresolved => {
                unresolved.insert(result.claim_id.clone());
            }
            ClaimExperimentDisposition::Negative => {
                negative.insert(result.claim_id.clone());
            }
            ClaimExperimentDisposition::Contradicted => {
                contradicted.insert(result.claim_id.clone());
            }
        }
        omissions.extend(
            result
                .omission_order
                .iter()
                .map(|omission| format!("{}:{omission}", result.claim_id)),
        );
        negative_evidence.extend(result.negative_evidence_order.iter().cloned());
        if result.disposition != ClaimExperimentDisposition::Closed {
            uncertainty.insert(format!("{}:closure-not-qualified", result.claim_id));
        }
    }
    let orphan_evidence = evidence
        .keys()
        .filter(|evidence_id| !used_evidence.contains(*evidence_id))
        .cloned()
        .collect::<Vec<_>>();
    let claim_order = results
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let disposition = if results.is_empty()
        || (closed.is_empty()
            && partial.is_empty()
            && negative.is_empty()
            && contradicted.is_empty())
    {
        ClaimExperimentClosureDisposition::Blocked
    } else if !closed.is_empty()
        && partial.is_empty()
        && unresolved.is_empty()
        && negative.is_empty()
        && contradicted.is_empty()
    {
        ClaimExperimentClosureDisposition::Qualified
    } else if closed.is_empty()
        && !negative.is_empty()
        && partial.is_empty()
        && unresolved.is_empty()
        && contradicted.is_empty()
    {
        ClaimExperimentClosureDisposition::Negative
    } else {
        ClaimExperimentClosureDisposition::Partial
    };
    let next_step = match disposition {
        ClaimExperimentClosureDisposition::Qualified => {
            "compile the closed claims into the next typed-knowledge and replication frontier".into()
        }
        ClaimExperimentClosureDisposition::Negative => {
            "retain the null result and prioritize a falsifier or boundary-condition action".into()
        }
        ClaimExperimentClosureDisposition::Partial => {
            "compile omissions and contradictions into the next claim-specific acquisition or experiment action".into()
        }
        ClaimExperimentClosureDisposition::Blocked => {
            "admit a bounded action for each unresolved claim before treating the research loop as closed".into()
        }
    };
    let mut output = ClaimExperimentClosure {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: request.knowledge.digest.clone(),
        claim_order,
        closed_claim_order: closed.into_iter().collect(),
        partial_claim_order: partial.into_iter().collect(),
        unresolved_claim_order: unresolved.into_iter().collect(),
        negative_claim_order: negative.into_iter().collect(),
        contradicted_claim_order: contradicted.into_iter().collect(),
        claims: results,
        orphan_action_order: orphan_actions.into_iter().collect(),
        orphan_evidence_order: orphan_evidence,
        omission_order: omissions.into_iter().collect(),
        negative_evidence_order: negative_evidence.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ClaimExperimentClosureError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        KnowledgeClaim, KnowledgeClaimDisposition, KnowledgeDisposition,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn knowledge() -> TypedKnowledge {
        let claim = KnowledgeClaim {
            claim_id: "claim-egfr".into(),
            statement: "egfr drives resistance".into(),
            scope: "organoid".into(),
            modality_order: vec![GliomaModality::Genomics],
            model_system_order: vec![GliomaModelSystem::Organoid],
            supporting_evidence_order: vec![],
            negative_evidence_order: vec![],
            contradictory_evidence_order: vec![],
            unresolved_evidence_order: vec![],
            missing_modality_order: vec![],
            missing_model_system_order: vec![],
            support_milli: 900,
            contradiction_milli: 0,
            confidence_milli: 900,
            disposition: KnowledgeClaimDisposition::Supported,
        };
        let mut value = TypedKnowledge {
            feature_id: super::super::knowledge_graph::FEATURE_ID.into(),
            output_schema: super::super::knowledge_graph::OUTPUT_SCHEMA.into(),
            objective: "closure".into(),
            claims: vec![claim],
            claim_order: vec!["claim-egfr".into()],
            top_claim_order: vec!["claim-egfr".into()],
            omission_order: vec![],
            negative_evidence_order: vec![],
            uncertainty_order: vec![],
            disposition: KnowledgeDisposition::Qualified,
            digest: ContentHash::of_bytes(b"placeholder"),
        };
        let input = serde_json::json!({
            "feature_id": value.feature_id,
            "output_schema": value.output_schema,
            "objective": value.objective,
            "claims": value.claims,
            "claim_order": value.claim_order,
            "top_claim_order": value.top_claim_order,
            "omission_order": value.omission_order,
            "negative_evidence_order": value.negative_evidence_order,
            "uncertainty_order": value.uncertainty_order,
            "disposition": value.disposition,
        });
        value.digest = ContentHash::of_value(&input).unwrap();
        value
    }

    fn record(id: &str, state: EvidenceState) -> EvidenceRecord {
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
            source_kind: EvidenceSourceKind::Assay,
            claim: "egfr drives resistance".into(),
            scope: "organoid".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        }
    }

    fn request(
        disposition: KnowledgeActionResultDisposition,
        evidence_id: &str,
        evidence_state: EvidenceState,
    ) -> ClaimExperimentClosureRequest {
        ClaimExperimentClosureRequest {
            objective: "close claim".into(),
            knowledge: knowledge(),
            action_outcomes: vec![ActionOutcomeSnapshot {
                action_id: "action-egfr".into(),
                claim_id: "claim-egfr".into(),
                attempt: 1,
                cost_units: 2,
                evidence_order: vec![evidence_id.into()],
                disposition,
                failure: None,
            }],
            evidence: vec![record(evidence_id, evidence_state)],
            required_modalities: [GliomaModality::Genomics].into_iter().collect(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            min_support_milli: 700,
            min_independent_artifacts: 1,
            max_claims: 10,
            require_completed_outcome: true,
        }
    }

    #[test]
    fn completed_supported_action_closes_claim() {
        let output = close_glioma_claims_to_experiments(&request(
            KnowledgeActionResultDisposition::Completed,
            "evidence-egfr",
            EvidenceState::Supported,
        ))
        .unwrap();
        assert_eq!(
            output.disposition,
            ClaimExperimentClosureDisposition::Qualified
        );
        assert_eq!(output.closed_claim_order, vec!["claim-egfr"]);
        output.validate().unwrap();
    }

    #[test]
    fn failed_action_is_explicitly_unresolved() {
        let output = close_glioma_claims_to_experiments(&request(
            KnowledgeActionResultDisposition::Failed,
            "evidence-missing",
            EvidenceState::Unknown,
        ))
        .unwrap();
        assert_eq!(
            output.disposition,
            ClaimExperimentClosureDisposition::Blocked
        );
        assert!(output.claims[0]
            .omission_order
            .iter()
            .any(|item| item.contains("failed-outcome")));
    }

    #[test]
    fn negative_result_is_first_class() {
        let output = close_glioma_claims_to_experiments(&request(
            KnowledgeActionResultDisposition::Negative,
            "evidence-null",
            EvidenceState::Negative,
        ))
        .unwrap();
        assert_eq!(
            output.disposition,
            ClaimExperimentClosureDisposition::Negative
        );
        assert_eq!(output.negative_claim_order, vec!["claim-egfr"]);
    }
}
