//! Version-negotiated typed-determinism interoperability (`AFA-ids-P17-F24`).
//!
//! The gateway proves that independently produced research manifests can be
//! canonicalized and migrated without silently changing semantics. It emits
//! only digest-bound metadata; it never moves raw study data or upgrades an
//! incompatible schema into a pass.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P17-F24";
pub const CONTRACT_VERSION: &str =
    "ids-multimodal-version-negotiated-typed-determinism-interoperability-gateway/1.0";
pub const INPUT_SCHEMA: &str = "TypedDeterminismRequest7@1";
pub const OUTPUT_SCHEMA: &str = "TypedDeterminismReceipt8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.typed-determinism-receipt-8+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_ENDPOINTS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeterminismEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterminismEndpoint6 {
    pub endpoint_id: String,
    pub origin: String,
    pub capability_id: String,
    pub semantic_profile: String,
    pub offered_versions: Vec<String>,
    pub canonical_field_order: Vec<String>,
    pub canonical_input_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: DeterminismEvidenceState,
    pub local: bool,
    pub aggregate_only: bool,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedDeterminismRequest7 {
    pub request_id: String,
    pub capability_id: String,
    pub required_version: String,
    pub preferred_version: String,
    pub semantic_profile: String,
    pub canonical_field_order: Vec<String>,
    pub canonical_input_digest: ContentHash,
    pub endpoints: Vec<DeterminismEndpoint6>,
    pub replay_identity: ContentHash,
    pub checkpoint: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedDeterminismReceipt8Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedDeterminismReceipt8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub capability_id: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub endpoint_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub migrated_order: Vec<String>,
    pub approval_required_order: Vec<String>,
    pub incompatible_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_version_order: Vec<String>,
    pub missing_provenance_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub canonical_field_order: Vec<String>,
    pub canonical_input_digest: ContentHash,
    pub negotiated_version: String,
    pub replay_identity: ContentHash,
    pub receipt_digest: ContentHash,
    pub artifact: TypedDeterminismReceipt8Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TypedDeterminismError {
    #[error("invalid typed-determinism request: {0}")]
    Invalid(String),
    #[error("typed-determinism receipt failed validation: {0}")]
    Receipt(String),
}

pub fn typed_determinism_interoperability_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["interoperability engineer", "research SDK maintainer", "federation steward", "replay auditor"],
        "behavior": "negotiates typed schema versions and canonical field order across local aggregate-only research endpoints",
        "value": "prevents schema migration, semantic loss, or endpoint disagreement from becoming byte-level replay claims",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:permitted-artifacts", "manage:local-capability"],
        "permissions": ["read:local-capability-manifests", "request:version-negotiation"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}

impl TypedDeterminismReceipt8 {
    pub fn validate(&self) -> Result<(), TypedDeterminismError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.checkpoint == 0
            || !nonempty(&self.request_id)
            || !nonempty(&self.capability_id)
            || !nonempty(&self.semantic_profile)
            || !nonempty(&self.negotiated_version)
            || self.endpoint_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(TypedDeterminismError::Receipt(
                "identity, checkpoint, locality, endpoint, version, or effect closure is incomplete".into(),
            ));
        }
        for values in [
            &self.endpoint_order,
            &self.accepted_order,
            &self.migrated_order,
            &self.approval_required_order,
            &self.incompatible_order,
            &self.blocked_order,
            &self.missing_version_order,
            &self.missing_provenance_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.canonical_field_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(TypedDeterminismError::Receipt(
                    "typed-determinism ordering is not canonical".into(),
                ));
            }
        }
        let endpoint_ids = BTreeSet::from_iter(self.endpoint_order.iter().cloned());
        let partitions = self
            .accepted_order
            .iter()
            .chain(&self.migrated_order)
            .chain(&self.approval_required_order)
            .chain(&self.incompatible_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if endpoint_ids.len() != self.endpoint_order.len()
            || partitions.len() != endpoint_ids.len()
            || BTreeSet::from_iter(partitions.iter().cloned()) != endpoint_ids
        {
            return Err(TypedDeterminismError::Receipt(
                "endpoint states do not partition the endpoint order".into(),
            ));
        }
        if !digest(&self.canonical_input_digest)
            || !digest(&self.replay_identity)
            || !digest(&self.receipt_digest)
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.receipt_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|value| !digest(value))
        {
            return Err(TypedDeterminismError::Receipt(
                "typed-determinism digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(TypedDeterminismError::Receipt(
                "effect is outside the governed determinism gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, TypedDeterminismError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| TypedDeterminismError::Receipt(error.to_string()))?,
        )
        .map_err(|error| TypedDeterminismError::Receipt(error.to_string()))
    }
}

