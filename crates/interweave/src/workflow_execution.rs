//! Domain-neutral binding between a reference workflow and an adaptive execution plan.
//!
//! A workflow catalogue is not an executor. This module supplies the missing typed seam without
//! claiming that it can schedule participants, approve a release, or perform an external effect.
//! A [`WorkflowExecutionBinding`] binds a workflow identity, its declared effect prohibitions,
//! caller-supplied capabilities, provider identity, and the digest of a validated epistemic plan.
//! Its receipt then preserves the adaptive execution receipt and can be replayed from that receipt
//! alone. The effect envelope is descriptive admission metadata; a real release authority must
//! still enforce it before any non-read-only action.

use crate::workflow::WorkflowId;
use bioprism_epistemic::{
    AcquisitionExecutor, AdaptiveExecutionError, AdaptiveExecutionReceipt, AdaptivePlan,
    ExecutionGrant,
};
use bioprism_fabric::effect::EffectKind;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;

/// Wire/schema identifier for a workflow-bound execution receipt.
pub const WORKFLOW_EXECUTION_SCHEMA: &str = "bioprism-interweave/workflow-execution/0.1";

/// A validated plan bound to one of the six reference workflow identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionBinding {
    pub schema: String,
    pub workflow: WorkflowId,
    pub workflow_spec_digest: String,
    pub adaptive_plan_digest: String,
    pub provider_id: String,
    pub required_capabilities: BTreeSet<String>,
    pub forbidden_effects: Vec<EffectKind>,
    pub binding_digest: String,
}

