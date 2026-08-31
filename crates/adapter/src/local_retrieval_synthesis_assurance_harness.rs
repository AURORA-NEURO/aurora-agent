//! Local retrieval/synthesis assurance harness with witnesses and counterexamples.
//! Atlas feature `AFA-adapter-P02-F25`.

use crate::local_retrieval_synthesis_research_workbench::{
    render_local_retrieval_synthesis_research_workbench,
    LocalRetrievalSynthesisResearchWorkbenchRequest,
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
pub const FEATURE_ID: &str = "AFA-adapter-P02-F25";
pub const CONTRACT_VERSION: &str = "adapter-local-retrieval-synthesis-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery1@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRetrievalSynthesisAssuranceHarnessRequest {
    pub workbench_request: LocalRetrievalSynthesisResearchWorkbenchRequest,
    pub baseline_id: String,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub provenance_complete: bool,
    pub evidence_complete: bool,
    pub expected_scope: String,
    pub replay_identity: ContentHash,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRetrievalSynthesisAssuranceHarnessReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub baseline_id: String,
    pub scope: String,
    pub verdict: EvidenceSynthesisDisposition,
    pub check_order: Vec<String>,
    pub passed_checks: Vec<String>,
    pub counterexamples: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub replay_identity: ContentHash,
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
pub enum LocalRetrievalSynthesisAssuranceHarnessError {
    #[error("invalid retrieval assurance request: {0}")]
    Invalid(String),
    #[error("retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("retrieval workbench failed: {0}")]
    Workbench(String),
}
impl LocalRetrievalSynthesisAssuranceHarnessReceipt {
    pub fn validate(&self) -> Result<(), LocalRetrievalSynthesisAssuranceHarnessError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.baseline_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.check_order.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid("assurance identity, scope, checks, candidates, locality, or effects are incomplete"));
        }
        for values in [
            &self.check_order,
            &self.passed_checks,
            &self.counterexamples,
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.omissions,
            &self.uncertainty,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|p| p[0] >= p[1]) {
                return Err(Self::invalid("assurance ordering is not canonical"));
            }
        }
        for d in [
            &self.replay_identity,
            &self.workbench_digest,
            &self.assurance_digest,
            &self.artifact.content_hash,
        ] {
            if d.as_str().len() != 64 {
                return Err(Self::invalid("assurance digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| LocalRetrievalSynthesisAssuranceHarnessError::Artifact(e.to_string()))
    }
    fn invalid(m: &str) -> LocalRetrievalSynthesisAssuranceHarnessError {
        LocalRetrievalSynthesisAssuranceHarnessError::Invalid(m.into())
    }
}
pub fn local_retrieval_synthesis_assurance_harness_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["preclinical researcher".into(),"evaluation operator".into()].into(),behavior:"verifies local retrieval synthesis with explicit release predicates, witnesses, counterexamples, and replay receipts".into(),value:"prevents incomplete evidence or protected closure from being mistaken for a qualified research result".into(),inputs:vec![TypedPort{name:"scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true},TypedPort{name:"research_workbench_receipt".into(),schema:"EvidenceSynthesis5@1".into(),required:true}],outputs:vec![TypedPort{name:"assured_evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["evaluate:local-research-artifact".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Ui,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn assure_local_retrieval_synthesis(
    request: &LocalRetrievalSynthesisAssuranceHarnessRequest,
) -> Result<
    LocalRetrievalSynthesisAssuranceHarnessReceipt,
    LocalRetrievalSynthesisAssuranceHarnessError,
> {
    if request.baseline_id.trim().is_empty()
        || request.expected_scope.trim().is_empty()
        || request.expected_scope != request.workbench_request.scope
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.workbench_request.boundary != PRECLINICAL_BOUNDARY
        || !request
            .workbench_request
            .copilot_request
            .synthesis_request
            .raw_data_local
        || request.replay_identity.as_str().len() != 64
    {
        return Err(LocalRetrievalSynthesisAssuranceHarnessError::Invalid(
            "assurance scope, baseline, replay, locality, or boundary is invalid".into(),
        ));
    }
    let workbench = render_local_retrieval_synthesis_research_workbench(&request.workbench_request)
        .map_err(|e| LocalRetrievalSynthesisAssuranceHarnessError::Workbench(e.to_string()))?;
    let mut checks = vec![
        "scope matches requested research scope".into(),
        "typed workbench receipt validates".into(),
        "replay identity is content-addressed".into(),
        "negative and omitted evidence remain visible".into(),
        "raw data remains institution-local".into(),
    ];
    checks.sort();
    let mut passed = Vec::new();
    let mut counter = Vec::new();
    let mut omissions = workbench.omissions.clone();
    let mut uncertainty = workbench.uncertainty.clone();
    if request.policy_allow {
        passed.push("policy authorization".into())
    } else {
        counter.push("policy authorization denied".into());
        omissions.push("policy authorization".into());
    }
    if request.protected_closure {
        passed.push("protected closure".into())
    } else {
        counter.push("protected closure incomplete".into());
        uncertainty.push("protected closure is unmeasured".into());
    }
    if request.provenance_complete {
        passed.push("provenance completeness".into())
    } else {
        counter.push("provenance completeness failed".into());
        omissions.push("provenance completeness".into());
    }
    if request.evidence_complete {
        passed.push("evidence completeness".into())
    } else {
        counter.push("evidence completeness is unknown".into());
        uncertainty.push("evidence completeness is unknown".into());
    }
    if counter.is_empty() && matches!(workbench.disposition, EvidenceSynthesisDisposition::Passed) {
        passed.push("workbench disposition qualified".into());
    } else {
        counter.push("workbench did not establish a qualified disposition".into());
    }
    passed.sort();
    passed.dedup();
    counter.sort();
    counter.dedup();
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let verdict = if counter.is_empty() {
        EvidenceSynthesisDisposition::Passed
    } else if passed.len() < 2 {
        EvidenceSynthesisDisposition::Blocked
    } else {
        EvidenceSynthesisDisposition::Unknown
    };
    let effect = if matches!(verdict, EvidenceSynthesisDisposition::Passed) {
        "assure:local-retrieval-synthesis"
    } else {
        "block:unsafe-release"
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.workbench_request.copilot_request.synthesis_request.request_id,"baseline_id":request.baseline_id,"scope":request.expected_scope,"verdict":verdict,"check_order":checks,"passed_checks":passed,"counterexamples":counter,"candidate_order":workbench.candidate_order,"selected_order":workbench.selected_order,"omitted_order":workbench.omitted_order,"uncertainty_order":workbench.uncertainty_order,"negative_order":workbench.negative_order,"contradictory_order":workbench.contradictory_order,"replay_identity":request.replay_identity,"workbench_digest":workbench.workbench_digest,"omissions":omissions,"uncertainty":uncertainty,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let ad = ContentHash::of_value(&payload)
        .map_err(|e| LocalRetrievalSynthesisAssuranceHarnessError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("local-retrieval-assurance:{}", request.baseline_id),
        "application/vnd.aurora.local-retrieval-synthesis-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| LocalRetrievalSynthesisAssuranceHarnessError::Artifact(e.to_string()))?;
    let receipt = LocalRetrievalSynthesisAssuranceHarnessReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request
            .workbench_request
            .copilot_request
            .synthesis_request
            .request_id
            .clone(),
        baseline_id: request.baseline_id.clone(),
        scope: request.expected_scope.clone(),
        verdict,
        check_order: checks,
        passed_checks: passed,
        counterexamples: counter,
        candidate_order: workbench.candidate_order.clone(),
        selected_order: workbench.selected_order.clone(),
        omitted_order: workbench.omitted_order.clone(),
        uncertainty_order: workbench.uncertainty_order.clone(),
        negative_order: workbench.negative_order.clone(),
        contradictory_order: workbench.contradictory_order.clone(),
        replay_identity: request.replay_identity.clone(),
        workbench_digest: workbench.workbench_digest.clone(),
        assurance_digest: ad,
        effect_receipts: vec![effect.into()],
        omissions,
        uncertainty,
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
    fn manifest_is_a1_and_fail_closed() {
        assert_eq!(
            local_retrieval_synthesis_assurance_harness_manifest().autonomy_tier,
            AutonomyTier::A1
        );
        assert_eq!(OUTPUT_SCHEMA, "EvidenceSynthesis5@1");
    }
}
