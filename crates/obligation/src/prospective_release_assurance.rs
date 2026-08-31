//! Prospective high-throughput publication and research-object release assurance.
//!
//! Atlas feature: `AFA-obligation-P16-F27`.
//!
//! This is the production-facing companion to the older single-object release harness.  It
//! admits a bounded set of preclinical runs, computes a deterministic complete partition of
//! selected/unresolved/blocked/missing state, and emits a portable research-object envelope.
//! Unknown, stale, contradictory, negative, and omitted evidence remain explicit in the receipt;
//! no convenience default can turn an incomplete closure into a release pass.

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

pub const FEATURE_ID: &str = "AFA-obligation-P16-F27";
pub const CONTRACT_VERSION: &str = "obligation-prospective-release-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ValidatedResearchRun3@1";
pub const OUTPUT_SCHEMA: &str = "SignedResearchObject7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.signed-research-object-7+json";
pub const MAX_RUNS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun3 {
    pub run_id: String,
    pub release_id: String,
    pub site_id: String,
    pub semantic_profile: String,
    pub artifact_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub signer_id: String,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub signature_verified: bool,
    pub provenance_complete: bool,
    pub policy_permitted: bool,
    pub protected_closure: bool,
    pub benchmark_passed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveReleaseAssuranceRequest {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub publication_purpose: String,
    pub semantic_profile: String,
    pub required_run_order: Vec<String>,
    pub required_site_order: Vec<String>,
    pub required_artifact_order: Vec<String>,
    pub required_evidence_order: Vec<String>,
    pub minimum_run_count: u32,
    pub minimum_site_count: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub runs: Vec<ValidatedResearchRun3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveReleaseAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub publication_purpose: String,
    pub semantic_profile: String,
    pub disposition: AssuranceDisposition,
    pub run_order: Vec<String>,
    pub selected_run_order: Vec<String>,
    pub unresolved_run_order: Vec<String>,
    pub blocked_run_order: Vec<String>,
    pub missing_run_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub blocked_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub selected_artifact_order: Vec<String>,
    pub unresolved_artifact_order: Vec<String>,
    pub blocked_artifact_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub selected_evidence_order: Vec<String>,
    pub signer_order: Vec<String>,
    pub selected_signer_order: Vec<String>,
    pub missing_signer_order: Vec<String>,
    pub revoked_signer_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub reasons: Vec<String>,
    pub release_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProspectiveReleaseAssuranceError {
    #[error("invalid prospective release assurance request or receipt: {0}")]
    Invalid(String),
    #[error("prospective release assurance artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ProspectiveReleaseAssuranceError {
    ProspectiveReleaseAssuranceError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn insert_all(target: &mut BTreeSet<String>, values: &[String]) {
    target.extend(values.iter().cloned());
}

pub fn prospective_release_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "obligation".into(),
        consumers: ["context compiler engineer".into(), "publication steward".into(), "federation verifier".into()].into(),
        behavior: "verifies prospective high-throughput preclinical release bundles with deterministic admission control, complete state partitions, omission certificates, and portable research-object receipts".into(),
        value: "prevents incomplete provenance, stale evidence, contradictory claims, or unauthorized data movement from appearing as a successful publication release".into(),
        inputs: vec![TypedPort { name: "validated_research_runs".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "signed_research_object".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) },
        ],
        authority_requirements: vec![AuthorityRequirement { role: "context compiler engineer".into(), reason: "release admission is bounded to typed capability-run evaluation and never grants publication authority".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl ProspectiveReleaseAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ProspectiveReleaseAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.publication_purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.run_order.is_empty()
            || self.site_order.is_empty()
            || self.artifact_order.is_empty()
            || self.evidence_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "identity, closure, artifact, evidence, locality, or effect fields are incomplete",
            ));
        }
        for values in [
            &self.run_order,
            &self.selected_run_order,
            &self.unresolved_run_order,
            &self.blocked_run_order,
            &self.missing_run_order,
            &self.site_order,
            &self.selected_site_order,
            &self.unresolved_site_order,
            &self.blocked_site_order,
            &self.missing_site_order,
            &self.artifact_order,
            &self.selected_artifact_order,
            &self.unresolved_artifact_order,
            &self.blocked_artifact_order,
            &self.evidence_order,
            &self.selected_evidence_order,
            &self.signer_order,
            &self.selected_signer_order,
            &self.missing_signer_order,
            &self.revoked_signer_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !canonical(values) {
                return Err(invalid("receipt ordering is not canonical"));
            }
        }
        partition(
            &self.run_order,
            [
                &self.selected_run_order,
                &self.unresolved_run_order,
                &self.blocked_run_order,
                &self.missing_run_order,
            ],
            "run",
        )?;
        partition(
            &self.site_order,
            [
                &self.selected_site_order,
                &self.unresolved_site_order,
                &self.blocked_site_order,
                &self.missing_site_order,
            ],
            "site",
        )?;
        partition(
            &self.artifact_order,
            [
                &self.selected_artifact_order,
                &self.unresolved_artifact_order,
                &self.blocked_artifact_order,
            ],
            "artifact",
        )?;
        subset(
            &self.selected_evidence_order,
            &self.evidence_order,
            "evidence",
        )?;
        partition(
            &self.signer_order,
            [
                &self.selected_signer_order,
                &self.missing_signer_order,
                &self.revoked_signer_order,
            ],
            "signer",
        )?;
        if !digest_valid(&self.release_digest) || self.artifact.content_hash != self.release_digest
        {
            return Err(invalid(
                "release digest does not match the content-addressed artifact",
            ));
        }
        if self.disposition == AssuranceDisposition::Qualified
            && self.effect_receipts != [format!("invoke:prospective-release:{}", self.request_id)]
        {
            return Err(invalid(
                "qualified release effect is not the request-scoped invocation",
            ));
        }
        if self.disposition != AssuranceDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release".to_string()]
        {
            return Err(invalid("unresolved or blocked release must fail closed"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| ProspectiveReleaseAssuranceError::Artifact(e.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ProspectiveReleaseAssuranceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| ProspectiveReleaseAssuranceError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ProspectiveReleaseAssuranceError::Artifact(e.to_string()))
    }
}

fn subset(
    values: &[String],
    universe: &[String],
    label: &str,
) -> Result<(), ProspectiveReleaseAssuranceError> {
    let all = universe.iter().cloned().collect::<BTreeSet<_>>();
    if values.iter().any(|item| !all.contains(item))
        || values.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err(invalid(format!(
            "{label} state contains an item outside its universe or duplicates"
        )));
    }
    Ok(())
}

fn partition<const N: usize>(
    universe: &[String],
    parts: [&[String]; N],
    label: &str,
) -> Result<(), ProspectiveReleaseAssuranceError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    let mut flattened = Vec::new();
    for part in parts {
        subset(part, universe, label)?;
        flattened.extend_from_slice(part);
    }
    if flattened.len() != expected.len()
        || flattened.iter().cloned().collect::<BTreeSet<_>>() != expected
    {
        return Err(invalid(format!(
            "{label} states do not form a complete partition"
        )));
    }
    Ok(())
}

