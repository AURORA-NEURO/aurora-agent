//! Federated continual publication and research-object release copilot for `AFA-mutation-P16-F12`.
//!
//! The copilot prepares a bounded, content-addressed release decision from caller-supplied
//! mutation-run attestations. It preserves omissions and negative evidence, keeps raw
//! preclinical bytes local, and never makes a clinical decision.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mutation-P16-F12";
pub const CONTRACT_VERSION: &str = "mutation-federated-continual-publication-release-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ValidatedResearchRun4@1";
pub const OUTPUT_SCHEMA: &str = "SignedResearchObject3@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.mutation-signed-research-object-3+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun4 {
    pub run_id: String,
    pub site_id: String,
    pub study_id: String,
    pub mutation_family: String,
    pub semantic_profile: String,
    pub publication_purpose: String,
    pub artifact_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signer_id: String,
    pub signature_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signature_verified: bool,
    pub policy_permitted: bool,
    pub bounded_tool_approved: bool,
    pub federation_permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationReleaseRequest6 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub publication_purpose: String,
    pub semantic_profile: String,
    pub required_site_order: Vec<String>,
    pub required_run_order: Vec<String>,
    pub minimum_site_count: u32,
    pub minimum_signed_run_count: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
    pub runs: Vec<ValidatedResearchRun4>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationReleaseDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationPublicationReleaseReceipt9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub publication_purpose: String,
    pub semantic_profile: String,
    pub disposition: PublicationReleaseDisposition,
    pub run_order: Vec<String>,
    pub selected_run_order: Vec<String>,
    pub unresolved_run_order: Vec<String>,
    pub blocked_run_order: Vec<String>,
    pub missing_run_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub selected_artifact_order: Vec<String>,
    pub unresolved_artifact_order: Vec<String>,
    pub blocked_artifact_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub blocked_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub signer_order: Vec<String>,
    pub selected_signer_order: Vec<String>,
    pub missing_signer_order: Vec<String>,
    pub revoked_signer_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub selected_evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub signed_run_count: u32,
    pub release_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PublicationReleaseError {
    #[error("invalid mutation publication-release request: {0}")]
    Invalid(String),
    #[error("mutation publication-release artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> PublicationReleaseError {
    PublicationReleaseError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl MutationPublicationReleaseReceipt9 {
    pub fn validate(&self) -> Result<(), PublicationReleaseError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.publication_purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.run_order.is_empty()
            || self.artifact_order.is_empty()
            || self.site_order.is_empty()
            || self.signer_order.is_empty()
            || self.evidence_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("release identity, runs, artifacts, sites, signers, evidence, locality, or effects are incomplete"));
        }
        for values in [
            &self.run_order,
            &self.selected_run_order,
            &self.unresolved_run_order,
            &self.blocked_run_order,
            &self.missing_run_order,
            &self.artifact_order,
            &self.selected_artifact_order,
            &self.unresolved_artifact_order,
            &self.blocked_artifact_order,
            &self.site_order,
            &self.selected_site_order,
            &self.unresolved_site_order,
            &self.blocked_site_order,
            &self.missing_site_order,
            &self.signer_order,
            &self.selected_signer_order,
            &self.missing_signer_order,
            &self.revoked_signer_order,
            &self.evidence_order,
            &self.selected_evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("publication-release ordering is not canonical"));
            }
        }
        let runs = self.run_order.iter().cloned().collect::<BTreeSet<_>>();
        let run_parts = self
            .selected_run_order
            .iter()
            .chain(&self.unresolved_run_order)
            .chain(&self.blocked_run_order)
            .chain(&self.missing_run_order)
            .cloned()
            .collect::<Vec<_>>();
        if runs.len() != self.run_order.len()
            || run_parts.len() != runs.len()
            || run_parts.iter().cloned().collect::<BTreeSet<_>>() != runs
        {
            return Err(invalid("run states do not form a complete partition"));
        }
        let artifacts = self.artifact_order.iter().cloned().collect::<BTreeSet<_>>();
        let artifact_parts = self
            .selected_artifact_order
            .iter()
            .chain(&self.unresolved_artifact_order)
            .chain(&self.blocked_artifact_order)
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
        let evidence = self.evidence_order.iter().cloned().collect::<BTreeSet<_>>();
        if evidence.len() != self.evidence_order.len()
            || !self
                .selected_evidence_order
                .iter()
                .all(|item| evidence.contains(item))
        {
            return Err(invalid("evidence order is invalid"));
        }
        if !digest(&self.release_digest)
            || !digest(&self.artifact.content_hash)
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.release_digest
        {
            return Err(PublicationReleaseError::Artifact(
                "release artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:release-copilot:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside the publication-release gate"));
        }
        if self.disposition == PublicationReleaseDisposition::Qualified
            && self.effect_receipts != [format!("invoke:release-copilot:{}", self.request_id)]
        {
            return Err(invalid("qualified publication effect is invalid"));
        }
        if self.disposition != PublicationReleaseDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid(
                "non-qualified publication workflow must block release",
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, PublicationReleaseError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| PublicationReleaseError::Artifact(e.to_string()))?,
        )
        .map_err(|e| PublicationReleaseError::Artifact(e.to_string()))
    }
}

