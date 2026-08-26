//! Multimodal multi-study evidence-surveillance research copilot.
//!
//! Atlas feature: `AFA-adapter-P01-F10`. This A2 agent surface makes semantic
//! comparability, study×modality closure, signed approval, and declared-tool
//! invocation part of the product contract.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceAvailability, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F10";
pub const CONTRACT_VERSION: &str = "adapter-multimodal-evidence-surveillance-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalCopilotEvidenceObservation {
    pub source_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub source_type: String,
    pub locator: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceResearchCopilotRequest {
    pub request_id: String,
    pub agent_id: String,
    pub semantic_profile: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub approval_reference: Option<String>,
    pub approval_granted: bool,
    pub observations: Vec<MultimodalCopilotEvidenceObservation>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalResearchCopilotDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalCopilotQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub semantic_profile: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub incomparable_order: Vec<String>,
    pub missing_cell_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub evidence_state: EvidenceState,
    pub tool_mode: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceResearchCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub semantic_profile: String,
    pub dry_run: bool,
    pub approval_granted: bool,
    pub requested_tool: String,
    pub disposition: MultimodalResearchCopilotDisposition,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub missing_cell_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub capability_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub run_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub tool_receipts: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: MultimodalCopilotQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalEvidenceSurveillanceResearchCopilotError {
    #[error("invalid multimodal research copilot request: {0}")]
    Invalid(String),
    #[error("multimodal research copilot artifact failed: {0}")]
    Artifact(String),
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl MultimodalEvidenceSurveillanceResearchCopilotReceipt {
    pub fn validate(&self) -> Result<(), MultimodalEvidenceSurveillanceResearchCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.requested_tool.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.semantic_profile != self.semantic_profile
        {
            return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid("multimodal copilot identity, closure, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.incomparable_order,
            &self.missing_cell_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.tool_receipts,
            &self.effect_receipts,
            &self.qualified_set.study_order,
            &self.qualified_set.modality_order,
            &self.qualified_set.selected_order,
            &self.qualified_set.incomparable_order,
            &self.qualified_set.missing_cell_order,
            &self.qualified_set.negative_order,
            &self.qualified_set.omissions,
            &self.qualified_set.uncertainty,
        ] {
            if !sorted_unique(values) {
                return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
                    "multimodal copilot ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .chain(self.missing_cell_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self
                .incomparable_order
                .iter()
                .any(|id| !self.denied_order.contains(id))
            || self.qualified_set.selected_order != self.selected_order
        {
            return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
                "multimodal copilot states do not partition candidates".into(),
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.capability_digest,
            &self.comparability_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.run_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
                    "multimodal copilot digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("dry-run:bounded-tool:")
                && !effect.starts_with("invoke:declared-tool:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
                "multimodal copilot effect is outside declared-tool gate".into(),
            ));
        }
        if self.disposition == MultimodalResearchCopilotDisposition::Blocked
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
                "blocked multimodal copilot must be explicitly blocked".into(),
            ));
        }
        if self.dry_run
            && self
                .effect_receipts
                .iter()
                .any(|effect| effect.starts_with("invoke:"))
        {
            return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
                "dry-run multimodal copilot cannot invoke tools".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })
    }
}

