//! Versioned production contracts for the research operating system.
//!
//! This module is the wire-level spine shared by the feature atlas verticals.  It does not
//! replace the richer domain types in `bioprism-section`, `bioprism-policy`, `bioprism-ledger`,
//! or `bioprism-bioir`; it binds their common release, boundary, determinism, provenance and
//! replay obligations into values that can cross Rust, Python, TypeScript, MCP and HTTP surfaces.
//!
//! Every checked value carries the same schema version and the preclinical boundary.  The
//! constructors are intentionally strict: a stringly-typed capability with no consumer, an
//! autonomy grant without a budget, an unresolved policy decision presented as an allow, or a
//! federation envelope that claims to localise raw data is rejected before it reaches a runner.

use crate::error::ResearchContractError;
use bioprism_ids::{to_canonical_bytes, ContentHash, RunId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const RESEARCH_CONTRACT_SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

/// Risk-tiered automation.  A higher tier never grants authority by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyTier {
    A0,
    A1,
    A2,
    A3,
    A4,
}

impl AutonomyTier {
    pub const ALL: [Self; 5] = [Self::A0, Self::A1, Self::A2, Self::A3, Self::A4];

    pub fn requires_approval(self) -> bool {
        matches!(self, Self::A2 | Self::A3 | Self::A4)
    }

