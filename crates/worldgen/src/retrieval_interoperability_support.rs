//! Version-pinned protocol gateways for retrieval-synthesis workbenches (P02 F21–F24).
//!
//! These gateways negotiate capability and schema compatibility, project only content-addressed
//! artifacts, and preserve semantic-loss/omission state.  They never move raw observations or
//! execute a remote tool.

use bioprism_foundation::{TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::interoperability_support::{self, InteroperabilityDisposition, InteroperabilityRequest};
use crate::retrieval_workbench_support::{self, RetrievalWorkbenchRequest};

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.retrieval-interoperability-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalInteroperabilityRequest {
    pub interop_request: InteroperabilityRequest,
    pub workbench_request: RetrievalWorkbenchRequest,
    pub requested_input_schema: String,
    pub requested_output_schema: String,
    pub migration_policy: String,
    pub semantic_loss_budget: u32,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalInteroperabilityReceipt {
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
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetrievalInteroperabilityError {
    #[error("invalid retrieval interoperability request: {0}")]
    Invalid(String),
    #[error("retrieval interoperability workbench failed: {0}")]
    Workbench(String),
    #[error("retrieval interoperability artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit()) }
fn ordered<T: Ord>(values: &[T]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }

impl RetrievalInteroperabilityReceipt {
    pub fn validate(&self) -> Result<(), RetrievalInteroperabilityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.boundary != PRECLINICAL_BOUNDARY || self.artifact.boundary != PRECLINICAL_BOUNDARY || self.artifact.content_type != CONTENT_TYPE || !self.raw_data_local || !self.aggregate_only || self.request_id.trim().is_empty() || self.endpoint_id.trim().is_empty() || self.negotiated_version.trim().is_empty() || self.input_schema.trim().is_empty() || self.output_schema.trim().is_empty() || self.migration_policy.trim().is_empty() || self.capability_order.is_empty() || self.effect_receipts.is_empty() || !digest(&self.workbench_digest) || !digest(&self.protocol_digest) { return Err(RetrievalInteroperabilityError::Invalid("gateway identity, schemas, locality, digests, or effects are incomplete".into())); }
        if !ordered(&self.capability_order) || !ordered(&self.artifact_digest_order) || !ordered(&self.semantic_loss_order) || !ordered(&self.omissions) || !ordered(&self.uncertainty) || !ordered(&self.effect_receipts) { return Err(RetrievalInteroperabilityError::Invalid("gateway output ordering is not canonical".into())); }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && effect != "exchange:permitted-artifact-digests-only") { return Err(RetrievalInteroperabilityError::Invalid("gateway effect is outside digest-only exchange gate".into())); }
        self.artifact.validate_metadata().map_err(|error| RetrievalInteroperabilityError::Artifact(error.to_string()))
    }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["bioinformatician","research program lead","imaging core scientist"],"behavior":format!("negotiate a version-pinned retrieval protocol and exchange only permitted workbench digests for {scale}"),"value":"makes protocol compatibility, semantic loss, omissions, and raw-data locality machine-checkable","input_schema":input_schema,"output_schema":"EvidenceSynthesis5@1","effects":["exchange:permitted-artifact-digests-only","block:unsafe-release"],"permissions":["connect:approved-endpoints"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":PRECLINICAL_BOUNDARY})
}

pub fn negotiate(request: &RetrievalInteroperabilityRequest, feature_id: &str, contract_version: &str, require_approval: bool, require_federation: bool) -> Result<RetrievalInteroperabilityReceipt, RetrievalInteroperabilityError> {
    if request.boundary != PRECLINICAL_BOUNDARY || request.requested_input_schema.trim().is_empty() || request.requested_output_schema.trim().is_empty() || request.migration_policy.trim().is_empty() || request.semantic_loss_budget == 0 || request.interop_request.boundary != PRECLINICAL_BOUNDARY || request.workbench_request.boundary != PRECLINICAL_BOUNDARY { return Err(RetrievalInteroperabilityError::Invalid("gateway boundary, schemas, migration policy, or loss budget is invalid".into())); }
    let integration = interoperability_support::negotiate_interoperability(&request.interop_request).map_err(|error| RetrievalInteroperabilityError::Invalid(error.to_string()))?;
    let workbench = retrieval_workbench_support::render(&request.workbench_request, feature_id, contract_version, require_approval, require_federation).map_err(|error| RetrievalInteroperabilityError::Workbench(error.to_string()))?;
    let mut semantic_loss_order = integration.semantic_loss.iter().map(|loss| loss.field.clone()).collect::<Vec<_>>(); semantic_loss_order.sort(); semantic_loss_order.dedup();
    if semantic_loss_order.len() as u32 > request.semantic_loss_budget { return Err(RetrievalInteroperabilityError::Invalid("semantic-loss budget exceeded".into())); }
    let mut capability_order = integration.capability_order.clone(); capability_order.sort(); capability_order.dedup(); let mut artifact_digest_order = integration.artifact_digest_order.clone(); artifact_digest_order.sort(); artifact_digest_order.dedup();
    let mut omissions = integration.omissions.clone(); omissions.extend(workbench.omissions.clone()); omissions.sort(); omissions.dedup(); let mut uncertainty = integration.uncertainty.clone(); uncertainty.extend(workbench.uncertainty.clone()); uncertainty.sort(); uncertainty.dedup();
    let workbench_digest = workbench.workbench_digest.clone();
    let protocol_payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.interop_request.request_id,"endpoint_id":request.interop_request.source.endpoint_id,"negotiated_version":integration.negotiated_version,"disposition":integration.disposition,"input_schema":request.requested_input_schema,"output_schema":request.requested_output_schema,"migration_policy":request.migration_policy,"semantic_loss_budget":request.semantic_loss_budget,"capability_order":capability_order,"artifact_digest_order":artifact_digest_order,"semantic_loss_order":semantic_loss_order,"omissions":omissions,"uncertainty":uncertainty,"workbench_digest":workbench_digest,"replay_token":request.interop_request.replay_token,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let protocol_digest = ContentHash::of_value(&protocol_payload).map_err(|error| RetrievalInteroperabilityError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(format!("worldgen-retrieval-interoperability:{}", request.interop_request.request_id), CONTENT_TYPE, &protocol_payload, vec![], vec![]).map_err(|error| RetrievalInteroperabilityError::Artifact(error.to_string()))?;
    let safe = matches!(integration.disposition, InteroperabilityDisposition::Accepted | InteroperabilityDisposition::Migrated) && workbench.disposition != "blocked";
    let effect_receipts = if safe { vec!["exchange:permitted-artifact-digests-only".into()] } else { vec!["block:unsafe-release".into()] };
    let receipt = RetrievalInteroperabilityReceipt { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:contract_version.into(), feature_id:feature_id.into(), request_id:request.interop_request.request_id.clone(), endpoint_id:request.interop_request.source.endpoint_id.clone(), negotiated_version:integration.negotiated_version, disposition:integration.disposition, input_schema:request.requested_input_schema.clone(), output_schema:request.requested_output_schema.clone(), migration_policy:request.migration_policy.clone(), semantic_loss_budget:request.semantic_loss_budget, capability_order, artifact_digest_order, semantic_loss_order, omissions, uncertainty, workbench_digest, protocol_digest, effect_receipts, artifact, raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*; use crate::interoperability_support::ExternalCapability; use crate::retrieval_support::{RetrievalCandidate, RetrievalQuery}; use crate::retrieval_workbench_support::{RetrievalWorkbenchRequest, VIEWS, PANELS};
    fn hash(seed:&str)->ContentHash{ContentHash::of_bytes(seed.as_bytes())}
    fn request()->RetrievalInteroperabilityRequest { let candidate=RetrievalCandidate{candidate_id:"candidate:interop".into(),source_id:"source:interop".into(),title:"organoid evidence".into(),study_id:"study:interop".into(),modality:"imaging".into(),relevance_milli:900,freshness_milli:900,evidence_state:"supported".into(),content_digest:hash("content"),provenance_digest:hash("provenance"),replay_identity:hash("replay"),estimated_units:1,permitted:true,comparable:true,negative_result:false}; let query=RetrievalQuery{request_id:"interop:req".into(),researcher:"researcher:interop".into(),corpus_id:"corpus:local".into(),purpose:"inspect evidence".into(),semantic_profile:"prov-v1".into(),query_terms:vec!["organoid".into()],candidates:vec![candidate],minimum_relevance_milli:500,minimum_freshness_milli:500,max_budget_units:4,replay_identity:hash("replay"),policy_allow:true,protected_closure:true,raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()}; let copilot=crate::retrieval_copilot_support::RetrievalCopilotRequest{agent_id:"agent:interop".into(),query,allowed_actions:vec!["read:local-evidence".into()],requested_actions:vec!["read:local-evidence".into()],action_budget:2,dry_run:true,signed_approval:true,federation_approved:true}; let workbench=RetrievalWorkbenchRequest{copilot,workspace_id:"workspace:interop".into(),scope:"study:interop".into(),requested_view_order:VIEWS.iter().map(|v|(*v).into()).collect(),requested_panel_order:PANELS.iter().map(|v|(*v).into()).collect(),budget_units:4,replay_identity:hash("replay"),boundary:PRECLINICAL_BOUNDARY.into()}; let source=ExternalCapability{capability_id:"worldgen-retrieval".into(),endpoint_id:"endpoint:interop".into(),source_contract_version:"1.0.0".into(),supported_contract_versions:vec!["1.0.0".into()],offered_capabilities:vec!["retrieval-workbench".into()],required_capabilities:vec![],artifact_digests:vec![hash("artifact")],permitted_export:true,raw_data_local:true,boundary:PRECLINICAL_BOUNDARY.into()}; let interop=InteroperabilityRequest{request_id:"interop:req".into(),source,target_contract_version:"1.0.0".into(),target_capabilities:vec!["retrieval-workbench".into()],policy_allow:true,protected_closure:true,replay_token:hash("replay"),raw_data_local:true,boundary:PRECLINICAL_BOUNDARY.into()}; RetrievalInteroperabilityRequest{interop_request:interop,workbench_request:workbench,requested_input_schema:"ScopedRetrievalQuery1@1".into(),requested_output_schema:"EvidenceSynthesis5@1".into(),migration_policy:"additive-only".into(),semantic_loss_budget:4,boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn accepted_gateway_exchanges_digest_only(){let r=negotiate(&request(),"AFA-worldgen-P02-F21","worldgen-local-retrieval-synthesis-interoperability-gateway/1.0",false,false).unwrap();assert!(matches!(r.disposition,InteroperabilityDisposition::Accepted));assert_eq!(r.effect_receipts,vec!["exchange:permitted-artifact-digests-only"]);}
    #[test] fn denied_export_blocks(){let mut r=request();r.interop_request.policy_allow=false;let out=negotiate(&r,"AFA-worldgen-P02-F21","worldgen-local-retrieval-synthesis-interoperability-gateway/1.0",false,false).unwrap();assert_eq!(out.effect_receipts,vec!["block:unsafe-release"]);}
    #[test] fn missing_capability_is_not_promoted(){let mut r=request();r.interop_request.target_capabilities.push("missing-capability".into());let out=negotiate(&r,"AFA-worldgen-P02-F21","worldgen-local-retrieval-synthesis-interoperability-gateway/1.0",false,false).unwrap();assert_eq!(out.effect_receipts,vec!["block:unsafe-release"]);}
}
