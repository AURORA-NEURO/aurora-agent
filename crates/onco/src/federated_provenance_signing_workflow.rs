//! Federated continual provenance and signing workflow for `AFA-onco-P18-F16`.
//!
//! The workflow materializes signed, digest-bound provenance attestations for preclinical
//! OncoWorld artifacts. It evaluates declarations and never diagnoses, treats, triages, enrolls,
//! or moves raw experimental data.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-onco-P18-F16";
pub const CONTRACT_VERSION: &str =
    "onco-federated-continual-provenance-signing-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "OncoProvenanceObject6@1";
pub const OUTPUT_SCHEMA: &str = "SignedProvenanceWorkflow9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.onco-signed-provenance-workflow-9+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OncoProvenanceObject6 {
    pub object_id: String,
    pub site_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub purpose: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signer_id: String,
    pub signature_digest: ContentHash,
    pub lineage_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub signature_verified: bool,
    pub permitted: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSigningRequest6 {
    pub schema_version: String,
    pub request_id: String,
    pub research_program: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_site_order: Vec<String>,
    pub required_artifact_order: Vec<String>,
    pub minimum_signer_count: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
    pub objects: Vec<OncoProvenanceObject6>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignedProvenanceDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceWorkflow9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub research_program: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: SignedProvenanceDisposition,
    pub artifact_order: Vec<String>,
    pub selected_artifact_order: Vec<String>,
    pub unresolved_artifact_order: Vec<String>,
    pub blocked_artifact_order: Vec<String>,
    pub missing_artifact_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub blocked_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub signer_order: Vec<String>,
    pub selected_signer_order: Vec<String>,
    pub missing_signer_order: Vec<String>,
    pub revoked_signer_order: Vec<String>,
    pub provenance_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub signature_coverage_milli: i64,
    pub replay_identity: ContentHash,
    pub workflow_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProvenanceSigningError {
    #[error("invalid federated provenance/signing request: {0}")]
    Invalid(String),
    #[error("federated provenance/signing artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ProvenanceSigningError {
    ProvenanceSigningError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl SignedProvenanceWorkflow9 {
    pub fn validate(&self) -> Result<(), ProvenanceSigningError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.research_program.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.artifact_order.is_empty()
            || self.site_order.is_empty()
            || self.signer_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "provenance identity, artifacts, sites, signers, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.artifact_order,
            &self.selected_artifact_order,
            &self.unresolved_artifact_order,
            &self.blocked_artifact_order,
            &self.missing_artifact_order,
            &self.site_order,
            &self.selected_site_order,
            &self.unresolved_site_order,
            &self.blocked_site_order,
            &self.missing_site_order,
            &self.signer_order,
            &self.selected_signer_order,
            &self.missing_signer_order,
            &self.revoked_signer_order,
            &self.provenance_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("provenance workflow ordering is not canonical"));
            }
        }
        let artifacts = self.artifact_order.iter().cloned().collect::<BTreeSet<_>>();
        let artifact_parts = self
            .selected_artifact_order
            .iter()
            .chain(&self.unresolved_artifact_order)
            .chain(&self.blocked_artifact_order)
            .chain(&self.missing_artifact_order)
            .cloned()
            .collect::<Vec<_>>();
        if artifacts.len() != self.artifact_order.len()
            || artifact_parts.len() != artifacts.len()
            || artifact_parts.iter().cloned().collect::<BTreeSet<_>>() != artifacts
        {
            return Err(invalid("artifact states do not form a complete partition"));
        }
        let sites = self.site_order.iter().cloned().collect::<BTreeSet<_>>();
        let site_parts = self
            .selected_site_order
            .iter()
            .chain(&self.unresolved_site_order)
            .chain(&self.blocked_site_order)
            .chain(&self.missing_site_order)
            .cloned()
            .collect::<Vec<_>>();
        if sites.len() != self.site_order.len()
            || site_parts.len() != sites.len()
            || site_parts.iter().cloned().collect::<BTreeSet<_>>() != sites
        {
            return Err(invalid("site states do not form a complete partition"));
        }
        let signers = self.signer_order.iter().cloned().collect::<BTreeSet<_>>();
        let signer_parts = self
            .selected_signer_order
            .iter()
            .chain(&self.missing_signer_order)
            .chain(&self.revoked_signer_order)
            .cloned()
            .collect::<Vec<_>>();
        if signers.len() != self.signer_order.len()
            || signer_parts.len() != signers.len()
            || signer_parts.iter().cloned().collect::<BTreeSet<_>>() != signers
        {
            return Err(invalid("signer states do not form a complete partition"));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.workflow_digest)
            || !digest(&self.artifact.content_hash)
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.workflow_digest
            || !(0..=1000).contains(&self.signature_coverage_milli)
        {
            return Err(ProvenanceSigningError::Artifact(
                "provenance artifact metadata, coverage, or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:signed-provenance:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside the provenance signing gate"));
        }
        if self.disposition == SignedProvenanceDisposition::Qualified
            && self.effect_receipts != [format!("exchange:signed-provenance:{}", self.request_id)]
        {
            return Err(invalid("qualified provenance effect is invalid"));
        }
        if self.disposition != SignedProvenanceDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid(
                "non-qualified provenance workflow must block release",
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ProvenanceSigningError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))?,
        )
        .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))
    }
}

