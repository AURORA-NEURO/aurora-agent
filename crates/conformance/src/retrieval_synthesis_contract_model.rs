//! Prospective high-throughput retrieval/synthesis contract model (`AFA-conformance-P02-F07`).
//!
//! The model negotiates typed retrieval envelopes and records compatibility, omissions, and
//! evidence state before a synthesis engine is selected. It never fetches sources or exports raw
//! documents.
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-conformance-P02-F07";
pub const CONTRACT_VERSION: &str =
    "conformance-prospective-high-throughput-retrieval-synthesis-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.conformance-evidence-synthesis-2+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalContractEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCandidate3 {
    pub candidate_id: String,
    pub source_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub relevance_milli: u16,
    pub freshness_milli: u16,
    pub evidence_state: RetrievalContractEvidenceState,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub comparable: bool,
    pub permitted: bool,
    pub local_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery3 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub scope: String,
    pub semantic_profile: String,
    pub input_schema: String,
    pub output_schema: String,
    pub minimum_relevance_milli: u16,
    pub minimum_freshness_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub candidates: Vec<RetrievalCandidate3>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis2Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub scope: String,
    pub semantic_profile: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub compatible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub contract_digest: ContentHash,
    pub artifact: EvidenceSynthesis2Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetrievalContractError {
    #[error("invalid retrieval contract request or receipt: {0}")]
    Invalid(String),
    #[error("retrieval contract artifact failed: {0}")]
    Artifact(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64
}
pub fn retrieval_synthesis_contract_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"conformance".into(),consumers:["institutional safety reviewer".into(),"retrieval schema steward".into(),"evidence synthesis engineer".into()].into(),behavior:"negotiate bounded prospective retrieval and synthesis schemas with typed compatibility and evidence-state witnesses".into(),value:"prevents schema drift, omitted evidence, and unauthorized data movement from becoming an apparently valid synthesis".into(),inputs:vec![TypedPort{name:"scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"evidence_synthesis_contract".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(),permissions:["read:local-research-artifacts".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"ro-crate-1.1".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification/1.1/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
impl EvidenceSynthesis2 {
    pub fn validate(&self) -> Result<(), RetrievalContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "compatible" | "partial" | "unknown" | "blocked"
            )
        {
            return Err(RetrievalContractError::Invalid(
                "retrieval contract identity, candidates, locality, or effects are incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.candidate_order,
            &self.compatible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.omitted_order,
            &self.negative_evidence_order,
            &self.migration_order,
            &self.semantic_loss_order,
            &self.effect_receipts,
        ] {
            if !ordered(v) {
                return Err(RetrievalContractError::Invalid(
                    "retrieval contract ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .compatible_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(RetrievalContractError::Invalid(
                "retrieval candidate states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.contract_digest)
            || self.artifact.content_hash != self.contract_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(RetrievalContractError::Artifact(
                "retrieval contract digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            e != "block:unsafe-release" && !e.starts_with("read:local-research-artifacts:")
        }) {
            return Err(RetrievalContractError::Invalid(
                "retrieval contract effect is outside local-read gate".into(),
            ));
        }
        if self.disposition == "blocked" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(RetrievalContractError::Invalid(
                "blocked retrieval contract must block".into(),
            ));
        }
        Ok(())
    }
}
pub fn negotiate_retrieval_synthesis_contract(
    request: &ScopedRetrievalQuery3,
) -> Result<EvidenceSynthesis2, RetrievalContractError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.candidates.is_empty()
        || request.minimum_relevance_milli == 0
        || request.minimum_freshness_milli == 0
        || !digest(&request.replay_identity)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalContractError::Invalid("retrieval contract identity, schema, bounds, replay, locality, or boundary are invalid".into()));
    }
    let compatibility =
        if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA {
            "compatible"
        } else if request.input_schema.starts_with("ScopedRetrievalQuery")
            && request.output_schema.starts_with("EvidenceSynthesis")
        {
            "additive_migration"
        } else {
            "breaking"
        };
    let mut rows = request.candidates.clone();
    rows.sort_by(|a, b| {
        b.relevance_milli
            .cmp(&a.relevance_milli)
            .then(a.candidate_id.cmp(&b.candidate_id))
    });
    let candidate_order = rows
        .iter()
        .map(|r| r.candidate_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if candidate_order.len() != rows.len() {
        return Err(RetrievalContractError::Invalid(
            "duplicate retrieval candidates are invalid".into(),
        ));
    }
    let mut compatible_order = BTreeSet::new();
    let mut unresolved_order = BTreeSet::new();
    let mut blocked_order = BTreeSet::new();
    let mut omitted_order = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut loss: BTreeSet<String> = BTreeSet::new();
    for r in &rows {
        for o in &r.omission_reasons {
            omitted_order.insert(format!("{}:{}", r.candidate_id, o));
        }
        if r.negative_result {
            negative.insert(format!("{}:negative-result", r.candidate_id));
        }
        let hard = !request.policy_allow
            || !request.protected_closure
            || !r.permitted
            || !r.local_only
            || r.semantic_profile != request.semantic_profile
            || !r.comparable
            || !digest(&r.content_digest)
            || !digest(&r.provenance_digest);
        let soft = r.replay_identity != request.replay_identity
            || r.relevance_milli < request.minimum_relevance_milli
            || r.freshness_milli < request.minimum_freshness_milli;
        if compatibility != "compatible" {
            migration.insert(format!("{}:schema-migration", r.candidate_id));
        }
        if hard || r.evidence_state == RetrievalContractEvidenceState::Contradicted {
            blocked_order.insert(r.candidate_id.clone());
        } else if soft
            || !matches!(
                r.evidence_state,
                RetrievalContractEvidenceState::Proven | RetrievalContractEvidenceState::Supported
            )
        {
            unresolved_order.insert(r.candidate_id.clone());
        } else {
            compatible_order.insert(r.candidate_id.clone());
        }
    }
    if !request.policy_allow {
        loss.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        loss.insert("workflow:protected-closure-incomplete".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !blocked_order.is_empty()
    {
        "blocked"
    } else if compatibility != "compatible" || !unresolved_order.is_empty() || !negative.is_empty()
    {
        "partial"
    } else {
        "compatible"
    };
    if disposition != "compatible" {
        loss.insert("workflow:contract-not-fully-compatible".into());
    }
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"consumer":request.consumer,"scope":request.scope,"semantic_profile":request.semantic_profile,"input_schema":request.input_schema,"output_schema":request.output_schema,"compatibility":compatibility,"disposition":disposition,"candidate_order":candidate_order,"compatible_order":compatible_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"omitted_order":omitted_order,"negative_evidence_order":negative,"migration_order":migration,"semantic_loss_order":loss,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let contract_digest = ContentHash::of_value(&payload)
        .map_err(|e| RetrievalContractError::Artifact(e.to_string()))?;
    let out = EvidenceSynthesis2 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        compatibility: compatibility.into(),
        disposition: disposition.into(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        compatible_order: payload["compatible_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omitted_order: payload["omitted_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        migration_order: payload["migration_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        semantic_loss_order: payload["semantic_loss_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        contract_digest: contract_digest.clone(),
        artifact: EvidenceSynthesis2Artifact {
            artifact_id: format!("conformance-evidence-synthesis-2:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: contract_digest,
            semantic_loss: payload["semantic_loss_order"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().into())
                .collect(),
            provenance_digests: rows.iter().map(|r| r.provenance_digest.clone()).collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "compatible" {
            vec![format!(
                "read:local-research-artifacts:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> ScopedRetrievalQuery3 {
        ScopedRetrievalQuery3 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "r".into(),
            consumer: "reviewer".into(),
            scope: "study".into(),
            semantic_profile: "profile:v1".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            minimum_relevance_milli: 500,
            minimum_freshness_milli: 500,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            candidates: vec![RetrievalCandidate3 {
                candidate_id: "c1".into(),
                source_id: "s1".into(),
                study_id: "study".into(),
                modality: "imaging".into(),
                semantic_profile: "profile:v1".into(),
                relevance_milli: 900,
                freshness_milli: 900,
                evidence_state: RetrievalContractEvidenceState::Supported,
                content_digest: h("content"),
                provenance_digest: h("prov"),
                replay_identity: h("replay"),
                comparable: true,
                permitted: true,
                local_only: true,
                negative_result: false,
                omission_reasons: vec![],
            }],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            retrieval_synthesis_contract_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn compatible() {
        assert_eq!(
            negotiate_retrieval_synthesis_contract(&req())
                .unwrap()
                .disposition,
            "compatible"
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allow = false;
        assert_eq!(
            negotiate_retrieval_synthesis_contract(&r)
                .unwrap()
                .disposition,
            "blocked"
        )
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            negotiate_retrieval_synthesis_contract(&req()).unwrap(),
            negotiate_retrieval_synthesis_contract(&req()).unwrap()
        )
    }
}