fn validate_request(
    request: &ProspectiveReleaseAssuranceRequest,
) -> Result<(), ProspectiveReleaseAssuranceError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.publication_purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_run_order.is_empty()
        || request.required_site_order.is_empty()
        || request.required_artifact_order.is_empty()
        || request.required_evidence_order.is_empty()
        || !canonical(&request.required_run_order)
        || !canonical(&request.required_site_order)
        || !canonical(&request.required_artifact_order)
        || !canonical(&request.required_evidence_order)
        || !canonical(&request.adversarial_event_order)
        || request.minimum_run_count == 0
        || request.minimum_site_count == 0
        || !digest_valid(&request.replay_identity)
        || !request.policy_allow
        || !request.federation_allow
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.runs.is_empty()
        || request.runs.len() > MAX_RUNS
    {
        return Err(invalid(
            "request identity, closure, policy, locality, boundary, or run bounds are invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for run in &request.runs {
        if run.run_id.trim().is_empty()
            || run.release_id.trim().is_empty()
            || run.site_id.trim().is_empty()
            || run.semantic_profile != request.semantic_profile
            || run.artifact_order.is_empty()
            || run.evidence_order.is_empty()
            || !canonical(&run.artifact_order)
            || !canonical(&run.evidence_order)
            || !canonical(&run.omission_order)
            || !canonical(&run.uncertainty_order)
            || run.signer_id.trim().is_empty()
            || !digest_valid(&run.provenance_digest)
            || !digest_valid(&run.replay_identity)
            || !ids.insert(run.run_id.clone())
        {
            return Err(invalid(
                "run identity, profile, closure, signer, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

fn add_run_state(
    map: &mut BTreeMap<String, AssuranceDisposition>,
    id: String,
    state: AssuranceDisposition,
) {
    map.insert(id, state);
}

pub fn assure_prospective_release(
    request: &ProspectiveReleaseAssuranceRequest,
) -> Result<ProspectiveReleaseAssuranceReceipt, ProspectiveReleaseAssuranceError> {
    validate_request(request)?;
    let mut runs_by_id = BTreeMap::new();
    for run in &request.runs {
        runs_by_id.insert(run.run_id.clone(), run);
    }
    let required_runs = request
        .required_run_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut run_ids = required_runs.clone();
    run_ids.extend(runs_by_id.keys().cloned());
    let run_order: Vec<String> = run_ids.iter().cloned().collect();
    let mut states = BTreeMap::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainties = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradictions = BTreeSet::new();
    let mut signers = BTreeSet::new();
    let mut revoked_signers = BTreeSet::new();
    for id in &run_order {
        let Some(run) = runs_by_id.get(id) else {
            add_run_state(&mut states, id.clone(), AssuranceDisposition::Blocked);
            omissions.insert(format!("missing required run: {id}"));
            continue;
        };
        signers.insert(run.signer_id.clone());
        if run.revoked {
            revoked_signers.insert(run.signer_id.clone());
        }
        insert_all(&mut omissions, &run.omission_order);
        insert_all(&mut uncertainties, &run.uncertainty_order);
        if run.negative_result {
            negative.insert(run.run_id.clone());
        }
        if run.evidence_state == EvidenceState::Contradicted {
            contradictions.insert(run.run_id.clone());
        }
        let blocked = run.revoked
            || !run.signature_verified
            || !run.provenance_complete
            || !run.policy_permitted
            || !run.protected_closure
            || !run.raw_data_local
            || !run.aggregate_only;
        let unresolved = run.stale
            || !run.benchmark_passed
            || run.replay_identity != request.replay_identity
            || !run.omission_order.is_empty()
            || !run.uncertainty_order.is_empty()
            || matches!(
                run.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative | EvidenceState::Contradicted
            );
        add_run_state(
            &mut states,
            id.clone(),
            if blocked {
                AssuranceDisposition::Blocked
            } else if unresolved {
                AssuranceDisposition::Unresolved
            } else {
                AssuranceDisposition::Qualified
            },
        );
    }
    let selected_run_order = run_order
        .iter()
        .filter(|id| states.get(*id) == Some(&AssuranceDisposition::Qualified))
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_run_order = run_order
        .iter()
        .filter(|id| states.get(*id) == Some(&AssuranceDisposition::Unresolved))
        .cloned()
        .collect::<Vec<_>>();
    let blocked_run_order = run_order
        .iter()
        .filter(|id| {
            states.get(*id) == Some(&AssuranceDisposition::Blocked) && runs_by_id.contains_key(*id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_run_order = run_order
        .iter()
        .filter(|id| !runs_by_id.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    let mut site_ids = request
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    site_ids.extend(request.runs.iter().map(|run| run.site_id.clone()));
    let site_order: Vec<String> = site_ids.into_iter().collect();
    let site_state = |site: &String| -> AssuranceDisposition {
        let site_runs = request
            .runs
            .iter()
            .filter(|run| &run.site_id == site)
            .map(|run| states[&run.run_id])
            .collect::<Vec<_>>();
        if site_runs.is_empty() {
            AssuranceDisposition::Blocked
        } else if site_runs
            .iter()
            .any(|s| *s == AssuranceDisposition::Qualified)
        {
            AssuranceDisposition::Qualified
        } else if site_runs
            .iter()
            .any(|s| *s == AssuranceDisposition::Unresolved)
        {
            AssuranceDisposition::Unresolved
        } else {
            AssuranceDisposition::Blocked
        }
    };
    let selected_site_order = site_order
        .iter()
        .filter(|site| site_state(site) == AssuranceDisposition::Qualified)
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_site_order = site_order
        .iter()
        .filter(|site| site_state(site) == AssuranceDisposition::Unresolved)
        .cloned()
        .collect::<Vec<_>>();
    let blocked_site_order = site_order
        .iter()
        .filter(|site| {
            site_state(site) == AssuranceDisposition::Blocked
                && !request.required_site_order.contains(site)
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_site_order = site_order
        .iter()
        .filter(|site| {
            site_state(site) == AssuranceDisposition::Blocked
                && request.required_site_order.contains(site)
                && !request.runs.iter().any(|run| &run.site_id == *site)
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut artifact_ids = request
        .required_artifact_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut evidence_ids = request
        .required_evidence_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for run in &request.runs {
        insert_all(&mut artifact_ids, &run.artifact_order);
        insert_all(&mut evidence_ids, &run.evidence_order);
    }
    let artifact_order: Vec<String> = artifact_ids.into_iter().collect();
    let evidence_order: Vec<String> = evidence_ids.into_iter().collect();
    let artifact_state = |artifact: &String| -> AssuranceDisposition {
        let states_for_artifact = request
            .runs
            .iter()
            .filter(|run| run.artifact_order.contains(artifact))
            .map(|run| states[&run.run_id])
            .collect::<Vec<_>>();
        if states_for_artifact.is_empty()
            || states_for_artifact
                .iter()
                .all(|s| *s == AssuranceDisposition::Blocked)
        {
            AssuranceDisposition::Blocked
        } else if states_for_artifact
            .iter()
            .any(|s| *s == AssuranceDisposition::Qualified)
        {
            AssuranceDisposition::Qualified
        } else {
            AssuranceDisposition::Unresolved
        }
    };
    let selected_artifact_order = artifact_order
        .iter()
        .filter(|id| artifact_state(id) == AssuranceDisposition::Qualified)
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_artifact_order = artifact_order
        .iter()
        .filter(|id| artifact_state(id) == AssuranceDisposition::Unresolved)
        .cloned()
        .collect::<Vec<_>>();
    let blocked_artifact_order = artifact_order
        .iter()
        .filter(|id| artifact_state(id) == AssuranceDisposition::Blocked)
        .cloned()
        .collect::<Vec<_>>();
    let selected_evidence_order = evidence_order
        .iter()
        .filter(|id| {
            request.runs.iter().any(|run| {
                run.evidence_order.contains(id)
                    && states[&run.run_id] == AssuranceDisposition::Qualified
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let selected_signer_order = signers
        .iter()
        .filter(|id| {
            request.runs.iter().any(|run| {
                &run.signer_id == *id && states[&run.run_id] == AssuranceDisposition::Qualified
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    let missing_signer_order = signers
        .iter()
        .filter(|id| !selected_signer_order.contains(id) && !revoked_signers.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let revoked_signer_order = revoked_signers.iter().cloned().collect::<Vec<_>>();
    let signer_order: Vec<String> = signers
        .into_iter()
        .chain(revoked_signers.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let disposition = if !request.policy_allow
        || !request.federation_allow
        || selected_run_order.len() < request.minimum_run_count as usize
        || selected_site_order.len() < request.minimum_site_count as usize
        || !missing_run_order.is_empty()
        || !missing_site_order.is_empty()
        || !blocked_run_order.is_empty()
        || !blocked_artifact_order.is_empty()
    {
        AssuranceDisposition::Blocked
    } else if !unresolved_run_order.is_empty()
        || !unresolved_site_order.is_empty()
        || !unresolved_artifact_order.is_empty()
        || selected_evidence_order.len() < evidence_order.len()
    {
        AssuranceDisposition::Unresolved
    } else {
        AssuranceDisposition::Qualified
    };
    let reasons = vec![match disposition { AssuranceDisposition::Qualified => "all prospective release assurance gates passed".into(), AssuranceDisposition::Unresolved => "unmeasured, stale, contradictory, omitted, or insufficient evidence prevents a pass".into(), AssuranceDisposition::Blocked => "policy, closure, provenance, required coverage, or authorization gates blocked admission".into() }];
    let effect_receipts = if disposition == AssuranceDisposition::Qualified {
        vec![format!("invoke:prospective-release:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "feature_id": FEATURE_ID, "contract_version": CONTRACT_VERSION, "request_id": request.request_id, "consumer": request.consumer, "publication_purpose": request.publication_purpose, "semantic_profile": request.semantic_profile, "disposition": disposition, "run_order": run_order, "selected_run_order": selected_run_order, "unresolved_run_order": unresolved_run_order, "blocked_run_order": blocked_run_order, "missing_run_order": missing_run_order, "site_order": site_order, "selected_site_order": selected_site_order, "unresolved_site_order": unresolved_site_order, "blocked_site_order": blocked_site_order, "missing_site_order": missing_site_order, "artifact_order": artifact_order, "selected_artifact_order": selected_artifact_order, "unresolved_artifact_order": unresolved_artifact_order, "blocked_artifact_order": blocked_artifact_order, "evidence_order": evidence_order, "selected_evidence_order": selected_evidence_order, "signer_order": signer_order, "selected_signer_order": selected_signer_order, "missing_signer_order": missing_signer_order, "revoked_signer_order": revoked_signer_order, "omission_order": omissions, "uncertainty_order": uncertainties, "negative_evidence_order": negative, "contradiction_order": contradictions, "adversarial_event_order": request.adversarial_event_order, "reasons": reasons, "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("signed-research-object:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| ProspectiveReleaseAssuranceError::Artifact(e.to_string()))?;
    let receipt = ProspectiveReleaseAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        publication_purpose: request.publication_purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        run_order: serde_json::from_value(payload["run_order"].clone()).unwrap(),
        selected_run_order: serde_json::from_value(payload["selected_run_order"].clone()).unwrap(),
        unresolved_run_order: serde_json::from_value(payload["unresolved_run_order"].clone())
            .unwrap(),
        blocked_run_order: serde_json::from_value(payload["blocked_run_order"].clone()).unwrap(),
        missing_run_order: serde_json::from_value(payload["missing_run_order"].clone()).unwrap(),
        site_order: serde_json::from_value(payload["site_order"].clone()).unwrap(),
        selected_site_order: serde_json::from_value(payload["selected_site_order"].clone())
            .unwrap(),
        unresolved_site_order: serde_json::from_value(payload["unresolved_site_order"].clone())
            .unwrap(),
        blocked_site_order: serde_json::from_value(payload["blocked_site_order"].clone()).unwrap(),
        missing_site_order: serde_json::from_value(payload["missing_site_order"].clone()).unwrap(),
        artifact_order: serde_json::from_value(payload["artifact_order"].clone()).unwrap(),
        selected_artifact_order: serde_json::from_value(payload["selected_artifact_order"].clone())
            .unwrap(),
        unresolved_artifact_order: serde_json::from_value(
            payload["unresolved_artifact_order"].clone(),
        )
        .unwrap(),
        blocked_artifact_order: serde_json::from_value(payload["blocked_artifact_order"].clone())
            .unwrap(),
        evidence_order: serde_json::from_value(payload["evidence_order"].clone()).unwrap(),
        selected_evidence_order: serde_json::from_value(payload["selected_evidence_order"].clone())
            .unwrap(),
        signer_order: serde_json::from_value(payload["signer_order"].clone()).unwrap(),
        selected_signer_order: serde_json::from_value(payload["selected_signer_order"].clone())
            .unwrap(),
        missing_signer_order: serde_json::from_value(payload["missing_signer_order"].clone())
            .unwrap(),
        revoked_signer_order: serde_json::from_value(payload["revoked_signer_order"].clone())
            .unwrap(),
        omission_order: omissions.into_iter().collect(),
        uncertainty_order: uncertainties.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradictions.into_iter().collect(),
        adversarial_event_order: request.adversarial_event_order.clone(),
        reasons,
        release_digest: artifact.content_hash.clone(),
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }
    fn run(id: &str, state: EvidenceState) -> ValidatedResearchRun3 {
        ValidatedResearchRun3 {
            run_id: id.into(),
            release_id: "release:1".into(),
            site_id: "site:a".into(),
            semantic_profile: "organoid".into(),
            artifact_order: vec![format!("artifact:{id}")],
            evidence_order: vec![format!("evidence:{id}")],
            signer_id: "signer:a".into(),
            provenance_digest: hash(id),
            replay_identity: hash("replay"),
            evidence_state: state,
            signature_verified: true,
            provenance_complete: true,
            policy_permitted: true,
            protected_closure: true,
            benchmark_passed: true,
            raw_data_local: true,
            aggregate_only: true,
            stale: false,
            revoked: false,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }
    fn request(runs: Vec<ValidatedResearchRun3>) -> ProspectiveReleaseAssuranceRequest {
        ProspectiveReleaseAssuranceRequest {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "request:1".into(),
            consumer: "context compiler".into(),
            publication_purpose: "release".into(),
            semantic_profile: "organoid".into(),
            required_run_order: vec!["run:1".into()],
            required_site_order: vec!["site:a".into()],
            required_artifact_order: vec!["artifact:run:1".into()],
            required_evidence_order: vec!["evidence:run:1".into()],
            minimum_run_count: 1,
            minimum_site_count: 1,
            replay_identity: hash("replay"),
            policy_allow: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            runs,
        }
    }

    #[test]
    fn qualified_receipt_is_byte_stable() {
        let receipt =
            assure_prospective_release(&request(vec![run("run:1", EvidenceState::Supported)]))
                .unwrap();
        assert_eq!(receipt.disposition, AssuranceDisposition::Qualified);
        assert_eq!(
            receipt.effect_receipts,
            vec!["invoke:prospective-release:request:1"]
        );
        assert_eq!(
            receipt.digest().unwrap(),
            assure_prospective_release(&request(vec![run("run:1", EvidenceState::Supported)]))
                .unwrap()
                .digest()
                .unwrap()
        );
    }
    #[test]
    fn unknown_evidence_is_unresolved() {
        let receipt =
            assure_prospective_release(&request(vec![run("run:1", EvidenceState::Unknown)]))
                .unwrap();
        assert_eq!(receipt.disposition, AssuranceDisposition::Unresolved);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn contradictory_evidence_is_preserved() {
        let receipt =
            assure_prospective_release(&request(vec![run("run:1", EvidenceState::Contradicted)]))
                .unwrap();
        assert_eq!(receipt.contradiction_order, vec!["run:1"]);
    }
    #[test]
    fn revoked_run_is_blocked() {
        let mut item = run("run:1", EvidenceState::Supported);
        item.revoked = true;
        let receipt = assure_prospective_release(&request(vec![item])).unwrap();
        assert_eq!(receipt.disposition, AssuranceDisposition::Blocked);
        assert_eq!(receipt.revoked_signer_order, vec!["signer:a"]);
    }
    #[test]
    fn missing_required_run_is_blocked() {
        let receipt = assure_prospective_release(&request(Vec::new())).unwrap_err();
        assert!(receipt.to_string().contains("runs"));
    }
    #[test]
    fn manifest_is_valid() {
        prospective_release_assurance_manifest().validate().unwrap();
    }
}
