//! End-to-end decision-context orchestration for preclinical glioma research.
//!
//! P04 owns the transition from typed scientific state to executable work. This feature composes
//! the existing context compiler, dependency graph, robust branch planner, and local campaign
//! executor into one replayable cycle. It does not infer missing evidence, treat a scenario as an
//! observation, or widen authority beyond the caller-owned executor.

use super::action_graph::{
    compile_decision_action_graph, DecisionActionGraph, DecisionActionGraphError,
    DecisionActionGraphRequest,
};
use super::branch_planner::{
    plan_glioma_decision_branches, DecisionBranchPlan, DecisionBranchPlannerError,
    DecisionBranchPlannerRequest,
};
use super::campaign::{
    execute_glioma_decision_context_campaign, DecisionContextCampaign,
    DecisionContextCampaignError, DecisionContextCampaignExecutor, DecisionContextCampaignRequest,
};
use super::context_compiler::{
    compile_decision_context, DecisionContext, DecisionContextError, DecisionContextRequest,
};
use crate::glioma::programs::p02_evidence_knowledge::{
    compile_typed_knowledge, KnowledgeComposition, KnowledgeCompositionError, KnowledgeRequest,
    TypedKnowledge,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaDecisionOperatingCycle1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOperatingCycleRequest {
    pub knowledge: KnowledgeRequest,
    pub context: DecisionContextRequest,
    pub graph: DecisionActionGraphRequest,
    pub branches: DecisionBranchPlannerRequest,
    pub composition: KnowledgeComposition,
    pub records: Vec<crate::glioma::evidence::EvidenceRecord>,
    pub action_plan: super::action_bridge::DecisionActionPlanRequest,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOperatingCycleDisposition {
    Qualified,
    Partial,
    BudgetBlocked,
    Failed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOperatingCycle {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub phase_order: Vec<String>,
    pub knowledge: TypedKnowledge,
    pub context: DecisionContext,
    pub action_graph: DecisionActionGraph,
    pub branch_plan: DecisionBranchPlan,
    pub campaign: DecisionContextCampaign,
    pub simulation_only: bool,
    pub disposition: DecisionOperatingCycleDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecisionOperatingCycleError {
    #[error("decision operating-cycle request is invalid: {0}")]
    InvalidRequest(String),
    #[error("decision operating-cycle knowledge compilation failed: {0}")]
    Knowledge(String),
    #[error("decision operating-cycle context compilation failed: {0}")]
    Context(#[from] DecisionContextError),
    #[error("decision operating-cycle action graph failed: {0}")]
    Graph(#[from] DecisionActionGraphError),
    #[error("decision operating-cycle branch planning failed: {0}")]
    Branch(#[from] DecisionBranchPlannerError),
    #[error("decision operating-cycle composition failed: {0}")]
    Composition(#[from] KnowledgeCompositionError),
    #[error("decision operating-cycle campaign failed: {0}")]
    Campaign(#[from] DecisionContextCampaignError),
    #[error("decision operating-cycle output is invalid: {0}")]
    InvalidOutput(String),
    #[error("decision operating-cycle digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &DecisionOperatingCycle) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "phase_order": output.phase_order,
        "knowledge": output.knowledge,
        "context": output.context,
        "action_graph": output.action_graph,
        "branch_plan": output.branch_plan,
        "campaign": output.campaign,
        "simulation_only": output.simulation_only,
        "disposition": output.disposition,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
    })
}

impl DecisionOperatingCycle {
    pub fn validate(&self) -> Result<(), DecisionOperatingCycleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.phase_order
                != [
                    "knowledge_compilation".to_string(),
                    "decision_context".to_string(),
                    "action_graph".to_string(),
                    "robust_branch_planning".to_string(),
                    "decision_campaign".to_string(),
                ]
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.knowledge.objective != self.objective
            || self.context.objective != self.objective
            || self.action_graph.objective != self.objective
            || self.branch_plan.objective != self.objective
            || self.campaign.objective != self.objective
            || self.simulation_only != self.campaign.simulation_only
            || self.action_graph.context_digest != self.context.digest
            || self.branch_plan.context_digest != self.context.digest
        {
            return Err(DecisionOperatingCycleError::InvalidOutput(
                "identity, phase order, objective, nested bindings, or ordering is invalid".into(),
            ));
        }
        self.knowledge
            .validate()
            .map_err(|error| DecisionOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.context
            .validate()
            .map_err(|error| DecisionOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.action_graph
            .validate()
            .map_err(|error| DecisionOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.branch_plan
            .validate()
            .map_err(|error| DecisionOperatingCycleError::InvalidOutput(error.to_string()))?;
        self.campaign
            .validate()
            .map_err(|error| DecisionOperatingCycleError::InvalidOutput(error.to_string()))?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| DecisionOperatingCycleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(DecisionOperatingCycleError::InvalidOutput(
                "decision operating-cycle digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn normalize_branch_request(
    request: &DecisionBranchPlannerRequest,
    context: &DecisionContext,
) -> DecisionBranchPlannerRequest {
    let mut normalized = request.clone();
    if normalized.candidates.is_empty() {
        normalized.candidates = context
            .actions
            .iter()
            .map(|action| action.candidate.clone())
            .collect();
    }
    if normalized.scenarios.is_empty() {
        let baseline = normalized
            .candidates
            .iter()
            .map(|candidate| super::branch_planner::DecisionScenarioOutcome {
                action_id: candidate.action_id.clone(),
                value_milli: 500,
                uncertainty_milli: 500,
                failure_probability_milli: 100,
            })
            .collect();
        let stress = normalized
            .candidates
            .iter()
            .map(|candidate| super::branch_planner::DecisionScenarioOutcome {
                action_id: candidate.action_id.clone(),
                value_milli: 250,
                uncertainty_milli: 750,
                failure_probability_milli: 300,
            })
            .collect();
        normalized.scenarios = vec![
            super::branch_planner::DecisionScenario {
                scenario_id: "baseline-declared".into(),
                probability_milli: 700,
                outcomes: baseline,
            },
            super::branch_planner::DecisionScenario {
                scenario_id: "stress-declared".into(),
                probability_milli: 300,
                outcomes: stress,
            },
        ];
    }
    normalized
}

fn disposition(
    campaign: super::campaign::DecisionContextCampaignDisposition,
    branch: super::branch_planner::DecisionBranchPlanDisposition,
) -> DecisionOperatingCycleDisposition {
    if matches!(
        campaign,
        super::campaign::DecisionContextCampaignDisposition::BudgetBlocked
    ) || matches!(
        branch,
        super::branch_planner::DecisionBranchPlanDisposition::BudgetBlocked
    ) {
        DecisionOperatingCycleDisposition::BudgetBlocked
    } else {
        match campaign {
            super::campaign::DecisionContextCampaignDisposition::Qualified => {
                DecisionOperatingCycleDisposition::Qualified
            }
            super::campaign::DecisionContextCampaignDisposition::Failed => {
                DecisionOperatingCycleDisposition::Failed
            }
            super::campaign::DecisionContextCampaignDisposition::Partial => {
                DecisionOperatingCycleDisposition::Partial
            }
            super::campaign::DecisionContextCampaignDisposition::NoActions
            | super::campaign::DecisionContextCampaignDisposition::Unresolved => {
                DecisionOperatingCycleDisposition::Unresolved
            }
            super::campaign::DecisionContextCampaignDisposition::BudgetBlocked => {
                DecisionOperatingCycleDisposition::BudgetBlocked
            }
        }
    }
}

/// Compile knowledge into a context, dependency graph, robust branch portfolio, and executable
/// decision campaign. MCP supplies a deterministic metadata-only executor; local Rust callers can
/// supply a governed preclinical adapter.
pub fn execute_glioma_decision_operating_cycle<E: DecisionContextCampaignExecutor>(
    request: &DecisionOperatingCycleRequest,
    executor: &mut E,
) -> Result<DecisionOperatingCycle, DecisionOperatingCycleError> {
    if request.knowledge.objective != request.context.objective
        || request.context.objective != request.graph.objective
        || request.graph.objective != request.branches.objective
        || request.branches.objective != request.action_plan.objective
        || request.action_plan.objective != request.composition.objective
        || request.budget_units == 0
        || request.max_rounds == 0
    {
        return Err(DecisionOperatingCycleError::InvalidRequest(
            "all objectives must match and budget/round bounds must be positive".into(),
        ));
    }
    let knowledge = compile_typed_knowledge(&request.knowledge, &request.records)
        .map_err(|error| DecisionOperatingCycleError::Knowledge(error.to_string()))?;
    if request.composition.knowledge_digest != knowledge.digest {
        return Err(DecisionOperatingCycleError::InvalidRequest(
            "composition is not bound to the compiled knowledge digest".into(),
        ));
    }
    let context = compile_decision_context(&request.context, &knowledge)?;
    let action_graph =
        compile_decision_action_graph(&request.graph, &context, &request.composition)?;
    let branches = normalize_branch_request(&request.branches, &context);
    let branch_plan = plan_glioma_decision_branches(&branches, &context)?;
    let campaign_request = DecisionContextCampaignRequest {
        knowledge: request.knowledge.clone(),
        context: request.context.clone(),
        action_plan: request.action_plan.clone(),
        records: request.records.clone(),
        budget_units: request.budget_units,
        max_rounds: request.max_rounds,
        max_retries: request.max_retries,
        stop_on_qualified: request.stop_on_qualified,
    };
    let campaign = execute_glioma_decision_context_campaign(&campaign_request, executor)?;
    let mut negative_evidence = context.negative_evidence_order.clone();
    negative_evidence.extend(action_graph.negative_evidence_order.iter().cloned());
    negative_evidence.extend(branch_plan.negative_evidence_order.iter().cloned());
    negative_evidence.extend(campaign.negative_evidence.iter().cloned());
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = context.uncertainty_order.clone();
    uncertainty.extend(action_graph.uncertainty_order.iter().cloned());
    uncertainty.extend(branch_plan.uncertainty_order.iter().cloned());
    uncertainty.extend(campaign.uncertainty.iter().cloned());
    uncertainty.sort();
    uncertainty.dedup();
    let cycle_disposition = disposition(campaign.disposition, branch_plan.disposition);
    let mut output = DecisionOperatingCycle {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.context.objective.clone(),
        phase_order: vec![
            "knowledge_compilation".into(),
            "decision_context".into(),
            "action_graph".into(),
            "robust_branch_planning".into(),
            "decision_campaign".into(),
        ],
        knowledge,
        context,
        action_graph,
        branch_plan,
        simulation_only: campaign.simulation_only,
        disposition: cycle_disposition,
        campaign,
        negative_evidence,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-decision-operating-cycle"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| DecisionOperatingCycleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
    use crate::glioma::programs::p02_evidence_knowledge::{
        compose_knowledge_graph, KnowledgeCompositionRequest, KnowledgeRelation,
    };
    use crate::glioma::programs::p04_decision_context::{
        DecisionActionGraphRequest, DecisionBranchPlannerRequest, DecisionContextRequest,
        DryRunDecisionContextCampaignExecutor,
    };
    use crate::glioma_engine::{
        GliomaModality, GliomaModelSystem, GliomaSelectionConfig, LocalArtifactRef,
    };

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

    fn request() -> DecisionOperatingCycleRequest {
        let objective = "resolve glioma invasion mechanism".to_string();
        let records = vec![EvidenceRecord {
            evidence_id: "record-1".into(),
            source_artifact: artifact("record-artifact"),
            source_kind: EvidenceSourceKind::Literature,
            claim: "EGFR signaling increases invasion".into(),
            scope: "organoid invasion".into(),
            modality: GliomaModality::Genomics,
            model_system: Some(GliomaModelSystem::Organoid),
            state: EvidenceState::Supported,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 850,
            release_epoch: 1,
        }];
        let knowledge = KnowledgeRequest {
            objective: objective.clone(),
            required_modalities: [GliomaModality::Genomics, GliomaModality::Imaging]
                .into_iter()
                .collect(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            min_support_milli: 100,
            min_sources_per_claim: 1,
            max_claims: 8,
        };
        let typed = compile_typed_knowledge(&knowledge, &records).unwrap();
        let composition = compose_knowledge_graph(
            &KnowledgeCompositionRequest {
                objective: objective.clone(),
                min_path_length: 2,
                max_paths: 8,
                min_strength_milli: 0,
                max_contradiction_milli: 1_000,
                require_supported_root: false,
            },
            &typed,
            &Vec::<KnowledgeRelation>::new(),
        )
        .unwrap();
        let selection = GliomaSelectionConfig {
            budget_units: 8,
            max_actions: 4,
            approval_granted: true,
            allow_instrument_execution: false,
            allow_federation: false,
            weights: Default::default(),
        };
        DecisionOperatingCycleRequest {
            knowledge,
            context: DecisionContextRequest {
                objective: objective.clone(),
                max_actions: 8,
                default_cost_units: 2,
            },
            graph: DecisionActionGraphRequest {
                objective: objective.clone(),
                max_nodes: 8,
                max_waves: 8,
                budget_units: 8,
                require_qualified_composition: false,
            },
            branches: DecisionBranchPlannerRequest {
                objective: objective.clone(),
                candidates: Vec::new(),
                completed_action_order: Vec::new(),
                scenarios: Vec::new(),
                budget_units: 8,
                max_actions_per_branch: 2,
                max_branches: 4,
                beam_width: 8,
                minimum_robustness_milli: -1_000_000,
                uncertainty_penalty_milli: 100,
                failure_penalty_milli: 100,
                selection_weights: Default::default(),
            },
            composition,
            records,
            action_plan: super::super::action_bridge::DecisionActionPlanRequest {
                objective,
                completed_action_order: Vec::new(),
                selection,
            },
            budget_units: 8,
            max_rounds: 2,
            max_retries: 1,
            stop_on_qualified: true,
        }
    }

    #[test]
    fn cycle_compiles_graph_branches_and_executes_local_campaign() {
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let output = execute_glioma_decision_operating_cycle(&request(), &mut executor).unwrap();
        assert!(output.simulation_only);
        assert_eq!(output.phase_order.len(), 5);
        assert!(!output.context.actions.is_empty());
        assert_eq!(output.action_graph.context_digest, output.context.digest);
        assert!(output.branch_plan.portfolios.len() <= 4);
        output.validate().unwrap();
    }

    #[test]
    fn cycle_refuses_composition_from_a_different_knowledge_snapshot() {
        let mut request = request();
        request.composition.knowledge_digest = ContentHash::of_bytes(b"different");
        let mut executor = DryRunDecisionContextCampaignExecutor;
        let error = execute_glioma_decision_operating_cycle(&request, &mut executor).unwrap_err();
        assert!(matches!(
            error,
            DecisionOperatingCycleError::InvalidRequest(_)
        ));
    }
}
