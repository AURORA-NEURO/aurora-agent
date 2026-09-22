//! Bridge adaptive P07 actions to the typed glioma stage execution contract.
//!
//! Institution-local [`GliomaStageExecutor`] implementations can be used directly by the
//! autonomous director through this adapter. It binds actions to one admitted plan and requires
//! source and direct-prerequisite references before dispatch; payload bytes never leave the local
//! artifact store. Implements the execution seam for `GAF-GLIOMA-P07-F32` (the P07 research
//! director) without claiming that any institution-specific scientific provider is bundled.

use super::action_execution::{
    ActionExecutionDisposition, ActionExecutionFailure, ActionExecutionResult,
    GliomaActionExecutionContext, GliomaActionExecutor, GliomaActionWorkflowScope,
};
use super::director::stage_modality;
use crate::glioma_engine::{
    compile_glioma_research, GliomaActionCandidate, GliomaEngineError, GliomaPlanDisposition,
    GliomaResearchIntent, GliomaResearchPlan, GliomaStage, GliomaStageDisposition,
    GliomaStageExecutor, GliomaStageFailure, GliomaStageInput, GliomaStageOutput, LocalArtifactRef,
    StageReadiness,
};
use bioprism_foundation::PRECLINICAL_BOUNDARY;

pub struct GliomaStageActionExecutor<'a, E> {
    plan: GliomaResearchPlan,
    scope: GliomaActionWorkflowScope,
    source_artifacts: Vec<LocalArtifactRef>,
    stage_executor: &'a mut E,
}

fn scope_from_intent(intent: &GliomaResearchIntent) -> GliomaActionWorkflowScope {
    GliomaActionWorkflowScope {
        research_id: intent.research_id.clone(),
        study_id: intent.study_id.clone(),
        objective: intent.objective.clone(),
        modalities: intent.modalities.iter().copied().collect(),
        model_systems: intent.model_systems.iter().copied().collect(),
        requested_autonomy: intent.requested_autonomy,
    }
}

fn failure(reason: impl Into<String>, retryable: bool) -> ActionExecutionFailure {
    ActionExecutionFailure {
        reason: reason.into(),
        retryable,
    }
}

fn bridge_result(
    candidate: &GliomaActionCandidate,
    stage: &GliomaStage,
    output: GliomaStageOutput,
    attempt: u8,
) -> Result<ActionExecutionResult, ActionExecutionFailure> {
    output.artifact.validate_metadata().map_err(|error| {
        failure(
            format!("stage artifact metadata is invalid: {error}"),
            false,
        )
    })?;
    if output.artifact.content_type != stage.output_schema
        || output.artifact.boundary != PRECLINICAL_BOUNDARY
        || output.uncertainty.iter().any(|item| item.trim().is_empty())
        || output
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
    {
        return Err(failure(
            "stage worker returned a schema-mismatched or out-of-boundary artifact",
            false,
        ));
    }
    let (disposition, note) = match output.disposition {
        GliomaStageDisposition::Completed => (
            ActionExecutionDisposition::Completed,
            "typed glioma stage completed in the institution-local executor".to_string(),
        ),
        GliomaStageDisposition::Negative => (
            ActionExecutionDisposition::Negative,
            "typed glioma stage returned a valid negative research result".to_string(),
        ),
        GliomaStageDisposition::Partial => (
            ActionExecutionDisposition::Partial,
            "typed glioma stage returned partial work; dependent actions will not dispatch".into(),
        ),
        GliomaStageDisposition::Blocked => {
            return Err(failure(
                "typed glioma stage worker blocked execution",
                false,
            ));
        }
    };
    Ok(ActionExecutionResult {
        action_id: candidate.action_id.clone(),
        disposition,
        attempt_count: attempt,
        artifact: Some(LocalArtifactRef {
            artifact_id: output.artifact.artifact_id,
            content_hash: output.artifact.content_hash,
            content_type: output.artifact.content_type,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }),
        note,
        uncertainty: output.uncertainty,
        negative_evidence: output.negative_evidence,
    })
}

