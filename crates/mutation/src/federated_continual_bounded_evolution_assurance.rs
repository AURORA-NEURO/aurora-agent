//! Federated continual bounded-evolution assurance (`AFA-mutation-P32-F28`).
//!
//! Evaluates proposed capability evolution as a signed, replayable promotion preview. It never
//! mutates a running implementation, grants authority, or publishes a release by itself.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mutation-P32-F28";
pub const CONTRACT_VERSION: &str = "mutation-federated-continual-bounded-evolution-assurance/1.0";
pub const INPUT_SCHEMA: &str = "MutationEvolutionRequest8@1";
pub const OUTPUT_SCHEMA: &str = "MutationEvolutionReceipt10@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.mutation-federated-evolution-decision+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_PROPOSALS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationEvolutionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEvolutionProposal7 {
    pub proposal_id: String,
    pub capability_id: String,
    pub from_version: String,
    pub to_version: String,
    pub artifact_digest: ContentHash,
    pub benchmark_digest: ContentHash,
    pub evidence_state: MutationEvolutionEvidenceState,
    pub benchmark_pass: bool,
    pub safety_pass: bool,
    pub compatible: bool,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEvolutionRequest8 {
    pub request_id: String,
    pub purpose: String,
    pub capability_id: String,
    pub current_version: String,
    pub proposals: Vec<MutationEvolutionProposal7>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEvolutionReceipt10Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEvolutionReceipt10 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub capability_id: String,
    pub current_version: String,
    pub disposition: String,
    pub proposal_order: Vec<String>,
    pub approved_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub incompatible_order: Vec<String>,
    pub benchmark_failed_order: Vec<String>,
    pub safety_failed_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub evolution_digest: ContentHash,
    pub artifact: MutationEvolutionReceipt10Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MutationFederatedEvolutionError {
    #[error("invalid mutation federated bounded-evolution request: {0}")]
    Invalid(String),
    #[error("mutation federated bounded-evolution report failed validation: {0}")]
    Report(String),
}
fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}
pub fn mutation_federated_bounded_evolution_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"mutation","consumers":["bioinformatician","mutation safety steward","release operator"],"behavior":"verify federated continual MutationEvolutionCandidate proposals with deterministic compatibility, benchmark, safety, evidence, replay, policy, locality, quorum, and protected-closure gates","value":"makes bounded evidence-driven evolution auditable and reversible before any implementation or release mutation","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["preview:bounded-evolution","manage:local-capability","block:unsafe-release"],"permissions":["read:local-evolution-summaries","request:bounded-evolution-preview"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}
impl MutationEvolutionReceipt10 {
    pub fn validate(&self) -> Result<(), MutationFederatedEvolutionError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.current_version.trim().is_empty()
            || self.proposal_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(MutationFederatedEvolutionError::Report(
                "mutation evolution identity, proposals, effects, locality, or disposition is incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.proposal_order,
            &self.approved_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.incompatible_order,
            &self.benchmark_failed_order,
            &self.safety_failed_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(MutationFederatedEvolutionError::Report(
                    "evolution ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.proposal_order.iter().cloned());
        let parts = self
            .approved_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.proposal_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.evolution_digest)
            || self.artifact.content_hash != self.evolution_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(MutationFederatedEvolutionError::Report(
                "evolution states, digests, or artifact metadata do not partition".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("preview:bounded-evolution:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MutationFederatedEvolutionError::Report(
                "effect is outside governed evolution gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, MutationFederatedEvolutionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MutationFederatedEvolutionError::Report(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MutationFederatedEvolutionError::Report(error.to_string()))
    }
}
fn validate_request(request: &MutationEvolutionRequest8) -> Result<(), MutationFederatedEvolutionError> {
    if request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.capability_id.trim().is_empty()
        || request.current_version.trim().is_empty()
        || request.proposals.is_empty()
        || request.proposals.len() > MAX_PROPOSALS
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(MutationFederatedEvolutionError::Invalid(
            "mutation evolution identity, proposal bound, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for proposal in &request.proposals {
        if proposal.proposal_id.trim().is_empty()
            || !ids.insert(proposal.proposal_id.clone())
            || proposal.capability_id.trim().is_empty()
            || proposal.from_version.trim().is_empty()
            || proposal.to_version.trim().is_empty()
            || !valid_digest(&proposal.artifact_digest)
            || !valid_digest(&proposal.benchmark_digest)
            || !valid_digest(&proposal.replay_identity)
            || !proposal.local
            || !proposal.aggregate_only
        {
            return Err(MutationFederatedEvolutionError::Invalid(format!(
                "proposal {} is invalid, duplicated, non-local, or not digest-bound",
                proposal.proposal_id
            )));
        }
    }
    Ok(())
}
pub fn assure_mutation_federated_bounded_evolution(
    request: &MutationEvolutionRequest8,
) -> Result<MutationEvolutionReceipt10, MutationFederatedEvolutionError> {
    validate_request(request)?;
    let mut proposals = request.proposals.clone();
    proposals.sort_by(|left, right| left.proposal_id.cmp(&right.proposal_id));
    let proposal_order = proposals
        .iter()
        .map(|proposal| proposal.proposal_id.clone())
        .collect::<Vec<_>>();
    let mut approved = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut benchmark_failed = BTreeSet::new();
    let mut safety_failed = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for proposal in &proposals {
        provenance.insert(proposal.artifact_digest.clone());
        provenance.insert(proposal.benchmark_digest.clone());
        if proposal.capability_id != request.capability_id
            || proposal.from_version != request.current_version
        {
            unresolved.insert(proposal.proposal_id.clone());
            incompatible.insert(proposal.proposal_id.clone());
        } else if !proposal.compatible {
            unresolved.insert(proposal.proposal_id.clone());
            incompatible.insert(proposal.proposal_id.clone());
        } else if !proposal.benchmark_pass {
            unresolved.insert(proposal.proposal_id.clone());
            benchmark_failed.insert(proposal.proposal_id.clone());
            negative.insert(format!("{}:benchmark-failed", proposal.proposal_id));
        } else if !proposal.safety_pass {
            blocked.insert(proposal.proposal_id.clone());
            safety_failed.insert(proposal.proposal_id.clone());
            negative.insert(format!("{}:safety-failed", proposal.proposal_id));
        } else if proposal.replay_identity != request.replay_identity {
            unresolved.insert(proposal.proposal_id.clone());
            uncertainty.insert(format!("{}:replay-identity", proposal.proposal_id));
        } else if !proposal.signed {
            blocked.insert(proposal.proposal_id.clone());
            omissions.insert(format!("{}:unsigned", proposal.proposal_id));
        } else if proposal.evidence_state == MutationEvolutionEvidenceState::Contradicted {
            blocked.insert(proposal.proposal_id.clone());
            negative.insert(format!("{}:contradicted", proposal.proposal_id));
        } else if !matches!(
            proposal.evidence_state,
            MutationEvolutionEvidenceState::Proven | MutationEvolutionEvidenceState::Supported
        ) {
            unresolved.insert(proposal.proposal_id.clone());
            uncertainty.insert(format!("{}:evidence-state", proposal.proposal_id));
        } else {
            approved.insert(proposal.proposal_id.clone());
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(proposal_order.iter().cloned());
        approved.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let approved_order = approved.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if global || approved_order.is_empty() && unresolved_order.is_empty() {
        "blocked"
    } else if !blocked_order.is_empty() || !unresolved_order.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:bounded-evolution-not-closed".into());
    }
    let mut effect_order = if disposition == "qualified" {
        vec![
            "manage:local-capability".to_string(),
            "preview:bounded-evolution".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    effect_order.sort();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"purpose":request.purpose,"capability_id":request.capability_id,"current_version":request.current_version,"disposition":disposition,"proposal_order":proposal_order,"approved_order":approved_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"incompatible_order":incompatible.into_iter().collect::<Vec<_>>(),"benchmark_failed_order":benchmark_failed.into_iter().collect::<Vec<_>>(),"safety_failed_order":safety_failed.into_iter().collect::<Vec<_>>(),"omission_order":omissions.into_iter().collect::<Vec<_>>(),"uncertainty_order":uncertainty.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"effect_order":effect_order,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| MutationFederatedEvolutionError::Report(error.to_string()))?;
    let report = MutationEvolutionReceipt10 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        purpose: request.purpose.clone(),
        capability_id: request.capability_id.clone(),
        current_version: request.current_version.clone(),
        disposition: disposition.into(),
        proposal_order: serde_json::from_value(payload["proposal_order"].clone()).unwrap(),
        approved_order: serde_json::from_value(payload["approved_order"].clone()).unwrap(),
        unresolved_order: serde_json::from_value(payload["unresolved_order"].clone()).unwrap(),
        blocked_order: serde_json::from_value(payload["blocked_order"].clone()).unwrap(),
        incompatible_order: serde_json::from_value(payload["incompatible_order"].clone()).unwrap(),
        benchmark_failed_order: serde_json::from_value(payload["benchmark_failed_order"].clone())
            .unwrap(),
        safety_failed_order: serde_json::from_value(payload["safety_failed_order"].clone())
            .unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        effect_order: serde_json::from_value(payload["effect_order"].clone()).unwrap(),
        replay_identity: request.replay_identity.clone(),
        evolution_digest: digest.clone(),
        artifact: MutationEvolutionReceipt10Artifact {
            artifact_id: format!("mutation-federated-evolution-decision-7:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: digest,
            semantic_loss: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: effect_order
            .iter()
            .map(|effect| {
                if effect == "block:unsafe-release" {
                    effect.clone()
                } else {
                    format!("{effect}:{}", request.request_id)
                }
            })
            .collect(),
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn proposal(id: &str) -> MutationEvolutionProposal7 {
        MutationEvolutionProposal7 {
            proposal_id: id.into(),
            capability_id: "ids.compute".into(),
            from_version: "1.0".into(),
            to_version: "1.1".into(),
            artifact_digest: h(id),
            benchmark_digest: h("benchmark"),
            evidence_state: MutationEvolutionEvidenceState::Supported,
            benchmark_pass: true,
            safety_pass: true,
            compatible: true,
            replay_identity: h("replay"),
            signed: true,
            local: true,
            aggregate_only: true,
        }
    }
    fn request() -> MutationEvolutionRequest8 {
        MutationEvolutionRequest8 {
            request_id: "request:evolution".into(),
            purpose: "promote".into(),
            capability_id: "ids.compute".into(),
            current_version: "1.0".into(),
            proposals: vec![proposal("proposal:b"), proposal("proposal:a")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(mutation_federated_bounded_evolution_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            assure_mutation_federated_bounded_evolution(&request()).unwrap().disposition,
            "qualified"
        );
    }
    #[test]
    fn incompatible_is_unresolved() {
        let mut q = request();
        q.proposals[0].compatible = false;
        assert_eq!(
            assure_mutation_federated_bounded_evolution(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn safety_failure_is_unresolved() {
        let mut q = request();
        q.proposals[0].safety_pass = false;
        assert_eq!(
            assure_mutation_federated_bounded_evolution(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            assure_mutation_federated_bounded_evolution(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let a = assure_mutation_federated_bounded_evolution(&request()).unwrap();
        let b = assure_mutation_federated_bounded_evolution(&request()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
