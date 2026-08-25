//! Federated research-object interoperability gateway.
//!
//! Atlas feature: `AFA-bioevalx-P16-F24`.
//!
//! This A2 gateway prepares only permitted, content-addressed release metadata for an approved
//! endpoint and pinned protocol. It never transports raw experimental data, silently upgrades
//! unknown evidence, or treats endpoint reachability as authority. A downstream federation
//! service performs signing and transport after consuming the receipt.

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

pub const FEATURE_ID: &str = "AFA-bioevalx-P16-F24";
pub const CONTRACT_VERSION: &str = "bioevalx-federated-release-gateway/1.0";
pub const MAX_RUNS: usize = 4096;
pub const MAX_PROTOCOLS: usize = 32;

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
pub enum GatewayDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun {
    pub run_id: String,
    pub release_id: String,
    pub scope: String,
    pub origin: String,
    pub purpose: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub state: RunState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationGatewayRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub scope: String,
    pub origin: String,
    pub purpose: String,
    pub endpoint: String,
    pub approved_endpoints: Vec<String>,
    pub protocol: String,
    pub pinned_protocols: Vec<String>,
    pub runs: Vec<ValidatedResearchRun>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub budget: u64,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject {
    pub run_id: String,
    pub release_id: String,
    pub origin: String,
    pub purpose: String,
    pub artifact_ids: Vec<String>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub endpoint: String,
    pub protocol: String,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationGatewayReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub endpoint: String,
    pub protocol: String,
    pub disposition: GatewayDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub release_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub evidence_order: Vec<String>,
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
    pub federation_artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederationGatewayError {
    #[error("invalid federation gateway request: {0}")]
    Invalid(String),
    #[error("federation gateway artifact failed: {0}")]
    Artifact(String),
    #[error("federation gateway serialization failed: {0}")]
    Serialization(String),
}

impl FederationGatewayReceipt {
    pub fn validate(&self) -> Result<(), FederationGatewayError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.protocol.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederationGatewayError::Invalid("federation identity, candidates, endpoint, protocol, locality, effects, or boundary is incomplete".into()));
        }
        if self
            .admitted_order
            .iter()
            .any(|id| !self.candidate_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederationGatewayError::Invalid(
                "candidate state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.release_order,
            &self.artifact_order,
            &self.evidence_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederationGatewayError::Invalid(
                    "federation gateway ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.provenance_order,
            &self.replay_order,
            &self.benchmark_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederationGatewayError::Invalid(
                    "federation gateway digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:federation-release"
                && !effect.starts_with("exchange:permitted-artifacts:")
        }) {
            return Err(FederationGatewayError::Invalid(
                "effect is outside permitted-artifacts exchange gate".into(),
            ));
        }
        for object in &self.objects {
            if !object.raw_data_local
                || object.boundary != PRECLINICAL_BOUNDARY
                || object.endpoint != self.endpoint
                || object.protocol != self.protocol
                || object.artifact_ids.is_empty()
                || object.evidence_receipt_ids.is_empty()
            {
                return Err(FederationGatewayError::Invalid(
                    "federation object is incomplete or inconsistent".into(),
                ));
            }
        }
        self.federation_artifact
            .validate_metadata()
            .map_err(|error| FederationGatewayError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederationGatewayError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederationGatewayError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederationGatewayError::Serialization(error.to_string()))
    }
}

pub fn federation_gateway_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioevalx".into(), consumers: ["research program lead".into(), "federation gateway operator".into(), "interoperability verifier".into()].into(), behavior: "prepares permitted content-addressed research-object metadata for approved federated endpoints and pinned protocols while retaining omissions, negative evidence, replay, provenance, and locality witnesses".into(), value: "enables continual multi-institution research-object interoperability without treating endpoint reachability or incomplete evidence as authorization".into(), inputs: vec![TypedPort { name: "federation_gateway_request".into(), schema: "ValidatedResearchRun4@1".into(), required: true }], outputs: vec![TypedPort { name: "federation_envelope".into(), schema: "SignedResearchObject6@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["connect:approved-endpoints".into(), "exchange:permitted-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: vec![AuthorityRequirement { role: "institutional federation steward".into(), reason: "approve endpoint, purpose, protocol, and permitted artifact exchange".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into()
    }
}