impl<'a, E: GliomaStageExecutor> GliomaStageActionExecutor<'a, E> {
    /// Compile and bind one intent to an institution-local typed stage worker.
    pub fn new(
        intent: &GliomaResearchIntent,
        stage_executor: &'a mut E,
    ) -> Result<Self, GliomaEngineError> {
        let plan = compile_glioma_research(intent)?;
        if plan.disposition != GliomaPlanDisposition::Admitted {
            return Err(GliomaEngineError::InvalidPlan(
                "stage-executor adapter requires an admitted glioma research plan".into(),
            ));
        }
        let mut source_artifacts = intent.input_artifacts.clone();
        source_artifacts.sort_by(|left, right| {
            left.artifact_id
                .cmp(&right.artifact_id)
                .then_with(|| left.content_hash.cmp(&right.content_hash))
        });
        Ok(Self {
            plan,
            scope: scope_from_intent(intent),
            source_artifacts,
            stage_executor,
        })
    }

    fn execute_with_context(
        &mut self,
        candidate: &GliomaActionCandidate,
        context: &GliomaActionExecutionContext,
        attempt: u8,
    ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
        if context.scope.as_ref() != Some(&self.scope)
            || context.source_artifacts != self.source_artifacts
        {
            return Err(failure(
                "action scope or local source artifacts differ from the compiled intent",
                false,
            ));
        }
        let stage = self
            .plan
            .stages
            .iter()
            .find(|stage| stage.stage_id == candidate.action_id)
            .ok_or_else(|| failure("action is absent from the compiled glioma plan", false))?;
        let dependency_artifact_order = context
            .dependency_artifacts
            .iter()
            .map(|dependency| dependency.action_id.clone())
            .collect::<Vec<_>>();
        if candidate.stage_kind != stage.kind
            || candidate.modality != stage_modality(stage.kind)
            || candidate.cost_units != stage.budget_units
            || !self.scope.model_systems.contains(&candidate.model_system)
            || candidate.autonomy_tier != stage.autonomy_tier
            || candidate.effects != stage.effects
            || candidate.depends_on != stage.depends_on
            || context.dependency_action_order != stage.depends_on
            || dependency_artifact_order != stage.depends_on
            || stage.readiness != StageReadiness::Ready
        {
            return Err(failure(
                "selected action does not match the admitted typed stage or its prerequisites",
                false,
            ));
        }
        if context.dependency_artifacts.iter().any(|dependency| {
            let expected_schema = self
                .plan
                .stages
                .iter()
                .find(|upstream| upstream.stage_id == dependency.action_id)
                .map(|upstream| upstream.output_schema.as_str());
            dependency.artifact.validate().is_err()
                || expected_schema != Some(dependency.artifact.content_type.as_str())
        }) {
            return Err(failure(
                "a direct prerequisite artifact is invalid, non-local, or has the wrong stage schema",
                false,
            ));
        }
        let input = GliomaStageInput {
            research_id: self.plan.research_id.clone(),
            study_id: self.plan.study_id.clone(),
            stage_id: stage.stage_id.clone(),
            kind: stage.kind,
            upstream_artifacts: context
                .dependency_artifacts
                .iter()
                .map(|dependency| dependency.artifact.content_hash.clone())
                .collect(),
            source_artifacts: context.source_artifacts.clone(),
            replay_identity: self.plan.replay_identity.clone(),
            attempt,
        };
        match self.stage_executor.execute(stage, &input) {
            Ok(output) => bridge_result(candidate, stage, output, attempt),
            Err(GliomaStageFailure { reason, retryable }) => Err(failure(reason, retryable)),
        }
    }
}

