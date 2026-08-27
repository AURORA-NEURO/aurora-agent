//! Federated continual WeaveLang limitation-closure control plane.
//!
//! Atlas feature: `AFA-weavelang-P26-F32`.  This control plane evaluates
//! caller-attested limitation cases and peer closure summaries before an
//! institution-local capability may be operated or a digest-only summary may
//! cross a federation boundary.  It does not execute WeaveLang programs,
//! discover limitations, move raw data, or infer scientific conclusions.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyGrant, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, PolicyDecision, PolicyReceipt, ResearchSurface, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-weavelang-P26-F32";
pub const CONTRACT_VERSION: &str =
    "weavelang-federated-continual-limitation-closure-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "WeavelangLimitationCase4@1";
pub const OUTPUT_SCHEMA: &str = "WeavelangClosureReceipt8@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.weavelang-closure-receipt-8+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeavelangLimitationCase {
    pub case_id: String,
    pub limitation_id: String,
    pub capability_id: String,
    pub institution_id: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_result: bool,
    pub local_only: bool,
    pub permitted: bool,
    pub operator_attested: bool,
    pub resource_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerClosureSummary {
    pub institution_id: String,
    pub closure_digest: ContentHash,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeavelangClosureRequest {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_case_order: Vec<String>,
    pub cases: Vec<WeavelangLimitationCase>,
    pub peers: Vec<PeerClosureSummary>,
    pub minimum_peer_quorum: u16,
    pub replay_identity: ContentHash,
    pub autonomy_grant: AutonomyGrant,
    pub policy_receipt: PolicyReceipt,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeavelangClosureReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: ClosureDisposition,
    pub case_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_case_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub closure_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LimitationClosureError {
    #[error("invalid limitation closure request: {0}")]
    Invalid(String),
    #[error("limitation closure artifact failed: {0}")]
    Artifact(String),
}

fn invalid(value: impl Into<String>) -> LimitationClosureError {
    LimitationClosureError::Invalid(value.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl WeavelangClosureReceipt {
    pub fn validate(&self) -> Result<(), LimitationClosureError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.case_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "closure identity, locality, cases, peers, or effects are incomplete",
            ));
        }
        for values in [
            &self.case_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_case_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("closure ordering is not canonical"));
            }
        }
        let cases = self.case_order.iter().cloned().collect::<BTreeSet<_>>();
        let case_parts = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if case_parts.len() != cases.len()
            || case_parts.iter().cloned().collect::<BTreeSet<_>>() != cases
        {
            return Err(invalid("closure case states do not partition cases"));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_parts = self
            .qualified_peer_order
            .iter()
            .chain(self.missing_peer_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if peer_parts.len() != peers.len()
            || peer_parts.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(invalid("closure peer states do not partition peers"));
        }
        for value in [
            &self.replay_identity,
            &self.closure_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("closure digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| LimitationClosureError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("closure artifact type is invalid"));
        }
        if self.disposition == ClosureDisposition::Qualified
            && self.effect_receipts
                != [
                    format!("exchange:permitted-summaries:{}", self.request_id),
                    format!("manage:local-capability:{}", self.request_id),
                ]
        {
            return Err(invalid("qualified closure effects are invalid"));
        }
        if self.disposition != ClosureDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified closure must block release"));
        }
        Ok(())
    }
}