pub fn multimodal_evidence_surveillance_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["preclinical researcher".into(), "MCP tool host".into(), "multimodal schema steward".into()].into(), behavior: "runs an A2 multimodal evidence-surveillance copilot with declared tools, study×modality closure, comparability, approval, and replay receipts".into(), value: "automates comparable imaging/omics evidence alerts without hiding missing cells, semantic mismatch, or unauthorized effects".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["invoke:declared-tools".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "MCP 2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_multimodal_evidence_surveillance_research_copilot(
    request: &MultimodalEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    MultimodalEvidenceSurveillanceResearchCopilotReceipt,
    MultimodalEvidenceSurveillanceResearchCopilotError,
> {
    if request.request_id.trim().is_empty()
        || request.agent_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_studies.len() < 2
        || request.required_modalities.len() < 2
        || request.declared_tools.is_empty()
        || request.requested_tool.trim().is_empty()
        || request.max_tool_calls == 0
        || request.observations.is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid("multimodal copilot identity, study/modality closure, tools, observations, replay, locality, or boundary is invalid".into()));
    }
    let declared_tools = request
        .declared_tools
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if declared_tools.len() != request.declared_tools.len()
        || declared_tools.iter().any(|tool| tool.trim().is_empty())
        || !declared_tools.contains(&request.requested_tool)
    {
        return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
            "requested tool must be declared exactly once".into(),
        ));
    }
    let mut studies = request.required_studies.clone();
    studies.sort();
    studies.dedup();
    let mut modalities = request.required_modalities.clone();
    modalities.sort();
    modalities.dedup();
    if studies.len() < 2 || modalities.len() < 2 {
        return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
            "study and modality identities must be unique and non-empty".into(),
        ));
    }
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    let observation_ids = observations
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    if observation_ids.windows(2).any(|pair| pair[0] == pair[1])
        || observations.iter().any(|item| {
            item.source_id.trim().is_empty()
                || item.study_id.trim().is_empty()
                || item.modality.trim().is_empty()
        })
    {
        return Err(MultimodalEvidenceSurveillanceResearchCopilotError::Invalid(
            "multimodal observation identities must be unique and non-empty".into(),
        ));
    }
    let required_cells = studies
        .iter()
        .flat_map(|study| {
            modalities
                .iter()
                .map(move |modality| format!("{}::{}::required", study, modality))
        })
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut selected_digest_map = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut missing_cells = required_cells.clone();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for item in &observations {
        let key = item.source_id.clone();
        if !studies.contains(&item.study_id)
            || !modalities.contains(&item.modality)
            || item.locator.trim().is_empty()
            || item.source_type.trim().is_empty()
            || !request.policy_allow
            || !request.protected_closure
        {
            denied.insert(key.clone());
            omissions.insert(format!("source:{}:scope-policy-closure", key));
        } else if item.semantic_profile != request.semantic_profile {
            denied.insert(key.clone());
            incomparable.insert(key.clone());
            omissions.insert(format!("source:{}:semantic-profile-mismatch", key));
        } else if item.availability != EvidenceAvailability::Available {
            unresolved.insert(key.clone());
            omissions.insert(format!(
                "source:{}:availability-{:?}",
                key, item.availability
            ));
        } else if item.relevance_score < request.min_relevance_score {
            unresolved.insert(key.clone());
            uncertainty.insert(format!("source:{}:relevance-below-threshold", key));
        } else if item.digest.is_none() {
            unresolved.insert(key.clone());
            omissions.insert(format!("source:{}:content-digest-missing", key));
        } else if matches!(
            item.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(key.clone());
            uncertainty.insert(format!("source:{}:unknown-not-asserted", key));
        } else if item.evidence_state == EvidenceState::Contradicted {
            denied.insert(key.clone());
            negative.insert(format!("source:{}:contradicted", key));
        } else {
            selected.insert(key.clone());
            selected_digest_map.insert(key.clone(), item.digest.clone().expect("digest checked"));
            missing_cells.remove(&format!("{}::{}::required", item.study_id, item.modality));
            if item.negative_result {
                negative.insert(format!("source:{}:negative-result", key));
            }
        }
    }
    for cell in &missing_cells {
        unresolved.insert(cell.clone());
        omissions.insert(format!("cell:{}:required-not-qualified", cell));
        uncertainty.insert(format!("cell:{}:missing-modality", cell));
    }
    // Satisfied cells are represented by their selected source; retain only
    // unresolved cells in the candidate partition.
    let mut candidate_set = observation_ids.iter().cloned().collect::<BTreeSet<_>>();
    candidate_set.extend(missing_cells.iter().cloned());
    let candidate_order = candidate_set.iter().cloned().collect::<Vec<_>>();
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
    }
    let approval_missing = !request.dry_run
        && (!request.approval_granted
            || request
                .approval_reference
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty());
    if approval_missing {
        omissions.insert("control:signed-approval-required".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || approval_missing
    {
        MultimodalResearchCopilotDisposition::Blocked
    } else if selected.is_empty() {
        MultimodalResearchCopilotDisposition::Unknown
    } else if !unresolved.is_empty() || !denied.is_empty() || !missing_cells.is_empty() {
        MultimodalResearchCopilotDisposition::Partial
    } else {
        MultimodalResearchCopilotDisposition::Completed
    };
    let study_order = studies.clone();
    let modality_order = modalities.clone();
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let incomparable_order = incomparable.iter().cloned().collect::<Vec<_>>();
    let missing_cell_order = missing_cells.iter().cloned().collect::<Vec<_>>();
    let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>();
    let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>();
    let negative_vec = negative.iter().cloned().collect::<Vec<_>>();
    let tool_receipts = if disposition == MultimodalResearchCopilotDisposition::Blocked {
        vec![format!("tool:{}:denied", request.requested_tool)]
    } else if request.dry_run {
        vec![format!("tool:{}:dry-run", request.requested_tool)]
    } else {
        vec![format!(
            "tool:{}:bounded-call:1/{}",
            request.requested_tool, request.max_tool_calls
        )]
    };
    let capability_digest = ContentHash::of_value(&json!({"agent_id": request.agent_id, "declared_tools": request.declared_tools, "requested_tool": request.requested_tool, "max_tool_calls": request.max_tool_calls, "dry_run": request.dry_run})).map_err(|error| MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let comparability_digest = ContentHash::of_value(&json!({"semantic_profile": request.semantic_profile, "study_order": study_order.clone(), "modality_order": modality_order.clone(), "incomparable_order": incomparable_order.clone()})).map_err(|error| MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let evidence_digest = ContentHash::of_value(&json!({"candidate_order": candidate_order.clone(), "selected_order": selected_order.clone(), "unresolved_order": unresolved_order.clone(), "denied_order": denied_order.clone(), "missing_cell_order": missing_cell_order.clone()})).map_err(|error| MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "agent_id": request.agent_id, "replay_identity": request.replay_identity, "capability_digest": capability_digest, "comparability_digest": comparability_digest, "evidence_digest": evidence_digest})).map_err(|error| MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let run_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "dry_run": request.dry_run, "approval_reference": request.approval_reference, "tool_receipts": tool_receipts.clone(), "provenance_digest": provenance_digest})).map_err(|error| MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let qualified_set = MultimodalCopilotQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!(
            "qualified-evidence-multimodal-copilot:{}",
            request.request_id
        ),
        semantic_profile: request.semantic_profile.clone(),
        study_order: study_order.clone(),
        modality_order: modality_order.clone(),
        selected_order: selected_order.clone(),
        selected_digests: selected_order
            .iter()
            .filter_map(|source| selected_digest_map.get(source).cloned())
            .collect(),
        incomparable_order: incomparable_order.clone(),
        missing_cell_order: missing_cell_order.clone(),
        negative_order: negative_vec.clone(),
        omissions: omissions_vec.clone(),
        uncertainty: uncertainty_vec.clone(),
        evidence_state: if disposition == MultimodalResearchCopilotDisposition::Completed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        },
        tool_mode: if request.dry_run {
            "dry_run".into()
        } else {
            "bounded_invocation".into()
        },
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&qualified_set).map_err(|error| {
        MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-evidence-set3+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        MultimodalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let receipt = MultimodalEvidenceSurveillanceResearchCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        agent_id: request.agent_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        dry_run: request.dry_run,
        approval_granted: request.approval_granted,
        requested_tool: request.requested_tool.clone(),
        disposition,
        study_order,
        modality_order,
        candidate_order,
        selected_order,
        unresolved_order,
        denied_order,
        incomparable_order,
        missing_cell_order,
        replay_identity: request.replay_identity.clone(),
        capability_digest,
        comparability_digest,
        evidence_digest,
        provenance_digest,
        run_digest,
        omissions: omissions_vec,
        uncertainty: uncertainty_vec,
        negative_evidence: negative_vec,
        tool_receipts,
        effect_receipts: if disposition == MultimodalResearchCopilotDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else if request.dry_run {
            vec![format!("dry-run:bounded-tool:{}", request.agent_id)]
        } else {
            vec![format!("invoke:declared-tool:{}", request.agent_id)]
        },
        qualified_set,
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
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> MultimodalEvidenceSurveillanceResearchCopilotRequest {
        let digest = hash("multimodal-copilot");
        let observation =
            |id: &str, study: &str, modality: &str| MultimodalCopilotEvidenceObservation {
                source_id: id.into(),
                study_id: study.into(),
                modality: modality.into(),
                semantic_profile: "profile:v1".into(),
                source_type: "imaging".into(),
                locator: format!("local://{id}"),
                digest: Some(digest.clone()),
                availability: EvidenceAvailability::Available,
                evidence_state: EvidenceState::Supported,
                relevance_score: 90,
                negative_result: false,
            };
        MultimodalEvidenceSurveillanceResearchCopilotRequest {
            request_id: "request:multimodal-copilot".into(),
            agent_id: "agent:multimodal-copilot".into(),
            semantic_profile: "profile:v1".into(),
            required_studies: vec!["study:a".into(), "study:b".into()],
            required_modalities: vec!["imaging".into(), "omics".into()],
            declared_tools: vec!["evidence.compare".into()],
            requested_tool: "evidence.compare".into(),
            max_tool_calls: 2,
            dry_run: true,
            approval_reference: None,
            approval_granted: false,
            observations: vec![
                observation("source:a", "study:a", "imaging"),
                observation("source:b", "study:a", "omics"),
                observation("source:c", "study:b", "imaging"),
                observation("source:d", "study:b", "omics"),
            ],
            min_relevance_score: 70,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: digest,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            multimodal_evidence_surveillance_research_copilot_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn complete_matrix_dry_runs() {
        let receipt = run_multimodal_evidence_surveillance_research_copilot(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalResearchCopilotDisposition::Completed
        );
        assert!(receipt.effect_receipts[0].starts_with("dry-run:"));
    }
    #[test]
    fn approval_required_for_invocation() {
        let mut value = request();
        value.dry_run = false;
        assert_eq!(
            run_multimodal_evidence_surveillance_research_copilot(&value)
                .unwrap()
                .disposition,
            MultimodalResearchCopilotDisposition::Blocked
        );
    }
    #[test]
    fn approved_invocation_is_declared() {
        let mut value = request();
        value.dry_run = false;
        value.approval_granted = true;
        value.approval_reference = Some("approval:one".into());
        assert!(
            run_multimodal_evidence_surveillance_research_copilot(&value)
                .unwrap()
                .effect_receipts[0]
                .starts_with("invoke:declared-tool:")
        );
    }
    #[test]
    fn missing_cell_is_explicit() {
        let mut value = request();
        value.observations.pop();
        let receipt = run_multimodal_evidence_surveillance_research_copilot(&value).unwrap();
        assert!(receipt
            .missing_cell_order
            .iter()
            .any(|cell| cell.contains("study:b::omics")));
    }
    #[test]
    fn semantic_mismatch_is_incomparable() {
        let mut value = request();
        value.observations[0].semantic_profile = "profile:other".into();
        assert!(
            run_multimodal_evidence_surveillance_research_copilot(&value)
                .unwrap()
                .incomparable_order
                .contains(&"source:a".to_string())
        );
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut value = request();
        value.observations[0].evidence_state = EvidenceState::Unknown;
        assert!(
            run_multimodal_evidence_surveillance_research_copilot(&value)
                .unwrap()
                .uncertainty
                .iter()
                .any(|item| item.contains("unknown-not-asserted"))
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            run_multimodal_evidence_surveillance_research_copilot(&value)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        );
    }
    #[test]
    fn replay_is_stable() {
        let first = run_multimodal_evidence_surveillance_research_copilot(&request()).unwrap();
        let second = run_multimodal_evidence_surveillance_research_copilot(&request()).unwrap();
        assert_eq!(first.run_digest, second.run_digest);
    }
}
