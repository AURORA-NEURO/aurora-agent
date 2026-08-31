//! Local single-study evidence-surveillance research copilot.
//!
//! Atlas feature: `AFA-worldgen-P01-F09`. The copilot is a bounded A1 agent surface:
//! it may invoke only a declared tool, keeps dry-run and effect receipts distinct,
//! and never promotes unknown or incomplete evidence to a qualified conclusion.

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

pub const FEATURE_ID: &str = "AFA-worldgen-P01-F09";
pub const CONTRACT_VERSION: &str = "worldgen-local-evidence-surveillance-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotEvidenceObservation {
    pub source_id: String,
    pub study_id: String,
    pub source_type: String,
    pub locator: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceResearchCopilotRequest {
    pub request_id: String,
    pub agent_id: String,
    pub study_id: String,
    pub intent: String,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub required_source_ids: Vec<String>,
    pub observations: Vec<CopilotEvidenceObservation>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchCopilotDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopilotQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub study_id: String,
    pub intent: String,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub negative_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub evidence_state: EvidenceState,
    pub ordering_rule: String,
    pub tool_mode: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceResearchCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub study_id: String,
    pub intent: String,
    pub dry_run: bool,
    pub requested_tool: String,
    pub disposition: ResearchCopilotDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub capability_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub run_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub tool_receipts: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: CopilotQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalEvidenceSurveillanceResearchCopilotError {
    #[error("invalid research copilot request: {0}")]
    Invalid(String),
    #[error("research copilot artifact failed: {0}")]
    Artifact(String),
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl LocalEvidenceSurveillanceResearchCopilotReceipt {
    pub fn validate(&self) -> Result<(), LocalEvidenceSurveillanceResearchCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.requested_tool.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.study_id != self.study_id
            || self.qualified_set.intent != self.intent
        {
            return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid("copilot identity, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.tool_receipts,
            &self.effect_receipts,
            &self.qualified_set.selected_order,
            &self.qualified_set.negative_order,
            &self.qualified_set.omissions,
            &self.qualified_set.uncertainty,
        ] {
            if !sorted_unique(values) {
                return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
                    "copilot ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self.qualified_set.selected_order != self.selected_order
        {
            return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
                "copilot states do not partition candidates".into(),
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.capability_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.run_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
                    "copilot digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("dry-run:bounded-tool:")
                && !effect.starts_with("invoke:declared-tool:")
                && effect != "block:unsafe-release"
        }) {
            return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
                "copilot effect is outside declared-tool gate".into(),
            ));
        }
        if self.disposition == ResearchCopilotDisposition::Blocked
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
                "blocked copilot must be explicitly blocked".into(),
            ));
        }
        if self.dry_run
            && self
                .effect_receipts
                .iter()
                .any(|effect| effect.starts_with("invoke:"))
        {
            return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
                "dry-run copilot cannot invoke tools".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            LocalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
        })
    }
}

