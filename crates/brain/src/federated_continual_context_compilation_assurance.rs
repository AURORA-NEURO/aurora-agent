//! Federated continual context-compilation verification and safety harness.
//!
//! Atlas feature: `AFA-brain-P03-F28`. Only permitted aggregate artifacts may
//! qualify; missing, stale, uncertain, contradictory, or unauthorized peers
//! remain explicit and prevent unsafe release.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F28";
pub const CONTRACT_VERSION: &str = "brain-federated-continual-context-compilation-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextAssurancePeer {
    pub institution_id: String,
    pub artifact_id: String,
    pub context_digest: ContentHash,
    pub section_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub fresh: bool,
    pub semantic_profile: String,
    pub permitted_artifact: bool,
    pub signed_approval: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextAssuranceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub scope: String,
    pub goal: String,
    pub semantic_profile: String,
    pub institution_order: Vec<String>,
    pub peers: Vec<FederatedContextAssurancePeer>,
    pub minimum_quorum: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub signed_approval: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContextAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub scope: String,
    pub goal: String,
    pub semantic_profile: String,
    pub verdict: FederatedContextAssuranceVerdict,
    pub institution_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub quorum: u16,
    pub minimum_quorum: u16,
    pub envelope_digest: ContentHash,
    pub verification_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContextAssuranceError {
    #[error("invalid federated context assurance request: {0}")]
    Invalid(String),
    #[error("federated context assurance artifact failed: {0}")]
    Artifact(String),
}

