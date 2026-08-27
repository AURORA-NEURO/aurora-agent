//! Federated continual Bioethics contract-frontier assurance.
//!
//! Atlas feature: `AFA-bioethics-P25-F28`.  This verification harness checks a
//! versioned capability frontier before a consortium workflow relies on it. It
//! consumes declarations, not biological content, and therefore cannot infer an
//! ethical status or perform an instrument, network, or clinical action.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P25-F28";
pub const CONTRACT_VERSION: &str =
    "bioethics-federated-continual-contract-frontier-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "BioethicsContractInput4@1";
pub const OUTPUT_SCHEMA: &str = "BioethicsCapabilityManifest7@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.bioethics-capability-manifest-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsCapabilityDescriptor {
    pub capability_id: String,
    pub version: String,
    pub input_schema: String,
    pub output_schema: String,
    pub semantic_profile: String,
    pub surface_order: Vec<String>,
    pub permission_order: Vec<String>,
    pub capability_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub local_only: bool,
    pub permitted: bool,
    pub approved: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsContractInput {
    pub schema_version: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_capability_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub descriptors: Vec<BioethicsCapabilityDescriptor>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontierDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsCapabilityManifestReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: FrontierDisposition,
    pub capability_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub manifest_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContractFrontierError {
    #[error("invalid bioethics contract frontier: {0}")]
    Invalid(String),
    #[error("bioethics frontier artifact failed: {0}")]
    Artifact(String),
}

fn invalid(value: impl Into<String>) -> ContractFrontierError {
    ContractFrontierError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl BioethicsCapabilityManifestReceipt {
    pub fn validate(&self) -> Result<(), ContractFrontierError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.capability_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "frontier identity, locality, capabilities, or effects are incomplete",
            ));
        }
        for values in [
            &self.capability_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_capability_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("frontier ordering is not canonical"));
            }
        }
        let ids = self
            .capability_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("frontier states do not partition capabilities"));
        }
        for value in [
            &self.replay_identity,
            &self.manifest_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("frontier digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContractFrontierError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("frontier artifact type is invalid"));
        }
        if self.disposition == FrontierDisposition::Qualified
            && self.effect_receipts
                != [format!(
                    "verify:bioethics-capability-frontier:{}",
                    self.request_id
                )]
        {
            return Err(invalid("qualified frontier effect is invalid"));
        }
        if self.disposition != FrontierDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified frontier must block release"));
        }
        Ok(())
    }
}

