//! Prospective high-throughput Oracle semantic-parity assurance.
//!
//! Atlas feature: `AFA-oracle-P28-F08`.  The harness admits a parity corpus only
//! when Rust, Python, TypeScript, schema, semantic, artifact, and provenance
//! digests agree byte-for-byte. It records mismatch and unknown witnesses rather
//! than allowing a convenient surface to silently define the contract.

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

pub const FEATURE_ID: &str = "AFA-oracle-P28-F08";
pub const CONTRACT_VERSION: &str =
    "oracle-federated-continual-oracle-semantic-parity-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "OracleParityContract3@1";
pub const OUTPUT_SCHEMA: &str = "OracleSemanticParityReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.oracle-semantic-parity-receipt-7+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleParityCase {
    pub case_id: String,
    pub rust_digest: ContentHash,
    pub python_digest: ContentHash,
    pub typescript_digest: ContentHash,
    pub schema_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub local_only: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleParityContract {
    pub schema_version: String,
    pub corpus_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub required_case_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub cases: Vec<OracleParityCase>,
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
pub enum OracleParityDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleSemanticParityReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub corpus_id: String,
    pub federation_id: String,
    pub semantic_profile: String,
    pub disposition: OracleParityDisposition,
    pub case_order: Vec<String>,
    pub passed_order: Vec<String>,
    pub mismatch_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_case_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub witness_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OracleSemanticParityError {
    #[error("invalid Oracle parity fixture: {0}")]
    Invalid(String),
    #[error("Oracle parity artifact failed: {0}")]
    Artifact(String),
}
fn invalid(value: impl Into<String>) -> OracleSemanticParityError {
    OracleSemanticParityError::Invalid(value.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl OracleSemanticParityReceipt7 {
    pub fn validate(&self) -> Result<(), OracleSemanticParityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.corpus_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.case_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "parity identity, locality, cases, or effects are incomplete",
            ));
        }
        for values in [
            &self.case_order,
            &self.passed_order,
            &self.mismatch_order,
            &self.unknown_order,
            &self.blocked_order,
            &self.missing_case_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("parity ordering is not canonical"));
            }
        }
        let ids = self.case_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .passed_order
            .iter()
            .chain(self.mismatch_order.iter())
            .chain(self.unknown_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if parts.len() != ids.len() || parts.iter().cloned().collect::<BTreeSet<_>>() != ids {
            return Err(invalid("parity states do not partition cases"));
        }
        for value in [
            &self.replay_identity,
            &self.witness_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("parity digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| OracleSemanticParityError::Artifact(error.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE {
            return Err(invalid("parity artifact type is invalid"));
        }
        if self.disposition == OracleParityDisposition::Qualified
            && self.effect_receipts != [format!("verify:oracle-semantic-parity:{}", self.corpus_id)]
        {
            return Err(invalid("qualified parity effect is invalid"));
        }
        if self.disposition != OracleParityDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified parity must block release"));
        }
        Ok(())
    }
}