pub fn weavelang_limitation_closure_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "weavelang".into(),
        consumers: BTreeSet::from([
            String::from("research workflow operator"),
            String::from("institution node administrator"),
            String::from("federation governance board"),
        ]),
        behavior: "operates typed WeaveLang limitation-closure attestations and digest-only peer summaries under explicit A2 authority, budget, policy, provenance, replay, and federation gates without executing WeaveLang programs".into(),
        value: "prevents unresolved, unauthorized, semantically drifting, or over-budget limitation states from silently becoming an operated capability or federated release".into(),
        inputs: vec![TypedPort {
            name: "weavelang_limitation_case".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "weavelang_closure_receipt".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: BTreeSet::from([
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]),
        permissions: BTreeSet::from([String::from("operate:institution-node")]),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "opentelemetry".into(),
            state: EvidenceState::Supported,
            locator: Some("https://opentelemetry.io/docs/specs/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "research workflow operator".into(),
            reason: "A2 capability operation and digest-only federation export require explicit institutional authority".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_weavelang_limitation_closure(
    request: &WeavelangClosureRequest,
) -> Result<WeavelangClosureReceipt, LimitationClosureError> {
    validate_request(request)?;
    let mut cases = request.cases.clone();
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let case_order = cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_case_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let budget = request
        .autonomy_grant
        .resource_budget
        .get("research_units")
        .copied()
        .unwrap_or(0.0);
    for case in &cases {
        if case.negative_result {
            negative.insert(format!("{}:negative-result", case.case_id));
        }
        omissions.extend(
            case.omission_order
                .iter()
                .map(|item| format!("{}:{item}", case.case_id)),
        );
        uncertainty.extend(
            case.uncertainty_order
                .iter()
                .map(|item| format!("{}:{item}", case.case_id)),
        );
        if case.evidence_state == EvidenceState::Contradicted
            || !case.local_only
            || !case.permitted
            || !case.operator_attested
        {
            blocked.insert(case.case_id.clone());
        } else if case.resource_units as f64 > budget {
            uncertainty.insert(format!("{}:resource-budget-exceeded", case.case_id));
            unresolved.insert(case.case_id.clone());
        } else if case.semantic_profile != request.semantic_profile
            || case.replay_identity != request.replay_identity
            || !digest(&case.evidence_digest)
            || !digest(&case.provenance_digest)
            || !digest(&case.artifact_digest)
            || !case.omission_order.is_empty()
            || !case.uncertainty_order.is_empty()
            || !matches!(
                case.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(case.case_id.clone());
        } else {
            selected.insert(case.case_id.clone());
        }
    }
    let missing = required
        .difference(&case_order.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for case_id in &missing {
        omissions.insert(format!("{case_id}:required-case-missing"));
    }

    let peer_order = {
        let mut values = request
            .peers
            .iter()
            .map(|peer| peer.institution_id.clone())
            .collect::<Vec<_>>();
        values.sort();
        values
    };
    let mut qualified_peer = BTreeSet::new();
    let mut missing_peer = BTreeSet::new();
    for peer in &request.peers {
        if peer.signed
            && peer.permitted
            && peer.aggregate_only
            && peer.semantic_profile == request.semantic_profile
            && peer.replay_identity == request.replay_identity
            && digest(&peer.closure_digest)
        {
            qualified_peer.insert(peer.institution_id.clone());
        } else {
            missing_peer.insert(peer.institution_id.clone());
        }
    }
    if qualified_peer.len() < request.minimum_peer_quorum as usize {
        uncertainty.insert("request:peer-quorum-incomplete".into());
    }
    if request.policy_receipt.decision != PolicyDecision::Allow {
        negative.insert("request:policy-denied".into());
    }
    if request.autonomy_grant.revoked {
        negative.insert("request:autonomy-grant-revoked".into());
    }
    if request.autonomy_grant.approval_reference.is_none() {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = request.policy_receipt.decision != PolicyDecision::Allow
        || request.autonomy_grant.revoked
        || request.autonomy_grant.approval_reference.is_none()
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only
        || !request
            .autonomy_grant
            .permitted_actions
            .contains("manage:local-capability")
        || !request
            .autonomy_grant
            .permitted_actions
            .contains("exchange:permitted-summaries")
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(case_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:limitation-closure-release-gate-blocked".into());
    }
    let disposition = if global_block {
        ClosureDisposition::Blocked
    } else if required.is_subset(&selected)
        && missing.is_empty()
        && qualified_peer.len() >= request.minimum_peer_quorum as usize
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        ClosureDisposition::Qualified
    } else {
        ClosureDisposition::Unresolved
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_case_order = missing.into_iter().collect::<Vec<_>>();
    let qualified_peer_order = qualified_peer.into_iter().collect::<Vec<_>>();
    let missing_peer_order = missing_peer.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == ClosureDisposition::Qualified {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "case_order": case_order,
        "selected_order": selected_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_case_order": missing_case_order,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
        "replay_identity": request.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local,
        "aggregate_only": request.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let closure_digest = ContentHash::of_value(&payload)
        .map_err(|error| LimitationClosureError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("weavelang-closure:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| LimitationClosureError::Artifact(error.to_string()))?;
    let receipt = WeavelangClosureReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        case_order: payload["case_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_case_order: payload["missing_case_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        peer_order: payload["peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        qualified_peer_order: payload["qualified_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_peer_order: payload["missing_peer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        replay_identity: request.replay_identity.clone(),
        closure_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &WeavelangClosureRequest) -> Result<(), LimitationClosureError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_case_order.is_empty()
        || !canonical(&request.required_case_order)
        || request.cases.is_empty()
        || request.peers.is_empty()
        || request.minimum_peer_quorum == 0
        || request.minimum_peer_quorum as usize > request.peers.len()
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "request identity, requirements, peers, replay, locality, or boundary is invalid",
        ));
    }
    request
        .autonomy_grant
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    if request.autonomy_grant.autonomy_tier != AutonomyTier::A2 {
        return Err(invalid("limitation closure requires an A2 autonomy grant"));
    }
    request
        .policy_receipt
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    let mut ids = BTreeSet::new();
    for case in &request.cases {
        if case.case_id.trim().is_empty()
            || !ids.insert(case.case_id.clone())
            || case.limitation_id.trim().is_empty()
            || case.capability_id.trim().is_empty()
            || case.institution_id.trim().is_empty()
            || case.semantic_profile.trim().is_empty()
            || !digest(&case.evidence_digest)
            || !digest(&case.provenance_digest)
            || !digest(&case.artifact_digest)
            || !digest(&case.replay_identity)
            || !canonical(&case.omission_order)
            || !canonical(&case.uncertainty_order)
        {
            return Err(invalid(format!(
                "limitation case {} is malformed or duplicated",
                case.case_id
            )));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &request.peers {
        if peer.institution_id.trim().is_empty()
            || !peers.insert(peer.institution_id.clone())
            || !digest(&peer.closure_digest)
            || peer.semantic_profile.trim().is_empty()
            || !digest(&peer.replay_identity)
        {
            return Err(invalid(format!(
                "peer {} is malformed or duplicated",
                peer.institution_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request() -> WeavelangClosureRequest {
        let d = hash("limitation");
        let case = |id: &str| WeavelangLimitationCase {
            case_id: id.into(),
            limitation_id: format!("limitation:{id}"),
            capability_id: format!("capability:{id}"),
            institution_id: "inst:a".into(),
            semantic_profile: "weavelang-v1".into(),
            evidence_state: EvidenceState::Supported,
            evidence_digest: d.clone(),
            provenance_digest: d.clone(),
            artifact_digest: d.clone(),
            replay_identity: d.clone(),
            omission_order: vec![],
            uncertainty_order: vec![],
            negative_result: false,
            local_only: true,
            permitted: true,
            operator_attested: true,
            resource_units: 2,
        };
        let peer = |id: &str| PeerClosureSummary {
            institution_id: id.into(),
            closure_digest: d.clone(),
            semantic_profile: "weavelang-v1".into(),
            replay_identity: d.clone(),
            signed: true,
            permitted: true,
            aggregate_only: true,
        };
        let mut budget = BTreeMap::new();
        budget.insert("research_units".into(), 10.0);
        let grant = AutonomyGrant {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            actor: "operator:a".into(),
            permitted_actions: BTreeSet::from([
                "manage:local-capability".into(),
                "exchange:permitted-summaries".into(),
            ]),
            resource_budget: budget,
            scope: "institution:inst:a".into(),
            expires_at: "2027-01-01T00:00:00Z".into(),
            revoked: false,
            autonomy_tier: AutonomyTier::A2,
            approval_reference: Some("approval:a2".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        WeavelangClosureRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "closure:one".into(),
            federation_id: "fed:commons".into(),
            semantic_profile: "weavelang-v1".into(),
            required_case_order: vec!["case:a".into()],
            cases: vec![case("case:a")],
            peers: vec![peer("inst:a"), peer("inst:b")],
            minimum_peer_quorum: 2,
            replay_identity: d.clone(),
            autonomy_grant: grant,
            policy_receipt: PolicyReceipt {
                schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                receipt_id: "policy:closure".into(),
                decision: PolicyDecision::Allow,
                reasons: vec!["bounded research operation".into()],
                evaluated_artifacts: vec![d],
                authority_reference: None,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2_with_authority() {
        let manifest = weavelang_limitation_closure_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(!manifest.authority_requirements.is_empty());
    }

    #[test]
    fn qualified_closure_emits_two_effects() {
        let receipt = assure_weavelang_limitation_closure(&request()).unwrap();
        assert_eq!(receipt.disposition, ClosureDisposition::Qualified);
        assert_eq!(receipt.effect_receipts.len(), 2);
    }

    #[test]
    fn deterministic_closure() {
        let a = assure_weavelang_limitation_closure(&request()).unwrap();
        let b = assure_weavelang_limitation_closure(&request()).unwrap();
        assert_eq!(a.closure_digest, b.closure_digest);
    }

    #[test]
    fn unresolved_case_is_not_operated() {
        let mut value = request();
        value.cases[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            assure_weavelang_limitation_closure(&value)
                .unwrap()
                .disposition,
            ClosureDisposition::Unresolved
        );
    }

    #[test]
    fn revoked_grant_blocks() {
        let mut value = request();
        value.autonomy_grant.revoked = true;
        assert_eq!(
            assure_weavelang_limitation_closure(&value)
                .unwrap()
                .disposition,
            ClosureDisposition::Blocked
        );
    }

    #[test]
    fn peer_quorum_is_required() {
        let mut value = request();
        value.peers[1].signed = false;
        assert_eq!(
            assure_weavelang_limitation_closure(&value)
                .unwrap()
                .disposition,
            ClosureDisposition::Unresolved
        );
    }

    #[test]
    fn adversarial_event_blocks() {
        let mut value = request();
        value.adversarial_events.push("poisoned-closure".into());
        assert_eq!(
            assure_weavelang_limitation_closure(&value)
                .unwrap()
                .disposition,
            ClosureDisposition::Blocked
        );
    }
}
