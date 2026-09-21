//! Multi-study typed decision-context artifact for preclinical glioma research.
//!
//! This feature aligns the portable P04 context artifacts produced by independent studies. It
//! exchanges only typed metadata and action contracts, not raw evidence. An action is promoted to
//! the shared frontier only when its definition is compatible, its support spans the configured
//! number of studies and independent groups, and its omissions/negative/unknown states are not
//! hidden by a complete-case shortcut.

use super::decision_context_artifact::{
    DecisionContextArtifact, DecisionContextArtifactAction, DecisionContextArtifactCompatibility,
};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiStudyDecisionContextArtifact1@1";
pub const MAX_STUDIES: usize = 256;
pub const MAX_ACTIONS: usize = 512;
pub const MAX_PARTITION_ITEMS: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyActionDisposition {
    Qualified,
    Underpowered,
    Conflicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiStudyContextDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyContextInput {
    pub study_id: String,
    pub independent_group: String,
    pub quality_milli: u16,
    pub policy_allowed: bool,
    pub artifact: DecisionContextArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyContextRequest {
    pub objective: String,
    pub epoch: u32,
    pub minimum_studies: usize,
    pub minimum_independent_groups: usize,
    pub minimum_action_support_milli: u16,
    pub minimum_quality_milli: u16,
    pub maximum_actions: usize,
    pub compatibility: DecisionContextArtifactCompatibility,
    pub studies: Vec<MultiStudyContextInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyDecisionAction {
    pub action: DecisionContextArtifactAction,
    pub study_order: Vec<String>,
    pub independent_group_order: Vec<String>,
    pub support_milli: u16,
    pub disagreement_milli: u16,
    pub disposition: MultiStudyActionDisposition,
    pub negative_study_order: Vec<String>,
    pub unknown_study_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiStudyDecisionContextArtifact {
    pub feature_id: String,
    pub output_schema: String,
    pub artifact_id: String,
    pub objective: String,
    pub epoch: u32,
    pub boundary: String,
    pub study_order: Vec<String>,
    pub eligible_study_order: Vec<String>,
    pub omitted_study_order: Vec<String>,
    pub action_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub actions: Vec<MultiStudyDecisionAction>,
    pub omissions: BTreeMap<String, String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: MultiStudyContextDisposition,
    pub compatibility: DecisionContextArtifactCompatibility,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiStudyContextError {
    #[error("multi-study context request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-study context input is invalid: {0}")]
    InvalidInput(String),
    #[error("multi-study context artifact is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-study context digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ranked_unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(output: &MultiStudyDecisionContextArtifact) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "artifact_id": output.artifact_id,
        "objective": output.objective,
        "epoch": output.epoch,
        "boundary": output.boundary,
        "study_order": output.study_order,
        "eligible_study_order": output.eligible_study_order,
        "omitted_study_order": output.omitted_study_order,
        "action_order": output.action_order,
        "frontier_order": output.frontier_order,
        "actions": output.actions,
        "omissions": output.omissions,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
        "compatibility": output.compatibility,
    })
}

impl MultiStudyDecisionContextArtifact {
    pub fn validate(&self) -> Result<(), MultiStudyContextError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.artifact_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.epoch == 0
            || self.boundary != PRECLINICAL_BOUNDARY
            || !canonical(&self.study_order)
            || !canonical(&self.eligible_study_order)
            || !canonical(&self.omitted_study_order)
            || !canonical(&self.action_order)
            || !ranked_unique(&self.frontier_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.actions.len() != self.action_order.len()
            || self
                .actions
                .iter()
                .map(|entry| entry.action.action_id.clone())
                .collect::<Vec<_>>()
                != self.action_order
            || self.actions.len() > MAX_ACTIONS
            || self.negative_evidence_order.len() > MAX_PARTITION_ITEMS
            || self.uncertainty_order.len() > MAX_PARTITION_ITEMS
            || self.compatibility.contract_version.trim().is_empty()
            || self.compatibility.consumer_order.is_empty()
            || !canonical(&self.compatibility.consumer_order)
            || !canonical(&self.compatibility.semantic_loss_order)
            || self.compatibility.local_raw_data_required
            || self.compatibility.clinical_decision_capable
            || self.digest.as_str().len() != 64
        {
            return Err(MultiStudyContextError::InvalidOutput(
                "identity, boundary, ordering, bounds, action partition, or digest fields are invalid".into(),
            ));
        }
        let study_set = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        let eligible_set = self
            .eligible_study_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let omitted_set = self
            .omitted_study_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if eligible_set.intersection(&omitted_set).next().is_some()
            || eligible_set
                .union(&omitted_set)
                .cloned()
                .collect::<BTreeSet<_>>()
                != study_set
            || self.actions.iter().any(|entry| {
                entry.action.action_id.trim().is_empty()
                    || entry.action.claim_id.trim().is_empty()
                    || entry.action.cost_units == 0
                    || entry.action.effects.is_empty()
                    || !canonical(&entry.study_order)
                    || !canonical(&entry.independent_group_order)
                    || !canonical(&entry.negative_study_order)
                    || !canonical(&entry.unknown_study_order)
                    || entry.support_milli > 1_000
                    || entry.disagreement_milli > 1_000
            })
        {
            return Err(MultiStudyContextError::InvalidOutput(
                "study partition or action summary invariants are inconsistent".into(),
            ));
        }
        let action_set = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        if self
            .frontier_order
            .iter()
            .any(|action| !action_set.contains(action))
        {
            return Err(MultiStudyContextError::InvalidOutput(
                "frontier references an unknown action".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiStudyContextError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiStudyContextError::Digest(
                "multi-study context digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn action_signature(action: &DecisionContextArtifactAction) -> serde_json::Value {
    serde_json::json!({
        "action_id": action.action_id,
        "claim_id": action.claim_id,
        "kind": action.kind,
        "stage_kind": action.stage_kind,
        "target_modality": action.target_modality,
        "target_model_system": action.target_model_system,
        "priority_milli": action.priority_milli,
        "cost_units": action.cost_units,
        "depends_on": action.depends_on,
        "autonomy_tier": action.autonomy_tier,
        "effects": action.effects,
    })
}

/// Align compatible P04 artifacts into a deterministic multi-study action frontier.
pub fn align_glioma_multi_study_context_artifacts(
    request: &MultiStudyContextRequest,
) -> Result<MultiStudyDecisionContextArtifact, MultiStudyContextError> {
    if request.objective.trim().is_empty()
        || request.epoch == 0
        || request.minimum_studies == 0
        || request.minimum_independent_groups == 0
        || request.minimum_action_support_milli > 1_000
        || request.minimum_quality_milli > 1_000
        || request.maximum_actions == 0
        || request.maximum_actions > MAX_ACTIONS
        || request.studies.is_empty()
        || request.studies.len() > MAX_STUDIES
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "objective, epoch, quorum, quality, support, action, and study bounds are invalid"
                .into(),
        ));
    }
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    if studies
        .windows(2)
        .any(|pair| pair[0].study_id == pair[1].study_id)
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "study identifiers must be unique".into(),
        ));
    }
    if studies.iter().any(|study| {
        study.study_id.trim().is_empty()
            || study.independent_group.trim().is_empty()
            || study.quality_milli > 1_000
            || study.artifact.objective != request.objective
            || study.artifact.study_id != study.study_id
            || study.artifact.epoch > request.epoch
    }) {
        return Err(MultiStudyContextError::InvalidInput(
            "study identity, quality, objective, epoch, and artifact bindings are invalid".into(),
        ));
    }
    for study in &studies {
        study
            .artifact
            .validate()
            .map_err(|error| MultiStudyContextError::InvalidInput(error.to_string()))?;
        if study.artifact.compatibility != request.compatibility {
            return Err(MultiStudyContextError::InvalidInput(
                "every source artifact must use the requested compatibility contract".into(),
            ));
        }
    }
    if request.compatibility.consumer_order != studies[0].artifact.compatibility.consumer_order
        || request.compatibility.contract_version
            != studies[0].artifact.compatibility.contract_version
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "request compatibility must match the first source artifact contract".into(),
        ));
    }
    if request.compatibility.consumer_order.is_empty()
        || !canonical(&request.compatibility.consumer_order)
        || !canonical(&request.compatibility.semantic_loss_order)
        || request.compatibility.local_raw_data_required
        || request.compatibility.clinical_decision_capable
    {
        return Err(MultiStudyContextError::InvalidRequest(
            "multi-study compatibility must be ordered, loss-explicit, local-only, and preclinical"
                .into(),
        ));
    }
    let study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let mut omissions = BTreeMap::new();
    let mut eligible = Vec::new();
    for study in &studies {
        let reason = if !study.policy_allowed {
            Some("study_policy_denied")
        } else if study.quality_milli < request.minimum_quality_milli {
            Some("study_quality_below_threshold")
        } else {
            None
        };
        if let Some(reason) = reason {
            omissions.insert(study.study_id.clone(), reason.into());
        } else {
            eligible.push(study);
        }
    }
    let eligible_study_order = eligible
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let omitted_study_order = omissions.keys().cloned().collect::<Vec<_>>();
    let mut action_ids = eligible
        .iter()
        .flat_map(|study| {
            study
                .artifact
                .actions
                .iter()
                .map(|entry| entry.action_id.clone())
        })
        .collect::<BTreeSet<_>>();
    if action_ids.len() > request.maximum_actions {
        let retained = action_ids
            .iter()
            .take(request.maximum_actions)
            .cloned()
            .collect::<BTreeSet<_>>();
        for action_id in action_ids.difference(&retained) {
            omissions.insert(
                format!("action:{action_id}"),
                "action_capacity_exceeded".into(),
            );
        }
        action_ids = retained;
    }
    let action_order = action_ids.iter().cloned().collect::<Vec<_>>();
    let mut actions = Vec::with_capacity(action_order.len());
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for study in &eligible {
        negative_evidence.extend(
            study
                .artifact
                .negative_evidence_order
                .iter()
                .map(|item| format!("{}:{item}", study.study_id)),
        );
        uncertainty.extend(
            study
                .artifact
                .uncertainty_order
                .iter()
                .map(|item| format!("{}:{item}", study.study_id)),
        );
    }
    for action_id in &action_order {
        let occurrences = eligible
            .iter()
            .filter_map(|study| {
                study
                    .artifact
                    .actions
                    .iter()
                    .find(|entry| entry.action_id == *action_id)
                    .map(|entry| (*study, entry))
            })
            .collect::<Vec<_>>();
        let first = occurrences.first().ok_or_else(|| {
            MultiStudyContextError::InvalidOutput("action disappeared during aggregation".into())
        })?;
        let first_signature = action_signature(first.1);
        let conflicted = occurrences
            .iter()
            .any(|(_, entry)| action_signature(entry) != first_signature);
        let study_order_for_action = occurrences
            .iter()
            .map(|(study, _)| study.study_id.clone())
            .collect::<Vec<_>>();
        let group_order = occurrences
            .iter()
            .map(|(study, _)| study.independent_group.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let support_milli =
            ((occurrences.len() as u64 * 1_000) / eligible.len().max(1) as u64).min(1_000) as u16;
        let disagreement_milli = if conflicted {
            1_000
        } else {
            1_000_u16.saturating_sub(support_milli)
        };
        let disposition = if conflicted {
            MultiStudyActionDisposition::Conflicted
        } else if occurrences.len() < request.minimum_studies
            || group_order.len() < request.minimum_independent_groups
            || support_milli < request.minimum_action_support_milli
        {
            MultiStudyActionDisposition::Underpowered
        } else {
            MultiStudyActionDisposition::Qualified
        };
        if disposition != MultiStudyActionDisposition::Qualified {
            uncertainty.insert(format!("action:{action_id}:multi-study-gate-unresolved"));
        }
        actions.push(MultiStudyDecisionAction {
            action: first.1.clone(),
            study_order: study_order_for_action,
            independent_group_order: group_order,
            support_milli,
            disagreement_milli,
            disposition,
            negative_study_order: Vec::new(),
            unknown_study_order: Vec::new(),
        });
    }
    actions.sort_by(|left, right| left.action.action_id.cmp(&right.action.action_id));
    let mut frontier = actions
        .iter()
        .filter(|entry| entry.disposition == MultiStudyActionDisposition::Qualified)
        .map(|entry| {
            (
                entry.action.action_id.clone(),
                entry.support_milli,
                entry.action.priority_milli,
            )
        })
        .collect::<Vec<_>>();
    frontier.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    let frontier_order = frontier
        .into_iter()
        .map(|entry| entry.0)
        .collect::<Vec<_>>();
    let disposition = if !frontier_order.is_empty() {
        MultiStudyContextDisposition::Qualified
    } else if !actions.is_empty() || !eligible.is_empty() {
        MultiStudyContextDisposition::Partial
    } else {
        MultiStudyContextDisposition::Unresolved
    };
    let mut negative_evidence_order = negative_evidence.into_iter().collect::<Vec<_>>();
    negative_evidence_order.sort();
    let mut uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty_order.sort();
    let mut output = MultiStudyDecisionContextArtifact {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        artifact_id: format!(
            "glioma-multi-study-decision-context:epoch-{}",
            request.epoch
        ),
        objective: request.objective.clone(),
        epoch: request.epoch,
        boundary: PRECLINICAL_BOUNDARY.into(),
        study_order,
        eligible_study_order,
        omitted_study_order,
        action_order: actions
            .iter()
            .map(|entry| entry.action.action_id.clone())
            .collect(),
        frontier_order,
        actions,
        omissions,
        negative_evidence_order,
        uncertainty_order,
        disposition,
        compatibility: request.compatibility.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-multi-study-context"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultiStudyContextError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma::programs::p04_decision_context::decision_context_artifact::{
        materialize_glioma_decision_context_artifact, DecisionContextArtifactConsumer,
        DecisionContextArtifactRequest,
    };
    use crate::glioma::programs::p04_decision_context::{
        compile_decision_context, DecisionContextRequest,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn artifact(study_id: &str) -> DecisionContextArtifact {
        let evidence = EvidenceRecord {
            evidence_id: format!("{study_id}-evidence"),
            source_artifact: LocalArtifactRef {
                artifact_id: format!("{study_id}-artifact"),
                content_hash: hash(study_id),
                content_type: "application/vnd.aurora.glioma-evidence+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_kind: EvidenceSourceKind::Dataset,
            claim: "EGFR signaling increases invasion".into(),
            scope: "preclinical glioma".into(),
            modality: GliomaModality::Proteomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 900,
            release_epoch: 1,
        };
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "align contexts".into(),
                required_modalities: BTreeSet::from([GliomaModality::Proteomics]),
                required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                min_support_milli: 700,
                min_sources_per_claim: 1,
                max_claims: 4,
            },
            &[evidence],
        )
        .unwrap();
        let context = compile_decision_context(
            &DecisionContextRequest {
                objective: "align contexts".into(),
                max_actions: 4,
                default_cost_units: 2,
            },
            &knowledge,
        )
        .unwrap();
        materialize_glioma_decision_context_artifact(&DecisionContextArtifactRequest {
            objective: "align contexts".into(),
            study_id: study_id.into(),
            epoch: 2,
            compatibility: compatibility(),
            context,
        })
        .unwrap()
    }

    fn compatibility() -> DecisionContextArtifactCompatibility {
        DecisionContextArtifactCompatibility {
            contract_version: "glioma-context-compat/1".into(),
            consumer_order: vec![
                DecisionContextArtifactConsumer::LocalAgent,
                DecisionContextArtifactConsumer::ResearcherWorkbench,
                DecisionContextArtifactConsumer::RustSdk,
                DecisionContextArtifactConsumer::PythonSdk,
                DecisionContextArtifactConsumer::TypeScriptSdk,
                DecisionContextArtifactConsumer::McpClient,
            ],
            semantic_loss_order: Vec::new(),
            local_raw_data_required: false,
            clinical_decision_capable: false,
        }
    }

    fn request(studies: Vec<MultiStudyContextInput>) -> MultiStudyContextRequest {
        MultiStudyContextRequest {
            objective: "align contexts".into(),
            epoch: 3,
            minimum_studies: 2,
            minimum_independent_groups: 2,
            minimum_action_support_milli: 700,
            minimum_quality_milli: 700,
            maximum_actions: 8,
            compatibility: compatibility(),
            studies,
        }
    }

    fn study(study_id: &str, group: &str) -> MultiStudyContextInput {
        MultiStudyContextInput {
            study_id: study_id.into(),
            independent_group: group.into(),
            quality_milli: 900,
            policy_allowed: true,
            artifact: artifact(study_id),
        }
    }

    fn reseal(artifact: &mut DecisionContextArtifact) {
        let input = serde_json::json!({
            "feature_id": artifact.feature_id,
            "output_schema": artifact.output_schema,
            "artifact_id": artifact.artifact_id,
            "objective": artifact.objective,
            "study_id": artifact.study_id,
            "epoch": artifact.epoch,
            "boundary": artifact.boundary,
            "source_context_digest": artifact.source_context_digest,
            "claim_order": artifact.claim_order,
            "actions": artifact.actions,
            "action_order": artifact.action_order,
            "deferred_action_order": artifact.deferred_action_order,
            "omission_order": artifact.omission_order,
            "negative_evidence_order": artifact.negative_evidence_order,
            "uncertainty_order": artifact.uncertainty_order,
            "compatibility": artifact.compatibility,
        });
        artifact.digest = ContentHash::of_value(&input).unwrap();
    }

    #[test]
    fn compatible_studies_promote_shared_frontier_deterministically() {
        let first = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-a", "group-a"),
            study("study-b", "group-b"),
        ]))
        .unwrap();
        let second = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-b", "group-b"),
            study("study-a", "group-a"),
        ]))
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, MultiStudyContextDisposition::Qualified);
        assert_eq!(first.frontier_order.len(), 1);
        first.validate().unwrap();
    }

    #[test]
    fn denied_study_is_omitted_and_shared_action_becomes_underpowered() {
        let mut denied = study("study-b", "group-b");
        denied.policy_allowed = false;
        let output = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-a", "group-a"),
            denied,
        ]))
        .unwrap();
        assert_eq!(output.omissions["study-b"], "study_policy_denied");
        assert_eq!(output.disposition, MultiStudyContextDisposition::Partial);
        assert!(output.frontier_order.is_empty());
    }

    #[test]
    fn typed_action_conflict_is_retained_but_not_promoted() {
        let mut conflicting = study("study-b", "group-b");
        conflicting.artifact.actions[0].cost_units += 1;
        reseal(&mut conflicting.artifact);
        let output = align_glioma_multi_study_context_artifacts(&request(vec![
            study("study-a", "group-a"),
            conflicting,
        ]))
        .unwrap();
        assert_eq!(
            output.actions[0].disposition,
            MultiStudyActionDisposition::Conflicted
        );
        assert_eq!(output.actions[0].disagreement_milli, 1_000);
        assert!(output
            .uncertainty_order
            .iter()
            .any(|item| item.contains("multi-study-gate")));
    }

    #[test]
    fn nested_negative_and_unknown_partitions_are_namespaced_by_study() {
        let mut first = study("study-a", "group-a");
        first.artifact.negative_evidence_order = vec!["negative-a".into()];
        first.artifact.uncertainty_order = vec!["unknown-a".into()];
        reseal(&mut first.artifact);
        let output = align_glioma_multi_study_context_artifacts(&request(vec![
            first,
            study("study-b", "group-b"),
        ]))
        .unwrap();
        assert!(output
            .negative_evidence_order
            .contains(&"study-a:negative-a".into()));
        assert!(output
            .uncertainty_order
            .contains(&"study-a:unknown-a".into()));
    }
}
