//! Federated continual BioLang contract-frontier workbench.
//!
//! Atlas feature: `AFA-biolang-P25-F20`. The workbench exposes versioned BioLang capability
//! manifests for researchers and operators. It validates compatibility and evidence but never
//! executes a query, mutates a world, moves raw data, or makes a clinical decision.

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

pub const FEATURE_ID: &str = "AFA-biolang-P25-F20";
pub const CONTRACT_VERSION: &str = "biolang-federated-continual-contract-frontier-workbench/1.0";
pub const INPUT_SCHEMA: &str = "BiolangContractInput4@1";
pub const OUTPUT_SCHEMA: &str = "BiolangCapabilityManifest5@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.biolang-capability-manifest-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractDescriptor {
    pub contract_id: String,
    pub version: String,
    pub input_schema: String,
    pub output_schema: String,
    pub surface: String,
    pub semantic_profile: String,
    pub capability_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub compatibility_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_data: bool,
    pub permitted: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiolangContractInput {
    pub request_id: String,
    pub federation_id: String,
    pub operator_id: String,
    pub requested_surface: String,
    pub semantic_profile: String,
    pub required_contract_order: Vec<String>,
    pub descriptors: Vec<ContractDescriptor>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiolangCapabilityManifest {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub operator_id: String,
    pub requested_surface: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub contract_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_contract_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
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
    #[error("invalid BioLang contract input: {0}")]
    Invalid(String),
    #[error("BioLang capability artifact failed: {0}")]
    Artifact(String),
}
fn invalid(v: impl Into<String>) -> ContractFrontierError {
    ContractFrontierError::Invalid(v.into())
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64
}

pub fn build_contract_frontier_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"biolang".into(),consumers:["imaging core scientist".into(),"BioLang extension maintainer".into(),"federation operator".into()].into(),behavior:"exposes versioned BioLang capability manifests with compatibility, evidence, provenance, replay, and authorization witnesses".into(),value:"lets researchers inspect and compare BioLang contract capabilities across institutions without executing unverified code or exporting raw observations".into(),inputs:vec![TypedPort{name:"biolang_contract_input".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"biolang_capability_manifest".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["view:authorized-research-state".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"json-schema".into(),state:EvidenceState::Supported,locator:Some("https://json-schema.org/draft/2020-12/schema".into())}],authority_requirements:vec![AuthorityRequirement{role:"research-state viewer".into(),reason:"contract manifests may expose institution-sensitive capability metadata".into()}],autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Operator,ResearchSurface::Policy].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

impl BiolangCapabilityManifest {
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
            || self.operator_id.trim().is_empty()
            || self.requested_surface.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "unresolved" | "blocked"
            )
            || self.contract_order.is_empty()
            || self.ranked_order.len() != self.contract_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "BioLang manifest identity, locality, ranking, or effects are incomplete",
            ));
        }
        for v in [
            &self.contract_order,
            &self.selected_order,
            &self.unknown_order,
            &self.blocked_order,
            &self.missing_contract_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(v) {
                return Err(invalid("BioLang manifest ordering is not canonical"));
            }
        }
        let ids = self.contract_order.iter().collect::<BTreeSet<_>>();
        let parts = self
            .selected_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.blocked_order.iter())
            .collect::<Vec<_>>();
        if parts.len() != ids.len()
            || parts.iter().any(|x| !ids.contains(x))
            || parts.iter().collect::<BTreeSet<_>>().len() != parts.len()
            || self.ranked_order.iter().collect::<BTreeSet<_>>() != ids
        {
            return Err(invalid(
                "BioLang contract states do not partition descriptors",
            ));
        }
        for d in [
            &self.replay_identity,
            &self.manifest_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(d) {
                return Err(invalid("BioLang manifest digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| ContractFrontierError::Artifact(e.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("BioLang manifest artifact type is invalid"));
        }
        if self.disposition == "qualified" {
            if self.effect_receipts.len() != 1
                || self.effect_receipts[0] != format!("view:contract-manifest:{}", self.request_id)
            {
                return Err(invalid("qualified BioLang view effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid("non-qualified BioLang manifest must block release"));
        }
        Ok(())
    }
}

pub fn validate_contract_frontier(
    input: &BiolangContractInput,
) -> Result<BiolangCapabilityManifest, ContractFrontierError> {
    validate_input(input)?;
    let mut rows = input.descriptors.clone();
    rows.sort_by(|a, b| {
        a.contract_id
            .cmp(&b.contract_id)
            .then(a.version.cmp(&b.version))
    });
    let ranked = rows
        .iter()
        .map(|x| x.contract_id.clone())
        .collect::<Vec<_>>();
    let mut order = ranked.clone();
    order.sort();
    let required = input
        .required_contract_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required
        .iter()
        .filter(|x| !order.contains(x))
        .cloned()
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for d in &rows {
        if d.negative_result {
            negative.insert(format!("{}:negative-result", d.contract_id));
        }
        omission.extend(d.omissions.iter().map(|x| format!("{}:{x}", d.contract_id)));
        uncertainty.extend(
            d.uncertainty
                .iter()
                .map(|x| format!("{}:{x}", d.contract_id)),
        );
        if d.evidence_state == EvidenceState::Contradicted {
            blocked.insert(d.contract_id.clone());
            negative.insert(format!("{}:contradicted-contract", d.contract_id));
            continue;
        }
        if matches!(
            d.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unknown.insert(d.contract_id.clone());
            uncertainty.insert(format!("{}:evidence-unresolved", d.contract_id));
            continue;
        }
        let complete = d.surface == input.requested_surface
            && d.semantic_profile == input.semantic_profile
            && d.local_data
            && d.permitted
            && d.omissions.is_empty()
            && d.uncertainty.is_empty()
            && digest(&d.capability_digest)
            && digest(&d.provenance_digest)
            && digest(&d.compatibility_digest)
            && d.replay_identity == input.replay_identity;
        if complete
            && matches!(
                d.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            selected.insert(d.contract_id.clone());
        } else {
            unknown.insert(d.contract_id.clone());
            if d.surface != input.requested_surface {
                omission.insert(format!("{}:surface-mismatch", d.contract_id));
            }
            if d.semantic_profile != input.semantic_profile {
                omission.insert(format!("{}:semantic-profile-mismatch", d.contract_id));
            }
            if d.replay_identity != input.replay_identity {
                omission.insert(format!("{}:replay-mismatch", d.contract_id));
            }
            if !d.local_data || !d.permitted {
                blocked.insert(d.contract_id.clone());
                unknown.remove(&d.contract_id);
                omission.insert(format!("{}:locality-or-permission-denied", d.contract_id));
            }
        }
    }
    for id in &missing {
        omission.insert(format!("{id}:required-contract-missing"));
    }
    if !input.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !input.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !input.signed_approval || !input.federation_approved {
        uncertainty.insert("request:institutional-approval-incomplete".into());
    }
    negative.extend(
        input
            .adversarial_events
            .iter()
            .map(|x| format!("adversarial:{x}")),
    );
    let global = !input.policy_allow
        || !input.protected_closure
        || !input.signed_approval
        || !input.federation_approved
        || !input.raw_data_local
        || !input.aggregate_only
        || !input.adversarial_events.is_empty();
    let disposition = if global {
        "blocked"
    } else if missing.is_empty() && !selected.is_empty() && unknown.is_empty() && blocked.is_empty()
    {
        "qualified"
    } else {
        "unresolved"
    };
    let selected = selected.into_iter().collect::<Vec<_>>();
    let unknown = unknown.into_iter().collect::<Vec<_>>();
    let blocked = blocked.into_iter().collect::<Vec<_>>();
    let omission = omission.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative = negative.into_iter().collect::<Vec<_>>();
    let effects = if disposition == "qualified" {
        vec![format!("view:contract-manifest:{}", input.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":input.request_id,"federation_id":input.federation_id,"operator_id":input.operator_id,"requested_surface":input.requested_surface,"semantic_profile":input.semantic_profile,"disposition":disposition,"contract_order":order,"ranked_order":ranked,"selected_order":selected,"unknown_order":unknown,"blocked_order":blocked,"missing_contract_order":missing,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"adversarial_event_order":input.adversarial_events,"replay_identity":input.replay_identity,"effect_receipts":effects,"raw_data_local":input.raw_data_local,"aggregate_only":input.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let manifest_digest = ContentHash::of_value(&payload)
        .map_err(|e| ContractFrontierError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("biolang-capability-manifest:{}", input.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| ContractFrontierError::Artifact(e.to_string()))?;
    let out = BiolangCapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: input.request_id.clone(),
        federation_id: input.federation_id.clone(),
        operator_id: input.operator_id.clone(),
        requested_surface: input.requested_surface.clone(),
        semantic_profile: input.semantic_profile.clone(),
        disposition: disposition.into(),
        contract_order: order,
        ranked_order: ranked,
        selected_order: selected,
        unknown_order: unknown,
        blocked_order: blocked,
        missing_contract_order: missing,
        omission_order: omission,
        uncertainty_order: uncertainty,
        negative_evidence_order: negative,
        adversarial_event_order: input.adversarial_events.clone(),
        replay_identity: input.replay_identity.clone(),
        manifest_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: input.raw_data_local,
        aggregate_only: input.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

fn validate_input(i: &BiolangContractInput) -> Result<(), ContractFrontierError> {
    if i.request_id.trim().is_empty()
        || i.federation_id.trim().is_empty()
        || i.operator_id.trim().is_empty()
        || i.requested_surface.trim().is_empty()
        || i.semantic_profile.trim().is_empty()
        || i.required_contract_order.is_empty()
        || i.descriptors.is_empty()
        || !canonical(&i.required_contract_order)
        || !canonical(&i.adversarial_events)
        || !digest(&i.replay_identity)
        || i.boundary != PRECLINICAL_BOUNDARY
        || !i.raw_data_local
        || !i.aggregate_only
    {
        return Err(invalid(
            "BioLang input identity, contract closure, digest, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for d in &i.descriptors {
        if d.contract_id.trim().is_empty()
            || !ids.insert(d.contract_id.clone())
            || d.version.trim().is_empty()
            || d.input_schema.trim().is_empty()
            || d.output_schema.trim().is_empty()
            || d.surface.trim().is_empty()
            || d.semantic_profile.trim().is_empty()
            || !digest(&d.capability_digest)
            || !digest(&d.provenance_digest)
            || !digest(&d.compatibility_digest)
            || !digest(&d.replay_identity)
            || !canonical(&d.omissions)
            || !canonical(&d.uncertainty)
        {
            return Err(invalid(format!(
                "descriptor {} is malformed or duplicated",
                d.contract_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn d(id: &str, state: EvidenceState) -> ContractDescriptor {
        ContractDescriptor {
            contract_id: id.into(),
            version: "1.0.0".into(),
            input_schema: "BioWorld".into(),
            output_schema: "TypedWorld".into(),
            surface: "workbench".into(),
            semantic_profile: "preclinical-neural".into(),
            capability_digest: h(&format!("cap:{id}")),
            provenance_digest: h(&format!("prov:{id}")),
            compatibility_digest: h(&format!("compat:{id}")),
            replay_identity: h("replay"),
            evidence_state: state,
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_data: true,
            permitted: true,
        }
    }
    fn i(ds: Vec<ContractDescriptor>) -> BiolangContractInput {
        BiolangContractInput {
            request_id: "request:biolang".into(),
            federation_id: "fed:commons".into(),
            operator_id: "operator:scientist".into(),
            requested_surface: "workbench".into(),
            semantic_profile: "preclinical-neural".into(),
            required_contract_order: vec!["contract:a".into()],
            descriptors: ds,
            replay_identity: h("replay"),
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
        let m = build_contract_frontier_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1)
    }
    #[test]
    fn qualified_manifest() {
        let r = validate_contract_frontier(&i(vec![d("contract:a", EvidenceState::Supported)]))
            .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.effect_receipts.len(), 1)
    }
    #[test]
    fn unknown_is_retained() {
        let r =
            validate_contract_frontier(&i(vec![d("contract:a", EvidenceState::Unknown)])).unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.uncertainty_order.is_empty())
    }
    #[test]
    fn contradiction_blocks() {
        let r = validate_contract_frontier(&i(vec![d("contract:a", EvidenceState::Contradicted)]))
            .unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert!(!r.blocked_order.is_empty())
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = i(vec![d("contract:a", EvidenceState::Supported)]);
        q.policy_allow = false;
        let r = validate_contract_frontier(&q).unwrap();
        assert_eq!(r.disposition, "blocked")
    }
    #[test]
    fn duplicate_rejected() {
        let q = i(vec![
            d("contract:a", EvidenceState::Supported),
            d("contract:a", EvidenceState::Supported),
        ]);
        assert!(validate_contract_frontier(&q).is_err())
    }
    #[test]
    fn deterministic() {
        let a = validate_contract_frontier(&i(vec![
            d("contract:b", EvidenceState::Supported),
            d("contract:a", EvidenceState::Supported),
        ]))
        .unwrap();
        let b = validate_contract_frontier(&i(vec![
            d("contract:a", EvidenceState::Supported),
            d("contract:b", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(a.ranked_order, b.ranked_order);
        assert_eq!(a.manifest_digest, b.manifest_digest)
    }
}
