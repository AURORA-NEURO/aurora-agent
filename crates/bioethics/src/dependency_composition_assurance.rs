//! Multimodal bioethics dependency-composition assurance.
//!
//! Atlas feature: `AFA-bioethics-P27-F26`.  The harness verifies that a preclinical
//! multimodal research composition has an explicit, comparable and policy-permitted
//! dependency closure before a downstream workflow may consume its summaries.  It
//! does not infer ethics, classify biological content, move raw data, or execute a
//! physical action: callers submit typed attestations and receive an honest,
//! deterministic receipt.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P27-F26";
pub const CONTRACT_VERSION: &str =
    "bioethics-multimodal-dependency-composition-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "BioethicsCompositionRequest2@1";
pub const OUTPUT_SCHEMA: &str = "BioethicsCompositionReceipt7@1";
const CONTENT_TYPE: &str = "application/vnd.aurora.bioethics-composition-receipt-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionDependency {
    pub dependency_id: String,
    pub owning_crate: String,
    pub version: String,
    pub role: String,
    pub study_order: Vec<String>,
    pub semantic_profile: String,
    pub capability_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omissions: Vec<String>,
    pub conflicts: Vec<String>,
    pub local_only: bool,
    pub permitted: bool,
    pub signed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsCompositionRequest {
    pub schema_version: String,
    pub composition_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub semantic_profile: String,
    pub required_dependency_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub dependencies: Vec<CompositionDependency>,
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
pub enum CompositionDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BioethicsCompositionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub composition_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub semantic_profile: String,
    pub disposition: CompositionDisposition,
    pub dependency_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_dependency_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub composition_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BioethicsCompositionError {
    #[error("invalid bioethics composition: {0}")]
    Invalid(String),
    #[error("bioethics composition artifact failed: {0}")]
    Artifact(String),
}

fn invalid(value: impl Into<String>) -> BioethicsCompositionError {
    BioethicsCompositionError::Invalid(value.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl BioethicsCompositionReceipt {
    pub fn validate(&self) -> Result<(), BioethicsCompositionError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.composition_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.dependency_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "composition identity, locality, dependencies, or effects are incomplete",
            ));
        }
        for values in [
            &self.dependency_order,
            &self.admitted_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_dependency_order,
            &self.contradiction_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("composition ordering is not canonical"));
            }
        }
        let ids = self
            .dependency_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .admitted_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("composition states do not partition dependencies"));
        }
        for value in [
            &self.replay_identity,
            &self.composition_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("composition digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| BioethicsCompositionError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("bioethics composition artifact type is invalid"));
        }
        if self.disposition == CompositionDisposition::Qualified
            && self.effect_receipts
                != [
                    format!("exchange:permitted-summaries:{}", self.composition_id),
                    format!("manage:local-capability:{}", self.composition_id),
                ]
        {
            return Err(invalid("qualified composition effects are invalid"));
        }
        if self.disposition != CompositionDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified composition must block release"));
        }
        Ok(())
    }
}

