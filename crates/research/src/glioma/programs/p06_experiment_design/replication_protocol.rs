//! Compile a replication continuation decision into an executable local protocol.
//!
//! P06-F27 decides whether another replication wave is justified. This feature is the handoff
//! boundary to P07: it materializes site setup, control, treatment, and optional QC tasks, then
//! runs the deterministic protocol simulator as a preflight. Held, negative, risk-blocked, and
//! unresolved decisions remain explicit and never become dispatches or clinical conclusions.

use super::replication_continuation::{ReplicationContinuationAction, ReplicationContinuationPlan};
use crate::glioma::programs::p07_protocol_simulation::{
    simulate_glioma_protocol, ProtocolDisposition, ProtocolResource, ProtocolResourceKind,
    ProtocolSimulation, ProtocolSimulationError, ProtocolSimulationRequest, ProtocolTask,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationProtocolCompilation1@1";
pub const MAX_TASKS: usize = 4_096;
pub const MAX_TICKS_PER_REPLICATE: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationProtocolCompileRequest {
    pub continuation: ReplicationContinuationPlan,
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
pub enum ReplicationProtocolCompilationDisposition {
    Compiled,
    Held,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationProtocolCompilation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub continuation_digest: ContentHash,
    pub protocol: Option<ProtocolSimulationRequest>,
    pub preflight: Option<ProtocolSimulation>,
    pub site_order: Vec<String>,
    pub compiled_site_order: Vec<String>,
    pub held_site_order: Vec<String>,
    pub task_order: Vec<String>,
    pub withheld_action_order: Vec<String>,
    pub disposition: ReplicationProtocolCompilationDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationProtocolCompilationError {
    #[error("replication protocol compilation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication protocol preflight failed: {0}")]
    Preflight(String),
    #[error("replication protocol compilation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication protocol compilation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ReplicationProtocolCompilation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "continuation_digest": output.continuation_digest,
        "protocol": output.protocol,
        "preflight": output.preflight,
        "site_order": output.site_order,
        "compiled_site_order": output.compiled_site_order,
        "held_site_order": output.held_site_order,
        "task_order": output.task_order,
        "withheld_action_order": output.withheld_action_order,
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
    request: &ReplicationProtocolCompileRequest,
) -> Result<(), ReplicationProtocolCompilationError> {
    request.continuation.validate().map_err(|error| {
        ReplicationProtocolCompilationError::InvalidRequest(format!(
            "continuation is invalid: {error}"
        ))
    })?;
    if request.max_ticks == 0
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
        return Err(ReplicationProtocolCompilationError::InvalidRequest(
            "horizon, risk, seed, duration, resources, and approval bounds are required".into(),
        ));
    }
    let mut resource_ids = BTreeSet::new();
    for resource in &request.resources {
        if resource.resource_id.trim().is_empty()
            || resource.capacity_units == 0
            || !resource_ids.insert(resource.resource_id.clone())
        {
            return Err(ReplicationProtocolCompilationError::InvalidRequest(
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
        return Err(ReplicationProtocolCompilationError::InvalidRequest(
            "quality-task compilation requires a compute resource".into(),
        ));
    }
    Ok(())
}

fn validate_output(
    output: &ReplicationProtocolCompilation,
) -> Result<(), ReplicationProtocolCompilationError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.boundary != PRECLINICAL_BOUNDARY
        || output.site_order.is_empty()
        || !canonical(&output.site_order)
        || !canonical(&output.compiled_site_order)
        || !canonical(&output.held_site_order)
        || !canonical(&output.task_order)
        || !canonical(&output.withheld_action_order)
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
    {
        return Err(ReplicationProtocolCompilationError::InvalidOutput(
            "identity, ordering, boundary, limitation, or preflight fields are invalid".into(),
        ));
    }
    if output
        .compiled_site_order
        .iter()
        .chain(output.held_site_order.iter())
        .any(|site| !output.site_order.contains(site))
        || output
            .compiled_site_order
            .iter()
            .any(|site| output.held_site_order.contains(site))
    {
        return Err(ReplicationProtocolCompilationError::InvalidOutput(
            "compiled and held site partitions do not reconcile".into(),
        ));
    }
    if output.task_order.len() > MAX_TASKS {
        return Err(ReplicationProtocolCompilationError::InvalidOutput(
            "compiled task order exceeds the bounded product limit".into(),
        ));
    }
    if output.disposition == ReplicationProtocolCompilationDisposition::Compiled
        && (output.protocol.is_none()
            || output.preflight.is_none()
            || output
                .preflight
                .as_ref()
                .is_some_and(|simulation| simulation.disposition != ProtocolDisposition::Feasible))
    {
        return Err(ReplicationProtocolCompilationError::InvalidOutput(
            "compiled output requires a feasible protocol preflight".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ReplicationProtocolCompilationError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ReplicationProtocolCompilationError::InvalidOutput(
            "compilation digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ReplicationProtocolCompilation {
    pub fn validate(&self) -> Result<(), ReplicationProtocolCompilationError> {
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
        output_schema: "GliomaReplicationProtocolTask1@1".into(),
        risk_milli: risk_milli.min(1_000),
        requires_instrument: false,
    }
}

/// Compile a continuation decision into a local protocol request and deterministic preflight.
pub fn compile_glioma_replication_protocol(
    request: &ReplicationProtocolCompileRequest,
) -> Result<ReplicationProtocolCompilation, ReplicationProtocolCompilationError> {
    validate_request(request)?;
    let continuation = &request.continuation;
    let site_order = continuation.site_order.clone();
    let mut compiled_site_order = Vec::new();
    let mut held_site_order = Vec::new();
    let mut withheld_action_order = Vec::new();
    let mut tasks = Vec::new();
    let primary_kind = primary_resource(continuation.model_system);
    let mut uncertainty = vec![
        "compiled tasks describe a local preclinical workflow, not biological efficacy".into(),
        "the caller-owned executor remains responsible for institution-local approvals and effects"
            .into(),
    ];
    let mut negative_evidence = Vec::new();
    for action in &continuation.actions {
        if matches!(
            action.action,
            ReplicationContinuationAction::ContinueReplication
        ) && action.planned_new_replicates > 0
        {
            compiled_site_order.push(action.site_id.clone());
            let prefix = format!("replication:{}", action.site_id);
            let setup_id = format!("{prefix}:setup");
            let control_id = format!("{prefix}:control");
            let treatment_id = format!("{prefix}:treatment");
            let qc_id = format!("{prefix}:qc");
            let duration = action
                .planned_new_replicates
                .saturating_mul(request.ticks_per_replicate)
                .max(1);
            tasks.push(task(
                setup_id.clone(),
                format!("prepare replication site {}", action.site_id),
                primary_kind,
                1,
                Vec::new(),
                continuation.model_system,
                action.risk_milli.min(100),
            ));
            tasks.push(task(
                control_id.clone(),
                format!("collect control arm at replication site {}", action.site_id),
                primary_kind,
                duration,
                vec![setup_id.clone()],
                continuation.model_system,
                action.risk_milli,
            ));
            tasks.push(task(
                treatment_id.clone(),
                format!(
                    "collect treatment arm at replication site {}",
                    action.site_id
                ),
                primary_kind,
                duration,
                vec![setup_id],
                continuation.model_system,
                action.risk_milli,
            ));
            if request.include_quality_task {
                tasks.push(task(
                    qc_id,
                    format!("quality-check replication site {}", action.site_id),
                    ProtocolResourceKind::Compute,
                    1,
                    vec![control_id, treatment_id],
                    continuation.model_system,
                    5,
                ));
            }
        } else {
            held_site_order.push(action.site_id.clone());
            withheld_action_order.push(format!("{}:{:?}", action.site_id, action.action));
            negative_evidence.push(format!(
                "site:{}:withheld:{:?}",
                action.site_id, action.action
            ));
        }
    }
    compiled_site_order.sort();
    held_site_order.sort();
    withheld_action_order.sort();
    tasks.sort_by(|left, right| left.task_id.cmp(&right.task_id));
    let task_order = tasks
        .iter()
        .map(|task| task.task_id.clone())
        .collect::<Vec<_>>();
    if tasks.is_empty() {
        uncertainty.push("continuation produced no dispatchable site actions".into());
        let mut output = ReplicationProtocolCompilation {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            objective: continuation.objective.clone(),
            model_system: continuation.model_system,
            continuation_digest: continuation.digest.clone(),
            protocol: None,
            preflight: None,
            site_order,
            compiled_site_order,
            held_site_order,
            task_order,
            withheld_action_order,
            disposition: ReplicationProtocolCompilationDisposition::Held,
            negative_evidence,
            uncertainty,
            next_action: "resolve the continuation hold before compiling another local protocol"
                .into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed-glioma-replication-protocol-compilation"),
        };
        output.digest = ContentHash::of_value(&digest_input(&output))
            .map_err(|error| ReplicationProtocolCompilationError::Digest(error.to_string()))?;
        validate_output(&output)?;
        return Ok(output);
    }
    let protocol = ProtocolSimulationRequest {
        objective: continuation.objective.clone(),
        model_system: continuation.model_system,
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
            ReplicationProtocolCompilationError::Preflight(error.to_string())
        })?;
    let disposition = if preflight.disposition == ProtocolDisposition::Feasible {
        ReplicationProtocolCompilationDisposition::Compiled
    } else {
        uncertainty.push(format!("preflight-disposition:{:?}", preflight.disposition));
        negative_evidence.extend(
            preflight
                .negative_evidence
                .iter()
                .map(|item| format!("preflight:{item}")),
        );
        ReplicationProtocolCompilationDisposition::Unresolved
    };
    let next_action = if disposition == ReplicationProtocolCompilationDisposition::Compiled {
        "submit the preflighted protocol to a caller-owned local executor after institutional approval"
    } else {
        "repair the preflight failure or resource envelope before any local execution"
    }
    .to_string();
    let mut output = ReplicationProtocolCompilation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: continuation.objective.clone(),
        model_system: continuation.model_system,
        continuation_digest: continuation.digest.clone(),
        protocol: Some(protocol),
        preflight: Some(preflight),
        site_order,
        compiled_site_order,
        held_site_order,
        task_order,
        withheld_action_order,
        disposition,
        negative_evidence,
        uncertainty,
        next_action,
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-protocol-compilation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReplicationProtocolCompilationError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::{
        plan_glioma_replication_continuation, ReplicationContinuationObservation,
        ReplicationPlanRequest,
    };
    use crate::glioma_engine::LocalArtifactRef;

    fn observation(
        site: &str,
        arm: &str,
        mean: i32,
        quality_milli: u16,
    ) -> ReplicationContinuationObservation {
        ReplicationContinuationObservation {
            round: 1,
            quality_milli,
            observation: super::super::replication_plan::ReplicationObservation {
                site_id: site.into(),
                arm_id: arm.into(),
                label: format!("{site}-{arm}"),
                artifact: LocalArtifactRef {
                    artifact_id: format!("artifact-{site}-{arm}"),
                    content_hash: ContentHash::of_bytes(format!("{site}-{arm}").as_bytes()),
                    content_type: "application/json".into(),
                    local_only: true,
                    contains_human_data: false,
                    contains_direct_identifiers: false,
                },
                model_system: GliomaModelSystem::Organoid,
                mean_response_milli: mean,
                variance_milli2: 100,
                observations: 2,
                cost_units_per_replicate: 1,
                risk_milli: 100,
            },
        }
    }

    fn continuation_with_quality(quality_milli: u16) -> ReplicationContinuationPlan {
        plan_glioma_replication_continuation(
            &crate::glioma::programs::p06_experiment_design::ReplicationContinuationRequest {
                plan_request: ReplicationPlanRequest {
                    objective: "compile an organoid replication wave".into(),
                    model_system: GliomaModelSystem::Organoid,
                    endpoint: "invasion".into(),
                    control_arm_id: "control".into(),
                    treatment_arm_id: "treated".into(),
                    target_effect_milli: 200,
                    alpha_total_milli: 50,
                    power_target_milli: 700,
                    min_sites: 2,
                    max_sites: 4,
                    min_replicates_per_site: 2,
                    max_replicates_per_site: 20,
                    budget_units: 100,
                    max_total_replicates: 80,
                    max_site_heterogeneity_milli: 200,
                    risk_ceiling_milli: 500,
                },
                current_round: 1,
                max_rounds: 4,
                minimum_quality_milli: 700,
                negative_effect_threshold_milli: 40,
                observations: vec![
                    observation("site-a", "control", 100, quality_milli),
                    observation("site-a", "treated", 220, quality_milli),
                    observation("site-b", "control", 110, quality_milli),
                    observation("site-b", "treated", 230, quality_milli),
                ],
                previous_plan: None,
            },
        )
        .unwrap()
    }

    fn continuation() -> ReplicationContinuationPlan {
        continuation_with_quality(900)
    }

    fn request() -> ReplicationProtocolCompileRequest {
        ReplicationProtocolCompileRequest {
            continuation: continuation(),
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
            randomization_seed: ContentHash::of_bytes(b"replication-protocol-test"),
            ticks_per_replicate: 2,
            include_quality_task: true,
        }
    }

    #[test]
    fn compilation_is_replay_stable_and_preflights_tasks() {
        let request = request();
        let left = compile_glioma_replication_protocol(&request).unwrap();
        let right = compile_glioma_replication_protocol(&request).unwrap();
        assert_eq!(left, right);
        assert_eq!(
            left.disposition,
            ReplicationProtocolCompilationDisposition::Compiled
        );
        assert!(left.task_order.iter().any(|task| task.ends_with(":qc")));
        assert_eq!(
            left.preflight.as_ref().unwrap().disposition,
            ProtocolDisposition::Feasible
        );
        left.validate().unwrap();
    }

    #[test]
    fn held_continuation_never_becomes_a_protocol() {
        let mut request = request();
        request.continuation = continuation_with_quality(400);
        let output = compile_glioma_replication_protocol(&request).unwrap();
        assert_eq!(
            output.disposition,
            ReplicationProtocolCompilationDisposition::Held
        );
        assert!(output.protocol.is_none());
        assert!(output.preflight.is_none());
    }
}
