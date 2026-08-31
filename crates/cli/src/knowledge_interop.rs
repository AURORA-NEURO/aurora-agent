//! CLI knowledge-representation interoperability gateway.
//!
//! Atlas feature: `AFA-cli-P04-F23`.
//!
//! The gateway is a local protocol boundary.  It accepts scoped research claims, verifies their
//! evidence and policy closure, and emits a typed knowledge-world artifact plus a replayable
//! receipt.  It never retrieves data, contacts an endpoint, executes an instrument, or makes a
//! clinical decision.  Only explicitly permitted artifact manifests may be exchanged.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-cli-P04-F23";
pub const CONTRACT_VERSION: &str = "cli-prospective-knowledge-representation-interoperability/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeDisposition {
    Passed,
    Conditional,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaim {
    pub claim_id: String,
    pub study_id: String,
    pub scope: String,
    pub subject_id: String,
    pub predicate: String,
    pub object: String,
    pub priority_milli: u32,
    pub state: ClaimState,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaims {
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub target_schema: String,
    pub study_order: Vec<String>,
    pub required_claim_ids: Vec<String>,
    pub claims: Vec<ScopedResearchClaim>,
    pub max_claims: usize,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub endpoint_allow: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorld {
    pub schema_version: String,
    pub world_id: String,
    pub target_schema: String,
    pub claim_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub subject_order: Vec<String>,
    pub predicate_order: Vec<String>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub world_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeInteroperabilityReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub disposition: KnowledgeDisposition,
    pub world: TypedKnowledgeWorld,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeInteropError {
    #[error("invalid scoped research claims: {0}")]
    Invalid(String),
    #[error("knowledge interoperability serialization failed: {0}")]
    Serialization(String),
}

impl TypedKnowledgeWorld {
    pub fn validate(&self) -> Result<(), KnowledgeInteropError> {
        if self.schema_version != SCHEMA_VERSION
            || self.world_id.trim().is_empty()
            || self.target_schema.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(KnowledgeInteropError::Invalid(
                "typed knowledge-world identity, schema, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.claim_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.subject_order,
            &self.predicate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(KnowledgeInteropError::Invalid(
                    "typed knowledge-world ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.evidence_order, &self.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(KnowledgeInteropError::Invalid(
                    "typed knowledge-world digest ordering is not canonical".into(),
                ));
            }
        }
        let claim_ids = self.claim_order.iter().collect::<BTreeSet<_>>();
        let mut classified_ids = BTreeSet::new();
        for values in [
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
        ] {
            for claim_id in values {
                if !claim_ids.contains(claim_id) {
                    if !claim_id.starts_with("request:") {
                        return Err(KnowledgeInteropError::Invalid(
                            "typed knowledge-world disposition is outside the claim set".into(),
                        ));
                    }
                    continue;
                }
                if !classified_ids.insert(claim_id) {
                    return Err(KnowledgeInteropError::Invalid(
                        "typed knowledge-world claim is classified more than once".into(),
                    ));
                }
            }
        }
        if classified_ids != claim_ids {
            return Err(KnowledgeInteropError::Invalid(
                "typed knowledge-world claims are not completely classified".into(),
            ));
        }
        let mut digest_payload = serde_json::to_value(self)
            .map_err(|error| KnowledgeInteropError::Serialization(error.to_string()))?;
        digest_payload
            .as_object_mut()
            .ok_or_else(|| {
                KnowledgeInteropError::Serialization(
                    "typed knowledge-world did not serialize as an object".into(),
                )
            })?
            .remove("world_digest");
        let expected_digest = ContentHash::of_value(&digest_payload)
            .map_err(|error| KnowledgeInteropError::Serialization(error.to_string()))?;
        if expected_digest != self.world_digest {
            return Err(KnowledgeInteropError::Invalid(
                "typed knowledge-world digest does not match its canonical payload".into(),
            ));
        }
        Ok(())
    }
}

impl KnowledgeInteroperabilityReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeInteropError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.world.boundary != PRECLINICAL_BOUNDARY
            || self.omissions != self.world.omissions
            || self.uncertainty != self.world.uncertainty
            || self.negative_evidence != self.world.negative_evidence
        {
            return Err(KnowledgeInteropError::Invalid(
                "knowledge interoperability identity, world linkage, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        self.world.validate()?;
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:")
                && effect != "block:knowledge-world-release"
        }) {
            return Err(KnowledgeInteropError::Invalid(
                "knowledge interoperability effect is outside the permitted-artifact boundary"
                    .into(),
            ));
        }
        if self
            .effect_receipts
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(KnowledgeInteropError::Invalid(
                "knowledge interoperability effects are not canonical".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, KnowledgeInteropError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| KnowledgeInteropError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| KnowledgeInteropError::Serialization(error.to_string()))
    }
}

#[derive(Debug)]
pub struct Verification {
    pub document: Value,
    pub disposition: KnowledgeDisposition,
    pub policy_denied: bool,
}

pub fn verify(request_value: &Value) -> Result<Verification, String> {
    let request: ScopedResearchClaims = serde_json::from_value(request_value.clone())
        .map_err(|error| format!("invalid scoped research claims request: {error}"))?;
    let policy_denied = !request.policy_allow || !request.endpoint_allow;
    let receipt = operate(&request).map_err(|error| error.to_string())?;
    let disposition = receipt.disposition;
    let receipt_digest = receipt.digest().map_err(|error| error.to_string())?;
    let receipt_value = serde_json::to_value(&receipt).map_err(|error| error.to_string())?;
    Ok(Verification {
        document: json!({
            "ok": disposition == KnowledgeDisposition::Passed,
            "feature_id": FEATURE_ID,
            "contract_version": CONTRACT_VERSION,
            "disposition": disposition,
            "receipt": receipt_value,
            "receipt_digest": receipt_digest,
            "execution": "verification-only; no endpoint, retrieval provider, instrument, or clinical effect was executed",
            "raw_data_local": true,
        }),
        disposition,
        policy_denied,
    })
}

pub fn operate(
    request: &ScopedResearchClaims,
) -> Result<KnowledgeInteroperabilityReceipt, KnowledgeInteropError> {
    validate_request(request)?;
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    let mut claim_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    claim_order.sort();
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut subjects = BTreeSet::new();
    let mut predicates = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let studies = request.study_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut spent = 0_u64;

    for claim in &claims {
        let cost = u64::try_from(claim.claim_id.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        if !studies.contains(&claim.study_id) || claim.scope != request.scope {
            blocked.insert(claim.claim_id.clone());
            omissions.insert(format!("claim:{}:scope-or-study-mismatch", claim.claim_id));
            continue;
        }
        if cost > request.budget.saturating_sub(spent) {
            blocked.insert(claim.claim_id.clone());
            omissions.insert(format!("claim:{}:budget-ceiling-exceeded", claim.claim_id));
            continue;
        }
        match claim.state {
            ClaimState::Contradicted => {
                blocked.insert(claim.claim_id.clone());
                negative.insert(format!(
                    "claim:{}:contradicted-knowledge-evidence",
                    claim.claim_id
                ));
                continue;
            }
            ClaimState::Unknown | ClaimState::Unmeasured => {
                unknown.insert(claim.claim_id.clone());
                uncertainty.insert(
                    format!(
                        "claim:{}:state-{:?}-not-admitted",
                        claim.claim_id, claim.state
                    )
                    .to_ascii_lowercase(),
                );
                continue;
            }
            ClaimState::Supported => {}
        }
        if !claim.omissions.is_empty() {
            unknown.insert(claim.claim_id.clone());
            omissions.extend(
                claim
                    .omissions
                    .iter()
                    .map(|item| format!("claim:{}:{item}", claim.claim_id)),
            );
            continue;
        }
        if !claim.uncertainty.is_empty() {
            unknown.insert(claim.claim_id.clone());
            uncertainty.extend(
                claim
                    .uncertainty
                    .iter()
                    .map(|item| format!("claim:{}:{item}", claim.claim_id)),
            );
            continue;
        }
        let (Some(evidence_digest), Some(provenance_digest)) = (
            claim.evidence_digest.clone(),
            claim.provenance_digest.clone(),
        ) else {
            unknown.insert(claim.claim_id.clone());
            omissions.insert(format!(
                "claim:{}:evidence-or-provenance-digest-missing",
                claim.claim_id
            ));
            continue;
        };
        if admitted.len() >= request.max_claims {
            blocked.insert(claim.claim_id.clone());
            omissions.insert(format!(
                "claim:{}:max-claims-admission-ceiling",
                claim.claim_id
            ));
            continue;
        }
        admitted.insert(claim.claim_id.clone());
        subjects.insert(claim.subject_id.clone());
        predicates.insert(claim.predicate.clone());
        evidence.insert(evidence_digest);
        provenance.insert(provenance_digest);
        spent = spent.saturating_add(cost);
        if claim.negative_result {
            negative.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
        }
    }

    for required in &request.required_claim_ids {
        if !admitted.contains(required) {
            omissions.insert(format!("claim:{}:required-but-not-admitted", required));
        }
    }
    if !request.endpoint_allow {
        blocked.insert("request:approved-endpoint-required".into());
        omissions.insert("request:approved-endpoint-required".into());
    }
    if !request.policy_allow {
        blocked.insert("request:policy-denied".into());
        negative.insert("request:policy-denied-no-artifact-exchange".into());
    }
    if !request.protected_closure {
        unknown.insert("request:protected-closure-incomplete".into());
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        blocked.insert("request:signed-approval-required".into());
        omissions.insert("request:signed-approval-required".into());
    }
    if !request.raw_data_local {
        blocked.insert("request:raw-data-locality-required".into());
        omissions.insert("request:raw-data-locality-required".into());
    }
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let subject_order = subjects.into_iter().collect::<Vec<_>>();
    let predicate_order = predicates.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let disposition = if !request.endpoint_allow
        || !request.policy_allow
        || !request.signed_approval
        || !request.raw_data_local
    {
        KnowledgeDisposition::Blocked
    } else if admitted_order.is_empty() {
        KnowledgeDisposition::Unknown
    } else if !blocked_order.is_empty()
        || !unknown_order.is_empty()
        || !omissions.is_empty()
        || !uncertainty.is_empty()
        || !request.protected_closure
    {
        KnowledgeDisposition::Conditional
    } else {
        KnowledgeDisposition::Passed
    };
    let world_id = format!("typed-knowledge-world:{}", request.request_id);
    let mut world_payload = json!({
        "schema_version": SCHEMA_VERSION,
        "world_id": world_id,
        "target_schema": request.target_schema,
        "claim_order": claim_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "subject_order": subject_order,
        "predicate_order": predicate_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let world_digest = ContentHash::of_value(&world_payload)
        .map_err(|error| KnowledgeInteropError::Serialization(error.to_string()))?;
    world_payload["world_digest"] = json!(world_digest);
    let world: TypedKnowledgeWorld = serde_json::from_value(world_payload)
        .map_err(|error| KnowledgeInteropError::Serialization(error.to_string()))?;
    let mut checks = vec![
        "claim identity and priority ordering are deterministic".to_string(),
        "scope, study, target-schema, and required-claim closure are explicit".to_string(),
        "evidence and provenance digests are required before artifact admission".to_string(),
        "unknown, unmeasured, contradicted, omitted, and negative claims remain visible".to_string(),
        "approved-endpoint, policy, protected-closure, signed-approval, locality, and budget gates fail closed".to_string(),
        "raw research payloads remain local; only permitted typed artifacts are exchanged".to_string(),
    ];
    checks.sort();
    let mut effect_receipts = world
        .admitted_order
        .iter()
        .map(|claim_id| format!("exchange:permitted-artifacts:{claim_id}"))
        .collect::<Vec<_>>();
    if disposition != KnowledgeDisposition::Passed {
        effect_receipts.push("block:knowledge-world-release".into());
    }
    effect_receipts.sort();
    let receipt = KnowledgeInteroperabilityReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        disposition,
        world,
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

fn validate_request(request: &ScopedResearchClaims) -> Result<(), KnowledgeInteropError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.target_schema.trim().is_empty()
        || request.study_order.is_empty()
        || request.claims.is_empty()
        || request.max_claims == 0
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .study_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .required_claim_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(KnowledgeInteropError::Invalid(
            "scoped research claims identity, schema, closure, budget, or boundary is incomplete"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for claim in &request.claims {
        if claim.claim_id.trim().is_empty()
            || claim.study_id.trim().is_empty()
            || claim.scope.trim().is_empty()
            || claim.subject_id.trim().is_empty()
            || claim.predicate.trim().is_empty()
            || claim.object.trim().is_empty()
            || claim.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(claim.claim_id.clone())
            || claim.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || claim.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(KnowledgeInteropError::Invalid(format!(
                "claim {} is invalid or duplicated",
                claim.claim_id
            )));
        }
    }
    if request
        .required_claim_ids
        .iter()
        .any(|id| !ids.contains(id))
    {
        return Err(KnowledgeInteropError::Invalid(
            "required claim closure references an unknown claim".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn claim(id: &str, state: ClaimState, negative_result: bool) -> ScopedResearchClaim {
        ScopedResearchClaim {
            claim_id: id.into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            subject_id: "subject:network".into(),
            predicate: "expresses".into(),
            object: "marker:gamma".into(),
            priority_milli: if id.ends_with('a') { 900 } else { 800 },
            state,
            evidence_digest: Some(hash(&format!("evidence:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            negative_result,
            omissions: vec![],
            uncertainty: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(claims: Vec<ScopedResearchClaim>) -> ScopedResearchClaims {
        ScopedResearchClaims {
            request_id: "knowledge:interop".into(),
            workflow_id: "workflow:knowledge".into(),
            scope: "organoid:neural".into(),
            target_schema: "typed-knowledge-world/6".into(),
            study_order: vec!["study:organoid".into()],
            required_claim_ids: vec!["claim:a".into(), "claim:b".into()],
            claims,
            max_claims: 8,
            replay_identity: hash("replay"),
            budget: 10_000,
            endpoint_allow: true,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn admits_scoped_claims_and_retains_negative_results() {
        let receipt = operate(&request(vec![
            claim("claim:a", ClaimState::Supported, false),
            claim("claim:b", ClaimState::Supported, true),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, KnowledgeDisposition::Passed);
        assert_eq!(receipt.world.admitted_order.len(), 2);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("negative-result")));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|item| item.starts_with("exchange:permitted-artifacts:")));
    }

    #[test]
    fn unknown_claim_is_not_released_as_knowledge() {
        let receipt = operate(&request(vec![
            claim("claim:a", ClaimState::Supported, false),
            claim("claim:b", ClaimState::Unknown, false),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, KnowledgeDisposition::Conditional);
        assert!(receipt.world.unknown_order.contains(&"claim:b".into()));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|item| item == "block:knowledge-world-release"));
    }

    #[test]
    fn contradiction_is_blocked_with_negative_evidence() {
        let receipt = operate(&request(vec![
            claim("claim:a", ClaimState::Supported, false),
            claim("claim:b", ClaimState::Contradicted, false),
        ]))
        .unwrap();
        assert!(receipt.world.blocked_order.contains(&"claim:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
        assert_ne!(receipt.disposition, KnowledgeDisposition::Passed);
    }

    #[test]
    fn endpoint_policy_denial_blocks_artifact_exchange() {
        let mut request = request(vec![
            claim("claim:a", ClaimState::Supported, false),
            claim("claim:b", ClaimState::Supported, false),
        ]);
        request.endpoint_allow = false;
        let receipt = operate(&request).unwrap();
        assert_eq!(receipt.disposition, KnowledgeDisposition::Blocked);
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|item| item == "block:knowledge-world-release"));
    }

    #[test]
    fn duplicate_claims_are_rejected() {
        let result = operate(&request(vec![
            claim("claim:a", ClaimState::Supported, false),
            claim("claim:a", ClaimState::Supported, false),
        ]));
        assert!(result.is_err());
    }

    #[test]
    fn tampered_world_digest_is_rejected() {
        let mut world = operate(&request(vec![
            claim("claim:a", ClaimState::Supported, false),
            claim("claim:b", ClaimState::Supported, false),
        ]))
        .unwrap()
        .world;
        world.world_digest = hash("tampered-world-digest");
        assert!(world.validate().is_err());
    }
}
