//! Federated continual replication and negative-result assurance.
//!
//! Atlas feature: `AFA-ops-P15-F28`.  The harness turns caller-attested
//! replication claims into deterministic, content-addressed release evidence.
//! Null and failed outcomes remain publishable evidence; they are never treated
//! as missing or converted into a positive conclusion.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ops-P15-F28";
pub const CONTRACT_VERSION: &str =
    "ops-federated-continual-replication-negative-results-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "ClaimAndProtocol4@1";
pub const OUTPUT_SCHEMA: &str = "ReplicationRecord7@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.replication-record-7+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationOutcome {
    Reproduced,
    Partial,
    Null,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClaim {
    pub claim_id: String,
    pub study_id: String,
    pub independent_site_id: String,
    pub protocol_digest: ContentHash,
    pub result_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub outcome: ReplicationOutcome,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub local_only: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAndProtocol {
    pub schema_version: String,
    pub run_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_claim_order: Vec<String>,
    pub protocol_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub claims: Vec<ReplicationClaim>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationRecord {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub run_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: ReplicationDisposition,
    pub claim_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub reproduced_order: Vec<String>,
    pub negative_result_order: Vec<String>,
    pub missing_claim_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub record_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplicationAssuranceError {
    #[error("invalid replication claim and protocol: {0}")]
    Invalid(String),
    #[error("replication record artifact failed: {0}")]
    Artifact(String),
}
fn invalid(value: impl Into<String>) -> ReplicationAssuranceError {
    ReplicationAssuranceError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl ReplicationRecord {
    pub fn validate(&self) -> Result<(), ReplicationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.run_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.claim_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "replication identity, locality, claims, or effects are incomplete",
            ));
        }
        for values in [
            &self.claim_order,
            &self.admitted_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.reproduced_order,
            &self.negative_result_order,
            &self.missing_claim_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("replication ordering is not canonical"));
            }
        }
        let ids = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .admitted_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("replication states do not partition claims"));
        }
        for value in [
            &self.replay_identity,
            &self.record_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("replication digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ReplicationAssuranceError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("replication artifact type is invalid"));
        }
        if self.disposition == ReplicationDisposition::Qualified
            && self.effect_receipts != [format!("verify:replication-record:{}", self.run_id)]
        {
            return Err(invalid("qualified replication effect is invalid"));
        }
        if self.disposition != ReplicationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified replication must block release"));
        }
        Ok(())
    }
}

