//! Version-negotiated multimodal retrieval/synthesis interoperability gateway.
//! Atlas feature `AFA-adapter-P02-F22`.

use crate::interoperability_gateway::{
    negotiate_interoperability, InteroperabilityDisposition, InteroperabilityRequest,
};
use crate::multimodal_retrieval_synthesis_research_workbench::{
    render_multimodal_retrieval_synthesis_research_workbench,
    MultimodalRetrievalSynthesisResearchWorkbenchRequest,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
pub const FEATURE_ID: &str = "AFA-adapter-P02-F22";
pub const CONTRACT_VERSION: &str =
    "adapter-multimodal-retrieval-synthesis-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery2@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis6@1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisInteroperabilityGatewayRequest {
    pub interop_request: InteroperabilityRequest,
    pub workbench_request: MultimodalRetrievalSynthesisResearchWorkbenchRequest,
    pub requested_input_schema: String,
    pub requested_output_schema: String,
    pub required_modalities: Vec<String>,
    pub comparability_digest: ContentHash,
    pub migration_policy: String,
    pub semantic_loss_budget: u32,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisInteroperabilityGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub endpoint_id: String,
    pub negotiated_version: String,
    pub disposition: InteroperabilityDisposition,
    pub input_schema: String,
    pub output_schema: String,
    pub required_modalities: Vec<String>,
    pub comparability_digest: ContentHash,
    pub migration_policy: String,
    pub semantic_loss_budget: u32,
    pub capability_order: Vec<String>,
    pub artifact_digest_order: Vec<ContentHash>,
    pub semantic_loss_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub workbench_digest: ContentHash,
    pub protocol_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MultimodalRetrievalSynthesisInteroperabilityGatewayError {
    #[error("invalid multimodal retrieval interoperability request: {0}")]
    Invalid(String),
    #[error("multimodal interoperability artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal workbench failed: {0}")]
    Workbench(String),
}
impl MultimodalRetrievalSynthesisInteroperabilityGatewayReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalSynthesisInteroperabilityGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.endpoint_id.trim().is_empty()
            || self.negotiated_version.trim().is_empty()
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.required_modalities.is_empty()
            || self.migration_policy.trim().is_empty()
            || self.semantic_loss_budget == 0
            || self.capability_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid("multimodal gateway identity, schemas, modalities, locality, budget, or effects are incomplete"));
        }
        for values in [
            &self.required_modalities,
            &self.capability_order,
            &self.semantic_loss_order,
            &self.omissions,
            &self.uncertainty,
        ] {
            if values.windows(2).any(|p| p[0] >= p[1]) {
                return Err(Self::invalid(
                    "multimodal gateway output ordering is not canonical",
                ));
            }
        }
        if self.artifact_digest_order.windows(2).any(|p| p[0] >= p[1]) {
            return Err(Self::invalid(
                "multimodal artifact digest ordering is not canonical",
            ));
        }
        for d in [
            &self.comparability_digest,
            &self.workbench_digest,
            &self.protocol_digest,
            &self.artifact.content_hash,
        ] {
            if d.as_str().len() != 64 {
                return Err(Self::invalid("multimodal gateway digest is invalid"));
            }
        }
        self.artifact.validate_metadata().map_err(|e| {
            MultimodalRetrievalSynthesisInteroperabilityGatewayError::Artifact(e.to_string())
        })
    }
    fn invalid(m: &str) -> MultimodalRetrievalSynthesisInteroperabilityGatewayError {
        MultimodalRetrievalSynthesisInteroperabilityGatewayError::Invalid(m.into())
    }
}
pub fn multimodal_retrieval_synthesis_interoperability_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["preclinical researcher".into(),"AURORA extension developer".into()].into(),behavior:"negotiates multimodal retrieval/synthesis protocol versions with comparability and loss receipts".into(),value:"prevents cross-study modality drift from being hidden behind a compatible API".into(),inputs:vec![TypedPort{name:"scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true},TypedPort{name:"comparability_profile".into(),schema:"ComparabilityDigest@1".into(),required:true}],outputs:vec![TypedPort{name:"evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["connect:approved-endpoints".into(),"view:authorized-research-state".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"ome-ngff-rfc5".into(),state:EvidenceState::Supported,locator:Some("https://ngff.openmicroscopy.org/rfc/5/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Ui].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn render_multimodal_retrieval_synthesis_interoperability_gateway(
    request: &MultimodalRetrievalSynthesisInteroperabilityGatewayRequest,
) -> Result<
    MultimodalRetrievalSynthesisInteroperabilityGatewayReceipt,
    MultimodalRetrievalSynthesisInteroperabilityGatewayError,
> {
    validate_request(request)?;
    let integration = negotiate_interoperability(&request.interop_request).map_err(|e| {
        MultimodalRetrievalSynthesisInteroperabilityGatewayError::Invalid(e.to_string())
    })?;
    let workbench =
        render_multimodal_retrieval_synthesis_research_workbench(&request.workbench_request)
            .map_err(|e| {
                MultimodalRetrievalSynthesisInteroperabilityGatewayError::Workbench(e.to_string())
            })?;
    let mut caps = integration.capability_order.clone();
    caps.sort();
    caps.dedup();
    let mut loss = integration
        .semantic_loss
        .iter()
        .map(|x| x.field.clone())
        .collect::<Vec<_>>();
    loss.sort();
    loss.dedup();
    if loss.len() as u32 > request.semantic_loss_budget {
        return Err(
            MultimodalRetrievalSynthesisInteroperabilityGatewayError::Invalid(
                "semantic-loss budget exceeded".into(),
            ),
        );
    }
    let mut omissions = integration.omissions.clone();
    omissions.extend(workbench.omissions.clone());
    omissions.sort();
    omissions.dedup();
    let mut uncertainty = integration.uncertainty.clone();
    uncertainty.extend(workbench.uncertainty.clone());
    uncertainty.sort();
    uncertainty.dedup();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.interop_request.request_id,"endpoint_id":request.interop_request.source.endpoint_id,"negotiated_version":integration.negotiated_version,"disposition":integration.disposition,"input_schema":request.requested_input_schema,"output_schema":request.requested_output_schema,"required_modalities":request.required_modalities,"comparability_digest":request.comparability_digest,"migration_policy":request.migration_policy,"semantic_loss_order":loss,"omissions":omissions,"uncertainty":uncertainty,"workbench_digest":workbench.workbench_digest,"replay_token":request.interop_request.replay_token,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let protocol_digest = ContentHash::of_value(&payload).map_err(|e| {
        MultimodalRetrievalSynthesisInteroperabilityGatewayError::Artifact(e.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "multimodal-retrieval-interoperability:{}",
            request.interop_request.request_id
        ),
        "application/vnd.aurora.multimodal-retrieval-synthesis-interoperability+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| {
        MultimodalRetrievalSynthesisInteroperabilityGatewayError::Artifact(e.to_string())
    })?;
    let receipt = MultimodalRetrievalSynthesisInteroperabilityGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.interop_request.request_id.clone(),
        endpoint_id: request.interop_request.source.endpoint_id.clone(),
        negotiated_version: integration.negotiated_version,
        disposition: integration.disposition,
        input_schema: request.requested_input_schema.clone(),
        output_schema: request.requested_output_schema.clone(),
        required_modalities: request.required_modalities.clone(),
        comparability_digest: request.comparability_digest.clone(),
        migration_policy: request.migration_policy.clone(),
        semantic_loss_budget: request.semantic_loss_budget,
        capability_order: caps,
        artifact_digest_order: integration.artifact_digest_order,
        semantic_loss_order: loss,
        omissions,
        uncertainty,
        workbench_digest: workbench.workbench_digest,
        protocol_digest,
        effect_receipts: vec![if matches!(
            integration.disposition,
            InteroperabilityDisposition::Accepted | InteroperabilityDisposition::Migrated
        ) {
            "exchange:permitted-artifact-digests-only".into()
        } else {
            "block:unsafe-release".into()
        }],
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}
fn validate_request(
    r: &MultimodalRetrievalSynthesisInteroperabilityGatewayRequest,
) -> Result<(), MultimodalRetrievalSynthesisInteroperabilityGatewayError> {
    if r.requested_input_schema != INPUT_SCHEMA
        || r.requested_output_schema != OUTPUT_SCHEMA
        || r.required_modalities.len() < 2
        || r.required_modalities.windows(2).any(|p| p[0] >= p[1])
        || r.migration_policy.trim().is_empty()
        || r.semantic_loss_budget == 0
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.interop_request.boundary != PRECLINICAL_BOUNDARY
        || !r.interop_request.raw_data_local
    {
        return Err(MultimodalRetrievalSynthesisInteroperabilityGatewayError::Invalid("multimodal protocol schemas, modalities, policy, loss budget, locality, or boundary are invalid".into()));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a1_and_multimodal() {
        let m = multimodal_retrieval_synthesis_interoperability_gateway_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
        assert_eq!(INPUT_SCHEMA, "ScopedRetrievalQuery2@1");
    }
}
