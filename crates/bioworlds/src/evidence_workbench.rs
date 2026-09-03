//! Local single-study evidence-surveillance workbench.
//!
//! Atlas feature: `AFA-bioworlds-P01-F17`.
//! This A0 interaction surface qualifies an institution-local evidence feed,
//! emits researcher-visible alerts for stale, contradictory, unmeasured, and
//! incomplete sources, and never performs network, laboratory, or clinical
//! effects.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioworlds-P01-F17";
pub const CONTRACT_VERSION: &str = "bioworlds-local-evidence-workbench/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSource {
    pub source_id: String,
    pub source_class: String,
    pub statement_digest: ContentHash,
    pub source_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub scope: String,
    pub state: EvidenceState,
    pub freshness: Freshness,
    pub relevance_score: u16,
    pub full_text_available: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeed {
    pub request_id: String,
    pub workflow_id: String,
    pub study_id: String,
    pub scope: String,
    pub query: String,
    pub required_source_classes: Vec<String>,
    pub minimum_relevance_score: u16,
    pub sources: Vec<EvidenceSource>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet {
    pub set_id: String,
    pub disposition: EvidenceDisposition,
    pub source_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub alert_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub set_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub study_id: String,
    pub disposition: EvidenceDisposition,
    pub evidence: QualifiedEvidenceSet,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceWorkbenchError {
    #[error("invalid evidence workbench request: {0}")]
    Invalid(String),
    #[error("evidence workbench serialization failed: {0}")]
    Serialization(String),
}

impl EvidenceWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), EvidenceWorkbenchError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.evidence.boundary != PRECLINICAL_BOUNDARY
            || (self.evidence.qualified_order.is_empty()
                && self.evidence.alert_order.is_empty()
                && self.evidence.blocked_order.is_empty()
                && self.evidence.omissions.is_empty()
                && self.evidence.uncertainty.is_empty()
                && self.evidence.negative_evidence.is_empty())
        {
            return Err(EvidenceWorkbenchError::Invalid(
                "workbench identity, evidence set, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.evidence.source_order,
            &self.evidence.qualified_order,
            &self.evidence.alert_order,
            &self.evidence.blocked_order,
            &self.evidence.omissions,
            &self.evidence.uncertainty,
            &self.evidence.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceWorkbenchError::Invalid(
                    "evidence workbench ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.evidence.evidence_order,
            &self.evidence.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceWorkbenchError::Invalid(
                    "evidence workbench digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceWorkbenchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceWorkbenchError::Serialization(error.to_string()))
    }
}

pub fn operate_evidence_workbench(
    feed: &EvidenceFeed,
) -> Result<EvidenceWorkbenchReceipt, EvidenceWorkbenchError> {
    validate_feed(feed)?;
    let mut sources = feed.sources.clone();
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    let required_classes = feed
        .required_source_classes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut source_order = BTreeSet::new();
    let mut qualified = BTreeSet::new();
    let mut alerts = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for source in &sources {
        source_order.insert(source.source_id.clone());
        let cost = source.source_id.len() as u64 + 1;
        let budget_ok = cost <= feed.budget.saturating_sub(spent);
        let complete = source.full_text_available
            && source.freshness == Freshness::Fresh
            && source.scope == feed.scope
            && source.relevance_score >= feed.minimum_relevance_score
            && source.omissions.is_empty()
            && source.uncertainty.is_empty();
        let gate = feed.policy_allow
            && feed.protected_closure
            && feed.raw_data_local
            && source.state == EvidenceState::Supported
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            qualified.insert(source.source_id.clone());
            evidence.insert(source.statement_digest.clone());
            provenance.insert(source.provenance_digest.clone());
        } else {
            blocked.insert(source.source_id.clone());
            if source.state != EvidenceState::Supported {
                alerts.insert(
                    format!("source:{}:state-{:?}", source.source_id, source.state)
                        .to_ascii_lowercase(),
                );
                negative.insert(
                    format!(
                        "source:{}:state-{:?}-not-qualified",
                        source.source_id, source.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if source.freshness != Freshness::Fresh {
                alerts.insert(
                    format!(
                        "source:{}:freshness-{:?}",
                        source.source_id, source.freshness
                    )
                    .to_ascii_lowercase(),
                );
                uncertainty.insert(format!("source:{}:freshness-not-current", source.source_id));
            }
            if source.scope != feed.scope {
                omissions.insert(format!("source:{}:scope-mismatch", source.source_id));
            }
            if source.relevance_score < feed.minimum_relevance_score {
                alerts.insert(format!("source:{}:below-relevance-floor", source.source_id));
            }
            if !source.full_text_available
                || !source.omissions.is_empty()
                || !source.uncertainty.is_empty()
            {
                omissions.insert(format!(
                    "source:{}:protected-closure-or-full-text-incomplete",
                    source.source_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "source:{}:budget-ceiling-exceeded",
                    source.source_id
                ));
            }
        }
    }
    for required_class in required_classes {
        if !sources.iter().any(|source| {
            source.source_class == required_class && qualified.contains(&source.source_id)
        }) {
            omissions.insert(format!(
                "source-class:{required_class}:required-but-not-qualified"
            ));
        }
    }
    if !feed.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !feed.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let alert_order = alerts.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !feed.policy_allow {
        EvidenceDisposition::Blocked
    } else if !feed.protected_closure || qualified_order.is_empty() {
        EvidenceDisposition::Unknown
    } else if blocked_order.is_empty() && omissions.is_empty() {
        EvidenceDisposition::Qualified
    } else {
        EvidenceDisposition::Partial
    };
    let mut checks = vec![
        "source, alert, omission, evidence, provenance, and effect ordering is canonical".into(),
        "scope, freshness, full-text, relevance, policy, locality, and budget gates are explicit".into(),
        "contradictory, unknown, unmeasured, stale, and incomplete sources remain researcher-visible alerts".into(),
        "A0 workbench operation is read-only and never performs network, laboratory, or clinical effects".into(),
    ];
    checks.sort();
    let source_order = source_order.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let mut effect_receipts = source_order
        .iter()
        .map(|source_id| format!("view:authorized-research-state:{source_id}"))
        .collect::<Vec<_>>();
    effect_receipts.sort();
    let set_id = format!("qualified-evidence:{}", feed.request_id);
    let set_payload = json!({
        "set_id": set_id,
        "disposition": disposition,
        "source_order": source_order,
        "qualified_order": qualified_order,
        "alert_order": alert_order,
        "blocked_order": blocked_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": feed.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let set_digest = ContentHash::of_value(&set_payload)
        .map_err(|error| EvidenceWorkbenchError::Serialization(error.to_string()))?;
    let evidence_set = QualifiedEvidenceSet {
        set_id,
        disposition,
        source_order,
        qualified_order,
        alert_order,
        blocked_order,
        evidence_order,
        provenance_order,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: feed.replay_identity.clone(),
        set_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = EvidenceWorkbenchReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: feed.request_id.clone(),
        workflow_id: feed.workflow_id.clone(),
        study_id: feed.study_id.clone(),
        disposition,
        evidence: evidence_set,
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

fn validate_feed(feed: &EvidenceFeed) -> Result<(), EvidenceWorkbenchError> {
    if feed.request_id.trim().is_empty()
        || feed.workflow_id.trim().is_empty()
        || feed.study_id.trim().is_empty()
        || feed.scope.trim().is_empty()
        || feed.query.trim().is_empty()
        || feed.required_source_classes.is_empty()
        || feed.sources.is_empty()
        || feed.budget == 0
        || feed.boundary != PRECLINICAL_BOUNDARY
        || feed
            .required_source_classes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(EvidenceWorkbenchError::Invalid(
            "workbench identity, query, source classes, evidence feed, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for source in &feed.sources {
        if source.source_id.trim().is_empty()
            || source.source_class.trim().is_empty()
            || source.scope.trim().is_empty()
            || !ids.insert(source.source_id.clone())
            || source.boundary != PRECLINICAL_BOUNDARY
            || source.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || source.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || source
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(EvidenceWorkbenchError::Invalid(format!(
                "evidence source {} is invalid or duplicated",
                source.source_id
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

    fn source(id: &str, state: EvidenceState, freshness: Freshness) -> EvidenceSource {
        EvidenceSource {
            source_id: id.into(),
            source_class: "primary-study".into(),
            statement_digest: hash(&format!("statement:{id}")),
            source_digest: hash(&format!("source:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            scope: "organoid:neural".into(),
            state,
            freshness,
            relevance_score: 90,
            full_text_available: true,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn feed(sources: Vec<EvidenceSource>) -> EvidenceFeed {
        EvidenceFeed {
            request_id: "evidence:workbench".into(),
            workflow_id: "workflow:surveillance".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            query: "neural organoid synaptic maturation".into(),
            required_source_classes: vec!["primary-study".into()],
            minimum_relevance_score: 70,
            sources,
            replay_identity: hash("replay"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn qualifies_fresh_scoped_evidence_read_only() {
        let receipt = operate_evidence_workbench(&feed(vec![source(
            "source:a",
            EvidenceState::Supported,
            Freshness::Fresh,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, EvidenceDisposition::Qualified);
        assert_eq!(receipt.evidence.qualified_order, vec!["source:a"]);
        assert!(receipt.effect_receipts[0].starts_with("view:"));
        assert_eq!(receipt.digest(), receipt.digest());
    }

    #[test]
    fn stale_evidence_becomes_an_alert_not_a_qualified_result() {
        let receipt = operate_evidence_workbench(&feed(vec![source(
            "source:a",
            EvidenceState::Supported,
            Freshness::Stale,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, EvidenceDisposition::Unknown);
        assert!(receipt
            .evidence
            .alert_order
            .iter()
            .any(|item| item.contains("freshness")));
    }

    #[test]
    fn contradiction_is_visible_as_negative_evidence() {
        let receipt = operate_evidence_workbench(&feed(vec![source(
            "source:a",
            EvidenceState::Contradicted,
            Freshness::Fresh,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, EvidenceDisposition::Unknown);
        assert!(!receipt.negative_evidence.is_empty());
    }

    #[test]
    fn protected_closure_gap_is_unknown() {
        let mut input = feed(vec![source(
            "source:a",
            EvidenceState::Supported,
            Freshness::Fresh,
        )]);
        input.protected_closure = false;
        let receipt = operate_evidence_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, EvidenceDisposition::Unknown);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("protected-closure")));
    }

    #[test]
    fn duplicate_source_ids_are_rejected() {
        let result = operate_evidence_workbench(&feed(vec![
            source("source:a", EvidenceState::Supported, Freshness::Fresh),
            source("source:a", EvidenceState::Supported, Freshness::Fresh),
        ]));
        assert!(result.is_err());
    }
}
