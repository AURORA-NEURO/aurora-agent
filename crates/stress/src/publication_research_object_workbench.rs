//! Federated continual publication and research-object release workbench (`AFA-stress-P16-F20`).
//!
//! This A1 workbench compiles caller-supplied, digest-only release attestations into a portable
//! research-object envelope. It never signs, uploads, dereferences, or transports raw data;
//! every omission, negative result, and release gate remains visible to a researcher.

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

pub const FEATURE_ID: &str = "AFA-stress-P16-F20";
pub const CONTRACT_VERSION: &str =
    "stress-federated-continual-publication-research-object-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ValidatedResearchRun4@1";
pub const OUTPUT_SCHEMA: &str = "SignedResearchObject5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.signed-research-object-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun4 {
    pub run_id: String,
    pub study_id: String,
    pub release_intent: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub protected_closure: bool,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub standards: Vec<String>,
    pub reproducibility_score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationWorkbenchRequest5 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_standards: Vec<String>,
    pub replay_identity: ContentHash,
    pub runs: Vec<ValidatedResearchRun4>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub network_permitted: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedResearchObject5 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub run_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub conditional_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub required_standards: Vec<String>,
    pub covered_standards: Vec<String>,
    pub signature_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub research_object_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PublicationWorkbenchError {
    #[error("invalid publication workbench request: {0}")]
    Invalid(String),
    #[error("publication research object artifact failed: {0}")]
    Artifact(String),
    #[error("publication workbench output failed: {0}")]
    Output(String),
}

