//! Federated continual scope interoperability gateway (`AFA-scope-P31-F24`).
//!
//! The gateway admits digest-bound scope summaries from institution-local peers, negotiates
//! semantic compatibility and migration requirements, and emits a deterministic exchange
//! receipt.  It never moves raw experimental data, resolves clinical questions, or treats an
//! incomplete protected closure as a successful exchange.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-scope-P31-F24";
pub const CONTRACT_VERSION: &str = "scope-federated-continual-scope-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "ScopeFederationGatewayRequest7@1";
pub const OUTPUT_SCHEMA: &str = "ScopeFederationGatewayReceipt10@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.scope-federation-gateway-receipt-10+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeGatewayArtifact4 {
    pub artifact_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub available: bool,
    pub permitted: bool,
    pub signed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopePeerManifest4 {
    pub peer_id: String,
    pub capability_id: String,
    pub schema: String,
    pub semantic_profile: String,
    pub scope: String,
    pub checkpoint_seq: u64,
    pub signed: bool,
    pub policy_allowed: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeFederationGatewayRequest7 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub source_scope: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub capability_id: String,
    pub required_schema: String,
    pub checkpoint_seq: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub signed_approval: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub boundary: String,
    pub artifacts: Vec<ScopeGatewayArtifact4>,
    pub peers: Vec<ScopePeerManifest4>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeFederationReceiptArtifact10 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeFederationGatewayReceipt10 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub source_scope: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub capability_id: String,
    pub required_schema: String,
    pub checkpoint_seq: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub compatible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub qualified_peer_order: Vec<String>,
    pub missing_peer_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub gateway_digest: ContentHash,
    pub artifact: ScopeFederationReceiptArtifact10,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ScopeGatewayError {
    #[error("invalid scope interoperability gateway request or receipt: {0}")]
    Invalid(String),
    #[error("scope interoperability gateway artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn federated_scope_interoperability_manifest() -> serde_json::Value {
    json!({"schema_version":SCHEMA_VERSION,"capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"scope","consumers":["federation interoperability steward","scope migration operator","downstream research workflow"],"behavior":"negotiate continual federated scope and schema compatibility from signed digest-only peer manifests","value":"prevents incomparable scope summaries, unauthorized exports, and silent semantic loss at a federation boundary","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["execute:local-computation","write:local-artifact","exchange:permitted-aggregates"],"permissions":["read:local-research-summaries","exchange:permitted-aggregates"],"determinism":"byte_stable","evidence":[{"source_id":"w3c-prov-o","state":"supported","locator":"https://www.w3.org/TR/prov-o/"}],"authority_requirements":["institution-federation-approval"],"autonomy_tier":"A2","surfaces":["ui","cli","api","sdk","mcp_tool","policy","operator"],"boundary":PRECLINICAL_BOUNDARY})
}

impl ScopeFederationGatewayReceipt10 {
    pub fn validate(&self) -> Result<(), ScopeGatewayError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.checkpoint_seq == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || [
                &self.request_id,
                &self.consumer,
                &self.purpose,
                &self.source_scope,
                &self.target_scope,
                &self.semantic_profile,
                &self.capability_id,
                &self.required_schema,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(ScopeGatewayError::Invalid(
                "gateway identity, scopes, bounds, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.compatible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_order,
            &self.migration_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.peer_order,
            &self.qualified_peer_order,
            &self.missing_peer_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ScopeGatewayError::Invalid(
                    "gateway ordering is not canonical".into(),
                ));
            }
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let states = self
            .compatible_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.missing_order)
            .cloned()
            .collect::<Vec<_>>();
        if candidates.len() != self.candidate_order.len()
            || states.len() != candidates.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != candidates
        {
            return Err(ScopeGatewayError::Invalid(
                "gateway candidate states do not partition".into(),
            ));
        }
        let peers = self.peer_order.iter().cloned().collect::<BTreeSet<_>>();
        let peer_states = self
            .qualified_peer_order
            .iter()
            .chain(&self.missing_peer_order)
            .cloned()
            .collect::<Vec<_>>();
        if peers.len() != self.peer_order.len()
            || peer_states.len() != peers.len()
            || peer_states.iter().cloned().collect::<BTreeSet<_>>() != peers
        {
            return Err(ScopeGatewayError::Invalid(
                "gateway peer states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.checkpoint_digest)
            || !digest(&self.gateway_digest)
            || self.artifact.content_hash != self.gateway_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(ScopeGatewayError::Artifact(
                "gateway or provenance digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("exchange:scope-summary:")
        }) {
            return Err(ScopeGatewayError::Invalid(
                "effect is outside scope summary exchange gate".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("exchange:scope-summary:{}", self.request_id)]
        {
            return Err(ScopeGatewayError::Invalid(
                "qualified scope exchange effect is invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(ScopeGatewayError::Invalid(
                "non-qualified scope exchange must block".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ScopeGatewayError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self).map_err(|e| ScopeGatewayError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ScopeGatewayError::Artifact(e.to_string()))
    }
}

pub fn operate_federated_scope_interoperability_gateway(
    request: &ScopeFederationGatewayRequest7,
) -> Result<ScopeFederationGatewayReceipt10, ScopeGatewayError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.source_scope.trim().is_empty()
        || request.target_scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.capability_id.trim().is_empty()
        || request.required_schema.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.artifacts.is_empty()
        || request.peers.is_empty()
        || !digest(&request.replay_identity)
        || !request.aggregate_only
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ScopeGatewayError::Invalid(
            "request identity, manifests, bounds, replay, locality, or boundary is invalid".into(),
        ));
    }
    let candidate_order = request
        .artifacts
        .iter()
        .map(|a| a.artifact_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if candidate_order.len() != request.artifacts.len()
        || candidate_order.iter().any(|id| id.trim().is_empty())
    {
        return Err(ScopeGatewayError::Invalid(
            "artifact ids must be unique and non-empty".into(),
        ));
    }
    let mut compatible = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty: BTreeSet<String> = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for artifact in &request.artifacts {
        let valid = artifact.available
            && artifact.permitted
            && artifact.signed
            && artifact.aggregate_only
            && artifact.raw_data_local
            && artifact.scope == request.source_scope
            && artifact.semantic_profile == request.semantic_profile
            && digest(&artifact.content_digest)
            && digest(&artifact.provenance_digest);
        if artifact.negative_result {
            negative.insert(artifact.artifact_id.clone());
        }
        if !artifact.available {
            unresolved.insert(artifact.artifact_id.clone());
            omissions.insert(format!("artifact:{}:unavailable", artifact.artifact_id));
        } else if !valid {
            blocked.insert(artifact.artifact_id.clone());
            omissions.insert(format!(
                "artifact:{}:policy-or-integrity",
                artifact.artifact_id
            ));
        } else if artifact.semantic_profile != request.semantic_profile {
            unresolved.insert(artifact.artifact_id.clone());
            migration.insert(format!("{}:semantic-profile", artifact.artifact_id));
        } else if artifact.scope != request.source_scope {
            unresolved.insert(artifact.artifact_id.clone());
            migration.insert(format!("{}:scope-map", artifact.artifact_id));
        } else {
            compatible.insert(artifact.artifact_id.clone());
        }
    }
    let peer_order = request
        .peers
        .iter()
        .map(|p| p.peer_id.clone())
        .collect::<BTreeSet<_>>();
    if peer_order.len() != request.peers.len() || peer_order.iter().any(|id| id.trim().is_empty()) {
        return Err(ScopeGatewayError::Invalid(
            "peer ids must be unique and non-empty".into(),
        ));
    }
    let qualified_peer_order = request
        .peers
        .iter()
        .filter(|peer| {
            peer.signed
                && peer.policy_allowed
                && peer.aggregate_only
                && peer.raw_data_local
                && peer.capability_id == request.capability_id
                && peer.schema == request.required_schema
                && peer.semantic_profile == request.semantic_profile
                && peer.scope == request.target_scope
                && peer.checkpoint_seq == request.checkpoint_seq
        })
        .map(|peer| peer.peer_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_peer_order = peer_order
        .difference(&qualified_peer_order)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_peer_order.is_empty() {
        omissions.insert(format!("peer-missing:{}", missing_peer_order.join(",")));
        uncertainty.insert("peer-compatibility-incomplete".into());
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.federation_approved {
        omissions.insert("workflow:federation-approval-missing".into());
    }
    if !request.signed_approval {
        omissions.insert("workflow:signed-approval-missing".into());
    }
    let globally_blocked = !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || !request.signed_approval;
    let disposition = if globally_blocked || !blocked.is_empty() {
        "blocked"
    } else if compatible.is_empty() || !unresolved.is_empty() || !missing_peer_order.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("workflow:closure-incomplete".into());
    }
    if globally_blocked {
        blocked.extend(candidate_order.iter().cloned());
        compatible.clear();
        unresolved.clear();
    }
    let checkpoint_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "checkpoint_seq": request.checkpoint_seq,
        "source_scope": request.source_scope,
        "target_scope": request.target_scope,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|e| ScopeGatewayError::Artifact(e.to_string()))?;
    let payload = json!({
        "candidate_order": candidate_order,
        "compatible_order": compatible,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "missing_order": BTreeSet::<String>::new(),
        "migration_order": migration,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "peer_order": peer_order,
        "qualified_peer_order": qualified_peer_order,
        "missing_peer_order": missing_peer_order,
        "checkpoint_digest": checkpoint_digest,
        "replay_identity": request.replay_identity,
    });
    let gateway_digest =
        ContentHash::of_value(&payload).map_err(|e| ScopeGatewayError::Artifact(e.to_string()))?;
    let strings = |name: &str| {
        payload[name]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let receipt = ScopeFederationGatewayReceipt10 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        purpose: request.purpose.clone(),
        source_scope: request.source_scope.clone(),
        target_scope: request.target_scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        capability_id: request.capability_id.clone(),
        required_schema: request.required_schema.clone(),
        checkpoint_seq: request.checkpoint_seq,
        disposition: disposition.into(),
        candidate_order: strings("candidate_order"),
        compatible_order: strings("compatible_order"),
        unresolved_order: strings("unresolved_order"),
        blocked_order: strings("blocked_order"),
        missing_order: strings("missing_order"),
        migration_order: strings("migration_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        peer_order: strings("peer_order"),
        qualified_peer_order: strings("qualified_peer_order"),
        missing_peer_order: strings("missing_peer_order"),
        replay_identity: request.replay_identity.clone(),
        checkpoint_digest,
        gateway_digest: gateway_digest.clone(),
        artifact: ScopeFederationReceiptArtifact10 {
            artifact_id: format!("scope-federation-gateway:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: gateway_digest,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["scope-exchange-not-qualified".into()]
            },
            provenance_digests: request
                .artifacts
                .iter()
                .map(|a| a.provenance_digest.clone())
                .collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("exchange:scope-summary:{}", request.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
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

    fn request() -> ScopeFederationGatewayRequest7 {
        ScopeFederationGatewayRequest7 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "scope-req".into(),
            consumer: "workflow".into(),
            purpose: "cross-site scope summary".into(),
            source_scope: "site-a:organoid".into(),
            target_scope: "site-b:organoid".into(),
            semantic_profile: "scope:v2".into(),
            capability_id: "scope-gateway".into(),
            required_schema: "ScopeSummary@2".into(),
            checkpoint_seq: 3,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            signed_approval: true,
            aggregate_only: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            artifacts: vec![ScopeGatewayArtifact4 {
                artifact_id: "artifact-1".into(),
                scope: "site-a:organoid".into(),
                semantic_profile: "scope:v2".into(),
                content_digest: h("content"),
                provenance_digest: h("provenance"),
                available: true,
                permitted: true,
                signed: true,
                aggregate_only: true,
                raw_data_local: true,
                negative_result: false,
            }],
            peers: vec![ScopePeerManifest4 {
                peer_id: "site-b".into(),
                capability_id: "scope-gateway".into(),
                schema: "ScopeSummary@2".into(),
                semantic_profile: "scope:v2".into(),
                scope: "site-b:organoid".into(),
                checkpoint_seq: 3,
                signed: true,
                policy_allowed: true,
                aggregate_only: true,
                raw_data_local: true,
            }],
        }
    }

    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            federated_scope_interoperability_manifest()["autonomy_tier"],
            "A2"
        );
    }

    #[test]
    fn qualified_exchange_is_deterministic() {
        let left = operate_federated_scope_interoperability_gateway(&request()).unwrap();
        let right = operate_federated_scope_interoperability_gateway(&request()).unwrap();
        assert_eq!(left, right);
        assert_eq!(left.disposition, "qualified");
    }

    #[test]
    fn policy_denial_blocks_without_export() {
        let mut input = request();
        input.policy_allow = false;
        let receipt = operate_federated_scope_interoperability_gateway(&input).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn missing_peer_is_partial_and_explicit() {
        let mut input = request();
        input.peers[0].checkpoint_seq = 2;
        let receipt = operate_federated_scope_interoperability_gateway(&input).unwrap();
        assert_eq!(receipt.disposition, "partial");
        assert_eq!(receipt.missing_peer_order, vec!["site-b"]);
        assert!(receipt
            .omission_order
            .iter()
            .any(|v| v.starts_with("peer-missing:")));
    }
}
