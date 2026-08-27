//! Bounded cross-surface semantic-parity copilot.
//!
//! Atlas feature: `AFA-interweave-P28-F11`.
//!
//! The copilot compares caller-supplied Rust, Python, and TypeScript canonical digests for a
//! prospective batch. It emits a witness and an invocation receipt, but does not invoke a tool;
//! the bounded-tool effect is available only to a downstream policy-authorized runtime.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-interweave-P28-F11";
pub const CONTRACT_VERSION: &str = "interweave-prospective-semantic-parity-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "InterweaveParityFixture3@1";
pub const OUTPUT_SCHEMA: &str = "InterweaveParityWitness3@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweaveParityFixture {
    pub fixture_id: String,
    pub batch_id: String,
    pub scope: String,
    pub schema_version: String,
    pub expected_canonical_digest: ContentHash,
    pub rust_digest: Option<ContentHash>,
    pub python_digest: Option<ContentHash>,
    pub typescript_digest: Option<ContentHash>,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub budget_units: u32,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterweaveParityWitness {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub fixture_id: String,
    pub batch_id: String,
    pub scope: String,
    pub disposition: String,
    pub parity_order: Vec<String>,
    pub matched_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub mismatch_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub invocation_receipt: String,
    pub replay_identity: ContentHash,
    pub witness_digest: ContentHash,
    pub semantic_loss: Vec<SemanticLoss>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemanticParityError {
    #[error("invalid semantic-parity fixture: {0}")]
    Invalid(String),
    #[error("semantic-parity artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl InterweaveParityWitness {
    pub fn validate(&self) -> Result<(), SemanticParityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.fixture_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.parity_order.is_empty()
            || self.invocation_receipt.trim().is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.effect_receipts.is_empty()
        {
            return Err(SemanticParityError::Invalid(
                "parity identity, witness, locality, boundary, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.parity_order,
            &self.matched_order,
            &self.missing_order,
            &self.mismatch_order,
            &self.uncertain_order,
            &self.omission_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(SemanticParityError::Invalid(
                    "parity witness ordering is not canonical".into(),
                ));
            }
        }
        let covered = self
            .matched_order
            .iter()
            .chain(self.missing_order.iter())
            .chain(self.mismatch_order.iter())
            .chain(self.uncertain_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if covered.len() != self.parity_order.len()
            || covered.iter().collect::<BTreeSet<_>>().len() != covered.len()
            || covered.iter().collect::<BTreeSet<_>>()
                != self.parity_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(SemanticParityError::Invalid(
                "parity states do not partition surfaces".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:declared-tool:") && effect != "block:unsafe-release"
        }) {
            return Err(SemanticParityError::Invalid(
                "parity effect is outside declared-tool gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| SemanticParityError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, SemanticParityError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| SemanticParityError::Artifact(error.to_string()))?,
        )
        .map_err(|error| SemanticParityError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "interweave".into(), consumers: BTreeSet::from(["research workflow operator".into(), "agent SDK".into(), "parity reviewer".into()]), behavior: "compares cross-language canonical digests and emits a bounded parity witness before any declared-tool invocation".into(), value: "prevents semantic drift and byte-level replay divergence from being hidden behind agent automation".into(), inputs: vec![TypedPort { name: "parity_fixture".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "parity_witness".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::from([Effect::ReadLocalData, Effect::WriteLocalArtifact]), permissions: BTreeSet::from(["invoke:declared-tools".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: vec![AuthorityRequirement { role: "parity-reviewer".into(), reason: "approve declared-tool invocation after parity closure".into() }], autonomy_tier: AutonomyTier::A2, surfaces: BTreeSet::from([ResearchSurface::McpTool, ResearchSurface::Sdk, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_fixture(fixture: &InterweaveParityFixture) -> Result<(), SemanticParityError> {
    if fixture.schema_version != INPUT_SCHEMA
        || fixture.fixture_id.trim().is_empty()
        || fixture.batch_id.trim().is_empty()
        || fixture.scope.trim().is_empty()
        || fixture.artifact_digest.is_none()
        || fixture.provenance_digest.is_none()
        || fixture.budget_units == 0
        || !fixture.raw_data_local
        || fixture.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(SemanticParityError::Invalid(
            "fixture identity, artifact/provenance, budget, locality, or boundary is invalid"
                .into(),
        ));
    }
    Ok(())
}

pub fn compare(
    fixture: &InterweaveParityFixture,
) -> Result<InterweaveParityWitness, SemanticParityError> {
    validate_fixture(fixture)?;
    let mut parity_order: Vec<String> = vec!["python".into(), "rust".into(), "typescript".into()];
    parity_order.sort();
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut mismatch = Vec::new();
    let mut uncertain = Vec::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let values = BTreeMap::from([
        ("python", fixture.python_digest.as_ref()),
        ("rust", fixture.rust_digest.as_ref()),
        ("typescript", fixture.typescript_digest.as_ref()),
    ]);
    for surface in &parity_order {
        match values[surface.as_str()] {
            None => {
                missing.push(surface.clone());
                omissions.insert(format!("{surface}:digest-missing"));
            }
            Some(value) if value == &fixture.expected_canonical_digest => {
                matched.push(surface.clone());
                negative.insert(format!("{surface}:negative-result-not-observed"));
            }
            Some(_) => {
                mismatch.push(surface.clone());
                semantic_loss.push(SemanticLoss {
                    field: format!("surface:{surface}"),
                    reason: "cross-language canonical digest mismatch".into(),
                    severity: LossSeverity::DecisionRelevant,
                });
                negative.insert(format!("{surface}:parity-mismatch"));
            }
        }
    }
    if matches!(
        fixture.evidence_state,
        EvidenceState::Unknown | EvidenceState::Speculative
    ) {
        uncertain.extend(parity_order.iter().cloned());
        uncertainty.insert("fixture:evidence-state".into());
    }
    if fixture.evidence_state == EvidenceState::Contradicted {
        uncertain.extend(parity_order.iter().cloned());
        uncertainty.insert("fixture:contradicted".into());
    }
    if !fixture.policy_allow {
        omissions.insert("fixture:policy-denied".into());
    }
    if !fixture.protected_closure {
        omissions.insert("fixture:protected-closure-incomplete".into());
    }
    if !fixture.signed_approval {
        omissions.insert("fixture:signed-approval-missing".into());
    }
    if !fixture.adversarial_events.is_empty() {
        omissions.extend(
            fixture
                .adversarial_events
                .iter()
                .map(|event| format!("fixture:adversarial:{event}")),
        );
    }
    if !fixture.policy_allow || !fixture.protected_closure || !fixture.adversarial_events.is_empty()
    {
        uncertain.clear();
    }
    let disposition = if !fixture.policy_allow
        || !fixture.protected_closure
        || !fixture.raw_data_local
        || !fixture.adversarial_events.is_empty()
    {
        "blocked"
    } else if !fixture.signed_approval {
        "approval_required"
    } else if !mismatch.is_empty() || !missing.is_empty() || !uncertain.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let invocation_receipt = if disposition == "qualified" {
        format!("invoke:declared-tool:parity-batch:{}", fixture.batch_id)
    } else {
        "block:unsafe-release".into()
    };
    let payload = json!({"schema_version": OUTPUT_SCHEMA, "fixture_id": fixture.fixture_id, "batch_id": fixture.batch_id, "parity_order": parity_order, "matched_order": matched, "missing_order": missing, "mismatch_order": mismatch, "uncertain_order": uncertain, "replay_identity": fixture.replay_identity, "disposition": disposition});
    let witness_digest = ContentHash::of_value(&payload)
        .map_err(|error| SemanticParityError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("parity-witness:{}", fixture.fixture_id),
        "application/vnd.aurora.interweave-parity-witness+json",
        &payload,
        semantic_loss.clone(),
        vec![ProvenanceLink {
            source_id: fixture.batch_id.clone(),
            relation: "interweave-semantic-parity".into(),
            digest: witness_digest.clone(),
        }],
    )
    .map_err(|error| SemanticParityError::Artifact(error.to_string()))?;
    let receipt = InterweaveParityWitness {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        fixture_id: fixture.fixture_id.clone(),
        batch_id: fixture.batch_id.clone(),
        scope: fixture.scope.clone(),
        disposition: disposition.into(),
        parity_order,
        matched_order: matched,
        missing_order: missing,
        mismatch_order: mismatch,
        uncertain_order: uncertain,
        omission_order: omissions.iter().cloned().collect(),
        invocation_receipt: invocation_receipt.clone(),
        replay_identity: fixture.replay_identity.clone(),
        witness_digest,
        semantic_loss,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        artifact,
        effect_receipts: vec![invocation_receipt],
        raw_data_local: fixture.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash() -> ContentHash {
        ContentHash::of_bytes(b"parity")
    }
    fn fixture() -> InterweaveParityFixture {
        InterweaveParityFixture {
            fixture_id: "fixture:parity".into(),
            batch_id: "batch:1".into(),
            scope: "organoid".into(),
            schema_version: INPUT_SCHEMA.into(),
            expected_canonical_digest: hash(),
            rust_digest: Some(hash()),
            python_digest: Some(hash()),
            typescript_digest: Some(hash()),
            artifact_digest: Some(hash()),
            provenance_digest: Some(hash()),
            replay_identity: hash(),
            evidence_state: EvidenceState::Supported,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            budget_units: 10,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn complete_parity_is_qualified_and_invocable() {
        let receipt = compare(&fixture()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert!(receipt
            .invocation_receipt
            .starts_with("invoke:declared-tool:"));
    }
    #[test]
    fn missing_surface_is_unresolved() {
        let mut value = fixture();
        value.python_digest = None;
        let receipt = compare(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(receipt.missing_order.contains(&"python".into()));
    }
    #[test]
    fn mismatch_is_negative_and_unresolved() {
        let mut value = fixture();
        value.typescript_digest = Some(ContentHash::of_bytes(b"mismatch"));
        let receipt = compare(&value).unwrap();
        assert_eq!(receipt.disposition, "unresolved");
        assert!(receipt.mismatch_order.contains(&"typescript".into()));
        assert!(!receipt.semantic_loss.is_empty());
    }
    #[test]
    fn approval_and_policy_gates_never_invoke() {
        let mut value = fixture();
        value.signed_approval = false;
        let receipt = compare(&value).unwrap();
        assert_eq!(receipt.disposition, "approval_required");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        value.policy_allow = false;
        let receipt = compare(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
    }
    #[test]
    fn unknown_and_adversarial_inputs_remain_explicit() {
        let mut value = fixture();
        value.evidence_state = EvidenceState::Unknown;
        value.adversarial_events = vec!["prompt-injection".into()];
        let receipt = compare(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(!receipt.omissions.is_empty());
    }
    #[test]
    fn manifest_is_a2_and_declared_tool_bound() {
        assert_eq!(capability_manifest().autonomy_tier, AutonomyTier::A2);
        assert!(capability_manifest()
            .permissions
            .contains("invoke:declared-tools"));
    }
}
