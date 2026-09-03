//! Federated continual evidence-surveillance research copilot.
//!
//! Atlas feature `AFA-worldgen-P01-F12`.  Only signed, permitted aggregate
//! contributions cross an institution boundary; raw observations remain local.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P01-F12";
pub const CONTRACT_VERSION: &str =
    "worldgen-federated-continual-evidence-surveillance-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCopilotEvidenceContribution {
    pub peer_id: String,
    pub institution_id: String,
    pub source_id: String,
    pub semantic_profile: String,
    pub artifact_kind: String,
    pub digest: Option<ContentHash>,
    pub signed: bool,
    pub permitted_artifact: bool,
    pub aggregate_only: bool,
    pub evidence_state: EvidenceState,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
    pub request_id: String,
    pub agent_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub declared_tools: Vec<String>,
    pub requested_tool: String,
    pub max_tool_calls: usize,
    pub dry_run: bool,
    pub approval_reference: Option<String>,
    pub approval_granted: bool,
    pub contributions: Vec<FederatedCopilotEvidenceContribution>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualResearchCopilotDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCopilotQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub aggregate_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub tool_mode: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub agent_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub disposition: FederatedContinualResearchCopilotDisposition,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub federation_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub run_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub tool_receipts: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: FederatedCopilotQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedContinualEvidenceSurveillanceResearchCopilotError {
    #[error("invalid federated continual copilot request: {0}")]
    Invalid(String),
    #[error("federated continual copilot artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl FederatedContinualEvidenceSurveillanceResearchCopilotReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), FederatedContinualEvidenceSurveillanceResearchCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.federation_id != self.federation_id
            || self.qualified_set.purpose != self.purpose
        {
            return Err(FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid("federation identity, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        for values in [
            &self.peer_order,
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.aggregate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.tool_receipts,
            &self.effect_receipts,
            &self.qualified_set.peer_order,
            &self.qualified_set.selected_order,
            &self.qualified_set.aggregate_order,
            &self.qualified_set.omissions,
            &self.qualified_set.uncertainty,
            &self.qualified_set.negative_order,
        ] {
            if !ordered(values) {
                return Err(
                    FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                        "federated ordering is not canonical".into(),
                    ),
                );
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
            || self.qualified_set.aggregate_order != self.aggregate_order
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated states do not partition candidates".into(),
                ),
            );
        }
        for digest in [
            &self.replay_identity,
            &self.federation_digest,
            &self.envelope_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.run_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(
                    FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                        "federated digest is invalid".into(),
                    ),
                );
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("dry-run:bounded-tool:")
                && !effect.starts_with("invoke:declared-tool:")
                && effect != "block:unsafe-release"
        }) {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "federated effect is outside declared-tool gate".into(),
                ),
            );
        }
        if self.disposition == FederatedContinualResearchCopilotDisposition::Blocked
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                    "blocked federation must be explicitly blocked".into(),
                ),
            );
        }
        Ok(())
    }
}

