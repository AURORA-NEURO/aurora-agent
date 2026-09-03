//! Prospective high-throughput retrieval-and-synthesis operations control plane.
//!
//! Atlas feature: `AFA-lab-P02-F31`.
//!
//! This A2 surface operates the existing retrieval assurance contract at a bounded institution
//! node.  It admits capacity and authority, invokes the local assurance boundary, and emits an
//! auditable operation receipt.  It does not perform network retrieval, export raw evidence, or
//! silently substitute an incomplete synthesis.

use crate::federated_retrieval_synthesis_assurance::{
    assure_federated_retrieval_synthesis, ScopedRetrievalQuery, SynthesisDisposition,
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

pub const FEATURE_ID: &str = "AFA-lab-P02-F31";
pub const CONTRACT_VERSION: &str =
    "lab-prospective-high-throughput-retrieval-synthesis-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis8@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.lab-retrieval-synthesis-operations-receipt-9+json";
pub const MAX_CAPACITY: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalOperationsRequest7 {
    pub request_id: String,
    pub query: ScopedRetrievalQuery,
    pub queue_depth: u32,
    pub active_runs: u32,
    pub capacity: u32,
    pub budget_units: u32,
    pub requested_effects: Vec<String>,
    pub authority_present: bool,
    pub checkpoint_digest: ContentHash,
    pub network_permitted: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalOperationsReceipt9 {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub request_id: String,
    pub query_id: String,
    pub federation_id: String,
    pub disposition: String,
    pub operation_order: Vec<String>,
    pub selected_operation_order: Vec<String>,
    pub unresolved_operation_order: Vec<String>,
    pub blocked_operation_order: Vec<String>,
    pub capacity_order: Vec<String>,
    pub authority_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub synthesis_disposition: SynthesisDisposition,
    pub synthesis_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_order: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error)]
pub enum RetrievalOperationsError {
    #[error("invalid retrieval operations field: {0}")]
    Invalid(String),
    #[error("retrieval operations synthesis failed: {0}")]
    Synthesis(String),
    #[error("retrieval operations artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl RetrievalOperationsReceipt9 {
    pub fn validate(&self) -> Result<(), RetrievalOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.operation_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !ordered(&self.operation_order)
            || !ordered(&self.selected_operation_order)
            || !ordered(&self.unresolved_operation_order)
            || !ordered(&self.blocked_operation_order)
            || !ordered(&self.capacity_order)
            || !ordered(&self.authority_order)
            || !ordered(&self.omission_order)
            || !ordered(&self.uncertainty_order)
            || !ordered(&self.negative_evidence_order)
            || !ordered(&self.effect_order)
            || !ordered(&self.effect_receipts)
            || self.selected_operation_order.len()
                + self.unresolved_operation_order.len()
                + self.blocked_operation_order.len()
                != self.operation_order.len()
            || !same_partition(
                &self.operation_order,
                &[
                    &self.selected_operation_order,
                    &self.unresolved_operation_order,
                    &self.blocked_operation_order,
                ],
            )
            || !digest(&self.synthesis_digest)
            || !digest(&self.checkpoint_digest)
            || !digest(&self.replay_identity)
            || self.artifact.content_hash != self.synthesis_digest
            || self.artifact.content_type != CONTENT_TYPE
        {
            return Err(RetrievalOperationsError::Invalid("operations identity, canonical partitions, capacity evidence, digests, locality, or effects are incomplete".into()));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalOperationsError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalOperationsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalOperationsError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalOperationsError::Artifact(error.to_string()))
    }
}

fn same_partition(all: &[String], parts: &[&Vec<String>]) -> bool {
    let mut combined = parts
        .iter()
        .flat_map(|part| part.iter().cloned())
        .collect::<Vec<_>>();
    combined.sort();
    let mut expected = all.to_vec();
    expected.sort();
    combined == expected && combined.windows(2).all(|pair| pair[0] != pair[1])
}

