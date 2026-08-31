//! DevX local single-study evidence-surveillance federated control plane (`AFA-devx-P01-F29`).
//! The control plane orders caller-supplied evidence summaries and emits only digest-bound
//! metadata; retrieval, raw-data movement, and scientific or clinical decisions remain out of scope.
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;
pub const FEATURE_ID: &str = "AFA-devx-P01-F29";
pub const CONTRACT_VERSION: &str =
    "devx-local-single-study-evidence-surveillance-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "DevxEvidenceFeed5@1";
pub const OUTPUT_SCHEMA: &str = "DevxEvidenceControlReceipt8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.devx-evidence-control-receipt-8+json";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevxEvidenceObservation5 {
    pub evidence_id: String,
    pub source_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub relevance_milli: u16,
    pub freshness_milli: u16,
    pub evidence_state: EvidenceState,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub permitted: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub signed: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevxEvidenceFeed5 {
    pub schema_version: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_evidence_order: Vec<String>,
    pub observations: Vec<DevxEvidenceObservation5>,
    pub minimum_relevance_milli: u16,
    pub minimum_freshness_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevxEvidenceDisposition {
    Qualified,
    Partial,
    Blocked,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevxEvidenceArtifact8 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevxEvidenceControlReceipt8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: DevxEvidenceDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub control_digest: ContentHash,
    pub artifact: DevxEvidenceArtifact8,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DevxEvidenceError {
    #[error("invalid DevX evidence control request: {0}")]
    Invalid(String),
    #[error("DevX evidence control artifact failed: {0}")]
    Artifact(String),
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn devx_evidence_surveillance_control_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"devx".into(),consumers:["research developer".into(),"single-study operator".into(),"evidence pipeline steward".into()].into(),behavior:"control deterministic local single-study evidence surveillance over typed summaries with explicit federation-safe receipts".into(),value:"prevents stale, unauthorized, incomparable, or incomplete evidence from becoming a silently accepted research input".into(),inputs:vec![TypedPort{name:"devx_evidence_feed".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"devx_evidence_control_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(),permissions:["read:local-research-artifacts".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
impl DevxEvidenceControlReceipt8 {
    pub fn validate(&self) -> Result<(), DevxEvidenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(DevxEvidenceError::Invalid(
                "evidence identity, locality, candidates, or effects are incomplete".into(),
            ));
        }
        for v in [
            &self.candidate_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(v) {
                return Err(DevxEvidenceError::Invalid(
                    "evidence control ordering is not canonical".into(),
                ));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.missing_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids.len() != self.candidate_order.len() || parts != ids {
            return Err(DevxEvidenceError::Invalid(
                "evidence states do not partition candidates".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.control_digest)
            || self.artifact.content_hash != self.control_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(DevxEvidenceError::Artifact(
                "evidence control digest is invalid".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(DevxEvidenceError::Artifact(
                "evidence control content type is invalid".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            e != "block:unsafe-release" && !e.starts_with("read:local-research-artifacts:")
        }) {
            return Err(DevxEvidenceError::Invalid(
                "effect is outside local read gate".into(),
            ));
        }
        if self.disposition == DevxEvidenceDisposition::Qualified
            && self.effect_receipts != [format!("read:local-research-artifacts:{}", self.study_id)]
        {
            return Err(DevxEvidenceError::Invalid(
                "qualified read effect is invalid".into(),
            ));
        }
        if self.disposition != DevxEvidenceDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(DevxEvidenceError::Invalid(
                "non-qualified evidence must block".into(),
            ));
        }
        Ok(())
    }
}
pub fn control_devx_evidence_surveillance(
    feed: &DevxEvidenceFeed5,
) -> Result<DevxEvidenceControlReceipt8, DevxEvidenceError> {
    if feed.schema_version != INPUT_SCHEMA
        || feed.request_id.trim().is_empty()
        || feed.study_id.trim().is_empty()
        || feed.scope.trim().is_empty()
        || feed.requester.trim().is_empty()
        || feed.purpose.trim().is_empty()
        || feed.semantic_profile.trim().is_empty()
        || feed.required_evidence_order.is_empty()
        || feed.observations.is_empty()
        || feed.minimum_relevance_milli == 0
        || feed.minimum_freshness_milli == 0
        || !canonical(&feed.required_evidence_order)
        || !canonical(&feed.adversarial_events)
        || !digest(&feed.replay_identity)
        || !feed.raw_data_local
        || !feed.aggregate_only
        || feed.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(DevxEvidenceError::Invalid(
            "request identity, bounds, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut rows = feed.observations.clone();
    rows.sort_by(|a, b| {
        b.relevance_milli
            .cmp(&a.relevance_milli)
            .then(a.evidence_id.cmp(&b.evidence_id))
    });
    let candidate_order = feed
        .required_evidence_order
        .iter()
        .cloned()
        .chain(rows.iter().map(|o| o.evidence_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    for o in &rows {
        if o.evidence_id.trim().is_empty()
            || o.study_id != feed.study_id
            || o.semantic_profile.trim().is_empty()
            || !seen.insert(o.evidence_id.clone())
            || !canonical(&o.omission_order)
            || !digest(&o.content_digest)
            || !digest(&o.provenance_digest)
            || !digest(&o.replay_identity)
        {
            return Err(DevxEvidenceError::Invalid(
                "observation identity, ordering, or digest is invalid".into(),
            ));
        }
    }
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for o in &rows {
        omission.extend(
            o.omission_order
                .iter()
                .map(|x| format!("{}:{x}", o.evidence_id)),
        );
        if o.negative_result {
            negative.insert(format!("{}:negative-result", o.evidence_id));
        }
        let hard = !o.permitted
            || !o.local_only
            || !o.aggregate_only
            || !o.signed
            || o.semantic_profile != feed.semantic_profile
            || !feed.policy_allow
            || !feed.protected_closure;
        let soft = o.replay_identity != feed.replay_identity
            || o.relevance_milli < feed.minimum_relevance_milli
            || o.freshness_milli < feed.minimum_freshness_milli
            || !matches!(
                o.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            );
        if hard {
            blocked.insert(o.evidence_id.clone());
        } else if soft {
            unresolved.insert(o.evidence_id.clone());
        } else {
            qualified.insert(o.evidence_id.clone());
        }
    }
    for id in &feed.required_evidence_order {
        if !rows.iter().any(|o| &o.evidence_id == id) {
            missing.insert(id.clone());
            omission.insert(format!("evidence:{id}:missing"));
        }
    }
    uncertainty.extend(
        feed.adversarial_events
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let global = !feed.policy_allow
        || !feed.protected_closure
        || !feed.signed_approval
        || !feed.raw_data_local
        || !feed.aggregate_only
        || !feed.adversarial_events.is_empty();
    if global {
        blocked.extend(
            candidate_order
                .iter()
                .filter(|id| rows.iter().any(|o| &o.evidence_id == *id))
                .cloned(),
        );
        qualified.clear();
        unresolved.clear();
        omission.insert("request:surveillance-gate-blocked".into());
    }
    let disposition = if global {
        DevxEvidenceDisposition::Blocked
    } else if unresolved.len() > 0 || !missing.is_empty() || !blocked.is_empty() {
        DevxEvidenceDisposition::Partial
    } else {
        DevxEvidenceDisposition::Qualified
    };
    if disposition != DevxEvidenceDisposition::Qualified {
        omission.insert("request:surveillance-not-release-ready".into());
    }
    let selected_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_order = missing.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == DevxEvidenceDisposition::Qualified {
        vec![format!("read:local-research-artifacts:{}", feed.study_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":feed.request_id,"study_id":feed.study_id,"scope":feed.scope,"requester":feed.requester,"purpose":feed.purpose,"semantic_profile":feed.semantic_profile,"disposition":disposition,"candidate_order":candidate_order,"qualified_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_order":missing_order,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"effect_receipts":effect_receipts,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let d =
        ContentHash::of_value(&payload).map_err(|e| DevxEvidenceError::Artifact(e.to_string()))?;
    let artifact = DevxEvidenceArtifact8 {
        artifact_id: format!("devx-evidence-control:{}", feed.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: d.clone(),
        semantic_loss: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        provenance_digests: rows.iter().map(|o| o.provenance_digest.clone()).collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let out = DevxEvidenceControlReceipt8 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: feed.request_id.clone(),
        study_id: feed.study_id.clone(),
        scope: feed.scope.clone(),
        requester: feed.requester.clone(),
        purpose: feed.purpose.clone(),
        semantic_profile: feed.semantic_profile.clone(),
        disposition,
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_order: payload["qualified_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_order: payload["missing_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        replay_identity: feed.replay_identity.clone(),
        control_digest: d,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn o(id: &str) -> DevxEvidenceObservation5 {
        DevxEvidenceObservation5 {
            evidence_id: id.into(),
            source_id: format!("source:{id}"),
            study_id: "study".into(),
            semantic_profile: "profile:v1".into(),
            relevance_milli: 900,
            freshness_milli: 900,
            evidence_state: EvidenceState::Supported,
            content_digest: h("content"),
            provenance_digest: h("provenance"),
            replay_identity: h("replay"),
            permitted: true,
            local_only: true,
            aggregate_only: true,
            signed: true,
            negative_result: false,
            omission_order: Vec::new(),
        }
    }
    fn q() -> DevxEvidenceFeed5 {
        DevxEvidenceFeed5 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req".into(),
            study_id: "study".into(),
            scope: "scope".into(),
            requester: "dev".into(),
            purpose: "surveil".into(),
            semantic_profile: "profile:v1".into(),
            required_evidence_order: vec!["e:a".into()],
            observations: vec![o("e:a")],
            minimum_relevance_milli: 500,
            minimum_freshness_milli: 500,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            devx_evidence_surveillance_control_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified() {
        assert_eq!(
            control_devx_evidence_surveillance(&q())
                .unwrap()
                .disposition,
            DevxEvidenceDisposition::Qualified
        )
    }
    #[test]
    fn policy_blocks() {
        let mut r = q();
        r.policy_allow = false;
        assert_eq!(
            control_devx_evidence_surveillance(&r).unwrap().disposition,
            DevxEvidenceDisposition::Blocked
        )
    }
    #[test]
    fn negative_preserved() {
        let mut r = q();
        r.observations[0].negative_result = true;
        assert!(control_devx_evidence_surveillance(&r)
            .unwrap()
            .negative_evidence_order
            .contains(&"e:a:negative-result".into()))
    }
    #[test]
    fn deterministic() {
        assert_eq!(
            control_devx_evidence_surveillance(&q()).unwrap(),
            control_devx_evidence_surveillance(&q()).unwrap()
        )
    }
}
