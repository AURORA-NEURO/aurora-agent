//! Prospective typed-knowledge drift detection for autonomous preclinical glioma research.
//!
//! Evidence surveillance can detect changing source observations, but an autonomous engine also
//! needs to know when the compiled claim world changed enough to replan. This module compares two
//! validated, content-addressed `TypedKnowledge` snapshots, classifies claim additions, removals,
//! strengthening, weakening, contradiction, and resolution, and emits bounded downstream actions.
//! It never invents a claim or treats a confidence change as causal or clinical evidence.

use super::knowledge_graph::{KnowledgeClaim, KnowledgeClaimDisposition, TypedKnowledge};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeDrift1@1";
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDriftRequest {
    pub objective: String,
    pub previous: TypedKnowledge,
    pub current: TypedKnowledge,
    pub min_change_milli: u16,
    pub min_priority_milli: u16,
    pub max_actions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeDriftKind {
    Added,
    Removed,
    Strengthened,
    Weakened,
    Contradicted,
    Resolved,
    Stable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDriftAction {
    pub action_id: String,
    pub claim_id: String,
    pub kind: KnowledgeDriftKind,
    pub previous_confidence_milli: Option<u16>,
    pub current_confidence_milli: Option<u16>,
    pub previous_disposition: Option<KnowledgeClaimDisposition>,
    pub current_disposition: Option<KnowledgeClaimDisposition>,
    pub delta_milli: i32,
    pub priority_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeDriftDisposition {
    Ready,
    Partial,
    NoMaterialChange,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDrift {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub previous_knowledge_digest: ContentHash,
    pub current_knowledge_digest: ContentHash,
    pub transition_order: Vec<String>,
    pub action_order: Vec<String>,
    pub actions: Vec<KnowledgeDriftAction>,
    pub stable_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: KnowledgeDriftDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeDriftError {
    #[error("knowledge drift request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge drift output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge drift digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn disposition_rank(disposition: KnowledgeClaimDisposition) -> u16 {
    match disposition {
        KnowledgeClaimDisposition::Supported => 1_000,
        KnowledgeClaimDisposition::Contested => 500,
        KnowledgeClaimDisposition::Negative => 200,
        KnowledgeClaimDisposition::Unresolved => 0,
    }
}

fn action_priority(
    kind: KnowledgeDriftKind,
    delta_milli: i32,
    previous: Option<&KnowledgeClaim>,
    current: Option<&KnowledgeClaim>,
) -> u16 {
    let magnitude = delta_milli.unsigned_abs().min(1_000);
    let confidence = current
        .or(previous)
        .map(|claim| u32::from(claim.confidence_milli))
        .unwrap_or(0);
    let disposition_pressure = match kind {
        KnowledgeDriftKind::Contradicted | KnowledgeDriftKind::Removed => 1_000,
        KnowledgeDriftKind::Resolved | KnowledgeDriftKind::Strengthened => 800,
        KnowledgeDriftKind::Weakened | KnowledgeDriftKind::Added => 700,
        KnowledgeDriftKind::Stable => 0,
    };
    (magnitude
        .saturating_mul(confidence)
        .saturating_mul(disposition_pressure)
        / 1_000_000)
        .min(1_000) as u16
}

fn digest_input(output: &KnowledgeDrift) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "previous_knowledge_digest": output.previous_knowledge_digest,
        "current_knowledge_digest": output.current_knowledge_digest,
        "transition_order": output.transition_order,
        "action_order": output.action_order,
        "actions": output.actions,
        "stable_order": output.stable_order,
        "unresolved_order": output.unresolved_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl KnowledgeDrift {
    pub fn validate(&self) -> Result<(), KnowledgeDriftError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.transition_order)
            || !canonical(&self.stable_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.actions.len() > MAX_CLAIMS
            || self.actions.iter().any(|action| {
                action.action_id != format!("knowledge-drift:{}", action.claim_id)
                    || action.priority_milli > 1_000
                    || action.rationale.trim().is_empty()
                    || action.claim_id.trim().is_empty()
            })
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
        {
            return Err(KnowledgeDriftError::InvalidOutput(
                "identity, ordering, action bounds, or rationale is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeDriftError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeDriftError::InvalidOutput(
                "knowledge drift digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn classify(
    previous: Option<&KnowledgeClaim>,
    current: Option<&KnowledgeClaim>,
    min_change_milli: u16,
) -> (KnowledgeDriftKind, i32, String) {
    match (previous, current) {
        (None, Some(claim)) => (
            KnowledgeDriftKind::Added,
            i32::from(claim.confidence_milli),
            "a new typed claim entered the compiled knowledge world; validate its support before downstream execution".into(),
        ),
        (Some(claim), None) => (
            KnowledgeDriftKind::Removed,
            -i32::from(claim.confidence_milli),
            "a previously compiled claim disappeared; preserve the loss as negative evidence and recheck source freshness".into(),
        ),
        (Some(previous), Some(current)) => {
            let delta = i32::from(current.confidence_milli)
                .saturating_sub(i32::from(previous.confidence_milli));
            let previous_rank = disposition_rank(previous.disposition);
            let current_rank = disposition_rank(current.disposition);
            let kind = if matches!(current.disposition, KnowledgeClaimDisposition::Contested)
                && !matches!(previous.disposition, KnowledgeClaimDisposition::Contested)
            {
                KnowledgeDriftKind::Contradicted
            } else if matches!(previous.disposition, KnowledgeClaimDisposition::Contested)
                && matches!(current.disposition, KnowledgeClaimDisposition::Supported)
            {
                KnowledgeDriftKind::Resolved
            } else if delta >= i32::from(min_change_milli) || current_rank > previous_rank {
                KnowledgeDriftKind::Strengthened
            } else if delta <= -i32::from(min_change_milli) || current_rank < previous_rank {
                KnowledgeDriftKind::Weakened
            } else {
                KnowledgeDriftKind::Stable
            };
            let rationale = match kind {
                KnowledgeDriftKind::Contradicted => {
                    "the current typed world marks the claim contested; route contradiction adjudication before autonomous replanning"
                }
                KnowledgeDriftKind::Resolved => {
                    "a formerly contested claim is now supported; require current provenance and independent validation before promotion"
                }
                KnowledgeDriftKind::Strengthened => {
                    "claim confidence or disposition strengthened beyond the drift gate; refresh downstream action value"
                }
                KnowledgeDriftKind::Weakened => {
                    "claim confidence or disposition weakened beyond the drift gate; preserve uncertainty and recheck dependencies"
                }
                KnowledgeDriftKind::Stable => "no material typed-knowledge change was detected",
                KnowledgeDriftKind::Added | KnowledgeDriftKind::Removed => unreachable!(),
            };
            (kind, delta, rationale.into())
        }
        (None, None) => unreachable!(),
    }
}

pub fn detect_glioma_knowledge_drift(
    request: &KnowledgeDriftRequest,
) -> Result<KnowledgeDrift, KnowledgeDriftError> {
    if request.objective.trim().is_empty()
        || request.previous.objective != request.objective
        || request.current.objective != request.objective
        || request.min_change_milli > 1_000
        || request.min_priority_milli > 1_000
        || request.max_actions == 0
        || request.max_actions > MAX_CLAIMS
    {
        return Err(KnowledgeDriftError::InvalidRequest(
            "objective binding, confidence/priority gates, and bounded action capacity are required".into(),
        ));
    }
    request
        .previous
        .validate()
        .map_err(|error| KnowledgeDriftError::InvalidRequest(error.to_string()))?;
    request
        .current
        .validate()
        .map_err(|error| KnowledgeDriftError::InvalidRequest(error.to_string()))?;
    let previous = request
        .previous
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let current = request
        .current
        .claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    if previous.len() > MAX_CLAIMS || current.len() > MAX_CLAIMS {
        return Err(KnowledgeDriftError::InvalidRequest(
            "typed knowledge claim count exceeds the supported bound".into(),
        ));
    }
    let claim_ids = previous
        .keys()
        .chain(current.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut transition_order = Vec::new();
    let mut stable_order = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    let mut actions = Vec::new();
    for claim_id in claim_ids {
        let previous_claim = previous.get(&claim_id).copied();
        let current_claim = current.get(&claim_id).copied();
        let (kind, delta, rationale) =
            classify(previous_claim, current_claim, request.min_change_milli);
        transition_order.push(claim_id.clone());
        if matches!(kind, KnowledgeDriftKind::Stable) {
            stable_order.push(claim_id);
            continue;
        }
        if matches!(kind, KnowledgeDriftKind::Added)
            && current_claim.is_some_and(|claim| {
                matches!(claim.disposition, KnowledgeClaimDisposition::Unresolved)
            })
        {
            unresolved_order.push(claim_id.clone());
            uncertainty.push(format!("{claim_id}: newly added claim is unresolved"));
        }
        if matches!(kind, KnowledgeDriftKind::Removed)
            || matches!(kind, KnowledgeDriftKind::Weakened)
            || matches!(kind, KnowledgeDriftKind::Contradicted)
        {
            negative_evidence.push(format!(
                "{claim_id}: typed knowledge drift is not positive support"
            ));
        }
        let priority = action_priority(kind, delta, previous_claim, current_claim);
        if priority >= request.min_priority_milli && actions.len() < request.max_actions {
            actions.push(KnowledgeDriftAction {
                action_id: format!("knowledge-drift:{claim_id}"),
                claim_id,
                kind,
                previous_confidence_milli: previous_claim.map(|claim| claim.confidence_milli),
                current_confidence_milli: current_claim.map(|claim| claim.confidence_milli),
                previous_disposition: previous_claim.map(|claim| claim.disposition),
                current_disposition: current_claim.map(|claim| claim.disposition),
                delta_milli: delta,
                priority_milli: priority,
                rationale,
            });
        } else {
            uncertainty.push(format!(
                "{}: drift did not clear the autonomous action priority gate",
                claim_id
            ));
        }
    }
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    transition_order.sort();
    stable_order.sort();
    unresolved_order.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = if transition_order.is_empty() {
        KnowledgeDriftDisposition::Blocked
    } else if !actions.is_empty() {
        KnowledgeDriftDisposition::Ready
    } else if !unresolved_order.is_empty() || !uncertainty.is_empty() {
        KnowledgeDriftDisposition::Partial
    } else {
        KnowledgeDriftDisposition::NoMaterialChange
    };
    let next_step = match disposition {
        KnowledgeDriftDisposition::Ready => {
            "route typed-knowledge drift actions to consistency closure, gap compilation, or bounded mechanism replanning"
        }
        KnowledgeDriftDisposition::Partial => {
            "resolve newly unresolved claims and priority-gated drift before autonomous execution"
        }
        KnowledgeDriftDisposition::NoMaterialChange => {
            "retain the current typed world and continue prospective evidence surveillance"
        }
        KnowledgeDriftDisposition::Blocked => {
            "compile non-empty typed knowledge snapshots before attempting drift analysis"
        }
    };
    let mut output = KnowledgeDrift {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        previous_knowledge_digest: request.previous.digest.clone(),
        current_knowledge_digest: request.current.digest.clone(),
        transition_order,
        action_order,
        actions,
        stable_order,
        unresolved_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| KnowledgeDriftError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeDriftError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};

    fn knowledge(
        claim: &str,
        confidence: u16,
        disposition: KnowledgeClaimDisposition,
    ) -> TypedKnowledge {
        let artifact = LocalArtifactRef {
            artifact_id: "artifact-1".into(),
            content_hash: ContentHash::of_value(&serde_json::json!({"claim": claim})).unwrap(),
            content_type: "evidence".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let record = EvidenceRecord {
            evidence_id: "evidence-1".into(),
            source_artifact: artifact,
            source_kind: EvidenceSourceKind::Assay,
            claim: claim.into(),
            scope: "organoid".into(),
            modality: GliomaModality::Transcriptomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: if matches!(disposition, KnowledgeClaimDisposition::Contested) {
                EvidenceState::Contradicted
            } else {
                EvidenceState::Supported
            },
            relevance_milli: confidence,
            quality_milli: confidence,
            reproducibility_milli: confidence,
            release_epoch: 1,
        };
        let mut output = super::super::knowledge_graph::compile_typed_knowledge(
            &super::super::knowledge_graph::KnowledgeRequest {
                objective: "track glioma claim drift".into(),
                required_modalities: BTreeSet::new(),
                required_model_systems: BTreeSet::new(),
                min_support_milli: 0,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &[record],
        )
        .unwrap();
        output.claims[0].confidence_milli = confidence;
        output.claims[0].disposition = disposition;
        output.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": output.feature_id,
            "output_schema": output.output_schema,
            "objective": output.objective,
            "claims": output.claims,
            "claim_order": output.claim_order,
            "top_claim_order": output.top_claim_order,
            "omission_order": output.omission_order,
            "negative_evidence_order": output.negative_evidence_order,
            "uncertainty_order": output.uncertainty_order,
            "disposition": output.disposition,
        }))
        .unwrap();
        output
    }

    #[test]
    fn emits_a_strengthening_action_for_a_material_claim_update() {
        let previous = knowledge(
            "egfr activation increases invasion",
            400,
            KnowledgeClaimDisposition::Supported,
        );
        let current = knowledge(
            "egfr activation increases invasion",
            850,
            KnowledgeClaimDisposition::Supported,
        );
        let output = detect_glioma_knowledge_drift(&KnowledgeDriftRequest {
            objective: "track glioma claim drift".into(),
            previous,
            current,
            min_change_milli: 100,
            min_priority_milli: 100,
            max_actions: 8,
        })
        .expect("knowledge drift");
        assert_eq!(output.disposition, KnowledgeDriftDisposition::Ready);
        assert_eq!(output.actions[0].kind, KnowledgeDriftKind::Strengthened);
        output.validate().expect("digest validates");
    }

    #[test]
    fn preserves_contradiction_as_negative_evidence() {
        let previous = knowledge(
            "egfr activation increases invasion",
            850,
            KnowledgeClaimDisposition::Supported,
        );
        let current = knowledge(
            "egfr activation increases invasion",
            350,
            KnowledgeClaimDisposition::Contested,
        );
        let output = detect_glioma_knowledge_drift(&KnowledgeDriftRequest {
            objective: "track glioma claim drift".into(),
            previous,
            current,
            min_change_milli: 100,
            min_priority_milli: 100,
            max_actions: 8,
        })
        .expect("knowledge drift");
        assert_eq!(output.actions[0].kind, KnowledgeDriftKind::Contradicted);
        assert!(!output.negative_evidence.is_empty());
    }
}
