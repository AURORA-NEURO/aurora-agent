//! Compile typed knowledge gaps into executable P01 acquisition candidates.
//!
//! P02 owns the scientific interpretation of missing coverage, contradiction, uncertainty, and
//! negative results. P01 owns acquisition selection and execution. This compiler is the typed
//! handoff between them: it turns a content-addressed knowledge/frontier pair plus institution
//! source templates into deterministic local candidate records. It never fills a gap, claims a
//! source exists, or promotes a frontier score into evidence.

use super::claim_frontier::{FrontierActionKind, KnowledgeFrontier, KnowledgeFrontierScore};
use super::knowledge_graph::{KnowledgeClaim, TypedKnowledge};
use crate::glioma::programs::p01_evidence_surveillance::{
    EvidenceAcquisitionCandidate, EvidenceAcquisitionSourceKind,
};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F23";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeGapPortfolio1@1";
pub const MAX_TEMPLATES: usize = 256;
pub const MAX_CLAIMS: usize = 4_096;
pub const MAX_CANDIDATES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGapSourceTemplate {
    pub template_id: String,
    pub source_family: String,
    pub source_kind: EvidenceAcquisitionSourceKind,
    pub modality: Option<GliomaModality>,
    pub model_system: Option<GliomaModelSystem>,
    pub cost_units: u64,
    pub reproducibility_milli: u16,
    pub failure_probability_milli: u16,
    pub privacy_risk_milli: u16,
    pub local_only: bool,
    pub contains_human_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGapCompilerRequest {
    pub objective: String,
    pub max_claims: usize,
    pub max_candidates: usize,
    pub max_candidates_per_claim: usize,
    pub max_template_cost_units: u64,
    pub min_frontier_priority_milli: u16,
    pub templates: Vec<KnowledgeGapSourceTemplate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGapClaimMapping {
    pub claim_id: String,
    pub action_kind: FrontierActionKind,
    pub priority_milli: u16,
    pub candidate_order: Vec<String>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeGapPortfolioDisposition {
    Ready,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGapPortfolio {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub frontier_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub candidates: Vec<EvidenceAcquisitionCandidate>,
    pub candidate_order: Vec<String>,
    pub claim_mappings: Vec<KnowledgeGapClaimMapping>,
    pub blocked_template_order: Vec<String>,
    pub omitted_claim_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: KnowledgeGapPortfolioDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeGapCompilerError {
    #[error("knowledge-gap compiler request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge-gap knowledge input is invalid: {0}")]
    InvalidKnowledge(String),
    #[error("knowledge-gap frontier input is invalid: {0}")]
    InvalidFrontier(String),
    #[error("knowledge-gap output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge-gap digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty()) && canonical(values)
}

fn digest_input(output: &KnowledgeGapPortfolio) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "knowledge_digest": output.knowledge_digest,
        "frontier_digest": output.frontier_digest,
        "claim_order": output.claim_order,
        "candidates": output.candidates,
        "candidate_order": output.candidate_order,
        "claim_mappings": output.claim_mappings,
        "blocked_template_order": output.blocked_template_order,
        "omitted_claim_order": output.omitted_claim_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl KnowledgeGapPortfolio {
    pub fn validate(&self) -> Result<(), KnowledgeGapCompilerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.knowledge_digest.as_str().len() != 64
            || self.frontier_digest.as_str().len() != 64
            || !unique_nonempty(&self.claim_order)
            || self.candidates.len() != self.candidate_order.len()
            || !unique_nonempty(&self.candidate_order)
            || !unique_nonempty(&self.blocked_template_order)
            || !unique_nonempty(&self.omitted_claim_order)
            || !unique_nonempty(&self.negative_evidence)
            || !unique_nonempty(&self.uncertainty)
            || self
                .candidates
                .iter()
                .map(|candidate| candidate.candidate_id.clone())
                .collect::<Vec<_>>()
                != self.candidate_order
            || self
                .claim_mappings
                .windows(2)
                .any(|pair| pair[0].claim_id >= pair[1].claim_id)
            || self.claim_mappings.iter().any(|mapping| {
                mapping.claim_id.trim().is_empty()
                    || mapping.priority_milli > 1_000
                    || !canonical(&mapping.candidate_order)
                    || !canonical(&mapping.missing_modality_order)
                    || !canonical(&mapping.missing_model_system_order)
                    || mapping.rationale.trim().is_empty()
                    || mapping
                        .candidate_order
                        .iter()
                        .any(|id| self.candidate_order.binary_search(id).is_err())
            })
        {
            return Err(KnowledgeGapCompilerError::InvalidOutput(
                "identity, candidate ordering, claim mappings, coverage, or digest fields are invalid"
                    .into(),
            ));
        }
        let claims = self.claim_order.iter().collect::<BTreeSet<_>>();
        if self
            .claim_mappings
            .iter()
            .any(|mapping| !claims.contains(&mapping.claim_id))
            || self
                .omitted_claim_order
                .iter()
                .any(|claim_id| !claims.contains(claim_id))
        {
            return Err(KnowledgeGapCompilerError::InvalidOutput(
                "claim mapping and omission identities do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeGapCompilerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeGapCompilerError::InvalidOutput(
                "knowledge-gap portfolio digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &KnowledgeGapCompilerRequest,
) -> Result<(), KnowledgeGapCompilerError> {
    if request.objective.trim().is_empty()
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
        || request.max_candidates == 0
        || request.max_candidates > MAX_CANDIDATES
        || request.max_candidates_per_claim == 0
        || request.max_candidates_per_claim > MAX_TEMPLATES
        || request.max_template_cost_units == 0
        || request.min_frontier_priority_milli > 1_000
        || request.templates.is_empty()
        || request.templates.len() > MAX_TEMPLATES
    {
        return Err(KnowledgeGapCompilerError::InvalidRequest(
            "bounded objective, claim/candidate/template limits, cost ceiling, priority floor, and source templates are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for template in &request.templates {
        if template.template_id.trim().is_empty()
            || template.source_family.trim().is_empty()
            || template.cost_units == 0
            || template.cost_units > request.max_template_cost_units
            || template.reproducibility_milli > 1_000
            || template.failure_probability_milli > 1_000
            || template.privacy_risk_milli > 1_000
            || !ids.insert(template.template_id.clone())
        {
            return Err(KnowledgeGapCompilerError::InvalidRequest(
                "source template identity, family, cost, risk, and score bounds are invalid or duplicated".into(),
            ));
        }
    }
    Ok(())
}

fn find_claim<'a>(knowledge: &'a TypedKnowledge, claim_id: &str) -> Option<&'a KnowledgeClaim> {
    knowledge
        .claims
        .iter()
        .find(|claim| claim.claim_id == claim_id)
}

fn metric_for_action(
    score: &KnowledgeFrontierScore,
    action: FrontierActionKind,
) -> (u16, u16, u16) {
    match action {
        FrontierActionKind::CloseCoverage => (
            score.support_milli,
            score.coverage_debt_milli.max(score.uncertainty_milli),
            score.workflow_leverage_milli,
        ),
        FrontierActionKind::ResolveContradiction => (
            score.support_milli,
            score.uncertainty_milli,
            score.contradiction_milli,
        ),
        FrontierActionKind::ResolveUncertainty => (
            score.support_milli,
            score.uncertainty_milli,
            score.contradiction_milli / 2,
        ),
        FrontierActionKind::RevalidateNegative => {
            (0, score.uncertainty_milli, score.contradiction_milli)
        }
        FrontierActionKind::ValidateSupported => {
            (score.support_milli, score.coverage_debt_milli, 0)
        }
    }
}

fn action_rationale(action: FrontierActionKind) -> &'static str {
    match action {
        FrontierActionKind::CloseCoverage => "close missing modality or model-system coverage",
        FrontierActionKind::ResolveContradiction => {
            "resolve an explicit contradiction with an independent source"
        }
        FrontierActionKind::ResolveUncertainty => "reduce unresolved evidence uncertainty",
        FrontierActionKind::RevalidateNegative => "revalidate a negative result without erasing it",
        FrontierActionKind::ValidateSupported => {
            "independently validate supported evidence before downstream use"
        }
    }
}

fn candidate_id(
    claim_id: &str,
    template_id: &str,
    modality: GliomaModality,
    model_system: Option<GliomaModelSystem>,
) -> String {
    let digest = ContentHash::of_value(&serde_json::json!({
        "claim_id": claim_id,
        "template_id": template_id,
        "modality": modality,
        "model_system": model_system,
    }))
    .expect("small gap candidate identity is hashable");
    format!("gap-{digest}")
}

fn make_candidate(
    claim: &KnowledgeClaim,
    score: &KnowledgeFrontierScore,
    template: &KnowledgeGapSourceTemplate,
    modality: GliomaModality,
    model_system: Option<GliomaModelSystem>,
) -> EvidenceAcquisitionCandidate {
    let (support, uncertainty, contradiction) = metric_for_action(score, score.action_kind);
    let target_claim = format!("{} [{}]", claim.statement.trim(), claim.scope.trim());
    let id = candidate_id(
        &claim.claim_id,
        &template.template_id,
        modality,
        model_system,
    );
    EvidenceAcquisitionCandidate {
        candidate_id: id,
        target_claim,
        source_family: template.source_family.clone(),
        source_kind: template.source_kind,
        modality,
        model_system,
        independence_group: template.template_id.clone(),
        depends_on: Vec::new(),
        cost_units: template.cost_units,
        expected_support_milli: support,
        expected_uncertainty_reduction_milli: uncertainty,
        contradiction_resolution_milli: contradiction,
        freshness_milli: score.priority_milli,
        workflow_leverage_milli: score.workflow_leverage_milli,
        reproducibility_milli: template.reproducibility_milli,
        failure_probability_milli: template.failure_probability_milli,
        privacy_risk_milli: template.privacy_risk_milli,
        local_only: template.local_only,
        contains_human_data: template.contains_human_data,
    }
}

/// Compile P02 frontier debt into P01 acquisition candidates. The returned candidates are ready
/// for `plan_glioma_evidence_acquisition`; this function itself performs no selection or effect.
pub fn compile_glioma_knowledge_gaps(
    request: &KnowledgeGapCompilerRequest,
    knowledge: &TypedKnowledge,
    frontier: &KnowledgeFrontier,
) -> Result<KnowledgeGapPortfolio, KnowledgeGapCompilerError> {
    validate_request(request)?;
    knowledge
        .validate()
        .map_err(|error| KnowledgeGapCompilerError::InvalidKnowledge(error.to_string()))?;
    frontier
        .validate()
        .map_err(|error| KnowledgeGapCompilerError::InvalidFrontier(error.to_string()))?;
    if frontier.objective != request.objective || frontier.knowledge_digest != knowledge.digest {
        return Err(KnowledgeGapCompilerError::InvalidFrontier(
            "frontier objective or knowledge digest does not match compiler input".into(),
        ));
    }
    let mut claim_order = frontier
        .ranking
        .iter()
        .filter(|score| score.priority_milli >= request.min_frontier_priority_milli)
        .take(request.max_claims)
        .map(|score| score.claim_id.clone())
        .collect::<Vec<_>>();
    claim_order.sort();
    claim_order.dedup();
    let mut candidates = BTreeMap::<String, EvidenceAcquisitionCandidate>::new();
    let mut mappings = BTreeMap::<String, KnowledgeGapClaimMapping>::new();
    let mut blocked_templates = BTreeSet::new();
    let mut omitted_claims = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for score in &frontier.ranking {
        if !claim_order.binary_search(&score.claim_id).is_ok()
            || score.priority_milli < request.min_frontier_priority_milli
        {
            continue;
        }
        let Some(claim) = find_claim(knowledge, &score.claim_id) else {
            omitted_claims.insert(score.claim_id.clone());
            uncertainty.insert(format!("{}:claim-not-found-in-knowledge", score.claim_id));
            continue;
        };
        let mut missing_modalities = claim.missing_modality_order.clone();
        let mut missing_models = claim.missing_model_system_order.clone();
        if missing_modalities.is_empty() {
            missing_modalities.push(
                claim
                    .modality_order
                    .first()
                    .copied()
                    .unwrap_or(GliomaModality::Genomics),
            );
        }
        if missing_models.is_empty() {
            missing_models.push(
                claim
                    .model_system_order
                    .first()
                    .copied()
                    .unwrap_or(GliomaModelSystem::Organoid),
            );
        }
        missing_modalities.sort();
        missing_modalities.dedup();
        missing_models.sort();
        missing_models.dedup();
        let mut mapping_candidates = BTreeSet::new();
        for template in &request.templates {
            if template.contains_human_data || !template.local_only {
                blocked_templates.insert(template.template_id.clone());
                negative_evidence.insert(format!(
                    "{}:{}-policy-blocked",
                    score.claim_id, template.template_id
                ));
                continue;
            }
            let mut generated_for_template = 0;
            let model_options = template
                .model_system
                .map(|model| vec![model])
                .unwrap_or_else(|| missing_models.clone());
            for modality in &missing_modalities {
                for model_system in &model_options {
                    if generated_for_template >= request.max_candidates_per_claim {
                        break;
                    }
                    if template.modality.is_some() && template.modality != Some(*modality) {
                        continue;
                    }
                    if template.model_system.is_some() && !missing_models.contains(model_system) {
                        continue;
                    }
                    let candidate =
                        make_candidate(claim, score, template, *modality, Some(*model_system));
                    mapping_candidates.insert(candidate.candidate_id.clone());
                    candidates
                        .entry(candidate.candidate_id.clone())
                        .or_insert(candidate);
                    generated_for_template += 1;
                }
            }
        }
        if mapping_candidates.is_empty() {
            omitted_claims.insert(score.claim_id.clone());
            uncertainty.insert(format!(
                "{}:no-local-source-template-covers-gap",
                score.claim_id
            ));
        } else {
            mappings.insert(
                score.claim_id.clone(),
                KnowledgeGapClaimMapping {
                    claim_id: score.claim_id.clone(),
                    action_kind: score.action_kind,
                    priority_milli: score.priority_milli,
                    candidate_order: mapping_candidates.into_iter().collect(),
                    missing_modality_order: missing_modalities,
                    missing_model_system_order: missing_models,
                    rationale: action_rationale(score.action_kind).into(),
                },
            );
        }
    }
    let mut candidate_values = candidates.into_values().collect::<Vec<_>>();
    candidate_values.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    if candidate_values.len() > request.max_candidates {
        let omitted = candidate_values.split_off(request.max_candidates);
        for candidate in omitted {
            uncertainty.insert(format!("{}:candidate-cap-deferred", candidate.candidate_id));
        }
    }
    let candidate_order = candidate_values
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let claim_mappings = mappings
        .into_values()
        .filter_map(|mut mapping| {
            mapping
                .candidate_order
                .retain(|id| candidate_order.binary_search(id).is_ok());
            if mapping.candidate_order.is_empty() {
                omitted_claims.insert(mapping.claim_id.clone());
                None
            } else {
                Some(mapping)
            }
        })
        .collect::<Vec<_>>();
    let disposition = if candidate_values.is_empty() && !blocked_templates.is_empty() {
        KnowledgeGapPortfolioDisposition::Blocked
    } else if candidate_values.is_empty() {
        KnowledgeGapPortfolioDisposition::Unresolved
    } else if !omitted_claims.is_empty() || !blocked_templates.is_empty() {
        KnowledgeGapPortfolioDisposition::Partial
    } else {
        KnowledgeGapPortfolioDisposition::Ready
    };
    let mut output = KnowledgeGapPortfolio {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: knowledge.digest.clone(),
        frontier_digest: frontier.digest.clone(),
        claim_order,
        candidates: candidate_values,
        candidate_order,
        claim_mappings,
        blocked_template_order: blocked_templates.into_iter().collect(),
        omitted_claim_order: omitted_claims.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-knowledge-gap-portfolio"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeGapCompilerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::claim_frontier::{
        prioritize_knowledge_frontier, KnowledgeFrontierRequest,
    };
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        compile_typed_knowledge, KnowledgeRequest,
    };
    use crate::glioma_engine::LocalArtifactRef;

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

    fn knowledge_and_frontier() -> (TypedKnowledge, KnowledgeFrontier) {
        let records = vec![EvidenceRecord {
            evidence_id: "egfr-paper".into(),
            source_artifact: artifact("egfr-artifact"),
            source_kind: EvidenceSourceKind::Literature,
            claim: "EGFR signaling increases invasion".into(),
            scope: "organoid invasion".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 800,
            release_epoch: 2,
        }];
        let knowledge = compile_typed_knowledge(
            &KnowledgeRequest {
                objective: "close glioma evidence debt".into(),
                required_modalities: [GliomaModality::Genomics, GliomaModality::Imaging]
                    .into_iter()
                    .collect(),
                required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
                min_support_milli: 100,
                min_sources_per_claim: 1,
                max_claims: 8,
            },
            &records,
        )
        .unwrap();
        let frontier = prioritize_knowledge_frontier(
            &KnowledgeFrontierRequest {
                objective: "close glioma evidence debt".into(),
                max_selected_claims: 4,
                min_priority_milli: 0,
                weights: Default::default(),
            },
            &knowledge,
        )
        .unwrap();
        (knowledge, frontier)
    }

    fn request() -> KnowledgeGapCompilerRequest {
        KnowledgeGapCompilerRequest {
            objective: "close glioma evidence debt".into(),
            max_claims: 8,
            max_candidates: 16,
            max_candidates_per_claim: 8,
            max_template_cost_units: 10,
            min_frontier_priority_milli: 0,
            templates: vec![
                KnowledgeGapSourceTemplate {
                    template_id: "imaging-atlas".into(),
                    source_family: "atlas".into(),
                    source_kind: EvidenceAcquisitionSourceKind::Dataset,
                    modality: Some(GliomaModality::Imaging),
                    model_system: Some(GliomaModelSystem::Organoid),
                    cost_units: 2,
                    reproducibility_milli: 800,
                    failure_probability_milli: 100,
                    privacy_risk_milli: 50,
                    local_only: true,
                    contains_human_data: false,
                },
                KnowledgeGapSourceTemplate {
                    template_id: "replication-site".into(),
                    source_family: "consortium".into(),
                    source_kind: EvidenceAcquisitionSourceKind::Replication,
                    modality: None,
                    model_system: None,
                    cost_units: 3,
                    reproducibility_milli: 900,
                    failure_probability_milli: 150,
                    privacy_risk_milli: 50,
                    local_only: true,
                    contains_human_data: false,
                },
            ],
        }
    }

    #[test]
    fn compiler_emits_p01_candidates_for_missing_modalities() {
        let (knowledge, frontier) = knowledge_and_frontier();
        let output = compile_glioma_knowledge_gaps(&request(), &knowledge, &frontier).unwrap();
        assert!(!output.candidates.is_empty());
        assert!(output
            .candidates
            .iter()
            .any(|candidate| candidate.modality == GliomaModality::Imaging));
        assert_eq!(output.knowledge_digest, knowledge.digest);
        output.validate().unwrap();
    }

    #[test]
    fn compiler_is_permutation_stable_and_blocks_nonlocal_templates() {
        let (knowledge, frontier) = knowledge_and_frontier();
        let mut request = request();
        request.templates.reverse();
        request.templates.push(KnowledgeGapSourceTemplate {
            template_id: "remote-human".into(),
            source_family: "clinical".into(),
            source_kind: EvidenceAcquisitionSourceKind::Dataset,
            modality: Some(GliomaModality::Imaging),
            model_system: Some(GliomaModelSystem::Organoid),
            cost_units: 1,
            reproducibility_milli: 900,
            failure_probability_milli: 100,
            privacy_risk_milli: 900,
            local_only: false,
            contains_human_data: true,
        });
        let first = compile_glioma_knowledge_gaps(&request, &knowledge, &frontier).unwrap();
        request.templates.reverse();
        let second = compile_glioma_knowledge_gaps(&request, &knowledge, &frontier).unwrap();
        assert_eq!(first, second);
        assert!(first
            .blocked_template_order
            .contains(&"remote-human".into()));
        assert!(first
            .negative_evidence
            .iter()
            .any(|item| item.contains("remote-human-policy-blocked")));
    }

    #[test]
    fn compiler_refuses_frontier_from_a_different_knowledge_digest() {
        let (knowledge, frontier) = knowledge_and_frontier();
        let mut altered = knowledge.clone();
        altered.objective = "different objective".into();
        let error = compile_glioma_knowledge_gaps(&request(), &altered, &frontier).unwrap_err();
        assert!(matches!(
            error,
            KnowledgeGapCompilerError::InvalidKnowledge(_)
        ));
    }
}
