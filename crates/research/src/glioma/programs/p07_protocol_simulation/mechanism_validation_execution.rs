//! Execute a preflighted glioma mechanism-validation protocol through a local worker.
//!
//! This is the product seam between P06 scientific stopping logic and P07 workflow execution.
//! It refuses held or unresolved validation plans, re-simulates the protocol before every run,
//! delegates effects to the caller-owned [`GliomaProtocolExecutor`], and returns honest partial,
//! failed, and negative outcomes. The default MCP surface uses the synthetic executor; an
//! institution can provide a local culture, computation, robotics, or instrument adapter through
//! the same typed trait without changing the stopping logic.

use super::super::p06_experiment_design::mechanism_validation_protocol::{
    MechanismValidationProtocolCompilation, MechanismValidationProtocolDisposition,
};
use super::execution::{
    execute_glioma_protocol, GliomaProtocolExecutor, ProtocolExecution,
    ProtocolExecutionDisposition, ProtocolExecutionError, ProtocolExecutionRequest,
    ProtocolExecutionStopReason, MAX_RETRIES,
};
use crate::glioma::workflow::FEATURE_ID;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const OUTPUT_SCHEMA: &str = "GliomaMechanismValidationExecution1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismValidationExecutionRequest {
    pub compilation: MechanismValidationProtocolCompilation,
    pub max_retries: u8,
    pub require_artifacts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismValidationExecutionDisposition {
    Completed,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismValidationExecution {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub compilation_digest: ContentHash,
    pub scheduled_arm_order: Vec<String>,
    pub withheld_arm_order: Vec<String>,
    pub task_order: Vec<String>,
    pub execution: Option<ProtocolExecution>,
    pub disposition: MechanismValidationExecutionDisposition,
    pub stop_reason: ProtocolExecutionStopReason,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismValidationExecutionError {
    #[error("mechanism validation execution request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism validation execution failed: {0}")]
    Execution(String),
    #[error("mechanism validation execution output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism validation execution digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismValidationExecution) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "compilation_digest": output.compilation_digest,
        "scheduled_arm_order": output.scheduled_arm_order,
        "withheld_arm_order": output.withheld_arm_order,
        "task_order": output.task_order,
        "execution": output.execution,
        "disposition": output.disposition,
        "stop_reason": output.stop_reason,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn validate_request(
    request: &MechanismValidationExecutionRequest,
) -> Result<(), MechanismValidationExecutionError> {
    request
        .compilation
        .validate()
        .map_err(|error| MechanismValidationExecutionError::InvalidRequest(error.to_string()))?;
    if request.max_retries > MAX_RETRIES {
        return Err(MechanismValidationExecutionError::InvalidRequest(format!(
            "max_retries must be at most {MAX_RETRIES}"
        )));
    }
    Ok(())
}

fn validate_output(
    output: &MechanismValidationExecution,
) -> Result<(), MechanismValidationExecutionError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.boundary != PRECLINICAL_BOUNDARY
        || !canonical(&output.scheduled_arm_order)
        || !canonical(&output.withheld_arm_order)
        || output
            .scheduled_arm_order
            .iter()
            .any(|arm| output.withheld_arm_order.binary_search(arm).is_ok())
        || output.task_order.iter().any(|task| task.trim().is_empty())
        || output.task_order.windows(2).any(|pair| pair[0] == pair[1])
        || output.next_action.trim().is_empty()
        || output
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
        || output.uncertainty.iter().any(|item| item.trim().is_empty())
        || output
            .execution
            .as_ref()
            .is_some_and(|execution| execution.validate().is_err())
    {
        return Err(MechanismValidationExecutionError::InvalidOutput(
            "identity, boundary, ordering, partitions, or execution fields are invalid".into(),
        ));
    }
    if output.disposition == MechanismValidationExecutionDisposition::Completed
        && output.execution.as_ref().is_none_or(|execution| {
            execution.disposition != ProtocolExecutionDisposition::Completed
        })
    {
        return Err(MechanismValidationExecutionError::InvalidOutput(
            "completed output requires a completed protocol execution".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| MechanismValidationExecutionError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(MechanismValidationExecutionError::InvalidOutput(
            "execution output is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl MechanismValidationExecution {
    pub fn validate(&self) -> Result<(), MechanismValidationExecutionError> {
        validate_output(self)
    }
}

/// Execute only a compiled, feasible mechanism-validation protocol.
pub fn execute_glioma_mechanism_validation_protocol<E: GliomaProtocolExecutor>(
    request: &MechanismValidationExecutionRequest,
    executor: &mut E,
) -> Result<MechanismValidationExecution, MechanismValidationExecutionError> {
    validate_request(request)?;
    let compilation = &request.compilation;
    let base = |execution: Option<ProtocolExecution>,
                disposition: MechanismValidationExecutionDisposition,
                stop_reason: ProtocolExecutionStopReason,
                negative_evidence: Vec<String>,
                uncertainty: Vec<String>,
                next_action: &str|
     -> Result<MechanismValidationExecution, MechanismValidationExecutionError> {
        let task_order = execution
            .as_ref()
            .map(|run| run.task_order.clone())
            .unwrap_or_default();
        let mut output = MechanismValidationExecution {
            feature_id: FEATURE_ID.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            objective: compilation.objective.clone(),
            model_system: compilation.model_system,
            compilation_digest: compilation.digest.clone(),
            scheduled_arm_order: compilation.scheduled_arm_order.clone(),
            withheld_arm_order: compilation.withheld_arm_order.clone(),
            task_order,
            execution,
            disposition,
            stop_reason,
            negative_evidence: sorted_unique(negative_evidence),
            uncertainty: sorted_unique(uncertainty),
            next_action: next_action.into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-validation-execution"),
        };
        output.digest = ContentHash::of_value(&digest_input(&output))
            .map_err(|error| MechanismValidationExecutionError::Digest(error.to_string()))?;
        output.validate()?;
        Ok(output)
    };

    if compilation.disposition != MechanismValidationProtocolDisposition::Compiled {
        let mut uncertainty = compilation.uncertainty.clone();
        uncertainty.push(format!(
            "protocol-compilation-disposition:{:?}",
            compilation.disposition
        ));
        return base(
            None,
            MechanismValidationExecutionDisposition::Blocked,
            ProtocolExecutionStopReason::ProtocolNotFeasible,
            compilation.negative_evidence.clone(),
            uncertainty,
            "repair or approve the protocol compilation before execution",
        );
    }
    let protocol = compilation.protocol.clone().ok_or_else(|| {
        MechanismValidationExecutionError::InvalidRequest(
            "compiled validation protocol has no protocol request".into(),
        )
    })?;
    let execution_request = ProtocolExecutionRequest {
        protocol,
        max_retries: request.max_retries,
        require_artifacts: request.require_artifacts,
    };
    let execution = execute_glioma_protocol(&execution_request, executor).map_err(
        |error: ProtocolExecutionError| {
            MechanismValidationExecutionError::Execution(error.to_string())
        },
    )?;
    let disposition = match execution.disposition {
        ProtocolExecutionDisposition::Completed => {
            MechanismValidationExecutionDisposition::Completed
        }
        ProtocolExecutionDisposition::Partial => MechanismValidationExecutionDisposition::Partial,
        ProtocolExecutionDisposition::Failed | ProtocolExecutionDisposition::Blocked => {
            MechanismValidationExecutionDisposition::Blocked
        }
    };
    let next_action = match disposition {
        MechanismValidationExecutionDisposition::Completed => {
            "attach measured local arm observations and re-run power-aware validation"
        }
        MechanismValidationExecutionDisposition::Partial => {
            "inspect partial task artifacts and repair dependencies before continuation"
        }
        MechanismValidationExecutionDisposition::Blocked => {
            "inspect executor failure and keep all unobserved validation arms withheld"
        }
    };
    let mut negative_evidence = compilation.negative_evidence.clone();
    negative_evidence.extend(execution.negative_evidence.clone());
    let mut uncertainty = compilation.uncertainty.clone();
    uncertainty.extend(execution.uncertainty.clone());
    base(
        Some(execution.clone()),
        disposition,
        execution.stop_reason,
        negative_evidence,
        uncertainty,
        next_action,
    )
}

#[cfg(test)]
mod tests {
    use super::super::execution::DryRunGliomaProtocolExecutor;
    use super::super::simulator::{
        simulate_glioma_protocol, ProtocolResource, ProtocolResourceKind,
        ProtocolSimulationRequest, ProtocolTask,
    };
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn seal_compilation(output: &mut MechanismValidationProtocolCompilation) {
        let input = serde_json::json!({
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
        });
        output.digest = ContentHash::of_value(&input).unwrap();
    }

    fn blocked_compilation() -> MechanismValidationProtocolCompilation {
        let mut output = MechanismValidationProtocolCompilation {
            feature_id: "GAF-GLIOMA-P06-F16".into(),
            output_schema: "GliomaMechanismValidationProtocol1@1".into(),
            objective: "execute an EGFR validation".into(),
            model_system: GliomaModelSystem::Organoid,
            validation_digest: hash("validation"),
            protocol: None,
            preflight: None,
            scheduled_arm_order: Vec::new(),
            withheld_arm_order: vec!["egfr-arm".into()],
            task_order: Vec::new(),
            disposition: MechanismValidationProtocolDisposition::Held,
            negative_evidence: vec!["efficacy-stop".into()],
            uncertainty: vec!["power-boundary".into()],
            next_action: "resolve the hold".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: hash("unsealed"),
        };
        seal_compilation(&mut output);
        output.validate().unwrap();
        output
    }

    fn compiled_compilation() -> MechanismValidationProtocolCompilation {
        let protocol = ProtocolSimulationRequest {
            objective: "execute an EGFR validation".into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![ProtocolTask {
                task_id: "validation:egfr-arm:assay".into(),
                label: "run EGFR assay".into(),
                resource_kind: ProtocolResourceKind::Culture,
                resource_units: 1,
                duration_ticks: 2,
                depends_on: Vec::new(),
                model_system: GliomaModelSystem::Organoid,
                output_schema: "GliomaMechanismValidationTask1@1".into(),
                risk_milli: 100,
                requires_instrument: false,
            }],
            resources: vec![ProtocolResource {
                resource_id: "culture".into(),
                kind: ProtocolResourceKind::Culture,
                capacity_units: 1,
            }],
            max_ticks: 10,
            max_risk_milli: 1_000,
            allow_instrument_execution: false,
            approval_reference: None,
            randomization_seed: hash("seed"),
        };
        let preflight = simulate_glioma_protocol(&protocol).unwrap();
        let mut output = MechanismValidationProtocolCompilation {
            feature_id: "GAF-GLIOMA-P06-F16".into(),
            output_schema: "GliomaMechanismValidationProtocol1@1".into(),
            objective: protocol.objective.clone(),
            model_system: protocol.model_system,
            validation_digest: hash("validation"),
            protocol: Some(protocol),
            preflight: Some(preflight),
            scheduled_arm_order: vec!["egfr-arm".into()],
            withheld_arm_order: vec!["control".into()],
            task_order: vec!["validation:egfr-arm:assay".into()],
            disposition: MechanismValidationProtocolDisposition::Compiled,
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            next_action: "execute".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: hash("unsealed"),
        };
        seal_compilation(&mut output);
        output.validate().unwrap();
        output
    }

    #[test]
    fn held_validation_is_blocked_without_touching_executor() {
        let request = MechanismValidationExecutionRequest {
            compilation: blocked_compilation(),
            max_retries: 1,
            require_artifacts: true,
        };
        let mut executor = DryRunGliomaProtocolExecutor;
        let output = execute_glioma_mechanism_validation_protocol(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            MechanismValidationExecutionDisposition::Blocked
        );
        assert!(output.execution.is_none());
        output.validate().unwrap();
    }

    #[test]
    fn compiled_validation_runs_through_local_executor() {
        let request = MechanismValidationExecutionRequest {
            compilation: compiled_compilation(),
            max_retries: 1,
            require_artifacts: true,
        };
        let mut executor = DryRunGliomaProtocolExecutor;
        let output = execute_glioma_mechanism_validation_protocol(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            MechanismValidationExecutionDisposition::Completed
        );
        assert_eq!(output.execution.as_ref().unwrap().completed_order.len(), 1);
        assert!(output.execution.as_ref().unwrap().task_results[0]
            .artifact
            .is_some());
        output.validate().unwrap();
    }
}
