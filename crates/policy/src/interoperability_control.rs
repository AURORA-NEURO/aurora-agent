//! Federated continual interoperability and extensibility control plane.
//!
//! Atlas feature: `AFA-policy-P22-F32`.
//!
//! This policy-owned surface negotiates external capability offers without treating a compatible
//! schema as permission to execute.  Offers are admitted only when policy, purpose, residency,
//! provenance, migration, replay, approval, and federation gates close; unknown and contradicted
//! offers remain explicit and only digest-bearing summaries cross the federation boundary.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-policy-P22-F32";
pub const CONTRACT_VERSION: &str = "policy-federated-interoperability-control-plane/1.0";
pub const MAX_OFFERS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfferEvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalCapabilityOffer {
    pub offer_id: String,
    pub capability_id: String,
    pub origin_institution: String,
    pub scope: String,
    pub schema_version: String,
    pub contract_version: String,
    pub input_digest: ContentHash,
    pub output_digest: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub evidence_digest: Option<ContentHash>,
    pub migration_digest: Option<ContentHash>,
    pub effects: Vec<String>,
    pub state: OfferEvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteroperabilityControlRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub scope: String,
    pub target_schema_version: String,
    pub required_capability_ids: Vec<String>,
    pub offers: Vec<ExternalCapabilityOffer>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub budget: u64,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteroperabilityControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub disposition: IntegrationDisposition,
    pub offer_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub schema_order: Vec<String>,
    pub input_order: Vec<ContentHash>,
    pub output_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub migration_order: Vec<ContentHash>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub integration_artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InteroperabilityControlError {
    #[error("invalid interoperability control request: {0}")]
    Invalid(String),
    #[error("interoperability control artifact failed: {0}")]
    Artifact(String),
    #[error("interoperability control serialization failed: {0}")]
    Serialization(String),
}

impl InteroperabilityControlReceipt {
    pub fn validate(&self) -> Result<(), InteroperabilityControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.offer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(InteroperabilityControlError::Invalid(
                "interoperability identity, offers, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.offer_order,
            &self.accepted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.capability_order,
            &self.schema_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InteroperabilityControlError::Invalid(
                    "interoperability ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.input_order,
            &self.output_order,
            &self.provenance_order,
            &self.evidence_order,
            &self.migration_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InteroperabilityControlError::Invalid(
                    "interoperability digest ordering is not canonical".into(),
                ));
            }
        }
        if self
            .accepted_order
            .iter()
            .any(|id| !self.offer_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.offer_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.offer_order.contains(id))
        {
            return Err(InteroperabilityControlError::Invalid(
                "interoperability state order is not covered by offer order".into(),
            ));
        }
        self.integration_artifact
            .validate_metadata()
            .map_err(|error| InteroperabilityControlError::Artifact(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, InteroperabilityControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InteroperabilityControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InteroperabilityControlError::Serialization(error.to_string()))
    }
}

pub fn interoperability_control_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: "0.1.0".into(),
        owner_crate: "policy".into(),
        consumers: [
            "research data steward".into(),
            "institution node operator".into(),
            "extension developer".into(),
        ]
        .into(),
        behavior: "negotiates external capability offers with explicit schema migration, policy, provenance, autonomy, and digest-only federation gates".into(),
        value: "lets policy-separated institutions interoperate without treating version compatibility as authority or exporting protected source data".into(),
        inputs: vec![TypedPort {
            name: "interoperability_control_request".into(),
            schema: "InteroperabilityControlRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "interoperability_control_receipt".into(),
            schema: "InteroperabilityControlReceipt@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: [
            "operate:institution-node".into(),
            "manage:local-capability".into(),
            "exchange:permitted-summaries".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification/1.3/".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "federation policy steward".into(),
            reason: "approve capability integration and permitted summary exchange".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn negotiate_interoperability(
    request: &InteroperabilityControlRequest,
) -> Result<InteroperabilityControlReceipt, InteroperabilityControlError> {
    validate_request(request)?;
    let mut offers = request.offers.clone();
    offers.sort_by(|left, right| left.offer_id.cmp(&right.offer_id));
    let required = request
        .required_capability_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut offer_ids = BTreeSet::new();
    let mut accepted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut capabilities = BTreeSet::new();
    let mut schemas = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for offer in &offers {
        offer_ids.insert(offer.offer_id.clone());
        let cost = offer.offer_id.len() as u64 + offer.capability_id.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = offer.scope == request.scope
            && offer.schema_version == request.target_schema_version
            && offer.provenance_digest.is_some()
            && offer.evidence_digest.is_some()
            && offer.migration_digest.is_some()
            && offer.omissions.is_empty()
            && offer.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.federation_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && offer.state == OfferEvidenceState::Supported
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            accepted.insert(offer.offer_id.clone());
            capabilities.insert(offer.capability_id.clone());
            schemas.insert(offer.schema_version.clone());
            inputs.insert(offer.input_digest.clone());
            outputs.insert(offer.output_digest.clone());
            provenance.insert(
                offer
                    .provenance_digest
                    .clone()
                    .expect("complete provenance"),
            );
            evidence.insert(offer.evidence_digest.clone().expect("complete evidence"));
            migration.insert(offer.migration_digest.clone().expect("complete migration"));
        } else {
            match offer.state {
                OfferEvidenceState::Unknown | OfferEvidenceState::Unmeasured => {
                    unknown.insert(offer.offer_id.clone());
                    uncertainty.insert(
                        format!(
                            "offer:{}:state-{:?}-not-admitted",
                            offer.offer_id, offer.state
                        )
                        .to_ascii_lowercase(),
                    );
                }
                OfferEvidenceState::Contradicted => {
                    blocked.insert(offer.offer_id.clone());
                    negative.insert(format!(
                        "offer:{}:contradicted-capability-retained",
                        offer.offer_id
                    ));
                }
                OfferEvidenceState::Supported => {
                    blocked.insert(offer.offer_id.clone());
                }
            }
            if offer.scope != request.scope {
                omissions.insert(format!("offer:{}:scope-mismatch", offer.offer_id));
            }
            if offer.schema_version != request.target_schema_version {
                omissions.insert(format!(
                    "offer:{}:schema-migration-required",
                    offer.offer_id
                ));
            }
            if offer.provenance_digest.is_none() || offer.evidence_digest.is_none() {
                omissions.insert(format!(
                    "offer:{}:provenance-or-evidence-missing",
                    offer.offer_id
                ));
            }
            if offer.migration_digest.is_none() {
                omissions.insert(format!(
                    "offer:{}:migration-receipt-missing",
                    offer.offer_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("offer:{}:budget-ceiling-exceeded", offer.offer_id));
            }
            if !offer.omissions.is_empty() {
                omissions.extend(
                    offer
                        .omissions
                        .iter()
                        .map(|value| format!("offer:{}:{value}", offer.offer_id)),
                );
            }
            if !offer.uncertainty.is_empty() {
                uncertainty.extend(
                    offer
                        .uncertainty
                        .iter()
                        .map(|value| format!("offer:{}:{value}", offer.offer_id)),
                );
            }
        }
    }
    for capability_id in required {
        if !capabilities.contains(&capability_id) {
            omissions.insert(format!(
                "capability:{capability_id}:required-but-not-admitted"
            ));
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let offer_order = offer_ids.into_iter().collect::<Vec<_>>();
    let accepted_order = accepted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let capability_order = capabilities.into_iter().collect::<Vec<_>>();
    let schema_order = schemas.into_iter().collect::<Vec<_>>();
    let input_order = inputs.into_iter().collect::<Vec<_>>();
    let output_order = outputs.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let migration_order = migration.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let hard_block = !request.policy_allow
        || !request.federation_allow
        || !request.signed_approval
        || !request.raw_data_local;
    let disposition = if hard_block {
        IntegrationDisposition::Blocked
    } else if accepted_order.is_empty() {
        IntegrationDisposition::Unknown
    } else if blocked_order.is_empty()
        && unknown_order.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
    {
        IntegrationDisposition::Qualified
    } else {
        IntegrationDisposition::Partial
    };
    let payload = json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "disposition": disposition,
        "offer_order": offer_order,
        "accepted_order": accepted_order,
        "capability_order": capability_order,
        "schema_order": schema_order,
        "input_order": input_order,
        "output_order": output_order,
        "provenance_order": provenance_order,
        "evidence_order": evidence_order,
        "migration_order": migration_order,
        "replay_identity": request.replay_identity,
        "benchmark_digest": request.benchmark_digest,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let integration_artifact = TypedResearchArtifact::from_payload(
        format!("negotiated-integration:{}", request.request_id),
        "application/vnd.aurora.negotiated-integration+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|error| InteroperabilityControlError::Artifact(error.to_string()))?;
    let mut effect_receipts = if !accepted_order.is_empty()
        && request.policy_allow
        && request.federation_allow
        && request.raw_data_local
    {
        vec![format!(
            "exchange:permitted-capability-summary:{}",
            request.request_id
        )]
    } else {
        Vec::new()
    };
    if disposition != IntegrationDisposition::Qualified {
        effect_receipts.push(format!(
            "block:policy-interoperability-release:{}",
            request.request_id
        ));
    }
    effect_receipts.sort();
    let receipt = InteroperabilityControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        disposition,
        offer_order,
        accepted_order,
        blocked_order,
        unknown_order,
        capability_order,
        schema_order,
        input_order,
        output_order,
        provenance_order,
        evidence_order,
        migration_order,
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        integration_artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &InteroperabilityControlRequest,
) -> Result<(), InteroperabilityControlError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.target_schema_version.trim().is_empty()
        || request.required_capability_ids.is_empty()
        || request.offers.is_empty()
        || request.offers.len() > MAX_OFFERS
        || request
            .required_capability_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InteroperabilityControlError::Invalid(
            "interoperability identity, scope, target schema, required capabilities, offers, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for offer in &request.offers {
        if offer.offer_id.trim().is_empty()
            || offer.capability_id.trim().is_empty()
            || offer.origin_institution.trim().is_empty()
            || offer.scope.trim().is_empty()
            || offer.schema_version.trim().is_empty()
            || offer.contract_version.trim().is_empty()
            || offer.effects.is_empty()
            || offer.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(offer.offer_id.clone())
            || offer.effects.windows(2).any(|pair| pair[0] >= pair[1])
            || offer.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || offer.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(InteroperabilityControlError::Invalid(format!(
                "capability offer {} is invalid or duplicated",
                offer.offer_id
            )));
        }
    }
    if request.required_capability_ids.iter().any(|id| {
        !request
            .offers
            .iter()
            .any(|offer| &offer.capability_id == id)
    }) {
        return Err(InteroperabilityControlError::Invalid(
            "required capability closure references an unknown offer".into(),
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

    fn offer(id: &str, state: OfferEvidenceState) -> ExternalCapabilityOffer {
        ExternalCapabilityOffer {
            offer_id: id.into(),
            capability_id: format!("capability:{id}"),
            origin_institution: "site:alpha".into(),
            scope: "organoid:neural".into(),
            schema_version: "research-capability/2".into(),
            contract_version: "1.0.0".into(),
            input_digest: hash(&format!("input:{id}")),
            output_digest: hash(&format!("output:{id}")),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            evidence_digest: Some(hash(&format!("evidence:{id}"))),
            migration_digest: Some(hash(&format!("migration:{id}"))),
            effects: vec!["read:local".into()],
            state,
            omissions: vec![],
            uncertainty: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(offers: Vec<ExternalCapabilityOffer>) -> InteroperabilityControlRequest {
        InteroperabilityControlRequest {
            request_id: "request:interop".into(),
            workflow_id: "workflow:interop".into(),
            federation_id: "federation:commons".into(),
            scope: "organoid:neural".into(),
            target_schema_version: "research-capability/2".into(),
            required_capability_ids: vec!["capability:offer:a".into()],
            offers,
            replay_identity: hash("replay"),
            benchmark_digest: hash("benchmark"),
            policy_allow: true,
            federation_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            budget: 200,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_declares_a2_byte_stable_interoperability() {
        let manifest = interoperability_control_manifest();
        assert_eq!(manifest.capability_id, FEATURE_ID);
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
    }

    #[test]
    fn negotiates_supported_capabilities_deterministically() {
        let receipt = negotiate_interoperability(&request(vec![
            offer("offer:b", OfferEvidenceState::Supported),
            offer("offer:a", OfferEvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, IntegrationDisposition::Qualified);
        assert_eq!(receipt.accepted_order, vec!["offer:a", "offer:b"]);
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("exchange:")));
    }

    #[test]
    fn unknown_offer_preserves_migration_omission() {
        let mut unknown = offer("offer:a", OfferEvidenceState::Unknown);
        unknown.migration_digest = None;
        let receipt = negotiate_interoperability(&request(vec![unknown])).unwrap();
        assert_eq!(receipt.disposition, IntegrationDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("migration")));
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown")));
    }

    #[test]
    fn schema_mismatch_is_explicit_not_silently_admitted() {
        let mut mismatch = offer("offer:a", OfferEvidenceState::Supported);
        mismatch.schema_version = "research-capability/1".into();
        let receipt = negotiate_interoperability(&request(vec![mismatch])).unwrap();
        assert_eq!(receipt.disposition, IntegrationDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("schema-migration")));
    }

    #[test]
    fn policy_denial_blocks_summary_exchange() {
        let mut input = request(vec![offer("offer:a", OfferEvidenceState::Supported)]);
        input.policy_allow = false;
        let receipt = negotiate_interoperability(&input).unwrap();
        assert_eq!(receipt.disposition, IntegrationDisposition::Blocked);
        assert!(!receipt
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("exchange:")));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("policy")));
    }

    #[test]
    fn duplicate_offers_are_rejected() {
        let result = negotiate_interoperability(&request(vec![
            offer("offer:a", OfferEvidenceState::Supported),
            offer("offer:a", OfferEvidenceState::Supported),
        ]));
        assert!(result.is_err());
    }
}