pub fn retrieval_synthesis_operations_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "lab".into(),
        consumers: ["bioinformatician".into(), "institution operations steward".into(), "consortium operator".into()].into(),
        behavior: "admit and operate a bounded prospective retrieval-and-synthesis service using typed assurance, capacity, authority, checkpoint, policy, and federation gates while preserving omissions".into(),
        value: "turn high-throughput evidence synthesis into an auditable local control-plane product without hiding missing evidence or exporting raw data".into(),
        inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["operate:institution-node".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![bioprism_foundation::AuthorityRequirement { role: "institution operations steward".into(), reason: "A2 control-plane capacity and permitted-summary exchange require explicit authority".into() }], autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate_retrieval_synthesis(
    request: &RetrievalOperationsRequest7,
) -> Result<RetrievalOperationsReceipt9, RetrievalOperationsError> {
    validate_request(request)?;
    let synthesis = assure_federated_retrieval_synthesis(&request.query)
        .map_err(|error| RetrievalOperationsError::Synthesis(error.to_string()))?;
    let operation_order = vec![
        "admit".into(),
        "retrieve-local".into(),
        "synthesize".into(),
        "checkpoint".into(),
    ];
    let mut selected = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut capacity_order = Vec::new();
    let mut authority_order = Vec::new();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut negative = Vec::new();
    let capacity_ok = request.queue_depth < request.capacity
        && request.active_runs < request.capacity
        && request.budget_units > 0;
    if !capacity_ok {
        unresolved.extend(operation_order.iter().cloned());
        capacity_order.push("capacity:queue-or-active-run-limit".into());
        uncertainty.push("capacity or budget is unresolved".into());
    }
    if !request.authority_present {
        blocked.extend(operation_order.iter().cloned());
        authority_order.push("authority:missing-institution-operator".into());
        negative.push("authority gate denied operation".into());
    }
    if !request.network_permitted {
        omissions.push("network retrieval remains disabled; local corpus only".into());
    }
    if !request.adversarial_events.is_empty() {
        blocked.extend(operation_order.iter().cloned());
        negative.extend(
            request
                .adversarial_events
                .iter()
                .map(|event| format!("adversarial:{event}")),
        );
    }
    if synthesis.disposition != SynthesisDisposition::Qualified {
        unresolved.extend(operation_order.iter().cloned());
        uncertainty.push("underlying evidence synthesis is not qualified".into());
    }
    if blocked.is_empty() && unresolved.is_empty() {
        selected.extend(operation_order.iter().cloned());
    }
    selected.sort();
    selected.dedup();
    unresolved.sort();
    unresolved.dedup();
    blocked.sort();
    blocked.dedup();
    capacity_order.sort();
    authority_order.sort();
    omissions.sort();
    uncertainty.sort();
    negative.sort();
    let disposition = if !blocked.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() || synthesis.disposition != SynthesisDisposition::Qualified {
        "unresolved"
    } else {
        "qualified"
    };
    let effect_order: Vec<String> = if disposition == "qualified" {
        vec![
            "exchange:permitted-summaries".into(),
            "manage:local-capability".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let effect_receipts = effect_order
        .iter()
        .map(|effect| {
            if effect == "block:unsafe-release" {
                effect.clone()
            } else {
                format!("{effect}:{}", request.request_id)
            }
        })
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"feature_id":FEATURE_ID,"contract_version":CONTRACT_VERSION,"request_id":request.request_id,"query_id":request.query.query_id,"federation_id":request.query.federation_id,"disposition":disposition,"operation_order":operation_order,"selected_operation_order":selected,"unresolved_operation_order":unresolved,"blocked_operation_order":blocked,"capacity_order":capacity_order,"authority_order":authority_order,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"synthesis_disposition":synthesis.disposition,"checkpoint_digest":request.checkpoint_digest,"replay_identity":request.query.replay_identity,"effect_order":effect_order,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let synthesis_digest = ContentHash::of_value(&payload)
        .map_err(|error| RetrievalOperationsError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("lab-retrieval-synthesis-operations:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalOperationsError::Artifact(error.to_string()))?;
    let receipt = RetrievalOperationsReceipt9 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        query_id: request.query.query_id.clone(),
        federation_id: request.query.federation_id.clone(),
        disposition: disposition.into(),
        operation_order: payload["operation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_operation_order: selected,
        unresolved_operation_order: unresolved,
        blocked_operation_order: blocked,
        capacity_order: capacity_order,
        authority_order: authority_order,
        omission_order: omissions,
        uncertainty_order: uncertainty,
        negative_evidence_order: negative,
        synthesis_disposition: synthesis.disposition,
        synthesis_digest,
        checkpoint_digest: request.checkpoint_digest.clone(),
        replay_identity: request.query.replay_identity.clone(),
        artifact,
        effect_order,
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &RetrievalOperationsRequest7) -> Result<(), RetrievalOperationsError> {
    if request.request_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.capacity == 0
        || request.capacity > MAX_CAPACITY
        || request.queue_depth > request.capacity
        || request.active_runs > request.capacity
        || request.budget_units == 0
        || !digest(&request.checkpoint_digest)
        || !request.query.raw_data_local
        || !request.query.aggregate_only
    {
        return Err(RetrievalOperationsError::Invalid("request identity, bounded capacity, budget, checkpoint, query locality, or boundary is invalid".into()));
    }
    if request.requested_effects.is_empty()
        || !ordered(&request.requested_effects)
        || request.requested_effects.iter().any(|effect| {
            effect != "exchange:permitted-summaries" && effect != "manage:local-capability"
        })
    {
        return Err(RetrievalOperationsError::Invalid("requested effects must be canonical and limited to permitted-summary exchange or local capability management".into()));
    }
    if !ordered(&request.adversarial_events) {
        return Err(RetrievalOperationsError::Invalid(
            "adversarial events must be canonical".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federated_retrieval_synthesis_assurance::{
        PeerSynthesisSummary, RetrievalCandidate,
    };
    fn hash(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn query() -> ScopedRetrievalQuery {
        let d = hash("retrieval");
        ScopedRetrievalQuery {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            query_id: "query:ops".into(),
            federation_id: "fed:ops".into(),
            semantic_profile: "preclinical-neural".into(),
            required_evidence_order: vec!["evidence:a".into()],
            required_scope_order: vec!["organoid-study".into()],
            minimum_freshness_epoch: 5,
            candidates: vec![RetrievalCandidate {
                evidence_id: "evidence:a".into(),
                study_id: "study:a".into(),
                source_id: "source:a".into(),
                scope: "organoid-study".into(),
                relevance_milli: 900,
                freshness_epoch: 10,
                semantic_profile: "preclinical-neural".into(),
                content_digest: d.clone(),
                provenance_digest: d.clone(),
                replay_identity: d.clone(),
                evidence_state: EvidenceState::Supported,
                omissions: vec![],
                uncertainty: vec![],
                negative_result: false,
                local_only: true,
                permitted: true,
            }],
            peers: vec![
                PeerSynthesisSummary {
                    institution_id: "inst:a".into(),
                    evidence_digest: d.clone(),
                    semantic_profile: "preclinical-neural".into(),
                    replay_identity: d.clone(),
                    signed: true,
                    permitted: true,
                    aggregate_only: true,
                },
                PeerSynthesisSummary {
                    institution_id: "inst:b".into(),
                    evidence_digest: d.clone(),
                    semantic_profile: "preclinical-neural".into(),
                    replay_identity: d.clone(),
                    signed: true,
                    permitted: true,
                    aggregate_only: true,
                },
            ],
            minimum_peer_quorum: 2,
            replay_identity: d,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request() -> RetrievalOperationsRequest7 {
        RetrievalOperationsRequest7 {
            request_id: "request:ops".into(),
            query: query(),
            queue_depth: 1,
            active_runs: 1,
            capacity: 16,
            budget_units: 10,
            requested_effects: vec![
                "exchange:permitted-summaries".into(),
                "manage:local-capability".into(),
            ],
            authority_present: true,
            checkpoint_digest: hash("checkpoint"),
            network_permitted: false,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            retrieval_synthesis_operations_manifest().autonomy_tier,
            AutonomyTier::A2
        );
        assert!(retrieval_synthesis_operations_manifest().validate().is_ok());
    }
    #[test]
    fn qualified_operation() {
        let r = operate_retrieval_synthesis(&request()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert!(r
            .effect_receipts
            .iter()
            .any(|e| e.starts_with("exchange:permitted-summaries:")));
    }
    #[test]
    fn capacity_is_unresolved() {
        let mut q = request();
        q.queue_depth = q.capacity;
        assert_eq!(
            operate_retrieval_synthesis(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn missing_authority_blocks() {
        let mut q = request();
        q.authority_present = false;
        assert_eq!(
            operate_retrieval_synthesis(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn adversarial_event_blocks() {
        let mut q = request();
        q.adversarial_events = vec!["poisoned-source".into()];
        assert_eq!(
            operate_retrieval_synthesis(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let r = operate_retrieval_synthesis(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
