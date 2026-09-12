//! Local single-study replication and negative-result workbench (`AFA-registry-P15-F17`).
//! The workbench evaluates caller-supplied attestations only; it never runs protocols or
//! infers biological truth. Every claim remains selected, unresolved, blocked, or missing.

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

pub const FEATURE_ID: &str = "AFA-registry-P15-F17";
pub const CONTRACT_VERSION: &str =
    "registry-local-single-study-replication-negative-results-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ClaimAndProtocol1@1";
pub const OUTPUT_SCHEMA: &str = "ReplicationRecord5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.registry-replication-record-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationClaim4 {
    pub claim_id: String,
    pub protocol_id: String,
    pub study_id: String,
    pub scope: String,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub replicated: bool,
    pub independent: bool,
    pub signed: bool,
    pub protected_closure: bool,
    pub policy_allow: bool,
    pub local_only: bool,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAndProtocol1 {
    pub schema_version: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub required_claim_order: Vec<String>,
    pub required_protocol_order: Vec<String>,
    pub claims: Vec<ReplicationClaim4>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub researcher_authorized: bool,
    pub raw_data_local: bool,
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
pub struct ReplicationRecord5 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: ReplicationDisposition,
    pub claim_order: Vec<String>,
    pub selected_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub blocked_claim_order: Vec<String>,
    pub missing_claim_order: Vec<String>,
    pub protocol_order: Vec<String>,
    pub selected_protocol_order: Vec<String>,
    pub missing_protocol_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub replication_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplicationWorkbenchError {
    #[error("invalid replication request: {0}")]
    Invalid(String),
    #[error("replication artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ReplicationWorkbenchError {
    ReplicationWorkbenchError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

impl ReplicationRecord5 {
    pub fn validate(&self) -> Result<(), ReplicationWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.claim_order.is_empty()
            || self.protocol_order.is_empty()
            || self.evidence_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("replication identity, claims, protocols, evidence, locality, or effects are incomplete"));
        }
        for values in [
            &self.claim_order,
            &self.selected_claim_order,
            &self.unresolved_claim_order,
            &self.blocked_claim_order,
            &self.missing_claim_order,
            &self.protocol_order,
            &self.selected_protocol_order,
            &self.missing_protocol_order,
            &self.evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("replication ordering is not canonical"));
            }
        }
        let all = self.claim_order.iter().cloned().collect::<BTreeSet<_>>();
        let state_parts = self
            .selected_claim_order
            .iter()
            .chain(self.unresolved_claim_order.iter())
            .chain(self.blocked_claim_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        let state_part_set = state_parts.iter().cloned().collect::<BTreeSet<_>>();
        let missing = self
            .missing_claim_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        // `claim_order` names claims that were observed. Missing required claims are
        // intentionally outside that universe, so including them in the state partition
        // makes every honest unresolved record fail validation. Require a complete,
        // disjoint partition of observed claims and separately ensure missing IDs do not
        // overlap or duplicate observed claims.
        if all.len() != self.claim_order.len()
            || state_parts.len() != state_part_set.len()
            || state_part_set != all
            || missing.len() != self.missing_claim_order.len()
            || missing.iter().any(|id| all.contains(id))
        {
            return Err(invalid("claim states do not form a complete partition"));
        }
        if !self
            .selected_protocol_order
            .iter()
            .all(|id| self.protocol_order.contains(id))
            || self
                .missing_protocol_order
                .iter()
                .any(|id| self.protocol_order.contains(id))
        {
            return Err(invalid(
                "replication closure references undeclared claims or protocols",
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.replication_digest)
            || self.artifact.content_hash != self.replication_digest
        {
            return Err(invalid("replication digest is invalid"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| ReplicationWorkbenchError::Artifact(e.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("replication artifact content type is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:authorized-research-state:")
                && effect != "block:unsafe-release"
        }) {
            return Err(invalid("replication effect is outside read-only gate"));
        }
        if self.disposition == ReplicationDisposition::Qualified
            && self.effect_receipts
                != [format!(
                    "view:authorized-research-state:{}",
                    self.request_id
                )]
        {
            return Err(invalid("qualified replication effect is invalid"));
        }
        if self.disposition != ReplicationDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified replication must block"));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ReplicationWorkbenchError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| ReplicationWorkbenchError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ReplicationWorkbenchError::Artifact(e.to_string()))
    }
}

pub fn replication_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "registry".into(), consumers: ["laboratory automation engineer".into(), "replication scientist".into(), "research workbench operator".into()].into(), behavior: "evaluates typed local replication attestations and preserves negative or unresolved evidence without running protocols".into(), value: "makes replication and null-result closure auditable while preventing unsupported claims from becoming release evidence".into(), inputs: vec![TypedPort { name: "claim_and_protocol".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "replication_record".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::WriteLocalArtifact].into(), permissions: ["view:authorized-research-state".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_replication(
    request: &ClaimAndProtocol1,
) -> Result<ReplicationRecord5, ReplicationWorkbenchError> {
    validate_request(request)?;
    let mut rows = request.claims.clone();
    rows.sort_by(|a, b| {
        a.study_id
            .cmp(&b.study_id)
            .then(a.claim_id.cmp(&b.claim_id))
    });
    let claim_order = rows.iter().map(|c| c.claim_id.clone()).collect::<Vec<_>>();
    let protocol_order = rows
        .iter()
        .map(|c| c.protocol_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let evidence_order = rows
        .iter()
        .map(|c| c.evidence_digest.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|h| h.to_string())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut selected_protocols = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for c in &rows {
        if c.scope != request.scope || !c.policy_allow || !c.local_only {
            blocked.insert(c.claim_id.clone());
            omissions.insert(format!("{}:scope-policy-locality", c.claim_id));
        } else if c.replay_identity != request.replay_identity
            || !c.signed
            || !c.protected_closure
            || !c.replicated
            || !c.independent
            || !matches!(
                c.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(c.claim_id.clone());
            if c.replay_identity != request.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", c.claim_id));
            }
            if !c.signed {
                uncertainty.insert(format!("{}:signature-missing", c.claim_id));
            }
            if !c.protected_closure {
                uncertainty.insert(format!("{}:protected-closure", c.claim_id));
            }
            if !c.replicated {
                uncertainty.insert(format!("{}:replication-unverified", c.claim_id));
            }
            if !c.independent {
                uncertainty.insert(format!("{}:independence-unverified", c.claim_id));
            }
            if !matches!(
                c.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            ) {
                uncertainty.insert(format!("{}:evidence-state", c.claim_id));
            }
        } else {
            selected.insert(c.claim_id.clone());
            selected_protocols.insert(c.protocol_id.clone());
        }
        omissions.extend(
            c.omission_order
                .iter()
                .map(|e| format!("{}:{e}", c.claim_id)),
        );
        if c.negative_result {
            negative.insert(format!("{}:negative-result", c.claim_id));
        }
    }
    let missing_claims = request
        .required_claim_order
        .iter()
        .filter(|id| !claim_order.contains(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_protocols = request
        .required_protocol_order
        .iter()
        .filter(|id| !protocol_order.contains(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    omissions.extend(
        missing_claims
            .iter()
            .map(|id| format!("claim:{id}:missing")),
    );
    omissions.extend(
        missing_protocols
            .iter()
            .map(|id| format!("protocol:{id}:missing")),
    );
    uncertainty.extend(
        request
            .adversarial_events
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.researcher_authorized
        || !request.raw_data_local
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(claim_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:replication-release-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        ReplicationDisposition::Blocked
    } else if selected.is_empty() || !missing_claims.is_empty() || !missing_protocols.is_empty() {
        ReplicationDisposition::Unresolved
    } else {
        ReplicationDisposition::Qualified
    };
    if disposition != ReplicationDisposition::Qualified {
        omissions.insert("request:replication-not-release-ready".into());
    }
    let selected_claim_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_claim_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_claim_order = blocked.into_iter().collect::<Vec<_>>();
    let effects = if disposition == ReplicationDisposition::Qualified {
        vec![format!(
            "view:authorized-research-state:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"study_id":request.study_id,"scope":request.scope,"disposition":disposition,"claim_order":claim_order,"selected_claim_order":selected_claim_order,"unresolved_claim_order":unresolved_claim_order,"blocked_claim_order":blocked_claim_order,"missing_claim_order":missing_claims,"protocol_order":protocol_order,"selected_protocol_order":selected_protocols,"missing_protocol_order":missing_protocols,"evidence_order":evidence_order,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"effect_receipts":effects,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let replication_digest = ContentHash::of_value(&payload)
        .map_err(|e| ReplicationWorkbenchError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("registry-replication:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| ReplicationWorkbenchError::Artifact(e.to_string()))?;
    let strings = |key: &str| {
        payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect::<Vec<String>>()
    };
    let record = ReplicationRecord5 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        disposition,
        claim_order: strings("claim_order"),
        selected_claim_order: strings("selected_claim_order"),
        unresolved_claim_order: strings("unresolved_claim_order"),
        blocked_claim_order: strings("blocked_claim_order"),
        missing_claim_order: strings("missing_claim_order"),
        protocol_order: strings("protocol_order"),
        selected_protocol_order: strings("selected_protocol_order"),
        missing_protocol_order: strings("missing_protocol_order"),
        evidence_order: strings("evidence_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: request.replay_identity.clone(),
        replication_digest,
        artifact,
        effect_receipts: strings("effect_receipts"),
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    record.validate()?;
    Ok(record)
}

pub fn assure_replication_json(value: &serde_json::Value) -> Result<serde_json::Value, String> {
    let request: ClaimAndProtocol1 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid claim and protocol request: {e}"))?;
    serde_json::to_value(assure_replication(&request).map_err(|e| e.to_string())?)
        .map_err(|e| format!("cannot serialize replication record: {e}"))
}
pub fn validate_replication_json(value: &serde_json::Value) -> Result<ReplicationRecord5, String> {
    let record: ReplicationRecord5 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid replication record: {e}"))?;
    record.validate().map_err(|e| e.to_string())?;
    Ok(record)
}
fn validate_request(request: &ClaimAndProtocol1) -> Result<(), ReplicationWorkbenchError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_claim_order.is_empty()
        || request.required_protocol_order.is_empty()
        || request.claims.is_empty()
        || !canonical(&request.required_claim_order)
        || !canonical(&request.required_protocol_order)
        || !canonical(&request.adversarial_events)
        || !digest(&request.replay_identity)
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "replication request identity, closure, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for c in &request.claims {
        if c.claim_id.trim().is_empty()
            || c.protocol_id.trim().is_empty()
            || c.study_id != request.study_id
            || c.scope.trim().is_empty()
            || !ids.insert(c.claim_id.clone())
            || !digest(&c.artifact_digest)
            || !digest(&c.evidence_digest)
            || !digest(&c.provenance_digest)
            || !digest(&c.replay_identity)
            || !canonical(&c.omission_order)
        {
            return Err(invalid(
                "replication claim identity, study, digests, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> ClaimAndProtocol1 {
        let c = |id: &str| ReplicationClaim4 {
            claim_id: id.into(),
            protocol_id: format!("protocol:{id}"),
            study_id: "study-1".into(),
            scope: "scope-1".into(),
            artifact_digest: h(id),
            evidence_digest: h("evidence"),
            provenance_digest: h("provenance"),
            replay_identity: h("replay"),
            evidence_state: EvidenceState::Supported,
            replicated: true,
            independent: true,
            signed: true,
            protected_closure: true,
            policy_allow: true,
            local_only: true,
            omission_order: Vec::new(),
            negative_result: false,
        };
        ClaimAndProtocol1 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "replicate-1".into(),
            study_id: "study-1".into(),
            scope: "scope-1".into(),
            required_claim_order: vec!["claim:a".into(), "claim:b".into()],
            required_protocol_order: vec!["protocol:claim:a".into(), "protocol:claim:b".into()],
            claims: vec![c("claim:a"), c("claim:b")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            researcher_authorized: true,
            raw_data_local: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            replication_workbench_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn qualified_is_deterministic() {
        let r = assure_replication(&request()).unwrap();
        assert_eq!(r.disposition, ReplicationDisposition::Qualified);
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn missing_claim_is_unresolved() {
        let mut q = request();
        q.claims.pop();
        let r = assure_replication(&q).unwrap();
        assert_eq!(r.disposition, ReplicationDisposition::Unresolved);
        assert!(r.missing_claim_order.contains(&"claim:b".into()));
    }
    #[test]
    fn negative_result_is_preserved() {
        let mut q = request();
        q.claims[0].negative_result = true;
        let r = assure_replication(&q).unwrap();
        assert!(r
            .negative_evidence_order
            .contains(&"claim:a:negative-result".into()));
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        let r = assure_replication(&q).unwrap();
        assert_eq!(r.disposition, ReplicationDisposition::Blocked);
    }
    #[test]
    fn replay_mismatch_is_unresolved() {
        let mut q = request();
        q.claims[0].replay_identity = h("other");
        let r = assure_replication(&q).unwrap();
        assert!(r.unresolved_claim_order.contains(&"claim:a".into()));
    }
}