pub fn mutation_publication_release_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id:FEATURE_ID.into(), version:CONTRACT_VERSION.into(), owner_crate:"mutation".into(), consumers:["bioinformatician".into(),"research program lead".into(),"publication steward".into()].into(), behavior:"compiles federated continual validated mutation runs into a signed research-object release decision".into(), value:"provides a reproducible release copilot that keeps raw preclinical data local while exposing signer, provenance, replay, omission, and negative-result gates".into(), inputs:vec![TypedPort{name:"validated_research_runs".into(),schema:INPUT_SCHEMA.into(),required:true}], outputs:vec![TypedPort{name:"signed_research_object".into(),schema:OUTPUT_SCHEMA.into(),required:true}], effects:[Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(), permissions:["read:local-research-artifacts".into(),"invoke:bounded-release-copilot".into()].into(), determinism:Determinism::ByteStable, evidence:vec![EvidenceReference{source_id:"ro-crate-1.3".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())}], authority_requirements:vec![AuthorityRequirement{role:"research program lead".into(),reason:"federated research-object publication invokes a bounded release tool and requires explicit approval".into()}], autonomy_tier:AutonomyTier::A2, surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Protocol,ResearchSurface::Operator].into(), boundary:PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(request: &PublicationReleaseRequest6) -> Result<(), PublicationReleaseError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.publication_purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_site_order.is_empty()
        || !canonical(&request.required_site_order)
        || request.required_run_order.is_empty()
        || !canonical(&request.required_run_order)
        || request.minimum_site_count == 0
        || request.minimum_signed_run_count == 0
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.runs.is_empty()
    {
        return Err(invalid("publication request identity, closure, floors, replay, locality, boundary, or runs are invalid"));
    }
    let mut ids = BTreeSet::new();
    for run in &request.runs {
        if run.run_id.trim().is_empty()
            || run.site_id.trim().is_empty()
            || run.study_id.trim().is_empty()
            || run.mutation_family.trim().is_empty()
            || run.semantic_profile.trim().is_empty()
            || run.publication_purpose.trim().is_empty()
            || run.artifact_order.is_empty()
            || !canonical(&run.artifact_order)
            || !canonical(&run.evidence_order)
            || run.evidence_order.is_empty()
            || run.signer_id.trim().is_empty()
            || !digest(&run.provenance_digest)
            || !digest(&run.replay_identity)
            || !digest(&run.signature_digest)
            || !canonical(&run.omission_order)
            || !canonical(&run.uncertainty_order)
            || !ids.insert(run.run_id.clone())
        {
            return Err(invalid("validated run identity, artifact/evidence closure, signer, digest, or ordering is invalid"));
        }
    }
    Ok(())
}

