//! Federated continual context-compilation research copilot.
//!
//! This contract turns typed, caller-supplied decision-context attestations into a
//! `CertifiedDecisionSection3` compatibility artifact.  It does not retrieve evidence,
//! invoke tools, or make a scientific or clinical decision; its product is the deterministic,
//! omission-aware admission boundary that an agent and a downstream workflow can replay.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-services-P03-F12";
pub const CONTRACT_VERSION: &str = "services-federated-continual-context-compilation-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "DecisionQuery4@1";
pub const OUTPUT_SCHEMA: &str = "CertifiedDecisionSection3@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.certified-decision-section-3+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery4 {
    pub query_id: String,
    pub study_id: String,
    pub intent: String,
    pub context_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub influence_complete: bool,
    pub policy_allow: bool,
    pub permitted: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationRequest {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub semantic_profile: String,
    pub required_query_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub queries: Vec<DecisionQuery4>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub agent_authorized: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSection3 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub semantic_profile: String,
    pub disposition: CompilationDisposition,
    pub required_query_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub query_order: Vec<String>,
    pub selected_query_order: Vec<String>,
    pub unresolved_query_order: Vec<String>,
    pub blocked_query_order: Vec<String>,
    pub missing_query_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub decision_section_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub compilation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextCompilationError {
    #[error("invalid context-compilation request: {0}")]
    Invalid(String),
    #[error("context-compilation artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ContextCompilationError {
    ContextCompilationError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

impl CertifiedDecisionSection3 {
    pub fn validate(&self) -> Result<(), ContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.required_query_order.is_empty()
            || self.required_study_order.is_empty()
            || self.query_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("decision-section identity, required axes, locality, or effects are incomplete"));
        }
        for values in [
            &self.required_query_order,
            &self.required_study_order,
            &self.query_order,
            &self.selected_query_order,
            &self.unresolved_query_order,
            &self.blocked_query_order,
            &self.missing_query_order,
            &self.missing_study_order,
            &self.decision_section_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("decision-section ordering is not canonical"));
            }
        }
        let all = self.query_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .selected_query_order
            .iter()
            .chain(self.unresolved_query_order.iter())
            .chain(self.blocked_query_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if all.len() != self.query_order.len()
            || parts.len() + self.missing_query_order.len() != all.len()
            || !parts.is_disjoint(&self.missing_query_order.iter().cloned().collect())
            || parts.union(&self.missing_query_order.iter().cloned().collect()).cloned().collect::<BTreeSet<_>>() != all
        {
            return Err(invalid("query states do not form a complete partition"));
        }
        let required_queries = self.required_query_order.iter().cloned().collect::<BTreeSet<_>>();
        let missing_queries = self.missing_query_order.iter().cloned().collect::<BTreeSet<_>>();
        if !missing_queries.is_subset(&required_queries) {
            return Err(invalid("missing query state is outside required queries"));
        }
        let required_studies = self.required_study_order.iter().cloned().collect::<BTreeSet<_>>();
        if !self.missing_study_order.iter().all(|id| required_studies.contains(id)) {
            return Err(invalid("missing study state is outside required studies"));
        }
        if !valid_digest(&self.compilation_digest) || self.artifact.content_hash != self.compilation_digest {
            return Err(invalid("compilation digest is invalid or inconsistent"));
        }
        self.artifact.validate_metadata().map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("decision-section artifact content type is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:declared-tool:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside the bounded tool gate"));
        }
        if self.disposition == CompilationDisposition::Qualified
            && self.effect_receipts != [format!("invoke:declared-tool:{}", self.request_id)]
        {
            return Err(invalid("qualified decision section effect is invalid"));
        }
        if self.disposition != CompilationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified decision section must block release"));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ContextCompilationError> {
        self.validate()?;
        ContentHash::of_value(&serde_json::to_value(self).map_err(|error| ContextCompilationError::Artifact(error.to_string()))?)
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))
    }
}

