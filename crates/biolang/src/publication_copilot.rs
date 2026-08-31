//! Prospective publication and research-object release copilot.
//!
//! Atlas feature: `AFA-biolang-P16-F11`.
//!
//! This A2 capability prepares a bounded, omission-aware release queue from validated preclinical
//! runs. It may invoke only caller-declared local tools under an explicit allow-list; it never
//! signs or publishes bytes itself. Every candidate receives a deterministic supported/unknown/
//! contradicted/blocked disposition, while evidence gaps, negative results, replay mismatches,
//! and tool refusals remain in the content-addressed publication artifact.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, PolicyDecision, PolicyReceipt, ResearchSurface, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-biolang-P16-F11";
pub const CONTRACT_VERSION: &str = "biolang-publication-copilot/1.0";
pub const MAX_RUNS: usize = 4096;
pub const MAX_TOOLS: usize = 128;
const MAX_TEXT_BYTES: usize = 512;
const MAX_LIST_ITEMS: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

/// A validated local run supplied by an upstream execution/evaluation crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun {
    pub run_id: String,
    pub release_id: String,
    pub origin: String,
    pub purpose: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub source_contract_version: String,
    pub requested_tools: Vec<String>,
    pub state: RunState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

/// Bounded agent request. Tool names are capabilities, never arbitrary commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationCopilotRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub runs: Vec<ValidatedResearchRun>,
    pub declared_tools: Vec<String>,
    pub tool_allow_list: Vec<String>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub max_releases: usize,
    pub budget: u64,
    pub policy: PolicyReceipt,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

/// Portable metadata-only output. A separate signer or publication service must authorize export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject {
    pub run_id: String,
    pub release_id: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub tool_invocations: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub disposition: PublicationDisposition,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub release_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub tool_invocation_order: Vec<String>,
    pub provenance_order: Vec<ContentHash>,
    pub replay_order: Vec<ContentHash>,
    pub benchmark_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub effect_receipts: Vec<String>,
    pub objects: Vec<SignedResearchObject>,
    pub publication_artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PublicationCopilotError {
    #[error("invalid publication copilot request: {0}")]
    Invalid(String),
    #[error("publication copilot policy gate failed: {0}")]
    Policy(String),
    #[error("publication copilot artifact failed: {0}")]
    Artifact(String),
    #[error("publication copilot serialization failed: {0}")]
    Serialization(String),
}

fn validate_text(field: &str, value: &str) -> Result<(), PublicationCopilotError> {
    if value.is_empty() || value.trim() != value {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} exceeds the {MAX_TEXT_BYTES}-byte bound"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} must not contain control characters"
        )));
    }
    Ok(())
}

fn validate_string_order(
    values: &[String],
    field: &str,
    allow_empty: bool,
) -> Result<(), PublicationCopilotError> {
    if !allow_empty && values.is_empty() {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} must not be empty"
        )));
    }
    if values.len() > MAX_LIST_ITEMS {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} exceeds the {MAX_LIST_ITEMS}-item bound"
        )));
    }
    for value in values {
        validate_text(field, value)?;
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} must be strictly sorted and unique"
        )));
    }
    Ok(())
}

fn validate_hash_order(values: &[ContentHash], field: &str) -> Result<(), PublicationCopilotError> {
    if values.len() > MAX_LIST_ITEMS {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} exceeds the {MAX_LIST_ITEMS}-item bound"
        )));
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PublicationCopilotError::Invalid(format!(
            "{field} must be strictly sorted and unique"
        )));
    }
    Ok(())
}