pub fn contract_frontier_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioethics".into(),
        consumers: ["consortium operator".into(), "bioethicist".into(), "downstream research workflow".into()].into(),
        behavior: "verifies a versioned bioethics capability frontier under explicit evidence, compatibility, provenance, policy, and federation gates without inferring ethics or executing research".into(),
        value: "prevents revoked, semantically drifting, unsigned, or incomplete capability declarations from silently entering a federated preclinical workflow".into(),
        inputs: vec![TypedPort { name: "bioethics_contract_input".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "bioethics_capability_manifest".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_contract_frontier(
    input: &BioethicsContractInput,
) -> Result<BioethicsCapabilityManifestReceipt, ContractFrontierError> {
    validate_input(input)?;
    let mut rows = input.descriptors.clone();
    rows.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let capability_order = rows
        .iter()
        .map(|row| row.capability_id.clone())
        .collect::<Vec<_>>();
    let required = input
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let known = capability_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = required
        .difference(&known)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for row in &rows {
        if row.revoked || !row.local_only || !row.permitted || !row.approved {
            blocked.insert(row.capability_id.clone());
            if row.revoked {
                negative.insert(format!("{}:revoked", row.capability_id));
            }
            if !row.approved {
                uncertainty.insert(format!("{}:approval-missing", row.capability_id));
            }
        } else if row.replay_identity != input.replay_identity
            || row.semantic_profile != input.semantic_profile
            || !row.omissions.is_empty()
            || !row.uncertainty.is_empty()
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(row.capability_id.clone());
            if row.replay_identity != input.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", row.capability_id));
            }
        } else {
            selected.insert(row.capability_id.clone());
        }
        omissions.extend(
            row.omissions
                .iter()
                .map(|item| format!("{}:{item}", row.capability_id)),
        );
        uncertainty.extend(
            row.uncertainty
                .iter()
                .map(|item| format!("{}:{item}", row.capability_id)),
        );
    }
    for id in &missing {
        omissions.insert(format!("{id}:required-capability-missing"));
    }
    if !input.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !input.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !input.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !input.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        input
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !input.policy_allow
        || !input.protected_closure
        || !input.signed_approval
        || !input.federation_approved
        || !input.raw_data_local
        || !input.aggregate_only
        || !input.adversarial_events.is_empty();
    if global_block {
        blocked.extend(capability_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        missing.clear();
        omissions.insert("request:frontier-release-gate-blocked".into());
    }
    let disposition = if global_block {
        FrontierDisposition::Blocked
    } else if required.is_subset(&selected) && unresolved.is_empty() && blocked.is_empty() {
        FrontierDisposition::Qualified
    } else {
        FrontierDisposition::Unresolved
    };
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_capability_order = missing.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == FrontierDisposition::Qualified {
        vec![format!(
            "verify:bioethics-capability-frontier:{}",
            input.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":input.request_id,"federation_id":input.federation_id,"semantic_profile":input.semantic_profile,"disposition":disposition,"capability_order":capability_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_capability_order":missing_capability_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_evidence_order,"replay_identity":input.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":input.raw_data_local,"aggregate_only":input.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let manifest_digest = ContentHash::of_value(&payload)
        .map_err(|error| ContractFrontierError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("bioethics-capability-frontier:{}", input.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContractFrontierError::Artifact(error.to_string()))?;
    let receipt = BioethicsCapabilityManifestReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: input.request_id.clone(),
        federation_id: input.federation_id.clone(),
        semantic_profile: input.semantic_profile.clone(),
        disposition,
        capability_order: payload["capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_capability_order: payload["missing_capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        replay_identity: input.replay_identity.clone(),
        manifest_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: input.raw_data_local,
        aggregate_only: input.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_input(input: &BioethicsContractInput) -> Result<(), ContractFrontierError> {
    if input.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || input.request_id.trim().is_empty()
        || input.federation_id.trim().is_empty()
        || input.semantic_profile.trim().is_empty()
        || input.required_capability_order.is_empty()
        || !canonical(&input.required_capability_order)
        || input.descriptors.is_empty()
        || !digest(&input.replay_identity)
        || !canonical(&input.adversarial_events)
        || !input.raw_data_local
        || !input.aggregate_only
        || input.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "contract input identity, closure, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for row in &input.descriptors {
        if row.capability_id.trim().is_empty()
            || !ids.insert(row.capability_id.clone())
            || row.version.trim().is_empty()
            || row.input_schema.trim().is_empty()
            || row.output_schema.trim().is_empty()
            || row.semantic_profile.trim().is_empty()
            || row.surface_order.is_empty()
            || !canonical(&row.surface_order)
            || !canonical(&row.permission_order)
            || !digest(&row.capability_digest)
            || !digest(&row.provenance_digest)
            || !digest(&row.replay_identity)
            || !canonical(&row.omissions)
            || !canonical(&row.uncertainty)
        {
            return Err(invalid(format!(
                "capability {} is malformed or duplicated",
                row.capability_id
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
    fn input() -> BioethicsContractInput {
        let d = hash("capability");
        let descriptor = |id: &str| BioethicsCapabilityDescriptor {
            capability_id: id.into(),
            version: "1.0.0".into(),
            input_schema: "Input@1".into(),
            output_schema: "Output@1".into(),
            semantic_profile: "preclinical-neural".into(),
            surface_order: vec!["api".into(), "sdk".into()],
            permission_order: vec!["evaluate:capability-runs".into()],
            capability_digest: d.clone(),
            provenance_digest: d.clone(),
            replay_identity: d.clone(),
            evidence_state: EvidenceState::Supported,
            omissions: vec![],
            uncertainty: vec![],
            local_only: true,
            permitted: true,
            approved: true,
            revoked: false,
        };
        BioethicsContractInput {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "frontier:one".into(),
            federation_id: "fed:commons".into(),
            semantic_profile: "preclinical-neural".into(),
            required_capability_order: vec!["capability:a".into(), "capability:b".into()],
            replay_identity: d.clone(),
            descriptors: vec![descriptor("capability:a"), descriptor("capability:b")],
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            contract_frontier_assurance_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_frontier() {
        assert_eq!(
            assure_contract_frontier(&input()).unwrap().disposition,
            FrontierDisposition::Qualified
        );
    }
    #[test]
    fn deterministic_manifest() {
        let a = assure_contract_frontier(&input()).unwrap();
        let b = assure_contract_frontier(&input()).unwrap();
        assert_eq!(a.manifest_digest, b.manifest_digest);
    }
    #[test]
    fn missing_capability_unresolved() {
        let mut value = input();
        value.required_capability_order = vec!["capability:a".into(), "capability:c".into()];
        assert_eq!(
            assure_contract_frontier(&value).unwrap().disposition,
            FrontierDisposition::Unresolved
        );
    }
    #[test]
    fn revoked_capability_blocked() {
        let mut value = input();
        value.descriptors[0].revoked = true;
        let out = assure_contract_frontier(&value).unwrap();
        assert!(out.blocked_order.contains(&"capability:a".into()));
        assert_eq!(out.disposition, FrontierDisposition::Unresolved);
    }
    #[test]
    fn policy_blocks_frontier() {
        let mut value = input();
        value.policy_allow = false;
        assert_eq!(
            assure_contract_frontier(&value).unwrap().disposition,
            FrontierDisposition::Blocked
        );
    }
    #[test]
    fn adversarial_blocks_frontier() {
        let mut value = input();
        value.adversarial_events.push("poisoned-manifest".into());
        assert_eq!(
            assure_contract_frontier(&value).unwrap().disposition,
            FrontierDisposition::Blocked
        );
    }
}