pub fn federated_provenance_signing_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "onco".into(),
        consumers: BTreeSet::from_iter(
            [
                "research program lead".into(),
                "provenance steward".into(),
                "preclinical data engineer".into(),
            ]
            .into_iter(),
        ),
        behavior: "compiles federated continual signed provenance declarations into an omission-aware workflow receipt".into(),
        value: "lets research programs exchange verifiable aggregate provenance without moving raw OncoWorld artifacts or hiding signer and lineage failures".into(),
        inputs: vec![TypedPort {
            name: "onco_provenance_objects".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "signed_provenance_workflow".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: BTreeSet::from_iter(
            [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into_iter(),
        ),
        permissions: BTreeSet::from_iter(
            [
                "read:local-research-artifacts".into(),
                "exchange:permitted-provenance".into(),
            ]
            .into_iter(),
        ),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "w3c-prov-o".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.w3.org/TR/prov-o/".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from_iter(
            [
                ResearchSurface::Ui,
                ResearchSurface::Api,
                ResearchSurface::Sdk,
                ResearchSurface::Protocol,
                ResearchSurface::Operator,
            ]
            .into_iter(),
        ),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &ProvenanceSigningRequest6) -> Result<(), ProvenanceSigningError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.research_program.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_site_order.is_empty()
        || !canonical(&request.required_site_order)
        || request.required_artifact_order.is_empty()
        || !canonical(&request.required_artifact_order)
        || request.minimum_signer_count == 0
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.objects.is_empty()
    {
        return Err(invalid(
            "provenance request identity, closure, signer floor, replay, locality, boundary, or objects are invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for object in &request.objects {
        if object.object_id.trim().is_empty()
            || object.site_id.trim().is_empty()
            || object.study_id.trim().is_empty()
            || object.semantic_profile.trim().is_empty()
            || object.purpose.trim().is_empty()
            || object.signer_id.trim().is_empty()
            || object.lineage_order.is_empty()
            || !canonical(&object.lineage_order)
            || !digest(&object.artifact_digest)
            || !digest(&object.provenance_digest)
            || !digest(&object.replay_identity)
            || !digest(&object.signature_digest)
            || !canonical(&object.omission_order)
            || !canonical(&object.uncertainty_order)
            || !ids.insert(object.object_id.clone())
        {
            return Err(invalid(
                "provenance object identity, lineage, signer, digests, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

pub fn compile_federated_provenance_signing(
    request: &ProvenanceSigningRequest6,
) -> Result<SignedProvenanceWorkflow9, ProvenanceSigningError> {
    validate_request(request)?;
    let mut objects = request.objects.clone();
    objects.sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let artifact_order = request
        .required_artifact_order
        .iter()
        .cloned()
        .chain(objects.iter().map(|object| object.object_id.clone()))
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut sites = BTreeSet::from_iter(request.required_site_order.iter().cloned());
    let mut signers = BTreeSet::new();
    let mut selected_signers = BTreeSet::new();
    let mut missing_signers = BTreeSet::new();
    let mut revoked_signers = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut by_site: BTreeMap<String, Vec<&OncoProvenanceObject6>> = BTreeMap::new();
    for object in &objects {
        sites.insert(object.site_id.clone());
        signers.insert(object.signer_id.clone());
        by_site
            .entry(object.site_id.clone())
            .or_default()
            .push(object);
        provenance.extend(
            object
                .lineage_order
                .iter()
                .map(|item| format!("{}:{}", object.object_id, item)),
        );
        omissions.extend(
            object
                .omission_order
                .iter()
                .map(|item| format!("{}:{}", object.object_id, item)),
        );
        uncertainty.extend(
            object
                .uncertainty_order
                .iter()
                .map(|item| format!("{}:{}", object.object_id, item)),
        );
        if object.negative_result {
            negative.insert(format!("{}:negative-result", object.object_id));
        }
        if object.revoked {
            blocked.insert(object.object_id.clone());
            revoked_signers.insert(object.signer_id.clone());
            negative.insert(format!("{}:signer-revoked", object.object_id));
        } else if !object.permitted
            || !object.local_only
            || !object.aggregate_only
            || !object.signature_verified
        {
            blocked.insert(object.object_id.clone());
            missing_signers.insert(object.signer_id.clone());
            omissions.insert(format!("{}:signature-or-locality", object.object_id));
        } else if object.stale
            || object.semantic_profile != request.semantic_profile
            || object.purpose != request.purpose
            || object.replay_identity != request.replay_identity
            || !matches!(
                object.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unresolved.insert(object.object_id.clone());
            if object.stale {
                uncertainty.insert(format!("{}:stale", object.object_id));
            }
            if object.semantic_profile != request.semantic_profile {
                uncertainty.insert(format!("{}:semantic-profile-mismatch", object.object_id));
            }
            if object.purpose != request.purpose {
                uncertainty.insert(format!("{}:purpose-mismatch", object.object_id));
            }
            if object.replay_identity != request.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", object.object_id));
            }
            if object.evidence_state == EvidenceState::Unknown {
                uncertainty.insert(format!("{}:unknown-evidence", object.object_id));
            }
            if object.evidence_state == EvidenceState::Speculative {
                uncertainty.insert(format!("{}:speculative-evidence", object.object_id));
            }
            if object.evidence_state == EvidenceState::Contradicted {
                unresolved.remove(&object.object_id);
                blocked.insert(object.object_id.clone());
                negative.insert(format!("{}:contradicted", object.object_id));
            }
            missing_signers.insert(object.signer_id.clone());
        } else {
            selected.insert(object.object_id.clone());
            selected_signers.insert(object.signer_id.clone());
        }
    }
    let required_sites = request
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected_sites = BTreeSet::new();
    let mut unresolved_sites = BTreeSet::new();
    let mut blocked_sites = BTreeSet::new();
    let mut missing_sites = BTreeSet::new();
    for site in &sites {
        let rows = by_site.get(site).cloned().unwrap_or_default();
        if rows.is_empty() {
            if required_sites.contains(site) {
                missing_sites.insert(site.clone());
                omissions.insert(format!("site:{}:missing", site));
            }
        } else {
            let ids = rows
                .iter()
                .map(|row| row.object_id.as_str())
                .collect::<Vec<_>>();
            if ids.iter().any(|id| blocked.contains(*id)) {
                blocked_sites.insert(site.clone());
            } else if ids.iter().any(|id| unresolved.contains(*id)) {
                unresolved_sites.insert(site.clone());
            } else {
                selected_sites.insert(site.clone());
            }
        }
    }
    let observed_artifacts = objects
        .iter()
        .map(|object| object.object_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_artifacts = request
        .required_artifact_order
        .iter()
        .filter(|id| !observed_artifacts.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    omissions.extend(
        missing_artifacts
            .iter()
            .map(|id| format!("artifact:{}:missing-or-unqualified", id)),
    );
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{}", event)),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allow
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(objects.iter().map(|object| object.object_id.clone()));
        selected.clear();
        unresolved.clear();
        selected_sites.clear();
        unresolved_sites.clear();
        blocked_sites.extend(sites.iter().cloned());
        omissions.insert("request:provenance-release-gate-blocked".into());
    }
    let artifact_order = artifact_order.into_iter().collect::<Vec<_>>();
    let missing_artifact_order = missing_artifacts.into_iter().collect::<Vec<_>>();
    let qualified_artifact_count = selected.len();
    let signature_coverage_milli = if objects.is_empty() {
        0
    } else {
        (selected.len() as i64 * 1000) / objects.len() as i64
    };
    let disposition = if global_block || !blocked.is_empty() || !blocked_sites.is_empty() {
        SignedProvenanceDisposition::Blocked
    } else if selected.len() < request.required_artifact_order.len()
        || !missing_artifact_order.is_empty()
        || selected_signers.len() < request.minimum_signer_count as usize
        || !missing_sites.is_empty()
        || !unresolved.is_empty()
        || !unresolved_sites.is_empty()
    {
        SignedProvenanceDisposition::Unresolved
    } else {
        SignedProvenanceDisposition::Qualified
    };
    if disposition != SignedProvenanceDisposition::Qualified {
        omissions.insert("request:provenance-workflow-not-release-ready".into());
    }
    let effects = if disposition == SignedProvenanceDisposition::Qualified {
        vec![format!("exchange:signed-provenance:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "research_program": request.research_program,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "artifact_order": artifact_order,
        "selected_artifact_order": selected,
        "unresolved_artifact_order": unresolved,
        "blocked_artifact_order": blocked,
        "missing_artifact_order": missing_artifact_order,
        "site_order": sites,
        "selected_site_order": selected_sites,
        "unresolved_site_order": unresolved_sites,
        "blocked_site_order": blocked_sites,
        "missing_site_order": missing_sites,
        "signer_order": signers,
        "selected_signer_order": selected_signers,
        "missing_signer_order": missing_signers,
        "revoked_signer_order": revoked_signers,
        "provenance_order": provenance,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "signature_coverage_milli": signature_coverage_milli,
        "replay_identity": request.replay_identity,
        "effect_receipts": effects,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let workflow_digest = ContentHash::of_value(&payload)
        .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("onco-signed-provenance-workflow-9:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ProvenanceSigningError::Artifact(error.to_string()))?;
    let receipt = SignedProvenanceWorkflow9 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        research_program: request.research_program.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        artifact_order: payload["artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_artifact_order: payload["selected_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_artifact_order: payload["unresolved_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_artifact_order: payload["blocked_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_artifact_order: payload["missing_artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        site_order: payload["site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_site_order: payload["selected_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_site_order: payload["unresolved_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_site_order: payload["blocked_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_site_order: payload["missing_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        signer_order: payload["signer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_signer_order: payload["selected_signer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_signer_order: payload["missing_signer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        revoked_signer_order: payload["revoked_signer_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        provenance_order: payload["provenance_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        signature_coverage_milli,
        replay_identity: request.replay_identity.clone(),
        workflow_digest: workflow_digest.clone(),
        artifact,
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if qualified_artifact_count > 0 && receipt.artifact.content_hash != workflow_digest {
        return Err(ProvenanceSigningError::Artifact(
            "workflow artifact digest mismatch".into(),
        ));
    }
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn object(id: &str, site: &str, signer: &str, replay: &ContentHash) -> OncoProvenanceObject6 {
        OncoProvenanceObject6 {
            object_id: id.into(),
            site_id: site.into(),
            study_id: "study-a".into(),
            semantic_profile: "onco-preclinical-v1".into(),
            purpose: "worldline-replication".into(),
            artifact_digest: hash(&format!("artifact-{id}")),
            provenance_digest: hash(&format!("provenance-{id}")),
            replay_identity: replay.clone(),
            signer_id: signer.into(),
            signature_digest: hash(&format!("signature-{id}")),
            lineage_order: vec!["specimen".into(), "worldline".into()],
            evidence_state: EvidenceState::Supported,
            signature_verified: true,
            permitted: true,
            local_only: true,
            aggregate_only: true,
            stale: false,
            revoked: false,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }

    fn request() -> ProvenanceSigningRequest6 {
        let replay = hash("replay");
        ProvenanceSigningRequest6 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "p1".into(),
            research_program: "preclinical-worldline".into(),
            purpose: "worldline-replication".into(),
            semantic_profile: "onco-preclinical-v1".into(),
            required_site_order: vec!["site-a".into(), "site-b".into()],
            required_artifact_order: vec!["o1".into(), "o2".into()],
            minimum_signer_count: 2,
            replay_identity: replay.clone(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            objects: vec![
                object("o2", "site-b", "signer-b", &replay),
                object("o1", "site-a", "signer-a", &replay),
            ],
        }
    }

    #[test]
    fn manifest_is_typed() {
        let manifest = federated_provenance_signing_manifest();
        assert_eq!(manifest.capability_id, FEATURE_ID);
        manifest.validate().unwrap();
    }

    #[test]
    fn complete_workflow_qualifies() {
        let receipt = compile_federated_provenance_signing(&request()).unwrap();
        assert_eq!(receipt.disposition, SignedProvenanceDisposition::Qualified);
        assert_eq!(receipt.selected_signer_order, vec!["signer-a", "signer-b"]);
        receipt.validate().unwrap();
    }

    #[test]
    fn stale_object_is_unresolved() {
        let mut req = request();
        req.objects[0].stale = true;
        let receipt = compile_federated_provenance_signing(&req).unwrap();
        assert_eq!(receipt.disposition, SignedProvenanceDisposition::Unresolved);
        assert!(receipt
            .uncertainty_order
            .iter()
            .any(|item| item.ends_with(":stale")));
    }

    #[test]
    fn revoked_signer_blocks() {
        let mut req = request();
        req.objects[0].revoked = true;
        let receipt = compile_federated_provenance_signing(&req).unwrap();
        assert_eq!(receipt.disposition, SignedProvenanceDisposition::Blocked);
        assert_eq!(receipt.revoked_signer_order, vec!["signer-b"]);
    }

    #[test]
    fn policy_denial_blocks() {
        let mut req = request();
        req.policy_allow = false;
        let receipt = compile_federated_provenance_signing(&req).unwrap();
        assert_eq!(receipt.disposition, SignedProvenanceDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn missing_artifact_is_explicit() {
        let mut req = request();
        req.required_artifact_order.push("o3".into());
        req.required_artifact_order.sort();
        let receipt = compile_federated_provenance_signing(&req).unwrap();
        assert_eq!(receipt.disposition, SignedProvenanceDisposition::Unresolved);
        assert_eq!(receipt.missing_artifact_order, vec!["o3"]);
    }

    #[test]
    fn duplicate_object_is_rejected() {
        let mut req = request();
        req.objects[1].object_id = req.objects[0].object_id.clone();
        assert!(matches!(
            compile_federated_provenance_signing(&req),
            Err(ProvenanceSigningError::Invalid(_))
        ));
    }

    #[test]
    fn canonical_order_is_reproducible() {
        let first = compile_federated_provenance_signing(&request()).unwrap();
        let mut reversed = request();
        reversed.objects.reverse();
        let second = compile_federated_provenance_signing(&reversed).unwrap();
        assert_eq!(first, second);
    }
}
