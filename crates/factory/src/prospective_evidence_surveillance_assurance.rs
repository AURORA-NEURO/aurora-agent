//! High-throughput evidence-stream admission and safety assurance.
//!
//! This is a product boundary around the factory queue lifecycle: every feed item is classified,
//! capacity overflow is retained, and an accepted set is released only when freshness, evidence,
//! provenance, scope, replay, policy, and locality gates are complete.

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

pub const FEATURE_ID: &str = "AFA-factory-P01-F27";
pub const CONTRACT_VERSION: &str =
    "factory-prospective-high-throughput-evidence-surveillance-assurance/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.factory-qualified-evidence-set-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedItem4 {
    pub item_id: String,
    pub stream_id: String,
    pub study_id: String,
    pub modality: String,
    pub source_id: String,
    pub scope_id: String,
    pub content_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub observed_at: u64,
    pub relevance_milli: u16,
    pub available: bool,
    pub evidence_state: EvidenceState,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillanceRequest8 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub stream_id: String,
    pub scope_id: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub max_items: usize,
    pub budget_units: usize,
    pub now_epoch: u64,
    pub max_age: u64,
    pub min_relevance_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
    pub items: Vec<EvidenceFeedItem4>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSurveillanceDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet7 {
    pub schema_version: String,
    pub set_id: String,
    pub stream_id: String,
    pub selected_order: Vec<String>,
    pub selected_content_digests: Vec<ContentHash>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillanceReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub stream_id: String,
    pub scope_id: String,
    pub semantic_profile: String,
    pub disposition: EvidenceSurveillanceDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub study_order: Vec<String>,
    pub selected_study_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub qualified_set: QualifiedEvidenceSet7,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceSurveillanceError {
    #[error("invalid evidence surveillance request or receipt: {0}")]
    Invalid(String),
    #[error("evidence surveillance artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> EvidenceSurveillanceError {
    EvidenceSurveillanceError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl EvidenceSurveillanceReceipt9 {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.qualified_set.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.stream_id.trim().is_empty()
            || self.scope_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "evidence identity, closure, locality, axes, or effects are incomplete",
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.overflow_order,
            &self.study_order,
            &self.selected_study_order,
            &self.missing_study_order,
            &self.modality_order,
            &self.selected_modality_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
            &self.qualified_set.selected_order,
            &self.qualified_set.omission_order,
            &self.qualified_set.uncertainty_order,
            &self.qualified_set.negative_evidence_order,
        ] {
            if !canonical(values) {
                return Err(invalid("evidence ordering is not canonical"));
            }
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.overflow_order)
            .cloned()
            .collect::<Vec<_>>();
        if candidates.len() != self.candidate_order.len()
            || parts.len() != candidates.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != candidates
        {
            return Err(invalid("evidence states do not form a complete partition"));
        }
        let studies = self.study_order.iter().cloned().collect::<BTreeSet<_>>();
        let study_parts = self
            .selected_study_order
            .iter()
            .chain(&self.missing_study_order)
            .cloned()
            .collect::<Vec<_>>();
        if studies.len() != self.study_order.len()
            || study_parts.len() != studies.len()
            || study_parts.iter().cloned().collect::<BTreeSet<_>>() != studies
        {
            return Err(invalid("study states do not form a complete partition"));
        }
        let modalities = self.modality_order.iter().cloned().collect::<BTreeSet<_>>();
        let modality_parts = self
            .selected_modality_order
            .iter()
            .chain(&self.missing_modality_order)
            .cloned()
            .collect::<Vec<_>>();
        if modalities.len() != self.modality_order.len()
            || modality_parts.len() != modalities.len()
            || modality_parts.iter().cloned().collect::<BTreeSet<_>>() != modalities
        {
            return Err(invalid("modality states do not form a complete partition"));
        }
        if !digest(&self.evidence_digest)
            || !digest(&self.provenance_digest)
            || !digest(&self.replay_identity)
            || self.artifact.content_hash != self.evidence_digest
            || self.artifact.content_type != CONTENT_TYPE
        {
            return Err(invalid("evidence or provenance digest is inconsistent"));
        }
        if self.qualified_set.selected_order != self.selected_order {
            return Err(invalid("qualified evidence set linkage is inconsistent"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-evidence:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside evidence surveillance gate"));
        }
        if self.disposition == EvidenceSurveillanceDisposition::Qualified
            && self.effect_receipts != [format!("read:local-evidence:{}", self.stream_id)]
        {
            return Err(invalid("qualified evidence effect is invalid"));
        }
        if self.disposition != EvidenceSurveillanceDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified evidence surveillance must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, EvidenceSurveillanceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?,
        )
        .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))
    }
}

