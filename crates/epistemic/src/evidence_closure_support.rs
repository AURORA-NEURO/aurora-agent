//! Epistemic P32 frontier: certify evidence-backed assertions without collapsing uncertainty.
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
pub const FEATURE_ID: &str = "AFA-epistemic-P32-F01";
pub const CONTRACT_VERSION: &str = "epistemic-local-evidence-closure/2.0";
pub const OUTPUT_SCHEMA_VERSION: &str = "2.0.0";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.epistemic.evidence-closure-card-2+json";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicAssertion4 {
    pub assertion_id: String,
    pub source_id: String,
    pub statement: String,
    pub evidence_digest: ContentHash,
    pub uncertainty_milli: u64,
    pub policy_epoch: u64,
    pub competing_explanation: bool,
    pub contradicted: bool,
    pub negative_result: bool,
    pub local: bool,
    pub aggregate_only: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceClosureRequest4 {
    pub request_id: String,
    pub purpose: String,
    pub assertions: Vec<EpistemicAssertion4>,
    pub required_assertion_order: Vec<String>,
    pub required_source_order: Vec<String>,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub adversarial_events: Vec<String>,
    pub action_budget: u32,
    pub action_count: u32,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceClosureArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    /// Evidence identity keyed by assertion so duplicate digests cannot erase provenance.
    pub assertion_digests: BTreeMap<String, ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceClosureCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub mode: String,
    pub scale: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub assertion_order: Vec<String>,
    /// Source identity retained for every assertion in `assertion_order`.
    pub source_by_assertion: BTreeMap<String, String>,
    /// Digest of each statement, retained without copying potentially sensitive statement text.
    pub statement_digest_by_assertion: BTreeMap<String, ContentHash>,
    pub supported_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub source_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub competing_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub closure_digest: ContentHash,
    pub artifact: EvidenceClosureArtifact4,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EvidenceClosureError {
    #[error("invalid evidence-closure request: {0}")]
    Invalid(String),
    #[error("evidence-closure card failed validation: {0}")]
    Output(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn ordered_nonempty(v: &[String]) -> bool {
    ordered(v) && v.iter().all(|value| nonempty(value))
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn nonempty(v: &str) -> bool {
    !v.trim().is_empty()
}
pub fn manifest(id: &str, version: &str, scale: &str, mode: &str) -> serde_json::Value {
    json!({"schema_version":"1.0.0","capability_id":id,"version":version,"owner_crate":"epistemic","consumers":["evidence compiler","retrieval synthesizer","decision-section compiler","release auditor"],"behavior":format!("qualify evidence-backed assertion closure at {scale} ({mode})"),"value":"keeps uncertainty, contradictions, competing explanations, and negative evidence visible instead of manufacturing confidence","input_schema":"EvidenceClosureRequest4@1","output_schema":"EvidenceClosureCard7@2","content_binding":"canonical card body excluding closure_digest and artifact.content_hash","effects":["emit:evidence-closure-card","retain:uncertainty","block:unsupported-claim"],"permissions":["read:local-assertions"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}

fn content_digest(value: &Value) -> Result<ContentHash, EvidenceClosureError> {
    let mut body = value.clone();
    let object = body.as_object_mut().ok_or_else(|| {
        EvidenceClosureError::Output("evidence closure is not a JSON object".into())
    })?;
    object.remove("closure_digest");
    if let Some(artifact) = object.get_mut("artifact").and_then(Value::as_object_mut) {
        artifact.remove("content_hash");
    }
    ContentHash::of_value(&body).map_err(|error| EvidenceClosureError::Output(error.to_string()))
}
impl EvidenceClosureCard7 {
    pub fn validate(&self) -> Result<(), EvidenceClosureError> {
        if self.schema_version != OUTPUT_SCHEMA_VERSION
            || !nonempty(&self.contract_version)
            || !nonempty(&self.feature_id)
            || !nonempty(&self.mode)
            || !nonempty(&self.scale)
            || !nonempty(&self.request_id)
            || !nonempty(&self.purpose)
            || self.boundary != BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.assertion_order.is_empty()
            || !ordered_nonempty(&self.assertion_order)
            || !ordered_nonempty(&self.supported_order)
            || !ordered_nonempty(&self.contradicted_order)
            || !ordered_nonempty(&self.unknown_order)
            || !ordered_nonempty(&self.omitted_order)
            || !ordered_nonempty(&self.rejected_order)
            || !ordered_nonempty(&self.source_order)
            || !ordered_nonempty(&self.uncertainty_order)
            || !ordered_nonempty(&self.competing_order)
            || !ordered_nonempty(&self.negative_evidence_order)
            || !digest(&self.replay_identity)
            || !digest(&self.closure_digest)
            || !nonempty(&self.artifact.artifact_id)
            || self.artifact.content_type != CONTENT_TYPE
            || !digest(&self.artifact.content_hash)
            || self.artifact.content_hash != self.closure_digest
            || self.artifact.semantic_loss != self.omitted_order
            || self.artifact.boundary != BOUNDARY
            || self.effect_receipts.len() != 1
        {
            return Err(EvidenceClosureError::Output(
                "evidence identity, ordering, locality, digest, or artifact is invalid".into(),
            ));
        }
        let ids = BTreeSet::from_iter(self.assertion_order.iter().cloned());
        let mapping_keys =
            |map: &BTreeMap<String, String>| map.keys().cloned().collect::<BTreeSet<_>>() == ids;
        let digest_keys = |map: &BTreeMap<String, ContentHash>| {
            map.keys().cloned().collect::<BTreeSet<_>>() == ids && map.values().all(digest)
        };
        if ids.len() != self.assertion_order.len()
            || !mapping_keys(&self.source_by_assertion)
            || self
                .source_by_assertion
                .values()
                .any(|source| !nonempty(source))
            || BTreeSet::from_iter(self.source_by_assertion.values().cloned())
                != BTreeSet::from_iter(self.source_order.iter().cloned())
            || !digest_keys(&self.statement_digest_by_assertion)
            || !digest_keys(&self.artifact.assertion_digests)
            || self.artifact.artifact_id != format!("epistemic-evidence:{}", self.request_id)
        {
            return Err(EvidenceClosureError::Output(
                "assertion provenance mapping or artifact identity is invalid".into(),
            ));
        }
        let states = self
            .supported_order
            .iter()
            .chain(&self.contradicted_order)
            .chain(&self.unknown_order)
            .chain(&self.omitted_order)
            .chain(&self.rejected_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.assertion_order.len()
            || states.len() != ids.len()
            || BTreeSet::from_iter(states) != ids
        {
            return Err(EvidenceClosureError::Output(
                "assertion states do not partition".into(),
            ));
        }
        let expected_effect = if self.disposition == "qualified" {
            format!("emit:evidence-closure:{}", self.request_id)
        } else {
            "block:unsupported-claim".to_string()
        };
        if self.effect_receipts != [expected_effect]
            || !["qualified", "partial", "unknown", "blocked"].contains(&self.disposition.as_str())
            || (self.disposition == "blocked"
                && (self.omitted_order != self.assertion_order
                    || !self.supported_order.is_empty()
                    || !self.contradicted_order.is_empty()
                    || !self.unknown_order.is_empty()
                    || !self.rejected_order.is_empty()))
            || (self.disposition == "qualified"
                && (self.supported_order != self.assertion_order
                    || !self.contradicted_order.is_empty()
                    || !self.unknown_order.is_empty()
                    || !self.omitted_order.is_empty()
                    || !self.rejected_order.is_empty()))
            || (self.disposition == "partial" && self.supported_order == self.assertion_order)
        {
            return Err(EvidenceClosureError::Output(
                "disposition, assertion states, or effect receipt is inconsistent".into(),
            ));
        }
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceClosureError::Output(error.to_string()))?;
        if content_digest(&value)? != self.closure_digest {
            return Err(EvidenceClosureError::Output(
                "closure digest does not bind the card body".into(),
            ));
        }
        Ok(())
    }
}
pub fn qualify(
    request: &EvidenceClosureRequest4,
    id: &str,
    version: &str,
    scale: &str,
    mode: &str,
) -> Result<EvidenceClosureCard7, EvidenceClosureError> {
    if !nonempty(&request.request_id)
        || !nonempty(&request.purpose)
        || !nonempty(id)
        || !nonempty(version)
        || !nonempty(scale)
        || !nonempty(mode)
        || request.assertions.is_empty()
        || request.required_assertion_order.is_empty()
        || request.required_source_order.is_empty()
        || !digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !ordered_nonempty(&request.required_assertion_order)
        || !ordered_nonempty(&request.required_source_order)
        || !ordered_nonempty(&request.adversarial_events)
    {
        return Err(EvidenceClosureError::Invalid(
            "evidence identity, requirements, digest, ordering, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut rows = request.assertions.clone();
    rows.sort_by(|a, b| a.assertion_id.cmp(&b.assertion_id));
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut supported = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut source_by_assertion = BTreeMap::new();
    let mut statement_digest_by_assertion = BTreeMap::new();
    let mut uncertainty = BTreeSet::new();
    let mut competing = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut digests = BTreeMap::new();
    for a in &rows {
        if !seen.insert(a.assertion_id.clone())
            || !nonempty(&a.assertion_id)
            || !nonempty(&a.source_id)
            || !nonempty(&a.statement)
            || !digest(&a.evidence_digest)
            || !a.local
            || !a.aggregate_only
        {
            return Err(EvidenceClosureError::Invalid(
                "assertion identity, source, evidence, or locality is invalid".into(),
            ));
        }
        order.push(a.assertion_id.clone());
        sources.insert(a.source_id.clone());
        source_by_assertion.insert(a.assertion_id.clone(), a.source_id.clone());
        statement_digest_by_assertion.insert(
            a.assertion_id.clone(),
            ContentHash::of_bytes(a.statement.as_bytes()),
        );
        uncertainty.insert(format!("{}:{}", a.assertion_id, a.uncertainty_milli));
        digests.insert(a.assertion_id.clone(), a.evidence_digest.clone());
        if a.competing_explanation {
            competing.insert(a.assertion_id.clone());
        }
        if a.negative_result {
            negative.insert(format!("{}:negative-result", a.assertion_id));
        }
        if a.policy_epoch == 0 {
            unknown.insert(a.assertion_id.clone());
        } else if !request.required_assertion_order.contains(&a.assertion_id)
            || !request.required_source_order.contains(&a.source_id)
        {
            rejected.insert(a.assertion_id.clone());
        } else if a.contradicted {
            contradicted.insert(a.assertion_id.clone());
        } else if a.evidence_digest == request.replay_identity {
            omitted.insert(a.assertion_id.clone());
        } else {
            supported.insert(a.assertion_id.clone());
        }
    }
    let global = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || request.action_count > request.action_budget;
    if global {
        omitted.extend(order.iter().cloned());
        supported.clear();
        contradicted.clear();
        unknown.clear();
        rejected.clear();
    }
    let missing = !request
        .required_assertion_order
        .iter()
        .all(|a| seen.contains(a))
        || !request
            .required_source_order
            .iter()
            .all(|s| sources.contains(s));
    let disposition = if global {
        "blocked"
    } else if missing {
        "unknown"
    } else if !contradicted.is_empty()
        || !unknown.is_empty()
        || !omitted.is_empty()
        || !rejected.is_empty()
    {
        "partial"
    } else {
        "qualified"
    };
    let mut payload = json!({"schema_version":OUTPUT_SCHEMA_VERSION,"contract_version":version,"feature_id":id,"mode":mode,"scale":scale,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"assertion_order":order,"source_by_assertion":source_by_assertion,"statement_digest_by_assertion":statement_digest_by_assertion,"supported_order":supported.into_iter().collect::<Vec<_>>(),"contradicted_order":contradicted.into_iter().collect::<Vec<_>>(),"unknown_order":unknown.into_iter().collect::<Vec<_>>(),"omitted_order":omitted.into_iter().collect::<Vec<_>>(),"rejected_order":rejected.into_iter().collect::<Vec<_>>(),"source_order":sources.into_iter().collect::<Vec<_>>(),"uncertainty_order":uncertainty.into_iter().collect::<Vec<_>>(),"competing_order":competing.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY});
    payload["artifact"] = json!({"artifact_id":format!("epistemic-evidence:{}",request.request_id),"content_type":CONTENT_TYPE,"semantic_loss":payload["omitted_order"],"assertion_digests":digests,"boundary":BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!("emit:evidence-closure:{}", request.request_id)]
    } else {
        vec!["block:unsupported-claim".to_string()]
    });
    let hash = content_digest(&payload)?;
    payload["closure_digest"] = json!(hash);
    payload["artifact"]["content_hash"] = json!(hash);
    let out: EvidenceClosureCard7 =
        serde_json::from_value(payload).map_err(|e| EvidenceClosureError::Output(e.to_string()))?;
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn assertion(assertion_id: &str, source_id: &str, statement: &str) -> EpistemicAssertion4 {
        EpistemicAssertion4 {
            assertion_id: assertion_id.into(),
            source_id: source_id.into(),
            statement: statement.into(),
            evidence_digest: hash(&format!("evidence:{assertion_id}")),
            uncertainty_milli: 125,
            policy_epoch: 1,
            competing_explanation: false,
            contradicted: false,
            negative_result: false,
            local: true,
            aggregate_only: true,
        }
    }

    fn request(assertions: Vec<EpistemicAssertion4>) -> EvidenceClosureRequest4 {
        EvidenceClosureRequest4 {
            request_id: "request:1".into(),
            purpose: "qualify a bounded evidence closure".into(),
            assertions,
            required_assertion_order: vec!["assertion-a".into()],
            required_source_order: vec!["source-a".into()],
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: hash("replay"),
            adversarial_events: Vec::new(),
            action_budget: 4,
            action_count: 1,
            boundary: BOUNDARY.into(),
        }
    }

    fn qualify_request(
        request: &EvidenceClosureRequest4,
    ) -> Result<EvidenceClosureCard7, EvidenceClosureError> {
        qualify(request, FEATURE_ID, CONTRACT_VERSION, "study", "local")
    }

    #[test]
    fn manifest_publishes_the_versioned_content_binding() {
        let manifest = manifest(FEATURE_ID, CONTRACT_VERSION, "study", "local");

        assert_eq!(manifest["output_schema"], "EvidenceClosureCard7@2");
        assert_eq!(
            manifest["content_binding"],
            "canonical card body excluding closure_digest and artifact.content_hash"
        );
    }

    #[test]
    fn qualified_card_binds_assertion_source_statement_and_evidence() {
        let card = qualify_request(&request(vec![assertion(
            "assertion-a",
            "source-a",
            "claim alpha",
        )]))
        .unwrap();

        assert_eq!(card.schema_version, OUTPUT_SCHEMA_VERSION);
        assert_eq!(card.disposition, "qualified");
        assert_eq!(
            card.source_by_assertion.get("assertion-a"),
            Some(&"source-a".to_string())
        );
        assert_eq!(
            card.statement_digest_by_assertion.get("assertion-a"),
            Some(&hash("claim alpha"))
        );
        assert_eq!(
            card.artifact.assertion_digests.get("assertion-a"),
            Some(&hash("evidence:assertion-a"))
        );
        card.validate().unwrap();

        let mut changed_source = card.clone();
        changed_source
            .source_by_assertion
            .insert("assertion-a".into(), "source-forged".into());
        assert!(changed_source.validate().is_err());

        let mut changed_evidence = card;
        changed_evidence
            .artifact
            .assertion_digests
            .insert("assertion-a".into(), hash("evidence-forged"));
        assert!(changed_evidence.validate().is_err());
    }

    #[test]
    fn out_of_scope_assertions_are_rejected_not_called_contradictions() {
        let mut extra = assertion("assertion-b", "source-b", "unrequested claim");
        extra.contradicted = true;
        let card = qualify_request(&request(vec![
            assertion("assertion-a", "source-a", "claim alpha"),
            extra,
        ]))
        .unwrap();

        assert_eq!(card.disposition, "partial");
        assert_eq!(card.rejected_order, ["assertion-b"]);
        assert!(card.contradicted_order.is_empty());
        card.validate().unwrap();
    }

    #[test]
    fn explicit_contradictions_remain_distinct_from_rejected_assertions() {
        let mut evidence = assertion("assertion-a", "source-a", "claim alpha");
        evidence.contradicted = true;
        let card = qualify_request(&request(vec![evidence])).unwrap();

        assert_eq!(card.disposition, "partial");
        assert_eq!(card.contradicted_order, ["assertion-a"]);
        assert!(card.rejected_order.is_empty());
        card.validate().unwrap();
    }

    #[test]
    fn policy_denial_blocks_and_accounts_for_every_assertion() {
        let mut request = request(vec![assertion("assertion-a", "source-a", "claim alpha")]);
        request.policy_allowed = false;

        let card = qualify_request(&request).unwrap();
        assert_eq!(card.disposition, "blocked");
        assert!(card.supported_order.is_empty());
        assert_eq!(card.omitted_order, card.assertion_order);
        assert_eq!(card.effect_receipts, ["block:unsupported-claim"]);
        card.validate().unwrap();
    }

    #[test]
    fn missing_required_evidence_remains_unknown() {
        let mut request = request(vec![assertion(
            "assertion-b",
            "source-b",
            "different claim",
        )]);
        let card = qualify_request(&request).unwrap();

        assert_eq!(card.disposition, "unknown");
        assert_eq!(card.rejected_order, ["assertion-b"]);
        assert!(card.supported_order.is_empty());
        card.validate().unwrap();

        request.required_assertion_order = vec![String::new()];
        assert!(matches!(
            qualify_request(&request),
            Err(EvidenceClosureError::Invalid(_))
        ));
    }

    #[test]
    fn input_assertion_order_does_not_change_the_canonical_card() {
        let first = qualify_request(&request(vec![
            assertion("assertion-a", "source-a", "claim alpha"),
            assertion("assertion-b", "source-b", "claim beta"),
        ]))
        .unwrap();
        let second = qualify_request(&request(vec![
            assertion("assertion-b", "source-b", "claim beta"),
            assertion("assertion-a", "source-a", "claim alpha"),
        ]))
        .unwrap();

        assert_eq!(first.assertion_order, second.assertion_order);
        assert_eq!(first.source_by_assertion, second.source_by_assertion);
        assert_eq!(first.closure_digest, second.closure_digest);
    }
}