fn validate_request(request: &TypedDeterminismRequest7) -> Result<(), TypedDeterminismError> {
    if !nonempty(&request.request_id)
        || !nonempty(&request.capability_id)
        || !nonempty(&request.required_version)
        || !nonempty(&request.preferred_version)
        || !nonempty(&request.semantic_profile)
        || request.canonical_field_order.is_empty()
        || !canonical(&request.canonical_field_order)
        || request.endpoints.is_empty()
        || request.endpoints.len() > MAX_ENDPOINTS
        || request.checkpoint == 0
        || !digest(&request.canonical_input_digest)
        || !digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(TypedDeterminismError::Invalid(
            "request identity, versions, canonical fields, bounds, digests, or locality is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for endpoint in &request.endpoints {
        if !nonempty(&endpoint.endpoint_id)
            || !nonempty(&endpoint.origin)
            || !nonempty(&endpoint.capability_id)
            || !nonempty(&endpoint.semantic_profile)
            || endpoint.offered_versions.is_empty()
            || endpoint.canonical_field_order.is_empty()
            || !canonical(&endpoint.canonical_field_order)
            || !digest(&endpoint.canonical_input_digest)
            || !digest(&endpoint.provenance_digest)
            || !digest(&endpoint.replay_identity)
            || !ids.insert(endpoint.endpoint_id.clone())
        {
            return Err(TypedDeterminismError::Invalid(
                "endpoint identity, versions, field ordering, digests, or uniqueness is invalid"
                    .into(),
            ));
        }
    }
    Ok(())
}

pub fn negotiate_typed_determinism(
    request: &TypedDeterminismRequest7,
) -> Result<TypedDeterminismReceipt8, TypedDeterminismError> {
    validate_request(request)?;
    let mut endpoints = request.endpoints.clone();
    endpoints.sort_by(|left, right| left.endpoint_id.cmp(&right.endpoint_id));
    let endpoint_order = endpoints
        .iter()
        .map(|item| item.endpoint_id.clone())
        .collect::<Vec<_>>();
    let mut accepted = BTreeSet::new();
    let mut migrated = BTreeSet::new();
    let mut approval_required = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_version = BTreeSet::new();
    let mut missing_provenance = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for endpoint in &endpoints {
        let id = endpoint.endpoint_id.clone();
        provenance.insert(endpoint.provenance_digest.clone());
        if endpoint.evidence_state == DeterminismEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative_evidence.insert(format!("{id}:contradicted"));
            continue;
        }
        if endpoint.capability_id != request.capability_id
            || endpoint.semantic_profile != request.semantic_profile
            || !endpoint.local
            || !endpoint.aggregate_only
        {
            blocked.insert(id.clone());
            omission.insert(format!("{id}:identity-or-locality-mismatch"));
            continue;
        }
        if endpoint.canonical_input_digest != request.canonical_input_digest
            || endpoint.canonical_field_order != request.canonical_field_order
        {
            incompatible.insert(id.clone());
            omission.insert(format!("{id}:canonical-field-or-input-mismatch"));
            continue;
        }
        if endpoint.provenance_digest.as_str().len() != 64 {
            missing_provenance.insert(id.clone());
        }
        if !endpoint
            .offered_versions
            .contains(&request.required_version)
            && !endpoint
                .offered_versions
                .contains(&request.preferred_version)
        {
            missing_version.insert(id.clone());
            incompatible.insert(id.clone());
            uncertainty.insert(format!("{id}:required-and-preferred-versions-unavailable"));
            continue;
        }
        if endpoint.replay_identity != request.replay_identity || !endpoint.signed {
            approval_required.insert(id.clone());
            omission.insert(format!("{id}:replay-or-signature-approval-required"));
            continue;
        }
        if endpoint.evidence_state != DeterminismEvidenceState::Proven
            && endpoint.evidence_state != DeterminismEvidenceState::Supported
        {
            approval_required.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state-not-supported"));
            continue;
        }
        if endpoint
            .offered_versions
            .contains(&request.required_version)
        {
            accepted.insert(id);
        } else {
            migrated.insert(id.clone());
            omission.insert(format!(
                "{id}:preferred-version-migrated-to-required-contract"
            ));
        }
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only;
    if global_block {
        blocked.extend(endpoint_order.iter().cloned());
        accepted.clear();
        migrated.clear();
        approval_required.clear();
        incompatible.clear();
        omission.insert("request:governance-or-locality-denied".into());
    }
    let accepted_order = accepted.iter().cloned().collect::<Vec<_>>();
    let migrated_order = migrated.iter().cloned().collect::<Vec<_>>();
    let approval_required_order = approval_required.iter().cloned().collect::<Vec<_>>();
    let incompatible_order = incompatible.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let disposition = if global_block
        || (accepted_order.is_empty()
            && migrated_order.is_empty()
            && approval_required_order.is_empty())
    {
        "blocked"
    } else if accepted_order.is_empty() && migrated_order.is_empty() {
        "unresolved"
    } else if !approval_required_order.is_empty()
        || !incompatible_order.is_empty()
        || !blocked_order.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omission.insert("request:determinism-negotiation-not-closed".into());
    }
    let mut payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "capability_id": request.capability_id,
        "semantic_profile": request.semantic_profile,
        "checkpoint": request.checkpoint,
        "disposition": disposition,
        "endpoint_order": endpoint_order,
        "accepted_order": accepted_order,
        "migrated_order": migrated_order,
        "approval_required_order": approval_required_order,
        "incompatible_order": incompatible_order,
        "blocked_order": blocked_order,
        "missing_version_order": missing_version.iter().cloned().collect::<Vec<_>>(),
        "missing_provenance_order": missing_provenance.iter().cloned().collect::<Vec<_>>(),
        "omission_order": omission.iter().cloned().collect::<Vec<_>>(),
        "uncertainty_order": uncertainty.iter().cloned().collect::<Vec<_>>(),
        "negative_evidence_order": negative_evidence.iter().cloned().collect::<Vec<_>>(),
        "canonical_field_order": request.canonical_field_order,
        "canonical_input_digest": request.canonical_input_digest,
        "negotiated_version": request.required_version,
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let receipt_digest = ContentHash::of_value(&payload)
        .map_err(|error| TypedDeterminismError::Receipt(error.to_string()))?;
    payload["receipt_digest"] = json!(receipt_digest);
    payload["artifact"] = json!({
        "artifact_id": format!("typed-determinism-receipt-8:{}", request.request_id),
        "content_type": CONTENT_TYPE,
        "content_hash": receipt_digest,
        "semantic_loss": omission.iter().cloned().collect::<Vec<_>>(),
        "provenance_digests": provenance.iter().cloned().collect::<Vec<_>>(),
        "boundary": PRECLINICAL_BOUNDARY,
    });
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("exchange:permitted-artifacts:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let receipt: TypedDeterminismReceipt8 = serde_json::from_value(payload)
        .map_err(|error| TypedDeterminismError::Receipt(error.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(id: &str, version: &str) -> DeterminismEndpoint6 {
        let hash = ContentHash::parse("a".repeat(64)).expect("hash");
        DeterminismEndpoint6 {
            endpoint_id: id.into(),
            origin: format!("origin-{id}"),
            capability_id: "capability:qc".into(),
            semantic_profile: "ome-ngff".into(),
            offered_versions: vec![version.into()],
            canonical_field_order: vec!["algorithm".into(), "threshold".into()],
            canonical_input_digest: hash.clone(),
            provenance_digest: hash.clone(),
            replay_identity: hash,
            evidence_state: DeterminismEvidenceState::Supported,
            local: true,
            aggregate_only: true,
            signed: true,
        }
    }

    fn request(endpoints: Vec<DeterminismEndpoint6>) -> TypedDeterminismRequest7 {
        let hash = ContentHash::parse("a".repeat(64)).expect("hash");
        TypedDeterminismRequest7 {
            request_id: "det:req".into(),
            capability_id: "capability:qc".into(),
            required_version: "2.0".into(),
            preferred_version: "1.0".into(),
            semantic_profile: "ome-ngff".into(),
            canonical_field_order: vec!["algorithm".into(), "threshold".into()],
            canonical_input_digest: hash.clone(),
            endpoints,
            replay_identity: hash,
            checkpoint: 1,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            typed_determinism_interoperability_manifest()["autonomy_tier"],
            "A1"
        );
    }
    #[test]
    fn required_version_is_accepted() {
        let r = negotiate_typed_determinism(&request(vec![endpoint("b", "2.0")])).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.accepted_order, vec!["b"]);
        assert_eq!(r.effect_receipts.len(), 2);
    }
    #[test]
    fn preferred_version_is_migrated() {
        let r = negotiate_typed_determinism(&request(vec![endpoint("a", "1.0")])).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.migrated_order, vec!["a"]);
        assert!(!r.omission_order.is_empty());
    }
    #[test]
    fn unsupported_version_is_incompatible() {
        let r = negotiate_typed_determinism(&request(vec![endpoint("a", "0.5")])).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.incompatible_order, vec!["a"]);
    }
    #[test]
    fn locality_denial_blocks_all() {
        let mut q = request(vec![endpoint("a", "2.0")]);
        q.policy_allow = false;
        let r = negotiate_typed_determinism(&q).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn field_mismatch_is_incompatible() {
        let mut e = endpoint("a", "2.0");
        e.canonical_field_order = vec!["algorithm".into(), "units".into()];
        let r = negotiate_typed_determinism(&request(vec![e])).unwrap();
        assert_eq!(r.incompatible_order, vec!["a"]);
    }
    #[test]
    fn endpoint_order_is_canonical() {
        let r =
            negotiate_typed_determinism(&request(vec![endpoint("z", "2.0"), endpoint("a", "2.0")]))
                .unwrap();
        assert_eq!(r.endpoint_order, vec!["a", "z"]);
        assert!(r.digest().is_ok());
    }
}
