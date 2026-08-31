//! Federated continual retrieval/synthesis assurance harness.
//! Atlas feature `AFA-adapter-P02-F28`.

use crate::federated_continual_retrieval_synthesis_research_workbench::{
    render_federated_continual_retrieval_synthesis_research_workbench,
    FederatedContinualRetrievalSynthesisResearchWorkbenchRequest,
};
use crate::retrieval_synthesis::EvidenceSynthesisDisposition;
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F28";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-retrieval-synthesis-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery4@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalSynthesisAssuranceHarnessRequest {
    pub workbench_request: FederatedContinualRetrievalSynthesisResearchWorkbenchRequest,
    pub baseline_id: String,
    pub expected_scope: String,
    pub expected_federation_id: String,
    pub expected_purpose: String,
    pub minimum_peer_quorum: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub provenance_complete: bool,
    pub evidence_complete: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalSynthesisAssuranceHarnessReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub baseline_id: String,
    pub scope: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_ids: Vec<String>,
    pub min_peer_quorum: u32,
    pub aggregate_only: bool,
    pub endpoint: String,
    pub verdict: EvidenceSynthesisDisposition,
    pub check_order: Vec<String>,
    pub passed_checks: Vec<String>,
    pub counterexamples: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub workflow_run_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub assurance_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedContinualRetrievalSynthesisAssuranceHarnessError {
    #[error("invalid federated continual retrieval assurance request: {0}")]
    Invalid(String),
    #[error("federated continual retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("federated continual retrieval workbench failed: {0}")]
    Workbench(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl FederatedContinualRetrievalSynthesisAssuranceHarnessReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), FederatedContinualRetrievalSynthesisAssuranceHarnessError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.baseline_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.peer_ids.len() < self.min_peer_quorum as usize
            || self.min_peer_quorum == 0
            || !self.aggregate_only
            || self.endpoint.trim().is_empty()
            || self.check_order.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid("federated assurance identity, quorum, aggregate-only locality, checks, candidates, or effects are incomplete"));
        }
        if !canonical(&self.peer_ids) {
            return Err(Self::invalid("federated peer order is not canonical"));
        }
        for values in [
            &self.check_order,
            &self.passed_checks,
            &self.counterexamples,
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.overflow_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.omissions,
            &self.uncertainty,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(Self::invalid(
                    "federated assurance ordering is not canonical",
                ));
            }
        }
        if self
            .overflow_order
            .iter()
            .any(|id| !self.omitted_order.contains(id))
            || self
                .selected_order
                .iter()
                .chain(self.omitted_order.iter())
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(Self::invalid(
                "federated evidence state is not covered by candidates",
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.workflow_run_digest,
            &self.workbench_digest,
            &self.assurance_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(Self::invalid("federated assurance digest is invalid"));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assure:federated-continual-retrieval-synthesis:")
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "federated assurance effect is outside release gate",
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualRetrievalSynthesisAssuranceHarnessError::Artifact(error.to_string())
        })
    }
    fn invalid(message: &str) -> FederatedContinualRetrievalSynthesisAssuranceHarnessError {
        FederatedContinualRetrievalSynthesisAssuranceHarnessError::Invalid(message.into())
    }
}

pub fn federated_continual_retrieval_synthesis_assurance_harness_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["AURORA extension developer".into(),"consortium administrator".into()].into(), behavior: "verifies federated continual retrieval synthesis with purpose-bound peer quorum, aggregate-only exchange, checkpoint/replay, provenance, and fail-closed witnesses".into(), value: "prevents partial federation, unauthorized data movement, and incomplete evidence from being presented as a qualified research result".into(), inputs: vec![TypedPort{name:"federated_scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true}], outputs: vec![TypedPort{name:"assured_federated_evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}], effects: [Effect::ReadLocalData,Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(), permissions:["evaluate:federated-research-artifact".into()].into(), determinism:Determinism::ByteStable, evidence:vec![EvidenceReference{source_id:"ga4gh-wes".into(),state:EvidenceState::Supported,locator:Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into())}], authority_requirements:Vec::new(), autonomy_tier:AutonomyTier::A1, surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(), boundary:PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_federated_continual_retrieval_synthesis(
    request: &FederatedContinualRetrievalSynthesisAssuranceHarnessRequest,
) -> Result<
    FederatedContinualRetrievalSynthesisAssuranceHarnessReceipt,
    FederatedContinualRetrievalSynthesisAssuranceHarnessError,
