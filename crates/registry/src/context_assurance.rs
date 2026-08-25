//! Federated continual context-compilation assurance harness.
//!
//! Atlas feature: `AFA-registry-P03-F28`.
//! Context is admitted only when required facts are typed, comparable,
//! provenance-bound, policy-authorized, and protected-closure complete. Raw
//! source bytes remain local; federation carries signed digests and omission
//! certificates only.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-registry-P03-F28";
pub const CONTRACT_VERSION: &str = "registry-federated-context-compilation-assurance/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFact {
    pub fact_id: String,
    pub fact_class: String,
    pub scope: String,
    pub semantic_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparability_digest: Option<ContentHash>,
    pub state: FactState,
    pub freshness_epoch: u64,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub question_id: String,
    pub scope: String,
    pub required_fact_classes: Vec<String>,
    pub minimum_freshness_epoch: u64,
    pub facts: Vec<ContextFact>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_allow: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextDisposition {
    Compiled,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledContext {
    pub context_id: String,
    pub disposition: ContextDisposition,
    pub fact_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub class_order: Vec<String>,
    pub semantic_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub context_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub question_id: String,
    pub disposition: ContextDisposition,
    pub context: CompiledContext,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextAssuranceError {
    #[error("invalid context assurance request: {0}")]
    Invalid(String),
    #[error("context assurance serialization failed: {0}")]
    Serialization(String),
}

impl ContextAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ContextAssuranceError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.question_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.context.boundary != PRECLINICAL_BOUNDARY
            || self.context.context_id.trim().is_empty()
            || (self.context.selected_order.is_empty()
                && self.context.blocked_order.is_empty()
                && self.context.omissions.is_empty()
                && self.context.uncertainty.is_empty()
                && self.context.negative_evidence.is_empty())
        {
            return Err(ContextAssuranceError::Invalid(
                "context assurance identity, context, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.context.fact_order,
            &self.context.selected_order,
            &self.context.blocked_order,
            &self.context.class_order,
            &self.context.omissions,
            &self.context.uncertainty,
            &self.context.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextAssuranceError::Invalid(
                    "context assurance ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.context.semantic_order,
            &self.context.evidence_order,
            &self.context.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextAssuranceError::Invalid(
                    "context assurance digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ContextAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextAssuranceError::Serialization(error.to_string()))
    }
}

pub fn assure_context_compilation(
    request: &ContextCompilationRequest,
) -> Result<ContextAssuranceReceipt, ContextAssuranceError> {
    validate_request(request)?;
    let mut facts = request.facts.clone();
    facts.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));
    let required = request
        .required_fact_classes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut fact_order = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut semantics = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for fact in &facts {
        fact_order.insert(fact.fact_id.clone());
        let cost = fact.fact_id.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = fact.comparability_digest.is_some()
            && fact.state == FactState::Supported
            && fact.scope == request.scope
            && fact.freshness_epoch >= request.minimum_freshness_epoch
            && fact.omissions.is_empty()
            && fact.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.protected_closure
            && request.federation_allow
            && request.signed_approval
            && request.raw_data_local
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            selected.insert(fact.fact_id.clone());
            classes.insert(fact.fact_class.clone());
            semantics.insert(fact.semantic_digest.clone());
            evidence.insert(fact.evidence_digest.clone());
            provenance.insert(fact.provenance_digest.clone());
        } else {
            blocked.insert(fact.fact_id.clone());
            if fact.state != FactState::Supported {
                negative.insert(
                    format!("fact:{}:state-{:?}-not-compiled", fact.fact_id, fact.state)
                        .to_ascii_lowercase(),
                );
            }
            if fact.comparability_digest.is_none() {
                omissions.insert(format!("fact:{}:comparability-missing", fact.fact_id));
            }
            if fact.scope != request.scope {
                omissions.insert(format!("fact:{}:scope-mismatch", fact.fact_id));
            }
            if fact.freshness_epoch < request.minimum_freshness_epoch {
                uncertainty.insert(format!("fact:{}:stale-context", fact.fact_id));
            }
            if !fact.omissions.is_empty() || !fact.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "fact:{}:protected-closure-incomplete",
                    fact.fact_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("fact:{}:budget-ceiling-exceeded", fact.fact_id));
            }
        }
    }
    for class in required {
        if !classes.contains(&class) {
            omissions.insert(format!("fact-class:{class}:required-but-not-compiled"));
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow || !request.federation_allow {
        ContextDisposition::Blocked
    } else if !request.protected_closure || selected_order.is_empty() {
        ContextDisposition::Unknown
    } else if blocked_order.is_empty() && omissions.is_empty() {
        ContextDisposition::Compiled
    } else {
        ContextDisposition::Partial
    };
    let mut checks = vec![
        "fact and digest ordering is canonical".into(),
        "scope, freshness, comparability, evidence, provenance, policy, federation, approval, locality, and budget gates are explicit".into(),
        "contradicted, unknown, unmeasured, stale, omitted, and negative facts remain unresolved".into(),
        "federation exchanges signed digest manifests without raw source bytes".into(),
    ];
    checks.sort();
    let fact_order = fact_order.into_iter().collect::<Vec<_>>();
    let class_order = classes.into_iter().collect::<Vec<_>>();
    let semantic_order = semantics.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = fact_order
        .iter()
        .map(|fact_id| format!("exchange:signed-context-digest:{fact_id}"))
        .collect::<Vec<_>>();
    let context_id = format!("compiled-context:{}", request.request_id);
    let context_payload = json!({
        "context_id": context_id,
        "disposition": disposition,
        "fact_order": fact_order,
        "selected_order": selected_order,
        "blocked_order": blocked_order,
        "class_order": class_order,
        "semantic_order": semantic_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let context_digest = ContentHash::of_value(&context_payload)
        .map_err(|error| ContextAssuranceError::Serialization(error.to_string()))?;
    let context = CompiledContext {
        context_id,
        disposition,
        fact_order,
        selected_order,
        blocked_order,
        class_order,
        semantic_order,
        evidence_order,
        provenance_order,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: request.replay_identity.clone(),
        context_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = ContextAssuranceReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        question_id: request.question_id.clone(),
        disposition,
        context,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ContextCompilationRequest) -> Result<(), ContextAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.question_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_fact_classes.is_empty()
        || request.facts.is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_fact_classes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(ContextAssuranceError::Invalid(
            "context assurance identity, scope, classes, facts, budget, or boundary is incomplete"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for fact in &request.facts {
        if fact.fact_id.trim().is_empty()
            || fact.fact_class.trim().is_empty()
            || fact.scope.trim().is_empty()
            || !ids.insert(fact.fact_id.clone())
            || fact.boundary != PRECLINICAL_BOUNDARY
            || fact.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || fact.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || fact
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(ContextAssuranceError::Invalid(format!(
                "context fact {} is invalid or duplicated",
                fact.fact_id
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

    fn fact(id: &str, class: &str, state: FactState) -> ContextFact {
        ContextFact {
            fact_id: id.into(),
            fact_class: class.into(),
            scope: "organoid:neural".into(),
            semantic_digest: hash(&format!("semantic:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            comparability_digest: Some(hash("comparability")),
            state,
            freshness_epoch: 10,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(facts: Vec<ContextFact>) -> ContextCompilationRequest {
        ContextCompilationRequest {
            request_id: "context:assurance".into(),
            workflow_id: "workflow:context".into(),
            question_id: "question:organoid".into(),
            scope: "organoid:neural".into(),
            required_fact_classes: vec!["mechanism".into(), "scope".into()],
            minimum_freshness_epoch: 5,
            facts,
            replay_identity: hash("replay"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            federation_allow: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn compiles_supported_fresh_context() {
        let receipt = assure_context_compilation(&request(vec![
            fact("fact:a", "mechanism", FactState::Supported),
            fact("fact:b", "scope", FactState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ContextDisposition::Compiled);
        assert_eq!(receipt.context.selected_order, vec!["fact:a", "fact:b"]);
    }

    #[test]
    fn stale_context_is_uncertain() {
        let mut stale = fact("fact:a", "mechanism", FactState::Supported);
        stale.freshness_epoch = 1;
        let receipt = assure_context_compilation(&request(vec![
            stale,
            fact("fact:b", "scope", FactState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, ContextDisposition::Partial);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("stale")));
    }

    #[test]
    fn contradiction_remains_negative_evidence() {
        let receipt = assure_context_compilation(&request(vec![
            fact("fact:a", "mechanism", FactState::Contradicted),
            fact("fact:b", "scope", FactState::Supported),
        ]))
        .unwrap();
        assert!(!receipt.negative_evidence.is_empty());
    }

    #[test]
    fn federation_denial_blocks_context_exchange() {
        let mut input = request(vec![fact("fact:a", "mechanism", FactState::Supported)]);
        input.federation_allow = false;
        let receipt = assure_context_compilation(&input).unwrap();
        assert_eq!(receipt.disposition, ContextDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("federation")));
    }

    #[test]
    fn duplicate_facts_are_rejected() {
        let result = assure_context_compilation(&request(vec![
            fact("fact:a", "mechanism", FactState::Supported),
            fact("fact:a", "scope", FactState::Supported),
        ]));
        assert!(result.is_err());
    }
}
