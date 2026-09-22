//! Compile a mechanism-validation decision into a deterministic local protocol.
//!
//! P06-F29 decides which robust mechanistic interventions deserve another bounded validation
//! batch. This feature is the P06-to-P07 handoff: only interim `continue` or `underpowered`
//! decisions become control/intervention/QC tasks, while efficacy, futility, risk, budget, and
//! missing-evidence decisions remain withheld. The simulator is a scheduling preflight, not a
//! biological or clinical conclusion; an institution-owned executor must provide every real-world
//! effect after its own approvals and interlocks.

use super::mechanism_validation::{
    MechanismValidationArm, MechanismValidationPlan, ValidationArmRole,
};
use super::power_reestimation::PowerDecisionKind;
use crate::glioma::programs::p07_protocol_simulation::{
    simulate_glioma_protocol, ProtocolDisposition, ProtocolResource, ProtocolResourceKind,
    ProtocolSimulation, ProtocolSimulationError, ProtocolSimulationRequest, ProtocolTask,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F16";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismValidationProtocol1@1";
pub const MAX_ARMS: usize = 256;
pub const MAX_TASKS: usize = 4_096;
pub const MAX_TICKS_PER_REPLICATE: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismValidationProtocolCompileRequest {
    pub validation: MechanismValidationPlan,
    pub arms: Vec<MechanismValidationArm>,
    pub resources: Vec<ProtocolResource>,
    pub max_ticks: u32,
    pub max_risk_milli: u16,
    pub allow_instrument_execution: bool,
    pub approval_reference: Option<String>,
    pub randomization_seed: ContentHash,
    pub ticks_per_replicate: u32,
    pub include_quality_task: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismValidationProtocolDisposition {
    Compiled,
    Held,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismValidationProtocolCompilation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub validation_digest: ContentHash,
    pub protocol: Option<ProtocolSimulationRequest>,
    pub preflight: Option<ProtocolSimulation>,
    pub scheduled_arm_order: Vec<String>,
    pub withheld_arm_order: Vec<String>,
    pub task_order: Vec<String>,
    pub disposition: MechanismValidationProtocolDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismValidationProtocolError {
    #[error("mechanism validation protocol request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism validation protocol preflight failed: {0}")]
    Preflight(String),
    #[error("mechanism validation protocol output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism validation protocol digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismValidationProtocolCompilation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "validation_digest": output.validation_digest,
        "protocol": output.protocol,
        "preflight": output.preflight,
        "scheduled_arm_order": output.scheduled_arm_order,
        "withheld_arm_order": output.withheld_arm_order,
        "task_order": output.task_order,
        "disposition": output.disposition,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn primary_resource(model_system: GliomaModelSystem) -> ProtocolResourceKind {
    match model_system {
        GliomaModelSystem::InSilico => ProtocolResourceKind::Compute,
        GliomaModelSystem::MouseModel
        | GliomaModelSystem::ZebrafishModel
        | GliomaModelSystem::PatientDerivedXenograft => ProtocolResourceKind::AnimalFacility,
        GliomaModelSystem::CellLine | GliomaModelSystem::Organoid => ProtocolResourceKind::Culture,
    }
}

fn validate_request(
    request: &MechanismValidationProtocolCompileRequest,
) -> Result<(), MechanismValidationProtocolError> {
    request
        .validation
        .validate()
        .map_err(|error| MechanismValidationProtocolError::InvalidRequest(error.to_string()))?;
    if request.arms.is_empty()
        || request.arms.len() > MAX_ARMS
        || request.max_ticks == 0
        || request.max_risk_milli > 1_000
        || request.randomization_seed.as_str().len() != 64
        || request.ticks_per_replicate == 0
        || request.ticks_per_replicate > MAX_TICKS_PER_REPLICATE
        || request.resources.is_empty()
        || request
            .approval_reference
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(MechanismValidationProtocolError::InvalidRequest(
            "validated plan, bounded arms, horizon, risk, seed, duration, resources, and approval are required".into(),
        ));
    }
    let mut arm_ids = BTreeSet::new();
    for arm in &request.arms {
        arm.artifact
            .validate()
            .map_err(|error| MechanismValidationProtocolError::InvalidRequest(error.to_string()))?;
        if arm.arm_id.trim().is_empty() || !arm_ids.insert(arm.arm_id.clone()) {
            return Err(MechanismValidationProtocolError::InvalidRequest(
                "validation arm identifiers must be unique and non-empty".into(),
            ));
        }
    }
    let mut resource_ids = BTreeSet::new();
    for resource in &request.resources {
        if resource.resource_id.trim().is_empty()
            || resource.capacity_units == 0
            || !resource_ids.insert(resource.resource_id.clone())
        {
            return Err(MechanismValidationProtocolError::InvalidRequest(
                "resource ids must be unique and capacities positive".into(),
            ));
        }
    }
    if request.include_quality_task
        && !request
            .resources
            .iter()
            .any(|resource| resource.kind == ProtocolResourceKind::Compute)
    {
        return Err(MechanismValidationProtocolError::InvalidRequest(
            "quality-task compilation requires a compute resource".into(),
        ));
    }
    Ok(())
}

fn validate_output(
    output: &MechanismValidationProtocolCompilation,
) -> Result<(), MechanismValidationProtocolError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.boundary != PRECLINICAL_BOUNDARY
        || !canonical(&output.scheduled_arm_order)
        || !canonical(&output.withheld_arm_order)
        || !canonical(&output.task_order)
        || output.next_action.trim().is_empty()
        || output
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
        || output.uncertainty.iter().any(|item| item.trim().is_empty())
        || output
            .preflight
            .as_ref()
            .is_some_and(|simulation| simulation.validate().is_err())
        || output
            .scheduled_arm_order
            .iter()
            .any(|arm| output.withheld_arm_order.binary_search(arm).is_ok())
    {
        return Err(MechanismValidationProtocolError::InvalidOutput(
            "identity, boundary, ordering, limitations, or preflight fields are invalid".into(),
        ));
    }
    if output.task_order.len() > MAX_TASKS {
        return Err(MechanismValidationProtocolError::InvalidOutput(
            "compiled task order exceeds the bounded product limit".into(),
        ));
    }
    if output.disposition == MechanismValidationProtocolDisposition::Compiled
        && (output.protocol.is_none()
            || output.preflight.is_none()
            || output
                .preflight
                .as_ref()
                .is_some_and(|simulation| simulation.disposition != ProtocolDisposition::Feasible))
    {
        return Err(MechanismValidationProtocolError::InvalidOutput(
            "compiled output requires a feasible protocol preflight".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| MechanismValidationProtocolError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(MechanismValidationProtocolError::InvalidOutput(
            "mechanism validation protocol digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl MechanismValidationProtocolCompilation {
    pub fn validate(&self) -> Result<(), MechanismValidationProtocolError> {
        validate_output(self)
    }
}

fn task(
    task_id: String,
    label: String,
    resource_kind: ProtocolResourceKind,
    duration_ticks: u32,
    depends_on: Vec<String>,
    model_system: GliomaModelSystem,
    risk_milli: u16,
) -> ProtocolTask {
    ProtocolTask {
        task_id,
        label,
        resource_kind,
        resource_units: 1,
        duration_ticks: duration_ticks.max(1),
        depends_on,
        model_system,
        output_schema: "GliomaMechanismValidationTask1@1".into(),
        risk_milli: risk_milli.min(1_000),
        requires_instrument: false,
    }
}

/// Compile only the still-open validation decisions into a local protocol and preflight.
pub fn compile_glioma_mechanism_validation_protocol(
    request: &MechanismValidationProtocolCompileRequest,
) -> Result<MechanismValidationProtocolCompilation, MechanismValidationProtocolError> {
    validate_request(request)?;
    let validation = &request.validation;
    let arm_map = request
        .arms
        .iter()
        .map(|arm| (arm.arm_id.clone(), arm))
        .collect::<BTreeMap<_, _>>();
    let Some(power_plan) = validation.power_plan.as_ref() else {
        let output = held_output(
            request,
            Vec::new(),
            request.arms.iter().map(|arm| arm.arm_id.clone()).collect(),
            vec!["validation plan has no executable power plan".into()],
            vec!["power-plan-missing".into()],
        )?;
        output.validate()?;
        return Ok(output);
    };
    let mut scheduled = BTreeSet::new();
    let mut withheld = BTreeSet::new();
    let mut negative = validation.negative_evidence.clone();
    let mut uncertainty = validation.uncertainty.clone();
    for decision in &power_plan.decisions {
        match decision.decision {
            PowerDecisionKind::Continue | PowerDecisionKind::HoldUnderpowered => {
                scheduled.insert(decision.arm_id.clone());
            }
            PowerDecisionKind::EfficacyStop => {
                withheld.insert(decision.arm_id.clone());
                negative.push(format!("arm:{}:efficacy-stop-withheld", decision.arm_id));
            }
            PowerDecisionKind::FutilityStop => {
                withheld.insert(decision.arm_id.clone());
                negative.push(format!("arm:{}:futility-stop-withheld", decision.arm_id));
            }
            PowerDecisionKind::RiskBlocked | PowerDecisionKind::BudgetBlocked => {
                withheld.insert(decision.arm_id.clone());
                uncertainty.push(format!("arm:{}:power-boundary-withheld", decision.arm_id));
            }
        }
    }
    if scheduled.is_empty() {
        let output = held_output(
            request,
            scheduled.into_iter().collect(),
            withheld.into_iter().collect(),
            negative,
            uncertainty,
        )?;
        output.validate()?;
        return Ok(output);
    }
    let missing = scheduled
        .iter()
        .filter(|arm_id| !arm_map.contains_key(*arm_id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        uncertainty.extend(
            missing
                .iter()
                .map(|arm| format!("missing-validation-arm:{arm}")),
        );
        let mut output = held_output(
            request,
            Vec::new(),
            scheduled.into_iter().collect(),
            negative,
            uncertainty,
        )?;
        output.disposition = MechanismValidationProtocolDisposition::Unresolved;
        output.next_action =
            "bind every scheduled validation arm before protocol compilation".into();
        output.digest = ContentHash::of_value(&digest_input(&output))
            .map_err(|error| MechanismValidationProtocolError::Digest(error.to_string()))?;
        output.validate()?;
        return Ok(output);
    }
    let primary_kind = primary_resource(validation.model_system);
    let mut tasks = Vec::new();
    for arm_id in &scheduled {
        let arm = arm_map[arm_id];
        let decision = power_plan
            .decisions
            .iter()
            .find(|decision| decision.arm_id == *arm_id)
            .expect("scheduled decision exists");
        let prefix = format!("validation:{arm_id}");
        let setup_id = format!("{prefix}:setup");
        let assay_id = format!("{prefix}:assay");
        let qc_id = format!("{prefix}:qc");
        let duration = u64::from(decision.planned_replicates.max(1))
            .saturating_mul(u64::from(request.ticks_per_replicate))
            .min(u64::from(u32::MAX)) as u32;
        let risk = if matches!(arm.role, ValidationArmRole::Control) {
            100
        } else {
            250
        };
        tasks.push(task(
            setup_id.clone(),
            format!("prepare mechanism validation arm {arm_id}"),
            primary_kind,
            1,
            Vec::new(),
            validation.model_system,
            risk,
        ));
        tasks.push(task(
            assay_id.clone(),
            format!("run validation assay {arm_id} using {}", arm.protocol_id),
            primary_kind,
            duration,
            vec![setup_id],
            validation.model_system,
            risk,
        ));
        if request.include_quality_task {
            tasks.push(task(
                qc_id,
                format!("quality-check validation arm {arm_id}"),
                ProtocolResourceKind::Compute,
                1,
                vec![assay_id],
                validation.model_system,
                5,
            ));
        }
    }
    tasks.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let task_order = tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    let protocol = ProtocolSimulationRequest {
        objective: validation.objective.clone(),
        model_system: validation.model_system,
        tasks,
        resources: request.resources.clone(),
        max_ticks: request.max_ticks,
        max_risk_milli: request.max_risk_milli,
        allow_instrument_execution: request.allow_instrument_execution,
        approval_reference: request.approval_reference.clone(),
        randomization_seed: request.randomization_seed.clone(),
    };
    let preflight =
        simulate_glioma_protocol(&protocol).map_err(|error: ProtocolSimulationError| {
            MechanismValidationProtocolError::Preflight(error.to_string())
        })?;
    let disposition = if preflight.disposition == ProtocolDisposition::Feasible {
        MechanismValidationProtocolDisposition::Compiled
    } else {
        uncertainty.extend(
            preflight
                .negative_evidence
                .iter()
                .map(|item| format!("preflight:{item}")),
        );
        negative.extend(
            preflight
                .negative_evidence
                .iter()
                .map(|item| format!("preflight:{item}")),
        );
        uncertainty.push(format!("preflight-disposition:{:?}", preflight.disposition));
        MechanismValidationProtocolDisposition::Unresolved
    };
    let next_action = if disposition == MechanismValidationProtocolDisposition::Compiled {
        "submit the preflighted validation protocol to a caller-owned local executor after institutional approval"
    } else {
        "repair the preflight failure or resource envelope before any validation execution"
    }
    .to_string();
    let mut output = MechanismValidationProtocolCompilation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: validation.objective.clone(),
        model_system: validation.model_system,
        validation_digest: validation.digest.clone(),
        protocol: Some(protocol),
        preflight: Some(preflight),
        scheduled_arm_order: scheduled.into_iter().collect(),
        withheld_arm_order: withheld.into_iter().collect(),
        task_order,
        disposition,
        negative_evidence: sorted_unique(negative),
        uncertainty: sorted_unique(uncertainty),
        next_action,
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-validation-protocol"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismValidationProtocolError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn held_output(
    request: &MechanismValidationProtocolCompileRequest,
    scheduled: Vec<String>,
    withheld: Vec<String>,
    negative_evidence: Vec<String>,
    uncertainty: Vec<String>,
) -> Result<MechanismValidationProtocolCompilation, MechanismValidationProtocolError> {
    let mut output = MechanismValidationProtocolCompilation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.validation.objective.clone(),
        model_system: request.validation.model_system,
        validation_digest: request.validation.digest.clone(),
        protocol: None,
        preflight: None,
        scheduled_arm_order: sorted_unique(scheduled),
        withheld_arm_order: sorted_unique(withheld),
        task_order: Vec::new(),
        disposition: MechanismValidationProtocolDisposition::Held,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: "resolve the validation hold before compiling a local protocol".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-validation-protocol"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismValidationProtocolError::Digest(error.to_string()))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::counterfactual::CounterfactualIntervention;
    use crate::glioma::programs::p05_mechanism_exploration::ensemble_counterfactual::CounterfactualModel;
    use crate::glioma::programs::p05_mechanism_exploration::graph_propagation::{
        MechanismGraphEdge, MechanismGraphNode, MechanismGraphRelation,
    };
    use crate::glioma::programs::p05_mechanism_exploration::robust_portfolio::{
        plan_glioma_robust_intervention_portfolio, PortfolioDirection, RobustInterventionCandidate,
        RobustInterventionRequest,
    };
    use crate::glioma::programs::p06_experiment_design::mechanism_validation::{
        plan_glioma_mechanism_validation, MechanismValidationPlanRequest,
    };
    use crate::glioma::programs::p06_experiment_design::power_reestimation::{
        PowerArmObservation, PowerReestimationRequest,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: hash(id),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn portfolio() -> crate::glioma::programs::p05_mechanism_exploration::RobustInterventionPortfolio
    {
        let node = |id: &str, modality| MechanismGraphNode {
            node_id: id.into(),
            label: id.into(),
            modality,
            prior_milli: 0,
            support_milli: 800,
            contradiction_milli: 0,
        };
        let model = |id: &str| CounterfactualModel {
            model_id: id.into(),
            prior_milli: 500,
            nodes: vec![
                node("egfr", crate::glioma_engine::GliomaModality::Genomics),
                node("invasion", crate::glioma_engine::GliomaModality::Imaging),
            ],
            edges: vec![MechanismGraphEdge {
                edge_id: format!("{id}-edge"),
                source_node_id: "egfr".into(),
                target_node_id: "invasion".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec![format!("evidence-{id}")],
            }],
        };
        plan_glioma_robust_intervention_portfolio(
            &RobustInterventionRequest {
                objective: "compile robust validation protocol".into(),
                model_system: GliomaModelSystem::Organoid,
                max_iterations: 100,
                convergence_tolerance_milli: 1,
                damping_milli: 600,
                min_edge_confidence_milli: 500,
                direction: PortfolioDirection::Decrease,
                budget_units: 3,
                max_selected: 1,
                min_robust_effect_milli: 10,
                min_agreement_milli: 750,
                risk_ceiling_milli: 800,
                effect_weight_milli: 500,
                tail_weight_milli: 300,
                worst_case_weight_milli: 200,
                feasibility_weight_milli: 1_000,
                risk_penalty_milli: 1,
            },
            &[model("a"), model("b")],
            &[RobustInterventionCandidate {
                candidate_id: "egfr-invasion".into(),
                label: "EGFR perturbation".into(),
                intervention: CounterfactualIntervention {
                    intervention_id: "egfr-invasion-intervention".into(),
                    node_id: "egfr".into(),
                    delta_milli: -600,
                    rationale: "test EGFR invasion mechanism".into(),
                    evidence_order: vec!["evidence-egfr".into()],
                },
                target_node_id: "invasion".into(),
                redundancy_group: "egfr".into(),
                feasibility_milli: 1_000,
                cost_units: 1,
                risk_milli: 100,
            }],
        )
        .unwrap()
    }

    fn validation() -> MechanismValidationPlan {
        let portfolio = portfolio();
        plan_glioma_mechanism_validation(&MechanismValidationPlanRequest {
            objective: "compile robust validation protocol".into(),
            portfolio,
            power: PowerReestimationRequest {
                objective: "compile robust validation protocol".into(),
                model_system: GliomaModelSystem::Organoid,
                endpoint: "invasion".into(),
                control_arm_id: "control".into(),
                target_effect_milli: 10,
                alpha_total_milli: 100,
                power_target_milli: 500,
                current_look: 1,
                max_looks: 2,
                min_replicates_per_arm: 1,
                max_replicates_per_arm: 16,
                max_new_replicates_per_arm: 4,
                budget_units: 16,
                risk_ceiling_milli: 800,
            },
            arms: vec![
                MechanismValidationArm {
                    arm_id: "control".into(),
                    candidate_id: None,
                    label: "control".into(),
                    target_node_id: "invasion".into(),
                    protocol_id: "validation-v1".into(),
                    role: ValidationArmRole::Control,
                    artifact: artifact("control-arm"),
                },
                MechanismValidationArm {
                    arm_id: "egfr-arm".into(),
                    candidate_id: Some("egfr-invasion".into()),
                    label: "EGFR".into(),
                    target_node_id: "invasion".into(),
                    protocol_id: "validation-v1".into(),
                    role: ValidationArmRole::Intervention,
                    artifact: artifact("egfr-arm"),
                },
            ],
            observations: vec![
                PowerArmObservation {
                    arm_id: "control".into(),
                    label: "control".into(),
                    artifact: artifact("control-observation"),
                    model_system: GliomaModelSystem::Organoid,
                    mean_response_milli: 100,
                    variance_milli2: 100,
                    observations: 2,
                    risk_milli: 100,
                    cost_units: 1,
                },
                PowerArmObservation {
                    arm_id: "egfr-arm".into(),
                    label: "EGFR".into(),
                    artifact: artifact("egfr-observation"),
                    model_system: GliomaModelSystem::Organoid,
                    mean_response_milli: 110,
                    variance_milli2: 100,
                    observations: 1,
                    risk_milli: 100,
                    cost_units: 1,
                },
            ],
            require_qualified_portfolio: true,
        })
        .unwrap()
    }

    fn request() -> MechanismValidationProtocolCompileRequest {
        MechanismValidationProtocolCompileRequest {
            validation: validation(),
            arms: vec![
                MechanismValidationArm {
                    arm_id: "control".into(),
                    candidate_id: None,
                    label: "control".into(),
                    target_node_id: "invasion".into(),
                    protocol_id: "validation-v1".into(),
                    role: ValidationArmRole::Control,
                    artifact: artifact("control-arm"),
                },
                MechanismValidationArm {
                    arm_id: "egfr-arm".into(),
                    candidate_id: Some("egfr-invasion".into()),
                    label: "EGFR".into(),
                    target_node_id: "invasion".into(),
                    protocol_id: "validation-v1".into(),
                    role: ValidationArmRole::Intervention,
                    artifact: artifact("egfr-arm"),
                },
            ],
            resources: vec![
                ProtocolResource {
                    resource_id: "culture".into(),
                    kind: ProtocolResourceKind::Culture,
                    capacity_units: 1,
                },
                ProtocolResource {
                    resource_id: "compute".into(),
                    kind: ProtocolResourceKind::Compute,
                    capacity_units: 1,
                },
            ],
            max_ticks: 200,
            max_risk_milli: 1_000,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: hash("protocol-seed"),
            ticks_per_replicate: 2,
            include_quality_task: true,
        }
    }

    #[test]
    fn open_validation_decisions_compile_to_a_preflighted_protocol() {
        let first = compile_glioma_mechanism_validation_protocol(&request()).unwrap();
        let second = compile_glioma_mechanism_validation_protocol(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            MechanismValidationProtocolDisposition::Compiled
        );
        assert!(first.task_order.iter().any(|task| task.ends_with(":qc")));
        assert_eq!(
            first.preflight.as_ref().unwrap().disposition,
            ProtocolDisposition::Feasible
        );
        first.validate().unwrap();
    }

    #[test]
    fn efficacy_stop_is_withheld_from_protocol_tasks() {
        let mut request = request();
        request.validation.power_plan.as_mut().unwrap().decisions[0].decision =
            PowerDecisionKind::EfficacyStop;
        // The plan digest is intentionally no longer valid, so the compiler must reject a
        // tampered stopping decision before a task can be materialized.
        assert!(compile_glioma_mechanism_validation_protocol(&request).is_err());
    }
}