/// Receipt envelope preserving workflow identity around the lower-level adaptive receipt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowExecutionReceipt {
    pub schema: String,
    pub workflow: WorkflowId,
    pub binding_digest: String,
    pub adaptive: AdaptiveExecutionReceipt,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkflowExecutionError {
    #[error("adaptive execution contract is invalid: {0}")]
    Adaptive(#[from] AdaptiveExecutionError),
    #[error("workflow execution field {field} must be non-empty")]
    EmptyField { field: &'static str },
    #[error("workflow execution field {field} contains an empty capability")]
    EmptyCapability { field: &'static str },
    #[error("workflow execution digest is malformed or cannot be computed: {0}")]
    Digest(String),
    #[error("workflow execution receipt is invalid: {0}")]
    InvalidReceipt(String),
    #[error("workflow execution receipt belongs to a different binding")]
    BindingMismatch,
}

impl WorkflowExecutionBinding {
    /// Creates a binding without executing the plan or contacting a provider.
    pub fn bind(
        workflow: WorkflowId,
        plan: &AdaptivePlan,
        provider_id: impl Into<String>,
        required_capabilities: impl IntoIterator<Item = String>,
    ) -> Result<Self, WorkflowExecutionError> {
        plan.validate()?;
        let provider_id = provider_id.into();
        if provider_id.trim().is_empty() {
            return Err(WorkflowExecutionError::EmptyField {
                field: "provider_id",
            });
        }
        let required_capabilities: BTreeSet<String> = required_capabilities.into_iter().collect();
        if required_capabilities
            .iter()
            .any(|value| value.trim().is_empty())
        {
            return Err(WorkflowExecutionError::EmptyCapability {
                field: "required_capabilities",
            });
        }
        let adaptive_plan_digest = plan.digest()?;
        let workflow_spec_digest = workflow_spec_digest(workflow)?;
        let forbidden_effects = workflow.forbidden_effects().to_vec();
        let binding_digest = binding_identity_digest(
            workflow,
            &workflow_spec_digest,
            &adaptive_plan_digest,
            &provider_id,
            &required_capabilities,
            &forbidden_effects,
        )?;
        Ok(Self {
            schema: WORKFLOW_EXECUTION_SCHEMA.into(),
            workflow,
            workflow_spec_digest,
            adaptive_plan_digest,
            provider_id,
            required_capabilities,
            forbidden_effects,
            binding_digest,
        })
    }

    /// The binding's canonical identity digest.
    pub fn digest(&self) -> &str {
        &self.binding_digest
    }

    /// Executes only through the lower-level grant/provider seam. No workflow effect is inferred
    /// or performed by this wrapper.
    pub fn execute<E: AcquisitionExecutor>(
        &self,
        plan: &AdaptivePlan,
        grant: &ExecutionGrant,
        executor: &mut E,
    ) -> Result<WorkflowExecutionReceipt, WorkflowExecutionError> {
        self.validate_against(plan)?;
        if executor.provider_id() != self.provider_id {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        let adaptive = plan.execute(Some(grant), executor)?;
        let receipt = WorkflowExecutionReceipt {
            schema: WORKFLOW_EXECUTION_SCHEMA.into(),
            workflow: self.workflow,
            binding_digest: self.binding_digest.clone(),
            adaptive,
        };
        receipt.validate_against(self)?;
        Ok(receipt)
    }

    /// Replays a stored receipt through the receipt-only executor. A mismatched binding or plan is
    /// rejected before the replay can be reported as successful.
    pub fn replay(
        &self,
        plan: &AdaptivePlan,
        receipt: &WorkflowExecutionReceipt,
    ) -> Result<WorkflowExecutionReceipt, WorkflowExecutionError> {
        self.validate_against(plan)?;
        receipt.validate_against(self)?;
        let adaptive = plan.replay(&receipt.adaptive)?;
        let replay = WorkflowExecutionReceipt {
            schema: WORKFLOW_EXECUTION_SCHEMA.into(),
            workflow: self.workflow,
            binding_digest: self.binding_digest.clone(),
            adaptive,
        };
        replay.validate_against(self)?;
        Ok(replay)
    }

    fn validate_against(&self, plan: &AdaptivePlan) -> Result<(), WorkflowExecutionError> {
        if self.schema != WORKFLOW_EXECUTION_SCHEMA {
            return Err(WorkflowExecutionError::InvalidReceipt(
                "binding schema is not the workflow execution schema".into(),
            ));
        }
        if self.provider_id.trim().is_empty()
            || self
                .required_capabilities
                .iter()
                .any(|value| value.trim().is_empty())
        {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        if self.binding_digest
            != binding_identity_digest(
                self.workflow,
                &self.workflow_spec_digest,
                &self.adaptive_plan_digest,
                &self.provider_id,
                &self.required_capabilities,
                &self.forbidden_effects,
            )?
        {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        let plan_digest = plan.digest()?;
        if self.adaptive_plan_digest != plan_digest {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        if self.workflow_spec_digest != workflow_spec_digest(self.workflow)? {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        if self.forbidden_effects != self.workflow.forbidden_effects().to_vec() {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        Ok(())
    }
}

impl WorkflowExecutionReceipt {
    pub fn validate_against(
        &self,
        binding: &WorkflowExecutionBinding,
    ) -> Result<(), WorkflowExecutionError> {
        if self.schema != WORKFLOW_EXECUTION_SCHEMA
            || self.workflow != binding.workflow
            || self.binding_digest != binding.binding_digest
        {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        self.adaptive.validate_shape()?;
        if self.adaptive.plan_digest != binding.adaptive_plan_digest {
            return Err(WorkflowExecutionError::BindingMismatch);
        }
        Ok(())
    }

    /// Count the underlying provenance classes without collapsing simulated or replayed work into
    /// observed evidence.
    pub fn provenance_counts(&self) -> (usize, usize, usize) {
        self.adaptive.provenance_counts()
    }

    /// Whether the underlying adaptive plan reached a terminal action.
    pub fn is_completed(&self) -> bool {
        self.adaptive.is_completed()
    }

    /// Explicitly reports the release posture of this crate: no external release authority exists.
    pub fn release_posture(&self) -> &'static str {
        "workflow_receipt_only_external_release_not_authorized"
    }
}

fn workflow_spec_digest(workflow: WorkflowId) -> Result<String, WorkflowExecutionError> {
    digest_value(&json!({
        "workflow": workflow,
        "number": workflow.number(),
        "roles": workflow.roles(),
        "distinctive_behaviours": workflow.distinctive_behaviours(),
        "forbidden_effects": workflow.forbidden_effects(),
    }))
}

fn binding_identity_digest(
    workflow: WorkflowId,
    workflow_spec_digest: &str,
    adaptive_plan_digest: &str,
    provider_id: &str,
    required_capabilities: &BTreeSet<String>,
    forbidden_effects: &[EffectKind],
) -> Result<String, WorkflowExecutionError> {
    digest_value(&json!({
        "schema": WORKFLOW_EXECUTION_SCHEMA,
        "workflow": workflow,
        "workflow_spec_digest": workflow_spec_digest,
        "adaptive_plan_digest": adaptive_plan_digest,
        "provider_id": provider_id,
        "required_capabilities": required_capabilities,
        "forbidden_effects": forbidden_effects,
    }))
}

fn digest_value(value: &serde_json::Value) -> Result<String, WorkflowExecutionError> {
    ContentHash::of_value(value)
        .map(|hash| hash.as_str().to_string())
        .map_err(|error| WorkflowExecutionError::Digest(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_epistemic::{Acquisition, Belief, DecisionProblem, ScriptedExecutor};

    fn plan() -> AdaptivePlan {
        let problem = DecisionProblem::new(
            vec!["hold".into(), "release".into()],
            vec!["safe".into(), "unsafe".into()],
            vec![0.0, 2.0, 2.0, 0.0],
        )
        .expect("problem");
        let belief = Belief::new(vec![0.6, 0.4]).expect("belief");
        let acquisition = Acquisition::new(
            "screen",
            0.1,
            vec![
                bioprism_epistemic::Outcome::new("negative", vec![0.9, 0.2]),
                bioprism_epistemic::Outcome::new("positive", vec![0.1, 0.8]),
            ],
            2,
        )
        .expect("acquisition");
        AdaptivePlan::new(problem, belief, vec![acquisition], 1.0, 1).expect("plan")
    }

    #[test]
    fn binding_executes_and_replays_with_explicit_workflow_identity() {
        let plan = plan();
        let binding = WorkflowExecutionBinding::bind(
            WorkflowId::ReliableSoftwareRepair,
            &plan,
            "workflow-simulator",
            ["repository.read".into(), "tests.run".into()],
        )
        .expect("binding");
        let digest = plan.digest().expect("digest");
        let grant = ExecutionGrant::issue("grant", &digest, "workflow-simulator").expect("grant");
        let mut executor = ScriptedExecutor::simulated(
            "workflow-simulator",
            vec![("screen".into(), "negative".into())],
        );
        let receipt = binding
            .execute(&plan, &grant, &mut executor)
            .expect("execution");
        assert!(receipt.is_completed());
        assert_eq!(receipt.provenance_counts(), (0, 1, 0));
        assert_eq!(
            receipt.release_posture(),
            "workflow_receipt_only_external_release_not_authorized"
        );
        let replay = binding.replay(&plan, &receipt).expect("replay");
        assert_eq!(replay.provenance_counts(), (0, 0, 1));
    }

    #[test]
    fn binding_rejects_a_receipt_from_another_workflow() {
        let plan = plan();
        let binding = WorkflowExecutionBinding::bind(
            WorkflowId::ReliableSoftwareRepair,
            &plan,
            "workflow-simulator",
            std::iter::empty(),
        )
        .expect("binding");
        let other = WorkflowExecutionBinding::bind(
            WorkflowId::IncidentResponse,
            &plan,
            "workflow-simulator",
            std::iter::empty(),
        )
        .expect("other binding");
        let digest = plan.digest().expect("digest");
        let grant = ExecutionGrant::issue("grant", &digest, "workflow-simulator").expect("grant");
        let mut executor = ScriptedExecutor::simulated(
            "workflow-simulator",
            vec![("screen".into(), "negative".into())],
        );
        let receipt = other
            .execute(&plan, &grant, &mut executor)
            .expect("execution");
        assert!(matches!(
            binding.replay(&plan, &receipt),
            Err(WorkflowExecutionError::BindingMismatch)
        ));
    }
}
