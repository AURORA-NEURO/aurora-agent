//! End-to-end evidence-to-replay service composition.
//!
//! This module is deliberately thin: scientific selection stays in `bioprism-bioir`, and effect
//! authorization/replay stays in `bioprism-runtime`. The service owns the boundary between them,
//! so an API, CLI, MCP tool and institution-local operator all receive the same typed result and
//! the same refusal behavior.

use bioprism_bioir::{
    EvidenceLedger, EvidenceSynthesis, KnowledgeCompiler, KnowledgeError, ScopedRetrievalQuery,
};
use bioprism_foundation::{
    AutonomyGrant, AutonomyTier, Effect as ResearchEffect, ExecutionStatus, ResearchContractError,
    ResearchWorkflowSpec, ResourceBudget, WorkflowNode,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::RunId;
use bioprism_runtime::{
    Effect, EffectOutcome, EffectRequest, ResearchExecutionSession, ResearchReplayBundle,
    ResearchRuntimeError,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use thiserror::Error;

/// Atlas feature implemented by this service composition.
pub const FEATURE_ID: &str = "AFA-services-P12-F01";

#[derive(Debug, Error)]
pub enum ResearchServiceError {
    #[error("evidence compilation failed: {0}")]
    Knowledge(#[from] KnowledgeError),
    #[error("execution failed: {0}")]
    Runtime(#[from] ResearchRuntimeError),
    #[error("contract rejected service composition: {0}")]
    Contract(#[from] ResearchContractError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceWorkflowResult {
    pub feature_id: String,
    pub synthesis: EvidenceSynthesis,
    pub replay: ResearchReplayBundle,
}

/// Composes local evidence compilation and replayable execution for one preclinical study intent.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResearchWorkflowService;

impl ResearchWorkflowService {
    pub fn compile_and_execute(
        &self,
        ledger: &EvidenceLedger,
        query: &ScopedRetrievalQuery,
        run_id: RunId,
    ) -> Result<EvidenceWorkflowResult, ResearchServiceError> {
        let compiler = KnowledgeCompiler::default();
        let synthesis = compiler.compile(ledger, query)?;
        let manifest = KnowledgeCompiler::manifest();
        let workflow = workflow_spec(query);
        let grant = autonomy_grant(query);
        let mut session = ResearchExecutionSession::new(
            manifest,
            workflow,
            grant,
            synthesis.policy.clone(),
            run_id,
        )?;
        let evidence = json!({
            "synthesis_hash": synthesis.artifact.content_hash,
            "receipt_id": synthesis.receipt.receipt_id,
            "policy": synthesis.policy.decision,
        });
        let effect = Effect::performed(
            EffectRequest::ModelCall {
                model: "local-evidence-compiler".into(),
                prompt: query.intent.clone(),
            },
            EffectOutcome::new(evidence.clone()),
        );
        session.append_effect(
            "compile_evidence",
            ResearchEffect::ExecuteLocalComputation,
            effect,
            Some(&evidence),
        )?;
        session.attach_result(synthesis.artifact.clone())?;
        session.checkpoint("evidence-compiled")?;
        session.finish(ExecutionStatus::Succeeded)?;
        let replay = session.bundle()?;
        replay.verify()?;
        Ok(EvidenceWorkflowResult {
            feature_id: FEATURE_ID.into(),
            synthesis,
            replay,
        })
    }
}

fn workflow_spec(query: &ScopedRetrievalQuery) -> ResearchWorkflowSpec {
    ResearchWorkflowSpec {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        workflow_id: format!("evidence-workflow:{}", query.query_id),
        intent: query.intent.clone(),
        nodes: vec![WorkflowNode {
            node_id: "compile-evidence".into(),
            capability_id: bioprism_bioir::FEATURE_ID.into(),
            actor: "research-workflow-service".into(),
            requires_approval: false,
        }],
        edges: Vec::new(),
        checkpoints: vec![bioprism_foundation::WorkflowCheckpoint {
            checkpoint_id: "evidence-compiled".into(),
            after_nodes: ["compile-evidence".into()].into(),
        }],
        budgets: vec![ResourceBudget {
            resource: "local_cpu_ms".into(),
            amount: 60_000.0,
        }],
        compensations: Vec::new(),
        approvals: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn autonomy_grant(query: &ScopedRetrievalQuery) -> AutonomyGrant {
    AutonomyGrant {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        actor: "research-workflow-service".into(),
        permitted_actions: ["execute_local_computation".into()].into(),
        resource_budget: BTreeMap::from([(String::from("local_cpu_ms"), 60_000.0)]),
        scope: format!("query:{}", query.query_id),
        expires_at: "2099-01-01T00:00:00Z".into(),
        revoked: false,
        autonomy_tier: AutonomyTier::A0,
        approval_reference: None,
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_bioir::{
        AccessPolicy, EvidenceId, EvidenceObject, Locator, MeasurementContext, Modality,
        Provenance, QualityAssertion,
    };
    use bioprism_ids::ContentHash;
    use bioprism_scope::{Interval, Timestamp};
    use bioprism_foundation::PolicyDecision;
    use std::collections::BTreeSet;

    fn fixture_evidence() -> EvidenceObject {
        EvidenceObject {
            id: EvidenceId::parse("services-paper-1").unwrap(),
            artifact_hash: ContentHash::of_bytes(b"services-paper-1"),
            locator: Locator::DocumentSpan {
                document: "paper-1.txt".into(),
                start: 0,
                end: 16,
            },
            modality: Modality::Text,
            content_type: "text/plain".into(),
            bindings: Default::default(),
            context: MeasurementContext::default(),
            quality: QualityAssertion {
                grade: "screened".into(),
                asserted_by: "fixture".into(),
                caveats: Default::default(),
            },
            provenance: Provenance {
                adapter: "fixture".into(),
                adapter_version: "1".into(),
                parser_version: "1".into(),
                extracted_at: Timestamp::parse("2024-01-01T00:00:00Z").unwrap(),
                source: "fixture".into(),
            },
            validity: Interval::UNBOUNDED,
            access: AccessPolicy {
                labels: BTreeSet::new(),
                embeddable: true,
            },
            derivation: None,
        }
    }

    #[test]
    fn service_returns_synthesis_and_replay_bundle_with_same_artifact() {
        let mut ledger = EvidenceLedger::new();
        ledger.insert(fixture_evidence()).unwrap();
        let query = ScopedRetrievalQuery {
            query_id: "services-q1".into(),
            intent: "compare preclinical mechanism evidence".into(),
            decision_time: Timestamp::parse("2025-01-01T00:00:00Z").unwrap(),
            selected_evidence: BTreeSet::new(),
            permitted_labels: BTreeSet::new(),
            max_sources: 8,
        };
        let result = ResearchWorkflowService::default()
            .compile_and_execute(&ledger, &query, RunId::parse("services-run-1").unwrap())
            .unwrap();
        assert_eq!(result.feature_id, FEATURE_ID);
        assert_eq!(result.replay.run.status, ExecutionStatus::Succeeded);
        assert_eq!(
            result.replay.result_artifact.unwrap().content_hash,
            result.synthesis.artifact.content_hash
        );
    }

    #[test]
    fn unresolved_policy_refuses_execution_instead_of_guessing() {
        let ledger = EvidenceLedger::new();
        let query = ScopedRetrievalQuery {
            query_id: "services-q2".into(),
            intent: "search protected evidence".into(),
            decision_time: Timestamp::parse("2025-01-01T00:00:00Z").unwrap(),
            selected_evidence: BTreeSet::new(),
            permitted_labels: BTreeSet::new(),
            max_sources: 8,
        };
        let error = ResearchWorkflowService::default()
            .compile_and_execute(&ledger, &query, RunId::parse("services-run-2").unwrap())
            .unwrap_err();
        assert!(matches!(
            error,
            ResearchServiceError::Runtime(ResearchRuntimeError::PolicyBlocked(
                PolicyDecision::Unresolved
            ))
        ));
    }
}
