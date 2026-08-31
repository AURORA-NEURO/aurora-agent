//! Contract-bound research execution and replay bundle.
//!
//! `WorldTape` already proves what the runtime did to its virtual world. This module binds that
//! execution evidence to the shared AURORA research contracts: a versioned capability, a typed
//! workflow, an autonomy grant, and a policy receipt. The result is a small production boundary
//! that can be called by the CLI, SDK, MCP adapter, or an institution-local instrument gateway
//! without each surface inventing its own authorization and replay rules.

use crate::tape::MAX_TAPE_CHECKPOINTS;
use crate::{Effect, RuntimeError, WorldTape};
use bioprism_foundation::{
    AutonomyGrant, CapabilityManifest, Effect as ResearchEffect, ExecutionEvent, ExecutionRun,
    ExecutionStatus, PolicyDecision, PolicyReceipt, ResearchContractError, ResearchWorkflowSpec,
    TypedResearchArtifact,
};
use bioprism_ids::{ContentHash, RunId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use thiserror::Error;

/// Atlas feature implemented by this module.
pub const FEATURE_ID: &str = "AFA-runtime-P12-F01";
pub const FEATURE_CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, Error)]
pub enum ResearchRuntimeError {
    #[error("research contract rejected the run: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("runtime tape rejected the effect: {0}")]
    Runtime(#[from] RuntimeError),
    #[error("effect {effect:?} is not declared by capability {capability}")]
    UndeclaredEffect {
        capability: String,
        effect: ResearchEffect,
    },
    #[error("actor {actor} is not authorized for effect {effect:?}")]
    UnauthorizedEffect {
        actor: String,
        effect: ResearchEffect,
    },
    #[error("policy decision {0:?} cannot execute a research run")]
    PolicyBlocked(PolicyDecision),
    #[error("instrument execution needs a signed or content-addressed evidence payload")]
    MissingInstrumentEvidence,
    #[error("checkpoint {0} is already present in the execution session")]
    DuplicateCheckpoint(String),
    #[error("session is already closed")]
    Closed,
    #[error("cannot serialize replay bundle: {0}")]
    Serialization(String),
}

/// The portable audit object emitted by [`ResearchExecutionSession::bundle`]. Raw experiment data
/// is never included; the tape and typed artifact refer to content by digest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchReplayBundle {
    pub feature_id: String,
    pub manifest: CapabilityManifest,
    pub workflow: ResearchWorkflowSpec,
    pub grant: AutonomyGrant,
    pub policy: PolicyReceipt,
    pub run: ExecutionRun,
    pub tape_json: String,
    pub result_artifact: Option<TypedResearchArtifact>,
    pub boundary: String,
}