pub fn bioethics_dependency_composition_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "bioethics".into(),
        consumers: ["consortium operator".into(), "bioethicist".into(), "downstream research workflow".into()]
            .into(),
        behavior: "verifies multimodal preclinical dependency closure, comparability, evidence, locality, and policy gates without inferring ethics or executing research".into(),
        value: "turns missing, contradictory, or unauthorized cross-crate dependencies into deterministic release evidence instead of silent composition".into(),
        inputs: vec![TypedPort { name: "bioethics_composition_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "bioethics_composition_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "SLSA provenance 1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "consortium operator".into(), reason: "composition release can export only policy-permitted summaries".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn evaluate_bioethics_composition(
    request: &BioethicsCompositionRequest,
) -> Result<BioethicsCompositionReceipt, BioethicsCompositionError> {
    validate_request(request)?;
    let mut rows = request.dependencies.clone();
    rows.sort_by(|a, b| a.dependency_id.cmp(&b.dependency_id));
    let dependency_order = rows
        .iter()
        .map(|row| row.dependency_id.clone())
        .collect::<Vec<_>>();
    let required = request
        .required_dependency_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let known = rows
        .iter()
        .map(|row| row.dependency_id.clone())
        .collect::<BTreeSet<_>>();
    let mut admitted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = required
        .difference(&known)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut contradiction = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let by_id = rows
        .iter()
        .map(|row| (row.dependency_id.as_str(), row))
        .collect::<HashMap<_, _>>();
    for id in &dependency_order {
        let row = by_id[id.as_str()];
        omissions.extend(row.omissions.iter().map(|item| format!("{id}:{item}")));
        uncertainty.extend(row.conflicts.iter().map(|item| format!("{id}:{item}")));
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(id.clone());
            blocked.insert(id.clone());
        } else if !row.local_only || !row.permitted || !row.signed {
            blocked.insert(id.clone());
            if !row.local_only {
                negative.insert(format!("{id}:raw-data-not-local"));
            }
            if !row.permitted {
                negative.insert(format!("{id}:policy-denied"));
            }
            if !row.signed {
                uncertainty.insert(format!("{id}:unsigned-capability"));
            }
        } else if row.semantic_profile != request.semantic_profile
            || !row.omissions.is_empty()
            || !row.conflicts.is_empty()
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(id.clone());
        } else {
            admitted.insert(id.clone());
        }
    }
    for id in &missing {
        omissions.insert(format!("{id}:required-dependency-missing"));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(dependency_order.iter().cloned());
        admitted.clear();
        unresolved.clear();
        missing.clear();
        omissions.insert("request:composition-release-gate-blocked".into());
    }
    let disposition = if global_block {
        CompositionDisposition::Blocked
    } else if admitted.is_empty() || !missing.is_empty() || !required.is_subset(&admitted) {
        CompositionDisposition::Unresolved
    } else {
        CompositionDisposition::Qualified
    };
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_dependency_order = missing.into_iter().collect::<Vec<_>>();
    let contradiction_order = contradiction.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == CompositionDisposition::Qualified {
        vec![
            format!("exchange:permitted-summaries:{}", request.composition_id),
            format!("manage:local-capability:{}", request.composition_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "composition_id": request.composition_id,
        "federation_id": request.federation_id,
        "institution_id": request.institution_id,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "dependency_order": dependency_order,
        "admitted_order": admitted_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_dependency_order": missing_dependency_order,
        "contradiction_order": contradiction_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "negative_evidence_order": negative_evidence_order,
        "replay_identity": request.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local,
        "aggregate_only": request.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let composition_digest = ContentHash::of_value(&payload)
        .map_err(|error| BioethicsCompositionError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("bioethics-composition:{}", request.composition_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        vec![ProvenanceLink {
            source_id: format!("federation:{}", request.federation_id),
            relation: "derived-from-dependency-attestations".into(),
            digest: request.replay_identity.clone(),
        }],
    )
    .map_err(|error| BioethicsCompositionError::Artifact(error.to_string()))?;
    let receipt = BioethicsCompositionReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        composition_id: request.composition_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        dependency_order: payload["dependency_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        admitted_order: payload["admitted_order"]
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
        missing_dependency_order: payload["missing_dependency_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        contradiction_order: payload["contradiction_order"]
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
        replay_identity: request.replay_identity.clone(),
        composition_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &BioethicsCompositionRequest,
) -> Result<(), BioethicsCompositionError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.composition_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_dependency_order.is_empty()
        || !canonical(&request.required_dependency_order)
        || request.dependencies.is_empty()
        || !digest(&request.comparability_digest)
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "composition identity, required closure, digests, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for row in &request.dependencies {
        if row.dependency_id.trim().is_empty()
            || !ids.insert(row.dependency_id.clone())
            || row.owning_crate.trim().is_empty()
            || row.version.trim().is_empty()
            || row.role.trim().is_empty()
            || row.study_order.is_empty()
            || !canonical(&row.study_order)
            || row.semantic_profile.trim().is_empty()
            || !digest(&row.capability_digest)
            || !digest(&row.provenance_digest)
            || !canonical(&row.omissions)
            || !canonical(&row.conflicts)
        {
            return Err(invalid(format!(
                "dependency {} is malformed or duplicated",
                row.dependency_id
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

    fn request() -> BioethicsCompositionRequest {
        let digest = hash("dependency");
        let dependency = |id: &str| CompositionDependency {
            dependency_id: id.into(),
            owning_crate: "crate-a".into(),
            version: "1.0.0".into(),
            role: "typed-evidence".into(),
            study_order: vec!["study:imaging".into(), "study:omics".into()],
            semantic_profile: "preclinical-neural".into(),
            capability_digest: digest.clone(),
            provenance_digest: digest.clone(),
            evidence_state: EvidenceState::Supported,
            omissions: vec![],
            conflicts: vec![],
            local_only: true,
            permitted: true,
            signed: true,
        };
        BioethicsCompositionRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            composition_id: "composition:one".into(),
            federation_id: "fed:commons".into(),
            institution_id: "inst:alpha".into(),
            semantic_profile: "preclinical-neural".into(),
            required_dependency_order: vec!["dep:imaging".into(), "dep:omics".into()],
            comparability_digest: digest.clone(),
            replay_identity: digest.clone(),
            dependencies: vec![dependency("dep:imaging"), dependency("dep:omics")],
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
            bioethics_dependency_composition_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_composition() {
        assert_eq!(
            evaluate_bioethics_composition(&request())
                .unwrap()
                .disposition,
            CompositionDisposition::Qualified
        );
    }
    #[test]
    fn deterministic_replay() {
        let a = evaluate_bioethics_composition(&request()).unwrap();
        let b = evaluate_bioethics_composition(&request()).unwrap();
        assert_eq!(a.composition_digest, b.composition_digest);
    }
    #[test]
    fn missing_dependency_is_unresolved() {
        let mut value = request();
        value.required_dependency_order = vec![
            "dep:imaging".into(),
            "dep:missing".into(),
            "dep:omics".into(),
        ];
        assert_eq!(
            evaluate_bioethics_composition(&value).unwrap().disposition,
            CompositionDisposition::Unresolved
        );
    }
    #[test]
    fn contradiction_is_blocked_dependency() {
        let mut value = request();
        value.dependencies[0].evidence_state = EvidenceState::Contradicted;
        let out = evaluate_bioethics_composition(&value).unwrap();
        assert!(out.contradiction_order.contains(&"dep:imaging".into()));
        assert_eq!(out.disposition, CompositionDisposition::Unresolved);
    }
    #[test]
    fn policy_blocks_everything() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            evaluate_bioethics_composition(&value).unwrap().disposition,
            CompositionDisposition::Blocked
        );
    }
    #[test]
    fn adversarial_event_blocks_everything() {
        let mut value = request();
        value.adversarial_events.push("prompt-injection".into());
        assert_eq!(
            evaluate_bioethics_composition(&value).unwrap().disposition,
            CompositionDisposition::Blocked
        );
    }
}
