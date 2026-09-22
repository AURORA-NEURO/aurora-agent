//! Evidence-frontier to research-action compilation for preclinical glioma.
//!
//! Typed knowledge and its frontier are only useful to an autonomous research engine when they
//! become executable, bounded work. This feature compiles frontier claims into dependency-closed
//! validation, replication, coverage, contradiction-resolution, and negative-result actions. It
//! never invents evidence or executes biology: every action retains the claim binding, expected
//! information, cost, risk, model/modality requirements, and the reason it was selected, deferred,
//! or blocked.

use super::claim_frontier::{FrontierActionKind, KnowledgeFrontier};
use super::knowledge_graph::{KnowledgeClaimDisposition, TypedKnowledge};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaKnowledgeActionCompiler1@1";
pub const MAX_TEMPLATES: usize = 16_384;
pub const MAX_SELECTED_ACTIONS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionCompilerRequest {
    pub objective: String,
    pub budget_milli: u64,
    pub risk_budget_milli: u64,
    pub max_selected_actions: usize,
    pub min_information_gain_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionTemplate {
    pub template_id: String,
    pub claim_id: String,
    pub action_kind: FrontierActionKind,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub information_gain_milli: u16,
    pub cost_milli: u64,
    pub risk_milli: u64,
    pub dependency_order: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledActionDisposition {
    Selected,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledResearchAction {
    pub action_id: String,
    pub claim_id: String,
    pub action_kind: FrontierActionKind,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub information_gain_milli: u16,
    pub cost_milli: u64,
    pub risk_milli: u64,
    pub dependency_order: Vec<String>,
    pub priority_milli: u16,
    pub disposition: CompiledActionDisposition,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeActionPlanDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeActionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub knowledge_digest: ContentHash,
    pub frontier_digest: ContentHash,
    pub action_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub actions: Vec<CompiledResearchAction>,
    pub selected_cost_milli: u64,
    pub selected_risk_milli: u64,
    pub selected_information_milli: u64,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: KnowledgeActionPlanDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeActionCompilerError {
    #[error("knowledge action compiler request is invalid: {0}")]
    InvalidRequest(String),
    #[error("knowledge action template is invalid: {0}")]
    InvalidTemplate(String),
    #[error("knowledge action compiler input is invalid: {0}")]
    InvalidInput(String),
    #[error("knowledge action compiler output is invalid: {0}")]
    InvalidOutput(String),
    #[error("knowledge action compiler digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn frontier_kind_matches(left: FrontierActionKind, right: FrontierActionKind) -> bool {
    left == right
}

pub(crate) fn digest_input(output: &KnowledgeActionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "knowledge_digest": output.knowledge_digest,
        "frontier_digest": output.frontier_digest,
        "action_order": output.action_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "blocked_order": output.blocked_order,
        "actions": output.actions,
        "selected_cost_milli": output.selected_cost_milli,
        "selected_risk_milli": output.selected_risk_milli,
        "selected_information_milli": output.selected_information_milli,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
    })
}

fn score(priority_milli: u16, information_gain_milli: u16, cost_milli: u64) -> u16 {
    if cost_milli == 0 {
        return 0;
    }
    (u128::from(priority_milli)
        .saturating_mul(u128::from(information_gain_milli))
        .saturating_mul(1_000)
        .checked_div(u128::from(cost_milli).min(u128::from(u64::MAX)))
        .unwrap_or_default()
        .min(1_000)) as u16
}

fn frontier_priority(frontier: &KnowledgeFrontier) -> BTreeMap<String, (u16, FrontierActionKind)> {
    frontier
        .ranking
        .iter()
        .map(|entry| {
            (
                entry.claim_id.clone(),
                (entry.priority_milli, entry.action_kind),
            )
        })
        .collect()
}

fn claim_disposition(
    knowledge: &TypedKnowledge,
    claim_id: &str,
) -> Option<KnowledgeClaimDisposition> {
    knowledge
        .claims
        .iter()
        .find(|claim| claim.claim_id == claim_id)
        .map(|claim| claim.disposition)
}

fn cycle_nodes(
    id: &str,
    templates: &BTreeMap<String, &KnowledgeActionTemplate>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    cycles: &mut BTreeSet<String>,
) {
    if visited.contains(id) {
        return;
    }
    if !visiting.insert(id.to_string()) {
        cycles.insert(id.to_string());
        return;
    }
    if let Some(template) = templates.get(id) {
        for dependency in &template.dependency_order {
            if !templates.contains_key(dependency) {
                continue;
            }
            cycle_nodes(dependency, templates, visiting, visited, cycles);
            if cycles.contains(dependency) {
                cycles.insert(id.to_string());
            }
        }
    }
    visiting.remove(id);
    visited.insert(id.to_string());
}

fn dependency_closure(
    id: &str,
    templates: &BTreeMap<String, &KnowledgeActionTemplate>,
    blocked: &BTreeSet<String>,
    out: &mut BTreeSet<String>,
) -> bool {
    if blocked.contains(id) {
        return false;
    }
    if !out.insert(id.to_string()) {
        return true;
    }
    let Some(template) = templates.get(id) else {
        return false;
    };
    template
        .dependency_order
        .iter()
        .all(|dependency| dependency_closure(dependency, templates, blocked, out))
}

fn validate_request(
    request: &KnowledgeActionCompilerRequest,
    knowledge: &TypedKnowledge,
    frontier: &KnowledgeFrontier,
    templates: &[KnowledgeActionTemplate],
) -> Result<(), KnowledgeActionCompilerError> {
    if request.objective.trim().is_empty()
        || request.budget_milli == 0
        || request.risk_budget_milli == 0
        || request.max_selected_actions == 0
        || request.max_selected_actions > MAX_SELECTED_ACTIONS
        || request.min_information_gain_milli == 0
        || templates.is_empty()
        || templates.len() > MAX_TEMPLATES
        || knowledge.objective != request.objective
        || frontier.objective != request.objective
    {
        return Err(KnowledgeActionCompilerError::InvalidRequest(
            "objective, bounded budgets, action cap, non-empty templates, and matching knowledge/frontier objectives are required".into(),
        ));
    }
    knowledge
        .validate()
        .map_err(|error| KnowledgeActionCompilerError::InvalidInput(error.to_string()))?;
    frontier
        .validate()
        .map_err(|error| KnowledgeActionCompilerError::InvalidInput(error.to_string()))?;
    let mut ids = BTreeSet::new();
    for template in templates {
        if template.template_id.trim().is_empty()
            || !ids.insert(template.template_id.clone())
            || template.claim_id.trim().is_empty()
            || template.modality_order.is_empty()
            || template.model_system_order.is_empty()
            || !canonical(&template.modality_order)
            || !canonical(&template.model_system_order)
            || !canonical(&template.dependency_order)
            || template.information_gain_milli < request.min_information_gain_milli
            || template.information_gain_milli > 1_000
            || template.cost_milli == 0
            || template.risk_milli > request.risk_budget_milli
            || template.rationale.trim().is_empty()
            || !frontier
                .claim_order
                .binary_search(&template.claim_id)
                .is_ok()
            || !frontier.ranking.iter().any(|entry| {
                entry.claim_id == template.claim_id
                    && frontier_kind_matches(entry.action_kind, template.action_kind)
            })
        {
            return Err(KnowledgeActionCompilerError::InvalidTemplate(
                "template identity, ordering, bounds, claim binding, or frontier action binding is invalid".into(),
            ));
        }
        if claim_disposition(knowledge, &template.claim_id).is_none() {
            return Err(KnowledgeActionCompilerError::InvalidTemplate(
                "template claim is absent from typed knowledge".into(),
            ));
        }
    }
    Ok(())
}

impl KnowledgeActionPlan {
    pub fn validate(&self) -> Result<(), KnowledgeActionCompilerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.action_order.len() != self.actions.len()
            || !canonical(&self.action_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.actions.iter().any(|action| {
                action.action_id.trim().is_empty()
                    || action.information_gain_milli > 1_000
                    || action.cost_milli == 0
                    || !canonical(&action.modality_order)
                    || !canonical(&action.model_system_order)
                    || !canonical(&action.dependency_order)
                    || action.reason.trim().is_empty()
            })
        {
            return Err(KnowledgeActionCompilerError::InvalidOutput(
                "identity, action partition, ordering, bounds, or rationale is invalid".into(),
            ));
        }
        let actions = self
            .actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        if actions != self.action_order.iter().cloned().collect::<BTreeSet<_>>()
            || self.selected_order.iter().any(|id| !actions.contains(id))
            || self.deferred_order.iter().any(|id| !actions.contains(id))
            || self.blocked_order.iter().any(|id| !actions.contains(id))
            || self
                .selected_order
                .iter()
                .chain(self.deferred_order.iter())
                .chain(self.blocked_order.iter())
                .cloned()
                .collect::<BTreeSet<_>>()
                != actions
        {
            return Err(KnowledgeActionCompilerError::InvalidOutput(
                "action partitions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| KnowledgeActionCompilerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(KnowledgeActionCompilerError::InvalidOutput(
                "action plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a deterministic, dependency-closed research-action portfolio from typed knowledge and
/// its frontier. This is a planning capability: no evidence is fabricated and no experiment is
/// executed by the compiler.
pub fn compile_glioma_knowledge_actions(
    request: &KnowledgeActionCompilerRequest,
    knowledge: &TypedKnowledge,
    frontier: &KnowledgeFrontier,
    templates: &[KnowledgeActionTemplate],
) -> Result<KnowledgeActionPlan, KnowledgeActionCompilerError> {
    validate_request(request, knowledge, frontier, templates)?;
    let templates = templates
        .iter()
        .map(|template| (template.template_id.clone(), template))
        .collect::<BTreeMap<_, _>>();
    let priorities = frontier_priority(frontier);
    let mut blocked = BTreeSet::new();
    for template in templates.values() {
        if template
            .dependency_order
            .iter()
            .any(|dependency| !templates.contains_key(dependency))
        {
            blocked.insert(template.template_id.clone());
        }
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    for id in templates.keys() {
        cycle_nodes(id, &templates, &mut visiting, &mut visited, &mut cycles);
    }
    blocked.extend(cycles);
    let mut ranking = templates
        .values()
        .filter_map(|template| {
            priorities.get(&template.claim_id).map(|(priority, _)| {
                (
                    score(
                        *priority,
                        template.information_gain_milli,
                        template.cost_milli,
                    ),
                    template.template_id.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| right.cmp(left));

    let mut selected = BTreeSet::new();
    let mut selected_cost = 0_u64;
    let mut selected_risk = 0_u64;
    let mut selected_information = 0_u64;
    let mut deferred = BTreeSet::new();
    for (_, id) in ranking {
        if blocked.contains(&id) || selected.contains(&id) {
            continue;
        }
        let mut closure = BTreeSet::new();
        if !dependency_closure(&id, &templates, &blocked, &mut closure) {
            blocked.insert(id);
            continue;
        }
        let new_actions = closure
            .difference(&selected)
            .cloned()
            .collect::<BTreeSet<_>>();
        let closure_cost = new_actions
            .iter()
            .filter_map(|action_id| templates.get(action_id).map(|template| template.cost_milli))
            .sum::<u64>();
        let closure_risk = new_actions
            .iter()
            .filter_map(|action_id| templates.get(action_id).map(|template| template.risk_milli))
            .sum::<u64>();
        let closure_new = new_actions.len();
        if selected.len() + closure_new > request.max_selected_actions
            || selected_cost.saturating_add(closure_cost) > request.budget_milli
            || selected_risk.saturating_add(closure_risk) > request.risk_budget_milli
        {
            deferred.insert(id);
            continue;
        }
        selected.extend(closure);
        selected_cost = selected_cost.saturating_add(closure_cost);
        selected_risk = selected_risk.saturating_add(closure_risk);
        selected_information = selected_information.saturating_add(
            templates
                .get(&id)
                .map(|template| u64::from(template.information_gain_milli))
                .unwrap_or_default(),
        );
    }
    let all_ids = templates.keys().cloned().collect::<BTreeSet<_>>();
    let deferred = deferred
        .union(&all_ids)
        .filter(|id| !selected.contains(*id) && !blocked.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut actions = templates
        .values()
        .map(|template| {
            let priority = priorities
                .get(&template.claim_id)
                .map(|(value, _)| *value)
                .unwrap_or_default();
            let disposition = if selected.contains(&template.template_id) {
                CompiledActionDisposition::Selected
            } else if blocked.contains(&template.template_id) {
                CompiledActionDisposition::Blocked
            } else {
                CompiledActionDisposition::Deferred
            };
            let reason = match disposition {
                CompiledActionDisposition::Selected => {
                    "dependency-closed action admitted within budget and risk gates"
                }
                CompiledActionDisposition::Deferred => {
                    "deferred by budget, risk, or action-count gate"
                }
                CompiledActionDisposition::Blocked => {
                    "blocked by missing dependency or dependency cycle"
                }
            };
            CompiledResearchAction {
                action_id: template.template_id.clone(),
                claim_id: template.claim_id.clone(),
                action_kind: template.action_kind,
                modality_order: template.modality_order.clone(),
                model_system_order: template.model_system_order.clone(),
                information_gain_milli: template.information_gain_milli,
                cost_milli: template.cost_milli,
                risk_milli: template.risk_milli,
                dependency_order: template.dependency_order.clone(),
                priority_milli: priority,
                disposition,
                reason: reason.into(),
            }
        })
        .collect::<Vec<_>>();
    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = frontier.negative_evidence_order.clone();
    for claim in &knowledge.claims {
        if claim.disposition == KnowledgeClaimDisposition::Negative {
            negative_evidence.extend(claim.negative_evidence_order.iter().cloned());
        }
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    let mut uncertainty = frontier.uncertainty_order.clone();
    uncertainty.extend(
        blocked
            .iter()
            .map(|id| format!("blocked-action:{id}"))
            .chain(deferred.iter().map(|id| format!("deferred-action:{id}"))),
    );
    uncertainty.sort();
    uncertainty.dedup();
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let deferred_order = deferred.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let disposition = if selected_order.is_empty() {
        KnowledgeActionPlanDisposition::Unresolved
    } else if deferred_order.is_empty() && blocked_order.is_empty() {
        KnowledgeActionPlanDisposition::Qualified
    } else {
        KnowledgeActionPlanDisposition::Partial
    };
    let mut output = KnowledgeActionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        knowledge_digest: knowledge.digest.clone(),
        frontier_digest: frontier.digest.clone(),
        action_order,
        selected_order,
        deferred_order,
        blocked_order,
        actions,
        selected_cost_milli: selected_cost,
        selected_risk_milli: selected_risk,
        selected_information_milli: selected_information,
        negative_evidence_order: negative_evidence,
        uncertainty_order: uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-knowledge-action-plan"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| KnowledgeActionCompilerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p02_evidence_knowledge::claim_frontier::KnowledgeFrontierScore;
    use crate::glioma::programs::p02_evidence_knowledge::knowledge_graph::{
        KnowledgeClaim, KnowledgeDisposition,
    };

    fn fixtures() -> (
        TypedKnowledge,
        KnowledgeFrontier,
        KnowledgeActionCompilerRequest,
    ) {
        let claim = KnowledgeClaim {
            claim_id: "claim-1".into(),
            statement: "invasion state is associated with the declared perturbation".into(),
            scope: "organoid:invasion".into(),
            modality_order: vec![GliomaModality::Imaging],
            model_system_order: vec![GliomaModelSystem::Organoid],
            supporting_evidence_order: vec!["ev-1".into()],
            negative_evidence_order: Vec::new(),
            contradictory_evidence_order: Vec::new(),
            unresolved_evidence_order: Vec::new(),
            missing_modality_order: Vec::new(),
            missing_model_system_order: Vec::new(),
            support_milli: 900,
            contradiction_milli: 0,
            confidence_milli: 900,
            disposition: KnowledgeClaimDisposition::Supported,
        };
        let knowledge_input = serde_json::json!({
            "feature_id": "GAF-GLIOMA-P02-F01",
            "output_schema": "GliomaTypedKnowledge1@1",
            "objective": "compile invasion evidence",
            "claims": [claim],
            "claim_order": ["claim-1"],
            "top_claim_order": ["claim-1"],
            "omission_order": [],
            "negative_evidence_order": [],
            "uncertainty_order": [],
            "disposition": "qualified"
        });
        let knowledge_digest = ContentHash::of_value(&knowledge_input).unwrap();
        let knowledge = TypedKnowledge {
            feature_id: "GAF-GLIOMA-P02-F01".into(),
            output_schema: "GliomaTypedKnowledge1@1".into(),
            objective: "compile invasion evidence".into(),
            claims: vec![claim],
            claim_order: vec!["claim-1".into()],
            top_claim_order: vec!["claim-1".into()],
            omission_order: Vec::new(),
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: KnowledgeDisposition::Qualified,
            digest: knowledge_digest,
        };
        let frontier_input = serde_json::json!({
            "feature_id": "GAF-GLIOMA-P02-F09",
            "output_schema": "GliomaKnowledgeFrontier1@1",
            "objective": "compile invasion evidence",
            "knowledge_digest": knowledge.digest,
            "claim_order": ["claim-1"],
            "ranking": [{"claim_id":"claim-1","action_kind":"validate_supported","priority_milli":900,"coverage_debt_milli":0,"contradiction_milli":0,"uncertainty_milli":0,"support_milli":900,"workflow_leverage_milli":900,"rationale":"supported claim needs independent validation"}],
            "selected_order": ["claim-1"],
            "deferred_order": [],
            "negative_evidence_order": [],
            "uncertainty_order": [],
            "disposition": "qualified"
        });
        let frontier_digest = ContentHash::of_value(&frontier_input).unwrap();
        let frontier = KnowledgeFrontier {
            feature_id: "GAF-GLIOMA-P02-F09".into(),
            output_schema: "GliomaKnowledgeFrontier1@1".into(),
            objective: "compile invasion evidence".into(),
            knowledge_digest: knowledge.digest.clone(),
            claim_order: vec!["claim-1".into()],
            ranking: vec![KnowledgeFrontierScore {
                claim_id: "claim-1".into(),
                action_kind: FrontierActionKind::ValidateSupported,
                priority_milli: 900,
                coverage_debt_milli: 0,
                contradiction_milli: 0,
                uncertainty_milli: 0,
                support_milli: 900,
                workflow_leverage_milli: 900,
                rationale: "supported claim needs independent validation".into(),
            }],
            selected_order: vec!["claim-1".into()],
            deferred_order: Vec::new(),
            negative_evidence_order: Vec::new(),
            uncertainty_order: Vec::new(),
            disposition: super::super::claim_frontier::KnowledgeFrontierDisposition::Qualified,
            digest: frontier_digest,
        };
        let request = KnowledgeActionCompilerRequest {
            objective: "compile invasion evidence".into(),
            budget_milli: 1_000,
            risk_budget_milli: 1_000,
            max_selected_actions: 4,
            min_information_gain_milli: 100,
        };
        (knowledge, frontier, request)
    }

    #[test]
    fn compiles_claim_frontier_into_selected_research_action() {
        let (knowledge, frontier, request) = fixtures();
        let templates = vec![KnowledgeActionTemplate {
            template_id: "validate-claim-1".into(),
            claim_id: "claim-1".into(),
            action_kind: FrontierActionKind::ValidateSupported,
            modality_order: vec![GliomaModality::Imaging],
            model_system_order: vec![GliomaModelSystem::Organoid],
            information_gain_milli: 800,
            cost_milli: 100,
            risk_milli: 100,
            dependency_order: Vec::new(),
            rationale: "independent imaging validation".into(),
        }];
        let plan = compile_glioma_knowledge_actions(&request, &knowledge, &frontier, &templates)
            .expect("plan");
        assert_eq!(plan.disposition, KnowledgeActionPlanDisposition::Qualified);
        assert_eq!(plan.selected_order, vec!["validate-claim-1"]);
        assert_eq!(plan.selected_information_milli, 800);
    }

    #[test]
    fn missing_dependency_is_blocked_without_fabricating_execution() {
        let (knowledge, frontier, request) = fixtures();
        let templates = vec![KnowledgeActionTemplate {
            template_id: "validate-claim-1".into(),
            claim_id: "claim-1".into(),
            action_kind: FrontierActionKind::ValidateSupported,
            modality_order: vec![GliomaModality::Imaging],
            model_system_order: vec![GliomaModelSystem::Organoid],
            information_gain_milli: 800,
            cost_milli: 100,
            risk_milli: 100,
            dependency_order: vec!["missing-prerequisite".into()],
            rationale: "validation requires an unavailable prerequisite".into(),
        }];
        let plan = compile_glioma_knowledge_actions(&request, &knowledge, &frontier, &templates)
            .expect("blocked plan");
        assert_eq!(plan.disposition, KnowledgeActionPlanDisposition::Unresolved);
        assert_eq!(plan.blocked_order, vec!["validate-claim-1"]);
        assert!(plan.selected_order.is_empty());
    }
}
