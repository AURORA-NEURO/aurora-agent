//! Versioned knowledge-representation federation gateway.
//!
//! Atlas feature: `AFA-docgraph-P04-F24`.
//! The gateway exchanges only permitted, content-addressed research artifacts.
//! It preserves scope, provenance, omissions, uncertainty, and contradictions;
//! it never turns an incomplete knowledge world into a confident conclusion.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-docgraph-P04-F24";
pub const CONTRACT_VERSION: &str = "docgraph-knowledge-gateway/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    Supported,
    Unknown,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaim {
    pub claim_id: String,
    pub study_id: String,
    pub scope: String,
    pub statement_digest: ContentHash,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_digest: ContentHash,
    pub state: ClaimState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedResearchClaims {
    pub request_id: String,
    pub federation_id: String,
    pub required_scope: String,
    pub target_schema: String,
    pub claims: Vec<ScopedResearchClaim>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub max_concurrency: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub endpoint_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedKnowledgeWorld {
    pub world_id: String,
    pub scope: String,
    pub target_schema: String,
    pub claim_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub world_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeGatewayDisposition {
    Shared,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub disposition: KnowledgeGatewayDisposition,
    pub world: TypedKnowledgeWorld,
    pub replay_identity: ContentHash,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeGatewayError {
    #[error("invalid knowledge gateway request: {0}")]
    Invalid(String),
    #[error("knowledge gateway serialization failed: {0}")]
    Serialization(String),
}

impl KnowledgeGatewayReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeGatewayError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.effect_receipts.is_empty()
            || self.checks.is_empty()
            || self.world.boundary != PRECLINICAL_BOUNDARY
            || (self.world.claim_order.is_empty()
                && self.world.omissions.is_empty()
                && self.world.uncertainty.is_empty()
                && self.world.negative_evidence.is_empty())
        {
            return Err(KnowledgeGatewayError::Invalid(
                "gateway identity, typed world, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.world.claim_order,
            &self.world.omissions,
            &self.world.uncertainty,
            &self.world.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(KnowledgeGatewayError::Invalid(
                    "knowledge gateway ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.world.artifact_order,
            &self.world.evidence_order,
            &self.world.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(KnowledgeGatewayError::Invalid(
                    "knowledge gateway digest ordering is not canonical".into(),
                ));
            }
        }
        if self.world.target_schema.trim().is_empty()
            || self.world.scope.trim().is_empty()
            || self.world.world_id.trim().is_empty()
        {
            return Err(KnowledgeGatewayError::Invalid(
                "typed knowledge world identity and schema are incomplete".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, KnowledgeGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| KnowledgeGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| KnowledgeGatewayError::Serialization(error.to_string()))
    }
}

pub fn operate_knowledge_gateway(
    request: &ScopedResearchClaims,
) -> Result<KnowledgeGatewayReceipt, KnowledgeGatewayError> {
    validate_request(request)?;
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for claim in &claims {
        let gate = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.endpoint_allow
            && claim.state == ClaimState::Supported
            && claim.scope == request.required_scope
            && !claim.evidence_order.is_empty()
            && claim.omissions.is_empty()
            && claim.uncertainty.is_empty();
        if gate {
            admitted.insert(claim.claim_id.clone());
        } else {
            blocked.insert(claim.claim_id.clone());
            if claim.state != ClaimState::Supported {
                negative.insert(
                    format!(
                        "claim:{}:state-{:?}-cannot-share",
                        claim.claim_id, claim.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if claim.scope != request.required_scope {
                omissions.insert(format!("claim:{}:scope-mismatch", claim.claim_id));
            }
            if claim.evidence_order.is_empty()
                || !claim.omissions.is_empty()
                || !claim.uncertainty.is_empty()
            {
                omissions.insert(format!(
                    "claim:{}:protected-closure-or-evidence-incomplete",
                    claim.claim_id
                ));
            }
        }
    }
    if !request.policy_allow || !request.endpoint_allow {
        negative.insert("request:policy-or-endpoint-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    let claim_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition =
        if !request.policy_allow || !request.endpoint_allow || !request.signed_approval {
            KnowledgeGatewayDisposition::Blocked
        } else if !request.protected_closure || claim_order.is_empty() {
            KnowledgeGatewayDisposition::Unknown
        } else if blocked_order.is_empty() {
            KnowledgeGatewayDisposition::Shared
        } else {
            KnowledgeGatewayDisposition::Partial
        };
    let admitted_set = claim_order.iter().collect::<BTreeSet<_>>();
    let admitted_claims = claims
        .iter()
        .filter(|claim| admitted_set.contains(&claim.claim_id))
        .collect::<Vec<_>>();
    let mut artifact_order = admitted_claims
        .iter()
        .map(|claim| claim.statement_digest.clone())
        .collect::<Vec<_>>();
    artifact_order.sort();
    let mut evidence_order = admitted_claims
        .iter()
        .flat_map(|claim| claim.evidence_order.clone())
        .collect::<Vec<_>>();
    evidence_order.sort();
    evidence_order.dedup();
    let mut provenance_order = admitted_claims
        .iter()
        .map(|claim| claim.provenance_digest.clone())
        .collect::<Vec<_>>();
    provenance_order.sort();
    let world_omissions = omissions.into_iter().collect::<Vec<_>>();
    let world_uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let world_negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let world_payload = json!({
        "world_id": format!("knowledge-world:{}", request.request_id),
        "scope": request.required_scope,
        "target_schema": request.target_schema,
        "claim_order": claim_order,
        "artifact_order": artifact_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "omissions": world_omissions,
        "uncertainty": world_uncertainty,
        "negative_evidence": world_negative_evidence,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let world_digest = ContentHash::of_value(&world_payload)
        .map_err(|error| KnowledgeGatewayError::Serialization(error.to_string()))?;
    let world = TypedKnowledgeWorld {
        world_id: format!("knowledge-world:{}", request.request_id),
        scope: request.required_scope.clone(),
        target_schema: request.target_schema.clone(),
        claim_order,
        artifact_order,
        evidence_order,
        provenance_order,
        omissions: world_omissions,
        uncertainty: world_uncertainty,
        negative_evidence: world_negative_evidence,
        world_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let mut checks = vec![
        "scope, schema, evidence, provenance, policy, endpoint, authority, and protected-closure gates are explicit".into(),
        "only permitted content-addressed research artifacts cross federation boundaries".into(),
        "unknown and contradicted claims remain visible and cannot become shared conclusions".into(),
    ];
    checks.sort();
    let mut effect_receipts = if matches!(
        disposition,
        KnowledgeGatewayDisposition::Shared | KnowledgeGatewayDisposition::Partial
    ) {
        world
            .claim_order
            .iter()
            .map(|claim_id| format!("exchange:permitted-artifact:{claim_id}"))
            .collect::<Vec<_>>()
    } else {
        vec![format!("block:knowledge-gateway:{disposition:?}").to_ascii_lowercase()]
    };
    effect_receipts.sort();
    let receipt_omissions = world.omissions.clone();
    let receipt_uncertainty = world.uncertainty.clone();
    let receipt_negative_evidence = world.negative_evidence.clone();
    let receipt = KnowledgeGatewayReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        disposition,
        world,
        replay_identity: request.replay_identity.clone(),
        checks,
        omissions: receipt_omissions,
        uncertainty: receipt_uncertainty,
        negative_evidence: receipt_negative_evidence,
        effect_receipts,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ScopedResearchClaims) -> Result<(), KnowledgeGatewayError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.required_scope.trim().is_empty()
        || request.target_schema.trim().is_empty()
        || request.claims.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.max_concurrency == 0
    {
        return Err(KnowledgeGatewayError::Invalid(
            "gateway identity, scope, schema, claims, concurrency, locality, or boundary is required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for claim in &request.claims {
        if claim.claim_id.trim().is_empty()
            || claim.study_id.trim().is_empty()
            || claim.scope.trim().is_empty()
            || !ids.insert(claim.claim_id.clone())
            || claim.boundary != PRECLINICAL_BOUNDARY
            || claim
                .evidence_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(KnowledgeGatewayError::Invalid(format!(
                "claim {} is invalid or duplicated",
                claim.claim_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn claim(id: &str, state: ClaimState) -> ScopedResearchClaim {
        ScopedResearchClaim {
            claim_id: id.into(),
            study_id: format!("study:{id}"),
            scope: "organoid:neural".into(),
            statement_digest: hash(&format!("statement:{id}")),
            evidence_order: vec![hash(&format!("evidence:{id}"))],
            provenance_digest: hash(&format!("provenance:{id}")),
            state,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request() -> ScopedResearchClaims {
        ScopedResearchClaims {
            request_id: "gateway:knowledge".into(),
            federation_id: "federation:commons".into(),
            required_scope: "organoid:neural".into(),
            target_schema: "typed-knowledge-world/6".into(),
            claims: vec![claim("claim:a", ClaimState::Supported)],
            replay_identity: hash("replay"),
            budget: 10,
            max_concurrency: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            endpoint_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn supported_claims_share_typed_world_artifacts() {
        let receipt = operate_knowledge_gateway(&request()).unwrap();
        assert_eq!(receipt.disposition, KnowledgeGatewayDisposition::Shared);
        assert!(receipt.effect_receipts[0].starts_with("exchange:permitted-artifact"));
        assert!(receipt.digest().is_ok());
    }

    #[test]
    fn unknown_claim_remains_blocked_with_negative_evidence() {
        let mut request = request();
        request.claims[0].state = ClaimState::Unknown;
        let receipt = operate_knowledge_gateway(&request).unwrap();
        assert_eq!(receipt.disposition, KnowledgeGatewayDisposition::Unknown);
        assert!(!receipt.world.negative_evidence.is_empty());
    }

    #[test]
    fn endpoint_denial_blocks_without_exchange_effect() {
        let mut request = request();
        request.endpoint_allow = false;
        let receipt = operate_knowledge_gateway(&request).unwrap();
        assert_eq!(receipt.disposition, KnowledgeGatewayDisposition::Blocked);
        assert!(receipt.effect_receipts[0].contains("block:knowledge-gateway"));
    }

    #[test]
    fn protected_closure_gap_is_unknown() {
        let mut request = request();
        request.protected_closure = false;
        let receipt = operate_knowledge_gateway(&request).unwrap();
        assert_eq!(receipt.disposition, KnowledgeGatewayDisposition::Unknown);
        assert!(!receipt.world.uncertainty.is_empty());
    }
}
