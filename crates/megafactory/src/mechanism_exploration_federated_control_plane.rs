//! Federated continual mechanism-exploration operations and federation control.
//!
//! The control plane ranks caller-supplied mechanism attestations and governs digest-only
//! exchange. It does not manufacture a mechanism, move factor tables, or turn an uncertain
//! influence estimate into a scientific conclusion.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-megafactory-P08-F32";
pub const FEATURE_VERSION: &str = "megafactory-federated-continual-mechanism-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "MechanismQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "MechanismPortfolio8@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub mechanism_id: String,
    pub origin: String,
    pub scope: String,
    pub semantic_profile: String,
    pub support_score_milli: i64,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub freshness_seq: u64,
    pub omission_count: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub oracle_verified: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismControlRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_origin_quorum: u32,
    pub capacity: u32,
    pub active_runs: u32,
    pub checkpoint_seq: u64,
    pub candidates: Vec<MechanismCandidate>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub network_permitted: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismDecision {
    pub mechanism_id: String,
    pub origin: String,
    pub support_score_milli: i64,
    pub disposition: String,
    pub failed_gates: Vec<String>,
    pub conditional_gates: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedMechanismAdmission {
    Admitted,
    ApprovalRequired,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedMechanismReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub admission: FederatedMechanismAdmission,
    pub origin_order: Vec<String>,
    pub admitted_origin_order: Vec<String>,
    pub mechanism_order: Vec<String>,
    pub rank_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub decisions: Vec<MechanismDecision>,
    pub checkpoint_seq: u64,
    pub checkpoint_digest: ContentHash,
    pub control_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedMechanismError {
    #[error("invalid federated mechanism control request: {0}")]
    Invalid(String),
    #[error("federated mechanism artifact failed: {0}")]
    Artifact(String),
}

impl FederatedMechanismReceipt {
    pub fn validate(&self) -> Result<(), FederatedMechanismError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != FEATURE_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.mechanism_order.is_empty()
            || self.decisions.len() != self.mechanism_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedMechanismError::Invalid(
                "identity, locality, candidates, decisions, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.origin_order,
            &self.admitted_origin_order,
            &self.mechanism_order,
            &self.admitted_order,
            &self.conditional_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|window| window[0] >= window[1]) {
                return Err(FederatedMechanismError::Invalid(
                    "federated mechanism ordering is not canonical".into(),
                ));
            }
        }
        if self.rank_order.len() != self.mechanism_order.len()
            || BTreeSet::from_iter(self.rank_order.iter().cloned())
                != BTreeSet::from_iter(self.mechanism_order.iter().cloned())
            || self
                .decisions
                .iter()
                .zip(&self.mechanism_order)
                .any(|(decision, id)| &decision.mechanism_id != id)
        {
            return Err(FederatedMechanismError::Invalid(
                "rank order or decisions do not match candidates".into(),
            ));
        }
        let classified = self
            .admitted_order
            .iter()
            .chain(&self.conditional_order)
            .chain(&self.blocked_order)
            .chain(&self.unknown_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != BTreeSet::from_iter(self.mechanism_order.iter().cloned()) {
            return Err(FederatedMechanismError::Invalid(
                "mechanism dispositions do not partition candidates".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-summaries:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedMechanismError::Invalid(
                "mechanism effect is outside the governed gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedMechanismError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedMechanismError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| FederatedMechanismError::Artifact(error.to_string()))?,
        )
        .map_err(|error| FederatedMechanismError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "megafactory".into(),
        consumers: BTreeSet::from([
            "formal methods researcher".into(),
            "federated research operator".into(),
        ]),
        behavior: "operates and governs digest-only federated mechanism attestations under continual A2 gates".into(),
        value: "unlocks megafactory consortium mechanism exploration without treating unknown influence or failed safety gates as qualified".into(),
        inputs: vec![TypedPort { name: "mechanism_question".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "mechanism_portfolio".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport]),
        permissions: BTreeSet::from(["operate:institution-node".into(), "exchange:permitted-summaries".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federation-steward".into(), reason: "signed mechanism-summary exchange".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: BTreeSet::from([ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Policy, ResearchSurface::Operator]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate(
    request: &FederatedMechanismControlRequest,
) -> Result<FederatedMechanismReceipt, FederatedMechanismError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_origin_quorum == 0
        || request.capacity == 0
        || request.active_runs > request.capacity
        || request.checkpoint_seq == 0
        || request.candidates.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedMechanismError::Invalid(
            "identity, quorum, capacity, checkpoint, candidates, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mechanism_order = candidates
        .iter()
        .map(|candidate| candidate.mechanism_id.clone())
        .collect::<Vec<_>>();
    if mechanism_order.iter().any(|id| id.trim().is_empty())
        || mechanism_order
            .windows(2)
            .any(|window| window[0] == window[1])
    {
        return Err(FederatedMechanismError::Invalid(
            "mechanism identifiers must be unique and non-empty".into(),
        ));
    }
    let origins = candidates
        .iter()
        .map(|candidate| candidate.origin.clone())
        .collect::<BTreeSet<_>>();
    if origins.iter().any(|origin| origin.trim().is_empty())
        || origins.len() < request.required_origin_quorum as usize
    {
        return Err(FederatedMechanismError::Invalid(
            "declared origin quorum is not available".into(),
        ));
    }
    let mut global_failed = BTreeSet::new();
    for (gate, failed) in [
        ("policy", !request.policy_allow),
        ("protected-closure", !request.protected_closure),
        ("signed-approval", !request.signed_approval),
        ("network-permission", !request.network_permitted),
        (
            "origin-quorum",
            origins.len() < request.required_origin_quorum as usize,
        ),
    ] {
        if failed {
            global_failed.insert(gate.to_string());
        }
    }
    let mut admitted = Vec::new();
    let mut conditional = Vec::new();
    let mut blocked = Vec::new();
    let unknown = Vec::new();
    let mut decisions = Vec::new();
    let mut semantic_loss = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut scores = std::collections::BTreeMap::new();
    for candidate in &candidates {
        let mut failed = global_failed.clone();
        let mut pending = BTreeSet::new();
        if candidate.semantic_profile != request.semantic_profile {
            failed.insert("semantic-profile".into());
        }
        if candidate.replay_identity != request.replay_identity {
            failed.insert("replay-identity".into());
        }
        if !candidate.policy_allow {
            failed.insert("candidate-policy".into());
        }
        if !candidate.protected_closure {
            failed.insert("candidate-protected-closure".into());
        }
        if !candidate.signed_approval {
            failed.insert("candidate-signed-approval".into());
        }
        if !candidate.oracle_verified {
            failed.insert("oracle-verification".into());
        }
        if !candidate.raw_data_local {
            failed.insert("candidate-locality".into());
        }
        let score = candidate.support_score_milli
            + (candidate.oracle_verified as i64 * 20_000)
            + (candidate.freshness_seq.min(20) as i64 * 100)
            - (candidate.omission_count.min(20) as i64 * 200);
        scores.insert(candidate.mechanism_id.clone(), score);
        match candidate.evidence_state {
            EvidenceState::Contradicted => {
                failed.insert("contradicted-evidence".into());
                negative.insert(format!("{}:contradicted", candidate.mechanism_id));
            }
            EvidenceState::Unknown | EvidenceState::Speculative => {
                pending.insert("evidence-state".into());
                uncertainty.insert(format!("{}:evidence-state", candidate.mechanism_id));
            }
            EvidenceState::Proven | EvidenceState::Supported => {}
        }
        if candidate.omission_count > 0 {
            pending.insert("omission-closure".into());
            omissions.insert(format!(
                "{}:omissions={}",
                candidate.mechanism_id, candidate.omission_count
            ));
        }
        negative.insert(format!(
            "{}:{}",
            candidate.mechanism_id,
            if candidate.negative_result {
                "negative-result"
            } else {
                "negative-result-not-observed"
            }
        ));
        let disposition = if !failed.is_empty() {
            blocked.push(candidate.mechanism_id.clone());
            "blocked"
        } else if !pending.is_empty() {
            conditional.push(candidate.mechanism_id.clone());
            "conditional"
        } else {
            admitted.push(candidate.mechanism_id.clone());
            "admitted"
        };
        if disposition == "blocked" {
            semantic_loss.push(SemanticLoss {
                field: format!("mechanism:{}", candidate.mechanism_id),
                reason: "mechanism attestation failed one or more federation gates".into(),
                severity: LossSeverity::DecisionRelevant,
            });
        }
        decisions.push(MechanismDecision {
            mechanism_id: candidate.mechanism_id.clone(),
            origin: candidate.origin.clone(),
            support_score_milli: candidate.support_score_milli,
            disposition: disposition.into(),
            failed_gates: failed.into_iter().collect(),
            conditional_gates: pending.into_iter().collect(),
            negative_result: candidate.negative_result,
        });
    }
    let rank_order = {
        let mut ranked = mechanism_order.clone();
        ranked.sort_by(|left, right| {
            scores[right]
                .cmp(&scores[left])
                .then_with(|| left.cmp(right))
        });
        ranked
    };
    let admitted_origins = candidates
        .iter()
        .filter(|candidate| admitted.contains(&candidate.mechanism_id))
        .map(|candidate| candidate.origin.clone())
        .collect::<BTreeSet<_>>();
    let admission = if !global_failed.is_empty() || !blocked.is_empty() {
        FederatedMechanismAdmission::Blocked
    } else if !conditional.is_empty() {
        FederatedMechanismAdmission::ApprovalRequired
    } else if admitted.is_empty() {
        FederatedMechanismAdmission::Unknown
    } else {
        FederatedMechanismAdmission::Admitted
    };
    let checkpoint_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "checkpoint_seq": request.checkpoint_seq, "mechanism_order": mechanism_order, "origin_order": origins})).map_err(|error| FederatedMechanismError::Artifact(error.to_string()))?;
    let control_digest = ContentHash::of_value(&json!({"admission": admission, "rank_order": rank_order, "decisions": decisions, "semantic_loss": semantic_loss})).map_err(|error| FederatedMechanismError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": FEATURE_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "admission": admission, "mechanism_order": mechanism_order, "rank_order": rank_order, "decisions": decisions, "checkpoint_digest": checkpoint_digest, "control_digest": control_digest, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-mechanism-control:{}", request.request_id),
        "application/vnd.aurora.federated-mechanism-portfolio+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: request.federation_id.clone(),
            relation: "federated-mechanism-control".into(),
            digest: control_digest.clone(),
        }],
    )
    .map_err(|error| FederatedMechanismError::Artifact(error.to_string()))?;
    let effect_receipts = match admission {
        FederatedMechanismAdmission::Admitted => vec![
            format!("exchange:permitted-summaries:{}", request.federation_id),
            format!("manage:local-capability:{}", request.federation_id),
        ],
        FederatedMechanismAdmission::ApprovalRequired => vec!["block:unsafe-release".into()],
        _ => vec!["block:unsafe-release".into()],
    };
    let receipt = FederatedMechanismReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: FEATURE_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        admission,
        origin_order: origins.into_iter().collect(),
        admitted_origin_order: admitted_origins.into_iter().collect(),
        mechanism_order: candidates
            .iter()
            .map(|candidate| candidate.mechanism_id.clone())
            .collect(),
        rank_order,
        admitted_order: admitted,
        conditional_order: conditional,
        blocked_order: blocked,
        unknown_order: unknown,
        decisions,
        checkpoint_seq: request.checkpoint_seq,
        checkpoint_digest,
        control_digest,
        replay_identity: request.replay_identity.clone(),
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"federated-mechanism")
    }
    fn candidate(id: &str, origin: &str, state: EvidenceState) -> MechanismCandidate {
        MechanismCandidate {
            mechanism_id: id.into(),
            origin: origin.into(),
            scope: "preclinical".into(),
            semantic_profile: "mechanism-v1".into(),
            support_score_milli: 5000,
            evidence_digest: hash(),
            provenance_digest: hash(),
            replay_identity: hash(),
            evidence_state: state,
            freshness_seq: 2,
            omission_count: 0,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            oracle_verified: true,
            raw_data_local: true,
            negative_result: false,
        }
    }
    fn request() -> FederatedMechanismControlRequest {
        FederatedMechanismControlRequest {
            request_id: "request:mechanism".into(),
            federation_id: "federation:mechanism".into(),
            purpose: "mechanism-exploration".into(),
            semantic_profile: "mechanism-v1".into(),
            required_origin_quorum: 2,
            capacity: 4,
            active_runs: 1,
            checkpoint_seq: 1,
            candidates: vec![
                candidate("m1", "site-a", EvidenceState::Supported),
                candidate("m2", "site-b", EvidenceState::Supported),
            ],
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            network_permitted: true,
            raw_data_local: true,
            replay_identity: hash(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_operator_facing() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A2);
        assert!(capability_manifest()
            .surfaces
            .contains(&ResearchSurface::Operator));
    }
    #[test]
    fn deterministic_rank_and_quorum() {
        let receipt = operate(&request()).unwrap();
        assert_eq!(receipt.admission, FederatedMechanismAdmission::Admitted);
        assert_eq!(receipt.rank_order, vec!["m1", "m2"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_is_approval_required() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Unknown;
        let receipt = operate(&value).unwrap();
        assert_eq!(
            receipt.admission,
            FederatedMechanismAdmission::ApprovalRequired
        );
    }
    #[test]
    fn contradiction_and_policy_block() {
        let mut value = request();
        value.candidates[0].evidence_state = EvidenceState::Contradicted;
        value.policy_allow = false;
        let receipt = operate(&value).unwrap();
        assert_eq!(receipt.admission, FederatedMechanismAdmission::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
    }
    #[test]
    fn locality_and_approval_are_fail_closed() {
        let mut value = request();
        value.candidates[0].raw_data_local = false;
        value.signed_approval = false;
        let receipt = operate(&value).unwrap();
        assert_eq!(receipt.admission, FederatedMechanismAdmission::Blocked);
        assert!(receipt
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }
}
