//! Federated continual context-compilation research copilot for `AFA-oraclex-P03-F12`.
//!
//! The copilot compiles caller-supplied, institution-local fact attestations and aggregate peer
//! summaries into a deterministic certified decision section.  It plans bounded declared-tool
//! calls but never invokes a connector, fetches evidence, exports raw data, or makes a biological
//! or clinical decision.  Unknown, speculative, contradictory, omitted, and unsafe states are
//! retained in the receipt instead of being promoted to confidence.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oraclex-P03-F12";
pub const CONTRACT_VERSION: &str = "oraclex-federated-context-compilation-copilot/1.0";
pub const INPUT_SCHEMA: &str = "DecisionQuery4@1";
pub const OUTPUT_SCHEMA: &str = "CertifiedDecisionSection3@1";
pub const TOOL_NAME: &str = "oraclex_context_compilation_research_copilot";
const CONTENT_TYPE: &str = "application/vnd.aurora.certified-decision-section-3+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionFact {
    pub fact_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub source_id: String,
    pub modality: String,
    pub digest: ContentHash,
    pub state: EvidenceState,
    pub influence_basis_points: u32,
    pub local_only: bool,
    pub permitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerContextSummary {
    pub peer_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub context_digest: ContentHash,
    pub fact_order: Vec<String>,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub state: EvidenceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub target_schema: String,
    pub required_fact_order: Vec<String>,
    pub facts: Vec<DecisionFact>,
    pub peers: Vec<PeerContextSummary>,
    pub max_facts: u32,
    pub max_tools: u32,
    pub tool_budget: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approval: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSection {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub target_schema: String,
    pub disposition: ContextDisposition,
    pub fact_order: Vec<String>,
    pub selected_fact_order: Vec<String>,
    pub unresolved_fact_order: Vec<String>,
    pub blocked_fact_order: Vec<String>,
    pub missing_fact_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub tool_plan_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextCompilationError {
    #[error("invalid decision query: {0}")]
    Invalid(String),
    #[error("certified decision section artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ContextCompilationError {
    ContextCompilationError::Invalid(message.into())
}

fn digest_is_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl CertifiedDecisionSection {
    pub fn validate(&self) -> Result<(), ContextCompilationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.target_schema.trim().is_empty()
            || self.fact_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "decision section identity, locality, facts, peers, or effects are incomplete",
            ));
        }
        for values in [
            &self.fact_order,
            &self.selected_fact_order,
            &self.unresolved_fact_order,
            &self.blocked_fact_order,
            &self.missing_fact_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.tool_plan_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("decision section ordering is not canonical"));
            }
        }
        let fact_ids = self.fact_order.iter().cloned().collect::<BTreeSet<_>>();
        let fact_parts = self
            .selected_fact_order
            .iter()
            .chain(self.unresolved_fact_order.iter())
            .chain(self.blocked_fact_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if fact_parts.len() != fact_ids.len()
            || fact_parts.iter().cloned().collect::<BTreeSet<_>>() != fact_ids
        {
            return Err(invalid(
                "decision facts do not partition the supplied context",
            ));
        }
        let peer_ids = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(self.missing_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_parts.len() != peer_ids.len()
            || peer_parts.iter().cloned().collect::<BTreeSet<_>>() != peer_ids
        {
            return Err(invalid(
                "decision peers do not partition the supplied federation",
            ));
        }
        for value in [
            &self.replay_identity,
            &self.section_digest,
            &self.evidence_digest,
            &self.artifact.content_hash,
        ] {
            if !digest_is_valid(value) {
                return Err(invalid("decision section digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("decision section artifact type is invalid"));
        }
        let expected = if self.disposition == ContextDisposition::Qualified {
            vec![format!("invoke:declared-tools:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected {
            return Err(invalid("decision section effect receipt is invalid"));
        }
        Ok(())
    }
}

pub fn context_compilation_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "oraclex".into(),
        consumers: BTreeSet::from([
            String::from("benchmark curator"),
            String::from("research workflow operator"),
            String::from("agent SDK integrator"),
        ]),
        behavior: "compiles typed local facts and aggregate peer summaries into a certified decision section and bounded declared-tool plan without reading raw evidence".into(),
        value: "makes federated continual context omissions, uncertainty, and tool authority observable before research automation proceeds".into(),
        inputs: vec![TypedPort {
            name: "decision_query".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "certified_decision_section".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: BTreeSet::from([Effect::ExecuteLocalComputation]),
        permissions: BTreeSet::from([String::from("invoke:declared-tools")]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "mcp-2025-06-18".into(),
                state: EvidenceState::Supported,
                locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()),
            },
            EvidenceReference {
                source_id: "wasm-component-model".into(),
                state: EvidenceState::Speculative,
                locator: None,
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "research workflow approver".into(),
            reason: "bounded declared-tool invocation may only be planned after institutional approval".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([
            ResearchSurface::Ui,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_context(
    request: &DecisionQuery,
) -> Result<CertifiedDecisionSection, ContextCompilationError> {
    validate_request(request)?;
    let mut facts = request.facts.clone();
    facts.sort_by(|a, b| a.fact_id.cmp(&b.fact_id));
    let fact_order = facts
        .iter()
        .map(|fact| fact.fact_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let required = request
        .required_fact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let present = facts
        .iter()
        .map(|fact| fact.fact_id.clone())
        .collect::<BTreeSet<_>>();
    missing.extend(required.difference(&present).cloned());
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for fact in &facts {
        if fact.negative_result {
            negative.insert(format!("{}:negative-result", fact.fact_id));
        }
        if !required.contains(&fact.fact_id) {
            omissions.insert(format!("{}:not-required", fact.fact_id));
        }
        match fact.state {
            EvidenceState::Contradicted => {
                blocked.insert(fact.fact_id.clone());
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                unresolved.insert(fact.fact_id.clone());
            }
            EvidenceState::Proven | EvidenceState::Supported => {
                if !fact.local_only || !fact.permitted {
                    blocked.insert(fact.fact_id.clone());
                } else if required.contains(&fact.fact_id) {
                    selected.insert(fact.fact_id.clone());
                } else {
                    unresolved.insert(fact.fact_id.clone());
                }
            }
        }
        if fact.semantic_profile != request.semantic_profile {
            uncertainty.insert(format!("{}:semantic-profile-mismatch", fact.fact_id));
            unresolved.insert(fact.fact_id.clone());
            selected.remove(&fact.fact_id);
        }
    }
    let mut peer_order = BTreeSet::new();
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &request.peers {
        peer_order.insert(peer.peer_id.clone());
        let valid = peer.purpose == request.purpose
            && peer.semantic_profile == request.semantic_profile
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && digest_is_valid(&peer.context_digest)
            && canonical(&peer.fact_order)
            && matches!(peer.state, EvidenceState::Proven | EvidenceState::Supported);
        if valid {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    let mut tool_plan = BTreeSet::new();
    for fact in &facts {
        if selected.contains(&fact.fact_id) && tool_plan.len() < request.max_tools as usize {
            tool_plan.insert(format!("tool:{}", fact.fact_id));
        }
    }
    if selected.len() > request.max_tools as usize || tool_plan.len() as u32 > request.tool_budget {
        omissions.insert("request:tool-budget-exhausted".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approval {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !request.policy_allow
        || !request.raw_data_local
        || !request.signed_approval
        || !request.federation_approval
        || !request.protected_closure
        || !request.adversarial_events.is_empty()
        || selected.len() > request.max_tools as usize
        || tool_plan.len() as u32 > request.tool_budget;
    if global_block {
        blocked.extend(fact_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:context-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        ContextDisposition::Blocked
    } else if !missing.is_empty() || !unresolved.is_empty() || !missing_peers.is_empty() {
        ContextDisposition::Unresolved
    } else {
        ContextDisposition::Qualified
    };
    let selected_fact_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_fact_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_fact_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_fact_order = missing.into_iter().collect::<Vec<_>>();
    let peer_order = peer_order.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peers.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peers.into_iter().collect::<Vec<_>>();
    let tool_plan_order = tool_plan.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == ContextDisposition::Qualified {
        vec![format!("invoke:declared-tools:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let evidence_payload = json!({
        "fact_order": fact_order,
        "selected_fact_order": selected_fact_order,
        "unresolved_fact_order": unresolved_fact_order,
        "blocked_fact_order": blocked_fact_order,
        "missing_fact_order": missing_fact_order,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
    });
    let evidence_digest = ContentHash::of_value(&evidence_payload)
        .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "consumer": request.consumer,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "target_schema": request.target_schema,
        "disposition": disposition,
        "evidence": evidence_payload,
        "tool_plan_order": tool_plan_order,
        "replay_identity": request.replay_identity,
        "evidence_digest": evidence_digest,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let section_digest = ContentHash::of_value(&payload)
        .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("certified-decision-section:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextCompilationError::Artifact(error.to_string()))?;
    let array_strings = |value: &Value| {
        value
            .as_array()
            .expect("canonical context arrays are arrays")
            .iter()
            .map(|item| {
                item.as_str()
                    .expect("canonical context values are strings")
                    .to_string()
            })
            .collect::<Vec<_>>()
    };
    let evidence = payload
        .get("evidence")
        .expect("evidence payload is present");
    let section = CertifiedDecisionSection {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        target_schema: request.target_schema.clone(),
        disposition,
        fact_order: array_strings(evidence.get("fact_order").unwrap()),
        selected_fact_order: array_strings(evidence.get("selected_fact_order").unwrap()),
        unresolved_fact_order: array_strings(evidence.get("unresolved_fact_order").unwrap()),
        blocked_fact_order: array_strings(evidence.get("blocked_fact_order").unwrap()),
        missing_fact_order: array_strings(evidence.get("missing_fact_order").unwrap()),
        peer_order: array_strings(evidence.get("peer_order").unwrap()),
        qualified_peer_order: array_strings(evidence.get("qualified_peer_order").unwrap()),
        missing_peer_order: array_strings(evidence.get("missing_peer_order").unwrap()),
        tool_plan_order: array_strings(payload.get("tool_plan_order").unwrap()),
        omission_order: array_strings(evidence.get("omission_order").unwrap()),
        uncertainty_order: array_strings(evidence.get("uncertainty_order").unwrap()),
        negative_evidence_order: array_strings(evidence.get("negative_evidence_order").unwrap()),
        replay_identity: request.replay_identity.clone(),
        section_digest,
        evidence_digest,
        effect_receipts: array_strings(payload.get("effect_receipts").unwrap()),
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    section.validate()?;
    Ok(section)
}

pub fn compile_context_json(value: &Value) -> Result<Value, String> {
    let request: DecisionQuery = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid decision query: {error}"))?;
    let section = compile_context(&request).map_err(|error| error.to_string())?;
    serde_json::to_value(section)
        .map_err(|error| format!("cannot serialize certified decision section: {error}"))
}

pub fn validate_context_json(value: &Value) -> Result<CertifiedDecisionSection, String> {
    let section: CertifiedDecisionSection = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid certified decision section: {error}"))?;
    section.validate().map_err(|error| error.to_string())?;
    Ok(section)
}

fn validate_request(request: &DecisionQuery) -> Result<(), ContextCompilationError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.target_schema.trim().is_empty()
        || request.required_fact_order.is_empty()
        || !canonical(&request.required_fact_order)
        || request.facts.is_empty()
        || request.peers.is_empty()
        || request.max_facts == 0
        || request.max_tools == 0
        || request.tool_budget == 0
        || request.facts.len() as u32 > request.max_facts
        || !digest_is_valid(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "decision query identity, closure, limits, replay, locality, or boundary is invalid",
        ));
    }
    let mut fact_ids = BTreeSet::new();
    for fact in &request.facts {
        if fact.fact_id.trim().is_empty()
            || !fact_ids.insert(fact.fact_id.clone())
            || fact.scope.trim().is_empty()
            || fact.semantic_profile.trim().is_empty()
            || fact.source_id.trim().is_empty()
            || fact.modality.trim().is_empty()
            || fact.influence_basis_points > 10_000
            || !digest_is_valid(&fact.digest)
        {
            return Err(invalid(format!(
                "fact {} is malformed or duplicated",
                fact.fact_id
            )));
        }
    }
    let mut peer_ids = BTreeSet::new();
    for peer in &request.peers {
        if peer.peer_id.trim().is_empty()
            || !peer_ids.insert(peer.peer_id.clone())
            || peer.purpose.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
            || peer.fact_order.is_empty()
            || !canonical(&peer.fact_order)
            || !digest_is_valid(&peer.context_digest)
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                peer.peer_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request() -> DecisionQuery {
        let digest = hash("context");
        let fact = |id: &str| DecisionFact {
            fact_id: id.into(),
            scope: "study:local".into(),
            semantic_profile: "preclinical:v1".into(),
            source_id: format!("source:{id}"),
            modality: "omics".into(),
            digest: digest.clone(),
            state: EvidenceState::Supported,
            influence_basis_points: 8_000,
            local_only: true,
            permitted: true,
            negative_result: false,
        };
        let peer = PeerContextSummary {
            peer_id: "peer:a".into(),
            purpose: "mechanism-screen".into(),
            semantic_profile: "preclinical:v1".into(),
            context_digest: digest.clone(),
            fact_order: vec!["fact:a".into(), "fact:b".into()],
            signed: true,
            aggregate_only: true,
            raw_data_local: true,
            state: EvidenceState::Supported,
        };
        DecisionQuery {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "query:one".into(),
            consumer: "benchmark curator".into(),
            federation_id: "federation:test".into(),
            purpose: "mechanism-screen".into(),
            semantic_profile: "preclinical:v1".into(),
            target_schema: "CertifiedDecisionSection3@1".into(),
            required_fact_order: vec!["fact:a".into(), "fact:b".into()],
            facts: vec![fact("fact:b"), fact("fact:a")],
            peers: vec![peer],
            max_facts: 4,
            max_tools: 4,
            tool_budget: 4,
            replay_identity: digest,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approval: true,
            raw_data_local: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            context_compilation_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }

    #[test]
    fn qualified_is_deterministic() {
        let first = compile_context(&request()).unwrap();
        let second = compile_context(&request()).unwrap();
        assert_eq!(first.disposition, ContextDisposition::Qualified);
        assert_eq!(first.section_digest, second.section_digest);
        assert_eq!(
            first.effect_receipts,
            vec!["invoke:declared-tools:query:one"]
        );
    }

    #[test]
    fn missing_fact_is_unresolved() {
        let mut value = request();
        value.facts.pop();
        assert_eq!(
            compile_context(&value).unwrap().disposition,
            ContextDisposition::Unresolved
        );
    }

    #[test]
    fn unknown_fact_is_unresolved() {
        let mut value = request();
        value.facts[0].state = EvidenceState::Unknown;
        assert_eq!(
            compile_context(&value).unwrap().disposition,
            ContextDisposition::Unresolved
        );
    }

    #[test]
    fn contradictory_fact_blocks() {
        let mut value = request();
        value.facts[0].state = EvidenceState::Contradicted;
        assert_eq!(
            compile_context(&value).unwrap().disposition,
            ContextDisposition::Blocked
        );
    }

    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            compile_context(&value).unwrap().disposition,
            ContextDisposition::Blocked
        );
    }

    #[test]
    fn peer_mismatch_is_unresolved() {
        let mut value = request();
        value.peers[0].semantic_profile = "other:v1".into();
        assert_eq!(
            compile_context(&value).unwrap().disposition,
            ContextDisposition::Unresolved
        );
    }
}