pub fn federated_continual_evidence_surveillance_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "worldgen".into(),
        consumers: [
            "benchmark curator".into(),
            "MCP host".into(),
            "federated world-generation steward".into(),
        ]
        .into(),
        behavior: "qualifies signed aggregate-only evidence contributions from structural benchmark worlds under purpose, signer, quorum, locality, and policy gates".into(),
        value: "enables continual federated benchmark surveillance without moving raw experimental observations across institutions".into(),
        inputs: vec![TypedPort {
            name: "federation_envelope".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "qualified_aggregate_evidence".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [
            Effect::ReadLocalData,
            Effect::ExecuteLocalComputation,
            Effect::WriteLocalArtifact,
            Effect::FederationExport,
        ]
        .into(),
        permissions: [
            "invoke:declared-tools".into(),
            "exchange:aggregate-evidence".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "ga4gh-drs".into(),
            state: EvidenceState::Supported,
            locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "federated evidence copilot approver".into(),
            reason: "approve purpose, signer, quorum, and export policy before any federation effect".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn run_federated_continual_evidence_surveillance_research_copilot(
    request: &FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceResearchCopilotReceipt,
    FederatedContinualEvidenceSurveillanceResearchCopilotError,
> {
    if request.request_id.trim().is_empty()
        || request.agent_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.min_peer_quorum == 0
        || request.max_tool_calls == 0
        || request.declared_tools.is_empty()
        || !request
            .declared_tools
            .iter()
            .any(|tool| tool == &request.requested_tool)
        || request.contributions.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid("federation identity, quorum, tools, contributions, locality, or boundary is invalid".into()));
    }
    if request.replay_identity.as_str().len() != 64
        || !request
            .replay_identity
            .as_str()
            .chars()
            .all(|c| c.is_ascii_hexdigit())
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                "replay identity is invalid".into(),
            ),
        );
    }
    let mut contributions = request.contributions.clone();
    contributions.sort_by(|a, b| {
        a.peer_id
            .cmp(&b.peer_id)
            .then_with(|| a.source_id.cmp(&b.source_id))
    });
    let peer_order = contributions
        .iter()
        .map(|item| item.peer_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if peer_order.len()
        != contributions
            .iter()
            .map(|item| item.peer_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
        || contributions.iter().any(|item| {
            item.peer_id.trim().is_empty()
                || item.institution_id.trim().is_empty()
                || item.source_id.trim().is_empty()
        })
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                "peer, institution, and source identities must be unique and non-empty".into(),
            ),
        );
    }
    let candidate_order = contributions
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    if candidate_order.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchCopilotError::Invalid(
                "source identities must be unique".into(),
            ),
        );
    }
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut digests = std::collections::BTreeMap::new();
    for item in &contributions {
        if !request.policy_allow || !request.protected_closure {
            denied.insert(item.source_id.clone());
            omissions.insert(format!("source:{}:policy-or-closure", item.source_id));
        } else if item.semantic_profile != request.semantic_profile {
            denied.insert(item.source_id.clone());
            omissions.insert(format!(
                "source:{}:semantic-profile-mismatch",
                item.source_id
            ));
        } else if !item.signed
            || !item.permitted_artifact
            || !item.aggregate_only
            || !request
                .allowed_artifacts
                .iter()
                .any(|kind| kind == &item.artifact_kind)
        {
            denied.insert(item.source_id.clone());
            omissions.insert(format!(
                "source:{}:signer-permission-or-artifact-gate",
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
            aggregate.insert(item.source_id.clone());
            digests.insert(
                item.source_id.clone(),
                item.digest.clone().expect("digest checked"),
            );
            if item.negative_result {
                negative.insert(format!("source:{}:negative-result", item.source_id));
            }
        }
    }
    if peer_order.len() < request.min_peer_quorum {
        omissions.insert("control:peer-quorum-incomplete".into());
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
        || peer_order.len() < request.min_peer_quorum
        || approval_missing
    {
        FederatedContinualResearchCopilotDisposition::Blocked
    } else if selected.is_empty() {
        FederatedContinualResearchCopilotDisposition::Unknown
    } else if !unresolved.is_empty() || !denied.is_empty() {
        FederatedContinualResearchCopilotDisposition::Partial
    } else {
        FederatedContinualResearchCopilotDisposition::Completed
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let denied_order = denied.into_iter().collect::<Vec<_>>();
    let aggregate_order = aggregate.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let tool_receipts = if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
        vec![format!("tool:{}:denied", request.requested_tool)]
    } else if request.dry_run {
        vec![format!("tool:{}:dry-run", request.requested_tool)]
    } else {
        vec![format!(
            "tool:{}:bounded-call:1/{}",
            request.requested_tool, request.max_tool_calls
        )]
    };
    let federation_digest=ContentHash::of_value(&json!({"federation_id":request.federation_id,"purpose":request.purpose,"endpoint":request.endpoint,"peer_order":peer_order,"min_peer_quorum":request.min_peer_quorum})).map_err(|e|FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let envelope_digest=ContentHash::of_value(&json!({"allowed_artifacts":request.allowed_artifacts,"semantic_profile":request.semantic_profile,"aggregate_order":aggregate_order,"raw_data_local":request.raw_data_local})).map_err(|e|FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let evidence_digest=ContentHash::of_value(&json!({"candidate_order":candidate_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"denied_order":denied_order})).map_err(|e|FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let provenance_digest=ContentHash::of_value(&json!({"request_id":request.request_id,"agent_id":request.agent_id,"replay_identity":request.replay_identity,"federation_digest":federation_digest,"envelope_digest":envelope_digest,"evidence_digest":evidence_digest})).map_err(|e|FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let run_digest=ContentHash::of_value(&json!({"request_id":request.request_id,"dry_run":request.dry_run,"tool_receipts":tool_receipts,"provenance_digest":provenance_digest})).map_err(|e|FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string()))?;
    let qualified_set = FederatedCopilotQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!(
            "worldgen-qualified-evidence-federated-continual-copilot:{}",
            request.request_id
        ),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        peer_order: peer_order.clone(),
        selected_order: selected_order.clone(),
        selected_digests: selected_order
            .iter()
            .filter_map(|id| digests.get(id).cloned())
            .collect(),
        aggregate_order: aggregate_order.clone(),
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_order: negative_evidence.clone(),
        evidence_state: if disposition == FederatedContinualResearchCopilotDisposition::Completed {
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
    let payload = serde_json::to_value(&qualified_set).map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.worldgen.qualified-evidence-set3+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchCopilotError::Artifact(e.to_string())
    })?;
    let receipt = FederatedContinualEvidenceSurveillanceResearchCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        agent_id: request.agent_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        endpoint: request.endpoint.clone(),
        disposition,
        peer_order,
        candidate_order,
        selected_order,
        unresolved_order,
        denied_order,
        aggregate_order,
        replay_identity: request.replay_identity.clone(),
        federation_digest,
        envelope_digest,
        evidence_digest,
        provenance_digest,
        run_digest,
        omissions,
        uncertainty,
        negative_evidence,
        tool_receipts,
        effect_receipts: if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
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
    fn request(dry_run: bool) -> FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
        FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
            request_id: "f12-test".into(),
            agent_id: "agent".into(),
            federation_id: "fed-1".into(),
            purpose: "preclinical-evidence".into(),
            endpoint: "local://aggregate".into(),
            semantic_profile: "profile-v1".into(),
            allowed_artifacts: vec!["qualified-evidence".into()],
            min_peer_quorum: 2,
            declared_tools: vec!["evidence.aggregate".into()],
            requested_tool: "evidence.aggregate".into(),
            max_tool_calls: 2,
            dry_run,
            approval_reference: (!dry_run).then(|| "approval-1".into()),
            approval_granted: !dry_run,
            contributions: (0..2)
                .map(|i| FederatedCopilotEvidenceContribution {
                    peer_id: format!("peer-{i}"),
                    institution_id: format!("inst-{i}"),
                    source_id: format!("source-{i}"),
                    semantic_profile: "profile-v1".into(),
                    artifact_kind: "qualified-evidence".into(),
                    digest: Some(ContentHash::of_bytes(&[i as u8])),
                    signed: true,
                    permitted_artifact: true,
                    aggregate_only: true,
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                })
                .collect(),
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
            federated_continual_evidence_surveillance_research_copilot_manifest().autonomy_tier,
            AutonomyTier::A2
        )
    }
    #[test]
    fn quorum_complete_dry_runs() {
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&request(true))
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Completed
        )
    }
    #[test]
    fn raw_export_gate_denies() {
        let mut i = request(true);
        i.contributions[0].aggregate_only = false;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Partial
        )
    }
    #[test]
    fn quorum_blocks() {
        let mut i = request(true);
        i.min_peer_quorum = 3;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Blocked
        )
    }
    #[test]
    fn approval_required() {
        let mut i = request(false);
        i.approval_granted = false;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .disposition,
            FederatedContinualResearchCopilotDisposition::Blocked
        )
    }
    #[test]
    fn unknown_not_asserted() {
        let mut i = request(true);
        i.contributions[0].evidence_state = EvidenceState::Unknown;
        let r = run_federated_continual_evidence_surveillance_research_copilot(&i).unwrap();
        assert!(!r.uncertainty.is_empty())
    }
    #[test]
    fn contradiction_is_negative() {
        let mut i = request(true);
        i.contributions[0].evidence_state = EvidenceState::Contradicted;
        let r = run_federated_continual_evidence_surveillance_research_copilot(&i).unwrap();
        assert!(!r.negative_evidence.is_empty())
    }
    #[test]
    fn policy_blocks() {
        let mut i = request(true);
        i.policy_allow = false;
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        )
    }
    #[test]
    fn replay_stable() {
        let i = request(true);
        assert_eq!(
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .run_digest,
            run_federated_continual_evidence_surveillance_research_copilot(&i)
                .unwrap()
                .run_digest
        )
    }
}