impl<E: GliomaStageExecutor> GliomaActionExecutor for GliomaStageActionExecutor<'_, E> {
    /// Refuse context-free dispatch: a stage action must carry its typed local inputs.
    fn execute_action(
        &mut self,
        _candidate: &GliomaActionCandidate,
        _attempt: u8,
    ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
        Err(failure(
            "typed local artifact context is required for glioma stage execution",
            false,
        ))
    }

    fn execute_action_with_context(
        &mut self,
        candidate: &GliomaActionCandidate,
        context: &GliomaActionExecutionContext,
        attempt: u8,
    ) -> Result<ActionExecutionResult, ActionExecutionFailure> {
        self.execute_with_context(candidate, context, attempt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
    use bioprism_foundation::{AutonomyTier, TypedResearchArtifact, PRECLINICAL_BOUNDARY};
    use bioprism_ids::ContentHash;
    use bioprism_onco::OutputUse;
    use serde_json::json;
    use std::collections::BTreeSet;

    fn intent() -> GliomaResearchIntent {
        let hash = ContentHash::of_bytes(b"glioma-stage-adapter-input");
        GliomaResearchIntent {
            research_id: "adapter-research".into(),
            study_id: "adapter-study".into(),
            objective: "map molecular mechanisms in a preclinical glioma organoid model".into(),
            output_uses: BTreeSet::from([OutputUse::CohortAnalysis, OutputUse::MethodDevelopment]),
            model_systems: BTreeSet::from([
                GliomaModelSystem::Organoid,
                GliomaModelSystem::InSilico,
            ]),
            modalities: BTreeSet::from([
                GliomaModality::Literature,
                GliomaModality::Genomics,
                GliomaModality::Imaging,
                GliomaModality::Transcriptomics,
                GliomaModality::Computational,
                GliomaModality::Replication,
            ]),
            input_artifacts: vec![LocalArtifactRef {
                artifact_id: "adapter-input".into(),
                content_hash: hash.clone(),
                content_type: "application/vnd.aurora.local-study+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            requested_autonomy: AutonomyTier::A1,
            approval_reference: None,
            budget_units: 300,
            max_retries: 1,
            allow_instrument_execution: false,
            allow_federation: false,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: hash,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[derive(Default)]
    struct RecordingStageExecutor {
        inputs: Vec<GliomaStageInput>,
    }

    impl GliomaStageExecutor for RecordingStageExecutor {
        fn execute(
            &mut self,
            stage: &GliomaStage,
            input: &GliomaStageInput,
        ) -> Result<GliomaStageOutput, GliomaStageFailure> {
            self.inputs.push(input.clone());
            let artifact = TypedResearchArtifact::from_payload(
                format!("stage-output:{}", stage.stage_id),
                stage.output_schema.clone(),
                &json!({ "stage": stage.stage_id, "result": "negative" }),
                Vec::new(),
                Vec::new(),
            )
            .expect("valid test artifact");
            Ok(GliomaStageOutput {
                artifact,
                disposition: GliomaStageDisposition::Negative,
                uncertainty: vec!["synthetic-adapter-fixture".into()],
                negative_evidence: vec!["synthetic-null-result".into()],
            })
        }
    }

    fn dependency_artifact(stage: &GliomaStage) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("checkpoint:{}", stage.stage_id),
            content_hash: ContentHash::of_bytes(stage.stage_id.as_bytes()),
            content_type: stage.output_schema.clone(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn candidate(stage: &GliomaStage) -> GliomaActionCandidate {
        GliomaActionCandidate {
            action_id: stage.stage_id.clone(),
            stage_kind: stage.kind,
            modality: stage_modality(stage.kind),
            model_system: GliomaModelSystem::InSilico,
            depends_on: stage.depends_on.clone(),
            cost_units: stage.budget_units,
            information_gain_milli: 800,
            frontier_novelty_milli: 700,
            workflow_leverage_milli: 900,
            cross_stage_unlock_milli: 900,
            reproducibility_safety_milli: 900,
            federation_value_milli: 100,
            feasibility_milli: 800,
            autonomy_tier: stage.autonomy_tier,
            effects: stage.effects.clone(),
        }
    }

    #[test]
    fn adapter_dispatches_typed_stage_and_preserves_negative_result_and_input_hashes() {
        let intent = intent();
        let plan = compile_glioma_research(&intent).unwrap();
        let stage = plan
            .stages
            .iter()
            .find(|stage| stage.readiness == StageReadiness::Ready && !stage.depends_on.is_empty())
            .unwrap()
            .clone();
        let dependencies = stage
            .depends_on
            .iter()
            .map(|action_id| {
                let upstream = plan
                    .stages
                    .iter()
                    .find(|stage| stage.stage_id == *action_id)
                    .unwrap();
                super::super::action_execution::GliomaActionArtifactInput {
                    action_id: action_id.clone(),
                    artifact: dependency_artifact(upstream),
                }
            })
            .collect::<Vec<_>>();
        let dependency_hashes = dependencies
            .iter()
            .map(|dependency| dependency.artifact.content_hash.clone())
            .collect::<Vec<_>>();
        let context = GliomaActionExecutionContext {
            scope: Some(scope_from_intent(&intent)),
            source_artifacts: intent.input_artifacts.clone(),
            dependency_action_order: stage.depends_on.clone(),
            dependency_artifacts: dependencies,
        };
        let mut stage_executor = RecordingStageExecutor::default();
        let mut adapter = GliomaStageActionExecutor::new(&intent, &mut stage_executor).unwrap();

        let output = adapter
            .execute_action_with_context(&candidate(&stage), &context, 1)
            .unwrap();

        assert_eq!(output.disposition, ActionExecutionDisposition::Negative);
        assert_eq!(output.artifact.unwrap().content_type, stage.output_schema);
        assert_eq!(stage_executor.inputs.len(), 1);
        assert_eq!(stage_executor.inputs[0].research_id, intent.research_id);
        assert_eq!(stage_executor.inputs[0].study_id, intent.study_id);
        assert_eq!(stage_executor.inputs[0].kind, stage.kind);
        assert_eq!(
            stage_executor.inputs[0].upstream_artifacts,
            dependency_hashes
        );
        assert_eq!(
            stage_executor.inputs[0].source_artifacts,
            intent.input_artifacts
        );
        assert_eq!(stage_executor.inputs[0].attempt, 1);
    }

    #[test]
    fn adapter_refuses_scope_mismatch_before_stage_dispatch() {
        let intent = intent();
        let plan = compile_glioma_research(&intent).unwrap();
        let stage = plan
            .stages
            .iter()
            .find(|stage| stage.readiness == StageReadiness::Ready)
            .unwrap()
            .clone();
        let mut scope = scope_from_intent(&intent);
        scope.study_id = "wrong-study".into();
        let context = GliomaActionExecutionContext {
            scope: Some(scope),
            source_artifacts: intent.input_artifacts.clone(),
            dependency_action_order: stage.depends_on.clone(),
            dependency_artifacts: Vec::new(),
        };
        let mut stage_executor = RecordingStageExecutor::default();
        let mut adapter = GliomaStageActionExecutor::new(&intent, &mut stage_executor).unwrap();

        assert!(adapter
            .execute_action_with_context(&candidate(&stage), &context, 1)
            .is_err());
        assert!(stage_executor.inputs.is_empty());
    }

    #[test]
    fn adapter_rejects_wrong_schema_for_a_direct_prerequisite() {
        let intent = intent();
        let plan = compile_glioma_research(&intent).unwrap();
        let stage = plan
            .stages
            .iter()
            .find(|stage| stage.readiness == StageReadiness::Ready && !stage.depends_on.is_empty())
            .unwrap()
            .clone();
        let mut dependencies = stage
            .depends_on
            .iter()
            .map(|action_id| {
                let upstream = plan
                    .stages
                    .iter()
                    .find(|stage| stage.stage_id == *action_id)
                    .unwrap();
                super::super::action_execution::GliomaActionArtifactInput {
                    action_id: action_id.clone(),
                    artifact: dependency_artifact(upstream),
                }
            })
            .collect::<Vec<_>>();
        dependencies[0].artifact.content_type = "application/json".into();
        let context = GliomaActionExecutionContext {
            scope: Some(scope_from_intent(&intent)),
            source_artifacts: intent.input_artifacts.clone(),
            dependency_action_order: stage.depends_on.clone(),
            dependency_artifacts: dependencies,
        };
        let mut stage_executor = RecordingStageExecutor::default();
        let mut adapter = GliomaStageActionExecutor::new(&intent, &mut stage_executor).unwrap();

        assert!(adapter
            .execute_action_with_context(&candidate(&stage), &context, 1)
            .is_err());
        assert!(stage_executor.inputs.is_empty());
    }

    #[test]
    fn research_director_executes_a_dependency_closed_batch_through_typed_stages() {
        let intent = intent();
        let request = crate::glioma::programs::p07_protocol_simulation::GliomaResearchDirectorRequest {
            intent: intent.clone(),
            focus: crate::glioma::programs::p07_protocol_simulation::GliomaDirectorFocus::FullProgram,
            completed_checkpoints: Vec::new(),
            budget_units: 120,
            max_actions: 8,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: crate::glioma_engine::GliomaSelectionWeights::default(),
            max_retries: 1,
            require_artifacts: true,
        };
        let mut stage_executor = RecordingStageExecutor::default();
        let mut adapter = GliomaStageActionExecutor::new(&intent, &mut stage_executor).unwrap();

        let output =
            crate::glioma::programs::p07_protocol_simulation::execute_glioma_research_director(
                &request,
                &mut adapter,
            )
            .unwrap();

        assert!(output.execution.is_some());
        assert!(!stage_executor.inputs.is_empty());
        assert!(stage_executor
            .inputs
            .iter()
            .all(|input| input.source_artifacts == intent.input_artifacts));
        assert!(stage_executor
            .inputs
            .iter()
            .any(|input| !input.upstream_artifacts.is_empty()));
        assert!(!output.execution.as_ref().unwrap().negative_order.is_empty());
        output.validate().unwrap();
    }
}
