//! Prospective high-throughput evidence-surveillance research copilot.
//!
//! Atlas feature `AFA-adapter-P01-F11`: a bounded A2 agent surface for
//! EvidenceFeed3 batches with checkpointed queue identity and explicit
//! overflow, uncertainty, negative evidence, and authorization receipts.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceAvailability, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F11";
pub const CONTRACT_VERSION: &str = "adapter-throughput-evidence-surveillance-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputCopilotEvidenceObservation {
    pub source_id: String,
    pub sequence: u64,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceResearchCopilotRequest {
    pub request_id: String,
    pub agent_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: usize,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub approval_reference: Option<String>,
    pub approval_granted: bool,
    pub observations: Vec<ThroughputCopilotEvidenceObservation>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputResearchCopilotDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputCopilotQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub overflow_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub tool_mode: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceResearchCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: usize,
    pub disposition: ThroughputResearchCopilotDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub run_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub tool_receipts: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: ThroughputCopilotQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ThroughputEvidenceSurveillanceResearchCopilotError {
    #[error("invalid throughput copilot request: {0}")]
    Invalid(String),
    #[error("throughput copilot artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ThroughputEvidenceSurveillanceResearchCopilotReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceResearchCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.batch_id != self.batch_id
            || self.qualified_set.checkpoint_seq != self.checkpoint_seq
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid("identity, checkpoint, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.overflow_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.tool_receipts,
            &self.effect_receipts,
            &self.qualified_set.selected_order,
            &self.qualified_set.overflow_order,
            &self.qualified_set.omissions,
            &self.qualified_set.uncertainty,
            &self.qualified_set.negative_order,
        ] {
            if !ordered(values) {
                return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                    "throughput ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .chain(self.overflow_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self.qualified_set.selected_order != self.selected_order
            || self.qualified_set.overflow_order != self.overflow_order
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput states do not partition candidates".into(),
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.queue_digest,
            &self.checkpoint_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.run_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                    "throughput digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("dry-run:bounded-tool:")
                && !effect.starts_with("invoke:declared-tool:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "throughput effect is outside declared-tool gate".into(),
            ));
        }
        if self.disposition == ThroughputResearchCopilotDisposition::Blocked
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "blocked throughput copilot must be explicitly blocked".into(),
            ));
        }
        if self.disposition == ThroughputResearchCopilotDisposition::Blocked
            && self
                .tool_receipts
                .iter()
                .any(|item| !item.ends_with(":denied"))
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "blocked tool receipt is not denied".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|effect| effect.starts_with("invoke:"))
            && self.qualified_set.tool_mode != "bounded_invocation"
        {
            return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
                "invocation mode is not recorded".into(),
            ));
        }
        Ok(())
    }
}

