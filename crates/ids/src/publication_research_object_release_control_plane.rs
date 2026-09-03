//! Prospective high-throughput publication and research-object release control
//! plane (`AFA-ids-P16-F31`).
//!
//! This module compiles a signed, digest-only release intent from institution
//! local research-object manifests.  It is a release gate, not a publisher:
//! raw imaging/omics bytes never leave the institution, and an emitted intent
//! does not claim that a scientific result is true or clinically actionable.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P16-F31";
pub const CONTRACT_VERSION: &str =
    "ids-prospective-high-throughput-publication-research-object-release-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ValidatedResearchRun7@1";
pub const OUTPUT_SCHEMA: &str = "SignedResearchObject11@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.signed-research-object-11+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_ARTIFACTS: usize = 8192;
pub const MAX_PEERS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchArtifact8 {
    pub artifact_id: String,
    pub run_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub media_type: String,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub estimated_units: u64,
    pub evidence_state: ReleaseEvidenceState,
    pub permitted: bool,
    pub signed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omitted_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleasePeer7 {
    pub peer_id: String,
    pub origin: String,
    pub run_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub object_digest: ContentHash,
    pub artifact_count: usize,
    pub evidence_state: ReleaseEvidenceState,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun7 {
    pub request_id: String,
    pub run_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub engine_version: String,
    pub artifacts: Vec<ResearchArtifact8>,
    pub peers: Vec<ReleasePeer7>,
    pub checkpoint: u64,
    pub minimum_artifact_count: usize,
    pub minimum_peer_quorum: usize,
    pub max_budget_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject11Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject11 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub run_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub engine_version: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub artifact_order: Vec<String>,
    pub selected_artifact_order: Vec<String>,
    pub unresolved_artifact_order: Vec<String>,
    pub blocked_artifact_order: Vec<String>,
    pub missing_provenance_order: Vec<String>,
    pub missing_evidence_order: Vec<String>,
    pub omitted_field_order: Vec<String>,
    pub negative_result_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub release_digest: ContentHash,
    pub artifact: SignedResearchObject11Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PublicationReleaseError {
    #[error("invalid publication/research-object release request: {0}")]
    Invalid(String),
    #[error("publication/research-object release artifact failed: {0}")]
    Artifact(String),
}

pub fn publication_release_control_plane_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["computational biologist", "research-object steward", "publication gateway operator", "federation steward"],
        "behavior": "compiles high-throughput validated research-run manifests into a deterministic signed release intent",
        "value": "prevents missing provenance, evidence, replay, policy, approval, peer, budget, or locality closure from becoming an apparently publishable research object",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:permitted-summaries", "manage:local-capability", "publish:signed-research-object"],
        "permissions": ["read:local-research-object-manifests", "request:governed-release"],
        "autonomy_tier": "A2",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl SignedResearchObject11 {
    pub fn validate(&self) -> Result<(), PublicationReleaseError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !all_nonempty([
                &self.request_id,
                &self.run_id,
                &self.requester,
                &self.purpose,
                &self.semantic_profile,
                &self.engine_version,
            ])
            || self.checkpoint == 0
            || self.artifact_order.is_empty()
            || self.peer_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(PublicationReleaseError::Invalid(
                "release identity, checkpoint, locality, artifacts, peers, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.artifact_order,
            &self.selected_artifact_order,
            &self.unresolved_artifact_order,
            &self.blocked_artifact_order,
            &self.missing_provenance_order,
            &self.missing_evidence_order,
            &self.omitted_field_order,
            &self.negative_result_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(PublicationReleaseError::Invalid(
                    "release ordering is not canonical".into(),
                ));
            }
        }
        let artifacts = BTreeSet::from_iter(self.artifact_order.iter().cloned());
        let states = self
            .selected_artifact_order
            .iter()
            .chain(&self.unresolved_artifact_order)
            .chain(&self.blocked_artifact_order)
            .cloned()
            .collect::<Vec<_>>();
        let peers = BTreeSet::from_iter(self.peer_order.iter().cloned());
        let peer_states = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if artifacts.len() != self.artifact_order.len()
            || BTreeSet::from_iter(states.iter().cloned()) != artifacts
            || states.len() != artifacts.len()
            || peers != BTreeSet::from_iter(peer_states.iter().cloned())
            || peer_states.len() != peers.len()
        {
            return Err(PublicationReleaseError::Invalid(
                "artifact or peer states do not partition".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.release_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| d.as_str().len() != 64)
        {
            return Err(PublicationReleaseError::Artifact(
                "release artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-summaries:")
                && !effect.starts_with("manage:local-capability:")
                && !effect.starts_with("publish:signed-research-object:")
                && effect != "block:unsafe-release"
        }) {
            return Err(PublicationReleaseError::Invalid(
                "effect is outside the governed release gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, PublicationReleaseError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| PublicationReleaseError::Artifact(e.to_string()))?,
        )
        .map_err(|e| PublicationReleaseError::Artifact(e.to_string()))
    }
}

fn all_nonempty<const N: usize>(values: [&String; N]) -> bool {
    values.iter().all(|v| !v.trim().is_empty())
}

pub fn compile_publication_release(
    request: &ValidatedResearchRun7,
) -> Result<SignedResearchObject11, PublicationReleaseError> {
    validate_request(request)?;
    let mut artifacts = request.artifacts.clone();
    artifacts.sort_by(|a, b| a.artifact_id.cmp(&b.artifact_id));
    let artifact_order = artifacts
        .iter()
        .map(|a| a.artifact_id.clone())
        .collect::<Vec<_>>();
    let mut peers = request.peers.clone();
    peers.sort_by(|a, b| a.peer_id.cmp(&b.peer_id));
    let peer_order = peers.iter().map(|p| p.peer_id.clone()).collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_provenance = BTreeSet::new();
    let mut missing_evidence = BTreeSet::new();
    let mut omitted_fields = BTreeSet::new();
    let mut negative_result = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut total_units = 0_u64;
    for artifact in &artifacts {
        let id = artifact.artifact_id.clone();
        total_units = total_units.saturating_add(artifact.estimated_units);
        if artifact.negative_result {
            negative_result.insert(id.clone());
            negative_evidence.insert(format!("{id}:negative-result"));
        }
        if artifact.run_id != request.run_id
            || artifact.semantic_profile != request.semantic_profile
        {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:run-or-semantic-profile-mismatch"));
            continue;
        }
        if !artifact.raw_data_local || !artifact.aggregate_only {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:raw-data-not-local-or-aggregate"));
            continue;
        }
        if !artifact.protected_closure {
            blocked.insert(id.clone());
            uncertainty.insert(format!("{id}:protected-closure-incomplete"));
            continue;
        }
        if artifact.evidence_state == ReleaseEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative_evidence.insert(format!("{id}:contradicted"));
            continue;
        }
        if artifact.provenance_digest.as_str().len() != 64 {
            missing_provenance.insert(id.clone());
        }
        if artifact.evidence_digest.as_str().len() != 64 {
            missing_evidence.insert(id.clone());
        }
        if !artifact.omitted_fields.is_empty() {
            omitted_fields.extend(
                artifact
                    .omitted_fields
                    .iter()
                    .map(|field| format!("{id}:{field}")),
            );
        }
        if artifact.replay_identity != request.replay_identity
            || !artifact.permitted
            || !artifact.signed
        {
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:replay-or-authorization"));
            continue;
        }
        if !matches!(
            artifact.evidence_state,
            ReleaseEvidenceState::Proven | ReleaseEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
            continue;
        }
        if !missing_provenance.contains(&id)
            && !missing_evidence.contains(&id)
            && artifact.omitted_fields.is_empty()
        {
            selected.insert(id);
        } else {
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:closure-or-omission"));
        }
    }
    if !missing_provenance.is_empty() {
        omissions.insert("request:provenance-closure-incomplete".into());
    }
    if !missing_evidence.is_empty() {
        omissions.insert("request:evidence-closure-incomplete".into());
    }
    if !omitted_fields.is_empty() {
        omissions.insert("request:explicit-semantic-loss-present".into());
    }
    let mut qualified_peers = BTreeSet::new();
    let mut missing_peers = BTreeSet::new();
    for peer in &peers {
        let ok = peer.run_id == request.run_id
            && peer.semantic_profile == request.semantic_profile
            && peer.checkpoint == request.checkpoint
            && peer.artifact_count > 0
            && peer.signed
            && peer.aggregate_only
            && peer.raw_data_local
            && matches!(
                peer.evidence_state,
                ReleaseEvidenceState::Proven | ReleaseEvidenceState::Supported
            );
        if ok {
            qualified_peers.insert(peer.peer_id.clone());
        } else {
            missing_peers.insert(peer.peer_id.clone());
            uncertainty.insert(format!("peer:{}:not-qualified", peer.peer_id));
        }
    }
    if qualified_peers.len() < request.minimum_peer_quorum {
        uncertainty.insert("peer:minimum-quorum-unmet".into());
    }
    if selected.len() < request.minimum_artifact_count {
        uncertainty.insert("release:minimum-artifact-count-unmet".into());
    }
    if total_units > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{total_units}"));
    }
    if !request.policy_allow {
        negative_evidence.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(artifact_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:publication-release-not-authorized".into());
    }
    let disposition = if global_block || selected.is_empty() && !blocked.is_empty() {
        "blocked"
    } else if selected.len() < request.minimum_artifact_count
        || qualified_peers.len() < request.minimum_peer_quorum
        || total_units > request.max_budget_units
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:release-intent-not-release-ready".into());
    }
    let selected_artifact_order = selected.iter().cloned().collect::<Vec<_>>();
    let unresolved_artifact_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let blocked_artifact_order = blocked.iter().cloned().collect::<Vec<_>>();
    let payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "run_id": request.run_id,
        "requester": request.requester,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "engine_version": request.engine_version,
        "checkpoint": request.checkpoint,
        "disposition": disposition,
        "artifact_order": artifact_order,
        "selected_artifact_order": selected_artifact_order,
        "unresolved_artifact_order": unresolved_artifact_order,
        "blocked_artifact_order": blocked_artifact_order,
        "missing_provenance_order": missing_provenance,
        "missing_evidence_order": missing_evidence,
        "omitted_field_order": omitted_fields,
        "negative_result_order": negative_result,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peers,
        "missing_peer_order": missing_peers,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative_evidence,
        "total_units": total_units,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| PublicationReleaseError::Artifact(e.to_string()))?;
    let artifact = SignedResearchObject11Artifact {
        artifact_id: format!("signed-research-object-11:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: omissions.iter().cloned().collect(),
        provenance_digests: artifacts
            .iter()
            .map(|a| a.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = SignedResearchObject11 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        run_id: request.run_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        engine_version: request.engine_version.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        artifact_order,
        selected_artifact_order,
        unresolved_artifact_order,
        blocked_artifact_order,
        missing_provenance_order: missing_provenance.into_iter().collect(),
        missing_evidence_order: missing_evidence.into_iter().collect(),
        omitted_field_order: omitted_fields.into_iter().collect(),
        negative_result_order: negative_result.into_iter().collect(),
        peer_order,
        qualified_peer_order: qualified_peers.into_iter().collect(),
        missing_peer_order: missing_peers.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative_evidence.into_iter().collect(),
        total_units,
        replay_identity: request.replay_identity.clone(),
        release_digest: digest,
        artifact,
        effect_receipts: if disposition == "qualified" {
            vec![
                format!("exchange:permitted-summaries:{}", request.request_id),
                format!("manage:local-capability:{}", request.request_id),
                format!("publish:signed-research-object:{}", request.request_id),
            ]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ValidatedResearchRun7) -> Result<(), PublicationReleaseError> {
    if !all_nonempty([
        &request.request_id,
        &request.run_id,
        &request.requester,
        &request.purpose,
        &request.semantic_profile,
        &request.engine_version,
    ]) || request.artifacts.is_empty()
        || request.artifacts.len() > MAX_ARTIFACTS
        || request.peers.is_empty()
        || request.peers.len() > MAX_PEERS
        || request.checkpoint == 0
        || request.minimum_artifact_count == 0
        || request.minimum_peer_quorum == 0
        || request.max_budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(PublicationReleaseError::Invalid(
            "request identity, artifacts, peers, bounds, replay, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for artifact in &request.artifacts {
        if !all_nonempty([
            &artifact.artifact_id,
            &artifact.run_id,
            &artifact.study_id,
            &artifact.semantic_profile,
            &artifact.media_type,
        ]) || !ids.insert(artifact.artifact_id.clone())
            || artifact.estimated_units == 0
            || artifact.content_digest.as_str().len() != 64
            || artifact.provenance_digest.as_str().len() != 64
            || artifact.evidence_digest.as_str().len() != 64
            || artifact.replay_identity.as_str().len() != 64
            || artifact.omitted_fields.windows(2).any(|w| w[0] >= w[1])
        {
            return Err(PublicationReleaseError::Invalid(
                "research artifact identity, digest, bounds, or omission ordering is invalid"
                    .into(),
            ));
        }
    }
    let mut peers = BTreeSet::new();
    for peer in &request.peers {
        if !all_nonempty([
            &peer.peer_id,
            &peer.origin,
            &peer.run_id,
            &peer.semantic_profile,
        ]) || !peers.insert(peer.peer_id.clone())
            || peer.checkpoint == 0
            || peer.artifact_count == 0
            || peer.object_digest.as_str().len() != 64
        {
            return Err(PublicationReleaseError::Invalid(
                "release peer identity, checkpoint, count, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn artifact(id: &str, negative: bool) -> ResearchArtifact8 {
        ResearchArtifact8 {
            artifact_id: id.into(),
            run_id: "run:one".into(),
            study_id: format!("study:{id}"),
            semantic_profile: "neuro:release:v1".into(),
            media_type: "application/ro-crate+json".into(),
            content_digest: h(id),
            provenance_digest: h("provenance"),
            evidence_digest: h("evidence"),
            replay_identity: h("replay"),
            estimated_units: 5,
            evidence_state: ReleaseEvidenceState::Supported,
            permitted: true,
            signed: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: negative,
            omitted_fields: Vec::new(),
        }
    }

    fn request() -> ValidatedResearchRun7 {
        ValidatedResearchRun7 {
            request_id: "request:release".into(),
            run_id: "run:one".into(),
            requester: "research-object-steward".into(),
            purpose: "high-throughput-preclinical-release".into(),
            semantic_profile: "neuro:release:v1".into(),
            engine_version: "engine:1".into(),
            artifacts: vec![artifact("artifact:a", false), artifact("artifact:b", true)],
            peers: vec![ReleasePeer7 {
                peer_id: "peer:one".into(),
                origin: "site:peer".into(),
                run_id: "run:one".into(),
                semantic_profile: "neuro:release:v1".into(),
                checkpoint: 2,
                object_digest: h("peer-object"),
                artifact_count: 2,
                evidence_state: ReleaseEvidenceState::Supported,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
            checkpoint: 2,
            minimum_artifact_count: 2,
            minimum_peer_quorum: 1,
            max_budget_units: 100,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            publication_release_control_plane_manifest()["autonomy_tier"],
            "A2"
        );
    }

    #[test]
    fn nominal_release_is_qualified_and_negative_is_retained() {
        let receipt = compile_publication_release(&request()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert_eq!(receipt.negative_result_order, vec!["artifact:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn missing_provenance_is_unresolved() {
        let mut request = request();
        request.artifacts[0].provenance_digest = h("bad");
        let receipt = compile_publication_release(&request).unwrap();
        assert!(receipt.missing_provenance_order.is_empty());
        // ContentHash is always well formed; omission must be represented by
        // an explicit semantic-loss declaration instead.
        request.artifacts[0].omitted_fields = vec!["provenance".into()];
        let receipt = compile_publication_release(&request).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
    }

    #[test]
    fn peer_quorum_gap_is_unresolved() {
        let mut request = request();
        request.minimum_peer_quorum = 2;
        let receipt = compile_publication_release(&request).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(receipt
            .uncertainty_order
            .contains(&"peer:minimum-quorum-unmet".into()));
    }

    #[test]
    fn policy_denial_blocks_without_publish_effect() {
        let mut request = request();
        request.policy_allow = false;
        let receipt = compile_publication_release(&request).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn semantic_profile_mismatch_is_blocked() {
        let mut request = request();
        request.artifacts[0].semantic_profile = "other:v1".into();
        let receipt = compile_publication_release(&request).unwrap();
        assert!(receipt
            .blocked_artifact_order
            .contains(&"artifact:a".into()));
    }

    #[test]
    fn explicit_omission_never_passes_as_complete() {
        let mut request = request();
        request.artifacts[1].omitted_fields = vec!["uncertainty".into()];
        let receipt = compile_publication_release(&request).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(!receipt.omitted_field_order.is_empty());
    }
}
