//! Version-negotiated throughput evidence-surveillance interoperability gateway.
//! Atlas feature `AFA-worldgen-P01-F23`.
//!
//! The gateway binds the local single-study researcher workbench to a typed
//! protocol envelope. It negotiates pinned input/output versions, records
//! semantic-loss and capability omissions, and exchanges only content-addressed
//! artifacts. Raw preclinical observations never cross the gateway boundary.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::interoperability_support::{
    negotiate_interoperability, InteroperabilityDisposition, InteroperabilityRequest,
};
use crate::throughput_evidence_surveillance_research_workbench::{
    render_throughput_evidence_surveillance_research_workbench,
    ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
};

pub const FEATURE_ID: &str = "AFA-worldgen-P01-F23";
pub const CONTRACT_VERSION: &str =
    "worldgen-throughput-evidence-surveillance-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet6@1";
pub const TARGET_PROTOCOL_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceInteroperabilityGatewayRequest {
    pub interop_request: InteroperabilityRequest,
    pub workbench_request: ThroughputEvidenceSurveillanceResearchWorkbenchRequest,
    pub requested_input_schema: String,
    pub requested_output_schema: String,
    pub migration_policy: String,
    pub semantic_loss_budget: u32,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub endpoint_id: String,
    pub negotiated_version: String,
    pub disposition: InteroperabilityDisposition,
    pub input_schema: String,
    pub output_schema: String,
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
pub enum ThroughputEvidenceSurveillanceInteroperabilityGatewayError {
    #[error("invalid local retrieval interoperability request: {0}")]
    Invalid(String),
    #[error("local retrieval interoperability artifact failed: {0}")]
    Artifact(String),
    #[error("local retrieval workbench failed: {0}")]
    Workbench(String),
}

impl ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceInteroperabilityGatewayError> {
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
            || self.migration_policy.trim().is_empty()
            || self.capability_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid(
                "gateway identity, schemas, capabilities, locality, or effects are incomplete",
            ));
        }
        if self
            .capability_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self
                .artifact_digest_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .semantic_loss_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(Self::invalid("gateway output ordering is not canonical"));
        }
        for digest in [
            &self.workbench_digest,
            &self.protocol_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(Self::invalid(
                    "gateway digest is not a 256-bit content hash",
                ));
            }
        }
        self.artifact.validate_metadata().map_err(|error| {
            ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Artifact(error.to_string())
        })
    }

    fn invalid(message: &str) -> ThroughputEvidenceSurveillanceInteroperabilityGatewayError {
        ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Invalid(message.into())
    }
}

pub fn throughput_evidence_surveillance_interoperability_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "worldgen".into(),
        consumers: ["bioinformatician".into(), "preclinical researcher".into()].into(),
        behavior: "negotiates a version-pinned throughput evidence-surveillance protocol and projects a typed workbench artifact".into(),
        value: "makes API/protocol compatibility, semantic loss, omissions, and local-only effects machine-checkable".into(),
        inputs: vec![
            TypedPort { name: "throughput_evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true },
            TypedPort { name: "protocol_capability_manifest".into(), schema: "CapabilityManifest@1".into(), required: true },
        ],
        outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation].into(),
        permissions: ["connect:approved-endpoints".into(), "view:authorized-research-state".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) },
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::Ui, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn render_throughput_evidence_surveillance_interoperability_gateway(
    request: &ThroughputEvidenceSurveillanceInteroperabilityGatewayRequest,
) -> Result<
    ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt,
    ThroughputEvidenceSurveillanceInteroperabilityGatewayError,
> {
    validate_request(request)?;
    let integration = negotiate_interoperability(&request.interop_request).map_err(|error| {
        ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Invalid(error.to_string())
    })?;
    let workbench = render_throughput_evidence_surveillance_research_workbench(
        &request.workbench_request,
    )
    .map_err(|error| {
        ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Workbench(error.to_string())
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
            ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Invalid(
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
    let workbench_digest = workbench.workbench_digest.clone();
    let protocol_payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.interop_request.request_id,
        "endpoint_id": request.interop_request.source.endpoint_id,
        "negotiated_version": integration.negotiated_version,
        "disposition": integration.disposition,
        "input_schema": request.requested_input_schema,
        "output_schema": request.requested_output_schema,
        "migration_policy": request.migration_policy,
        "semantic_loss_budget": request.semantic_loss_budget,
        "capability_order": capabilities,
        "artifact_digest_order": integration.artifact_digest_order,
        "semantic_loss_order": loss,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "workbench_digest": workbench_digest,
        "replay_token": request.interop_request.replay_token,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let protocol_digest = ContentHash::of_value(&protocol_payload).map_err(|error| {
        ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Artifact(error.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "local-retrieval-interoperability:{}",
            request.interop_request.request_id
        ),
        "application/vnd.aurora.worldgen-throughput-evidence-surveillance-interoperability+json",
        &protocol_payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Artifact(error.to_string())
    })?;
    let receipt = ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.interop_request.request_id.clone(),
        endpoint_id: request.interop_request.source.endpoint_id.clone(),
        negotiated_version: integration.negotiated_version,
        disposition: integration.disposition,
        input_schema: request.requested_input_schema.clone(),
        output_schema: request.requested_output_schema.clone(),
        migration_policy: request.migration_policy.clone(),
        semantic_loss_budget: request.semantic_loss_budget,
        capability_order: capabilities,
        artifact_digest_order: integration.artifact_digest_order,
        semantic_loss_order: loss,
        omissions,
        uncertainty,
        workbench_digest,
        protocol_digest,
        effect_receipts: vec![if matches!(
            integration.disposition,
            InteroperabilityDisposition::Blocked | InteroperabilityDisposition::Incompatible
        ) {
            "block:unsafe-release".into()
        } else {
            "exchange:permitted-artifact-digests-only".into()
        }],
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ThroughputEvidenceSurveillanceInteroperabilityGatewayRequest,
) -> Result<(), ThroughputEvidenceSurveillanceInteroperabilityGatewayError> {
    if request.requested_input_schema != INPUT_SCHEMA
        || request.requested_output_schema != OUTPUT_SCHEMA
        || request.migration_policy.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.semantic_loss_budget == 0
        || request.interop_request.boundary != PRECLINICAL_BOUNDARY
        || !request.interop_request.raw_data_local
        || request.workbench_request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputEvidenceSurveillanceInteroperabilityGatewayError::Invalid("protocol schemas, migration policy, loss budget, locality, and boundary are invalid".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_a1_and_pinned() {
        let manifest = throughput_evidence_surveillance_interoperability_gateway_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(TARGET_PROTOCOL_VERSION, "1.0.0");
        assert_eq!(INPUT_SCHEMA, "EvidenceFeed3@1");
    }
}


