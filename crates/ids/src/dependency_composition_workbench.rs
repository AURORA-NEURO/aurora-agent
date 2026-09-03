//! Multimodal dependency-composition workbench (`AFA-ids-P27-F18`).
//!
//! The workbench composes caller-declared capability manifests without loading
//! implementations or moving raw imaging/omics data. Missing providers,
//! incompatible profiles, and unsafe components remain explicit in the receipt.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P27-F18";
pub const CONTRACT_VERSION: &str = "ids-multimodal-dependency-composition-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "IdsCompositionRequest7@1";
pub const OUTPUT_SCHEMA: &str = "IdsCompositionReceipt9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-composition-receipt-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CANDIDATES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCompositionCandidate8 {
    pub candidate_id: String,
    pub capability_id: String,
    pub provider_id: String,
    pub semantic_profile: String,
    pub modality_order: Vec<String>,
    pub study_order: Vec<String>,
    pub requires: Vec<String>,
    pub artifact_digests: Vec<ContentHash>,
    pub provenance_digest: ContentHash,
    pub evidence_state: CompositionEvidenceState,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCompositionRequest7 {
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_capability: String,
    pub required_modalities: Vec<String>,
    pub required_studies: Vec<String>,
    pub candidates: Vec<IdsCompositionCandidate8>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCompositionReceipt9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCompositionReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_capability: String,
    pub required_modality_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub study_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub reasons: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub composition_digest: ContentHash,
    pub artifact: IdsCompositionReceipt9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DependencyCompositionError {
    #[error("invalid IDS dependency-composition request: {0}")]
    Invalid(String),
    #[error("IDS dependency-composition receipt failed validation: {0}")]
    Receipt(String),
}

pub fn dependency_composition_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["context compiler engineer", "formal methods researcher", "workbench operator"],
        "behavior": "compose typed capability dependencies for comparable imaging and omics studies with deterministic provider ranking",
        "value": "makes missing capabilities, dependency gaps, semantic mismatch, and protected omissions visible before a workflow is admitted",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["view:ids-composition", "manage:local-capability"],
        "permissions": ["read:local-capability-manifests", "request:dependency-composition"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn ordered_hashes(values: &[ContentHash]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

impl IdsCompositionReceipt9 {
    pub fn validate(&self) -> Result<(), DependencyCompositionError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.required_capability.trim().is_empty()
            || self.required_modality_order.is_empty()
            || self.required_study_order.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(DependencyCompositionError::Receipt(
                "composition identity, requirements, candidates, effects, locality, or disposition is incomplete".into(),
            ));
        }
        for values in [
            &self.required_modality_order,
            &self.required_study_order,
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_capability_order,
            &self.dependency_order,
            &self.modality_order,
            &self.study_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.reasons,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(DependencyCompositionError::Receipt(
                    "composition ordering is not canonical".into(),
                ));
            }
        }
        if !ordered_hashes(&self.artifact_order) {
            return Err(DependencyCompositionError::Receipt(
                "composition artifact ordering is not canonical".into(),
            ));
        }
        let ids = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
        {
            return Err(DependencyCompositionError::Receipt(
                "candidate states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.composition_digest)
            || self.artifact.content_hash != self.composition_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(DependencyCompositionError::Receipt(
                "composition digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:ids-composition:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(DependencyCompositionError::Receipt(
                "effect is outside the governed composition gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, DependencyCompositionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| DependencyCompositionError::Receipt(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| DependencyCompositionError::Receipt(error.to_string()))
    }
}

fn validate_request(request: &IdsCompositionRequest7) -> Result<(), DependencyCompositionError> {
    if request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_capability.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.required_studies.is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(DependencyCompositionError::Invalid(
            "composition identity, requirements, candidate bound, replay, locality, or boundary is invalid".into(),
        ));
    }
    for values in [&request.required_modalities, &request.required_studies] {
        if values.iter().any(|value| value.trim().is_empty())
            || BTreeSet::from_iter(values.iter().cloned()).len() != values.len()
        {
            return Err(DependencyCompositionError::Invalid(
                "required modalities and studies must be unique and non-empty".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || !ids.insert(candidate.candidate_id.clone())
            || candidate.capability_id.trim().is_empty()
            || candidate.provider_id.trim().is_empty()
            || candidate.semantic_profile.trim().is_empty()
            || candidate
                .modality_order
                .iter()
                .any(|value| value.trim().is_empty())
            || candidate
                .study_order
                .iter()
                .any(|value| value.trim().is_empty())
            || candidate
                .requires
                .iter()
                .any(|value| value.trim().is_empty())
            || candidate
                .artifact_digests
                .iter()
                .any(|digest| !valid_digest(digest))
            || !valid_digest(&candidate.provenance_digest)
            || !valid_digest(&candidate.replay_identity)
            || !candidate.local
            || !candidate.aggregate_only
        {
            return Err(DependencyCompositionError::Invalid(format!(
                "candidate {} is invalid, duplicated, non-local, or not digest-bound",
                candidate.candidate_id
            )));
        }
    }
    Ok(())
}

pub fn compose_ids_dependencies(
    request: &IdsCompositionRequest7,
) -> Result<IdsCompositionReceipt9, DependencyCompositionError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    let mut providers = BTreeMap::<String, Vec<&str>>::new();
    for candidate in &candidates {
        providers
            .entry(candidate.capability_id.clone())
            .or_default()
            .push(candidate.candidate_id.as_str());
    }
    for ids in providers.values_mut() {
        ids.sort_unstable();
    }
    let mut queue = VecDeque::from([request.required_capability.clone()]);
    let mut seen = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut dependency_order = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    while let Some(capability) = queue.pop_front() {
        if !seen.insert(capability.clone()) {
            continue;
        }
        let Some(provider_id) = providers
            .get(&capability)
            .and_then(|ids| ids.first())
            .copied()
        else {
            missing.insert(capability.clone());
            omissions.insert(format!("capability:{capability}:no-compatible-provider"));
            negative.insert(format!(
                "capability:{capability}:negative-provider-evidence"
            ));
            continue;
        };
        if providers[&capability].len() > 1 {
            uncertainty.insert(format!(
                "capability:{capability}:multiple-providers-ranked-by-candidate-id"
            ));
        }
        let candidate = by_id[provider_id];
        if candidate.semantic_profile != request.semantic_profile {
            unresolved.insert(provider_id.to_owned());
            omissions.insert(format!("candidate:{provider_id}:semantic-profile"));
        } else if candidate.replay_identity != request.replay_identity {
            unresolved.insert(provider_id.to_owned());
            uncertainty.insert(format!("candidate:{provider_id}:replay-identity"));
        } else if !matches!(
            candidate.evidence_state,
            CompositionEvidenceState::Proven | CompositionEvidenceState::Supported
        ) {
            if candidate.evidence_state == CompositionEvidenceState::Contradicted {
                blocked.insert(provider_id.to_owned());
                negative.insert(format!("candidate:{provider_id}:contradicted"));
            } else {
                unresolved.insert(provider_id.to_owned());
                uncertainty.insert(format!("candidate:{provider_id}:evidence-state"));
            }
        } else if !candidate.local || !candidate.aggregate_only {
            blocked.insert(provider_id.to_owned());
            omissions.insert(format!("candidate:{provider_id}:raw-data-locality"));
        } else {
            let mut requirement_gap = false;
            let candidate_modalities = candidate
                .modality_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            for modality in BTreeSet::from_iter(request.required_modalities.iter().cloned())
                .difference(&candidate_modalities)
            {
                requirement_gap = true;
                unresolved.insert(provider_id.to_owned());
                omissions.insert(format!(
                    "candidate:{provider_id}:missing-modality:{modality}"
                ));
            }
            let candidate_studies = candidate
                .study_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            for study in BTreeSet::from_iter(request.required_studies.iter().cloned())
                .difference(&candidate_studies)
            {
                requirement_gap = true;
                unresolved.insert(provider_id.to_owned());
                omissions.insert(format!("candidate:{provider_id}:missing-study:{study}"));
            }
            if !requirement_gap {
                selected.insert(provider_id.to_owned());
                modalities.extend(candidate.modality_order.iter().cloned());
                studies.extend(candidate.study_order.iter().cloned());
                artifacts.extend(candidate.artifact_digests.iter().cloned());
                provenance.insert(candidate.provenance_digest.clone());
            }
        }
        for dependency in &candidate.requires {
            dependency_order.insert(format!("{provider_id}->{dependency}"));
            queue.push_back(dependency.clone());
        }
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_capability_order = missing.into_iter().collect::<Vec<_>>();
    let disposition = if global_block || selected_order.is_empty() && unresolved_order.is_empty() {
        "blocked"
    } else if !blocked_order.is_empty()
        || !unresolved_order.is_empty()
        || !missing_capability_order.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:dependency-composition-not-closed".into());
    }
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let study_order = studies.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let mut effect_order = if disposition == "qualified" {
        vec![
            "manage:local-capability".to_string(),
            "view:ids-composition".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    effect_order.sort();
    let mut effect_receipts = effect_order
        .iter()
        .map(|effect| {
            if effect == "block:unsafe-release" {
                effect.clone()
            } else {
                format!("{effect}:{}", request.request_id)
            }
        })
        .collect::<Vec<_>>();
    effect_receipts.sort();
    let mut reasons = vec![format!(
        "{} required capability evaluated across {} declared candidates",
        request.required_capability,
        candidate_order.len()
    )];
    if !missing_capability_order.is_empty() {
        reasons.push("missing capabilities remain explicit and cannot be composed".into());
    }
    if !unresolved_order.is_empty() {
        reasons.push("semantic, replay, evidence, modality, or study gaps remain visible".into());
    }
    if !blocked_order.is_empty() {
        reasons.push("blocked candidates remain visible and cannot be admitted".into());
    }
    reasons.sort();
    let payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "required_capability": request.required_capability,
        "required_modality_order": BTreeSet::from_iter(request.required_modalities.iter().cloned()).into_iter().collect::<Vec<_>>(),
        "required_study_order": BTreeSet::from_iter(request.required_studies.iter().cloned()).into_iter().collect::<Vec<_>>(),
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_capability_order": missing_capability_order,
        "dependency_order": dependency_order.into_iter().collect::<Vec<_>>(),
        "modality_order": modality_order,
        "study_order": study_order,
        "artifact_order": artifact_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
        "reasons": reasons,
        "effect_order": effect_order,
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let composition_digest = ContentHash::of_value(&payload)
        .map_err(|error| DependencyCompositionError::Receipt(error.to_string()))?;
    let receipt = IdsCompositionReceipt9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        required_capability: request.required_capability.clone(),
        required_modality_order: payload["required_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        required_study_order: payload["required_study_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        disposition: disposition.into(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        missing_capability_order: payload["missing_capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        dependency_order: payload["dependency_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        modality_order: payload["modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        study_order: payload["study_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        artifact_order: payload["artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| serde_json::from_value(v.clone()).ok())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        reasons: payload["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        effect_order: payload["effect_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        replay_identity: request.replay_identity.clone(),
        composition_digest: composition_digest.clone(),
        artifact: IdsCompositionReceipt9Artifact {
            artifact_id: format!("ids-composition-receipt-9:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: composition_digest,
            semantic_loss: payload["omission_order"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(id: &str, capability: &str) -> IdsCompositionCandidate8 {
        IdsCompositionCandidate8 {
            candidate_id: id.into(),
            capability_id: capability.into(),
            provider_id: format!("provider:{id}"),
            semantic_profile: "ome-v1".into(),
            modality_order: vec!["imaging".into(), "omics".into()],
            study_order: vec!["study-1".into()],
            requires: vec![],
            artifact_digests: vec![h(id)],
            provenance_digest: h("provenance"),
            evidence_state: CompositionEvidenceState::Supported,
            replay_identity: h("replay"),
            local: true,
            aggregate_only: true,
        }
    }
    fn request() -> IdsCompositionRequest7 {
        IdsCompositionRequest7 {
            request_id: "ids:composition:req".into(),
            purpose: "compose context".into(),
            semantic_profile: "ome-v1".into(),
            required_capability: "context.compiler".into(),
            required_modalities: vec!["imaging".into(), "omics".into()],
            required_studies: vec!["study-1".into()],
            candidates: vec![candidate("candidate:context", "context.compiler")],
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
    fn manifest_is_a1() {
        assert_eq!(dependency_composition_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn nominal_composition_is_qualified() {
        let receipt = compose_ids_dependencies(&request()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert_eq!(receipt.selected_order, vec!["candidate:context"]);
    }
    #[test]
    fn missing_dependency_is_unresolved() {
        let mut q = request();
        q.candidates[0].requires = vec!["missing.capability".into()];
        let receipt = compose_ids_dependencies(&q).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(!receipt.missing_capability_order.is_empty());
    }
    #[test]
    fn profile_mismatch_is_unresolved() {
        let mut q = request();
        q.candidates[0].semantic_profile = "other".into();
        let receipt = compose_ids_dependencies(&q).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        let receipt = compose_ids_dependencies(&q).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn composition_digest_is_deterministic() {
        let first = compose_ids_dependencies(&request()).unwrap();
        let second = compose_ids_dependencies(&request()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}
