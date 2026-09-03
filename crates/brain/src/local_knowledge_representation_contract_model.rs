//! Local single-study knowledge-representation contract model.
//!
//! Atlas feature: `AFA-brain-P04-F05`. This is a typed, versioned contract boundary,
//! distinct from the inference engine: it makes schema compatibility and migration
//! explicit before a knowledge-world artifact can be consumed.

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

pub const FEATURE_ID: &str = "AFA-brain-P04-F05";
pub const CONTRACT_VERSION: &str = "brain-local-knowledge-representation-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ScopedResearchClaims1@1";
pub const OUTPUT_SCHEMA: &str = "TypedKnowledgeWorld1@1";
const CONTRACT_CONTENT_TYPE: &str = "application/vnd.aurora.typed-knowledge-world+json";
const MAX_ITEMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeContractClaim {
    pub claim_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub state: EvidenceState,
    pub study_id: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeContractModelRequest {
    pub request_id: String,
    pub study_id: String,
    pub claims: Vec<KnowledgeContractClaim>,
    pub required_claim_ids: Vec<String>,
    pub input_schema: String,
    pub output_schema: String,
    pub source_revision: u16,
    pub target_revision: u16,
    pub migration_requested: bool,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeContractDisposition {
    Compatible,
    Migrated,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeContractModelReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub disposition: KnowledgeContractDisposition,
    pub input_schema: String,
    pub output_schema: String,
    pub source_revision: u16,
    pub target_revision: u16,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub contract_digest: ContentHash,
    pub migration_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KnowledgeContractModelError {
    #[error("invalid local knowledge contract: {0}")]
    Invalid(String),
    #[error("knowledge contract artifact failed: {0}")]
    Artifact(String),
}

impl KnowledgeContractModelReceipt {
    pub fn validate(&self) -> Result<(), KnowledgeContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.source_revision == 0
            || self.target_revision < self.source_revision
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge contract identity, schema, revision, locality, candidates, or effects are incomplete".into(),
            ));
        }
        let collections = [
            &self.candidate_order,
            &self.admitted_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.missing_order,
            &self.semantic_loss_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ];
        if collections.iter().any(|values| values.len() > MAX_ITEMS) {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge contract collection exceeds the bounded contract limit".into(),
            ));
        }
        for values in collections {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(KnowledgeContractModelError::Invalid(
                    "knowledge contract ordering is not canonical".into(),
                ));
            }
        }
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let admitted = self.admitted_order.iter().collect::<BTreeSet<_>>();
        let unresolved = self.unresolved_order.iter().collect::<BTreeSet<_>>();
        let denied = self.denied_order.iter().collect::<BTreeSet<_>>();
        let mut classified = admitted.clone();
        classified.extend(unresolved.iter());
        classified.extend(denied.iter());
        if classified != candidates
            || !admitted.is_disjoint(&unresolved)
            || !admitted.is_disjoint(&denied)
            || !unresolved.is_disjoint(&denied)
            || self
                .missing_order
                .iter()
                .any(|claim| !candidates.contains(claim))
            || !self
                .semantic_loss_order
                .iter()
                .all(|claim| admitted.contains(claim))
        {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge contract states do not partition declared claims".into(),
            ));
        }
        for digest in [
            &self.contract_digest,
            &self.migration_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(KnowledgeContractModelError::Invalid(
                    "knowledge contract digest is invalid".into(),
                ));
            }
        }
        let gate_blocked = self.omissions.iter().any(|item| {
            item == "control:policy-denied"
                || item == "control:protected-closure-incomplete"
                || item == "control:raw-data-locality-failed"
        });
        let expected_disposition = if gate_blocked {
            KnowledgeContractDisposition::Blocked
        } else if self.admitted_order.is_empty()
            || !self.unresolved_order.is_empty()
            || !self.denied_order.is_empty()
            || !self.missing_order.is_empty()
        {
            KnowledgeContractDisposition::Partial
        } else if self.target_revision > self.source_revision {
            KnowledgeContractDisposition::Migrated
        } else {
            KnowledgeContractDisposition::Compatible
        };
        if self.disposition != expected_disposition {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge contract disposition does not match classified claims or gates".into(),
            ));
        }
        let expected_contract_digest = ContentHash::of_value(&json!({
            "study_id": self.study_id,
            "input_schema": INPUT_SCHEMA,
            "output_schema": OUTPUT_SCHEMA,
            "source_revision": self.source_revision,
            "target_revision": self.target_revision,
            "candidate_order": self.candidate_order,
            "admitted_order": self.admitted_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
            "missing_order": self.missing_order,
            "semantic_loss_order": self.semantic_loss_order
        }))
        .map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))?;
        if self.contract_digest != expected_contract_digest {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge contract digest does not match classified claims".into(),
            ));
        }
        let expected_migration_digest = ContentHash::of_value(&json!({
            "source_revision": self.source_revision,
            "target_revision": self.target_revision,
            "migration_requested": self.target_revision > self.source_revision,
            "semantic_loss_order": self.semantic_loss_order
        }))
        .map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))?;
        if self.migration_digest != expected_migration_digest {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge migration digest does not match revision state".into(),
            ));
        }
        let expected_effect = if self.disposition == KnowledgeContractDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!("read:local-knowledge-contract:{}", self.request_id)]
        };
        if self.effect_receipts != expected_effect {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge contract effect does not match disposition".into(),
            ));
        }
        let expected_artifact_id = format!("brain-local-knowledge-contract:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTRACT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(KnowledgeContractModelError::Invalid(
                "knowledge contract artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, KnowledgeContractModelError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))
    }
}