    pub fn requires_signed_preflight(self) -> bool {
        matches!(self, Self::A3 | Self::A4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Determinism {
    ByteStable,
    Seeded,
    BoundedNondeterminism,
    HumanMediated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchSurface {
    Ui,
    Cli,
    Api,
    Sdk,
    McpTool,
    Protocol,
    Policy,
    Model,
    Operator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    ReadLocalData,
    WriteLocalArtifact,
    ExecuteLocalComputation,
    ExternalDataAccess,
    FederationExport,
    InstrumentExecution,
    ConsumeMaterial,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TypedPort {
    pub name: String,
    pub schema: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AuthorityRequirement {
    pub role: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceReference {
    pub source_id: String,
    pub state: EvidenceState,
    #[serde(default)]
    pub locator: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Proven,
    Supported,
    Speculative,
    Contradicted,
    Unknown,
}

/// A capability's externally observable contract and authority requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub schema_version: String,
    pub capability_id: String,
    pub version: String,
    pub owner_crate: String,
    pub consumers: BTreeSet<String>,
    pub behavior: String,
    pub value: String,
    pub inputs: Vec<TypedPort>,
    pub outputs: Vec<TypedPort>,
    pub effects: BTreeSet<Effect>,
    pub permissions: BTreeSet<String>,
    pub determinism: Determinism,
    pub evidence: Vec<EvidenceReference>,
    pub authority_requirements: Vec<AuthorityRequirement>,
    pub autonomy_tier: AutonomyTier,
    pub surfaces: BTreeSet<ResearchSurface>,
    pub boundary: String,
}

impl CapabilityManifest {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        require_field("capability_id", &self.capability_id)?;
        require_field("version", &self.version)?;
        require_field("owner_crate", &self.owner_crate)?;
        require_field("behavior", &self.behavior)?;
        require_field("value", &self.value)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.capability_id.clone(),
            });
        }
        if self.consumers.is_empty() {
            return Err(ResearchContractError::NoConsumer {
                item: self.capability_id.clone(),
            });
        }
        if self.inputs.is_empty() || self.outputs.is_empty() {
            return Err(ResearchContractError::MissingTypedContract {
                item: self.capability_id.clone(),
            });
        }
        for port in self.inputs.iter().chain(self.outputs.iter()) {
            require_field("port.name", &port.name)?;
            require_field("port.schema", &port.schema)?;
        }
        if self.autonomy_tier.requires_approval() && self.authority_requirements.is_empty() {
            return Err(ResearchContractError::MissingAuthority {
                item: self.capability_id.clone(),
                tier: self.autonomy_tier,
            });
        }
        if self.autonomy_tier.requires_signed_preflight()
            && !self.effects.contains(&Effect::InstrumentExecution)
        {
            return Err(ResearchContractError::MissingInstrumentPreflight {
                item: self.capability_id.clone(),
            });
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, ResearchContractError> {
        self.validate()?;
        let value =
            serde_json::to_value(self).map_err(|error| ResearchContractError::Serialization {
                item: self.capability_id.clone(),
                message: error.to_string(),
            })?;
        to_canonical_bytes(&value).map_err(|error| ResearchContractError::Serialization {
            item: self.capability_id.clone(),
            message: error.to_string(),
        })
    }

    pub fn digest(&self) -> Result<ContentHash, ResearchContractError> {
        Ok(ContentHash::of_bytes(&self.canonical_bytes()?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub node_id: String,
    pub capability_id: String,
    pub actor: String,
    #[serde(default)]
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    pub checkpoint_id: String,
    pub after_nodes: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub resource: String,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compensation {
    pub effect: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequirement {
    pub approval_id: String,
    pub actor: String,
    pub action: String,
}

/// A typed workflow graph with checkpoints, budgets, compensations and approvals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchWorkflowSpec {
    pub schema_version: String,
    pub workflow_id: String,
    pub intent: String,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    pub checkpoints: Vec<WorkflowCheckpoint>,
    pub budgets: Vec<ResourceBudget>,
    pub compensations: Vec<Compensation>,
    pub approvals: Vec<ApprovalRequirement>,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

impl ResearchWorkflowSpec {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        require_field("workflow_id", &self.workflow_id)?;
        require_field("intent", &self.intent)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.workflow_id.clone(),
            });
        }
        let mut node_ids = BTreeSet::new();
        for node in &self.nodes {
            require_field("node_id", &node.node_id)?;
            require_field("node.capability_id", &node.capability_id)?;
            require_field("node.actor", &node.actor)?;
            if !node_ids.insert(node.node_id.clone()) {
                return Err(ResearchContractError::DuplicateId {
                    kind: "workflow node",
                    id: node.node_id.clone(),
                });
            }
        }
        let mut edge_ids = BTreeSet::new();
        for edge in &self.edges {
            if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
                return Err(ResearchContractError::UnknownWorkflowNode {
                    from: edge.from.clone(),
                    to: edge.to.clone(),
                });
            }
            if edge.from == edge.to {
                return Err(ResearchContractError::WorkflowSelfEdge {
                    node: edge.from.clone(),
                });
            }
            if !edge_ids.insert((edge.from.clone(), edge.to.clone())) {
                return Err(ResearchContractError::DuplicateId {
                    kind: "workflow edge",
                    id: format!("{} -> {}", edge.from, edge.to),
                });
            }
        }
        let mut adjacency: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for edge in &self.edges {
            adjacency
                .entry(edge.from.clone())
                .or_default()
                .push(edge.to.clone());
        }
        for successors in adjacency.values_mut() {
            successors.sort();
            successors.dedup();
        }
        let mut colors = BTreeMap::new();
        for node in &node_ids {
            if workflow_cycle(node, &adjacency, &mut colors)? {
                return Err(ResearchContractError::WorkflowCycle {
                    workflow: self.workflow_id.clone(),
                    node: node.clone(),
                });
            }
        }
        let mut checkpoint_ids = BTreeSet::new();
        for checkpoint in &self.checkpoints {
            require_field("checkpoint_id", &checkpoint.checkpoint_id)?;
            if !checkpoint_ids.insert(checkpoint.checkpoint_id.clone()) {
                return Err(ResearchContractError::DuplicateId {
                    kind: "workflow checkpoint",
                    id: checkpoint.checkpoint_id.clone(),
                });
            }
            if checkpoint.after_nodes.is_empty()
                || !checkpoint
                    .after_nodes
                    .iter()
                    .all(|id| node_ids.contains(id))
            {
                return Err(ResearchContractError::UnknownWorkflowNode {
                    from: checkpoint.checkpoint_id.clone(),
                    to: "checkpoint".into(),
                });
            }
        }
        let mut budget_resources = BTreeSet::new();
        for budget in &self.budgets {
            require_field("budget.resource", &budget.resource)?;
            if !budget_resources.insert(budget.resource.clone()) {
                return Err(ResearchContractError::DuplicateId {
                    kind: "workflow budget resource",
                    id: budget.resource.clone(),
                });
            }
            if !budget.amount.is_finite() || budget.amount < 0.0 {
                return Err(ResearchContractError::InvalidBudget {
                    resource: budget.resource.clone(),
                });
            }
        }
        let mut compensation_pairs = BTreeSet::new();
        for compensation in &self.compensations {
            require_field("compensation.effect", &compensation.effect)?;
            require_field("compensation.action", &compensation.action)?;
            if !compensation_pairs
                .insert((compensation.effect.clone(), compensation.action.clone()))
            {
                return Err(ResearchContractError::DuplicateId {
                    kind: "workflow compensation",
                    id: format!("{} -> {}", compensation.effect, compensation.action),
                });
            }
        }
        let mut approval_ids = BTreeSet::new();
        for approval in &self.approvals {
            require_field("approval_id", &approval.approval_id)?;
            require_field("approval.actor", &approval.actor)?;
            require_field("approval.action", &approval.action)?;
            if !approval_ids.insert(approval.approval_id.clone()) {
                return Err(ResearchContractError::DuplicateId {
                    kind: "workflow approval",
                    id: approval.approval_id.clone(),
                });
            }
        }
        if self.autonomy_tier.requires_approval()
            && self.approvals.is_empty()
            && self.nodes.iter().any(|node| node.requires_approval)
        {
            return Err(ResearchContractError::MissingWorkflowApproval {
                workflow: self.workflow_id.clone(),
            });
        }
        Ok(())
    }
}

fn workflow_cycle(
    node: &str,
    adjacency: &BTreeMap<String, Vec<String>>,
    colors: &mut BTreeMap<String, u8>,
) -> Result<bool, ResearchContractError> {
    match colors.get(node).copied().unwrap_or(0) {
        1 => return Ok(true),
        2 => return Ok(false),
        _ => {}
    }
    colors.insert(node.to_owned(), 1);
    if let Some(successors) = adjacency.get(node) {
        for successor in successors {
            if workflow_cycle(successor, adjacency, colors)? {
                return Ok(true);
            }
        }
    }
    colors.insert(node.to_owned(), 2);
    Ok(false)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticLoss {
    pub field: String,
    pub reason: String,
    pub severity: LossSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LossSeverity {
    None,
    Bounded,
    DecisionRelevant,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceLink {
    pub source_id: String,
    pub relation: String,
    pub digest: ContentHash,
}

/// A content-addressed data, model or result object. Payload bytes stay outside this envelope;
/// the hash is the durable join key and `verify_payload` is the only trust boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedResearchArtifact {
    pub schema_version: String,
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub provenance: Vec<ProvenanceLink>,
    pub boundary: String,
}

impl TypedResearchArtifact {
    pub fn from_payload(
        artifact_id: impl Into<String>,
        content_type: impl Into<String>,
        payload: &Value,
        semantic_loss: Vec<SemanticLoss>,
        provenance: Vec<ProvenanceLink>,
    ) -> Result<Self, ResearchContractError> {
        let artifact_id = artifact_id.into();
        let content_type = content_type.into();
        require_field("artifact_id", &artifact_id)?;
        require_field("content_type", &content_type)?;
        let content_hash = ContentHash::of_value(payload).map_err(|error| {
            ResearchContractError::Serialization {
                item: artifact_id.clone(),
                message: error.to_string(),
            }
        })?;
        Ok(Self {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            artifact_id,
            content_type,
            content_hash,
            semantic_loss,
            provenance,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
    }

    pub fn verify_payload(&self, payload: &Value) -> Result<(), ResearchContractError> {
        self.validate_metadata()?;
        let actual = ContentHash::of_value(payload).map_err(|error| {
            ResearchContractError::Serialization {
                item: self.artifact_id.clone(),
                message: error.to_string(),
            }
        })?;
        if actual != self.content_hash {
            return Err(ResearchContractError::DigestMismatch {
                item: self.artifact_id.clone(),
                expected: self.content_hash.to_string(),
                found: actual.to_string(),
            });
        }
        Ok(())
    }

    /// Validates the portable artifact envelope without requiring the payload bytes.
    ///
    /// Federation metadata intentionally travels separately from institution-local payloads. A
    /// recipient can therefore reject a malformed or out-of-boundary artifact before asking the
    /// origin to release any content, while payload verification remains an explicit second step.
    pub fn validate_metadata(&self) -> Result<(), ResearchContractError> {
        require_field("artifact_id", &self.artifact_id)?;
        require_field("content_type", &self.content_type)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.artifact_id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomyGrant {
    pub schema_version: String,
    pub actor: String,
    pub permitted_actions: BTreeSet<String>,
    pub resource_budget: BTreeMap<String, f64>,
    pub scope: String,
    pub expires_at: String,
    pub revoked: bool,
    pub autonomy_tier: AutonomyTier,
    pub approval_reference: Option<String>,
    pub boundary: String,
}

impl AutonomyGrant {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        require_field("grant.actor", &self.actor)?;
        require_field("grant.scope", &self.scope)?;
        require_field("grant.expires_at", &self.expires_at)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.actor.clone(),
            });
        }
        if self.permitted_actions.is_empty()
            || self.resource_budget.is_empty()
            || self
                .permitted_actions
                .iter()
                .any(|action| action.trim().is_empty())
            || self
                .resource_budget
                .keys()
                .any(|resource| resource.trim().is_empty())
        {
            return Err(ResearchContractError::IncompleteGrant {
                actor: self.actor.clone(),
            });
        }
        if self.autonomy_tier.requires_approval()
            && self
                .approval_reference
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
        {
            return Err(ResearchContractError::MissingAuthority {
                item: self.actor.clone(),
                tier: self.autonomy_tier,
            });
        }
        if self
            .resource_budget
            .values()
            .any(|amount| !amount.is_finite() || *amount < 0.0)
        {
            return Err(ResearchContractError::InvalidBudget {
                resource: self.actor.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    Allow,
    Deny,
    Redact,
    LocalOnly,
    ApprovalRequired,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub decision: PolicyDecision,
    pub reasons: Vec<String>,
    pub evaluated_artifacts: Vec<ContentHash>,
    pub authority_reference: Option<String>,
    pub boundary: String,
}

impl PolicyReceipt {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        require_field("receipt_id", &self.receipt_id)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.receipt_id.clone(),
            });
        }
        if self.reasons.is_empty() || self.reasons.iter().any(|reason| reason.trim().is_empty()) {
            return Err(ResearchContractError::MissingReason {
                item: self.receipt_id.clone(),
            });
        }
        if self.decision == PolicyDecision::Allow
            && self.reasons.iter().any(|reason| reason == "unresolved")
        {
            return Err(ResearchContractError::UnresolvedAllow {
                receipt: self.receipt_id.clone(),
            });
        }
        if matches!(
            self.decision,
            PolicyDecision::ApprovalRequired | PolicyDecision::Unresolved
        ) && self.authority_reference.is_some()
        {
            return Err(ResearchContractError::PrematureAuthority {
                receipt: self.receipt_id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Planned,
    Running,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEvent {
    pub sequence: u64,
    pub event_type: String,
    pub effect: Option<Effect>,
    pub payload_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCheckpoint {
    pub checkpoint_id: String,
    pub event_sequence: u64,
    pub replay_hash: ContentHash,
}

/// Append-only execution evidence with an explicit replay identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRun {
    pub schema_version: String,
    pub run_id: RunId,
    pub workflow_id: String,
    pub plan_hash: ContentHash,
    pub status: ExecutionStatus,
    pub events: Vec<ExecutionEvent>,
    pub checkpoints: Vec<ExecutionCheckpoint>,
    pub retry_count: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

impl ExecutionRun {
    pub fn planned(
        run_id: RunId,
        workflow_id: impl Into<String>,
        plan_hash: ContentHash,
    ) -> Result<Self, ResearchContractError> {
        let workflow_id = workflow_id.into();
        require_field("workflow_id", &workflow_id)?;
        let replay_identity =
            ContentHash::of_bytes(format!("{}:{}", workflow_id, plan_hash).as_bytes());
        Ok(Self {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            run_id,
            workflow_id,
            plan_hash,
            status: ExecutionStatus::Planned,
            events: Vec::new(),
            checkpoints: Vec::new(),
            retry_count: 0,
            replay_identity,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
    }

    pub fn append_event(&mut self, event: ExecutionEvent) -> Result<(), ResearchContractError> {
        if self.status == ExecutionStatus::Succeeded
            || self.status == ExecutionStatus::Failed
            || self.status == ExecutionStatus::Cancelled
        {
            return Err(ResearchContractError::RunClosed {
                run: self.run_id.to_string(),
            });
        }
        let expected = self.events.len() as u64;
        if event.sequence != expected {
            return Err(ResearchContractError::EventSequence {
                run: self.run_id.to_string(),
                expected,
                found: event.sequence,
            });
        }
        require_field("event_type", &event.event_type)?;
        if event.effect == Some(Effect::InstrumentExecution) && event.payload_hash.is_none() {
            return Err(ResearchContractError::MissingEffectEvidence {
                run: self.run_id.to_string(),
            });
        }
        self.status = ExecutionStatus::Running;
        self.events.push(event);
        Ok(())
    }

    pub fn checkpoint(
        &mut self,
        checkpoint_id: impl Into<String>,
    ) -> Result<(), ResearchContractError> {
        if matches!(
            self.status,
            ExecutionStatus::Succeeded | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        ) {
            return Err(ResearchContractError::RunClosed {
                run: self.run_id.to_string(),
            });
        }
        let checkpoint_id = checkpoint_id.into();
        require_field("checkpoint_id", &checkpoint_id)?;
        if self
            .checkpoints
            .iter()
            .any(|checkpoint| checkpoint.checkpoint_id == checkpoint_id)
        {
            return Err(ResearchContractError::DuplicateId {
                kind: "execution checkpoint",
                id: checkpoint_id,
            });
        }
        let events_value = serde_json::to_value(&self.events).map_err(|error| {
            ResearchContractError::Serialization {
                item: self.run_id.to_string(),
                message: error.to_string(),
            }
        })?;
        let replay_hash = ContentHash::of_value(&events_value).map_err(|error| {
            ResearchContractError::Serialization {
                item: self.run_id.to_string(),
                message: error.to_string(),
            }
        })?;
        self.checkpoints.push(ExecutionCheckpoint {
            checkpoint_id,
            event_sequence: self.events.len() as u64,
            replay_hash,
        });
        Ok(())
    }

    pub fn finish(&mut self, status: ExecutionStatus) -> Result<(), ResearchContractError> {
        if !matches!(
            status,
            ExecutionStatus::Succeeded | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        ) {
            return Err(ResearchContractError::InvalidFinalStatus);
        }
        if matches!(
            self.status,
            ExecutionStatus::Succeeded | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        ) {
            return Err(ResearchContractError::RunClosed {
                run: self.run_id.to_string(),
            });
        }
        self.status = status;
        Ok(())
    }

    /// Validates a reconstructed execution ledger without requiring its source workflow.
    ///
    /// The append methods protect live mutation, but this type is also transported over JSON.
    /// A caller must not be able to deserialize skipped events, duplicate checkpoints, or replay
    /// hashes for a different prefix and have the record accepted as execution evidence.
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        require_field("workflow_id", &self.workflow_id)?;
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.workflow_id.clone(),
            });
        }
        let expected_identity =
            ContentHash::of_bytes(format!("{}:{}", self.workflow_id, self.plan_hash).as_bytes());
        if self.replay_identity != expected_identity {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.workflow_id.clone(),
            });
        }
        for (sequence, event) in self.events.iter().enumerate() {
            if event.sequence != sequence as u64 {
                return Err(ResearchContractError::EventSequence {
                    run: self.run_id.to_string(),
                    expected: sequence as u64,
                    found: event.sequence,
                });
            }
            require_field("event_type", &event.event_type)?;
            if event.effect == Some(Effect::InstrumentExecution) && event.payload_hash.is_none() {
                return Err(ResearchContractError::MissingEffectEvidence {
                    run: self.run_id.to_string(),
                });
            }
        }
        if self.status == ExecutionStatus::Planned && !self.events.is_empty() {
            return Err(ResearchContractError::EventSequence {
                run: self.run_id.to_string(),
                expected: 0,
                found: self.events.len() as u64,
            });
        }
        let mut checkpoint_ids = BTreeSet::new();
        for checkpoint in &self.checkpoints {
            require_field("checkpoint_id", &checkpoint.checkpoint_id)?;
            if !checkpoint_ids.insert(checkpoint.checkpoint_id.clone()) {
                return Err(ResearchContractError::DuplicateId {
                    kind: "execution checkpoint",
                    id: checkpoint.checkpoint_id.clone(),
                });
            }
            if checkpoint.event_sequence > self.events.len() as u64 {
                return Err(ResearchContractError::EventSequence {
                    run: self.run_id.to_string(),
                    expected: self.events.len() as u64,
                    found: checkpoint.event_sequence,
                });
            }
            let end = usize::try_from(checkpoint.event_sequence).map_err(|_| {
                ResearchContractError::Serialization {
                    item: self.run_id.to_string(),
                    message: "checkpoint sequence exceeds addressable memory".into(),
                }
            })?;
            let events = serde_json::to_value(&self.events[..end]).map_err(|error| {
                ResearchContractError::Serialization {
                    item: self.run_id.to_string(),
                    message: error.to_string(),
                }
            })?;
            let expected_hash = ContentHash::of_value(&events).map_err(|error| {
                ResearchContractError::Serialization {
                    item: self.run_id.to_string(),
                    message: error.to_string(),
                }
            })?;
            if checkpoint.replay_hash != expected_hash {
                return Err(ResearchContractError::DigestMismatch {
                    item: checkpoint.checkpoint_id.clone(),
                    expected: expected_hash.to_string(),
                    found: checkpoint.replay_hash.to_string(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSource {
    pub source_id: String,
    pub source_type: String,
    pub locator: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAvailability {
    Available,
    Stale,
    Contradictory,
    Missing,
    Protected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UncertaintyStatement {
    pub kind: String,
    pub statement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Omission {
    pub item: String,
    pub reason: String,
    pub could_change_decision: DecisionImpact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionImpact {
    NoKnownImpact,
    PotentiallyMaterial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompetingExplanation {
    pub explanation: String,
    pub supporting_sources: Vec<String>,
    pub unresolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NegativeEvidence {
    pub source_id: String,
    pub result: String,
    pub interpretation: String,
}

/// Evidence-to-typed-knowledge output.  Omission and negative evidence are first-class fields;
/// absence is never silently turned into confidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub intent: String,
    pub sources: Vec<EvidenceSource>,
    pub derivation: Vec<String>,
    pub uncertainty: Vec<UncertaintyStatement>,
    pub omissions: Vec<Omission>,
    pub competing_explanations: Vec<CompetingExplanation>,
    pub negative_evidence: Vec<NegativeEvidence>,
    pub conclusion_state: EvidenceState,
    pub boundary: String,
}

impl EvidenceReceipt {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        require_field("receipt_id", &self.receipt_id)?;
        require_field("intent", &self.intent)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.receipt_id.clone(),
            });
        }
        if self.sources.is_empty()
            && (self.conclusion_state != EvidenceState::Unknown
                || self.omissions.is_empty()
                || self.uncertainty.is_empty())
        {
            return Err(ResearchContractError::NoEvidence {
                receipt: self.receipt_id.clone(),
            });
        }
        if self.derivation.is_empty() {
            return Err(ResearchContractError::MissingDerivation {
                receipt: self.receipt_id.clone(),
            });
        }
        if self
            .sources
            .iter()
            .any(|source| source.source_id.trim().is_empty())
        {
            return Err(ResearchContractError::MissingSourceId {
                receipt: self.receipt_id.clone(),
            });
        }
        if self.conclusion_state == EvidenceState::Proven
            && self
                .omissions
                .iter()
                .any(|omission| omission.could_change_decision != DecisionImpact::NoKnownImpact)
        {
            return Err(ResearchContractError::ProtectedOmissionBlocksConclusion {
                receipt: self.receipt_id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationMetric {
    pub name: String,
    pub value: String,
    pub uncertainty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationCard {
    pub schema_version: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub baselines: Vec<String>,
    pub metrics: Vec<EvaluationMetric>,
    pub uncertainty: Vec<UncertaintyStatement>,
    pub limitations: Vec<String>,
    pub release_verdict: ReleaseVerdict,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseVerdict {
    Pass,
    Conditional,
    Blocked,
    NotEvaluated,
}

impl EvaluationCard {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        require_field("capability_id", &self.capability_id)?;
        require_field("benchmark_world", &self.benchmark_world)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.capability_id.clone(),
            });
        }
        if self.baselines.is_empty() || self.metrics.is_empty() || self.limitations.is_empty() {
            return Err(ResearchContractError::IncompleteEvaluation {
                capability: self.capability_id.clone(),
            });
        }
        if self.release_verdict == ReleaseVerdict::Pass && self.uncertainty.is_empty() {
            return Err(ResearchContractError::MissingUncertainty {
                item: self.capability_id.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationEnvelope {
    pub schema_version: String,
    pub envelope_id: String,
    pub origin: String,
    pub purpose: String,
    pub export: TypedResearchArtifact,
    pub policy_constraints: Vec<String>,
    pub integrity_evidence: Vec<ContentHash>,
    pub localization_statement: String,
    pub raw_data_local: bool,
    pub signature: Option<String>,
    pub boundary: String,
}

impl FederationEnvelope {
    pub fn validate(&self) -> Result<(), ResearchContractError> {
        require_field("envelope_id", &self.envelope_id)?;
        require_field("origin", &self.origin)?;
        require_field("purpose", &self.purpose)?;
        require_field("localization_statement", &self.localization_statement)?;
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContractError::SchemaVersion {
                expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                found: self.schema_version.clone(),
            });
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContractError::BoundaryMismatch {
                capability: self.envelope_id.clone(),
            });
        }
        if self.policy_constraints.is_empty() || self.integrity_evidence.is_empty() {
            return Err(ResearchContractError::IncompleteFederation {
                envelope: self.envelope_id.clone(),
            });
        }
        if self.raw_data_local
            && !self
                .localization_statement
                .to_ascii_lowercase()
                .contains("local")
        {
            return Err(ResearchContractError::LocalizationMismatch {
                envelope: self.envelope_id.clone(),
            });
        }
        if self.signature.as_deref().unwrap_or("").trim().is_empty() {
            return Err(ResearchContractError::UnsignedFederation {
                envelope: self.envelope_id.clone(),
            });
        }
        // The envelope carries the exported artifact's hash, not its payload. A consumer must
        // call `verify_payload` on the artifact after local policy admission.
        self.export.validate_metadata()
    }
}

fn require_field(field: &'static str, value: &str) -> Result<(), ResearchContractError> {
    if value.trim().is_empty() {
        Err(ResearchContractError::MissingField { field })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn manifest() -> CapabilityManifest {
        CapabilityManifest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            capability_id: "AFA-foundation-P01-F01".into(),
            version: "0.1.0".into(),
            owner_crate: "foundation".into(),
            consumers: ["context compiler engineer".into()].into(),
            behavior: "compiles bounded research evidence into a typed receipt".into(),
            value: "makes omissions and derivation replayable".into(),
            inputs: vec![TypedPort {
                name: "sources".into(),
                schema: "EvidenceSource[]@1".into(),
                required: true,
            }],
            outputs: vec![TypedPort {
                name: "receipt".into(),
                schema: "EvidenceReceipt@1".into(),
                required: true,
            }],
            effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(),
            permissions: ["read:local-research-sources".into()].into(),
            determinism: Determinism::ByteStable,
            evidence: vec![EvidenceReference {
                source_id: "fixture:typed-knowledge".into(),
                state: EvidenceState::Supported,
                locator: None,
            }],
            authority_requirements: Vec::new(),
            autonomy_tier: AutonomyTier::A1,
            surfaces: [ResearchSurface::Cli, ResearchSurface::Api].into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_digest_is_stable_and_boundary_checked() {
        let value = manifest();
        let left = value.digest().unwrap();
        let right = serde_json::from_slice::<CapabilityManifest>(&value.canonical_bytes().unwrap())
            .unwrap()
            .digest()
            .unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn capability_without_a_consumer_is_refused() {
        let mut value = manifest();
        value.consumers.clear();
        assert!(matches!(
            value.validate(),
            Err(ResearchContractError::NoConsumer { .. })
        ));
    }

    #[test]
    fn workflow_cycle_is_refused_before_execution() {
        let workflow = ResearchWorkflowSpec {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "wf-cycle".into(),
            intent: "cycle fixture".into(),
            nodes: vec![
                WorkflowNode {
                    node_id: "a".into(),
                    capability_id: "cap-a".into(),
                    actor: "operator".into(),
                    requires_approval: false,
                },
                WorkflowNode {
                    node_id: "b".into(),
                    capability_id: "cap-b".into(),
                    actor: "operator".into(),
                    requires_approval: false,
                },
            ],
            edges: vec![
                WorkflowEdge {
                    from: "a".into(),
                    to: "b".into(),
                },
                WorkflowEdge {
                    from: "b".into(),
                    to: "a".into(),
                },
            ],
            checkpoints: vec![],
            budgets: vec![ResourceBudget {
                resource: "cpu".into(),
                amount: 1.0,
            }],
            compensations: vec![],
            approvals: vec![],
            autonomy_tier: AutonomyTier::A0,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(matches!(
            workflow.validate(),
            Err(ResearchContractError::WorkflowCycle { .. })
        ));
    }

    #[test]
    fn workflow_collections_reject_ambiguous_duplicates() {
        let mut workflow = ResearchWorkflowSpec {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            workflow_id: "wf-duplicates".into(),
            intent: "duplicate validation fixture".into(),
            nodes: vec![WorkflowNode {
                node_id: "node".into(),
                capability_id: "capability".into(),
                actor: "operator".into(),
                requires_approval: false,
            }],
            edges: Vec::new(),
            checkpoints: Vec::new(),
            budgets: vec![ResourceBudget {
                resource: "cpu".into(),
                amount: 1.0,
            }],
            compensations: Vec::new(),
            approvals: Vec::new(),
            autonomy_tier: AutonomyTier::A0,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };

        workflow.budgets.push(ResourceBudget {
            resource: "cpu".into(),
            amount: 2.0,
        });
        assert!(matches!(
            workflow.validate(),
            Err(ResearchContractError::DuplicateId {
                kind: "workflow budget resource",
                ..
            })
        ));

        workflow.budgets.pop();
        workflow.compensations = vec![
            Compensation {
                effect: "compute".into(),
                action: "stop".into(),
            },
            Compensation {
                effect: "compute".into(),
                action: "stop".into(),
            },
        ];
        assert!(matches!(
            workflow.validate(),
            Err(ResearchContractError::DuplicateId {
                kind: "workflow compensation",
                ..
            })
        ));
    }

    #[test]
    fn artifact_hash_rejects_tampering() {
        let artifact = TypedResearchArtifact::from_payload(
            "artifact-1",
            "application/json",
            &json!({"x": 1}),
            vec![],
            vec![],
        )
        .unwrap();
        assert!(artifact.verify_payload(&json!({"x": 2})).is_err());
        artifact.verify_payload(&json!({"x": 1})).unwrap();
    }

    #[test]
    fn artifact_metadata_rejects_empty_identity_fields() {
        let mut artifact = TypedResearchArtifact::from_payload(
            "artifact-identity",
            "application/json",
            &json!({"x": 1}),
            vec![],
            vec![],
        )
        .unwrap();
        artifact.artifact_id.clear();
        assert!(matches!(
            artifact.validate_metadata(),
            Err(ResearchContractError::MissingField {
                field: "artifact_id"
            })
        ));

        let mut artifact = TypedResearchArtifact::from_payload(
            "artifact-content-type",
            "application/json",
            &json!({"x": 1}),
            vec![],
            vec![],
        )
        .unwrap();
        artifact.content_type.clear();
        assert!(matches!(
            artifact.validate_metadata(),
            Err(ResearchContractError::MissingField {
                field: "content_type"
            })
        ));
    }

    #[test]
    fn unresolved_policy_cannot_be_presented_as_allow() {
        let receipt = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy-1".into(),
            decision: PolicyDecision::Allow,
            reasons: vec!["unresolved".into()],
            evaluated_artifacts: vec![],
            authority_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(matches!(
            receipt.validate(),
            Err(ResearchContractError::UnresolvedAllow { .. })
        ));
    }

    #[test]
    fn policy_receipts_reject_blank_reasons() {
        let receipt = PolicyReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "policy-blank-reason".into(),
            decision: PolicyDecision::Deny,
            reasons: vec!["   ".into()],
            evaluated_artifacts: vec![],
            authority_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(matches!(
            receipt.validate(),
            Err(ResearchContractError::MissingReason { .. })
        ));
    }

    #[test]
    fn autonomy_grants_reject_blank_permission_and_resource_names() {
        let mut grant = AutonomyGrant {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            actor: "operator".into(),
            permitted_actions: ["".into()].into(),
            resource_budget: [("cpu".into(), 1.0)].into(),
            scope: "study:demo".into(),
            expires_at: "2099-01-01T00:00:00Z".into(),
            revoked: false,
            autonomy_tier: AutonomyTier::A0,
            approval_reference: None,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(matches!(
            grant.validate(),
            Err(ResearchContractError::IncompleteGrant { .. })
        ));

        grant.permitted_actions = ["read".into()].into();
        grant.resource_budget = [("   ".into(), 1.0)].into();
        assert!(matches!(
            grant.validate(),
            Err(ResearchContractError::IncompleteGrant { .. })
        ));
    }

    #[test]
    fn execution_events_are_append_only_and_instrument_effects_need_evidence() {
        let plan = ContentHash::of_bytes(b"plan");
        let run_id = RunId::parse("run-1").unwrap();
        let mut run = ExecutionRun::planned(run_id, "wf-1", plan).unwrap();
        let event = ExecutionEvent {
            sequence: 0,
            event_type: "instrument.start".into(),
            effect: Some(Effect::InstrumentExecution),
            payload_hash: None,
        };
        assert!(matches!(
            run.append_event(event),
            Err(ResearchContractError::MissingEffectEvidence { .. })
        ));
        run.append_event(ExecutionEvent {
            sequence: 0,
            event_type: "local.compute".into(),
            effect: Some(Effect::ExecuteLocalComputation),
            payload_hash: None,
        })
        .unwrap();
        run.finish(ExecutionStatus::Succeeded).unwrap();
        assert!(matches!(
            run.append_event(ExecutionEvent {
                sequence: 1,
                event_type: "late".into(),
                effect: None,
                payload_hash: None
            }),
            Err(ResearchContractError::RunClosed { .. })
        ));
    }

    #[test]
    fn execution_run_rejects_duplicate_checkpoints_and_repeated_finish() {
        let plan = ContentHash::of_bytes(b"plan");
        let mut run = ExecutionRun::planned(
            RunId::parse("run-checkpoint-boundary").unwrap(),
            "wf-1",
            plan,
        )
        .unwrap();
        run.checkpoint("admission").unwrap();
        assert!(matches!(
            run.checkpoint("admission"),
            Err(ResearchContractError::DuplicateId { .. })
        ));
        run.finish(ExecutionStatus::Failed).unwrap();
        assert!(matches!(
            run.finish(ExecutionStatus::Succeeded),
            Err(ResearchContractError::RunClosed { .. })
        ));
        run.validate().unwrap();
    }

    #[test]
    fn execution_run_validation_rejects_tampered_checkpoint_replay_hash() {
        let plan = ContentHash::of_bytes(b"plan");
        let mut run = ExecutionRun::planned(
            RunId::parse("run-checkpoint-integrity").unwrap(),
            "wf-1",
            plan,
        )
        .unwrap();
        run.append_event(ExecutionEvent {
            sequence: 0,
            event_type: "local.compute".into(),
            effect: Some(Effect::ExecuteLocalComputation),
            payload_hash: None,
        })
        .unwrap();
        run.checkpoint("admission").unwrap();
        run.checkpoints[0].replay_hash = ContentHash::of_bytes(b"tampered");
        assert!(matches!(
            run.validate(),
            Err(ResearchContractError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn protected_omission_blocks_a_proven_conclusion() {
        let receipt = EvidenceReceipt {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            receipt_id: "evidence-1".into(),
            intent: "compare two preclinical mechanisms".into(),
            sources: vec![EvidenceSource {
                source_id: "paper-1".into(),
                source_type: "paper".into(),
                locator: "doi:1".into(),
                digest: None,
                availability: EvidenceAvailability::Available,
            }],
            derivation: vec!["extract:claim-1".into()],
            uncertainty: vec![UncertaintyStatement {
                kind: "epistemic".into(),
                statement: "one study".into(),
            }],
            omissions: vec![Omission {
                item: "protected-dataset".into(),
                reason: "institution policy".into(),
                could_change_decision: DecisionImpact::Unknown,
            }],
            competing_explanations: vec![],
            negative_evidence: vec![],
            conclusion_state: EvidenceState::Proven,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        assert!(matches!(
            receipt.validate(),
            Err(ResearchContractError::ProtectedOmissionBlocksConclusion { .. })
        ));
    }
}