pub fn throughput_evidence_surveillance_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), title: "Prospective high-throughput evidence surveillance research copilot".into(), description: "Run bounded EvidenceFeed3 batches with checkpointed queue identity, explicit overflow, omission, negative evidence, and signed tool effects.".into(), autonomy_tier: AutonomyTier::A2, determinism: Determinism::Deterministic, inputs: vec![TypedPort::new(INPUT_SCHEMA, "typed prospective evidence batch")], outputs: vec![TypedPort::new(OUTPUT_SCHEMA, "qualified evidence batch")], effects: vec![Effect::LocalRead, Effect::LocalCompute, Effect::LocalWrite], permissions: vec!["invoke:declared-tools".into(), "read:local-evidence".into()], surfaces: vec![ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::Mcp], consumers: vec!["consortium administrator".into(), "MCP host".into(), "queue schema steward".into()], evidence: vec![], boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_throughput_evidence_surveillance_research_copilot(
    request: &ThroughputEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    ThroughputEvidenceSurveillanceResearchCopilotReceipt,
    ThroughputEvidenceSurveillanceResearchCopilotError,
> {
    if request.request_id.trim().is_empty()
        || request.agent_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.capacity == 0
        || request.max_tool_calls == 0
        || request.declared_tools.is_empty()
        || request.requested_tool.trim().is_empty()
        || !request
            .declared_tools
            .iter()
            .any(|tool| tool == &request.requested_tool)
        || request.observations.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            "identity, checkpoint, capacity, tools, observations, locality, or boundary is invalid"
                .into(),
        ));
    }
    if !request
        .replay_identity
        .as_str()
        .chars()
        .all(|c| c.is_ascii_hexdigit())
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            "replay identity must be a 64-character hex digest".into(),
        ));
    }
    let mut observations = request.observations.clone();
    observations.sort_by(|a, b| {
        a.sequence
            .cmp(&b.sequence)
            .then_with(|| a.source_id.cmp(&b.source_id))
    });
    if observations
        .windows(2)
        .any(|pair| pair[0].source_id == pair[1].source_id)
        || observations
            .iter()
            .any(|item| item.source_id.trim().is_empty())
    {
        return Err(ThroughputEvidenceSurveillanceResearchCopilotError::Invalid(
            "observation ids must be unique and non-empty".into(),
        ));
    }
    let candidate_order = observations
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut selected_digests = BTreeMap::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut overflow = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for (index, item) in observations.iter().enumerate() {
        if index >= request.capacity {
            overflow.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:capacity-overflow", item.source_id));
            continue;
        }
        if !request.policy_allow || !request.protected_closure {
            denied.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:policy-or-closure", item.source_id));
        } else if item.availability != EvidenceAvailability::Available {
            unresolved.insert(item.source_id.clone());
            omissions.insert(format!(
                "source:{}:availability-{:?}",
                item.source_id, item.availability
            ));
        } else if item.relevance_score < request.min_relevance_score {
            unresolved.insert(item.source_id.clone());
            uncertainty.insert(format!(
                "source:{}:relevance-below-threshold",
                item.source_id
            ));
        } else if item.digest.is_none() {
            unresolved.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:content-digest-missing", item.source_id));
        } else if matches!(
            item.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(item.source_id.clone());
            uncertainty.insert(format!("source:{}:unknown-not-asserted", item.source_id));
        } else if item.evidence_state == EvidenceState::Contradicted {
            denied.insert(item.source_id.clone());
            negative.insert(format!("source:{}:contradicted", item.source_id));
        } else {
            selected.insert(item.source_id.clone());
            selected_digests.insert(
                item.source_id.clone(),
                item.digest.clone().expect("digest checked"),
            );
            if item.negative_result {
                negative.insert(format!("source:{}:negative-result", item.source_id));
            }
        }
    }
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
        ThroughputResearchCopilotDisposition::Blocked
    } else if selected.is_empty() {
        ThroughputResearchCopilotDisposition::Unknown
    } else if !unresolved.is_empty() || !denied.is_empty() || !overflow.is_empty() {
        ThroughputResearchCopilotDisposition::Partial
    } else {
        ThroughputResearchCopilotDisposition::Completed
    };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let overflow_order = overflow.iter().cloned().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let tool_receipts = if disposition == ThroughputResearchCopilotDisposition::Blocked {
        vec![format!("tool:{}:denied", request.requested_tool)]
    } else if request.dry_run {
        vec![format!("tool:{}:dry-run", request.requested_tool)]
    } else {
        vec![format!(
            "tool:{}:bounded-call:1/{}",
            request.requested_tool, request.max_tool_calls
        )]
    };
    let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "capacity": request.capacity, "candidate_order": candidate_order, "checkpoint_seq": request.checkpoint_seq})).map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "replay_identity": request.replay_identity})).map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let evidence_digest = ContentHash::of_value(&json!({"selected_order": selected_order, "unresolved_order": unresolved_order, "denied_order": denied_order, "overflow_order": overflow_order})).map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "agent_id": request.agent_id, "queue_digest": queue_digest, "checkpoint_digest": checkpoint_digest, "evidence_digest": evidence_digest})).map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let run_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "dry_run": request.dry_run, "tool_receipts": tool_receipts, "provenance_digest": provenance_digest})).map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let qualified_set = ThroughputCopilotQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!(
            "qualified-evidence-throughput-copilot:{}",
            request.request_id
        ),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        selected_order: selected_order.clone(),
        selected_digests: selected_order
            .iter()
            .filter_map(|id| selected_digests.get(id).cloned())
            .collect(),
        overflow_order: overflow_order.clone(),
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_order: negative_evidence.clone(),
        evidence_state: if disposition == ThroughputResearchCopilotDisposition::Completed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        },
        tool_mode: if request.dry_run {
            "dry_run"
        } else {
            "bounded_invocation"
        }
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let artifact_payload = serde_json::to_value(&qualified_set)
        .map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-evidence-set3+json",
        &artifact_payload,
        vec![],
        vec![],
    )
    .map_err(|e| ThroughputEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let receipt = ThroughputEvidenceSurveillanceResearchCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        agent_id: request.agent_id.clone(),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        capacity: request.capacity,
        disposition,
        candidate_order,
        selected_order,
        unresolved_order,
        denied_order,
        overflow_order,
        replay_identity: request.replay_identity.clone(),
        queue_digest,
        checkpoint_digest,
        evidence_digest,
        provenance_digest,
        run_digest,
        omissions,
        uncertainty,
        negative_evidence,
        tool_receipts,
        effect_receipts: if disposition == ThroughputResearchCopilotDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else if request.dry_run {
            vec![format!("dry-run:bounded-tool:{}", request.agent_id)]
        } else {
            vec![format!("invoke:declared-tool:{}", request.agent_id)]
        },
        qualified_set,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(dry_run: bool) -> ThroughputEvidenceSurveillanceResearchCopilotRequest {
        ThroughputEvidenceSurveillanceResearchCopilotRequest {
            request_id: "f11-test".into(),
            agent_id: "agent".into(),
            batch_id: "batch-1".into(),
            checkpoint_seq: 1,
            capacity: 3,
            declared_tools: vec!["evidence.query".into()],
            requested_tool: "evidence.query".into(),
            max_tool_calls: 2,
            dry_run,
            approval_reference: (!dry_run).then(|| "approval-1".into()),
            approval_granted: !dry_run,
            observations: (0..3)
                .map(|sequence| ThroughputCopilotEvidenceObservation {
                    source_id: format!("source-{sequence}"),
                    sequence,
                    digest: Some(ContentHash::of_bytes(&[sequence as u8])),
                    availability: EvidenceAvailability::Available,
                    evidence_state: EvidenceState::Supported,
                    relevance_score: 90,
                    negative_result: false,
                })
                .collect(),
            min_relevance_score: 80,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: ContentHash::of_bytes(&[7]),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            throughput_evidence_surveillance_research_copilot_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn complete_batch_dry_runs() {
        let receipt =
            run_throughput_evidence_surveillance_research_copilot(&request(true)).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputResearchCopilotDisposition::Completed
        );
    }
    #[test]
    fn capacity_overflow_is_explicit() {
        let mut input = request(true);
        input.capacity = 2;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(receipt.overflow_order, vec!["source-2"]);
    }
    #[test]
    fn approval_required_for_invocation() {
        let mut input = request(false);
        input.approval_granted = false;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputResearchCopilotDisposition::Blocked
        );
    }
    #[test]
    fn approved_invocation_is_declared() {
        let receipt =
            run_throughput_evidence_surveillance_research_copilot(&request(false)).unwrap();
        assert!(receipt.effect_receipts[0].starts_with("invoke:declared-tool:"));
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut input = request(true);
        input.observations[1].evidence_state = EvidenceState::Unknown;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn negative_result_is_retained() {
        let mut input = request(true);
        input.observations[0].negative_result = true;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert!(!receipt.negative_evidence.is_empty());
    }
    #[test]
    fn policy_blocks() {
        let mut input = request(true);
        input.policy_allow = false;
        let receipt = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn replay_is_stable() {
        let input = request(true);
        let first = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        let second = run_throughput_evidence_surveillance_research_copilot(&input).unwrap();
        assert_eq!(first.run_digest, second.run_digest);
    }
}
