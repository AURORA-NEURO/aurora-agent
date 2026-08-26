//! Federated continual retrieval/synthesis interoperability gateway.
//! Atlas feature `AFA-adapter-P02-F24`.
//!
//! This gateway negotiates a version-pinned protocol for policy-separated
//! institutions. Only permitted artifact digests and federation metadata cross
//! the boundary; raw preclinical observations remain at their origin.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::federated_continual_retrieval_synthesis_research_workbench::{
    render_federated_continual_retrieval_synthesis_research_workbench,
    FederatedContinualRetrievalSynthesisResearchWorkbenchRequest,
};
use crate::interoperability_gateway::{
    negotiate_interoperability, InteroperabilityDisposition, InteroperabilityRequest,
};

pub const FEATURE_ID: &str = "AFA-adapter-P02-F24";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-retrieval-synthesis-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery4@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalSynthesisInteroperabilityGatewayRequest {
    pub interop_request: InteroperabilityRequest,
    pub workbench_request: FederatedContinualRetrievalSynthesisResearchWorkbenchRequest,
    pub federation_id: String,
    pub purpose: String,
    pub peer_ids: Vec<String>,
    pub min_peer_quorum: u32,
    pub aggregate_only: bool,
    pub requested_input_schema: String,
    pub requested_output_schema: String,
    pub federation_digest: ContentHash,
    pub migration_policy: String,
    pub semantic_loss_budget: u32,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalSynthesisInteroperabilityGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub endpoint_id: String,
    pub negotiated_version: String,
    pub disposition: InteroperabilityDisposition,
    pub federation_id: String,
    pub purpose: String,
    pub peer_ids: Vec<String>,
    pub min_peer_quorum: u32,
    pub aggregate_only: bool,
    pub input_schema: String,
    pub output_schema: String,
    pub federation_digest: ContentHash,
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
pub enum FederatedContinualRetrievalSynthesisInteroperabilityGatewayError {
    #[error("invalid federated continual retrieval interoperability request: {0}")]
    Invalid(String),
    #[error("federated continual interoperability artifact failed: {0}")]
    Artifact(String),
    #[error("federated continual workbench failed: {0}")]
    Workbench(String),
}

impl FederatedContinualRetrievalSynthesisInteroperabilityGatewayReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), FederatedContinualRetrievalSynthesisInteroperabilityGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.endpoint_id.trim().is_empty()
            || self.negotiated_version.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.peer_ids.len() < self.min_peer_quorum as usize
            || self.peer_ids.windows(2).any(|pair| pair[0] >= pair[1])
            || self.min_peer_quorum == 0
            || !self.aggregate_only
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.migration_policy.trim().is_empty()
            || self.semantic_loss_budget == 0
            || self.capability_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid(
                "federation identity, quorum, schemas, locality, budget, or effects are incomplete",
            ));
        }
        for values in [
            &self.capability_order,
            &self.semantic_loss_order,
            &self.omissions,
            &self.uncertainty,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(Self::invalid(
                    "federated gateway output ordering is not canonical",
                ));
            }
        }
        if self
            .artifact_digest_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(Self::invalid(
                "federated artifact digest ordering is not canonical",
            ));
        }
        for digest in [
            &self.federation_digest,
            &self.workbench_digest,
            &self.protocol_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(Self::invalid("federated gateway digest is invalid"));
            }
        }
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Artifact(
                error.to_string(),
            )
        })
    }
    fn invalid(message: &str) -> FederatedContinualRetrievalSynthesisInteroperabilityGatewayError {
        FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Invalid(message.into())
    }
}