pub fn oracle_semantic_parity_contract_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "oracle".into(), consumers: ["context compiler engineer".into(), "release governance board".into(), "federation operator".into()].into(), behavior: "verifies prospective high-throughput Oracle parity fixtures across Rust, Python, TypeScript, schema, semantic, artifact, and provenance surfaces without executing workflows".into(), value: "prevents cross-language semantic drift from changing a research contract silently and preserves mismatch evidence for release review".into(), inputs: vec![TypedPort { name: "oracle_parity_fixture".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "oracle_parity_witness".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_oracle_semantic_parity_contract(
    fixture: &OracleParityContract,
) -> Result<OracleSemanticParityReceipt7, OracleSemanticParityError> {
    validate_fixture(fixture)?;
    let mut cases = fixture.cases.clone();
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let case_order = cases
        .iter()
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let required = fixture
        .required_case_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let known = case_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut passed = BTreeSet::new();
    let mut mismatch = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = required
        .difference(&known)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for row in &cases {
        let digests = [
            &row.rust_digest,
            &row.python_digest,
            &row.typescript_digest,
            &row.schema_digest,
            &row.semantic_digest,
            &row.artifact_digest,
            &row.provenance_digest,
        ];
        let parity = digests.windows(2).all(|pair| pair[0] == pair[1]);
        omissions.extend(
            row.omissions
                .iter()
                .map(|item| format!("{}:{item}", row.case_id)),
        );
        uncertainty.extend(
            row.uncertainty
                .iter()
                .map(|item| format!("{}:{item}", row.case_id)),
        );
        if row.evidence_state == EvidenceState::Contradicted || !row.local_only || !row.permitted {
            blocked.insert(row.case_id.clone());
        } else if !parity {
            mismatch.insert(row.case_id.clone());
        } else if row.replay_identity != fixture.replay_identity
            || row.semantic_profile != fixture.semantic_profile
            || !row.omissions.is_empty()
            || !row.uncertainty.is_empty()
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            unknown.insert(row.case_id.clone());
        } else {
            passed.insert(row.case_id.clone());
        }
    }
    for id in &missing {
        omissions.insert(format!("{id}:required-case-missing"));
    }
    if !fixture.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !fixture.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !fixture.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !fixture.federation_approved {
        uncertainty.insert("request:federation-approval-missing".into());
    }
    negative.extend(
        fixture
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !fixture.policy_allow
        || !fixture.protected_closure
        || !fixture.signed_approval
        || !fixture.federation_approved
        || !fixture.raw_data_local
        || !fixture.aggregate_only
        || !fixture.adversarial_events.is_empty();
    if global_block {
        blocked.extend(case_order.iter().cloned());
        passed.clear();
        unknown.clear();
        mismatch.clear();
        missing.clear();
        omissions.insert("request:parity-release-gate-blocked".into());
    }
    let disposition = if global_block {
        OracleParityDisposition::Blocked
    } else if required.is_subset(&passed)
        && unknown.is_empty()
        && mismatch.is_empty()
        && blocked.is_empty()
    {
        OracleParityDisposition::Qualified
    } else {
        OracleParityDisposition::Unresolved
    };
    let passed_order = passed.into_iter().collect::<Vec<_>>();
    let mismatch_order = mismatch.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_case_order = missing.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == OracleParityDisposition::Qualified {
        vec![format!(
            "verify:oracle-semantic-parity:{}",
            fixture.corpus_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"corpus_id":fixture.corpus_id,"federation_id":fixture.federation_id,"semantic_profile":fixture.semantic_profile,"disposition":disposition,"case_order":case_order,"passed_order":passed_order,"mismatch_order":mismatch_order,"unknown_order":unknown_order,"blocked_order":blocked_order,"missing_case_order":missing_case_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_evidence_order,"replay_identity":fixture.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":fixture.raw_data_local,"aggregate_only":fixture.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let witness_digest = ContentHash::of_value(&payload)
        .map_err(|error| OracleSemanticParityError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("oracle-semantic-parity:{}", fixture.corpus_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| OracleSemanticParityError::Artifact(error.to_string()))?;
    let witness = OracleSemanticParityReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        corpus_id: fixture.corpus_id.clone(),
        federation_id: fixture.federation_id.clone(),
        semantic_profile: fixture.semantic_profile.clone(),
        disposition,
        case_order: payload["case_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        passed_order: payload["passed_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        mismatch_order: payload["mismatch_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unknown_order: payload["unknown_order"]
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
        missing_case_order: payload["missing_case_order"]
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
        replay_identity: fixture.replay_identity.clone(),
        witness_digest,
        artifact,
        effect_receipts: payload["effect_receipts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        raw_data_local: fixture.raw_data_local,
        aggregate_only: fixture.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    witness.validate()?;
    Ok(witness)
}

fn validate_fixture(fixture: &OracleParityContract) -> Result<(), OracleSemanticParityError> {
    if fixture.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || fixture.corpus_id.trim().is_empty()
        || fixture.federation_id.trim().is_empty()
        || fixture.semantic_profile.trim().is_empty()
        || fixture.required_case_order.is_empty()
        || !canonical(&fixture.required_case_order)
        || fixture.cases.is_empty()
        || !digest(&fixture.replay_identity)
        || !canonical(&fixture.adversarial_events)
        || !fixture.raw_data_local
        || !fixture.aggregate_only
        || fixture.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(invalid(
            "parity fixture identity, closure, replay, locality, or boundary is invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for row in &fixture.cases {
        if row.case_id.trim().is_empty()
            || !ids.insert(row.case_id.clone())
            || row.semantic_profile.trim().is_empty()
            || !digest(&row.rust_digest)
            || !digest(&row.python_digest)
            || !digest(&row.typescript_digest)
            || !digest(&row.schema_digest)
            || !digest(&row.semantic_digest)
            || !digest(&row.artifact_digest)
            || !digest(&row.provenance_digest)
            || !digest(&row.replay_identity)
            || !canonical(&row.omissions)
            || !canonical(&row.uncertainty)
        {
            return Err(invalid(format!(
                "parity case {} is malformed or duplicated",
                row.case_id
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
    fn fixture() -> OracleParityContract {
        let d = hash("parity");
        let case = |id: &str| OracleParityCase {
            case_id: id.into(),
            rust_digest: d.clone(),
            python_digest: d.clone(),
            typescript_digest: d.clone(),
            schema_digest: d.clone(),
            semantic_digest: d.clone(),
            artifact_digest: d.clone(),
            provenance_digest: d.clone(),
            replay_identity: d.clone(),
            semantic_profile: "preclinical-neural".into(),
            evidence_state: EvidenceState::Supported,
            omissions: vec![],
            uncertainty: vec![],
            local_only: true,
            permitted: true,
        };
        OracleParityContract {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            corpus_id: "corpus:one".into(),
            federation_id: "fed:commons".into(),
            semantic_profile: "preclinical-neural".into(),
            required_case_order: vec!["case:a".into(), "case:b".into()],
            replay_identity: d.clone(),
            cases: vec![case("case:a"), case("case:b")],
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
            oracle_semantic_parity_contract_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn qualified_parity() {
        assert_eq!(
            model_oracle_semantic_parity_contract(&fixture())
                .unwrap()
                .disposition,
            OracleParityDisposition::Qualified
        );
    }
    #[test]
    fn deterministic_witness() {
        let a = model_oracle_semantic_parity_contract(&fixture()).unwrap();
        let b = model_oracle_semantic_parity_contract(&fixture()).unwrap();
        assert_eq!(a.witness_digest, b.witness_digest);
    }
    #[test]
    fn mismatch_is_unresolved() {
        let mut value = fixture();
        value.cases[0].python_digest = hash("different");
        let out = model_oracle_semantic_parity_contract(&value).unwrap();
        assert!(out.mismatch_order.contains(&"case:a".into()));
        assert_eq!(out.disposition, OracleParityDisposition::Unresolved);
    }
    #[test]
    fn unknown_state_is_unresolved() {
        let mut value = fixture();
        value.cases[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            model_oracle_semantic_parity_contract(&value)
                .unwrap()
                .disposition,
            OracleParityDisposition::Unresolved
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = fixture();
        value.policy_allow = false;
        assert_eq!(
            model_oracle_semantic_parity_contract(&value)
                .unwrap()
                .disposition,
            OracleParityDisposition::Blocked
        );
    }
    #[test]
    fn adversarial_blocks() {
        let mut value = fixture();
        value.adversarial_events.push("poisoned-fixture".into());
        assert_eq!(
            model_oracle_semantic_parity_contract(&value)
                .unwrap()
                .disposition,
            OracleParityDisposition::Blocked
        );
    }
}
