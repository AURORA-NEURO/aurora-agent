//! Prospective high-throughput context-compilation assurance.
//!
//! Atlas feature: `AFA-hubapi-P03-F27`.
//!
//! The harness turns a caller-supplied set of typed research facts into an omission-aware,
//! content-addressed context report.  It does not infer facts, fetch evidence, run a model, or
//! make a clinical decision.  Ranking is deterministic and every missing, uncertain,
//! contradictory, negative, policy, locality, or budget condition remains observable.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ProvenanceLink, ResearchSurface, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-hubapi-P03-F27";
pub const CONTRACT_VERSION: &str = "hubapi-prospective-context-compilation-assurance/1.0";
pub const INPUT_SCHEMA: &str = "DecisionQuery5@1";
pub const OUTPUT_SCHEMA: &str = "ContextAssuranceReport8@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFact {
    pub fact_id: String,
    pub proposition: String,
    pub scope: String,
    pub evidence_state: EvidenceState,
    pub influence_milli: u16,
    pub source_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub local_data: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery {
    pub request_id: String,
    pub workflow_id: String,
    pub target_schema: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_fact_order: Vec<String>,
    pub required_scope_order: Vec<String>,
    pub facts: Vec<ContextFact>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget: u64,
    pub max_budget: u64,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssuranceReport {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub target_schema: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub fact_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_fact_order: Vec<String>,
    pub missing_scope_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub checkpoint_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub context_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextAssuranceError {
    #[error("invalid context assurance request: {0}")]
    Invalid(String),
    #[error("context assurance artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ContextAssuranceError {
    ContextAssuranceError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

const CHECKPOINTS: [&str; 5] = [
    "admit-typed-query",
    "check-evidence-and-scope",
    "check-provenance-and-replay",
    "check-policy-and-federation",
    "retain-omission-and-negative-receipt",
];

pub fn context_compilation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "hubapi".into(),
        consumers: [
            "AURORA extension developer".into(),
            "prospective research program lead".into(),
            "context-release steward".into(),
        ]
        .into(),
        behavior: "assures prospective context compilation from typed local facts with deterministic influence ranking, scope closure, evidence witnesses, provenance, replay, federation, and policy gates".into(),
        value: "prevents omitted or unsupported research facts from being presented as a complete decision context while retaining useful partial evidence for review".into(),
        inputs: vec![TypedPort {
            name: "decision_query".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "context_assurance_report".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact]
            .into(),
        permissions: ["verify:hubapi-context-assurance".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "cwl".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.commonwl.org/specification/".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "context-release steward".into(),
            reason: "a qualified aggregate context must be approved before federation exchange".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl ContextAssuranceReport {
    pub fn validate(&self) -> Result<(), ContextAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.target_schema.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || !matches!(self.disposition.as_str(), "qualified" | "unresolved" | "blocked")
            || self.fact_order.is_empty()
            || self.ranked_order.len() != self.fact_order.len()
            || self.effect_receipts.is_empty()
            || self.checkpoint_order != CHECKPOINTS
        {
            return Err(invalid("context report identity, locality, ranking, checkpoints, disposition, or effects are incomplete"));
        }
        for values in [
            &self.fact_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_fact_order,
            &self.missing_scope_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("context report ordering is not canonical"));
            }
        }
        let facts = self.fact_order.iter().collect::<BTreeSet<_>>();
        let partitions = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .collect::<Vec<_>>();
        if partitions.iter().any(|id| !facts.contains(id))
            || partitions.len() != facts.len()
            || partitions.iter().collect::<BTreeSet<_>>().len() != partitions.len()
            || self.missing_fact_order.iter().any(|id| facts.contains(id))
            || self.ranked_order.iter().collect::<BTreeSet<_>>() != facts
        {
            return Err(invalid("context fact states do not partition observed facts"));
        }
        for value in [&self.replay_identity, &self.context_digest, &self.artifact.content_hash] {
            if !digest(value) {
                return Err(invalid("context report digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))?;
        if self.artifact.content_type
            != "application/vnd.aurora.hubapi-context-assurance-report+json"
        {
            return Err(invalid("context report artifact content type is invalid"));
        }
        if self.disposition == "qualified" {
            if self.effect_receipts.len() != 1
                || !self.effect_receipts[0].starts_with("verify:hubapi-context-assurance:")
            {
                return Err(invalid("qualified context report effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid("non-qualified context report must block release"));
        }
        Ok(())
    }
}

pub fn assure_context_compilation(
    query: &DecisionQuery,
) -> Result<ContextAssuranceReport, ContextAssuranceError> {
    validate_query(query)?;
    let mut facts = query.facts.clone();
    facts.sort_by(|left, right| {
        right
            .influence_milli
            .cmp(&left.influence_milli)
            .then(left.fact_id.cmp(&right.fact_id))
    });
    let ranked_order = facts.iter().map(|fact| fact.fact_id.clone()).collect::<Vec<_>>();
    let mut fact_order = ranked_order.clone();
    fact_order.sort();
    let fact_map = facts
        .iter()
        .map(|fact| (fact.fact_id.clone(), fact))
        .collect::<BTreeMap<_, _>>();
    let required = query.required_fact_order.iter().cloned().collect::<BTreeSet<_>>();
    let missing_fact_order = required
        .iter()
        .filter(|fact| !fact_map.contains_key(*fact))
        .cloned()
        .collect::<Vec<_>>();
    let scopes = query.required_scope_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradictions = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for fact in &facts {
        if fact.negative_result {
            negative.insert(format!("{}:negative-result", fact.fact_id));
        }
        omissions.extend(
            fact.omissions
                .iter()
                .map(|item| format!("{}:{item}", fact.fact_id)),
        );
        uncertainty.extend(
            fact.uncertainty
                .iter()
                .map(|item| format!("{}:{item}", fact.fact_id)),
        );
        if fact.evidence_state == EvidenceState::Contradicted {
            blocked.insert(fact.fact_id.clone());
            contradictions.insert(format!("{}:contradicted-evidence", fact.fact_id));
            continue;
        }
        if matches!(fact.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative) {
            unresolved.insert(fact.fact_id.clone());
            uncertainty.insert(format!("{}:evidence-unresolved", fact.fact_id));
            continue;
        }
        let complete = fact.proposition.trim() != ""
            && fact.scope.trim() != ""
            && fact.source_digest.is_some()
            && fact.provenance_digest.is_some()
            && fact.semantic_profile == query.semantic_profile
            && fact.replay_identity == query.replay_identity
            && scopes.contains(&fact.scope)
            && fact.omissions.is_empty()
            && fact.uncertainty.is_empty()
            && fact.local_data
            && fact.permitted
            && fact.influence_milli >= 500;
        if complete && matches!(fact.evidence_state, EvidenceState::Proven | EvidenceState::Supported)
        {
            selected.insert(fact.fact_id.clone());
        } else {
            unresolved.insert(fact.fact_id.clone());
            if fact.source_digest.is_none() || fact.provenance_digest.is_none() {
                omissions.insert(format!("{}:source-or-provenance-missing", fact.fact_id));
            }
            if !scopes.contains(&fact.scope) {
                omissions.insert(format!("{}:required-scope-missing", fact.fact_id));
            }
            if fact.influence_milli < 500 {
                uncertainty.insert(format!("{}:influence-threshold-not-met", fact.fact_id));
            }
            if !fact.local_data || !fact.permitted {
                blocked.insert(fact.fact_id.clone());
                unresolved.remove(&fact.fact_id);
                omissions.insert(format!("{}:locality-or-permission-denied", fact.fact_id));
            }
        }
    }
    for fact in &missing_fact_order {
        omissions.insert(format!("{fact}:required-fact-missing"));
    }
    let missing_scope_order = query
        .required_scope_order
        .iter()
        .filter(|scope| !facts.iter().any(|fact| &fact.scope == *scope))
        .cloned()
        .collect::<Vec<_>>();
    for scope in &missing_scope_order {
        omissions.insert(format!("required-scope-missing:{scope}"));
    }
    negative.extend(
        query
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !query.policy_allow
        || !query.protected_closure
        || !query.signed_approval
        || !query.federation_approved
        || !query.raw_data_local
        || !query.aggregate_only
        || query.budget > query.max_budget
        || !query.adversarial_events.is_empty();
    if !query.policy_allow {
        uncertainty.insert("request:policy-denied".into());
    }
    if !query.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !query.signed_approval || !query.federation_approved {
        uncertainty.insert("request:institutional-approval-incomplete".into());
    }
    if query.budget > query.max_budget {
        omissions.insert("request:budget-ceiling-exceeded".into());
    }
    let disposition = if global_block {
        "blocked"
    } else if missing_fact_order.is_empty()
        && missing_scope_order.is_empty()
        && !selected.is_empty()
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        "qualified"
    } else {
        "unresolved"
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let contradiction_order = contradictions.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let adversarial_event_order = query.adversarial_events.clone();
    let checkpoint_order = CHECKPOINTS.map(str::to_string).to_vec();
    let effect_receipts = if disposition == "qualified" {
        vec![format!("verify:hubapi-context-assurance:{}", query.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": query.request_id,
        "workflow_id": query.workflow_id,
        "target_schema": query.target_schema,
        "purpose": query.purpose,
        "semantic_profile": query.semantic_profile,
        "disposition": disposition,
        "fact_order": fact_order,
        "ranked_order": ranked_order,
        "selected_order": selected_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_fact_order": missing_fact_order,
        "missing_scope_order": missing_scope_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "contradiction_order": contradiction_order,
        "negative_evidence_order": negative_evidence_order,
        "adversarial_event_order": adversarial_event_order,
        "checkpoint_order": checkpoint_order,
        "replay_identity": query.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": query.raw_data_local,
        "aggregate_only": query.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let context_digest = ContentHash::of_value(&payload)
        .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("hubapi-context-assurance:{}", query.request_id),
        "application/vnd.aurora.hubapi-context-assurance-report+json",
        &payload,
        Vec::new(),
        vec![ProvenanceLink {
            source_id: format!("workflow:{}", query.workflow_id),
            relation: "derived-from-local-context-manifest".into(),
            digest: query.replay_identity.clone(),
        }],
    )
    .map_err(|error| ContextAssuranceError::Artifact(error.to_string()))?;
    let report = ContextAssuranceReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: query.request_id.clone(),
        workflow_id: query.workflow_id.clone(),
        target_schema: query.target_schema.clone(),
        purpose: query.purpose.clone(),
        semantic_profile: query.semantic_profile.clone(),
        disposition: disposition.into(),
        fact_order,
        ranked_order,
        selected_order,
        unresolved_order,
        blocked_order,
        missing_fact_order,
        missing_scope_order,
        omission_order,
        uncertainty_order,
        contradiction_order,
        negative_evidence_order,
        adversarial_event_order,
        checkpoint_order,
        replay_identity: query.replay_identity.clone(),
        context_digest,
        artifact,
        effect_receipts,
        raw_data_local: query.raw_data_local,
        aggregate_only: query.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

fn validate_query(query: &DecisionQuery) -> Result<(), ContextAssuranceError> {
    if query.request_id.trim().is_empty()
        || query.workflow_id.trim().is_empty()
        || query.target_schema.trim().is_empty()
        || query.purpose.trim().is_empty()
        || query.semantic_profile.trim().is_empty()
        || query.facts.is_empty()
        || query.required_fact_order.is_empty()
        || query.required_scope_order.is_empty()
        || !canonical(&query.required_fact_order)
        || !canonical(&query.required_scope_order)
        || !canonical(&query.adversarial_events)
        || !digest(&query.replay_identity)
        || query.budget == 0
        || query.max_budget == 0
        || query.boundary != PRECLINICAL_BOUNDARY
        || !query.raw_data_local
        || !query.aggregate_only
    {
        return Err(invalid("query identity, required closure, digest, budget, locality, or boundary is invalid"));
    }
    let mut seen = BTreeSet::new();
    for fact in &query.facts {
        if fact.fact_id.trim().is_empty()
            || fact.proposition.trim().is_empty()
            || fact.scope.trim().is_empty()
            || !seen.insert(fact.fact_id.clone())
            || fact.influence_milli > 1000
            || !digest(fact.source_digest.as_ref().unwrap_or(&query.replay_identity)) && fact.source_digest.is_some()
            || !digest(fact.provenance_digest.as_ref().unwrap_or(&query.replay_identity)) && fact.provenance_digest.is_some()
            || !digest(&fact.replay_identity)
            || fact.semantic_profile.trim().is_empty()
            || !canonical(&fact.omissions)
            || !canonical(&fact.uncertainty)
    {
            return Err(invalid(format!("fact {} is malformed or duplicated", fact.fact_id)));
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

    fn fact(id: &str, state: EvidenceState, influence: u16) -> ContextFact {
        ContextFact {
            fact_id: id.into(),
            proposition: format!("proposition-{id}"),
            scope: "organoid-study".into(),
            evidence_state: state,
            influence_milli: influence,
            source_digest: Some(hash(&format!("source-{id}"))),
            provenance_digest: Some(hash(&format!("provenance-{id}"))),
            semantic_profile: "preclinical-neural-organoid".into(),
            replay_identity: hash("replay"),
            negative_result: false,
            omissions: vec![],
            uncertainty: vec![],
            local_data: true,
            permitted: true,
        }
    }

    fn query(facts: Vec<ContextFact>) -> DecisionQuery {
        DecisionQuery {
            request_id: "query-1".into(),
            workflow_id: "workflow-1".into(),
            target_schema: "CertifiedDecisionSection2@1".into(),
            purpose: "bounded-context-release".into(),
            semantic_profile: "preclinical-neural-organoid".into(),
            required_fact_order: vec!["fact-a".into()],
            required_scope_order: vec!["organoid-study".into()],
            facts,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            budget: 4,
            max_budget: 8,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2_and_typed() {
        let manifest = context_compilation_assurance_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        manifest.validate().unwrap();
    }

    #[test]
    fn supported_context_is_qualified() {
        let report = assure_context_compilation(&query(vec![fact(
            "fact-a",
            EvidenceState::Supported,
            900,
        )]))
        .unwrap();
        assert_eq!(report.disposition, "qualified");
        assert_eq!(report.selected_order, vec!["fact-a"]);
        report.validate().unwrap();
    }

    #[test]
    fn missing_scope_is_explicitly_unresolved() {
        let mut q = query(vec![fact("fact-a", EvidenceState::Supported, 900)]);
        q.required_scope_order.push("animal-study".into());
        q.required_scope_order.sort();
        let report = assure_context_compilation(&q).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report.missing_scope_order.contains(&"animal-study".into()));
    }

    #[test]
    fn unknown_and_contradicted_facts_are_not_selected() {
        let mut q = query(vec![
            fact("fact-a", EvidenceState::Unknown, 990),
            fact("fact-b", EvidenceState::Contradicted, 990),
        ]);
        q.required_fact_order = vec!["fact-a".into()];
        let report = assure_context_compilation(&q).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report.unresolved_order.contains(&"fact-a".into()));
        assert!(report.blocked_order.contains(&"fact-b".into()));
    }

    #[test]
    fn policy_and_adversarial_events_block() {
        let mut q = query(vec![fact("fact-a", EvidenceState::Supported, 900)]);
        q.adversarial_events = vec!["poisoned-context".into()];
        let report = assure_context_compilation(&q).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert_eq!(report.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn duplicate_fact_is_rejected() {
        let q = query(vec![
            fact("fact-a", EvidenceState::Supported, 900),
            fact("fact-a", EvidenceState::Supported, 800),
        ]);
        assert!(matches!(
            assure_context_compilation(&q),
            Err(ContextAssuranceError::Invalid(_))
        ));
    }

    #[test]
    fn ranking_is_deterministic() {
        let first = assure_context_compilation(&query(vec![
            fact("fact-b", EvidenceState::Supported, 700),
            fact("fact-a", EvidenceState::Supported, 900),
        ]))
        .unwrap();
        let second = assure_context_compilation(&query(vec![
            fact("fact-a", EvidenceState::Supported, 900),
            fact("fact-b", EvidenceState::Supported, 700),
        ]))
        .unwrap();
        assert_eq!(first.ranked_order, second.ranked_order);
        assert_eq!(first.context_digest, second.context_digest);
    }
}