pub fn compile_mutation_publication_release(
    request: &PublicationReleaseRequest6,
) -> Result<MutationPublicationReleaseReceipt9, PublicationReleaseError> {
    validate_request(request)?;
    let mut runs = request.runs.clone();
    runs.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    let run_order = request
        .required_run_order
        .iter()
        .cloned()
        .chain(runs.iter().map(|r| r.run_id.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
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
    let mut evidence = BTreeSet::new();
    let mut selected_evidence = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut selected_artifacts = BTreeSet::new();
    let mut unresolved_artifacts = BTreeSet::new();
    let mut blocked_artifacts = BTreeSet::new();
    let mut by_site: BTreeMap<String, Vec<&ValidatedResearchRun4>> = BTreeMap::new();
    for run in &runs {
        sites.insert(run.site_id.clone());
        signers.insert(run.signer_id.clone());
        evidence.extend(run.evidence_order.iter().cloned());
        artifacts.extend(run.artifact_order.iter().cloned());
        by_site.entry(run.site_id.clone()).or_default().push(run);
        omissions.extend(
            run.omission_order
                .iter()
                .map(|x| format!("{}:{}", run.run_id, x)),
        );
        uncertainty.extend(
            run.uncertainty_order
                .iter()
                .map(|x| format!("{}:{}", run.run_id, x)),
        );
        if run.negative_result {
            negative.insert(format!("{}:negative-result", run.run_id));
        }
        let mut state = "selected";
        if run.revoked {
            state = "blocked";
            revoked_signers.insert(run.signer_id.clone());
            negative.insert(format!("{}:signer-revoked", run.run_id));
        } else if !run.signature_verified
            || !run.policy_permitted
            || !run.bounded_tool_approved
            || !run.federation_permitted
            || !run.raw_data_local
            || !run.aggregate_only
        {
            state = "blocked";
            missing_signers.insert(run.signer_id.clone());
            omissions.insert(format!("{}:signature-or-authority", run.run_id));
        } else if run.stale
            || run.semantic_profile != request.semantic_profile
            || run.publication_purpose != request.publication_purpose
            || run.replay_identity != request.replay_identity
            || !matches!(
                run.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            state = "unresolved";
            missing_signers.insert(run.signer_id.clone());
            if run.stale {
                uncertainty.insert(format!("{}:stale", run.run_id));
            }
            if run.semantic_profile != request.semantic_profile {
                uncertainty.insert(format!("{}:semantic-profile-mismatch", run.run_id));
            }
            if run.publication_purpose != request.publication_purpose {
                uncertainty.insert(format!("{}:purpose-mismatch", run.run_id));
            }
            if run.replay_identity != request.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", run.run_id));
            }
            if run.evidence_state == EvidenceState::Unknown {
                uncertainty.insert(format!("{}:unknown-evidence", run.run_id));
            }
            if run.evidence_state == EvidenceState::Speculative {
                uncertainty.insert(format!("{}:speculative-evidence", run.run_id));
            }
            if run.evidence_state == EvidenceState::Contradicted {
                state = "blocked";
                unresolved.remove(&run.run_id);
                negative.insert(format!("{}:contradicted", run.run_id));
            }
        }
        match state {
            "selected" => {
                selected.insert(run.run_id.clone());
                selected_signers.insert(run.signer_id.clone());
                selected_evidence.extend(run.evidence_order.iter().cloned());
                selected_artifacts.extend(run.artifact_order.iter().cloned());
            }
            "unresolved" => {
                unresolved.insert(run.run_id.clone());
                unresolved_artifacts.extend(run.artifact_order.iter().cloned());
            }
            _ => {
                blocked.insert(run.run_id.clone());
                blocked_artifacts.extend(run.artifact_order.iter().cloned());
            }
        }
    }
    let required_sites = BTreeSet::from_iter(request.required_site_order.iter().cloned());
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
            let ids = rows.iter().map(|r| r.run_id.as_str()).collect::<Vec<_>>();
            if ids.iter().any(|id| blocked.contains(*id)) {
                blocked_sites.insert(site.clone());
            } else if ids.iter().any(|id| unresolved.contains(*id)) {
                unresolved_sites.insert(site.clone());
            } else {
                selected_sites.insert(site.clone());
            }
        }
    }
    let observed = runs
        .iter()
        .map(|r| r.run_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_runs = request
        .required_run_order
        .iter()
        .filter(|id| !observed.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    omissions.extend(missing_runs.iter().map(|id| format!("run:{}:missing", id)));
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
            .map(|e| format!("adversarial:{}", e)),
    );
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allow
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global {
        blocked.extend(run_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        selected_sites.clear();
        unresolved_sites.clear();
        blocked_sites.extend(sites.iter().cloned());
        omissions.insert("request:publication-release-gate-blocked".into());
    }
    let missing_artifacts = artifacts
        .iter()
        .filter(|id| {
            !selected_artifacts.contains(*id)
                && !unresolved_artifacts.contains(*id)
                && !blocked_artifacts.contains(*id)
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let _ = missing_artifacts;
    let disposition = if global || !blocked.is_empty() || !blocked_sites.is_empty() {
        PublicationReleaseDisposition::Blocked
    } else if selected.len() < request.minimum_signed_run_count as usize
        || selected_sites.len() < request.minimum_site_count as usize
        || !missing_runs.is_empty()
        || !unresolved.is_empty()
        || !unresolved_sites.is_empty()
    {
        PublicationReleaseDisposition::Unresolved
    } else {
        PublicationReleaseDisposition::Qualified
    };
    if disposition != PublicationReleaseDisposition::Qualified {
        omissions.insert("request:publication-release-not-ready".into());
    }
    let effects = if disposition == PublicationReleaseDisposition::Qualified {
        vec![format!("invoke:release-copilot:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let artifact_order = artifacts.iter().cloned().collect::<Vec<_>>();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"researcher":request.researcher,"publication_purpose":request.publication_purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"run_order":run_order,"selected_run_order":selected,"unresolved_run_order":unresolved,"blocked_run_order":blocked,"missing_run_order":missing_runs,"artifact_order":artifact_order,"selected_artifact_order":selected_artifacts,"unresolved_artifact_order":unresolved_artifacts,"blocked_artifact_order":blocked_artifacts,"site_order":sites,"selected_site_order":selected_sites,"unresolved_site_order":unresolved_sites,"blocked_site_order":blocked_sites,"missing_site_order":missing_sites,"signer_order":signers,"selected_signer_order":selected_signers,"missing_signer_order":missing_signers,"revoked_signer_order":revoked_signers,"evidence_order":evidence,"selected_evidence_order":selected_evidence,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"signed_run_count":selected.len() as u32,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let release_digest = ContentHash::of_value(&payload)
        .map_err(|e| PublicationReleaseError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("mutation-signed-research-object-3:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| PublicationReleaseError::Artifact(e.to_string()))?;
    let receipt = MutationPublicationReleaseReceipt9 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        researcher: request.researcher.clone(),
        publication_purpose: request.publication_purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        run_order: payload["run_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_run_order: payload["selected_run_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_run_order: payload["unresolved_run_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_run_order: payload["blocked_run_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_run_order: payload["missing_run_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
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
        evidence_order: payload["evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_evidence_order: payload["selected_evidence_order"]
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
        signed_run_count: selected.len() as u32,
        release_digest: release_digest.clone(),
        artifact,
        effect_receipts: effects,
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
    fn hash(s: &str) -> ContentHash {
        ContentHash::of_bytes(s.as_bytes())
    }
    fn run(id: &str, site: &str, signer: &str, replay: &ContentHash) -> ValidatedResearchRun4 {
        ValidatedResearchRun4 {
            run_id: id.into(),
            site_id: site.into(),
            study_id: "study".into(),
            mutation_family: "family".into(),
            semantic_profile: "mutation-v1".into(),
            publication_purpose: "release".into(),
            artifact_order: vec![format!("artifact-{id}")],
            evidence_order: vec![format!("evidence-{id}")],
            provenance_digest: hash(&format!("prov-{id}")),
            replay_identity: replay.clone(),
            signer_id: signer.into(),
            signature_digest: hash(&format!("sig-{id}")),
            evidence_state: EvidenceState::Supported,
            signature_verified: true,
            policy_permitted: true,
            bounded_tool_approved: true,
            federation_permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            stale: false,
            revoked: false,
            negative_result: false,
            omission_order: vec![],
            uncertainty_order: vec![],
        }
    }
    fn request() -> PublicationReleaseRequest6 {
        let replay = hash("replay");
        PublicationReleaseRequest6 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "release-1".into(),
            researcher: "bioinformatician".into(),
            publication_purpose: "release".into(),
            semantic_profile: "mutation-v1".into(),
            required_site_order: vec!["site-a".into(), "site-b".into()],
            required_run_order: vec!["run-a".into(), "run-b".into()],
            minimum_site_count: 2,
            minimum_signed_run_count: 2,
            replay_identity: replay.clone(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
            runs: vec![
                run("run-b", "site-b", "signer-b", &replay),
                run("run-a", "site-a", "signer-a", &replay),
            ],
        }
    }
    #[test]
    fn manifest_is_typed() {
        mutation_publication_release_manifest().validate().unwrap();
    }
    #[test]
    fn complete_release_qualifies() {
        let r = compile_mutation_publication_release(&request()).unwrap();
        assert_eq!(r.disposition, PublicationReleaseDisposition::Qualified);
        r.validate().unwrap();
    }
    #[test]
    fn stale_run_is_unresolved() {
        let mut q = request();
        q.runs[0].stale = true;
        assert_eq!(
            compile_mutation_publication_release(&q)
                .unwrap()
                .disposition,
            PublicationReleaseDisposition::Unresolved
        );
    }
    #[test]
    fn revoked_run_blocks() {
        let mut q = request();
        q.runs[0].revoked = true;
        assert_eq!(
            compile_mutation_publication_release(&q)
                .unwrap()
                .disposition,
            PublicationReleaseDisposition::Blocked
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            compile_mutation_publication_release(&q)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        );
    }
    #[test]
    fn missing_run_is_explicit() {
        let mut q = request();
        q.runs.remove(0);
        let r = compile_mutation_publication_release(&q).unwrap();
        assert_eq!(r.disposition, PublicationReleaseDisposition::Unresolved);
        assert_eq!(r.missing_run_order, vec!["run-b"]);
    }
    #[test]
    fn duplicate_run_rejected() {
        let mut q = request();
        q.runs[1].run_id = q.runs[0].run_id.clone();
        assert!(compile_mutation_publication_release(&q).is_err())
    }
    #[test]
    fn ordering_is_reproducible() {
        let a = compile_mutation_publication_release(&request()).unwrap();
        let mut q = request();
        q.runs.reverse();
        let b = compile_mutation_publication_release(&q).unwrap();
        assert_eq!(a, b)
    }
}