pub fn prepare_federation_release(
    request: &FederationGatewayRequest,
) -> Result<FederationGatewayReceipt, FederationGatewayError> {
    validate_request(request)?;
    let endpoint_ok = request.approved_endpoints.contains(&request.endpoint);
    let protocol_ok = request.pinned_protocols.contains(&request.protocol);
    let mut runs = request.runs.clone();
    runs.sort_by(|left, right| {
        left.release_id
            .cmp(&right.release_id)
            .then(left.run_id.cmp(&right.run_id))
    });
    let candidate_order = runs
        .iter()
        .map(|run| run.release_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut releases = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
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
            + run.evidence_receipt_ids.len()) as u64
            + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = endpoint_ok
            && protocol_ok
            && request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && run.raw_data_local
            && run.state == RunState::Supported
            && run.scope == request.scope
            && !run.artifact_ids.is_empty()
            && !run.evidence_receipt_ids.is_empty()
            && run.benchmark_digest.is_some()
            && request.benchmark_digest.is_some()
            && run.replay_identity == request.replay_identity
            && run.provenance_digest != ContentHash::of_bytes(b"")
            && run.omissions.is_empty()
            && run.uncertainty.is_empty()
            && run.negative_evidence.is_empty()
            && budget_ok;
        if complete {
            spent = spent.saturating_add(cost);
            admitted.push(run.release_id.clone());
            releases.insert(run.release_id.clone());
            artifacts.extend(run.artifact_ids.iter().cloned());
            evidence.extend(run.evidence_receipt_ids.iter().cloned());
            provenance.insert(run.provenance_digest.clone());
            replay.insert(run.replay_identity.clone());
            if let Some(digest) = &run.benchmark_digest {
                benchmarks.insert(digest.clone());
            }
            objects.push(SignedResearchObject {
                run_id: run.run_id.clone(),
                release_id: run.release_id.clone(),
                origin: run.origin.clone(),
                purpose: run.purpose.clone(),
                artifact_ids: run.artifact_ids.clone(),
                evidence_receipt_ids: run.evidence_receipt_ids.clone(),
                release_digest: run.release_digest.clone(),
                provenance_digest: run.provenance_digest.clone(),
                replay_identity: run.replay_identity.clone(),
                benchmark_digest: run.benchmark_digest.clone().expect("checked above"),
                endpoint: request.endpoint.clone(),
                protocol: request.protocol.clone(),
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
            if !endpoint_ok {
                negative.insert("request:endpoint-not-approved".into());
            }
            if !protocol_ok {
                omissions.insert(format!("request:protocol-not-pinned:{}", request.protocol));
            }
            if !request.policy_allow {
                negative.insert("request:policy-denied".into());
            }
            if !request.protected_closure {
                uncertainty.insert("request:protected-closure-incomplete".into());
            }
            if !request.signed_approval {
                omissions.insert("request:signed-approval-required".into());
            }
            if !request.raw_data_local || !run.raw_data_local {
                negative.insert(format!(
                    "release:{}:raw-data-locality-failed",
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
            if run.replay_identity != request.replay_identity {
                uncertainty.insert(format!("release:{}:replay-mismatch", run.release_id));
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
        }
    }
    if request.benchmark_digest.is_none() {
        uncertainty.insert("request:benchmark-missing".into());
    }
    let disposition = if !endpoint_ok
        || !protocol_ok
        || !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
    {
        GatewayDisposition::Blocked
    } else if admitted.is_empty() {
        GatewayDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        GatewayDisposition::Qualified
    } else {
        GatewayDisposition::Partial
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "federation_id": request.federation_id, "endpoint": request.endpoint, "protocol": request.protocol, "disposition": disposition, "candidate_order": candidate_order, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "release_order": releases, "artifact_order": artifacts, "evidence_order": evidence, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "benchmark_digest": request.benchmark_digest, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let federation_artifact = TypedResearchArtifact::from_payload(
        format!("federation-release:{}", request.request_id),
        "application/vnd.aurora.federation-release+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederationGatewayError::Artifact(error.to_string()))?;
    let effect_receipts = if admitted.is_empty() {
        vec!["block:federation-release".into()]
    } else {
        vec![format!(
            "exchange:permitted-artifacts:{}",
            request.request_id
        )]
    };
    let receipt = FederationGatewayReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        endpoint: request.endpoint.clone(),
        protocol: request.protocol.clone(),
        disposition,
        candidate_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        release_order: releases.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
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
        federation_artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederationGatewayRequest) -> Result<(), FederationGatewayError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.origin.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.protocol.trim().is_empty()
        || request.approved_endpoints.is_empty()
        || request.pinned_protocols.is_empty()
        || request.pinned_protocols.len() > MAX_PROTOCOLS
        || request.runs.is_empty()
        || request.runs.len() > MAX_RUNS
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederationGatewayError::Invalid("federation request identity, endpoint/protocol allow-list, runs, budget, or boundary is incomplete".into()));
    }
    let mut ids = BTreeSet::new();
    for run in &request.runs {
        if run.run_id.trim().is_empty()
            || run.release_id.trim().is_empty()
            || run.origin.trim().is_empty()
            || run.purpose.trim().is_empty()
            || run.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(run.release_id.clone())
        {
            return Err(FederationGatewayError::Invalid(format!(
                "release {} is invalid or duplicated",
                run.release_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn run(id: &str, state: RunState) -> ValidatedResearchRun {
        ValidatedResearchRun {
            run_id: format!("run:{id}"),
            release_id: format!("release:{id}"),
            scope: "organoid:neural".into(),
            origin: "institution:alpha".into(),
            purpose: "preclinical-commons".into(),
            artifact_ids: vec![format!("artifact:{id}")],
            evidence_receipt_ids: vec![format!("evidence:{id}")],
            release_digest: hash(&format!("release:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            state,
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(runs: Vec<ValidatedResearchRun>) -> FederationGatewayRequest {
        FederationGatewayRequest {
            request_id: "request:federation".into(),
            workflow_id: "workflow:publish".into(),
            federation_id: "federation:commons".into(),
            scope: "organoid:neural".into(),
            origin: "institution:alpha".into(),
            purpose: "preclinical-commons".into(),
            endpoint: "https://hub.example/research".into(),
            approved_endpoints: vec!["https://hub.example/research".into()],
            protocol: "mcp/2025-06-18".into(),
            pinned_protocols: vec!["mcp/2025-06-18".into()],
            runs,
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            budget: 10_000,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_typed_a2() {
        let manifest = federation_gateway_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn approved_endpoint_and_protocol_qualify() {
        let receipt = prepare_federation_release(&request(vec![
            run("b", RunState::Supported),
            run("a", RunState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::Qualified);
        assert_eq!(receipt.candidate_order, vec!["release:a", "release:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_runs_remain_visible() {
        let receipt = prepare_federation_release(&request(vec![
            run("a", RunState::Supported),
            run("b", RunState::Unknown),
            run("c", RunState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"release:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("release:c")));
    }
    #[test]
    fn endpoint_denial_blocks_exchange() {
        let mut input = request(vec![run("a", RunState::Supported)]);
        input.endpoint = "https://unapproved.example".into();
        let receipt = prepare_federation_release(&input).unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:federation-release"]);
    }
    #[test]
    fn protocol_migration_is_omitted() {
        let mut input = request(vec![run("a", RunState::Supported)]);
        input.protocol = "ga4gh-wes/1.0".into();
        let receipt = prepare_federation_release(&input).unwrap();
        assert_eq!(receipt.disposition, GatewayDisposition::Blocked);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("protocol-not-pinned")));
    }
    #[test]
    fn duplicate_release_is_rejected() {
        let mut duplicate = run("a", RunState::Supported);
        duplicate.run_id = "run:other".into();
        assert!(prepare_federation_release(&request(vec![
            run("a", RunState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