impl FederatedContextAssuranceReceipt {
    pub fn validate(&self) -> Result<(), FederatedContextAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.institution_order.len() < 2
            || self.institution_order.len() > usize::from(u16::MAX)
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.minimum_quorum == 0
            || usize::from(self.quorum) != self.qualified_order.len()
            || usize::from(self.quorum) > self.candidate_order.len()
        {
            return Err(FederatedContextAssuranceError::Invalid(
                "federated assurance identity, quorum, locality, aggregate-only, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.institution_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.stale_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedContextAssuranceError::Invalid(
                    "federated assurance ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified.len() != self.candidate_order.len()
            || classified
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
        {
            return Err(FederatedContextAssuranceError::Invalid(
                "federated assurance outcomes do not partition candidates".into(),
            ));
        }
        if self.aggregate_order.len() != self.qualified_order.len() {
            return Err(FederatedContextAssuranceError::Invalid(
                "federated aggregate order does not match qualified peers".into(),
            ));
        }
        for digest in self.aggregate_order.iter().chain([
            &self.envelope_digest,
            &self.verification_digest,
            &self.replay_identity,
        ]) {
            if digest.as_str().len() != 64 {
                return Err(FederatedContextAssuranceError::Invalid(
                    "federated assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assurance:federated-context:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedContextAssuranceError::Invalid(
                "federated assurance effect is outside the governed release gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedContextAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedContextAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedContextAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedContextAssuranceError::Artifact(error.to_string()))
    }
}

pub fn federated_continual_context_compilation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: [
            "research workflow operator".into(),
            "federation administrator".into(),
            "certified context release gate".into(),
        ]
        .into(),
        behavior: "verifies federated continual context candidates with semantic-profile, freshness, provenance, replay, quorum, permission, and aggregate-only gates".into(),
        value: "prevents unauthorized or incomplete institution context from becoming a certified decision section".into(),
        inputs: vec![TypedPort {
            name: "federated_context_assurance_request".into(),
            schema: "FederatedContextAssuranceRequest1@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "federated_context_assurance_receipt".into(),
            schema: "FederatedContextAssuranceResponse1@1".into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]
        .into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "slsa-provenance-1.2".into(),
            state: EvidenceState::Supported,
            locator: Some("https://slsa.dev/spec/v1.2/provenance".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_federated_continual_context_compilation(
    request: &FederatedContextAssuranceRequest,
) -> Result<FederatedContextAssuranceReceipt, FederatedContextAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.institution_order.len() < 2
        || request.institution_order.len() > usize::from(u16::MAX)
        || request.minimum_quorum == 0
        || request.replay_identity.as_str().len() != 64
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContextAssuranceError::Invalid(
            "federated assurance identity, quorum, replay, locality, aggregate-only, or boundary is invalid".into(),
        ));
    }
    let institutions = request
        .institution_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if institutions.len() != request.institution_order.len()
        || institutions
            .iter()
            .any(|institution| institution.trim().is_empty())
        || usize::from(request.minimum_quorum) > institutions.len()
    {
        return Err(FederatedContextAssuranceError::Invalid(
            "federated institution identifiers or quorum are invalid".into(),
        ));
    }
    let mut peer_map = BTreeMap::new();
    for peer in &request.peers {
        if !institutions.contains(&peer.institution_id)
            || peer.artifact_id.trim().is_empty()
            || peer.semantic_profile.trim().is_empty()
        {
            return Err(FederatedContextAssuranceError::Invalid(
                "federated peer identity is invalid".into(),
            ));
        }
        if peer_map.insert(peer.institution_id.clone(), peer).is_some() {
            return Err(FederatedContextAssuranceError::Invalid(
                "federated peers must be unique per institution".into(),
            ));
        }
    }
    let candidate_order = institutions.iter().cloned().collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut aggregate_order = Vec::new();
    let mut witnesses = BTreeSet::from([
        "gate:typed-federated-contract".to_string(),
        "gate:institution-closure".to_string(),
        "gate:semantic-profile".to_string(),
        "gate:freshness".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:permitted-aggregate".to_string(),
        "gate:quorum".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let global_open = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.aggregate_only
        && request.signed_approval;

    for institution in &candidate_order {
        let Some(peer) = peer_map.get(institution) else {
            unknown.insert(institution.clone());
            omissions.insert(format!("institution:{}:missing-peer", institution));
            continue;
        };
        if !global_open
            || !peer.permitted_artifact
            || !peer.signed_approval
            || !peer.aggregate_only
            || !peer.raw_data_local
            || peer.boundary != PRECLINICAL_BOUNDARY
        {
            blocked.insert(institution.clone());
            counterexamples.insert(format!(
                "counterexample:{}:permission-approval-aggregate-locality",
                institution
            ));
        } else if peer.semantic_profile != request.semantic_profile {
            blocked.insert(institution.clone());
            negative.insert(format!(
                "institution:{}:semantic-profile-mismatch",
                institution
            ));
        } else if !peer.fresh {
            unknown.insert(institution.clone());
            stale.insert(institution.clone());
            uncertainty.insert(format!("institution:{}:stale", institution));
        } else if peer.replay_identity != request.replay_identity {
            unknown.insert(institution.clone());
            uncertainty.insert(format!("institution:{}:replay-mismatch", institution));
        } else if peer.evidence_digest.is_none() || peer.provenance_digest.is_none() {
            unknown.insert(institution.clone());
            omissions.insert(format!(
                "institution:{}:evidence-or-provenance-missing",
                institution
            ));
        } else if matches!(
            peer.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unknown.insert(institution.clone());
            uncertainty.insert(format!("institution:{}:evidence-uncertain", institution));
        } else if matches!(peer.state, EvidenceState::Contradicted) {
            blocked.insert(institution.clone());
            negative.insert(format!("institution:{}:contradicted", institution));
        } else {
            qualified.insert(institution.clone());
            aggregate_order.push(
                ContentHash::of_value(&json!({
                    "institution_id": peer.institution_id,
                    "artifact_id": peer.artifact_id,
                    "context_digest": peer.context_digest,
                    "section_digest": peer.section_digest,
                    "evidence_digest": peer.evidence_digest,
                    "provenance_digest": peer.provenance_digest,
                    "semantic_profile": peer.semantic_profile,
                    "replay_identity": peer.replay_identity,
                }))
                .map_err(|error| FederatedContextAssuranceError::Artifact(error.to_string()))?,
            );
        }
    }
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("assurance:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("assurance:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        counterexamples.insert("counterexample:signed-approval-missing".into());
        omissions.insert("assurance:signed-approval-missing".into());
    }
    if !unknown.is_empty() || qualified.len() < usize::from(request.minimum_quorum) {
        witnesses.insert("gate:incomplete-federated-closure-retained".into());
    }
    aggregate_order.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let verdict = if !global_open || !blocked.is_empty() {
        FederatedContextAssuranceVerdict::Blocked
    } else if !unknown.is_empty() || qualified.len() < usize::from(request.minimum_quorum) {
        FederatedContextAssuranceVerdict::Unresolved
    } else {
        FederatedContextAssuranceVerdict::Qualified
    };
    let envelope_digest = ContentHash::of_value(&json!({
        "feature_id": FEATURE_ID,
        "federation_id": request.federation_id,
        "semantic_profile": request.semantic_profile,
        "aggregate_order": aggregate_order,
        "quorum": qualified.len(),
        "minimum_quorum": request.minimum_quorum,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| FederatedContextAssuranceError::Artifact(error.to_string()))?;
    let verification_digest = ContentHash::of_value(&json!({
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "candidate_order": candidate_order,
        "qualified_order": qualified,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "stale_order": stale,
        "envelope_digest": envelope_digest,
        "verdict": verdict,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| FederatedContextAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "scope": request.scope,
        "goal": request.goal,
        "semantic_profile": request.semantic_profile,
        "verdict": verdict,
        "institution_order": candidate_order,
        "candidate_order": candidate_order,
        "qualified_order": qualified,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "stale_order": stale,
        "aggregate_order": aggregate_order,
        "quorum": qualified.len(),
        "minimum_quorum": request.minimum_quorum,
        "envelope_digest": envelope_digest,
        "verification_digest": verification_digest,
        "replay_identity": request.replay_identity,
        "witness_order": witnesses,
        "counterexample_order": counterexamples,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-continual-context-compilation-assurance:{}",
            request.request_id
        ),
        "application/vnd.aurora.federated-continual-context-compilation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedContextAssuranceError::Artifact(error.to_string()))?;
    let quorum = u16::try_from(qualified.len()).map_err(|_| {
        FederatedContextAssuranceError::Invalid(
            "federated qualified institution count exceeds the receipt quorum width".into(),
        )
    })?;
    let receipt = FederatedContextAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        scope: request.scope.clone(),
        goal: request.goal.clone(),
        semantic_profile: request.semantic_profile.clone(),
        verdict,
        institution_order: candidate_order.clone(),
        candidate_order,
        qualified_order: qualified.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        stale_order: stale.into_iter().collect(),
        aggregate_order,
        quorum,
        minimum_quorum: request.minimum_quorum,
        envelope_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(verdict, FederatedContextAssuranceVerdict::Qualified) {
            vec![format!(
                "assurance:federated-context:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn request() -> FederatedContextAssuranceRequest {
        let replay = hash("federated-continual-context-assurance");
        let peer = |institution: &str| FederatedContextAssurancePeer {
            institution_id: institution.into(),
            artifact_id: format!("artifact:{institution}"),
            context_digest: replay.clone(),
            section_digest: replay.clone(),
            evidence_digest: Some(replay.clone()),
            provenance_digest: Some(replay.clone()),
            replay_identity: replay.clone(),
            state: EvidenceState::Supported,
            fresh: true,
            semantic_profile: "preclinical:context:v1".into(),
            permitted_artifact: true,
            signed_approval: true,
            aggregate_only: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedContextAssuranceRequest {
            request_id: "request:federated-context-assurance".into(),
            federation_id: "federation:preclinical-commons".into(),
            purpose: "replication-context".into(),
            scope: "organoid:neural-circuit".into(),
            goal: "compile-certified-context".into(),
            semantic_profile: "preclinical:context:v1".into(),
            institution_order: vec!["site:a".into(), "site:b".into()],
            peers: vec![peer("site:a"), peer("site:b")],
            minimum_quorum: 2,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            signed_approval: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_continual_context_compilation_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn complete_is_qualified() {
        assert_eq!(
            assure_federated_continual_context_compilation(&request())
                .unwrap()
                .verdict,
            FederatedContextAssuranceVerdict::Qualified
        );
    }
    #[test]
    fn missing_peer_is_unresolved() {
        let mut value = request();
        value.peers.pop();
        assert_eq!(
            assure_federated_continual_context_compilation(&value)
                .unwrap()
                .verdict,
            FederatedContextAssuranceVerdict::Unresolved
        );
    }
    #[test]
    fn stale_peer_is_unresolved() {
        let mut value = request();
        value.peers[0].fresh = false;
        assert_eq!(
            assure_federated_continual_context_compilation(&value)
                .unwrap()
                .verdict,
            FederatedContextAssuranceVerdict::Unresolved
        );
    }
    #[test]
    fn semantic_profile_is_blocked() {
        let mut value = request();
        value.peers[0].semantic_profile = "other:v2".into();
        assert_eq!(
            assure_federated_continual_context_compilation(&value)
                .unwrap()
                .verdict,
            FederatedContextAssuranceVerdict::Blocked
        );
    }
    #[test]
    fn quorum_is_unresolved() {
        let mut value = request();
        value.minimum_quorum = 2;
        value.peers[0].state = EvidenceState::Unknown;
        assert_eq!(
            assure_federated_continual_context_compilation(&value)
                .unwrap()
                .verdict,
            FederatedContextAssuranceVerdict::Unresolved
        );
    }
    #[test]
    fn non_local_input_returns_blocked_metadata_receipt() {
        let mut value = request();
        value.peers[0].raw_data_local = false;
        let receipt = assure_federated_continual_context_compilation(&value).unwrap();
        assert_eq!(receipt.verdict, FederatedContextAssuranceVerdict::Blocked);
        assert!(receipt.raw_data_local);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = assure_federated_continual_context_compilation(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn institution_count_cannot_overflow_receipt_quorum() {
        let mut value = request();
        value.institution_order = (0..=usize::from(u16::MAX))
            .map(|index| format!("institution:{index}"))
            .collect();
        assert!(matches!(
            assure_federated_continual_context_compilation(&value),
            Err(FederatedContextAssuranceError::Invalid(_))
        ));
    }
}
