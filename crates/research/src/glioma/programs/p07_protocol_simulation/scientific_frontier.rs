//! Scientific frontier orchestration for autonomous preclinical glioma research.
//!
//! P07 already contains the bounded action selector, while P02 and P03 independently describe
//! what is known and which modalities are usable. This feature is the decision layer between
//! them: it admits only actions whose scientific prerequisites are currently open, passes the
//! admitted portfolio through the dependency-aware selector, and returns an executable next
//! batch plus explicit holds for every action that is premature. It is deliberately a research
//! scheduler, not a claim generator, treatment recommender, or physical-instrument bypass.

use crate::glioma::programs::p02_evidence_knowledge::{
    KnowledgeFrontier, KnowledgeFrontierDisposition, TypedKnowledge,
};
use crate::glioma::programs::p03_multimodal_ingestion_qc::{
    MultimodalResearchReadiness, MultimodalResearchReadinessDisposition, MultimodalResearchSurface,
};
use crate::glioma_engine::{
    select_glioma_actions, GliomaActionCandidate, GliomaActionSelection, GliomaEngineError,
    GliomaSelectionConfig, GliomaStageKind,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaScientificFrontier1@1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificFrontierRequest {
    pub objective: String,
    pub knowledge: TypedKnowledge,
    pub frontier: KnowledgeFrontier,
    pub readiness: MultimodalResearchReadiness,
    pub candidates: Vec<GliomaActionCandidate>,
    pub completed_action_order: Vec<String>,
    pub selection: GliomaSelectionConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierCandidateStatus {
    Admitted,
    Held,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontierCandidateGate {
    pub action_id: String,
    pub stage_kind: GliomaStageKind,
    pub status: FrontierCandidateStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScientificFrontierDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificFrontierPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub frontier_digest: ContentHash,
    pub readiness_digest: ContentHash,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub held_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub gates: Vec<FrontierCandidateGate>,
    pub selection: Option<GliomaActionSelection>,
    pub next_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ScientificFrontierDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScientificFrontierError {
    #[error("scientific frontier request is invalid: {0}")]
    InvalidRequest(String),
    #[error("scientific frontier knowledge is invalid: {0}")]
    Knowledge(String),
    #[error("scientific frontier selection failed: {0}")]
    Selection(#[from] GliomaEngineError),
    #[error("scientific frontier output is invalid: {0}")]
    InvalidOutput(String),
    #[error("scientific frontier digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn readiness_allows(stage: GliomaStageKind, readiness: &MultimodalResearchReadiness) -> bool {
    let admitted = &readiness.admitted_surface_order;
    match stage {
        GliomaStageKind::IntentNormalization
        | GliomaStageKind::EvidenceSurveillance
        | GliomaStageKind::EvidenceCompilation
        | GliomaStageKind::MultimodalIngestionQc => true,
        GliomaStageKind::MolecularLandscape | GliomaStageKind::MechanismExploration => {
            admitted.contains(&MultimodalResearchSurface::Mechanism)
        }
        GliomaStageKind::ExperimentDesign | GliomaStageKind::ProtocolSimulation => {
            admitted.contains(&MultimodalResearchSurface::ExperimentDesign)
        }
        GliomaStageKind::InstrumentPreflight => {
            admitted.contains(&MultimodalResearchSurface::ExperimentDesign)
        }
        GliomaStageKind::ComputationalExecution | GliomaStageKind::StatisticalInterpretation => {
            admitted.contains(&MultimodalResearchSurface::Analysis)
        }
        GliomaStageKind::ReplicationRobustness => {
            admitted.contains(&MultimodalResearchSurface::Replication)
        }
        GliomaStageKind::ResearchObjectRelease => {
            admitted.contains(&MultimodalResearchSurface::Publication)
        }
        GliomaStageKind::FederationBenchmarking => {
            admitted.contains(&MultimodalResearchSurface::Publication)
        }
    }
}

fn knowledge_allows(stage: GliomaStageKind, knowledge: &TypedKnowledge) -> bool {
    match knowledge.disposition {
        crate::glioma::programs::p02_evidence_knowledge::KnowledgeDisposition::Qualified => true,
        crate::glioma::programs::p02_evidence_knowledge::KnowledgeDisposition::Partial => matches!(
            stage,
            GliomaStageKind::IntentNormalization
                | GliomaStageKind::EvidenceSurveillance
                | GliomaStageKind::EvidenceCompilation
                | GliomaStageKind::MultimodalIngestionQc
        ),
        crate::glioma::programs::p02_evidence_knowledge::KnowledgeDisposition::Unresolved => {
            matches!(
                stage,
                GliomaStageKind::IntentNormalization
                    | GliomaStageKind::EvidenceSurveillance
                    | GliomaStageKind::EvidenceCompilation
                    | GliomaStageKind::MultimodalIngestionQc
            )
        }
    }
}

fn gate_candidate(
    candidate: &GliomaActionCandidate,
    knowledge: &TypedKnowledge,
    readiness: &MultimodalResearchReadiness,
    completed: &BTreeSet<String>,
) -> FrontierCandidateGate {
    if completed.contains(&candidate.action_id) {
        return FrontierCandidateGate {
            action_id: candidate.action_id.clone(),
            stage_kind: candidate.stage_kind,
            status: FrontierCandidateStatus::Blocked,
            reason: "already-completed".into(),
        };
    }
    if !knowledge_allows(candidate.stage_kind, knowledge) {
        return FrontierCandidateGate {
            action_id: candidate.action_id.clone(),
            stage_kind: candidate.stage_kind,
            status: FrontierCandidateStatus::Held,
            reason: "typed knowledge is partial or unresolved for this downstream stage".into(),
        };
    }
    if !readiness_allows(candidate.stage_kind, readiness) {
        return FrontierCandidateGate {
            action_id: candidate.action_id.clone(),
            stage_kind: candidate.stage_kind,
            status: FrontierCandidateStatus::Held,
            reason: "required multimodal research surface is not admitted".into(),
        };
    }
    FrontierCandidateGate {
        action_id: candidate.action_id.clone(),
        stage_kind: candidate.stage_kind,
        status: FrontierCandidateStatus::Admitted,
        reason:
            "scientific prerequisites are admitted; downstream selector retains policy authority"
                .into(),
    }
}

fn digest_input(plan: &ScientificFrontierPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "knowledge_digest": plan.knowledge_digest,
        "frontier_digest": plan.frontier_digest,
        "readiness_digest": plan.readiness_digest,
        "candidate_order": plan.candidate_order,
        "admitted_order": plan.admitted_order,
        "held_order": plan.held_order,
        "blocked_order": plan.blocked_order,
        "gates": plan.gates,
        "selection": plan.selection,
        "next_action_order": plan.next_action_order,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl ScientificFrontierPlan {
    pub fn validate(&self) -> Result<(), ScientificFrontierError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.candidate_order.is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.admitted_order)
            || !canonical(&self.held_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.next_action_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.gates.len() != self.candidate_order.len()
            || self
                .gates
                .windows(2)
                .any(|pair| pair[0].action_id >= pair[1].action_id)
            || self.knowledge_digest.as_str().len() != 64
            || self.frontier_digest.as_str().len() != 64
            || self.readiness_digest.as_str().len() != 64
        {
            return Err(ScientificFrontierError::InvalidOutput(
                "identity, candidate/gate order, evidence order, or digest fields are invalid"
                    .into(),
            ));
        }
        if self.admitted_order.iter().any(|id| {
            self.held_order.binary_search(id).is_ok()
                || self.blocked_order.binary_search(id).is_ok()
        }) || self
            .held_order
            .iter()
            .any(|id| self.blocked_order.binary_search(id).is_ok())
        {
            return Err(ScientificFrontierError::InvalidOutput(
                "candidate gate partitions overlap".into(),
            ));
        }
        let all = self
            .admitted_order
            .iter()
            .chain(self.held_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if all.len() != self.candidate_order.len()
            || all
                .iter()
                .any(|id| self.candidate_order.binary_search(id).is_err())
        {
            return Err(ScientificFrontierError::InvalidOutput(
                "candidate gate partitions do not cover the candidate set".into(),
            ));
        }
        if let Some(selection) = &self.selection {
            selection
                .validate()
                .map_err(|error| ScientificFrontierError::InvalidOutput(error.to_string()))?;
            if selection
                .candidate_order
                .iter()
                .any(|id| self.admitted_order.binary_search(id).is_err())
            {
                return Err(ScientificFrontierError::InvalidOutput(
                    "selector received a non-admitted candidate".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ScientificFrontierError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ScientificFrontierError::InvalidOutput(
                "scientific frontier digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Admit and select the next research batch from typed knowledge, multimodal readiness, and a
/// caller-provided candidate pool. Only the final selector output is executable; held candidates
/// remain visible with a concrete scientific reason.
pub fn plan_glioma_scientific_frontier(
    request: &ScientificFrontierRequest,
) -> Result<ScientificFrontierPlan, ScientificFrontierError> {
    if request.objective.trim().is_empty()
        || request.objective != request.knowledge.objective
        || request.objective != request.frontier.objective
        || request.candidates.is_empty()
        || request
            .completed_action_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.selection.max_actions == 0
        || request.selection.budget_units == 0
        || request.selection.weights.information_gain
            + request.selection.weights.frontier_novelty
            + request.selection.weights.workflow_leverage
            + request.selection.weights.cross_stage_unlock
            + request.selection.weights.reproducibility_safety
            + request.selection.weights.federation_value
            + request.selection.weights.feasibility
            != 100
        || request.selection.max_actions > 64
    {
        return Err(ScientificFrontierError::InvalidRequest(
            "objective binding, candidate pool, completed order, budget, action bound, and selection weights are required".into(),
        ));
    }
    request
        .knowledge
        .validate()
        .map_err(|error| ScientificFrontierError::Knowledge(error.to_string()))?;
    request
        .frontier
        .validate()
        .map_err(|error| ScientificFrontierError::Knowledge(error.to_string()))?;
    request
        .readiness
        .validate()
        .map_err(|error| ScientificFrontierError::Knowledge(error.to_string()))?;
    if request.frontier.knowledge_digest != request.knowledge.digest {
        return Err(ScientificFrontierError::InvalidRequest(
            "knowledge frontier is bound to a different typed-knowledge digest".into(),
        ));
    }
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut gates = request
        .candidates
        .iter()
        .map(|candidate| {
            gate_candidate(
                candidate,
                &request.knowledge,
                &request.readiness,
                &completed,
            )
        })
        .collect::<Vec<_>>();
    gates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    if gates
        .windows(2)
        .any(|pair| pair[0].action_id == pair[1].action_id)
    {
        return Err(ScientificFrontierError::InvalidRequest(
            "candidate action identifiers must be unique".into(),
        ));
    }
    let candidate_order = gates
        .iter()
        .map(|gate| gate.action_id.clone())
        .collect::<Vec<_>>();
    let mut admitted_order = gates
        .iter()
        .filter(|gate| matches!(gate.status, FrontierCandidateStatus::Admitted))
        .map(|gate| gate.action_id.clone())
        .collect::<Vec<_>>();
    let held_order = gates
        .iter()
        .filter(|gate| matches!(gate.status, FrontierCandidateStatus::Held))
        .map(|gate| gate.action_id.clone())
        .collect::<Vec<_>>();
    let blocked_order = gates
        .iter()
        .filter(|gate| matches!(gate.status, FrontierCandidateStatus::Blocked))
        .map(|gate| gate.action_id.clone())
        .collect::<Vec<_>>();
    admitted_order.sort();
    let admitted = request
        .candidates
        .iter()
        .filter(|candidate| admitted_order.binary_search(&candidate.action_id).is_ok())
        .cloned()
        .collect::<Vec<_>>();
    let selection = if admitted.is_empty() {
        None
    } else {
        Some(select_glioma_actions(
            &admitted,
            &completed,
            &request.selection,
        )?)
    };
    let mut next_action_order = selection
        .as_ref()
        .map(|selection| selection.selected_order.clone())
        .unwrap_or_default();
    if next_action_order.is_empty() {
        next_action_order = request.readiness.next_action_order.clone();
    }
    next_action_order.sort();
    next_action_order.dedup();
    let mut negative_evidence = request.knowledge.negative_evidence_order.clone();
    negative_evidence.extend(request.frontier.negative_evidence_order.clone());
    negative_evidence.extend(request.readiness.negative_evidence.clone());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = request.knowledge.uncertainty_order.clone();
    uncertainty.extend(request.frontier.uncertainty_order.clone());
    uncertainty.extend(request.readiness.uncertainty.clone());
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if admitted.is_empty() {
        if matches!(
            request.readiness.disposition,
            MultimodalResearchReadinessDisposition::Blocked
                | MultimodalResearchReadinessDisposition::Unresolved
        ) || matches!(
            request.frontier.disposition,
            KnowledgeFrontierDisposition::Unresolved
        ) {
            ScientificFrontierDisposition::Unresolved
        } else {
            ScientificFrontierDisposition::Blocked
        }
    } else if selection
        .as_ref()
        .is_some_and(|selection| selection.selected_order.is_empty())
    {
        ScientificFrontierDisposition::Partial
    } else if held_order.is_empty() && blocked_order.is_empty() {
        ScientificFrontierDisposition::Ready
    } else {
        ScientificFrontierDisposition::Partial
    };
    let mut output = ScientificFrontierPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: request.knowledge.digest.clone(),
        frontier_digest: request.frontier.digest.clone(),
        readiness_digest: request.readiness.digest.clone(),
        candidate_order,
        admitted_order,
        held_order,
        blocked_order,
        gates,
        selection,
        next_action_order,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ScientificFrontierError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}
