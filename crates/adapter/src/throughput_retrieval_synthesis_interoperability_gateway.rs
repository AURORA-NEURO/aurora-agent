//! Version-negotiated prospective high-throughput retrieval/synthesis gateway.
//! Atlas feature `AFA-adapter-P02-F23`.
use crate::interoperability_gateway::{
    negotiate_interoperability, InteroperabilityDisposition, InteroperabilityRequest,
};
use crate::throughput_retrieval_synthesis_research_workbench::{
    render_throughput_retrieval_synthesis_research_workbench,
    ThroughputRetrievalSynthesisResearchWorkbenchRequest,
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
pub const FEATURE_ID: &str = "AFA-adapter-P02-F23";
pub const CONTRACT_VERSION: &str =
    "adapter-throughput-retrieval-synthesis-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis7@1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisInteroperabilityGatewayRequest {
    pub interop_request: InteroperabilityRequest,
    pub workbench_request: ThroughputRetrievalSynthesisResearchWorkbenchRequest,
    pub requested_input_schema: String,
    pub requested_output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: u32,
    pub migration_policy: String,
    pub semantic_loss_budget: u32,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisInteroperabilityGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub endpoint_id: String,
    pub negotiated_version: String,
    pub disposition: InteroperabilityDisposition,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: u32,
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
pub enum ThroughputRetrievalSynthesisInteroperabilityGatewayError {
    #[error("invalid throughput retrieval interoperability request: {0}")]
    Invalid(String),
    #[error("throughput interoperability artifact failed: {0}")]
    Artifact(String),
    #[error("throughput workbench failed: {0}")]
    Workbench(String),
}
impl ThroughputRetrievalSynthesisInteroperabilityGatewayReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalSynthesisInteroperabilityGatewayError> {
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
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
            || self.migration_policy.trim().is_empty()
            || self.semantic_loss_budget == 0
            || self.capability_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid("throughput gateway identity, schemas, batch, capacity, locality, budget, or effects are incomplete"));
        }
        if self.capability_order.windows(2).any(|p| p[0] >= p[1])
            || self.artifact_digest_order.windows(2).any(|p| p[0] >= p[1])
            || self.semantic_loss_order.windows(2).any(|p| p[0] >= p[1])
            || self.omissions.windows(2).any(|p| p[0] >= p[1])
            || self.uncertainty.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(Self::invalid(
                "throughput gateway output ordering is not canonical",
            ));
        }
        for d in [
            &self.workbench_digest,
            &self.protocol_digest,
            &self.artifact.content_hash,
        ] {
            if d.as_str().len() != 64 {
                return Err(Self::invalid("throughput gateway digest is invalid"));
            }
        }
        self.artifact.validate_metadata().map_err(|e| {
            ThroughputRetrievalSynthesisInteroperabilityGatewayError::Artifact(e.to_string())
        })
    }
    fn invalid(m: &str) -> ThroughputRetrievalSynthesisInteroperabilityGatewayError {
        ThroughputRetrievalSynthesisInteroperabilityGatewayError::Invalid(m.into())
    }
}
pub fn throughput_retrieval_synthesis_interoperability_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["consortium administrator".into(),"preclinical researcher".into()].into(),behavior:"negotiates throughput retrieval/synthesis protocol versions with bounded batch and checkpoint admission".into(),value:"makes high-throughput capacity, migration loss, and permitted artifact exchange auditable".into(),inputs:vec![TypedPort{name:"scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true},TypedPort{name:"throughput_admission".into(),schema:"BatchCheckpoint@1".into(),required:true}],outputs:vec![TypedPort{name:"evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["connect:approved-endpoints".into(),"operate:bounded-batch".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"ga4gh-wes".into(),state:EvidenceState::Supported,locator:Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Ui,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn render_throughput_retrieval_synthesis_interoperability_gateway(
    r: &ThroughputRetrievalSynthesisInteroperabilityGatewayRequest,
) -> Result<
    ThroughputRetrievalSynthesisInteroperabilityGatewayReceipt,
    ThroughputRetrievalSynthesisInteroperabilityGatewayError,
> {
    if r.requested_input_schema != INPUT_SCHEMA
        || r.requested_output_schema != OUTPUT_SCHEMA
        || r.batch_id.trim().is_empty()
        || r.checkpoint_seq == 0
        || r.capacity == 0
        || r.migration_policy.trim().is_empty()
        || r.semantic_loss_budget == 0
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.interop_request.boundary != PRECLINICAL_BOUNDARY
        || !r.interop_request.raw_data_local
    {
        return Err(ThroughputRetrievalSynthesisInteroperabilityGatewayError::Invalid("throughput protocol schemas, batch, checkpoint, capacity, locality, or boundary are invalid".into()));
    }
    let i = negotiate_interoperability(&r.interop_request).map_err(|e| {
        ThroughputRetrievalSynthesisInteroperabilityGatewayError::Invalid(e.to_string())
    })?;
    let w = render_throughput_retrieval_synthesis_research_workbench(&r.workbench_request)
        .map_err(|e| {
            ThroughputRetrievalSynthesisInteroperabilityGatewayError::Workbench(e.to_string())
        })?;
    let mut caps = i.capability_order.clone();
    caps.sort();
    caps.dedup();
    let mut loss = i
        .semantic_loss
        .iter()
        .map(|x| x.field.clone())
        .collect::<Vec<_>>();
    loss.sort();
    loss.dedup();
    if loss.len() as u32 > r.semantic_loss_budget {
        return Err(
            ThroughputRetrievalSynthesisInteroperabilityGatewayError::Invalid(
                "semantic-loss budget exceeded".into(),
            ),
        );
    }
    let mut omissions = i.omissions.clone();
    omissions.extend(w.omissions.clone());
    omissions.sort();
    omissions.dedup();
    let mut uncertainty = i.uncertainty.clone();
    uncertainty.extend(w.uncertainty.clone());
    uncertainty.sort();
    uncertainty.dedup();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r.interop_request.request_id,"endpoint_id":r.interop_request.source.endpoint_id,"negotiated_version":i.negotiated_version,"disposition":i.disposition,"input_schema":r.requested_input_schema,"output_schema":r.requested_output_schema,"batch_id":r.batch_id,"checkpoint_seq":r.checkpoint_seq,"capacity":r.capacity,"migration_policy":r.migration_policy,"semantic_loss_order":loss,"omissions":omissions,"uncertainty":uncertainty,"workbench_digest":w.workbench_digest,"replay_token":r.interop_request.replay_token,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let pd = ContentHash::of_value(&payload).map_err(|e| {
        ThroughputRetrievalSynthesisInteroperabilityGatewayError::Artifact(e.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "throughput-retrieval-interoperability:{}",
            r.interop_request.request_id
        ),
        "application/vnd.aurora.throughput-retrieval-synthesis-interoperability+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| {
        ThroughputRetrievalSynthesisInteroperabilityGatewayError::Artifact(e.to_string())
    })?;
    let receipt = ThroughputRetrievalSynthesisInteroperabilityGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.interop_request.request_id.clone(),
        endpoint_id: r.interop_request.source.endpoint_id.clone(),
        negotiated_version: i.negotiated_version,
        disposition: i.disposition,
        input_schema: r.requested_input_schema.clone(),
        output_schema: r.requested_output_schema.clone(),
        batch_id: r.batch_id.clone(),
        checkpoint_seq: r.checkpoint_seq,
        capacity: r.capacity,
        migration_policy: r.migration_policy.clone(),
        semantic_loss_budget: r.semantic_loss_budget,
        capability_order: caps,
        artifact_digest_order: i.artifact_digest_order,
        semantic_loss_order: loss,
        omissions,
        uncertainty,
        workbench_digest: w.workbench_digest,
        protocol_digest: pd,
        effect_receipts: vec![if matches!(
            i.disposition,
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
    fn manifest_is_a1_and_bounded() {
        let m = throughput_retrieval_synthesis_interoperability_gateway_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
        assert_eq!(INPUT_SCHEMA, "ScopedRetrievalQuery3@1");
    }
}
