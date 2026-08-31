//! Replay observability and semantic divergence auditing.
//!
//! Atlas feature: `AFA-runtime-P23-F01`.
//!
//! A replay bundle can be well-formed yet semantically different from its baseline. This feature
//! compares two already policy-bound, hash-chained research runs, names the first observable
//! divergence, and emits a fail-closed receipt. The auditor never re-executes effects and never
//! exports raw tape or experiment data.

use super::research_run::ResearchReplayBundle;
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    LossSeverity, ProvenanceLink, ResearchContractError, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-runtime-P23-F01";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayAuditRequest {
    pub audit_id: String,
    pub baseline: ResearchReplayBundle,
    pub candidate: ResearchReplayBundle,
}

impl ReplayAuditRequest {
    fn validate(&self) -> Result<(), ReplayAuditError> {
        if self.audit_id.trim().is_empty() {
            return Err(ReplayAuditError::InvalidRequest(
                "audit_id is required".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayAuditStatus {
    Equivalent,
    Diverged,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayAuditReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub audit_id: String,
    pub status: ReplayAuditStatus,
    pub baseline_digest: ContentHash,
    pub candidate_digest: ContentHash,
    pub first_difference: Option<String>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ReplayAuditReceipt {
    pub fn validate(&self) -> Result<(), ReplayAuditError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ReplayAuditError::Contract(
                ResearchContractError::SchemaVersion {
                    expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                    found: self.schema_version.clone(),
                },
            ));
        }
        let difference_state_is_consistent = match self.status {
            ReplayAuditStatus::Diverged => self.first_difference.is_some(),
            ReplayAuditStatus::Equivalent | ReplayAuditStatus::Invalid => {
                self.first_difference.is_none()
            }
        };
        if self.feature_id != FEATURE_ID
            || self.audit_id.trim().is_empty()
            || self.baseline_digest == ContentHash::of_bytes(b"")
            || self.candidate_digest == ContentHash::of_bytes(b"")
            || !difference_state_is_consistent
        {
            return Err(ReplayAuditError::InvalidRequest(
                "replay-audit identity, source digests, or divergence state is incomplete".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ReplayAuditError::InvalidRequest(
                "replay-audit artifact is not bound to its source bundle digests".into(),
            ));
        }
        if self.reasons.is_empty() {
            return Err(ReplayAuditError::InvalidRequest(
                "replay-audit reasons are required".into(),
            ));
        }
        if self.artifact.artifact_id != format!("replay-audit:{}", self.audit_id)
            || self.artifact.content_type != "application/vnd.aurora.replay-audit+json"
            || self.artifact.semantic_loss
                != vec![SemanticLoss {
                    field: "replay_tape_and_raw_effects".into(),
                    reason: "raw tape and experiment data remain local; audit exports content identities and divergence reasons".into(),
                    severity: LossSeverity::Bounded,
                }]
            || self.artifact.provenance
                != vec![
                    ProvenanceLink {
                        source_id: "baseline-replay-bundle".into(),
                        relation: "audit-input".into(),
                        digest: self.baseline_digest.clone(),
                    },
                    ProvenanceLink {
                        source_id: "candidate-replay-bundle".into(),
                        relation: "audit-input".into(),
                        digest: self.candidate_digest.clone(),
                    },
                ]
        {
            return Err(ReplayAuditError::Contract(
                ResearchContractError::BoundaryMismatch {
                    capability: self.audit_id.clone(),
                },
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(ReplayAuditError::Contract)
    }

    pub fn verify_payload(&self, payload: &Value) -> Result<(), ReplayAuditError> {
        self.validate()?;
        self.artifact
            .verify_payload(payload)
            .map_err(ReplayAuditError::Contract)
    }

    pub fn digest(&self) -> Result<ContentHash, ReplayAuditError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReplayAuditError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReplayAuditError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ReplayAuditError {
    #[error("invalid replay-audit request: {0}")]
    InvalidRequest(String),
    #[error("research runtime bundle error: {0}")]
    Runtime(String),
    #[error("research contract error: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn replay_audit_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "runtime".into(),
        consumers: ["bioinformatician".into(), "platform reliability engineer".into()].into(),
        behavior: "compares policy-bound replay bundles and identifies the first semantic divergence without re-executing effects".into(),
        value: "turns silent replay drift into a signed, machine-readable release and investigation signal".into(),
        inputs: vec![TypedPort { name: "replay_audit_request".into(), schema: "ReplayAuditRequest@1".into(), required: true }],
        outputs: vec![TypedPort { name: "replay_audit_receipt".into(), schema: "ReplayAuditReceipt@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["read:local-replay-bundles".into(), "write:local-audit-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "fixture:runtime-replay-audit".into(), state: EvidenceState::Supported, locator: Some("fixtures/runtime".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn bundle_digest(bundle: &ResearchReplayBundle) -> Result<ContentHash, ReplayAuditError> {
    let value = serde_json::to_value(bundle)
        .map_err(|error| ReplayAuditError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ReplayAuditError::Serialization(error.to_string()))
}

fn first_difference(
    baseline: &ResearchReplayBundle,
    candidate: &ResearchReplayBundle,
) -> Option<String> {
    let checks = [
        ("feature_id", baseline.feature_id != candidate.feature_id),
        (
            "manifest.capability_id",
            baseline.manifest.capability_id != candidate.manifest.capability_id,
        ),
        ("manifest", baseline.manifest != candidate.manifest),
        (
            "workflow.workflow_id",
            baseline.workflow.workflow_id != candidate.workflow.workflow_id,
        ),
        ("workflow", baseline.workflow != candidate.workflow),
        (
            "workflow.plan_hash",
            baseline.run.plan_hash != candidate.run.plan_hash,
        ),
        ("grant.actor", baseline.grant.actor != candidate.grant.actor),
        ("grant", baseline.grant != candidate.grant),
        (
            "policy.decision",
            baseline.policy.decision != candidate.policy.decision,
        ),
        ("policy", baseline.policy != candidate.policy),
        (
            "run.replay_identity",
            baseline.run.replay_identity != candidate.run.replay_identity,
        ),
        ("run.run_id", baseline.run.run_id != candidate.run.run_id),
        ("run.status", baseline.run.status != candidate.run.status),
        (
            "run.retry_count",
            baseline.run.retry_count != candidate.run.retry_count,
        ),
        ("run.events", baseline.run.events != candidate.run.events),
        (
            "run.checkpoints",
            baseline.run.checkpoints != candidate.run.checkpoints,
        ),
        ("tape_json", baseline.tape_json != candidate.tape_json),
        (
            "result_artifact",
            baseline.result_artifact != candidate.result_artifact,
        ),
        ("boundary", baseline.boundary != candidate.boundary),
    ];
    checks
        .into_iter()
        .find_map(|(field, differs)| differs.then_some(field.into()))
}

pub fn audit_replay(request: &ReplayAuditRequest) -> Result<ReplayAuditReceipt, ReplayAuditError> {
    request.validate()?;
    let baseline_digest = bundle_digest(&request.baseline)?;
    let candidate_digest = bundle_digest(&request.candidate)?;
    let mut reasons = Vec::new();
    let mut status = ReplayAuditStatus::Equivalent;
    let mut difference = None;
    if let Err(error) = request.baseline.verify() {
        status = ReplayAuditStatus::Invalid;
        reasons.push(format!("baseline bundle failed verification: {error}"));
    }
    if let Err(error) = request.candidate.verify() {
        status = ReplayAuditStatus::Invalid;
        reasons.push(format!("candidate bundle failed verification: {error}"));
    }
    if status != ReplayAuditStatus::Invalid {
        difference = first_difference(&request.baseline, &request.candidate);
        if let Some(field) = &difference {
            status = ReplayAuditStatus::Diverged;
            reasons.push(format!("first observable replay divergence: {field}"));
        } else {
            reasons
                .push("baseline and candidate replay bundles are semantically equivalent".into());
        }
    }
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "audit_id": request.audit_id,
        "status": status,
        "baseline_digest": baseline_digest,
        "candidate_digest": candidate_digest,
        "first_difference": difference,
        "reasons": reasons,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("replay-audit:{}", request.audit_id),
        "application/vnd.aurora.replay-audit+json",
        &payload,
        vec![SemanticLoss {
            field: "replay_tape_and_raw_effects".into(),
            reason: "raw tape and experiment data remain local; audit exports content identities and divergence reasons".into(),
            severity: LossSeverity::Bounded,
        }],
        vec![
            ProvenanceLink { source_id: "baseline-replay-bundle".into(), relation: "audit-input".into(), digest: baseline_digest.clone() },
            ProvenanceLink { source_id: "candidate-replay-bundle".into(), relation: "audit-input".into(), digest: candidate_digest.clone() },
        ],
    )?;
    let receipt = ReplayAuditReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        audit_id: request.audit_id.clone(),
        status,
        baseline_digest,
        candidate_digest,
        first_difference: difference,
        reasons,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    receipt.verify_payload(&payload)?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_run::ResearchExecutionSession;
    use crate::{Effect as RuntimeEffect, EffectOutcome, EffectRequest};
    use bioprism_foundation::{
        AutonomyGrant, AutonomyTier, PolicyDecision, PolicyReceipt, ResearchWorkflowSpec,
        ResourceBudget, WorkflowNode, RESEARCH_CONTRACT_SCHEMA_VERSION,
    };
    use bioprism_ids::RunId;
    use serde_json::json;
    use std::collections::BTreeMap;

    fn bundle(run_id: &str, prompt: &str) -> ResearchReplayBundle {
        let manifest = replay_audit_manifest();
        let workflow = ResearchWorkflowSpec {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "replay-audit-workflow".into(),
            intent: "audit a preclinical computation replay".into(),
            nodes: vec![WorkflowNode {
                node_id: "compute".into(),
                capability_id: FEATURE_ID.into(),
                actor: "operator".into(),
                requires_approval: false,
            }],
            edges: Vec::new(),
            checkpoints: Vec::new(),
            budgets: vec![ResourceBudget {
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
            scope: "study:replay-audit".into(),
            expires_at: "2099-01-01T00:00:00Z".into(),
            revoked: false,
            autonomy_tier: AutonomyTier::A0,
            approval_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let policy = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: format!("policy:{run_id}"),
            decision: PolicyDecision::Allow,
            reasons: vec!["fixture approved".into()],
            evaluated_artifacts: Vec::new(),
            authority_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        let mut session = ResearchExecutionSession::new(
            manifest,
            workflow,
            grant,
            policy,
            RunId::parse(run_id).unwrap(),
        )
        .unwrap();
        session
            .append_effect(
                "compute",
                Effect::ExecuteLocalComputation,
                RuntimeEffect::performed(
                    EffectRequest::ModelCall {
                        model: "fixture".into(),
                        prompt: prompt.into(),
                    },
                    EffectOutcome::new(json!({"result": prompt})),
                ),
                Some(&json!({"result": prompt})),
            )
            .unwrap();
        session
            .finish(bioprism_foundation::ExecutionStatus::Succeeded)
            .unwrap();
        session.bundle().unwrap()
    }

    #[test]
    fn equivalent_bundles_are_admitted() {
        let baseline = bundle("replay-audit-1", "same");
        let receipt = audit_replay(&ReplayAuditRequest {
            audit_id: "audit-equivalent".into(),
            baseline: baseline.clone(),
            candidate: baseline,
        })
        .unwrap();
        assert_eq!(receipt.status, ReplayAuditStatus::Equivalent);
        assert!(receipt.first_difference.is_none());
    }

    #[test]
    fn changed_effect_is_reported_as_divergence() {
        let receipt = audit_replay(&ReplayAuditRequest {
            audit_id: "audit-diverged".into(),
            baseline: bundle("replay-audit-2", "baseline"),
            candidate: bundle("replay-audit-2", "changed"),
        })
        .unwrap();
        assert_eq!(receipt.status, ReplayAuditStatus::Diverged);
        assert!(receipt.first_difference.is_some());
    }

    #[test]
    fn changed_authority_metadata_is_reported_as_divergence() {
        let baseline = bundle("replay-audit-metadata", "same");
        let mut candidate = baseline.clone();
        candidate.grant.scope = "study:other-scope".into();
        candidate.manifest.version = "0.2.0".into();
        let receipt = audit_replay(&ReplayAuditRequest {
            audit_id: "audit-metadata-diverged".into(),
            baseline,
            candidate,
        })
        .unwrap();

        assert_eq!(receipt.status, ReplayAuditStatus::Diverged);
        assert_eq!(receipt.first_difference.as_deref(), Some("manifest"));
    }

    #[test]
    fn malformed_tape_is_invalid_not_equivalent() {
        let mut candidate = bundle("replay-audit-3", "same");
        candidate.tape_json.push('x');
        let receipt = audit_replay(&ReplayAuditRequest {
            audit_id: "audit-invalid".into(),
            baseline: bundle("replay-audit-3", "same"),
            candidate,
        })
        .unwrap();
        assert_eq!(receipt.status, ReplayAuditStatus::Invalid);
    }

    #[test]
    fn identical_audits_have_identical_digests() {
        let baseline = bundle("replay-audit-4", "same");
        let request = ReplayAuditRequest {
            audit_id: "audit-deterministic".into(),
            baseline: baseline.clone(),
            candidate: baseline,
        };
        let left = audit_replay(&request).unwrap();
        let right = audit_replay(&request).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }

    #[test]
    fn receipt_rejects_tampered_source_digest_binding() {
        let baseline = bundle("replay-audit-5", "same");
        let mut receipt = audit_replay(&ReplayAuditRequest {
            audit_id: "audit-tamper-binding".into(),
            baseline: baseline.clone(),
            candidate: baseline,
        })
        .unwrap();
        receipt.baseline_digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }
}