impl ResearchReplayBundle {
    pub fn digest(&self) -> Result<ContentHash, ResearchRuntimeError> {
        self.verify()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))
    }

    /// Reloads and verifies the hash-chained tape before exposing a bundle to a consumer.
    pub fn verify(&self) -> Result<(), ResearchRuntimeError> {
        if self.feature_id != FEATURE_ID
            || self.boundary != bioprism_foundation::PRECLINICAL_BOUNDARY
        {
            return Err(ResearchRuntimeError::Serialization(
                "replay bundle feature or boundary identity mismatch".into(),
            ));
        }
        self.manifest.validate()?;
        self.workflow.validate()?;
        self.grant.validate()?;
        self.policy.validate()?;
        self.run.validate()?;
        if self.run.schema_version != bioprism_foundation::RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.run.workflow_id != self.workflow.workflow_id
            || self.run.boundary != bioprism_foundation::PRECLINICAL_BOUNDARY
        {
            return Err(ResearchRuntimeError::Serialization(
                "replay bundle run metadata does not match its workflow".into(),
            ));
        }
        let workflow_value = serde_json::to_value(&self.workflow)
            .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))?;
        let expected_plan_hash = ContentHash::of_value(&workflow_value)
            .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))?;
        if self.run.plan_hash != expected_plan_hash
            || self.run.replay_identity
                != ContentHash::of_bytes(
                    format!("{}:{}", self.run.workflow_id, self.run.plan_hash).as_bytes(),
                )
        {
            return Err(ResearchRuntimeError::Serialization(
                "replay bundle plan or replay identity is not workflow-bound".into(),
            ));
        }
        let tape = WorldTape::from_json(&self.tape_json)?;
        if tape.run() != &self.run.run_id {
            return Err(ResearchRuntimeError::Serialization(
                "tape/run identity mismatch".into(),
            ));
        }
        if tape.len() != self.run.events.len() as u64 {
            return Err(ResearchRuntimeError::Serialization(
                "tape/event length mismatch".into(),
            ));
        }
        for (sequence, event) in self.run.events.iter().enumerate() {
            if event.sequence != sequence as u64 || event.effect.is_none() {
                return Err(ResearchRuntimeError::Serialization(
                    "execution event sequence or effect identity is invalid".into(),
                ));
            }
        }
        if tape.checkpoints().len() != self.run.checkpoints.len() {
            return Err(ResearchRuntimeError::Serialization(
                "tape/run checkpoint count mismatch".into(),
            ));
        }
        let mut checkpoint_ids = BTreeSet::new();
        for checkpoint in &self.run.checkpoints {
            if checkpoint.checkpoint_id.trim().is_empty()
                || !checkpoint_ids.insert(checkpoint.checkpoint_id.clone())
                || checkpoint.event_sequence > self.run.events.len() as u64
            {
                return Err(ResearchRuntimeError::Serialization(
                    "execution checkpoint identity or sequence is invalid".into(),
                ));
            }
        }
        for (run_checkpoint, tape_checkpoint) in self.run.checkpoints.iter().zip(tape.checkpoints())
        {
            if run_checkpoint.event_sequence != tape_checkpoint.step {
                return Err(ResearchRuntimeError::Serialization(
                    "tape/run checkpoint sequence mismatch".into(),
                ));
            }
            let end = usize::try_from(run_checkpoint.event_sequence).map_err(|_| {
                ResearchRuntimeError::Serialization(
                    "replay checkpoint sequence exceeds addressable memory".into(),
                )
            })?;
            let event_value = serde_json::to_value(&self.run.events[..end])
                .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))?;
            let expected_replay_hash = ContentHash::of_value(&event_value)
                .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))?;
            if run_checkpoint.replay_hash != expected_replay_hash {
                return Err(ResearchRuntimeError::Serialization(
                    "execution checkpoint replay hash mismatch".into(),
                ));
            }
        }
        Ok(())
    }
}

/// A policy- and authority-bound execution session.
#[derive(Debug, Clone)]
pub struct ResearchExecutionSession {
    manifest: CapabilityManifest,
    workflow: ResearchWorkflowSpec,
    grant: AutonomyGrant,
    policy: PolicyReceipt,
    run: ExecutionRun,
    tape: WorldTape,
    result_artifact: Option<TypedResearchArtifact>,
}

