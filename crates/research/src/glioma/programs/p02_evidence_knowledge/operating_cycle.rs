//! Closed-loop typed-knowledge synthesis for autonomous preclinical glioma research.
//!
//! This feature composes the P02 scientific reasoning stages into one replayable product
//! operation. Local evidence is compiled into scoped claims, explicit claim relations are
//! composed into usable paths, typed conflicts are resolved into a bounded competing-belief
//! portfolio, and the resulting frontier is compiled into P01 acquisition candidates. The
//! operation never infers a conflict from prose, fills missing measurements, or promotes an
//! unresolved claim; every omission and negative result remains attached to the next action.

use super::belief_revision::{
    revise_glioma_beliefs, BeliefConflict, BeliefRevision, BeliefRevisionDisposition,
    BeliefRevisionError, BeliefRevisionRequest,
};
use super::claim_frontier::{
    prioritize_knowledge_frontier, KnowledgeFrontier, KnowledgeFrontierDisposition,
    KnowledgeFrontierError, KnowledgeFrontierRequest,
};
use super::composition::{
    compose_knowledge_graph, KnowledgeComposition, KnowledgeCompositionDisposition,
    KnowledgeCompositionError, KnowledgeCompositionRequest, KnowledgeRelation,
};
use super::gap_compiler::{
    compile_glioma_knowledge_gaps, KnowledgeGapCompilerError, KnowledgeGapCompilerRequest,
    KnowledgeGapPortfolio, KnowledgeGapPortfolioDisposition,
};
use super::knowledge_graph::{
    compile_typed_knowledge, KnowledgeError, KnowledgeRequest, TypedKnowledge,
};
use crate::glioma::evidence::EvidenceRecord;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeSynthesisOperatingCycle1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeSynthesisOperatingCycleRequest {
    pub knowledge: KnowledgeRequest,
    pub records: Vec<EvidenceRecord>,
    pub composition: KnowledgeCompositionRequest,
    pub relations: Vec<KnowledgeRelation>,
    pub revision: BeliefRevisionRequest,
    pub conflicts: Vec<BeliefConflict>,
    pub frontier: KnowledgeFrontierRequest,
    pub gap: KnowledgeGapCompilerRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeSynthesisOperatingCycleDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeSynthesisOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub knowledge: TypedKnowledge,
    pub composition: KnowledgeComposition,
    pub revision: BeliefRevision,
    pub frontier: KnowledgeFrontier,
    pub gap_portfolio: KnowledgeGapPortfolio,
    pub next_action: String,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: KnowledgeSynthesisOperatingCycleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeSynthesisOperatingCycleError {
    #[error("knowledge synthesis cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge synthesis compilation failed: {0}")]
    Compilation(#[from] KnowledgeError),
    #[error("knowledge synthesis composition failed: {0}")]
    Composition(#[from] KnowledgeCompositionError),
    #[error("knowledge synthesis belief revision failed: {0}")]
    Revision(#[from] BeliefRevisionError),
    #[error("knowledge synthesis frontier failed: {0}")]
    Frontier(#[from] KnowledgeFrontierError),
    #[error("knowledge synthesis gap compilation failed: {0}")]
    Gap(#[from] KnowledgeGapCompilerError),
    #[error("knowledge synthesis output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge synthesis digest failed: {0}")]
    Digest(String),
}

fn phase_order() -> Vec<String> {
    [
        "typed_knowledge_compilation",
        "claim_path_composition",
        "belief_revision",
        "frontier_prioritization",
        "acquisition_gap_compilation",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn disposition(
    knowledge: &TypedKnowledge,
    composition: &KnowledgeComposition,
    revision: &BeliefRevision,
    frontier: &KnowledgeFrontier,
    gap: &KnowledgeGapPortfolio,
) -> KnowledgeSynthesisOperatingCycleDisposition {
    if matches!(gap.disposition, KnowledgeGapPortfolioDisposition::Blocked) {
        return KnowledgeSynthesisOperatingCycleDisposition::Blocked;
    }
    if matches!(
        knowledge.disposition,
        super::knowledge_graph::KnowledgeDisposition::Unresolved
    ) || matches!(
        frontier.disposition,
        KnowledgeFrontierDisposition::NoClaims | KnowledgeFrontierDisposition::Unresolved
    ) {
        return KnowledgeSynthesisOperatingCycleDisposition::Unresolved;
    }
    if matches!(
        gap.disposition,
        KnowledgeGapPortfolioDisposition::Unresolved
    ) || matches!(
        composition.disposition,
        KnowledgeCompositionDisposition::Unresolved
    ) || matches!(revision.disposition, BeliefRevisionDisposition::Unresolved)
    {
        return KnowledgeSynthesisOperatingCycleDisposition::Unresolved;
    }
    if matches!(gap.disposition, KnowledgeGapPortfolioDisposition::Partial)
        || matches!(
            composition.disposition,
            KnowledgeCompositionDisposition::Partial
        )
        || matches!(revision.disposition, BeliefRevisionDisposition::Contested)
    {
        return KnowledgeSynthesisOperatingCycleDisposition::Partial;
    }
    KnowledgeSynthesisOperatingCycleDisposition::Ready
}

fn next_action(
    gap: &KnowledgeGapPortfolio,
    frontier: &KnowledgeFrontier,
    composition: &KnowledgeComposition,
) -> String {
    if let Some(candidate) = gap.candidate_order.first() {
        return format!("dispatch P01 local acquisition candidate {candidate}");
    }
    if let Some(claim) = frontier.selected_order.first() {
        return format!("compile a typed local action for frontier claim {claim}");
    }
    if let Some(claim) = composition.bottleneck_claim_order.first() {
        return format!("resolve composition bottleneck claim {claim}");
    }
    "hold: collect typed evidence for the unresolved knowledge frontier".into()
}

fn digest_input(output: &KnowledgeSynthesisOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "knowledge": output.knowledge,
        "composition": output.composition,
        "revision": output.revision,
        "frontier": output.frontier,
        "gap_portfolio": output.gap_portfolio,
        "next_action": output.next_action,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl KnowledgeSynthesisOperatingCycle {
    pub fn validate(&self) -> Result<(), KnowledgeSynthesisOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order != phase_order()
            || self.next_action.trim().is_empty()
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.knowledge.objective != self.objective
            || self.composition.objective != self.objective
            || self.revision.objective != self.objective
            || self.frontier.objective != self.objective
            || self.gap_portfolio.objective != self.objective
        {
            return Err(KnowledgeSynthesisOperatingCycleError::InvalidOutput(
                "identity, phase order, objective binding, action, or canonical evidence fields are invalid".into(),
            ));
        }
        self.knowledge.validate().map_err(|error| {
            KnowledgeSynthesisOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        self.composition.validate().map_err(|error| {
            KnowledgeSynthesisOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        self.revision.validate().map_err(|error| {
            KnowledgeSynthesisOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        self.frontier.validate().map_err(|error| {
            KnowledgeSynthesisOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        self.gap_portfolio.validate().map_err(|error| {
            KnowledgeSynthesisOperatingCycleError::InvalidOutput(error.to_string())
        })?;
        if self.digest.as_str().len() != 64 {
            return Err(KnowledgeSynthesisOperatingCycleError::InvalidOutput(
                "knowledge synthesis digest must be a 64-character content hash".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeSynthesisOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeSynthesisOperatingCycleError::InvalidOutput(
                "knowledge synthesis digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Run the complete P02 synthesis path and emit the next P01 acquisition handoff.
pub fn execute_glioma_knowledge_synthesis_operating_cycle(
    request: &KnowledgeSynthesisOperatingCycleRequest,
) -> Result<KnowledgeSynthesisOperatingCycle, KnowledgeSynthesisOperatingCycleError> {
    let objective = request.knowledge.objective.trim();
    if objective.is_empty()
        || request.composition.objective != objective
        || request.revision.objective != objective
        || request.frontier.objective != objective
        || request.gap.objective != objective
    {
        return Err(KnowledgeSynthesisOperatingCycleError::InvalidRequest(
            "all P02 stage objectives must match the knowledge objective".into(),
        ));
    }
    let knowledge = compile_typed_knowledge(&request.knowledge, &request.records)?;
    let composition =
        compose_knowledge_graph(&request.composition, &knowledge, &request.relations)?;
    let revision = revise_glioma_beliefs(&request.revision, &knowledge, &request.conflicts)?;
    let frontier = prioritize_knowledge_frontier(&request.frontier, &knowledge)?;
    let gap_portfolio = compile_glioma_knowledge_gaps(&request.gap, &knowledge, &frontier)?;

    let mut negative_evidence = knowledge.negative_evidence_order.clone();
    negative_evidence.extend(composition.negative_evidence_order.clone());
    negative_evidence.extend(revision.negative_evidence.clone());
    negative_evidence.extend(frontier.negative_evidence_order.clone());
    negative_evidence.extend(gap_portfolio.negative_evidence.clone());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = knowledge.uncertainty_order.clone();
    uncertainty.extend(composition.uncertainty_order.clone());
    uncertainty.extend(revision.uncertainty.clone());
    uncertainty.extend(frontier.uncertainty_order.clone());
    uncertainty.extend(gap_portfolio.uncertainty.clone());
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = KnowledgeSynthesisOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: objective.into(),
        phase_order: phase_order(),
        next_action: next_action(&gap_portfolio, &frontier, &composition),
        disposition: disposition(
            &knowledge,
            &composition,
            &revision,
            &frontier,
            &gap_portfolio,
        ),
        knowledge,
        composition,
        revision,
        frontier,
        gap_portfolio,
        negative_evidence,
        uncertainty,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeSynthesisOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::gap_compiler::KnowledgeGapSourceTemplate;
    use super::*;
    use crate::glioma::evidence::{EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p01_evidence_surveillance::EvidenceAcquisitionSourceKind;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
    use std::collections::BTreeSet;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> KnowledgeSynthesisOperatingCycleRequest {
        let objective = "resolve organoid invasion evidence".to_string();
        let mut modalities = BTreeSet::new();
        modalities.insert(GliomaModality::Genomics);
        let mut models = BTreeSet::new();
        models.insert(GliomaModelSystem::Organoid);
        let knowledge = KnowledgeRequest {
            objective: objective.clone(),
            required_modalities: modalities,
            required_model_systems: models,
            min_support_milli: 500,
            min_sources_per_claim: 1,
            max_claims: 8,
        };
        let records = vec![EvidenceRecord {
            evidence_id: "evidence-1".into(),
            source_artifact: artifact("evidence-1"),
            source_kind: EvidenceSourceKind::Assay,
            claim: "EGFR signaling increases invasion".into(),
            scope: "organoid".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 850,
            release_epoch: 1,
        }];
        let composition = KnowledgeCompositionRequest {
            objective: objective.clone(),
            min_path_length: 2,
            max_paths: 8,
            min_strength_milli: 0,
            max_contradiction_milli: 1_000,
            require_supported_root: false,
        };
        let revision = BeliefRevisionRequest {
            objective: objective.clone(),
            min_support_milli: 0,
            min_conflict_milli: 1,
            max_hypotheses: 1,
            beam_width: 8,
            allow_contested: true,
        };
        let frontier = KnowledgeFrontierRequest {
            objective: objective.clone(),
            max_selected_claims: 4,
            min_priority_milli: 0,
            weights: Default::default(),
        };
        let gap = KnowledgeGapCompilerRequest {
            objective: objective.clone(),
            max_claims: 8,
            max_candidates: 8,
            max_candidates_per_claim: 4,
            max_template_cost_units: 4,
            min_frontier_priority_milli: 0,
            templates: vec![KnowledgeGapSourceTemplate {
                template_id: "local-assay".into(),
                source_family: "local-assay".into(),
                source_kind: EvidenceAcquisitionSourceKind::Assay,
                modality: Some(GliomaModality::Genomics),
                model_system: Some(GliomaModelSystem::Organoid),
                cost_units: 1,
                reproducibility_milli: 800,
                failure_probability_milli: 100,
                privacy_risk_milli: 0,
                local_only: true,
                contains_human_data: false,
            }],
        };
        KnowledgeSynthesisOperatingCycleRequest {
            knowledge,
            records,
            composition,
            relations: Vec::new(),
            revision,
            conflicts: Vec::new(),
            frontier,
            gap,
        }
    }

    #[test]
    fn synthesis_cycle_replays_and_emits_next_acquisition_action() {
        let request = request();
        let output = execute_glioma_knowledge_synthesis_operating_cycle(&request).expect("cycle");
        output.validate().expect("valid output");
        assert_eq!(output.phase_order.len(), 5);
        assert_eq!(output.knowledge.claim_order.len(), 1);
        assert!(output.next_action.starts_with("dispatch P01"));
        let replay = execute_glioma_knowledge_synthesis_operating_cycle(&request).expect("replay");
        assert_eq!(output.digest, replay.digest);
    }
}
