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
    GliomaActionCandidate, GliomaActionSelection, GliomaEngineError, GliomaSelectionConfig,
    GliomaStageKind, select_glioma_actions,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaScientificFrontier2@1";

/// Explicit research question(s) an executable action is meant to resolve. Claimless candidates
/// are reserved for the evidence/ingestion bootstrap stages that create the initial claim base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScientificFrontierCandidateClaimLink {
    pub action_id: String,
    pub claim_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScientificFrontierRequest {
    pub objective: String,
    pub knowledge: TypedKnowledge,
    pub frontier: KnowledgeFrontier,
    pub readiness: MultimodalResearchReadiness,
    pub candidates: Vec<GliomaActionCandidate>,
    /// One canonical action-to-claim link per candidate, including an explicit empty link only
    /// for evidence and multimodal-ingestion bootstrap actions.
    pub candidate_claim_links: Vec<ScientificFrontierCandidateClaimLink>,
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
    pub linked_claim_order: Vec<String>,
    pub claim_priority_milli: u16,
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

fn is_frontier_bootstrap_stage(stage: GliomaStageKind) -> bool {
    matches!(
        stage,
        GliomaStageKind::IntentNormalization
            | GliomaStageKind::EvidenceSurveillance
            | GliomaStageKind::EvidenceCompilation
            | GliomaStageKind::MultimodalIngestionQc
    )
}

fn linked_claim_priority(
    claim_order: &[String],
    priority_by_claim: &std::collections::BTreeMap<String, u16>,
) -> u16 {
    if claim_order.is_empty() {
        return 0;
    }
    let sum = claim_order
        .iter()
        .filter_map(|claim_id| priority_by_claim.get(claim_id))
        .map(|priority| u32::from(*priority))
        .sum::<u32>();
    ((sum + (claim_order.len() as u32 / 2)) / claim_order.len() as u32).min(1_000) as u16
}

fn gate_candidate(
    candidate: &GliomaActionCandidate,
    linked_claim_order: &[String],
    priority_by_claim: &std::collections::BTreeMap<String, u16>,
    frontier: &KnowledgeFrontier,
    knowledge: &TypedKnowledge,
    readiness: &MultimodalResearchReadiness,
    completed: &BTreeSet<String>,
) -> FrontierCandidateGate {
    let claim_priority_milli = linked_claim_priority(linked_claim_order, priority_by_claim);
    if completed.contains(&candidate.action_id) {
        return FrontierCandidateGate {
            action_id: candidate.action_id.clone(),
            stage_kind: candidate.stage_kind,
            linked_claim_order: linked_claim_order.to_vec(),
            claim_priority_milli,
            status: FrontierCandidateStatus::Blocked,
            reason: "already-completed".into(),
        };
    }
    if !linked_claim_order.is_empty()
        && linked_claim_order
            .iter()
            .any(|claim_id| frontier.selected_order.binary_search(claim_id).is_err())
    {
        return FrontierCandidateGate {
            action_id: candidate.action_id.clone(),
            stage_kind: candidate.stage_kind,
            linked_claim_order: linked_claim_order.to_vec(),
            claim_priority_milli,
            status: FrontierCandidateStatus::Held,
            reason: "one or more linked claims are outside the selected P02 knowledge frontier"
                .into(),
        };
    }
    if !knowledge_allows(candidate.stage_kind, knowledge) {
        return FrontierCandidateGate {
            action_id: candidate.action_id.clone(),
            stage_kind: candidate.stage_kind,
            linked_claim_order: linked_claim_order.to_vec(),
            claim_priority_milli,
            status: FrontierCandidateStatus::Held,
            reason: "typed knowledge is partial or unresolved for this downstream stage".into(),
        };
    }
    if !readiness_allows(candidate.stage_kind, readiness) {
        return FrontierCandidateGate {
            action_id: candidate.action_id.clone(),
            stage_kind: candidate.stage_kind,
            linked_claim_order: linked_claim_order.to_vec(),
            claim_priority_milli,
            status: FrontierCandidateStatus::Held,
            reason: "required multimodal research surface is not admitted".into(),
        };
    }
    FrontierCandidateGate {
        action_id: candidate.action_id.clone(),
        stage_kind: candidate.stage_kind,
        linked_claim_order: linked_claim_order.to_vec(),
        claim_priority_milli,
        status: FrontierCandidateStatus::Admitted,
        reason: if linked_claim_order.is_empty() {
            "claim-generating evidence or ingestion bootstrap action; downstream selector retains policy authority".into()
        } else {
            format!(
                "linked P02 claims are selected; mean priority {claim_priority_milli} is blended with candidate information gain"
            )
        },
    }
}

pub(super) fn admitted_candidates_with_frontier_priority(
    candidates: &[GliomaActionCandidate],
    gates: &[FrontierCandidateGate],
) -> Vec<GliomaActionCandidate> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let gate = gates
                .binary_search_by(|gate| gate.action_id.cmp(&candidate.action_id))
                .ok()
                .map(|index| &gates[index])?;
            if gate.status != FrontierCandidateStatus::Admitted {
                return None;
            }
            let mut adjusted = candidate.clone();
            if !gate.linked_claim_order.is_empty() {
                adjusted.information_gain_milli = ((u32::from(adjusted.information_gain_milli)
                    + u32::from(gate.claim_priority_milli)
                    + 1)
                    / 2) as u16;
            }
            Some(adjusted)
        })
        .collect()
}