pub fn replication_negative_results_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "ops".into(), consumers: ["research data steward".into(), "replication lead".into(), "consortium operator".into()].into(), behavior: "verifies federated replication claims and retains null or failed outcomes as typed evidence without converting them into positive conclusions".into(), value: "makes reproducibility, contradiction, negative results, omissions, and policy boundaries auditable before research-object release".into(), inputs: vec![TypedPort { name: "claim_and_protocol".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "replication_record".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_replication(
    input: &ClaimAndProtocol,
) -> Result<ReplicationRecord, ReplicationAssuranceError> {
    validate_input(input)?;
    let mut claims = input.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_order = claims
        .iter()
        .map(|row| row.claim_id.clone())
        .collect::<Vec<_>>();
    let required = input
        .required_claim_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let known = claim_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut admitted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut reproduced = BTreeSet::new();
    let mut negative_result = BTreeSet::new();
    let mut missing = required
        .difference(&known)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for row in &claims {
        if matches!(
            row.outcome,
            ReplicationOutcome::Null | ReplicationOutcome::Failed
        ) {
            negative_result.insert(row.claim_id.clone());
            negative.insert(format!("{}:{:?}", row.claim_id, row.outcome).to_lowercase());
        }
        omissions.extend(
            row.omissions
                .iter()
                .map(|item| format!("{}:{item}", row.claim_id)),
        );
        uncertainty.extend(
            row.uncertainty
                .iter()
                .map(|item| format!("{}:{item}", row.claim_id)),
        );
        if row.evidence_state == EvidenceState::Contradicted || !row.local_only || !row.permitted {
            blocked.insert(row.claim_id.clone());
        } else if row.replay_identity != input.replay_identity
            || row.semantic_profile != input.semantic_profile
            || !row.omissions.is_empty()
            || !row.uncertainty.is_empty()
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(row.claim_id.clone());
        } else {
            admitted.insert(row.claim_id.clone());
            if row.outcome == ReplicationOutcome::Reproduced {
                reproduced.insert(row.claim_id.clone());
            }
        }
    }
    for id in &missing {
        omissions.insert(format!("{id}:required-claim-missing"));
    }
    negative.extend(
        input
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    if !input.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !input.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !input.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !input.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    let global_block = !input.policy_allow
        || !input.protected_closure
        || !input.signed_approval
        || !input.federation_approved
        || !input.raw_data_local
        || !input.aggregate_only
        || !input.adversarial_events.is_empty();
    if global_block {
        blocked.extend(claim_order.iter().cloned());
        admitted.clear();
        unresolved.clear();
        missing.clear();
        omissions.insert("request:replication-release-gate-blocked".into());
    }
    let disposition = if global_block {
        ReplicationDisposition::Blocked
    } else if required.is_subset(&admitted) && unresolved.is_empty() && blocked.is_empty() {
        ReplicationDisposition::Qualified
    } else {
        ReplicationDisposition::Unresolved
    };
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let reproduced_order = reproduced.into_iter().collect::<Vec<_>>();
    let negative_result_order = negative_result.into_iter().collect::<Vec<_>>();
    let missing_claim_order = missing.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == ReplicationDisposition::Qualified {
        vec![format!("verify:replication-record:{}", input.run_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"run_id":input.run_id,"federation_id":input.federation_id,"semantic_profile":input.semantic_profile,"disposition":disposition,"claim_order":claim_order,"admitted_order":admitted_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"reproduced_order":reproduced_order,"negative_result_order":negative_result_order,"missing_claim_order":missing_claim_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_evidence_order,"replay_identity":input.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":input.raw_data_local,"aggregate_only":input.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let record_digest = ContentHash::of_value(&payload)
        .map_err(|error| ReplicationAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("replication-record:{}", input.run_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ReplicationAssuranceError::Artifact(error.to_string()))?;
    let record = ReplicationRecord {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        run_id: input.run_id.clone(),
        federation_id: input.federation_id.clone(),
        semantic_profile: input.semantic_profile.clone(),
        disposition,
        claim_order: payload["claim_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        admitted_order: payload["admitted_order"]
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
        reproduced_order: payload["reproduced_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_result_order: payload["negative_result_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_claim_order: payload["missing_claim_order"]
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
        replay_identity: input.replay_identity.clone(),
        record_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: input.raw_data_local,
        aggregate_only: input.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    record.validate()?;
    Ok(record)
}

fn validate_input(input: &ClaimAndProtocol) -> Result<(), ReplicationAssuranceError> {
    if input.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || input.run_id.trim().is_empty()
        || input.federation_id.trim().is_empty()
        || input.semantic_profile.trim().is_empty()
        || input.required_claim_order.is_empty()
        || !canonical(&input.required_claim_order)
        || !digest(&input.protocol_digest)
        || !digest(&input.replay_identity)
        || input.claims.is_empty()
        || !canonical(&input.adversarial_events)
        || !input.raw_data_local
        || !input.aggregate_only
        || input.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "claim/protocol identity, closure, digests, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for row in &input.claims {
        if row.claim_id.trim().is_empty()
            || !ids.insert(row.claim_id.clone())
            || row.study_id.trim().is_empty()
            || row.independent_site_id.trim().is_empty()
            || !digest(&row.protocol_digest)
            || !digest(&row.result_digest)
            || !digest(&row.provenance_digest)
            || !digest(&row.replay_identity)
            || row.semantic_profile.trim().is_empty()
            || !canonical(&row.omissions)
            || !canonical(&row.uncertainty)
        {
            return Err(invalid(format!(
                "claim {} is malformed or duplicated",
                row.claim_id
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
    fn input() -> ClaimAndProtocol {
        let d = hash("replication");
        let claim = |id: &str, outcome: ReplicationOutcome| ReplicationClaim {
            claim_id: id.into(),
            study_id: format!("study:{id}"),
            independent_site_id: format!("site:{id}"),
            protocol_digest: d.clone(),
            result_digest: d.clone(),
            provenance_digest: d.clone(),
            replay_identity: d.clone(),
            semantic_profile: "preclinical-neural".into(),
            evidence_state: EvidenceState::Supported,
            outcome,
            omissions: vec![],
            uncertainty: vec![],
            local_only: true,
            permitted: true,
        };
        ClaimAndProtocol {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            run_id: "run:one".into(),
            federation_id: "fed:commons".into(),
            semantic_profile: "preclinical-neural".into(),
            required_claim_order: vec!["claim:a".into(), "claim:b".into()],
            protocol_digest: d.clone(),
            replay_identity: d.clone(),
            claims: vec![
                claim("claim:a", ReplicationOutcome::Reproduced),
                claim("claim:b", ReplicationOutcome::Null),
            ],
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            replication_negative_results_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_replication_keeps_null() {
        let out = assure_replication(&input()).unwrap();
        assert_eq!(out.disposition, ReplicationDisposition::Qualified);
        assert!(out.negative_result_order.contains(&"claim:b".into()));
    }
    #[test]
    fn deterministic_record() {
        let a = assure_replication(&input()).unwrap();
        let b = assure_replication(&input()).unwrap();
        assert_eq!(a.record_digest, b.record_digest);
    }
    #[test]
    fn missing_claim_unresolved() {
        let mut value = input();
        value.required_claim_order = vec!["claim:a".into(), "claim:c".into()];
        assert_eq!(
            assure_replication(&value).unwrap().disposition,
            ReplicationDisposition::Unresolved
        );
    }
    #[test]
    fn contradictory_claim_blocked() {
        let mut value = input();
        value.claims[0].evidence_state = EvidenceState::Contradicted;
        let out = assure_replication(&value).unwrap();
        assert!(out.blocked_order.contains(&"claim:a".into()));
    }
    #[test]
    fn policy_blocks() {
        let mut value = input();
        value.policy_allow = false;
        assert_eq!(
            assure_replication(&value).unwrap().disposition,
            ReplicationDisposition::Blocked
        );
    }
    #[test]
    fn adversarial_blocks() {
        let mut value = input();
        value.adversarial_events.push("poisoned-result".into());
        assert_eq!(
            assure_replication(&value).unwrap().disposition,
            ReplicationDisposition::Blocked
        );
    }
}