pub fn federated_continual_retrieval_synthesis_interoperability_gateway_manifest(
) -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(),
        consumers: ["integration engineer".into(), "consortium administrator".into()].into(),
        behavior: "negotiates a purpose-bound federated continual retrieval/synthesis protocol with quorum and aggregate-only controls".into(),
        value: "makes cross-institution compatibility, federation locality, quorum, and semantic loss auditable".into(),
        inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }, TypedPort { name: "federation_manifest".into(), schema: "FederationEnvelope@1".into(), required: true }],
        outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation].into(), permissions: ["connect:approved-endpoints".into(), "exchange:permitted-artifact-digests".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::Ui, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn render_federated_continual_retrieval_synthesis_interoperability_gateway(
    request: &FederatedContinualRetrievalSynthesisInteroperabilityGatewayRequest,
) -> Result<
    FederatedContinualRetrievalSynthesisInteroperabilityGatewayReceipt,
    FederatedContinualRetrievalSynthesisInteroperabilityGatewayError,
> {
    if request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.peer_ids.len() < request.min_peer_quorum as usize
        || request.peer_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || request.min_peer_quorum == 0
        || !request.aggregate_only
        || request.requested_input_schema != INPUT_SCHEMA
        || request.requested_output_schema != OUTPUT_SCHEMA
        || request.federation_digest.as_str().len() != 64
        || request.migration_policy.trim().is_empty()
        || request.semantic_loss_budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.interop_request.boundary != PRECLINICAL_BOUNDARY
        || !request.interop_request.raw_data_local
    {
        return Err(FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Invalid("federation identity, purpose, quorum, schemas, aggregate-only policy, loss budget, locality, or boundary is invalid".into()));
    }
    let integration = negotiate_interoperability(&request.interop_request).map_err(|error| {
        FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Invalid(error.to_string())
    })?;
    let workbench = render_federated_continual_retrieval_synthesis_research_workbench(
        &request.workbench_request,
    )
    .map_err(|error| {
        FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Workbench(
            error.to_string(),
        )
    })?;
    let mut capabilities = integration.capability_order.clone();
    capabilities.sort();
    capabilities.dedup();
    let mut loss = integration
        .semantic_loss
        .iter()
        .map(|item| item.field.clone())
        .collect::<Vec<_>>();
    loss.sort();
    loss.dedup();
    if loss.len() as u32 > request.semantic_loss_budget {
        return Err(
            FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Invalid(
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
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.interop_request.request_id,"endpoint_id":request.interop_request.source.endpoint_id,"negotiated_version":integration.negotiated_version,"disposition":integration.disposition,"federation_id":request.federation_id,"purpose":request.purpose,"peer_ids":request.peer_ids,"min_peer_quorum":request.min_peer_quorum,"aggregate_only":request.aggregate_only,"input_schema":request.requested_input_schema,"output_schema":request.requested_output_schema,"federation_digest":request.federation_digest,"migration_policy":request.migration_policy,"semantic_loss_budget":request.semantic_loss_budget,"capability_order":capabilities,"artifact_digest_order":integration.artifact_digest_order,"semantic_loss_order":loss,"omissions":omissions,"uncertainty":uncertainty,"workbench_digest":workbench.workbench_digest,"replay_token":request.interop_request.replay_token,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let protocol_digest = ContentHash::of_value(&payload).map_err(|error| {
        FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Artifact(
            error.to_string(),
        )
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "federated-continual-retrieval-interoperability:{}",
            request.interop_request.request_id
        ),
        "application/vnd.aurora.federated-continual-retrieval-synthesis-interoperability+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        FederatedContinualRetrievalSynthesisInteroperabilityGatewayError::Artifact(
            error.to_string(),
        )
    })?;
    let receipt = FederatedContinualRetrievalSynthesisInteroperabilityGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.interop_request.request_id.clone(),
        endpoint_id: request.interop_request.source.endpoint_id.clone(),
        negotiated_version: integration.negotiated_version,
        disposition: integration.disposition,
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        peer_ids: request.peer_ids.clone(),
        min_peer_quorum: request.min_peer_quorum,
        aggregate_only: request.aggregate_only,
        input_schema: request.requested_input_schema.clone(),
        output_schema: request.requested_output_schema.clone(),
        federation_digest: request.federation_digest.clone(),
        migration_policy: request.migration_policy.clone(),
        semantic_loss_budget: request.semantic_loss_budget,
        capability_order: capabilities,
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a1_and_federated() {
        let manifest = federated_continual_retrieval_synthesis_interoperability_gateway_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert_eq!(INPUT_SCHEMA, "ScopedRetrievalQuery4@1");
    }
}