/// Return a deterministic topological order for the action graph. Dependency admission can then
/// propagate once from prerequisites to descendants instead of rescanning the entire pool until a
/// fixed point.
fn dependency_order(
    candidates: &[GliomaActionCandidate],
    completed: &BTreeSet<String>,
) -> Option<Vec<String>> {
    let mut indegree = BTreeMap::<String, usize>::new();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for candidate in candidates {
        let dependencies = candidate
            .depends_on
            .iter()
            .filter(|dependency| !completed.contains(*dependency))
            .collect::<Vec<_>>();
        indegree.insert(candidate.action_id.clone(), dependencies.len());
        for dependency in dependencies {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(candidate.action_id.clone());
        }
    }
    for children in dependents.values_mut() {
        children.sort();
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(action_id, _)| action_id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(candidates.len());
    while let Some(action_id) = ready.iter().next().cloned() {
        ready.remove(&action_id);
        order.push(action_id.clone());
        for child in dependents.get(&action_id).into_iter().flatten() {
            let degree = indegree.get_mut(child)?;
            *degree = degree.checked_sub(1)?;
            if *degree == 0 {
                ready.insert(child.clone());
            }
        }
    }
    (order.len() == candidates.len()).then_some(order)
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
            || self.gates.iter().any(|gate| {
                !canonical(&gate.linked_claim_order)
                    || gate.claim_priority_milli > 1_000
                    || gate.reason.trim().is_empty()
            })
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
    let weight_total = u32::from(request.selection.weights.information_gain)
        + u32::from(request.selection.weights.frontier_novelty)
        + u32::from(request.selection.weights.workflow_leverage)
        + u32::from(request.selection.weights.cross_stage_unlock)
        + u32::from(request.selection.weights.reproducibility_safety)
        + u32::from(request.selection.weights.federation_value)
        + u32::from(request.selection.weights.feasibility);
    let candidate_ids = request
        .candidates
        .iter()
        .map(|candidate| candidate.action_id.clone())
        .collect::<BTreeSet<_>>();
    let candidate_by_id = request
        .candidates
        .iter()
        .map(|candidate| (candidate.action_id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let link_ids = request
        .candidate_claim_links
        .iter()
        .map(|link| link.action_id.clone())
        .collect::<BTreeSet<_>>();
    if request.objective.trim().is_empty()
        || request.objective != request.knowledge.objective
        || request.objective != request.frontier.objective
        || request.candidates.is_empty()
        || request
            .completed_action_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.candidate_claim_links.len() != request.candidates.len()
        || candidate_ids.len() != request.candidates.len()
        || link_ids.len() != request.candidate_claim_links.len()
        || link_ids != candidate_ids
        || request
            .candidate_claim_links
            .windows(2)
            .any(|pair| pair[0].action_id >= pair[1].action_id)
        || request.candidate_claim_links.iter().any(|link| {
            !canonical(&link.claim_order)
                || (link.claim_order.is_empty()
                    && candidate_by_id
                        .get(link.action_id.as_str())
                        .map_or(true, |candidate| {
                            !is_frontier_bootstrap_stage(candidate.stage_kind)
                        }))
        })
        || request.selection.max_actions == 0
        || request.selection.budget_units == 0
        || weight_total != 100
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
    let knowledge_claims = request
        .knowledge
        .claim_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let frontier_scores = request
        .frontier
        .ranking
        .iter()
        .map(|score| (score.claim_id.clone(), score.priority_milli))
        .collect::<std::collections::BTreeMap<_, _>>();
    if request.candidate_claim_links.iter().any(|link| {
        link.claim_order.iter().any(|claim_id| {
            !knowledge_claims.contains(claim_id) || !frontier_scores.contains_key(claim_id)
        })
    }) {
        return Err(ScientificFrontierError::InvalidRequest(
            "every linked action claim must exist in typed knowledge and the P02 ranked frontier"
                .into(),
        ));
    }
    let completed = request
        .completed_action_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request.candidates.iter().any(|candidate| {
        candidate.action_id.trim().is_empty()
            || !candidate
                .depends_on
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            || candidate.depends_on.iter().any(|dependency| {
                dependency == &candidate.action_id
                    || (!candidate_ids.contains(dependency) && !completed.contains(dependency))
            })
    }) {
        return Err(ScientificFrontierError::InvalidRequest(
            "candidate identities and dependencies must be canonical and closed over candidates or completed actions".into(),
        ));
    }
    let ordered_actions = dependency_order(&request.candidates, &completed).ok_or_else(|| {
        ScientificFrontierError::InvalidRequest("candidate dependency graph must be acyclic".into())
    })?;
    let link_by_action = request
        .candidate_claim_links
        .iter()
        .map(|link| (link.action_id.clone(), link))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut gates = request
        .candidates
        .iter()
        .map(|candidate| {
            let link = link_by_action
                .get(&candidate.action_id)
                .expect("request validation binds one claim link per candidate");
            gate_candidate(
                candidate,
                &link.claim_order,
                &frontier_scores,
                &request.frontier,
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
    let gate_index = gates
        .iter()
        .enumerate()
        .map(|(index, gate)| (gate.action_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for action_id in ordered_actions {
        let Some(candidate) = candidate_by_id.get(action_id.as_str()).copied() else {
            continue;
        };
        let gate_position = *gate_index
            .get(&action_id)
            .expect("each candidate has exactly one scientific gate");
        if gates[gate_position].status != FrontierCandidateStatus::Admitted {
            continue;
        }
        if let Some(dependency) = candidate.depends_on.iter().find(|dependency| {
            !completed.contains(*dependency)
                && gate_index
                    .get(*dependency)
                    .map(|index| gates[*index].status != FrontierCandidateStatus::Admitted)
                    .unwrap_or(true)
        }) {
            gates[gate_position].status = FrontierCandidateStatus::Held;
            gates[gate_position].reason = format!(
                "dependency {dependency} is not admitted by the current scientific frontier"
            );
        }
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
    let admitted = admitted_candidates_with_frontier_priority(&request.candidates, &gates);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{
        select_glioma_actions, GliomaModality, GliomaModelSystem, GliomaSelectionWeights,
    };
    use bioprism_foundation::{AutonomyTier, Effect};

    fn candidate(action_id: &str, information_gain_milli: u16) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: action_id.into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            modality: GliomaModality::Genomics,
            model_system: GliomaModelSystem::Organoid,
            depends_on: Vec::new(),
            cost_units: 1,
            information_gain_milli,
            frontier_novelty_milli: 500,
            workflow_leverage_milli: 500,
            cross_stage_unlock_milli: 500,
            reproducibility_safety_milli: 900,
            federation_value_milli: 100,
            feasibility_milli: 900,
            autonomy_tier: AutonomyTier::A1,
            effects: BTreeSet::from([
                Effect::ReadLocalData,
                Effect::ExecuteLocalComputation,
                Effect::WriteLocalArtifact,
            ]),
        }
    }

    fn gate(action_id: &str, claim_id: &str, priority_milli: u16) -> FrontierCandidateGate {
        FrontierCandidateGate {
            action_id: action_id.into(),
            stage_kind: GliomaStageKind::MechanismExploration,
            linked_claim_order: vec![claim_id.into()],
            claim_priority_milli: priority_milli,
            status: FrontierCandidateStatus::Admitted,
            reason: "test admission".into(),
        }
    }

    #[test]
    fn selected_claim_priority_changes_the_executable_action_ranking() {
        let candidates = vec![candidate("action-a", 100), candidate("action-b", 900)];
        let gates = vec![
            gate("action-a", "claim-high", 1_000),
            gate("action-b", "claim-low", 0),
        ];
        let adjusted = admitted_candidates_with_frontier_priority(&candidates, &gates);
        let by_id = adjusted
            .iter()
            .map(|candidate| {
                (
                    candidate.action_id.as_str(),
                    candidate.information_gain_milli,
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(by_id.get("action-a"), Some(&550));
        assert_eq!(by_id.get("action-b"), Some(&450));

        let selection = select_glioma_actions(
            &adjusted,
            &BTreeSet::new(),
            &GliomaSelectionConfig {
                budget_units: 2,
                max_actions: 1,
                weights: GliomaSelectionWeights {
                    information_gain: 100,
                    frontier_novelty: 0,
                    workflow_leverage: 0,
                    cross_stage_unlock: 0,
                    reproducibility_safety: 0,
                    federation_value: 0,
                    feasibility: 0,
                },
                ..GliomaSelectionConfig::default()
            },
        )
        .unwrap();
        assert_eq!(selection.selected_order, vec!["action-a"]);
    }

    #[test]
    fn held_actions_are_never_forwarded_to_the_portfolio_selector() {
        let candidates = vec![
            candidate("action-admitted", 500),
            candidate("action-held", 900),
        ];
        let gates = vec![
            gate("action-admitted", "claim-a", 700),
            FrontierCandidateGate {
                status: FrontierCandidateStatus::Held,
                reason: "claim is outside selected frontier".into(),
                ..gate("action-held", "claim-b", 900)
            },
        ];
        let admitted = admitted_candidates_with_frontier_priority(&candidates, &gates);
        assert_eq!(
            admitted
                .iter()
                .map(|candidate| candidate.action_id.as_str())
                .collect::<Vec<_>>(),
            vec!["action-admitted"]
        );
    }

    #[test]
    fn dependency_graph_orders_prerequisites_and_rejects_cycles() {
        let parent = candidate("action-parent", 500);
        let mut child = candidate("action-child", 500);
        child.depends_on = vec![parent.action_id.clone()];
        assert_eq!(
            dependency_order(&[child.clone(), parent.clone()], &BTreeSet::new()).unwrap(),
            vec![parent.action_id.clone(), child.action_id.clone()]
        );

        let mut cyclic_parent = parent;
        cyclic_parent.depends_on = vec![child.action_id.clone()];
        assert!(dependency_order(&[child, cyclic_parent], &BTreeSet::new()).is_none());
    }
}
