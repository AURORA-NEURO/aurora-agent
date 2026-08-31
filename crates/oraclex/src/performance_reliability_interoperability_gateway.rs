//! Federated continual performance/reliability interoperability gateway (`AFA-oraclex-P21-F24`).
//!
//! The gateway exchanges capability telemetry, not workload payloads.  It evaluates signed
//! invocation summaries under explicit latency, retry, duplicate-event, policy, provenance, and
//! replay gates and emits a deterministic `ReliableCapabilityResult6` envelope.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oraclex-P21-F24";
pub const CONTRACT_VERSION: &str =
    "oraclex-federated-continual-performance-reliability-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "CapabilityWorkload4@1";
pub const OUTPUT_SCHEMA: &str = "ReliableCapabilityResult6@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.reliable-capability-result-6+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadEvidenceState {
    Proven,
    Supported,
    Speculative,
    Contradicted,
    Unknown,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityInvocation5 {
    pub invocation_id: String,
    pub capability_id: String,
    pub endpoint: String,
    pub input_digest: ContentHash,
    pub expected_tasks: u32,
    pub completed_tasks: u32,
    pub p95_latency_ms: u64,
    pub latency_slo_ms: u64,
    pub retry_count: u32,
    pub max_retries: u32,
    pub duplicate_events: u32,
    pub evidence_state: WorkloadEvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityWorkload4 {
    pub schema_version: String,
    pub request_id: String,
    pub scope: String,
    pub federation_id: String,
    pub institution_id: String,
    pub capability_id: String,
    pub required_invocation_order: Vec<String>,
    pub invocations: Vec<CapabilityInvocation5>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub network_available: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget_units: u64,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliableCapabilityResult6 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub scope: String,
    pub federation_id: String,
    pub institution_id: String,
    pub capability_id: String,
    pub disposition: String,
    pub invocation_order: Vec<String>,
    pub dependable_order: Vec<String>,
    pub degraded_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub endpoint_order: Vec<String>,
    pub retry_order: Vec<String>,
    pub timeout_order: Vec<String>,
    pub duplicate_event_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub budget_used_units: u64,
    pub replay_identity: ContentHash,
    pub result_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PerformanceReliabilityInteroperabilityError {
    #[error("invalid performance/reliability interoperability request or receipt: {0}")]
    Invalid(String),
    #[error("performance/reliability interoperability artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> PerformanceReliabilityInteroperabilityError {
    PerformanceReliabilityInteroperabilityError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn digest(value: &Value) -> Result<ContentHash, PerformanceReliabilityInteroperabilityError> {
    ContentHash::of_value(value)
        .map_err(|error| PerformanceReliabilityInteroperabilityError::Artifact(error.to_string()))
}

pub fn performance_reliability_interoperability_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "oraclex".into(), consumers: ["research program lead".into(), "federation reliability steward".into(), "institution operations operator".into()].into(), behavior: "negotiate signed capability reliability summaries into deterministic federated envelopes with explicit retry, timeout, duplicate-event, migration, and failure evidence".into(), value: "lets research programs depend on measurable, replayable capability health without moving raw workloads or hiding degraded operation".into(), inputs: vec![TypedPort { name: "capability_workload".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "reliable_capability_result".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["connect:approved-endpoints".into(), "exchange:permitted-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }, EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federation reliability steward".into(), reason: "A2 exchange effects require explicit institution approval and remain limited to aggregate reliability envelopes".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

impl ReliableCapabilityResult6 {
    pub fn validate(&self) -> Result<(), PerformanceReliabilityInteroperabilityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.invocation_order.is_empty()
            || self.endpoint_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_used_units == 0
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(invalid(
                "reliability identity, locality, endpoint, budget, or effects are incomplete",
            ));
        }
        for values in [
            &self.invocation_order,
            &self.dependable_order,
            &self.degraded_order,
            &self.blocked_order,
            &self.missing_order,
            &self.endpoint_order,
            &self.retry_order,
            &self.timeout_order,
            &self.duplicate_event_order,
            &self.migration_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("reliability ordering is not canonical"));
            }
        }
        let ids = self
            .invocation_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .dependable_order
            .iter()
            .chain(&self.degraded_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        let missing = self.missing_order.iter().cloned().collect::<BTreeSet<_>>();
        if ids.len() != self.invocation_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
            || missing.len() != self.missing_order.len()
            || !missing.is_disjoint(&ids)
        {
            return Err(invalid("invocation states do not partition"));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.result_digest)
            || self.artifact.content_hash != self.result_digest
        {
            return Err(PerformanceReliabilityInteroperabilityError::Artifact(
                "reliability digest or artifact hash is inconsistent".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            PerformanceReliabilityInteroperabilityError::Artifact(error.to_string())
        })?;
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("exchange:permitted-artifacts:")
        }) {
            return Err(invalid("reliability effect is outside exchange gate"));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, PerformanceReliabilityInteroperabilityError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|error| {
                PerformanceReliabilityInteroperabilityError::Artifact(error.to_string())
            })
            .and_then(|value| digest(&value))
    }
}

pub fn negotiate_performance_reliability(
    workload: &CapabilityWorkload4,
) -> Result<ReliableCapabilityResult6, PerformanceReliabilityInteroperabilityError> {
    if workload.schema_version != INPUT_SCHEMA
        || workload.request_id.trim().is_empty()
        || workload.scope.trim().is_empty()
        || workload.federation_id.trim().is_empty()
        || workload.institution_id.trim().is_empty()
        || workload.capability_id.trim().is_empty()
        || workload.required_invocation_order.is_empty()
        || !canonical(&workload.required_invocation_order)
        || !canonical(&workload.adversarial_event_order)
        || !valid_digest(&workload.replay_identity)
        || workload.budget_units == 0
        || !workload.raw_data_local
        || !workload.aggregate_only
        || workload.boundary != PRECLINICAL_BOUNDARY
        || workload.invocations.is_empty()
    {
        return Err(invalid(
            "workload identity, required closure, replay, budget, locality, or boundary is invalid",
        ));
    }
    let mut rows = workload.invocations.clone();
    rows.sort_by(|left, right| left.invocation_id.cmp(&right.invocation_id));
    let invocation_order = rows
        .iter()
        .map(|row| row.invocation_id.clone())
        .collect::<Vec<_>>();
    if invocation_order.windows(2).any(|pair| pair[0] == pair[1])
        || invocation_order.iter().any(|id| id.trim().is_empty())
    {
        return Err(invalid(
            "invocation identifiers must be unique and non-empty",
        ));
    }
    let mut dependable = BTreeSet::new();
    let mut degraded = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut retry = BTreeSet::new();
    let mut timeout = BTreeSet::new();
    let mut duplicate = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut budget_used_units = 0_u64;
    for row in &rows {
        let id = row.invocation_id.clone();
        endpoints.insert(row.endpoint.clone());
        budget_used_units = budget_used_units
            .checked_add(u64::from(row.expected_tasks))
            .ok_or_else(|| invalid("reliability budget overflow"))?;
        omission.extend(row.omission_order.iter().map(|item| format!("{id}:{item}")));
        if row.negative_result
            || matches!(
                row.evidence_state,
                WorkloadEvidenceState::Negative | WorkloadEvidenceState::Contradicted
            )
        {
            negative.insert(format!("{id}:negative-result"));
        }
        if row.retry_count > 0 {
            retry.insert(id.clone());
        }
        if row.p95_latency_ms > row.latency_slo_ms {
            timeout.insert(id.clone());
        }
        if row.duplicate_events > 0 {
            duplicate.insert(id.clone());
        }
        if !row.signed || !row.permitted || !row.raw_data_local || !row.aggregate_only {
            blocked.insert(id);
            omission.insert(format!(
                "{}:signature-permission-or-locality",
                row.invocation_id
            ));
        } else if row.replay_identity != workload.replay_identity {
            degraded.insert(id);
            uncertainty.insert(format!("{}:replay-mismatch", row.invocation_id));
        } else if row.expected_tasks == 0
            || row.completed_tasks < row.expected_tasks
            || row.retry_count > row.max_retries
            || row.duplicate_events > 0
            || row.p95_latency_ms > row.latency_slo_ms
            || !matches!(
                row.evidence_state,
                WorkloadEvidenceState::Proven | WorkloadEvidenceState::Supported
            )
            || !valid_digest(&row.artifact_digest)
            || !valid_digest(&row.provenance_digest)
        {
            degraded.insert(id);
            uncertainty.insert(format!(
                "{}:reliability-threshold-or-evidence",
                row.invocation_id
            ));
        } else {
            dependable.insert(id);
        }
    }
    let required_missing = workload
        .required_invocation_order
        .iter()
        .filter(|id| !invocation_order.contains(id))
        .cloned()
        .collect::<BTreeSet<_>>();
    missing.extend(required_missing.iter().cloned());
    omission.extend(
        required_missing
            .iter()
            .map(|id| format!("request:missing-invocation:{id}")),
    );
    let global_block = !workload.policy_allow
        || !workload.protected_closure
        || !workload.signed_approval
        || !workload.network_available
        || !workload.raw_data_local
        || !workload.aggregate_only
        || !workload.adversarial_event_order.is_empty();
    if global_block {
        blocked.extend(invocation_order.iter().cloned());
        dependable.clear();
        degraded.clear();
        omission.insert("request:security-policy-protected-closure-or-network-blocked".into());
    }
    uncertainty.extend(
        workload
            .adversarial_event_order
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    if budget_used_units > workload.budget_units {
        blocked.extend(invocation_order.iter().cloned());
        dependable.clear();
        degraded.clear();
        omission.insert("request:budget-exhausted".into());
    }
    let dependable_order = dependable.into_iter().collect::<Vec<_>>();
    let degraded_order = degraded.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_order = missing.into_iter().collect::<Vec<_>>();
    let endpoint_order = endpoints.into_iter().collect::<Vec<_>>();
    let retry_order = retry.into_iter().collect::<Vec<_>>();
    let timeout_order = timeout.into_iter().collect::<Vec<_>>();
    let duplicate_event_order = duplicate.into_iter().collect::<Vec<_>>();
    let migration_order: Vec<String> = Vec::new();
    let omission_order = omission.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let disposition = if global_block
        || budget_used_units > workload.budget_units
        || dependable_order.is_empty() && degraded_order.is_empty()
    {
        "blocked"
    } else if !degraded_order.is_empty() || !blocked_order.is_empty() || !missing_order.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let mut payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":workload.request_id,"scope":workload.scope,"federation_id":workload.federation_id,"institution_id":workload.institution_id,"capability_id":workload.capability_id,"disposition":disposition,"invocation_order":invocation_order,"dependable_order":dependable_order,"degraded_order":degraded_order,"blocked_order":blocked_order,"missing_order":missing_order,"endpoint_order":endpoint_order,"retry_order":retry_order,"timeout_order":timeout_order,"duplicate_event_order":duplicate_event_order,"migration_order":migration_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"adversarial_event_order":workload.adversarial_event_order,"budget_used_units":budget_used_units,"replay_identity":workload.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let result_digest = digest(&payload)?;
    payload["result_digest"] = json!(result_digest);
    let semantic_loss = payload["omission_order"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| {
            value.as_str().map(|field| SemanticLoss {
                field: field.into(),
                reason: "reliability gate or migration boundary".into(),
                severity: bioprism_foundation::LossSeverity::Unknown,
            })
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        artifact_id: format!("reliable-capability-result-6:{}", workload.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: result_digest.clone(),
        semantic_loss,
        provenance: Vec::new(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    payload["artifact"] = serde_json::to_value(artifact).map_err(|error| {
        PerformanceReliabilityInteroperabilityError::Artifact(error.to_string())
    })?;
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!(
            "exchange:permitted-artifacts:{}",
            workload.request_id
        )]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let result: ReliableCapabilityResult6 = serde_json::from_value(payload).map_err(|error| {
        PerformanceReliabilityInteroperabilityError::Artifact(error.to_string())
    })?;
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn invocation(id: &str) -> CapabilityInvocation5 {
        CapabilityInvocation5 {
            invocation_id: id.into(),
            capability_id: "capability:qc".into(),
            endpoint: "wes:v1".into(),
            input_digest: h(id),
            expected_tasks: 2,
            completed_tasks: 2,
            p95_latency_ms: 20,
            latency_slo_ms: 100,
            retry_count: 0,
            max_retries: 2,
            duplicate_events: 0,
            evidence_state: WorkloadEvidenceState::Supported,
            artifact_digest: h(id),
            provenance_digest: h("prov"),
            replay_identity: h("replay"),
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_order: vec![],
        }
    }
    fn workload() -> CapabilityWorkload4 {
        CapabilityWorkload4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req".into(),
            scope: "study".into(),
            federation_id: "fed".into(),
            institution_id: "inst".into(),
            capability_id: "capability:qc".into(),
            required_invocation_order: vec!["a".into(), "b".into()],
            invocations: vec![invocation("b"), invocation("a")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            network_available: true,
            raw_data_local: true,
            aggregate_only: true,
            budget_units: 10,
            adversarial_event_order: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            performance_reliability_interoperability_gateway_manifest().autonomy_tier,
            AutonomyTier::A2
        )
    }
    #[test]
    fn qualified() {
        let r = negotiate_performance_reliability(&workload()).unwrap();
        assert_eq!(r.disposition, "qualified")
    }
    #[test]
    fn retry_degrades() {
        let mut w = workload();
        w.invocations[0].retry_count = 3;
        let r = negotiate_performance_reliability(&w).unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.retry_order.is_empty())
    }
    #[test]
    fn policy_blocks() {
        let mut w = workload();
        w.policy_allow = false;
        let r = negotiate_performance_reliability(&w).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"])
    }
    #[test]
    fn duplicate_and_negative_visible() {
        let mut w = workload();
        w.invocations[0].duplicate_events = 1;
        w.invocations[0].negative_result = true;
        let r = negotiate_performance_reliability(&w).unwrap();
        assert!(!r.duplicate_event_order.is_empty());
        assert!(!r.negative_evidence_order.is_empty())
    }
    #[test]
    fn digest_stable() {
        let a = negotiate_performance_reliability(&workload()).unwrap();
        let b = negotiate_performance_reliability(&workload()).unwrap();
        assert_eq!(a.result_digest, b.result_digest)
    }
}
