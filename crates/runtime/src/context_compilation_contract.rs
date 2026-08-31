//! Prospective high-throughput context-compilation contract model.
//!
//! Atlas feature: `AFA-runtime-P03-F07`.
//!
//! This contract model is the typed boundary between a bounded `DecisionQuery3` and a certified
//! `CertifiedDecisionSection2`. It selects only supplied, local, evidence-backed facts; omitted,
//! unknown, contradictory, policy-blocked, and budget-deferred facts remain in the certificate.
//! No model, network, instrument, or clinical workflow is executed here.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-runtime-P03-F07";
pub const CONTRACT_VERSION: &str = "runtime-prospective-context-compilation-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "DecisionQuery3@1";
pub const OUTPUT_SCHEMA: &str = "CertifiedDecisionSection2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactBinding {
    pub fact_id: String,
    pub evidence_state: EvidenceState,
    pub source_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub influence_milli: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionQuery {
    pub query_id: String,
    pub scope: String,
    pub target: String,
    pub schema_version: String,
    pub facts: Vec<FactBinding>,
    pub required_fact_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub semantic_digest: ContentHash,
    pub budget_units: u32,
    pub max_budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertifiedDecisionSection {
    pub schema_version: String,
    pub section_id: String,
    pub query_id: String,
    pub scope: String,
    pub target: String,
    pub selected_fact_order: Vec<String>,
    pub omitted_fact_order: Vec<String>,
    pub unresolved_fact_order: Vec<String>,
    pub contradicted_fact_order: Vec<String>,
    pub unknown_fact_order: Vec<String>,
    pub required_fact_order: Vec<String>,
    pub omission_certificate: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub section_digest: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub query_id: String,
    pub compatibility: String,
    pub migration_order: Vec<String>,
    pub section: CertifiedDecisionSection,
    pub checks: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextContractError {
    #[error("invalid context contract: {0}")]
    Invalid(String),
    #[error("context contract artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl CertifiedDecisionSection {
    pub fn validate(&self) -> Result<(), ContextContractError> {
        if self.schema_version != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.section_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.target.trim().is_empty()
            || self.required_fact_order.is_empty()
            || self.artifact.content_type
                != "application/vnd.aurora.certified-decision-section+json"
        {
            return Err(ContextContractError::Invalid(
                "section identity, schema, boundary, required facts, or artifact type is incomplete".into(),
            ));
        }
        for values in [
            &self.selected_fact_order,
            &self.omitted_fact_order,
            &self.unresolved_fact_order,
            &self.contradicted_fact_order,
            &self.unknown_fact_order,
            &self.required_fact_order,
            &self.omission_certificate,
            &self.uncertainty,
            &self.negative_evidence,
        ] {
            if !canonical(values) {
                return Err(ContextContractError::Invalid(
                    "section orders and certificates are not canonical".into(),
                ));
            }
        }
        let required = self
            .required_fact_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partition = self
            .selected_fact_order
            .iter()
            .chain(self.omitted_fact_order.iter())
            .chain(self.unresolved_fact_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if partition != required
            || partition.len()
                != self.selected_fact_order.len()
                    + self.omitted_fact_order.len()
                    + self.unresolved_fact_order.len()
        {
            return Err(ContextContractError::Invalid(
                "section fact states do not partition the required query".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextContractError::Artifact(error.to_string()))
    }
}

impl ContextContractReceipt {
    pub fn validate(&self) -> Result<(), ContextContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.query_id.trim().is_empty()
            || self.compatibility.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self
                .migration_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.checks.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ContextContractError::Invalid(
                "contract identity, compatibility, checks, migration, locality, or effects are incomplete".into(),
            ));
        }
        if !self
            .effect_receipts
            .iter()
            .all(|effect| effect == "retain:context-contract" || effect == "block:unsafe-release")
        {
            return Err(ContextContractError::Invalid(
                "context contract effect is outside the retention gate".into(),
            ));
        }
        self.section.validate()
    }

    pub fn digest(&self) -> Result<ContentHash, ContextContractError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ContextContractError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ContextContractError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "runtime".into(),
        consumers: BTreeSet::from([
            "context compiler".into(),
            "decision-section reviewer".into(),
            "high-throughput research workflow".into(),
        ]),
        behavior: "serializes and validates a bounded DecisionQuery3 into a certified omission-aware decision section".into(),
        value: "provides a byte-stable context contract for high-throughput research without silently replacing omitted or uncertain facts".into(),
        inputs: vec![TypedPort { name: "decision_query".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "certified_decision_section".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]),
        permissions: BTreeSet::from(["read:local-research-artifacts".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "context-reviewer".into(), reason: "certified decision section retention".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_query(query: &DecisionQuery) -> Result<(), ContextContractError> {
    if query.query_id.trim().is_empty()
        || query.scope.trim().is_empty()
        || query.target.trim().is_empty()
        || query.schema_version != INPUT_SCHEMA
        || query.facts.is_empty()
        || query.required_fact_order.is_empty()
        || query.budget_units == 0
        || query.max_budget_units == 0
        || query.budget_units > query.max_budget_units
        || !query.raw_data_local
        || query.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextContractError::Invalid(
            "query identity, schema, facts, budget, locality, or boundary is invalid".into(),
        ));
    }
    if !canonical(&query.required_fact_order)
        || query
            .required_fact_order
            .iter()
            .any(|fact| fact.trim().is_empty())
    {
        return Err(ContextContractError::Invalid(
            "required fact order is not canonical".into(),
        ));
    }
    let mut fact_ids = query
        .facts
        .iter()
        .map(|fact| fact.fact_id.clone())
        .collect::<Vec<_>>();
    fact_ids.sort();
    if fact_ids.windows(2).any(|pair| pair[0] == pair[1])
        || query.facts.iter().any(|fact| {
            fact.fact_id.trim().is_empty()
                || fact.source_digest.is_none()
                || fact.provenance_digest.is_none()
        })
    {
        return Err(ContextContractError::Invalid(
            "fact identifiers must be unique and content/provenance complete".into(),
        ));
    }
    let supplied = fact_ids.into_iter().collect::<BTreeSet<_>>();
    if !query
        .required_fact_order
        .iter()
        .all(|fact| supplied.contains(fact))
    {
        return Err(ContextContractError::Invalid(
            "required fact is absent from query bindings".into(),
        ));
    }
    Ok(())
}

pub fn compile(query: &DecisionQuery) -> Result<ContextContractReceipt, ContextContractError> {
    validate_query(query)?;
    let facts = query
        .facts
        .iter()
        .map(|fact| (fact.fact_id.clone(), fact))
        .collect::<BTreeMap<_, _>>();
    let required = query
        .required_fact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut ranked = query
        .required_fact_order
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        facts[right]
            .influence_milli
            .cmp(&facts[left].influence_milli)
            .then_with(|| left.cmp(right))
    });
    let mut selected = Vec::new();
    let mut omitted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omission_certificate = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut spent = 0_u32;
    for fact_id in &ranked {
        let fact = facts[fact_id];
        let mut blocked_by_metadata = false;
        match fact.evidence_state {
            EvidenceState::Contradicted => {
                omitted.insert(fact_id.clone());
                contradicted.insert(fact_id.clone());
                negative.insert(format!("{fact_id}:contradicted"));
                semantic_loss.push(SemanticLoss {
                    field: format!("fact:{fact_id}"),
                    reason: "contradicted fact cannot enter a certified section".into(),
                    severity: LossSeverity::DecisionRelevant,
                });
                continue;
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                unresolved.insert(fact_id.clone());
                unknown.insert(fact_id.clone());
                uncertainty.insert(format!("{fact_id}:evidence-state"));
                continue;
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        if !query.policy_allow {
            unresolved.insert(fact_id.clone());
            omission_certificate.insert(format!("{fact_id}:policy-denied"));
            blocked_by_metadata = true;
        }
        if !query.protected_closure {
            unresolved.insert(fact_id.clone());
            omission_certificate.insert(format!("{fact_id}:protected-closure-incomplete"));
            blocked_by_metadata = true;
        }
        if !fact.omissions.is_empty() {
            unresolved.insert(fact_id.clone());
            omission_certificate.extend(
                fact.omissions
                    .iter()
                    .map(|item| format!("{fact_id}:{item}")),
            );
            blocked_by_metadata = true;
        }
        if !fact.uncertainty.is_empty() {
            unresolved.insert(fact_id.clone());
            uncertainty.extend(
                fact.uncertainty
                    .iter()
                    .map(|item| format!("{fact_id}:{item}")),
            );
            blocked_by_metadata = true;
        }
        if blocked_by_metadata {
            continue;
        }
        let cost = fact_id.len() as u32 + 1;
        if cost > query.budget_units.saturating_sub(spent) {
            unresolved.insert(fact_id.clone());
            omission_certificate.insert(format!("{fact_id}:budget-ceiling"));
            continue;
        }
        spent = spent.saturating_add(cost);
        selected.push(fact_id.clone());
        negative.insert(format!("{fact_id}:negative-result-not-observed"));
    }
    for fact_id in &required {
        if !selected.contains(fact_id)
            && !unresolved.contains(fact_id)
            && !omitted.contains(fact_id)
        {
            omitted.insert(fact_id.clone());
            omission_certificate.insert(format!("{fact_id}:not-admitted"));
        }
    }
    selected.sort();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let mut checks = vec![
        "artifact-closure",
        "budget-bound",
        "canonical-order",
        "evidence-state",
        "policy-boundary",
        "provenance-closure",
        "replay-identity",
        "schema-compatibility",
        "protected-closure",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    checks.sort();
    let disposition = if !query.policy_allow || !query.protected_closure {
        "blocked"
    } else if !omitted_order.is_empty() || !unresolved_order.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let payload = json!({
        "schema_version": OUTPUT_SCHEMA,
        "section_id": format!("section:{}", query.query_id),
        "query_id": query.query_id,
        "scope": query.scope,
        "target": query.target,
        "selected_fact_order": selected,
        "omitted_fact_order": omitted_order,
        "unresolved_fact_order": unresolved_order,
        "required_fact_order": query.required_fact_order,
        "semantic_digest": query.semantic_digest,
        "replay_identity": query.replay_identity,
        "disposition": disposition,
    });
    let section_digest = ContentHash::of_value(&payload)
        .map_err(|error| ContextContractError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("certified-decision-section:{}", query.query_id),
        "application/vnd.aurora.certified-decision-section+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: query.query_id.clone(),
            relation: "context-contract-compilation".into(),
            digest: section_digest.clone(),
        }],
    )
    .map_err(|error| ContextContractError::Artifact(error.to_string()))?;
    let section = CertifiedDecisionSection {
        schema_version: OUTPUT_SCHEMA.into(),
        section_id: format!("section:{}", query.query_id),
        query_id: query.query_id.clone(),
        scope: query.scope.clone(),
        target: query.target.clone(),
        selected_fact_order: selected,
        omitted_fact_order: omitted_order,
        unresolved_fact_order: unresolved_order,
        contradicted_fact_order: contradicted.into_iter().collect(),
        unknown_fact_order: unknown.into_iter().collect(),
        required_fact_order: query.required_fact_order.clone(),
        omission_certificate: omission_certificate.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        semantic_digest: query.semantic_digest.clone(),
        replay_identity: query.replay_identity.clone(),
        section_digest,
        semantic_loss,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    section.validate()?;
    let compatibility = "compatible".to_string();
    let receipt = ContextContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        query_id: query.query_id.clone(),
        compatibility,
        migration_order: Vec::new(),
        section,
        checks,
        effect_receipts: if disposition == "qualified" {
            vec!["retain:context-contract".into()]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: query.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

pub fn compile_json(value: &Value) -> Result<Value, ContextContractError> {
    let query: DecisionQuery = serde_json::from_value(value.clone())
        .map_err(|error| ContextContractError::Invalid(error.to_string()))?;
    serde_json::to_value(compile(&query)?)
        .map_err(|error| ContextContractError::Artifact(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"context-contract")
    }
    fn fact(id: &str, state: EvidenceState) -> FactBinding {
        FactBinding {
            fact_id: id.into(),
            evidence_state: state,
            source_digest: Some(hash()),
            provenance_digest: Some(hash()),
            influence_milli: 90_000,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
        }
    }
    fn query() -> DecisionQuery {
        DecisionQuery {
            query_id: "query:context".into(),
            scope: "organoid".into(),
            target: "resilience".into(),
            schema_version: INPUT_SCHEMA.into(),
            facts: vec![
                fact("fact-a", EvidenceState::Supported),
                fact("fact-b", EvidenceState::Proven),
            ],
            required_fact_order: vec!["fact-a".into(), "fact-b".into()],
            replay_identity: hash(),
            semantic_digest: hash(),
            budget_units: 100,
            max_budget_units: 100,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn complete_query_compiles_and_replays() {
        let receipt = compile(&query()).unwrap();
        assert_eq!(receipt.compatibility, "compatible");
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_and_budget_remain_unresolved() {
        let mut value = query();
        value.facts[0].evidence_state = EvidenceState::Unknown;
        value.budget_units = 1;
        let receipt = compile(&value).unwrap();
        assert_eq!(receipt.section.unresolved_fact_order.len(), 2);
        assert!(!receipt.section.uncertainty.is_empty());
    }
    #[test]
    fn contradiction_and_policy_block() {
        let mut value = query();
        value.facts[0].evidence_state = EvidenceState::Contradicted;
        value.policy_allow = false;
        let receipt = compile(&value).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        assert!(receipt
            .section
            .contradicted_fact_order
            .contains(&"fact-a".into()));
    }
    #[test]
    fn protected_closure_is_never_silently_released() {
        let mut value = query();
        value.protected_closure = false;
        let receipt = compile(&value).unwrap();
        assert_eq!(receipt.section.selected_fact_order.len(), 0);
        assert!(receipt
            .section
            .omission_certificate
            .iter()
            .any(|item| item.contains("protected")));
    }
    #[test]
    fn manifest_is_a1_and_typed() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A1);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Api));
    }
}