impl ResearchExecutionSession {
    pub fn new(
        manifest: CapabilityManifest,
        workflow: ResearchWorkflowSpec,
        grant: AutonomyGrant,
        policy: PolicyReceipt,
        run_id: RunId,
    ) -> Result<Self, ResearchRuntimeError> {
        manifest.validate()?;
        workflow.validate()?;
        grant.validate()?;
        policy.validate()?;
        if matches!(
            policy.decision,
            PolicyDecision::Deny
                | PolicyDecision::Redact
                | PolicyDecision::ApprovalRequired
                | PolicyDecision::Unresolved
        ) {
            return Err(ResearchRuntimeError::PolicyBlocked(policy.decision));
        }
        if workflow.autonomy_tier > grant.autonomy_tier {
            return Err(ResearchRuntimeError::Contract(
                ResearchContractError::MissingAuthority {
                    item: workflow.workflow_id.clone(),
                    tier: workflow.autonomy_tier,
                },
            ));
        }
        let workflow_value = serde_json::to_value(&workflow)
            .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))?;
        let plan_hash = ContentHash::of_value(&workflow_value)
            .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))?;
        let run = ExecutionRun::planned(run_id.clone(), workflow.workflow_id.clone(), plan_hash)?;
        Ok(Self {
            manifest,
            workflow,
            grant,
            policy,
            run,
            tape: WorldTape::new(run_id),
            result_artifact: None,
        })
    }

    pub fn manifest(&self) -> &CapabilityManifest {
        &self.manifest
    }
    pub fn workflow(&self) -> &ResearchWorkflowSpec {
        &self.workflow
    }
    pub fn run(&self) -> &ExecutionRun {
        &self.run
    }
    pub fn tape(&self) -> &WorldTape {
        &self.tape
    }

    /// Records one runtime effect and its typed research role in a single append-only step.
    pub fn append_effect(
        &mut self,
        event_type: impl Into<String>,
        research_effect: ResearchEffect,
        effect: Effect,
        evidence_payload: Option<&Value>,
    ) -> Result<u64, ResearchRuntimeError> {
        if matches!(
            self.run.status,
            ExecutionStatus::Succeeded | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        ) {
            return Err(ResearchRuntimeError::Closed);
        }
        if !self.manifest.effects.contains(&research_effect) {
            return Err(ResearchRuntimeError::UndeclaredEffect {
                capability: self.manifest.capability_id.clone(),
                effect: research_effect,
            });
        }
        let action = effect_name(research_effect);
        if !self.grant.permitted_actions.contains("*")
            && !self.grant.permitted_actions.contains(&action)
        {
            return Err(ResearchRuntimeError::UnauthorizedEffect {
                actor: self.grant.actor.clone(),
                effect: research_effect,
            });
        }
        if research_effect == ResearchEffect::InstrumentExecution && evidence_payload.is_none() {
            return Err(ResearchRuntimeError::MissingInstrumentEvidence);
        }
        let event_type = event_type.into();
        if event_type.trim().is_empty() {
            return Err(ResearchRuntimeError::Contract(
                ResearchContractError::MissingField {
                    field: "event_type",
                },
            ));
        }
        let payload_hash = evidence_payload
            .map(|payload| {
                ContentHash::of_value(payload)
                    .map_err(|error| ResearchRuntimeError::Serialization(error.to_string()))
            })
            .transpose()?;
        let sequence = self.run.events.len() as u64;
        // The tape is appended first only after all contract checks. Its append is deterministic;
        // if serialization fails, no execution event is admitted.
        self.tape.append(effect)?;
        self.run.append_event(ExecutionEvent {
            sequence,
            event_type,
            effect: Some(research_effect),
            payload_hash,
        })?;
        Ok(sequence)
    }

    pub fn attach_result(
        &mut self,
        artifact: TypedResearchArtifact,
    ) -> Result<(), ResearchRuntimeError> {
        artifact.validate_metadata()?;
        self.result_artifact = Some(artifact);
        Ok(())
    }

    pub fn checkpoint(
        &mut self,
        checkpoint_id: impl Into<String>,
    ) -> Result<(), ResearchRuntimeError> {
        if matches!(
            self.run.status,
            ExecutionStatus::Succeeded | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        ) {
            return Err(ResearchRuntimeError::Closed);
        }
        let checkpoint_id = checkpoint_id.into();
        if checkpoint_id.trim().is_empty() {
            return Err(ResearchRuntimeError::Serialization(
                "checkpoint identity must be non-empty".into(),
            ));
        }
        if self
            .run
            .checkpoints
            .iter()
            .any(|checkpoint| checkpoint.checkpoint_id == checkpoint_id)
        {
            return Err(ResearchRuntimeError::DuplicateCheckpoint(checkpoint_id));
        }
        if self.run.checkpoints.len() != self.tape.checkpoints().len() {
            return Err(ResearchRuntimeError::Serialization(
                "execution and tape checkpoint ledgers are already out of sync".into(),
            ));
        }
        if self.tape.checkpoints().len() >= MAX_TAPE_CHECKPOINTS {
            return Err(ResearchRuntimeError::Runtime(
                RuntimeError::TapeLimitExceeded {
                    kind: "checkpoints",
                    actual: self.tape.checkpoints().len().saturating_add(1),
                    maximum: MAX_TAPE_CHECKPOINTS,
                },
            ));
        }
        self.run.checkpoint(checkpoint_id)?;
        self.tape.checkpoint(
            "aurora-research-runtime",
            crate::RestorationDeclaration::portable(),
        )?;
        Ok(())
    }

    pub fn finish(&mut self, status: ExecutionStatus) -> Result<(), ResearchRuntimeError> {
        self.tape.verify_chain()?;
        if status == ExecutionStatus::Succeeded && self.run.events.is_empty() {
            return Err(ResearchRuntimeError::Serialization(
                "successful run has no execution evidence".into(),
            ));
        }
        self.run.finish(status)?;
        Ok(())
    }

    pub fn bundle(&self) -> Result<ResearchReplayBundle, ResearchRuntimeError> {
        Ok(ResearchReplayBundle {
            feature_id: FEATURE_ID.into(),
            manifest: self.manifest.clone(),
            workflow: self.workflow.clone(),
            grant: self.grant.clone(),
            policy: self.policy.clone(),
            run: self.run.clone(),
            tape_json: self.tape.to_json()?,
            result_artifact: self.result_artifact.clone(),
            boundary: bioprism_foundation::PRECLINICAL_BOUNDARY.into(),
        })
    }
}