pub fn local_evidence_surveillance_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "worldgen".into(), consumers: ["benchmark curator".into(), "world-generation engineer".into(), "MCP tool host".into()].into(), behavior: "runs a bounded local evidence-surveillance copilot over a structural benchmark world through a declared tool with dry-run, replay, omission, and effect receipts".into(), value: "automates benchmark evidence alerts without hiding unknown evidence, protected closure, hidden-family splits, or unauthorized tool effects".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["invoke:declared-tools".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "MCP 2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_local_evidence_surveillance_research_copilot(
    request: &LocalEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    LocalEvidenceSurveillanceResearchCopilotReceipt,
    LocalEvidenceSurveillanceResearchCopilotError,
> {
    if request.request_id.trim().is_empty()
        || request.agent_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.intent.trim().is_empty()
        || request.declared_tools.is_empty()
        || request.requested_tool.trim().is_empty()
        || request.max_tool_calls == 0
        || request.observations.is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid("copilot identity, tool declaration, observations, replay, locality, or boundary is invalid".into()));
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
        return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
            "requested tool must be declared exactly once".into(),
        ));
    }
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    let candidate = observations
        .iter()
        .map(|observation| observation.source_id.clone())
        .collect::<Vec<_>>();
    if candidate.windows(2).any(|pair| pair[0] == pair[1])
        || candidate.iter().any(|value| value.trim().is_empty())
    {
        return Err(LocalEvidenceSurveillanceResearchCopilotError::Invalid(
            "observation source identities must be unique and non-empty".into(),
        ));
    }
    let mut selected = BTreeSet::new();
    let mut selected_digest_map = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for observation in &observations {
        if observation.study_id != request.study_id
            || observation.locator.trim().is_empty()
            || observation.source_type.trim().is_empty()
            || !request.policy_allow
            || !request.protected_closure
        {
            denied.insert(observation.source_id.clone());
            omissions.insert(format!(
                "source:{}:scope-policy-closure",
                observation.source_id
            ));
        } else if observation.availability != EvidenceAvailability::Available {
            unresolved.insert(observation.source_id.clone());
            omissions.insert(format!(
                "source:{}:availability-{:?}",
                observation.source_id, observation.availability
            ));
        } else if observation.relevance_score < request.min_relevance_score {
            unresolved.insert(observation.source_id.clone());
            uncertainty.insert(format!(
                "source:{}:relevance-below-threshold",
                observation.source_id
            ));
        } else if observation.digest.is_none() {
            unresolved.insert(observation.source_id.clone());
            omissions.insert(format!(
                "source:{}:content-digest-missing",
                observation.source_id
            ));
        } else if matches!(
            observation.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(observation.source_id.clone());
            uncertainty.insert(format!(
                "source:{}:unknown-not-asserted",
                observation.source_id
            ));
        } else if observation.evidence_state == EvidenceState::Contradicted {
            denied.insert(observation.source_id.clone());
            negative.insert(format!("source:{}:contradicted", observation.source_id));
        } else {
            selected.insert(observation.source_id.clone());
            selected_digest_map.insert(
                observation.source_id.clone(),
                observation.digest.clone().expect("digest checked"),
            );
            if observation.negative_result {
                negative.insert(format!("source:{}:negative-result", observation.source_id));
            }
        }
    }
    for required in request.required_source_ids.iter().collect::<BTreeSet<_>>() {
        if !selected.contains(required) {
            omissions.insert(format!("source:{}:required-not-qualified", required));
            uncertainty.insert(format!("source:{}:required-unresolved", required));
        }
    }
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            ResearchCopilotDisposition::Blocked
        } else if selected.is_empty() {
            ResearchCopilotDisposition::Unknown
        } else if !unresolved.is_empty()
            || !denied.is_empty()
            || request
                .required_source_ids
                .iter()
                .any(|required| !selected.contains(required))
        {
            ResearchCopilotDisposition::Partial
        } else {
            ResearchCopilotDisposition::Completed
        };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>();
    let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>();
    let negative_vec = negative.iter().cloned().collect::<Vec<_>>();
    let tool_receipts = if disposition == ResearchCopilotDisposition::Blocked {
        vec![format!("tool:{}:denied", request.requested_tool)]
    } else if request.dry_run {
        vec![format!("tool:{}:dry-run", request.requested_tool)]
    } else {
        vec![format!(
            "tool:{}:bounded-call:1/{}",
            request.requested_tool, request.max_tool_calls
        )]
    };
    let capability_digest = ContentHash::of_value(&json!({"agent_id": request.agent_id, "declared_tools": request.declared_tools, "requested_tool": request.requested_tool, "max_tool_calls": request.max_tool_calls, "dry_run": request.dry_run})).map_err(|error| LocalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let evidence_digest = ContentHash::of_value(&json!({"candidate_order": candidate.clone(), "selected_order": selected_order.clone(), "unresolved_order": unresolved_order.clone(), "denied_order": denied_order.clone()})).map_err(|error| LocalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "agent_id": request.agent_id, "replay_identity": request.replay_identity, "capability_digest": capability_digest, "evidence_digest": evidence_digest})).map_err(|error| LocalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let run_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "dry_run": request.dry_run, "tool_receipts": tool_receipts.clone(), "provenance_digest": provenance_digest})).map_err(|error| LocalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let tool_mode = if request.dry_run {
        "dry_run"
    } else {
        "bounded_invocation"
    };
    let qualified_set = CopilotQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("worldgen-qualified-evidence-copilot:{}", request.request_id),
        study_id: request.study_id.clone(),
        intent: request.intent.clone(),
        selected_order: selected_order.clone(),
        selected_digests: selected_order
            .iter()
            .filter_map(|source| selected_digest_map.get(source).cloned())
            .collect(),
        negative_order: negative_vec.clone(),
        omissions: omissions_vec.clone(),
        uncertainty: uncertainty_vec.clone(),
        evidence_state: if disposition == ResearchCopilotDisposition::Completed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        },
        ordering_rule:
            "relevance_score descending, source_id ascending; artifact digests ascending".into(),
        tool_mode: tool_mode.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&qualified_set).map_err(|error| {
        LocalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.worldgen.qualified-evidence-set3+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| LocalEvidenceSurveillanceResearchCopilotError::Artifact(error.to_string()))?;
    let receipt = LocalEvidenceSurveillanceResearchCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        agent_id: request.agent_id.clone(),
        study_id: request.study_id.clone(),
        intent: request.intent.clone(),
        dry_run: request.dry_run,
        requested_tool: request.requested_tool.clone(),
        disposition,
        candidate_order: candidate,
        selected_order,
        unresolved_order,
        denied_order,
        replay_identity: request.replay_identity.clone(),
        capability_digest,
        evidence_digest,
        provenance_digest,
        run_digest,
        omissions: omissions_vec,
        uncertainty: uncertainty_vec,
        negative_evidence: negative_vec,
        tool_receipts,
        effect_receipts: if disposition == ResearchCopilotDisposition::Blocked {
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
    fn request() -> LocalEvidenceSurveillanceResearchCopilotRequest {
        let digest = hash("copilot");
        let observation = |id: &str, state: EvidenceState| CopilotEvidenceObservation {
            source_id: id.into(),
            study_id: "study:one".into(),
            source_type: "paper".into(),
            locator: format!("local://{id}"),
            digest: Some(digest.clone()),
            availability: EvidenceAvailability::Available,
            evidence_state: state,
            relevance_score: 90,
            negative_result: id == "source:b",
        };
        LocalEvidenceSurveillanceResearchCopilotRequest {
            request_id: "request:copilot".into(),
            agent_id: "agent:research-copilot".into(),
            study_id: "study:one".into(),
            intent: "monitor mechanism".into(),
            declared_tools: vec!["evidence.search".into()],
            requested_tool: "evidence.search".into(),
            max_tool_calls: 2,
            dry_run: true,
            required_source_ids: vec!["source:a".into()],
            observations: vec![
                observation("source:a", EvidenceState::Supported),
                observation("source:b", EvidenceState::Supported),
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
    fn manifest_is_a1() {
        assert_eq!(
            local_evidence_surveillance_research_copilot_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn dry_run_is_complete_without_invocation() {
        let receipt = run_local_evidence_surveillance_research_copilot(&request()).unwrap();
        assert_eq!(receipt.disposition, ResearchCopilotDisposition::Completed);
        assert!(receipt.effect_receipts[0].starts_with("dry-run:"));
    }
    #[test]
    fn bounded_invocation_is_declared() {
        let mut value = request();
        value.dry_run = false;
        let receipt = run_local_evidence_surveillance_research_copilot(&value).unwrap();
        assert!(receipt.effect_receipts[0].starts_with("invoke:declared-tool:"));
    }
    #[test]
    fn undeclared_tool_is_rejected() {
        let mut value = request();
        value.requested_tool = "unsafe.shell".into();
        assert!(run_local_evidence_surveillance_research_copilot(&value).is_err());
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut value = request();
        value.observations[0].evidence_state = EvidenceState::Unknown;
        assert!(run_local_evidence_surveillance_research_copilot(&value)
            .unwrap()
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn contradiction_is_denied() {
        let mut value = request();
        value.observations[0].evidence_state = EvidenceState::Contradicted;
        assert!(run_local_evidence_surveillance_research_copilot(&value)
            .unwrap()
            .denied_order
            .contains(&"source:a".to_string()));
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = run_local_evidence_surveillance_research_copilot(&value).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn replay_is_stable() {
        let first = run_local_evidence_surveillance_research_copilot(&request()).unwrap();
        let second = run_local_evidence_surveillance_research_copilot(&request()).unwrap();
        assert_eq!(first.run_digest, second.run_digest);
    }
}
