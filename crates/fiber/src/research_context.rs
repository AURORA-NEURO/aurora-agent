//! Question-to-Decision-Section compilation with omission certificates.
//!
//! Atlas feature: `AFA-fiber-P03-F01`.
//!
//! This public boundary binds the existing FIBER compiler output to the production research
//! contract. It never upgrades an incomplete context into a conclusion: protected-closure
//! failures are rejected, unresolved obligations are surfaced, and callers may require an
//! influence-classified sufficiency claim before accepting the receipt. Raw world and section
//! values remain local; the portable artifact carries only content identities and omission state.

use crate::{compile, FiberError, Query};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    LossSeverity, ProvenanceLink, ResearchContractError, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use bioprism_section::CertificateProfile;
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fiber-P03-F01";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchContextRequest {
    pub context_id: String,
    pub intent: String,
    pub require_sufficiency: bool,
    pub allow_unresolved: bool,
}

impl ResearchContextRequest {
    fn validate(&self) -> Result<(), ResearchContextError> {
        if self.context_id.trim().is_empty() || self.intent.trim().is_empty() {
            return Err(ResearchContextError::InvalidRequest(
                "context_id and intent are required".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchContextReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub context_id: String,
    pub intent: String,
    pub section_digest: ContentHash,
    pub certificate_digest: ContentHash,
    pub protected_closure_satisfied: bool,
    pub supports_sufficiency_claim: bool,
    pub unresolved_obligations: usize,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl ResearchContextReceipt {
    pub fn validate(&self) -> Result<(), ResearchContextError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(ResearchContextError::Contract(
                ResearchContractError::SchemaVersion {
                    expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                    found: self.schema_version.clone(),
                },
            ));
        }
        if self.feature_id != FEATURE_ID
            || self.context_id.trim().is_empty()
            || self.intent.trim().is_empty()
        {
            return Err(ResearchContextError::InvalidRequest(
                "context receipt identity is incomplete".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(ResearchContextError::Contract(
                ResearchContractError::BoundaryMismatch {
                    capability: self.context_id.clone(),
                },
            ));
        }
        if !self.protected_closure_satisfied {
            return Err(ResearchContextError::ProtectedClosure);
        }
        self.artifact
            .validate_metadata()
            .map_err(ResearchContextError::Contract)
    }

    pub fn verify_payload(&self, payload: &Value) -> Result<(), ResearchContextError> {
        self.validate()?;
        self.artifact
            .verify_payload(payload)
            .map_err(ResearchContextError::Contract)
    }

    pub fn digest(&self) -> Result<ContentHash, ResearchContextError> {
        let value = serde_json::to_value(self)
            .map_err(|error| ResearchContextError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ResearchContextError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ResearchContextError {
    #[error("invalid research-context request: {0}")]
    InvalidRequest(String),
    #[error("FIBER compilation error: {0}")]
    Compile(#[from] FiberError),
    #[error("protected closure was not satisfied")]
    ProtectedClosure,
    #[error("sufficiency claim was required but omission manifest is not sufficient")]
    InsufficientContext,
    #[error("unresolved obligations are not allowed for this request")]
    UnresolvedObligations,
    #[error("research contract error: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn research_context_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "fiber".into(),
        consumers: ["imaging core scientist".into(), "research program lead".into()].into(),
        behavior: "compiles bounded research intent into a Decision Section and omission-certified context receipt without upgrading unresolved evidence".into(),
        value: "makes question-to-context generation auditable, replayable, and safe for downstream researchers and agents".into(),
        inputs: vec![
            TypedPort { name: "research_context_request".into(), schema: "ResearchContextRequest@1".into(), required: true },
            TypedPort { name: "fiber_query".into(), schema: "fiber-query/0.5".into(), required: true },
            TypedPort { name: "fiber_world".into(), schema: "fiber-world/0.1".into(), required: true },
        ],
        outputs: vec![
            TypedPort { name: "research_context_receipt".into(), schema: "ResearchContextReceipt@1".into(), required: true },
            TypedPort { name: "typed_artifact".into(), schema: "TypedResearchArtifact@1".into(), required: true },
        ],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["read:institution-local-world".into(), "write:local-context-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "fixture:fiber-context-compilation".into(), state: EvidenceState::Supported, locator: Some("fixtures/fiber-v0.1".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_research_context(
    world: &World,
    query: &Query,
    request: &ResearchContextRequest,
) -> Result<ResearchContextReceipt, ResearchContextError> {
    request.validate()?;
    let output = compile(world, query)?;
    if !output.protected_closure_satisfied() {
        return Err(ResearchContextError::ProtectedClosure);
    }
    if request.require_sufficiency && !output.certificate.manifest.supports_sufficiency_claim() {
        return Err(ResearchContextError::InsufficientContext);
    }
    if !request.allow_unresolved && output.section.requires_refinement() {
        return Err(ResearchContextError::UnresolvedObligations);
    }
    let section_payload = output.section.to_json();
    let certificate_payload = output
        .certificate
        .to_json(CertificateProfile::Extended)
        .map_err(|error| ResearchContextError::Serialization(error.to_string()))?;
    let section_digest = ContentHash::of_value(&section_payload)
        .map_err(|error| ResearchContextError::Serialization(error.to_string()))?;
    let certificate_digest = ContentHash::of_value(&certificate_payload)
        .map_err(|error| ResearchContextError::Serialization(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "context_id": request.context_id,
        "intent": request.intent,
        "query_id": query.query_id.to_string(),
        "section_digest": section_digest,
        "certificate_digest": certificate_digest,
        "protected_closure_satisfied": output.protected_closure_satisfied(),
        "supports_sufficiency_claim": output.certificate.manifest.supports_sufficiency_claim(),
        "unresolved_obligations": output.section.unresolved_obligations.len(),
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("research-context:{}", request.context_id),
        "application/vnd.aurora.research-context+json",
        &payload,
        vec![SemanticLoss {
            field: "world_and_evidence_values".into(),
            reason: "raw local context remains at origin; portable receipt exports hashes and omission state".into(),
            severity: LossSeverity::Bounded,
        }],
        vec![
            ProvenanceLink {
                source_id: "decision-section".into(),
                relation: "compiled-section-digest".into(),
                digest: section_digest.clone(),
            },
            ProvenanceLink {
                source_id: "context-certificate".into(),
                relation: "compiled-certificate-digest".into(),
                digest: certificate_digest.clone(),
            },
        ],
    )?;
    let receipt = ResearchContextReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        context_id: request.context_id.clone(),
        intent: request.intent.clone(),
        section_digest,
        certificate_digest,
        protected_closure_satisfied: output.protected_closure_satisfied(),
        supports_sufficiency_claim: output.certificate.manifest.supports_sufficiency_claim(),
        unresolved_obligations: output.section.unresolved_obligations.len(),
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    receipt.verify_payload(&payload)?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::from_str;
    use std::path::PathBuf;

    fn fixture(relative: &str) -> Value {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "fixtures",
            "fiber-v0.1",
            relative,
        ]
        .iter()
        .collect();
        from_str(&std::fs::read_to_string(&path).expect("fixture exists")).expect("fixture JSON")
    }

    fn request() -> ResearchContextRequest {
        ResearchContextRequest {
            context_id: "context-demo".into(),
            intent: "compile bounded preclinical decision context".into(),
            require_sufficiency: false,
            allow_unresolved: true,
        }
    }

    #[test]
    fn context_receipt_preserves_protected_closure_and_hashes() {
        let world = World::from_json(fixture("radiogenomic_world.json")).unwrap();
        let query = Query::from_json(fixture("leakage_query.json")).unwrap();
        let receipt = compile_research_context(&world, &query, &request()).unwrap();
        assert!(receipt.protected_closure_satisfied);
        assert_eq!(receipt.section_digest.as_str().len(), 64);
        assert_eq!(receipt.certificate_digest.as_str().len(), 64);
        receipt.validate().unwrap();
        research_context_manifest().validate().unwrap();
    }

    #[test]
    fn identical_world_query_and_request_are_byte_stable() {
        let world = World::from_json(fixture("radiogenomic_world.json")).unwrap();
        let query = Query::from_json(fixture("leakage_query.json")).unwrap();
        let left = compile_research_context(&world, &query, &request()).unwrap();
        let right = compile_research_context(&world, &query, &request()).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }

    #[test]
    fn blank_context_identity_is_rejected_before_compilation() {
        let world = World::from_json(fixture("radiogenomic_world.json")).unwrap();
        let query = Query::from_json(fixture("leakage_query.json")).unwrap();
        let mut invalid = request();
        invalid.context_id.clear();
        assert!(matches!(
            compile_research_context(&world, &query, &invalid),
            Err(ResearchContextError::InvalidRequest(_))
        ));
    }
}
