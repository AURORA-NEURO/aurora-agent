//! Multimodal retrieval/synthesis assurance harness with comparability witnesses.
//!
//! Atlas feature `AFA-adapter-P02-F26`: an A1, method-rich verification surface over
//! the multimodal researcher workbench. Every release predicate is represented in a
//! deterministic receipt; missing or contradictory evidence is never upgraded to a pass.

use crate::multimodal_retrieval_synthesis_research_workbench::{
    render_multimodal_retrieval_synthesis_research_workbench,
    MultimodalRetrievalSynthesisResearchWorkbenchRequest,
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

pub const FEATURE_ID: &str = "AFA-adapter-P02-F26";
pub const CONTRACT_VERSION: &str = "adapter-multimodal-retrieval-synthesis-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery2@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisAssuranceHarnessRequest {
    pub workbench_request: MultimodalRetrievalSynthesisResearchWorkbenchRequest,
    pub baseline_id: String,
    pub expected_scope: String,
    pub expected_modalities: Vec<String>,
    pub expected_comparability_digest: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub provenance_complete: bool,
    pub evidence_complete: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisAssuranceHarnessReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub baseline_id: String,
    pub scope: String,
    pub required_modalities: Vec<String>,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalRetrievalSynthesisAssuranceHarnessError {
    #[error("invalid multimodal retrieval assurance request: {0}")]
    Invalid(String),
    #[error("multimodal retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal retrieval workbench failed: {0}")]
    Workbench(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl MultimodalRetrievalSynthesisAssuranceHarnessReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalSynthesisAssuranceHarnessError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.baseline_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.required_modalities.len() < 2
            || !canonical(&self.required_modalities)
            || self.candidate_order.is_empty()
            || self.check_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid(
                "multimodal assurance identity, modality closure, checks, candidates, locality, or effects are incomplete",
            ));
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
            if !canonical(values) {
                return Err(Self::invalid(
                    "multimodal assurance ordering is not canonical",
                ));
            }
        }
        if self
            .selected_order
            .iter()
            .chain(self.omitted_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(Self::invalid(
                "assurance evidence state is not covered by candidates",
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.replay_identity,
            &self.workbench_digest,
            &self.assurance_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(Self::invalid("multimodal assurance digest is invalid"));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assure:multimodal-retrieval-synthesis:")
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "multimodal assurance effect is outside release gate",
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            MultimodalRetrievalSynthesisAssuranceHarnessError::Artifact(error.to_string())
        })
    }

    fn invalid(message: &str) -> MultimodalRetrievalSynthesisAssuranceHarnessError {
        MultimodalRetrievalSynthesisAssuranceHarnessError::Invalid(message.into())
    }
}