pub fn context_compilation_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "services".into(),
        consumers: ["research workflow operator".into(), "agent developer".into(), "downstream context compiler".into()].into(),
        behavior: "compiles typed DecisionQuery4 attestations into omission-aware CertifiedDecisionSection3 artifacts at federated continual scale without retrieving sources or granting authority".into(),
        value: "automates bounded decision-context preparation while preserving replay, provenance, uncertainty, omissions, negative results, and protected closure".into(),
        inputs: vec![TypedPort { name: "decision_query".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "certified_decision_section".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["invoke:declared-tools".into(), "read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) },
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "research workflow operator".into(), reason: "declared tool invocations require an explicit bounded authority at the caller boundary".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_context_compilation(
    request: &ContextCompilationRequest,
) -> Result<CertifiedDecisionSection3, ContextCompilationError> {
    validate_request(request)?;
    let mut queries = request.queries.clone();
    queries.sort_by(|left, right| left.query_id.cmp(&right.query_id));
    let query_order = queries.iter().map(|query| query.query_id.clone()).collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut selected_studies = BTreeSet::new();
    for query in &queries {
        if !query.local_only || !query.aggregate_only || !query.permitted || !query.policy_allow {
            blocked.insert(query.query_id.clone());
            omissions.insert(format!("{}:permission-or-locality", query.query_id));
        } else if query.replay_identity != request.replay_identity
            || !query.influence_complete
            || !matches!(query.evidence_state, EvidenceState::Proven | EvidenceState::Supported)
        {
            unresolved.insert(query.query_id.clone());
            if query.replay_identity != request.replay_identity { uncertainty.insert(format!("{}:replay-mismatch", query.query_id)); }
            if !query.influence_complete { uncertainty.insert(format!("{}:influence-unmeasured", query.query_id)); }
            if !matches!(query.evidence_state, EvidenceState::Proven | EvidenceState::Supported) { uncertainty.insert(format!("{}:evidence-state", query.query_id)); }
        } else {
            selected.insert(query.query_id.clone());
            selected_studies.insert(query.study_id.clone());
        }
        omissions.extend(query.omission_order.iter().map(|item| format!("{}:{item}", query.query_id)));
        uncertainty.extend(query.uncertainty_order.iter().map(|item| format!("{}:{item}", query.query_id)));
        if query.negative_result { negative.insert(format!("{}:negative-result", query.query_id)); }
    }
    let required_queries = request.required_query_order.iter().cloned().collect::<BTreeSet<_>>();
    let missing_queries = required_queries.difference(&query_order.iter().cloned().collect()).cloned().collect::<BTreeSet<_>>();
    let required_studies = request.required_study_order.iter().cloned().collect::<BTreeSet<_>>();
    let missing_studies = required_studies.difference(&selected_studies).cloned().collect::<BTreeSet<_>>();
    omissions.extend(missing_queries.iter().map(|id| format!("query:{id}:missing")));
    omissions.extend(missing_studies.iter().map(|id| format!("study:{id}:missing")));
    uncertainty.extend(request.adversarial_events.iter().map(|event| format!("adversarial:{event}")));
    let global_block = !request.policy_allow || !request.protected_closure || !request.signed_approval || !request.agent_authorized || !request.raw_data_local || !request.aggregate_only || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(query_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:release-gate-blocked".into());
    }
    let disposition = if global_block { CompilationDisposition::Blocked } else if selected.is_empty() || !missing_queries.is_empty() || !missing_studies.is_empty() { CompilationDisposition::Unresolved } else { CompilationDisposition::Qualified };
    if disposition != CompilationDisposition::Qualified { omissions.insert("request:decision-section-not-release-ready".into()); }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_query_order = missing_queries.into_iter().collect::<Vec<_>>();
    let missing_study_order = missing_studies.into_iter().collect::<Vec<_>>();
    let decision_section_order = if disposition == CompilationDisposition::Qualified { selected_order.clone() } else { Vec::new() };
    let effect_receipts = if disposition == CompilationDisposition::Qualified { vec![format!("invoke:declared-tool:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID,
        "request_id": request.request_id, "researcher": request.researcher, "semantic_profile": request.semantic_profile,
        "disposition": disposition, "required_query_order": request.required_query_order, "required_study_order": request.required_study_order,
        "query_order": query_order, "selected_query_order": selected_order, "unresolved_query_order": unresolved_order, "blocked_query_order": blocked_order,
        "missing_query_order": missing_query_order, "missing_study_order": missing_study_order, "decision_section_order": decision_section_order,
        "omission_order": omissions, "uncertainty_order": uncertainty, "negative_evidence_order": negative, "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "boundary": PRECLINICAL_BOUNDARY,
    });
    let compilation_digest = ContentHash::of_value(&payload).map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(format!("certified-decision-section:{}", request.request_id), CONTENT_TYPE, &payload, vec![SemanticLoss { field: "omission_order".into(), reason: "omissions remain explicit rather than being silently filled".into(), severity: bioprism_foundation::LossSeverity::Bounded }], vec![ProvenanceLink { source_id: request.request_id.clone(), relation: "compiled-from-decision-query".into(), digest: compilation_digest.clone() }]).map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let receipt = CertifiedDecisionSection3 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(), researcher: request.researcher.clone(), semantic_profile: request.semantic_profile.clone(), disposition,
        required_query_order: request.required_query_order.clone(), required_study_order: request.required_study_order.clone(), query_order,
        selected_query_order: payload["selected_query_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        unresolved_query_order: payload["unresolved_query_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        blocked_query_order: payload["blocked_query_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        missing_query_order: payload["missing_query_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        missing_study_order: payload["missing_study_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        decision_section_order: payload["decision_section_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        omission_order: payload["omission_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        uncertainty_order: payload["uncertainty_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        negative_evidence_order: payload["negative_evidence_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        compilation_digest, artifact,
        effect_receipts: payload["effect_receipts"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(),
        raw_data_local: request.raw_data_local, aggregate_only: request.aggregate_only, boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ContextCompilationRequest) -> Result<(), ContextCompilationError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || request.request_id.trim().is_empty() || request.researcher.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_query_order.is_empty() || request.required_study_order.is_empty() || !canonical(&request.required_query_order) || !canonical(&request.required_study_order) || request.queries.is_empty() || !canonical(&request.adversarial_events) || !valid_digest(&request.replay_identity) || !request.raw_data_local || !request.aggregate_only || request.boundary != PRECLINICAL_BOUNDARY { return Err(invalid("decision-query identity, axes, replay, locality, or boundary is invalid")); }
    let mut ids = BTreeSet::new();
    for query in &request.queries {
        if query.query_id.trim().is_empty() || query.study_id.trim().is_empty() || query.intent.trim().is_empty() || !ids.insert(query.query_id.clone()) || !valid_digest(&query.context_digest) || !valid_digest(&query.evidence_digest) || !valid_digest(&query.provenance_digest) || !valid_digest(&query.replay_identity) || !canonical(&query.omission_order) || !canonical(&query.uncertainty_order) { return Err(invalid(format!("decision query {} is malformed or duplicated", query.query_id))); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> ContextCompilationRequest {
        let query = |id: &str, study: &str| DecisionQuery4 { query_id: id.into(), study_id: study.into(), intent: format!("intent:{id}"), context_digest: hash(id), evidence_digest: hash("evidence"), provenance_digest: hash("provenance"), replay_identity: hash("replay"), evidence_state: EvidenceState::Supported, influence_complete: true, policy_allow: true, permitted: true, local_only: true, aggregate_only: true, omission_order: Vec::new(), uncertainty_order: Vec::new(), negative_result: false };
        ContextCompilationRequest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), request_id: "context-request".into(), researcher: "workflow-operator".into(), semantic_profile: "decision-context:v1".into(), required_query_order: vec!["query:a".into(), "query:b".into()], required_study_order: vec!["study:a".into(), "study:b".into()], queries: vec![query("query:a", "study:a"), query("query:b", "study:b")], replay_identity: hash("replay"), policy_allow: true, protected_closure: true, signed_approval: true, agent_authorized: true, raw_data_local: true, aggregate_only: true, adversarial_events: Vec::new(), boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn manifest_is_a2() { assert_eq!(context_compilation_research_copilot_manifest().autonomy_tier, AutonomyTier::A2); }
    #[test] fn qualified_context_is_deterministic() { let receipt = compile_context_compilation(&request()).unwrap(); assert_eq!(receipt.disposition, CompilationDisposition::Qualified); assert_eq!(receipt.decision_section_order.len(), 2); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
    #[test] fn missing_query_is_unresolved() { let mut req = request(); req.queries.pop(); let receipt = compile_context_compilation(&req).unwrap(); assert_eq!(receipt.disposition, CompilationDisposition::Unresolved); assert!(receipt.missing_query_order.contains(&"query:b".into())); }
    #[test] fn denied_query_is_blocked() { let mut req = request(); req.queries[0].permitted = false; let receipt = compile_context_compilation(&req).unwrap(); assert!(receipt.blocked_query_order.contains(&"query:a".into())); assert_eq!(receipt.disposition, CompilationDisposition::Unresolved); }
    #[test] fn adversarial_request_blocks_all() { let mut req = request(); req.adversarial_events = vec!["prompt-injection".into()]; let receipt = compile_context_compilation(&req).unwrap(); assert_eq!(receipt.disposition, CompilationDisposition::Blocked); assert!(receipt.selected_query_order.is_empty()); }
    #[test] fn negative_result_is_preserved() { let mut req = request(); req.queries[0].negative_result = true; let receipt = compile_context_compilation(&req).unwrap(); assert!(receipt.negative_evidence_order.contains(&"query:a:negative-result".into())); }
    #[test] fn replay_mismatch_is_unresolved() { let mut req = request(); req.queries[0].replay_identity = hash("other"); let receipt = compile_context_compilation(&req).unwrap(); assert!(receipt.unresolved_query_order.contains(&"query:a".into())); assert!(receipt.uncertainty_order.contains(&"query:a:replay-mismatch".into())); }
}