fn effect_name(effect: ResearchEffect) -> String {
    match effect {
        ResearchEffect::ReadLocalData => "read_local_data",
        ResearchEffect::WriteLocalArtifact => "write_local_artifact",
        ResearchEffect::ExecuteLocalComputation => "execute_local_computation",
        ResearchEffect::ExternalDataAccess => "external_data_access",
        ResearchEffect::FederationExport => "federation_export",
        ResearchEffect::InstrumentExecution => "instrument_execution",
        ResearchEffect::ConsumeMaterial => "consume_material",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::{
        AutonomyTier, Determinism, Effect as ResearchEffect, EvidenceReference, EvidenceState,
        ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    fn fixture() -> (
        CapabilityManifest,
        ResearchWorkflowSpec,
        AutonomyGrant,
        PolicyReceipt,
    ) {
        let manifest = CapabilityManifest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            capability_id: FEATURE_ID.into(),
            version: FEATURE_CONTRACT_VERSION.into(),
            owner_crate: "runtime".into(),
            consumers: ["research workflow operator".into()].into(),
            behavior: "executes a bounded typed research workflow with replay evidence".into(),
            value: "keeps effects, policy and provenance joined".into(),
            inputs: vec![TypedPort {
                name: "plan".into(),
                schema: "ResearchWorkflowSpec@1".into(),
                required: true,
            }],
            outputs: vec![TypedPort {
                name: "bundle".into(),
                schema: "ResearchReplayBundle@1".into(),
                required: true,
            }],
            effects: [
                ResearchEffect::ExecuteLocalComputation,
                ResearchEffect::WriteLocalArtifact,
            ]
            .into(),
            permissions: ["execute:local".into()].into(),
            determinism: Determinism::ByteStable,
            evidence: vec![EvidenceReference {
                source_id: "fixture:runtime-session".into(),
                state: EvidenceState::Supported,
                locator: None,
            }],
            authority_requirements: Vec::new(),
            autonomy_tier: AutonomyTier::A0,
            surfaces: [
                ResearchSurface::Cli,
                ResearchSurface::Api,
                ResearchSurface::Sdk,
            ]
            .into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let workflow = ResearchWorkflowSpec {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "wf-knowledge-run".into(),
            intent: "compile local evidence".into(),
            nodes: vec![bioprism_foundation::WorkflowNode {
                node_id: "compile".into(),
                capability_id: FEATURE_ID.into(),
                actor: "operator".into(),
                requires_approval: false,
            }],
            edges: Vec::new(),
            checkpoints: Vec::new(),
            budgets: vec![bioprism_foundation::ResourceBudget {
                resource: "cpu_ms".into(),
                amount: 1000.0,
            }],
            compensations: Vec::new(),
            approvals: Vec::new(),
            autonomy_tier: AutonomyTier::A0,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let grant = AutonomyGrant {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            actor: "operator".into(),
            permitted_actions: ["execute_local_computation".into()].into(),
            resource_budget: BTreeMap::from([(String::from("cpu_ms"), 1000.0)]),
            scope: "study:fixture".into(),
            expires_at: "2099-01-01T00:00:00Z".into(),
            revoked: false,
            autonomy_tier: AutonomyTier::A0,
            approval_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let policy = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy:wf-knowledge-run".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["fixture approved".into()],
            evaluated_artifacts: Vec::new(),
            authority_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        (manifest, workflow, grant, policy)
    }

    #[test]
    fn denied_effect_does_not_mutate_tape_or_run() {
        let (manifest, workflow, grant, policy) = fixture();
        let run_id = RunId::parse("runtime-session-1").unwrap();
        let mut session =
            ResearchExecutionSession::new(manifest, workflow, grant, policy, run_id).unwrap();
        let before = (session.tape().len(), session.run().events.len());
        let effect = Effect::performed(
            crate::EffectRequest::FileWrite {
                path: "result.json".into(),
                content: "{}".into(),
            },
            crate::EffectOutcome::new(json!({"ok": true})),
        );
        let evidence = json!({"signed": true});
        let error = session
            .append_effect(
                "write",
                ResearchEffect::InstrumentExecution,
                effect,
                Some(&evidence),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            ResearchRuntimeError::UndeclaredEffect { .. }
        ));
        assert_eq!(before, (session.tape().len(), session.run().events.len()));
    }

    #[test]
    fn bundle_is_replayable_and_digest_stable() {
        let (manifest, workflow, grant, policy) = fixture();
        let mut a = ResearchExecutionSession::new(
            manifest.clone(),
            workflow.clone(),
            grant.clone(),
            policy.clone(),
            RunId::parse("runtime-session-2").unwrap(),
        )
        .unwrap();
        let mut b = ResearchExecutionSession::new(
            manifest,
            workflow,
            grant,
            policy,
            RunId::parse("runtime-session-2").unwrap(),
        )
        .unwrap();
        for session in [&mut a, &mut b] {
            let effect = Effect::performed(
                crate::EffectRequest::ModelCall {
                    model: "fixture".into(),
                    prompt: "compile".into(),
                },
                crate::EffectOutcome::new(json!({"answer": "unknown"})),
            );
            let evidence = json!({"answer": "unknown"});
            session
                .append_effect(
                    "compile",
                    ResearchEffect::ExecuteLocalComputation,
                    effect,
                    Some(&evidence),
                )
                .unwrap();
            session.checkpoint("compile-complete").unwrap();
            session.finish(ExecutionStatus::Succeeded).unwrap();
        }
        let left = a.bundle().unwrap();
        let right = b.bundle().unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        left.verify().unwrap();

        let mut tampered = left.clone();
        tampered.run.events[0].sequence = 9;
        assert!(tampered.verify().is_err());
    }

    #[test]
    fn checkpoints_are_unique_and_cannot_be_added_after_finish() {
        let (manifest, workflow, grant, policy) = fixture();
        let mut session = ResearchExecutionSession::new(
            manifest,
            workflow,
            grant,
            policy,
            RunId::parse("runtime-session-checkpoint-boundary").unwrap(),
        )
        .unwrap();
        session.checkpoint("admission").unwrap();
        assert!(matches!(
            session.checkpoint("admission").unwrap_err(),
            ResearchRuntimeError::DuplicateCheckpoint(_)
        ));
        session.finish(ExecutionStatus::Failed).unwrap();
        assert!(matches!(
            session.checkpoint("after-finish").unwrap_err(),
            ResearchRuntimeError::Closed
        ));
    }
}