fn text(value: &str) -> bool {
    !value.trim().is_empty()
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn publication_research_object_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "stress".into(),
        consumers: ["benchmark curator".into(), "research-object publisher".into(), "release reviewer".into()].into(),
        behavior: "compile digest-only validated research runs into an omission-aware portable research-object release envelope without signing or publishing".into(),
        value: "gives federated research teams a deterministic release view that preserves replay, provenance, negative results, and the distinction between unknown and unmeasured evidence".into(),
        inputs: vec![TypedPort { name: "validated_research_run".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "signed_research_object".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(),
        permissions: ["view:authorized-research-state".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) },
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(
    request: &PublicationWorkbenchRequest5,
) -> Result<(), PublicationWorkbenchError> {
    if request.schema_version != INPUT_SCHEMA
        || !text(&request.request_id)
        || !text(&request.consumer)
        || !text(&request.federation_id)
        || !text(&request.purpose)
        || !text(&request.semantic_profile)
        || request.required_standards.is_empty()
        || !ordered(&request.required_standards)
        || request.runs.is_empty()
        || !digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(PublicationWorkbenchError::Invalid(
            "request identity, standards, runs, replay, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for run in &request.runs {
        if !text(&run.run_id)
            || !ids.insert(run.run_id.clone())
            || !text(&run.study_id)
            || !text(&run.release_intent)
            || !text(&run.semantic_profile)
            || !digest(&run.artifact_digest)
            || !digest(&run.evidence_digest)
            || !digest(&run.provenance_digest)
            || run.replay_identity != request.replay_identity
            || !ordered(&run.omission_order)
            || !ordered(&run.standards)
            || run.reproducibility_score > 100
        {
            return Err(PublicationWorkbenchError::Invalid(
                "run identity, ordering, score, digest, or replay is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl SignedResearchObject5 {
    pub fn validate(&self) -> Result<(), PublicationWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || self.run_order.is_empty()
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "conditional" | "blocked" | "unknown"
            )
        {
            return Err(PublicationWorkbenchError::Output(
                "research-object identity, locality, disposition, runs, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.run_order,
            &self.qualified_order,
            &self.conditional_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.required_standards,
            &self.covered_standards,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(PublicationWorkbenchError::Output(
                    "research-object ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.run_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .qualified_order
            .iter()
            .chain(&self.conditional_order)
            .chain(&self.blocked_order)
            .chain(&self.unknown_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids.len() != self.run_order.len() || states != ids {
            return Err(PublicationWorkbenchError::Output(
                "run dispositions do not partition".into(),
            ));
        }
        if !digest(&self.signature_digest)
            || !digest(&self.replay_identity)
            || !digest(&self.research_object_digest)
            || self.artifact.content_hash != self.research_object_digest
        {
            return Err(PublicationWorkbenchError::Output(
                "research-object digest or signature metadata is invalid".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| PublicationWorkbenchError::Output(error.to_string()))
    }
}

pub fn compile_publication_research_object(
    request: &PublicationWorkbenchRequest5,
) -> Result<SignedResearchObject5, PublicationWorkbenchError> {
    validate_request(request)?;
    let mut runs = request.runs.clone();
    runs.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    let run_order = runs
        .iter()
        .map(|run| run.run_id.clone())
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut conditional = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for run in &runs {
        covered.extend(run.standards.iter().cloned());
        omissions.extend(
            run.omission_order
                .iter()
                .map(|item| format!("{}:{item}", run.run_id)),
        );
        if run.negative_result {
            negative.insert(format!("{}:negative-result", run.run_id));
        }
        let missing = request
            .required_standards
            .iter()
            .filter(|standard| !run.standards.contains(standard))
            .cloned()
            .collect::<Vec<_>>();
        let hard = !run.policy_allow
            || !run.protected_closure
            || !run.raw_data_local
            || run.semantic_profile != request.semantic_profile
            || !digest(&run.artifact_digest)
            || !digest(&run.evidence_digest)
            || !digest(&run.provenance_digest);
        if hard {
            blocked.insert(run.run_id.clone());
            omissions.insert(format!(
                "{}:policy-provenance-locality-or-semantic-blocked",
                run.run_id
            ));
        } else if !missing.is_empty()
            || run.reproducibility_score < 80
            || matches!(
                run.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            )
        {
            conditional.insert(run.run_id.clone());
            uncertainty.insert(format!("{}:release-closure-incomplete", run.run_id));
            for standard in missing {
                omissions.insert(format!("{}:missing-standard:{standard}", run.run_id));
            }
        } else if run.evidence_state == EvidenceState::Contradicted {
            unknown.insert(run.run_id.clone());
            negative.insert(format!("{}:contradicted-evidence", run.run_id));
        } else {
            qualified.insert(run.run_id.clone());
        }
    }
    let global_block =
        !request.policy_allow || !request.protected_closure || !request.raw_data_local;
    if global_block {
        blocked.extend(run_order.iter().cloned());
        qualified.clear();
        conditional.clear();
        unknown.clear();
        omissions.insert("request:policy-protected-closure-or-locality-blocked".into());
    }
    let disposition = if global_block || (!blocked.is_empty() && qualified.is_empty()) {
        "blocked"
    } else if !conditional.is_empty() || !unknown.is_empty() || !blocked.is_empty() {
        "conditional"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:release-closure-not-ready".into());
    }
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "consumer": request.consumer,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "run_order": run_order,
        "qualified_order": qualified,
        "conditional_order": conditional,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "required_standards": request.required_standards,
        "covered_standards": covered,
        "replay_identity": request.replay_identity,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("stress-research-object: {}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| PublicationWorkbenchError::Artifact(error.to_string()))?;
    let research_object_digest = artifact.content_hash.clone();
    let signature_digest = ContentHash::of_value(&json!({"research_object_digest": research_object_digest, "replay_identity": request.replay_identity, "contract_version": CONTRACT_VERSION})).map_err(|error| PublicationWorkbenchError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "qualified" {
        vec![format!(
            "view:authorized-research-state:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let out = SignedResearchObject5 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        run_order: serde_json::from_value(payload["run_order"].clone()).unwrap(),
        qualified_order: serde_json::from_value(payload["qualified_order"].clone()).unwrap(),
        conditional_order: serde_json::from_value(payload["conditional_order"].clone()).unwrap(),
        blocked_order: serde_json::from_value(payload["blocked_order"].clone()).unwrap(),
        unknown_order: serde_json::from_value(payload["unknown_order"].clone()).unwrap(),
        omission_order: serde_json::from_value(payload["omission_order"].clone()).unwrap(),
        uncertainty_order: serde_json::from_value(payload["uncertainty_order"].clone()).unwrap(),
        negative_evidence_order: serde_json::from_value(payload["negative_evidence_order"].clone())
            .unwrap(),
        required_standards: request.required_standards.clone(),
        covered_standards: serde_json::from_value(payload["covered_standards"].clone()).unwrap(),
        signature_digest,
        replay_identity: request.replay_identity.clone(),
        research_object_digest,
        artifact,
        effect_receipts,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

pub fn compile_publication_research_object_json(
    value: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request: PublicationWorkbenchRequest5 = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid stress publication request: {error}"))?;
    serde_json::to_value(
        compile_publication_research_object(&request).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

pub fn validate_publication_research_object_json(
    value: &serde_json::Value,
) -> Result<SignedResearchObject5, String> {
    let receipt: SignedResearchObject5 = serde_json::from_value(value.clone())
        .map_err(|error| format!("invalid stress publication receipt: {error}"))?;
    receipt.validate().map_err(|error| error.to_string())?;
    if receipt.feature_id != FEATURE_ID || receipt.contract_version != CONTRACT_VERSION {
        return Err("stress publication identity mismatch".into());
    }
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> PublicationWorkbenchRequest5 {
        PublicationWorkbenchRequest5 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "release-1".into(),
            consumer: "benchmark curator".into(),
            federation_id: "consortium-a".into(),
            purpose: "publish held-out stress benchmark".into(),
            semantic_profile: "stress-v2".into(),
            required_standards: vec!["ro-crate-1.3".into()],
            replay_identity: hash("replay"),
            runs: vec![ValidatedResearchRun4 {
                run_id: "run-a".into(),
                study_id: "study-a".into(),
                release_intent: "benchmark".into(),
                semantic_profile: "stress-v2".into(),
                artifact_digest: hash("artifact"),
                evidence_digest: hash("evidence"),
                provenance_digest: hash("provenance"),
                replay_identity: hash("replay"),
                evidence_state: EvidenceState::Supported,
                protected_closure: true,
                policy_allow: true,
                raw_data_local: true,
                negative_result: false,
                omission_order: Vec::new(),
                standards: vec!["ro-crate-1.3".into()],
                reproducibility_score: 95,
            }],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            network_permitted: false,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            publication_research_object_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_release_view() {
        assert_eq!(
            compile_publication_research_object(&request())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn unknown_evidence_is_conditional() {
        let mut r = request();
        r.runs[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            compile_publication_research_object(&r).unwrap().disposition,
            "conditional"
        );
    }
    #[test]
    fn policy_blocks_release() {
        let mut r = request();
        r.policy_allow = false;
        assert_eq!(
            compile_publication_research_object(&r).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn negative_result_is_retained() {
        let mut r = request();
        r.runs[0].negative_result = true;
        assert!(!compile_publication_research_object(&r)
            .unwrap()
            .negative_evidence_order
            .is_empty());
    }
}