pub fn local_knowledge_representation_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["agent developer".into(), "research workflow compiler".into()].into(),
        behavior: "validates and migrates typed single-study knowledge representation contracts before local artifact exchange".into(),
        value: "makes schema compatibility, semantic loss, and migration evidence explicit instead of silently coercing research claims".into(),
        inputs: vec![TypedPort { name: "scoped_research_claims".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "typed_knowledge_world_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn model_local_knowledge_representation_contract(
    request: &KnowledgeContractModelRequest,
) -> Result<KnowledgeContractModelReceipt, KnowledgeContractModelError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.claims.is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.source_revision == 0
        || request.target_revision < request.source_revision
        || (request.target_revision > request.source_revision && !request.migration_requested)
        || request.replay_identity.as_str().len() != 64
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(KnowledgeContractModelError::Invalid(
            "knowledge contract identity, schemas, revisions, migration, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let candidate = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if candidate.windows(2).any(|pair| pair[0] == pair[1])
        || candidate.iter().any(|value| value.trim().is_empty())
    {
        return Err(KnowledgeContractModelError::Invalid(
            "knowledge contract claim identifiers must be unique and non-empty".into(),
        ));
    }
    let map = claims
        .iter()
        .map(|claim| (claim.claim_id.clone(), claim))
        .collect::<BTreeMap<_, _>>();
    let mut admitted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::from([
        "gate:schema-compatibility".to_string(),
        "gate:unknown-is-not-asserted".to_string(),
        "gate:locality".to_string(),
    ]);
    let mut negative = BTreeSet::new();
    for claim_id in &candidate {
        let claim = map[claim_id];
        if !request.policy_allow
            || !request.protected_closure
            || claim.study_id != request.study_id
            || claim.boundary != PRECLINICAL_BOUNDARY
        {
            denied.insert(claim_id.clone());
            negative.insert(format!("claim:{claim_id}:scope-policy-closure"));
        } else if claim.evidence_digest.is_none() || claim.provenance_digest.is_none() {
            unresolved.insert(claim_id.clone());
            missing.insert(claim_id.clone());
            omissions.insert(format!("claim:{claim_id}:evidence-or-provenance-missing"));
        } else if matches!(
            claim.state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(claim_id.clone());
            uncertainty.insert(format!("claim:{claim_id}:unknown-not-asserted"));
        } else if matches!(claim.state, EvidenceState::Contradicted) {
            denied.insert(claim_id.clone());
            negative.insert(format!("claim:{claim_id}:contradicted"));
        } else if request.target_revision > request.source_revision {
            semantic_loss.insert(claim_id.clone());
            admitted.insert(claim_id.clone());
        } else {
            admitted.insert(claim_id.clone());
        }
    }
    for required_id in request.required_claim_ids.iter().collect::<BTreeSet<_>>() {
        if !map.contains_key(required_id) {
            omissions.insert(format!("claim:{required_id}:required-missing"));
            uncertainty.insert(format!("claim:{required_id}:required-unresolved"));
        } else if !admitted.contains(required_id) {
            uncertainty.insert(format!("claim:{required_id}:required-not-admitted"));
        }
    }
    if request.target_revision > request.source_revision {
        uncertainty.insert(format!(
            "migration:{}-to-{}",
            request.source_revision, request.target_revision
        ));
    }
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("control:raw-data-locality-failed".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            KnowledgeContractDisposition::Blocked
        } else if admitted.is_empty()
            || !unresolved.is_empty()
            || !denied.is_empty()
            || !missing.is_empty()
        {
            KnowledgeContractDisposition::Partial
        } else if request.target_revision > request.source_revision {
            KnowledgeContractDisposition::Migrated
        } else {
            KnowledgeContractDisposition::Compatible
        };
    let contract_digest = ContentHash::of_value(&json!({"study_id": request.study_id, "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "source_revision": request.source_revision, "target_revision": request.target_revision, "candidate_order": candidate, "admitted_order": admitted, "unresolved_order": unresolved, "denied_order": denied, "missing_order": missing, "semantic_loss_order": semantic_loss})).map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))?;
    let migration_digest = ContentHash::of_value(&json!({"source_revision": request.source_revision, "target_revision": request.target_revision, "migration_requested": request.migration_requested, "semantic_loss_order": semantic_loss})).map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "disposition": disposition, "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "source_revision": request.source_revision, "target_revision": request.target_revision, "candidate_order": candidate, "admitted_order": admitted, "unresolved_order": unresolved, "denied_order": denied, "missing_order": missing, "semantic_loss_order": semantic_loss, "contract_digest": contract_digest, "migration_digest": migration_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-local-knowledge-contract:{}", request.request_id),
        CONTRACT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| KnowledgeContractModelError::Artifact(error.to_string()))?;
    let receipt = KnowledgeContractModelReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        disposition,
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        source_revision: request.source_revision,
        target_revision: request.target_revision,
        candidate_order: candidate,
        admitted_order: admitted.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        missing_order: missing.into_iter().collect(),
        semantic_loss_order: semantic_loss.into_iter().collect(),
        contract_digest,
        migration_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if matches!(
            disposition,
            KnowledgeContractDisposition::Compatible
                | KnowledgeContractDisposition::Migrated
                | KnowledgeContractDisposition::Partial
        ) {
            vec![format!(
                "read:local-knowledge-contract:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn receipt_payload(receipt: &KnowledgeContractModelReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_id": receipt.study_id,
        "disposition": receipt.disposition,
        "input_schema": receipt.input_schema,
        "output_schema": receipt.output_schema,
        "source_revision": receipt.source_revision,
        "target_revision": receipt.target_revision,
        "candidate_order": receipt.candidate_order,
        "admitted_order": receipt.admitted_order,
        "unresolved_order": receipt.unresolved_order,
        "denied_order": receipt.denied_order,
        "missing_order": receipt.missing_order,
        "semantic_loss_order": receipt.semantic_loss_order,
        "contract_digest": receipt.contract_digest,
        "migration_digest": receipt.migration_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> KnowledgeContractModelRequest {
        let h = hash("contract");
        let claim = |id: &str, state: EvidenceState| KnowledgeContractClaim {
            claim_id: id.into(),
            subject: "organoid".into(),
            predicate: "expresses".into(),
            object: "marker".into(),
            evidence_digest: Some(h.clone()),
            provenance_digest: Some(h.clone()),
            state,
            study_id: "study:one".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        KnowledgeContractModelRequest {
            request_id: "request:contract".into(),
            study_id: "study:one".into(),
            claims: vec![
                claim("claim:a", EvidenceState::Supported),
                claim("claim:b", EvidenceState::Supported),
            ],
            required_claim_ids: vec!["claim:a".into()],
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            source_revision: 1,
            target_revision: 1,
            migration_requested: false,
            replay_identity: h,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            local_knowledge_representation_contract_model_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn compatible_contract_is_admitted() {
        assert_eq!(
            model_local_knowledge_representation_contract(&request())
                .unwrap()
                .disposition,
            KnowledgeContractDisposition::Compatible
        );
    }
    #[test]
    fn revision_migration_is_explicit() {
        let mut v = request();
        v.target_revision = 2;
        v.migration_requested = true;
        let r = model_local_knowledge_representation_contract(&v).unwrap();
        assert_eq!(r.disposition, KnowledgeContractDisposition::Migrated);
        assert!(r.uncertainty.iter().any(|x| x.starts_with("migration:")));
    }
    #[test]
    fn missing_evidence_is_partial() {
        let mut v = request();
        v.claims[0].evidence_digest = None;
        let r = model_local_knowledge_representation_contract(&v).unwrap();
        assert_eq!(r.disposition, KnowledgeContractDisposition::Partial);
        assert_eq!(r.missing_order, vec!["claim:a".to_string()]);
    }
    #[test]
    fn unknown_and_contradiction_remain_nonasserted() {
        let mut v = request();
        v.claims[0].state = EvidenceState::Unknown;
        v.claims[1].state = EvidenceState::Contradicted;
        let r = model_local_knowledge_representation_contract(&v).unwrap();
        assert_eq!(r.disposition, KnowledgeContractDisposition::Partial);
        assert!(!r.negative_evidence.is_empty());
    }
    #[test]
    fn schema_mismatch_is_rejected() {
        let mut v = request();
        v.input_schema = "Other@1".into();
        assert!(matches!(
            model_local_knowledge_representation_contract(&v),
            Err(KnowledgeContractModelError::Invalid(_))
        ));
    }
    #[test]
    fn policy_denial_blocks_effects() {
        let mut v = request();
        v.policy_allow = false;
        let r = model_local_knowledge_representation_contract(&v).unwrap();
        assert_eq!(r.disposition, KnowledgeContractDisposition::Blocked);
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn digest_is_stable() {
        let r = model_local_knowledge_representation_contract(&request()).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