pub fn multimodal_retrieval_synthesis_assurance_harness_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["consortium administrator".into(), "independent validator".into()].into(),
        behavior: "verifies multimodal retrieval synthesis with study/modality closure, comparability, provenance, replay, and fail-closed witnesses".into(),
        value: "prevents incomplete or incomparable imaging and omics evidence from being presented as a qualified research result".into(),
        inputs: vec![TypedPort { name: "multimodal_scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "assured_evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:multimodal-research-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) },
            EvidenceReference { source_id: "anndata".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_multimodal_retrieval_synthesis(
    request: &MultimodalRetrievalSynthesisAssuranceHarnessRequest,
) -> Result<
    MultimodalRetrievalSynthesisAssuranceHarnessReceipt,
    MultimodalRetrievalSynthesisAssuranceHarnessError,
> {
    if request.baseline_id.trim().is_empty()
        || request.expected_scope.trim().is_empty()
        || request.expected_scope != request.workbench_request.scope
        || request.expected_modalities.len() < 2
        || !canonical(&request.expected_modalities)
        || request.expected_modalities
            != request
                .workbench_request
                .copilot_request
                .required_modalities
        || request.expected_comparability_digest
            != request
                .workbench_request
                .copilot_request
                .comparability_digest
        || request.replay_identity != request.workbench_request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.workbench_request.boundary != PRECLINICAL_BOUNDARY
        || !request
            .workbench_request
            .copilot_request
            .synthesis_request
            .raw_data_local
        || request.replay_identity.as_str().len() != 64
    {
        return Err(MultimodalRetrievalSynthesisAssuranceHarnessError::Invalid(
            "assurance scope, modality/comparability contract, replay, locality, or boundary is invalid".into(),
        ));
    }
    let workbench =
        render_multimodal_retrieval_synthesis_research_workbench(&request.workbench_request)
            .map_err(|error| {
                MultimodalRetrievalSynthesisAssuranceHarnessError::Workbench(error.to_string())
            })?;
    let mut checks = vec![
        "comparability digest matches the declared modality profile".to_string(),
        "evidence states preserve selected, omitted, uncertain, negative, and contradictory items"
            .to_string(),
        "modality closure covers at least two canonical modalities".to_string(),
        "provenance and replay identities are content-addressed".to_string(),
        "raw multimodal observations remain institution-local".to_string(),
        "typed workbench receipt validates".to_string(),
        "requested scope matches the workbench scope".to_string(),
    ];
    checks.sort();
    let mut passed = Vec::new();
    let mut counterexamples = Vec::new();
    let mut omissions = workbench.omissions.clone();
    let mut uncertainty = workbench.uncertainty.clone();
    for (ok, passed_label, failure, uncertain) in [
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
            passed.push(passed_label.to_string());
        } else {
            counterexamples.push(failure.to_string());
            if uncertain {
                uncertainty.push(failure.to_string());
            } else {
                omissions.push(failure.to_string());
            }
        }
    }
    if matches!(workbench.disposition, EvidenceSynthesisDisposition::Passed)
        && counterexamples.is_empty()
    {
        passed.push("workbench disposition qualified".into());
    } else {
        counterexamples
            .push("workbench did not establish a qualified multimodal disposition".into());
    }
    passed.sort();
    passed.dedup();
    counterexamples.sort();
    counterexamples.dedup();
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request
            .workbench_request
            .copilot_request
            .synthesis_request
            .raw_data_local
    {
        EvidenceSynthesisDisposition::Blocked
    } else if counterexamples.is_empty() {
        EvidenceSynthesisDisposition::Passed
    } else {
        EvidenceSynthesisDisposition::Unknown
    };
    let effect = if matches!(verdict, EvidenceSynthesisDisposition::Passed) {
        format!(
            "assure:multimodal-retrieval-synthesis:{}",
            request.baseline_id
        )
    } else {
        "block:unsafe-release".into()
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.workbench_request.copilot_request.synthesis_request.request_id,"baseline_id":request.baseline_id,"scope":request.expected_scope,"required_modalities":request.expected_modalities,"comparability_digest":request.expected_comparability_digest,"verdict":verdict,"check_order":checks,"passed_checks":passed,"counterexamples":counterexamples,"candidate_order":workbench.candidate_order,"selected_order":workbench.selected_order,"omitted_order":workbench.omitted_order,"uncertainty_order":workbench.uncertainty_order,"negative_order":workbench.negative_order,"contradictory_order":workbench.contradictory_order,"replay_identity":request.replay_identity,"workbench_digest":workbench.workbench_digest,"omissions":omissions,"uncertainty":uncertainty,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let assurance_digest = ContentHash::of_value(&payload).map_err(|error| {
        MultimodalRetrievalSynthesisAssuranceHarnessError::Artifact(error.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-multimodal-retrieval-assurance:{}",
            request.baseline_id
        ),
        "application/vnd.aurora.multimodal-retrieval-synthesis-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        MultimodalRetrievalSynthesisAssuranceHarnessError::Artifact(error.to_string())
    })?;
    let receipt = MultimodalRetrievalSynthesisAssuranceHarnessReceipt {
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
        required_modalities: request.expected_modalities.clone(),
        comparability_digest: request.expected_comparability_digest.clone(),
        verdict,
        check_order: checks,
        passed_checks: passed,
        counterexamples,
        candidate_order: workbench.candidate_order.clone(),
        selected_order: workbench.selected_order.clone(),
        omitted_order: workbench.omitted_order.clone(),
        uncertainty_order: workbench.uncertainty_order.clone(),
        negative_order: workbench.negative_order.clone(),
        contradictory_order: workbench.contradictory_order.clone(),
        replay_identity: request.replay_identity.clone(),
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
    fn manifest_is_a1_and_multimodal() {
        let manifest = multimodal_retrieval_synthesis_assurance_harness_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert_eq!(INPUT_SCHEMA, "ScopedRetrievalQuery2@1");
    }
}