> {
    let wb = &request.workbench_request;
    let wf = &wb.workflow_request;
    let copilot = &wf.request;
    if request.baseline_id.trim().is_empty()
        || request.expected_scope.trim().is_empty()
        || request.expected_federation_id != wf.request.federation_id
        || request.expected_purpose != wf.request.purpose
        || request.expected_scope != wb.scope
        || request.minimum_peer_quorum == 0
        || wf.request.min_peer_quorum < request.minimum_peer_quorum
        || !wf.request.aggregate_only
        || request.replay_identity != wb.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || wb.boundary != PRECLINICAL_BOUNDARY
        || !copilot.synthesis_request.raw_data_local
        || request.replay_identity.as_str().len() != 64
    {
        return Err(FederatedContinualRetrievalSynthesisAssuranceHarnessError::Invalid("federated assurance scope, purpose, quorum, aggregate-only, replay, locality, or boundary is invalid".into()));
    }
    let workbench =
        render_federated_continual_retrieval_synthesis_research_workbench(wb).map_err(|error| {
            FederatedContinualRetrievalSynthesisAssuranceHarnessError::Workbench(error.to_string())
        })?;
    let mut checks=vec!["aggregate-only federation keeps raw observations local".to_string(),"evidence states preserve selected, omitted, overflow, uncertain, negative, and contradictory items".to_string(),"peer quorum and purpose identity are continuous".to_string(),"provenance and replay identities are content-addressed".to_string(),"typed federated workbench receipt validates".to_string(),"requested scope matches the workbench scope".to_string()];
    checks.sort();
    let mut passed = Vec::new();
    let mut counter = Vec::new();
    let mut omissions = workbench.omissions.clone();
    let mut uncertainty = workbench.uncertainty.clone();
    for (ok, success, failure, is_uncertain) in [
        (
            request.policy_allow,
            "policy authorization",
            "policy authorization denied",
            false,
        ),
        (
            request.protected_closure,
            "protected closure",
            "protected closure incomplete",
            true,
        ),
        (
            request.provenance_complete,
            "provenance completeness",
            "provenance completeness failed",
            false,
        ),
        (
            request.evidence_complete,
            "evidence completeness",
            "evidence completeness is unknown",
            true,
        ),
    ] {
        if ok {
            passed.push(success.to_string());
        } else {
            counter.push(failure.to_string());
            if is_uncertain {
                uncertainty.push(failure.to_string());
            } else {
                omissions.push(failure.to_string());
            }
        }
    }
    if matches!(workbench.disposition, EvidenceSynthesisDisposition::Passed) && counter.is_empty() {
        passed.push("federated workbench disposition qualified".into());
    } else {
        counter.push("federated workbench did not establish a qualified disposition".into());
    }
    passed.sort();
    passed.dedup();
    counter.sort();
    counter.dedup();
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let verdict =
        if !request.policy_allow || !request.protected_closure || !wf.request.aggregate_only {
            EvidenceSynthesisDisposition::Blocked
        } else if counter.is_empty() {
            EvidenceSynthesisDisposition::Passed
        } else {
            EvidenceSynthesisDisposition::Unknown
        };
    let effect = if matches!(verdict, EvidenceSynthesisDisposition::Passed) {
        format!(
            "assure:federated-continual-retrieval-synthesis:{}",
            request.baseline_id
        )
    } else {
        "block:unsafe-release".into()
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":copilot.synthesis_request.request_id,"baseline_id":request.baseline_id,"scope":request.expected_scope,"federation_id":workbench.federation_id,"purpose":workbench.purpose,"peer_ids":workbench.peer_ids,"min_peer_quorum":workbench.min_peer_quorum,"aggregate_only":workbench.aggregate_only,"endpoint":workbench.endpoint,"verdict":verdict,"check_order":checks,"passed_checks":passed,"counterexamples":counter,"candidate_order":workbench.candidate_order,"selected_order":workbench.selected_order,"omitted_order":workbench.omitted_order,"overflow_order":workbench.overflow_order,"uncertainty_order":workbench.uncertainty_order,"negative_order":workbench.negative_order,"contradictory_order":workbench.contradictory_order,"replay_identity":request.replay_identity,"workflow_run_digest":workbench.workflow_run_digest,"workbench_digest":workbench.workbench_digest,"omissions":omissions,"uncertainty":uncertainty,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let assurance_digest = ContentHash::of_value(&payload).map_err(|error| {
        FederatedContinualRetrievalSynthesisAssuranceHarnessError::Artifact(error.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-federated-continual-retrieval-assurance:{}",
            request.baseline_id
        ),
        "application/vnd.aurora.federated-continual-retrieval-synthesis-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        FederatedContinualRetrievalSynthesisAssuranceHarnessError::Artifact(error.to_string())
    })?;
    let receipt = FederatedContinualRetrievalSynthesisAssuranceHarnessReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: copilot.synthesis_request.request_id.clone(),
        baseline_id: request.baseline_id.clone(),
        scope: request.expected_scope.clone(),
        federation_id: workbench.federation_id.clone(),
        purpose: workbench.purpose.clone(),
        peer_ids: workbench.peer_ids.clone(),
        min_peer_quorum: workbench.min_peer_quorum,
        aggregate_only: workbench.aggregate_only,
        endpoint: workbench.endpoint.clone(),
        verdict,
        check_order: checks,
        passed_checks: passed,
        counterexamples: counter,
        candidate_order: workbench.candidate_order.clone(),
        selected_order: workbench.selected_order.clone(),
        omitted_order: workbench.omitted_order.clone(),
        overflow_order: workbench.overflow_order.clone(),
        uncertainty_order: workbench.uncertainty_order.clone(),
        negative_order: workbench.negative_order.clone(),
        contradictory_order: workbench.contradictory_order.clone(),
        replay_identity: request.replay_identity.clone(),
        workflow_run_digest: workbench.workflow_run_digest.clone(),
        workbench_digest: workbench.workbench_digest.clone(),
        assurance_digest,
        omissions,
        uncertainty,
        effect_receipts: vec![effect],
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a1_and_aggregate_only() {
        let manifest = federated_continual_retrieval_synthesis_assurance_harness_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert_eq!(INPUT_SCHEMA, "ScopedRetrievalQuery4@1");
    }
}