pub fn prospective_evidence_surveillance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "factory".into(), consumers: ["agent developer".into(), "researcher".into(), "queue operator".into()].into(), behavior: "verifies and admits bounded prospective evidence feeds into qualified evidence-set release receipts".into(), value: "preserves high-throughput omissions, stale evidence, contradictions, and overflow instead of silently dropping research signals".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into(), "evaluate:evidence-feed".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn assure_prospective_evidence_surveillance(
    request: &EvidenceSurveillanceRequest8,
) -> Result<EvidenceSurveillanceReceipt9, EvidenceSurveillanceError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.stream_id.trim().is_empty()
        || request.scope_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_study_order.is_empty()
        || !canonical(&request.required_study_order)
        || request.required_modality_order.is_empty()
        || !canonical(&request.required_modality_order)
        || request.max_items == 0
        || request.budget_units == 0
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.items.is_empty()
    {
        return Err(invalid("evidence request identity, axes, capacity, replay, locality, boundary, or feed is invalid"));
    }
    let mut items = request.items.clone();
    items.sort_by(|left, right| left.item_id.cmp(&right.item_id));
    let mut ids = BTreeSet::new();
    for item in &items {
        if item.item_id.trim().is_empty()
            || item.stream_id != request.stream_id
            || item.scope_id != request.scope_id
            || item.study_id.trim().is_empty()
            || item.modality.trim().is_empty()
            || item.source_id.trim().is_empty()
            || !ids.insert(item.item_id.clone())
        {
            return Err(invalid(
                "feed item identity, stream, scope, or uniqueness is invalid",
            ));
        }
    }
    let candidate_order = items
        .iter()
        .map(|item| item.item_id.clone())
        .collect::<Vec<_>>();
    let admission_limit = request.max_items.min(request.budget_units);
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut overflow = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut selected_digests = BTreeMap::new();
    for (index, item) in items.iter().enumerate() {
        if index >= admission_limit {
            overflow.insert(item.item_id.clone());
            omissions.insert(format!("item:{}:capacity-overflow", item.item_id));
            continue;
        }
        let mut state = "selected";
        if !request.policy_allow
            || !request.protected_closure
            || !request.raw_data_local
            || !request.aggregate_only
        {
            state = "blocked";
            omissions.insert(format!("item:{}:policy-closure-locality", item.item_id));
        } else if !item.available {
            state = "unresolved";
            omissions.insert(format!("item:{}:unavailable", item.item_id));
        } else if item.observed_at > request.now_epoch
            || request.now_epoch.saturating_sub(item.observed_at) > request.max_age
        {
            state = "unresolved";
            uncertainty.insert(format!("item:{}:stale", item.item_id));
        } else if item.relevance_milli < request.min_relevance_milli {
            state = "unresolved";
            uncertainty.insert(format!("item:{}:relevance-below-threshold", item.item_id));
        } else if item.content_digest.is_none() || item.provenance_digest.is_none() {
            state = "unresolved";
            omissions.insert(format!("item:{}:digest-missing", item.item_id));
        } else if matches!(
            item.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            state = "unresolved";
            uncertainty.insert(format!("item:{}:evidence-not-asserted", item.item_id));
        } else if item.evidence_state == EvidenceState::Contradicted {
            state = "blocked";
            negative.insert(format!("item:{}:contradicted", item.item_id));
        }
        match state {
            "selected" => {
                selected.insert(item.item_id.clone());
                selected_digests.insert(
                    item.item_id.clone(),
                    item.content_digest.clone().expect("digest checked"),
                );
                if item.negative_result {
                    negative.insert(format!("item:{}:negative-result", item.item_id));
                }
            }
            "unresolved" => {
                unresolved.insert(item.item_id.clone());
            }
            _ => {
                blocked.insert(item.item_id.clone());
            }
        }
    }
    let required_studies = request
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_modalities = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let study_order = required_studies
        .iter()
        .chain(items.iter().map(|item| &item.study_id))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = required_modalities
        .iter()
        .chain(items.iter().map(|item| &item.modality))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut selected_study_order = study_order
        .iter()
        .filter(|axis| {
            items
                .iter()
                .any(|item| item.study_id == **axis && selected.contains(&item.item_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut selected_modality_order = modality_order
        .iter()
        .filter(|axis| {
            items
                .iter()
                .any(|item| item.modality == **axis && selected.contains(&item.item_id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_study_order = study_order
        .iter()
        .filter(|axis| required_studies.contains(*axis) && !selected_study_order.contains(axis))
        .cloned()
        .collect::<Vec<_>>();
    let missing_modality_order = modality_order
        .iter()
        .filter(|axis| {
            required_modalities.contains(*axis) && !selected_modality_order.contains(axis)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !request.raw_data_local || !request.aggregate_only {
        omissions.insert("control:locality-or-aggregate-only-failed".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{}", event)),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        overflow.clear();
        selected_study_order.clear();
        selected_modality_order.clear();
        omissions.insert("control:release-gate-blocked".into());
    }
    let disposition = if global_block || !blocked.is_empty() {
        EvidenceSurveillanceDisposition::Blocked
    } else if selected.is_empty()
        || !unresolved.is_empty()
        || !overflow.is_empty()
        || !missing_study_order.is_empty()
        || !missing_modality_order.is_empty()
    {
        EvidenceSurveillanceDisposition::Unresolved
    } else {
        EvidenceSurveillanceDisposition::Qualified
    };
    if disposition != EvidenceSurveillanceDisposition::Qualified {
        omissions.insert("control:evidence-set-not-qualified".into());
    }
    let effects = if disposition == EvidenceSurveillanceDisposition::Qualified {
        vec![format!("read:local-evidence:{}", request.stream_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>();
    let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>();
    let negative_vec = negative.iter().cloned().collect::<Vec<_>>();
    let provenance_digest = ContentHash::of_value(&json!({"selected": selected_order, "items": items.iter().map(|item| (&item.item_id, &item.provenance_digest)).collect::<Vec<_>>() })).map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let qualified_set = QualifiedEvidenceSet7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("qualified-evidence-set-7:{}", request.stream_id),
        stream_id: request.stream_id.clone(),
        selected_order: selected.iter().cloned().collect(),
        selected_content_digests: selected
            .iter()
            .filter_map(|id| selected_digests.get(id).cloned())
            .collect(),
        omission_order: omissions_vec.clone(),
        uncertainty_order: uncertainty_vec.clone(),
        negative_evidence_order: negative_vec.clone(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&qualified_set)
        .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let evidence_digest = ContentHash::of_value(&payload)
        .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let receipt = EvidenceSurveillanceReceipt9 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        researcher: request.researcher.clone(),
        stream_id: request.stream_id.clone(),
        scope_id: request.scope_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        candidate_order,
        selected_order: selected.iter().cloned().collect(),
        unresolved_order: unresolved.iter().cloned().collect(),
        blocked_order: blocked.iter().cloned().collect(),
        overflow_order: overflow.iter().cloned().collect(),
        study_order,
        selected_study_order,
        missing_study_order,
        modality_order,
        selected_modality_order,
        missing_modality_order,
        omission_order: omissions_vec,
        uncertainty_order: uncertainty_vec,
        negative_evidence_order: negative_vec,
        evidence_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        effect_receipts: effects,
        qualified_set,
        artifact,
        raw_data_local: request.raw_data_local,
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
    fn request() -> EvidenceSurveillanceRequest8 {
        let item = |id: &str, study: &str, modality: &str| EvidenceFeedItem4 {
            item_id: id.into(),
            stream_id: "stream-1".into(),
            study_id: study.into(),
            modality: modality.into(),
            source_id: format!("source-{id}"),
            scope_id: "scope-1".into(),
            content_digest: Some(hash(id)),
            provenance_digest: Some(hash(&format!("prov-{id}"))),
            observed_at: 90,
            relevance_milli: 900,
            available: true,
            evidence_state: EvidenceState::Supported,
            negative_result: false,
        };
        EvidenceSurveillanceRequest8 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request-1".into(),
            researcher: "agent developer".into(),
            stream_id: "stream-1".into(),
            scope_id: "scope-1".into(),
            semantic_profile: "evidence-v1".into(),
            required_study_order: vec!["study-a".into(), "study-b".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            max_items: 4,
            budget_units: 4,
            now_epoch: 100,
            max_age: 20,
            min_relevance_milli: 700,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
            items: vec![
                item("item-b", "study-b", "omics"),
                item("item-a", "study-a", "imaging"),
            ],
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            prospective_evidence_surveillance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn complete_feed_qualifies() {
        let receipt = assure_prospective_evidence_surveillance(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Qualified
        );
    }
    #[test]
    fn overflow_is_unresolved() {
        let mut value = request();
        value.max_items = 1;
        let receipt = assure_prospective_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Unresolved
        );
        assert_eq!(receipt.overflow_order.len(), 1);
    }
    #[test]
    fn stale_is_unresolved() {
        let mut value = request();
        value.items[0].observed_at = 10;
        let receipt = assure_prospective_evidence_surveillance(&value).unwrap();
        assert!(receipt
            .uncertainty_order
            .iter()
            .any(|item| item.contains("stale")));
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut value = request();
        value.items[0].evidence_state = EvidenceState::Unknown;
        let receipt = assure_prospective_evidence_surveillance(&value).unwrap();
        assert!(receipt.unresolved_order.contains(&"item-b".to_string()));
    }
    #[test]
    fn contradiction_blocks() {
        let mut value = request();
        value.items[0].evidence_state = EvidenceState::Contradicted;
        let receipt = assure_prospective_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Blocked
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = assure_prospective_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Blocked
        );
    }
    #[test]
    fn receipt_digest_is_stable() {
        let first = assure_prospective_evidence_surveillance(&request()).unwrap();
        let second = assure_prospective_evidence_surveillance(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}