impl PublicationCopilotReceipt {
    pub fn validate(&self) -> Result<(), PublicationCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.ranked_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(PublicationCopilotError::Invalid(
                "identity, ranking, locality, effects, or boundary is incomplete".into(),
            ));
        }
        for (field, value) in [
            ("request_id", &self.request_id),
            ("workflow_id", &self.workflow_id),
            ("scope", &self.scope),
        ] {
            validate_text(field, value)?;
        }
        for (values, field, allow_empty) in [
            (&self.ranked_order, "ranked_order", false),
            (&self.admitted_order, "admitted_order", true),
            (&self.blocked_order, "blocked_order", true),
            (&self.unknown_order, "unknown_order", true),
            (&self.release_order, "release_order", true),
            (&self.artifact_order, "artifact_order", true),
            (&self.evidence_order, "evidence_order", true),
            (&self.tool_invocation_order, "tool_invocation_order", true),
            (&self.omissions, "omissions", true),
            (&self.uncertainty, "uncertainty", true),
            (&self.negative_evidence, "negative_evidence", true),
            (&self.effect_receipts, "effect_receipts", false),
        ] {
            validate_string_order(values, field, allow_empty)?;
        }
        for (values, field) in [
            (&self.provenance_order, "provenance_order"),
            (&self.replay_order, "replay_order"),
            (&self.benchmark_order, "benchmark_order"),
        ] {
            validate_hash_order(values, field)?;
        }
        if self.release_order != self.admitted_order
            || self
                .admitted_order
                .iter()
                .any(|id| !self.ranked_order.contains(id) || self.blocked_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.ranked_order.contains(id) || self.admitted_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.blocked_order.contains(id))
            || self
                .ranked_order
                .iter()
                .any(|id| !self.admitted_order.contains(id) && !self.blocked_order.contains(id))
            || self.objects.len() != self.admitted_order.len()
        {
            return Err(PublicationCopilotError::Invalid(
                "publication candidate state and signed-object coverage are inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:declared-tools:") && effect != "block:unsafe-release"
        }) {
            return Err(PublicationCopilotError::Invalid(
                "effect is outside the bounded publication-tool gate".into(),
            ));
        }
        let mut object_releases = Vec::with_capacity(self.objects.len());
        for object in &self.objects {
            if !object.raw_data_local
                || object.boundary != PRECLINICAL_BOUNDARY
                || object.run_id.trim().is_empty()
                || object.release_id.trim().is_empty()
                || object.artifact_ids.is_empty()
                || object.evidence_receipt_ids.is_empty()
                || !self.admitted_order.contains(&object.release_id)
                || self
                    .benchmark_digest
                    .as_ref()
                    .is_none_or(|digest| digest != &object.benchmark_digest)
            {
                return Err(PublicationCopilotError::Invalid(
                    "signed research object is incomplete or non-local".into(),
                ));
            }
            validate_text("object.run_id", &object.run_id)?;
            validate_text("object.release_id", &object.release_id)?;
            validate_string_order(&object.artifact_ids, "object.artifact_ids", false)?;
            validate_string_order(
                &object.evidence_receipt_ids,
                "object.evidence_receipt_ids",
                false,
            )?;
            validate_string_order(&object.tool_invocations, "object.tool_invocations", true)?;
            if object_releases.contains(&object.release_id) {
                return Err(PublicationCopilotError::Invalid(
                    "signed object release identities must be unique".into(),
                ));
            }
            object_releases.push(object.release_id.clone());
        }
        if object_releases != self.admitted_order {
            return Err(PublicationCopilotError::Invalid(
                "signed object release order does not match admitted order".into(),
            ));
        }
        self.publication_artifact
            .validate_metadata()
            .map_err(|error| PublicationCopilotError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, PublicationCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| PublicationCopilotError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| PublicationCopilotError::Serialization(error.to_string()))
    }
}

pub fn publication_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "biolang".into(),
        consumers: [
            "preclinical neuroscientist".into(),
            "publication steward".into(),
            "bounded-tool operator".into(),
        ]
        .into(),
        behavior: "prepares omission-aware metadata-only signed research-object candidates from validated local runs using only declared, allow-listed tools and explicit A2 approval gates".into(),
        value: "reduces publication preparation time while retaining negative evidence, replay identity, provenance, and raw-data locality for independent release services".into(),
        inputs: vec![TypedPort {
            name: "validated_research_run_batch".into(),
            schema: "ValidatedResearchRun3@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "signed_research_object_batch".into(),
            schema: "SignedResearchObject3@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact]
            .into(),
        permissions: ["invoke:declared-tools".into(), "write:local-publication-queue".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "ro-crate-1.3".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "publication release approver".into(),
            reason: "A2 tool invocation and publication preparation require explicit institutional approval".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn prepare_publication_queue(
    request: &PublicationCopilotRequest,
) -> Result<PublicationCopilotReceipt, PublicationCopilotError> {
    validate_request(request)?;
    request
        .policy
        .validate()
        .map_err(|error| PublicationCopilotError::Policy(error.to_string()))?;
    if request.policy.decision != PolicyDecision::Allow {
        return Err(PublicationCopilotError::Policy(
            "publication policy decision is not allow".into(),
        ));
    }
    let mut runs = request.runs.clone();
    runs.sort_by(|left, right| {
        left.release_id
            .cmp(&right.release_id)
            .then(left.run_id.cmp(&right.run_id))
    });
    let ranked_order = runs
        .iter()
        .map(|run| run.release_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut releases = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut tools = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut replay = BTreeSet::new();
    let mut benchmarks = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut objects = Vec::new();
    let mut spent = 0_u64;
    for run in &runs {
        let cost = (run.run_id.len()
            + run.release_id.len()
            + run.artifact_ids.len()
            + run.evidence_receipt_ids.len()
            + run.requested_tools.len()) as u64
            + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let tools_ok = run.requested_tools.iter().all(|tool| {
            request.declared_tools.contains(tool) && request.tool_allow_list.contains(tool)
        });
        let complete = run.state == RunState::Supported
            && !run.artifact_ids.is_empty()
            && !run.evidence_receipt_ids.is_empty()
            && !run.source_contract_version.trim().is_empty()
            && run.provenance_digest != ContentHash::of_bytes(b"")
            && run.replay_identity == request.replay_identity
            && run.benchmark_digest.is_some()
            && request.benchmark_digest.is_some()
            && run.benchmark_digest == request.benchmark_digest
            && run.release_digest != ContentHash::of_bytes(b"")
            && run.omissions.is_empty()
            && run.uncertainty.is_empty()
            && run.negative_evidence.is_empty()
            && run.raw_data_local
            && request.raw_data_local
            && request.protected_closure
            && request.signed_approval
            && tools_ok
            && budget_ok;
        if complete && admitted.len() < request.max_releases {
            let benchmark_digest = run.benchmark_digest.clone().ok_or_else(|| {
                PublicationCopilotError::Invalid(
                    "admitted publication run is missing its benchmark digest".into(),
                )
            })?;
            spent = spent.saturating_add(cost);
            admitted.push(run.release_id.clone());
            releases.insert(run.release_id.clone());
            artifacts.extend(run.artifact_ids.iter().cloned());
            evidence.extend(run.evidence_receipt_ids.iter().cloned());
            tools.extend(run.requested_tools.iter().cloned());
            provenance.insert(run.provenance_digest.clone());
            replay.insert(run.replay_identity.clone());
            benchmarks.insert(benchmark_digest.clone());
            objects.push(SignedResearchObject {
                run_id: run.run_id.clone(),
                release_id: run.release_id.clone(),
                artifact_ids: run.artifact_ids.clone(),
                evidence_receipt_ids: run.evidence_receipt_ids.clone(),
                release_digest: run.release_digest.clone(),
                provenance_digest: run.provenance_digest.clone(),
                replay_identity: run.replay_identity.clone(),
                benchmark_digest,
                tool_invocations: run.requested_tools.clone(),
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            });
        } else {
            blocked.insert(run.release_id.clone());
            if matches!(run.state, RunState::Unknown | RunState::Unmeasured) {
                unknown.insert(run.release_id.clone());
                uncertainty.insert(
                    format!(
                        "release:{}:state-{:?}-not-admitted",
                        run.release_id, run.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if run.state == RunState::Contradicted {
                negative.insert(format!(
                    "release:{}:contradicted-negative-evidence",
                    run.release_id
                ));
            }
            if run.artifact_ids.is_empty() {
                omissions.insert(format!("release:{}:artifact-missing", run.release_id));
            }
            if run.evidence_receipt_ids.is_empty() {
                omissions.insert(format!("release:{}:evidence-missing", run.release_id));
            }
            if run.benchmark_digest.is_none() || request.benchmark_digest.is_none() {
                omissions.insert(format!("release:{}:benchmark-missing", run.release_id));
            }
            if run.benchmark_digest != request.benchmark_digest {
                uncertainty.insert(format!("release:{}:benchmark-mismatch", run.release_id));
            }
            if run.replay_identity != request.replay_identity {
                uncertainty.insert(format!("release:{}:replay-mismatch", run.release_id));
            }
            if !tools_ok {
                negative.insert(format!("release:{}:tool-not-allow-listed", run.release_id));
            }
            if !run.omissions.is_empty() {
                uncertainty.insert(format!(
                    "release:{}:protected-closure-incomplete",
                    run.release_id
                ));
            }
            if !run.uncertainty.is_empty() {
                uncertainty.insert(format!("release:{}:uncertainty-unresolved", run.release_id));
            }
            if !run.negative_evidence.is_empty() {
                negative.insert(format!(
                    "release:{}:negative-evidence-present",
                    run.release_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("release:{}:budget-exhausted", run.release_id));
            }
            if admitted.len() >= request.max_releases {
                omissions.insert(format!("release:{}:release-limit", run.release_id));
            }
        }
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    if request.benchmark_digest.is_none() {
        uncertainty.insert("request:benchmark-missing".into());
    }
    let disposition =
        if !request.protected_closure || !request.signed_approval || !request.raw_data_local {
            PublicationDisposition::Blocked
        } else if admitted.is_empty() {
            PublicationDisposition::Unknown
        } else if blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            PublicationDisposition::Qualified
        } else {
            PublicationDisposition::Partial
        };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "scope": request.scope,
        "disposition": disposition,
        "ranked_order": ranked_order,
        "admitted_order": admitted,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "release_order": releases,
        "artifact_order": artifacts,
        "evidence_order": evidence,
        "tool_invocation_order": tools,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "replay_identity": request.replay_identity,
        "benchmark_digest": request.benchmark_digest,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let publication_artifact = TypedResearchArtifact::from_payload(
        format!("publication-copilot:{}", request.request_id),
        "application/vnd.aurora.publication-copilot+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| PublicationCopilotError::Artifact(error.to_string()))?;
    let effect_receipts = if admitted.is_empty() {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!("invoke:declared-tools:{}", request.request_id)]
    };
    let receipt = PublicationCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.scope.clone(),
        disposition,
        ranked_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        release_order: releases.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        tool_invocation_order: tools.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        replay_order: replay.into_iter().collect(),
        benchmark_order: benchmarks.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        effect_receipts,
        objects,
        publication_artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &PublicationCopilotRequest) -> Result<(), PublicationCopilotError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.runs.is_empty()
        || request.runs.len() > MAX_RUNS
        || request.declared_tools.is_empty()
        || request.declared_tools.len() > MAX_TOOLS
        || request.tool_allow_list.is_empty()
        || request.max_releases == 0
        || request.max_releases > MAX_RUNS
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(PublicationCopilotError::Invalid(
            "request identity, runs, bounded tools, release limit, budget, or boundary is incomplete".into(),
        ));
    }
    for (field, value) in [
        ("request_id", &request.request_id),
        ("workflow_id", &request.workflow_id),
        ("scope", &request.scope),
    ] {
        validate_text(field, value)?;
    }
    let empty_hash = ContentHash::of_bytes(b"");
    if request.replay_identity == empty_hash
        || request
            .benchmark_digest
            .as_ref()
            .is_some_and(|digest| digest == &empty_hash)
    {
        return Err(PublicationCopilotError::Invalid(
            "request content identity must not be empty".into(),
        ));
    }
    unique_strings(&request.declared_tools, "declared tool")?;
    unique_strings(&request.tool_allow_list, "allow-listed tool")?;
    let declared = request.declared_tools.iter().collect::<BTreeSet<_>>();
    if request
        .tool_allow_list
        .iter()
        .any(|tool| !declared.contains(tool))
    {
        return Err(PublicationCopilotError::Invalid(
            "allow-listed tools must be declared capabilities".into(),
        ));
    }
    let mut runs = BTreeSet::new();
    let mut releases = BTreeSet::new();
    for run in &request.runs {
        if run.run_id.trim().is_empty()
            || run.release_id.trim().is_empty()
            || run.origin.trim().is_empty()
            || run.purpose.trim().is_empty()
            || run.source_contract_version.trim().is_empty()
            || run.boundary != PRECLINICAL_BOUNDARY
            || !runs.insert(run.run_id.clone())
            || !releases.insert(run.release_id.clone())
        {
            return Err(PublicationCopilotError::Invalid(format!(
                "run {} is invalid or duplicated",
                run.run_id
            )));
        }
        validate_text("run.run_id", &run.run_id)?;
        validate_text("run.release_id", &run.release_id)?;
        validate_text("run.origin", &run.origin)?;
        validate_text("run.purpose", &run.purpose)?;
        validate_text("run.source_contract_version", &run.source_contract_version)?;
        unique_strings(&run.artifact_ids, "artifact")?;
        unique_strings(&run.evidence_receipt_ids, "evidence receipt")?;
        unique_strings(&run.requested_tools, "requested tool")?;
        unique_strings(&run.omissions, "omission")?;
        unique_strings(&run.uncertainty, "uncertainty")?;
        unique_strings(&run.negative_evidence, "negative evidence")?;
    }
    Ok(())
}

fn unique_strings(values: &[String], kind: &str) -> Result<(), PublicationCopilotError> {
    if values.len() > MAX_LIST_ITEMS {
        return Err(PublicationCopilotError::Invalid(format!(
            "{kind} list exceeds the {MAX_LIST_ITEMS}-item bound"
        )));
    }
    let mut seen = BTreeSet::new();
    for value in values {
        if validate_text(kind, value).is_err() || !seen.insert(value) {
            return Err(PublicationCopilotError::Invalid(format!(
                "{kind} identity is empty or duplicated"
            )));
        }
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PublicationCopilotError::Invalid(format!(
            "{kind} list must be strictly sorted"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn policy() -> PolicyReceipt {
        PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy:publication".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["release digest and declared tools evaluated".into()],
            evaluated_artifacts: vec![hash("release")],
            authority_reference: Some("approval:publication".into()),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn run(id: &str, state: RunState) -> ValidatedResearchRun {
        ValidatedResearchRun {
            run_id: format!("run:{id}"),
            release_id: format!("release:{id}"),
            origin: "institution:alpha".into(),
            purpose: "preclinical-organoid-replication".into(),
            artifact_ids: vec![format!("artifact:{id}")],
            evidence_receipt_ids: vec![format!("evidence:{id}")],
            release_digest: hash(&format!("release-digest:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            source_contract_version: "validated-research-run/3".into(),
            requested_tools: vec!["tool:ro-crate-pack".into()],
            state,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(runs: Vec<ValidatedResearchRun>) -> PublicationCopilotRequest {
        PublicationCopilotRequest {
            request_id: "request:publication".into(),
            workflow_id: "workflow:release".into(),
            scope: "organoid:neural".into(),
            runs,
            declared_tools: vec!["tool:ro-crate-pack".into()],
            tool_allow_list: vec!["tool:ro-crate-pack".into()],
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            max_releases: 4,
            budget: 10_000,
            policy: policy(),
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_typed_a2_with_approval() {
        let manifest = publication_copilot_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert_eq!(manifest.capability_id, FEATURE_ID);
    }

    #[test]
    fn supported_run_becomes_deterministic_signed_object_candidate() {
        let receipt = prepare_publication_queue(&request(vec![
            run("b", RunState::Supported),
            run("a", RunState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, PublicationDisposition::Qualified);
        assert_eq!(receipt.ranked_order, vec!["release:a", "release:b"]);
        assert_eq!(receipt.objects.len(), 2);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn unknown_and_contradicted_runs_remain_visible() {
        let receipt = prepare_publication_queue(&request(vec![
            run("a", RunState::Supported),
            run("b", RunState::Unknown),
            run("c", RunState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, PublicationDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"release:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("release:c")));
    }

    #[test]
    fn undeclared_tool_is_blocked_with_negative_evidence() {
        let mut input = request(vec![run("a", RunState::Supported)]);
        input.runs[0].requested_tools = vec!["tool:other".into()];
        let receipt = prepare_publication_queue(&input).unwrap();
        assert_eq!(receipt.disposition, PublicationDisposition::Unknown);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("tool-not-allow-listed")));
    }

    #[test]
    fn missing_benchmark_is_unknown_and_omitted() {
        let mut input = request(vec![run("a", RunState::Supported)]);
        input.runs[0].benchmark_digest = None;
        let receipt = prepare_publication_queue(&input).unwrap();
        assert_eq!(receipt.disposition, PublicationDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("benchmark-missing")));
    }

    #[test]
    fn policy_denial_rejects_publication_preparation() {
        let mut input = request(vec![run("a", RunState::Supported)]);
        input.policy.decision = PolicyDecision::Deny;
        assert!(prepare_publication_queue(&input).is_err());
    }

    #[test]
    fn duplicate_run_identity_is_rejected() {
        let mut duplicate = run("a", RunState::Supported);
        duplicate.release_id = "release:other".into();
        assert!(prepare_publication_queue(&request(vec![
            run("a", RunState::Supported),
            duplicate
        ]))
        .is_err());
    }

    #[test]
    fn benchmark_identity_mismatch_is_not_admitted() {
        let mut input = request(vec![run("a", RunState::Supported)]);
        input.runs[0].benchmark_digest = Some(hash("different-benchmark"));
        let receipt = prepare_publication_queue(&input).unwrap();
        assert_ne!(receipt.disposition, PublicationDisposition::Qualified);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("benchmark-mismatch")));
    }

    #[test]
    fn tampered_admitted_object_is_rejected_by_receipt_validation() {
        let receipt =
            prepare_publication_queue(&request(vec![run("a", RunState::Supported)])).unwrap();
        let mut tampered = receipt;
        tampered.objects[0].release_id = "release:not-admitted".into();
        assert!(tampered.validate().is_err());
    }
}
