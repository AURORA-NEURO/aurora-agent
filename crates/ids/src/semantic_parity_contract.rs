//! IDS semantic-parity contract model (`AFA-ids-P28-F06`).
//!
//! Compares content-addressed summaries from independent imaging and omics
//! producers. Schema, semantic, study, modality, and replay disagreement stays
//! visible; no raw outputs are imported or moved.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P28-F06";
pub const CONTRACT_VERSION: &str = "ids-multimodal-semantic-parity-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "IdsParityFixture8@1";
pub const OUTPUT_SCHEMA: &str = "IdsParityWitness9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-parity-witness-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_FIXTURES: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParityEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsParityFixture8 {
    pub fixture_id: String,
    pub producer_id: String,
    pub study_id: String,
    pub schema_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub modality_order: Vec<String>,
    pub artifact_digests: Vec<ContentHash>,
    pub provenance_digest: ContentHash,
    pub evidence_state: ParityEvidenceState,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsParityRequest7 {
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub fixtures: Vec<IdsParityFixture8>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsParityWitness9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsParityWitness9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub disposition: String,
    pub fixture_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub schema_digest_order: Vec<ContentHash>,
    pub semantic_digest_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub parity_digest: ContentHash,
    pub artifact: IdsParityWitness9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SemanticParityError {
    #[error("invalid IDS semantic-parity request: {0}")]
    Invalid(String),
    #[error("IDS semantic-parity witness failed validation: {0}")]
    Witness(String),
}

pub fn semantic_parity_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["formal methods researcher", "context compiler engineer", "compatibility operator"],
        "behavior": "compare schema, semantic, study, modality, artifact, provenance, and replay identities across typed multimodal fixtures",
        "value": "detects semantic drift and incomparable multimodal summaries before a research workflow consumes them",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["exchange:semantic-parity-digests", "manage:local-capability"],
        "permissions": ["read:local-parity-fixtures", "request:semantic-parity"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}
fn ordered_hashes(values: &[ContentHash]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

impl IdsParityWitness9 {
    pub fn validate(&self) -> Result<(), SemanticParityError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.required_study_order.is_empty()
            || self.required_modality_order.is_empty()
            || self.fixture_order.len() < 2
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(SemanticParityError::Witness("parity identity, requirements, fixtures, effects, locality, or disposition is incomplete".into()));
        }
        for values in [
            &self.required_study_order,
            &self.required_modality_order,
            &self.fixture_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(SemanticParityError::Witness(
                    "parity ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.schema_digest_order,
            &self.semantic_digest_order,
            &self.artifact_order,
        ] {
            if !ordered_hashes(values) {
                return Err(SemanticParityError::Witness(
                    "parity digest ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.fixture_order.iter().cloned());
        let parts = self
            .qualified_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.fixture_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
        {
            return Err(SemanticParityError::Witness(
                "fixture states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.parity_digest)
            || self.artifact.content_hash != self.parity_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(SemanticParityError::Witness(
                "parity digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:semantic-parity-digests:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(SemanticParityError::Witness(
                "effect is outside the governed parity gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, SemanticParityError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| SemanticParityError::Witness(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| SemanticParityError::Witness(error.to_string()))
    }
}

fn validate_request(request: &IdsParityRequest7) -> Result<(), SemanticParityError> {
    if request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || request.fixtures.len() < 2
        || request.fixtures.len() > MAX_FIXTURES
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(SemanticParityError::Invalid("parity identity, requirements, fixture bound, replay, locality, or boundary is invalid".into()));
    }
    for values in [
        &request.required_study_order,
        &request.required_modality_order,
    ] {
        if values.iter().any(|value| value.trim().is_empty())
            || BTreeSet::from_iter(values.iter().cloned()).len() != values.len()
        {
            return Err(SemanticParityError::Invalid(
                "required studies and modalities must be unique and non-empty".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for fixture in &request.fixtures {
        if fixture.fixture_id.trim().is_empty()
            || !ids.insert(fixture.fixture_id.clone())
            || fixture.producer_id.trim().is_empty()
            || fixture.study_id.trim().is_empty()
            || !valid_digest(&fixture.schema_digest)
            || !valid_digest(&fixture.semantic_digest)
            || fixture.modality_order.is_empty()
            || fixture
                .modality_order
                .iter()
                .any(|value| value.trim().is_empty())
            || fixture
                .artifact_digests
                .iter()
                .any(|digest| !valid_digest(digest))
            || !valid_digest(&fixture.provenance_digest)
            || !valid_digest(&fixture.replay_identity)
            || !fixture.local
            || !fixture.aggregate_only
        {
            return Err(SemanticParityError::Invalid(format!(
                "fixture {} is invalid, duplicated, non-local, or not digest-bound",
                fixture.fixture_id
            )));
        }
    }
    Ok(())
}

pub fn evaluate_ids_semantic_parity(
    request: &IdsParityRequest7,
) -> Result<IdsParityWitness9, SemanticParityError> {
    validate_request(request)?;
    let mut fixtures = request.fixtures.clone();
    fixtures.sort_by(|left, right| left.fixture_id.cmp(&right.fixture_id));
    let fixture_order = fixtures
        .iter()
        .map(|fixture| fixture.fixture_id.clone())
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut schema = BTreeSet::new();
    let mut semantic = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut studies = BTreeSet::new();
    for fixture in &fixtures {
        schema.insert(fixture.schema_digest.clone());
        semantic.insert(fixture.semantic_digest.clone());
        artifacts.extend(fixture.artifact_digests.iter().cloned());
        provenance.insert(fixture.provenance_digest.clone());
        modalities.extend(fixture.modality_order.iter().cloned());
        studies.insert(fixture.study_id.clone());
        if fixture.evidence_state == ParityEvidenceState::Contradicted {
            blocked.insert(fixture.fixture_id.clone());
            negative.insert(format!("{}:contradicted", fixture.fixture_id));
        } else if !matches!(
            fixture.evidence_state,
            ParityEvidenceState::Proven | ParityEvidenceState::Supported
        ) {
            unresolved.insert(fixture.fixture_id.clone());
            uncertainty.insert(format!("{}:evidence-state", fixture.fixture_id));
        } else if fixture.replay_identity != request.replay_identity {
            unresolved.insert(fixture.fixture_id.clone());
            uncertainty.insert(format!("{}:replay-identity", fixture.fixture_id));
        }
    }
    let required_modalities = BTreeSet::from_iter(request.required_modality_order.iter().cloned());
    let required_studies = BTreeSet::from_iter(request.required_study_order.iter().cloned());
    for modality in required_modalities.difference(&modalities) {
        omissions.insert(format!("modality:{modality}:missing"));
        negative.insert(format!("modality:{modality}:no-parity-evidence"));
    }
    for study in required_studies.difference(&studies) {
        omissions.insert(format!("study:{study}:missing"));
        negative.insert(format!("study:{study}:no-parity-evidence"));
    }
    let parity_match = fixtures.windows(2).all(|pair| {
        pair[0].schema_digest == pair[1].schema_digest
            && pair[0].semantic_digest == pair[1].semantic_digest
            && pair[0]
                .modality_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
                == pair[1]
                    .modality_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
    });
    if !parity_match {
        uncertainty.insert("fixtures:schema-semantic-or-modality-disagreement".into());
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if !global_block && parity_match && omissions.is_empty() {
        qualified.extend(fixtures.iter().map(|fixture| fixture.fixture_id.clone()));
    } else if !global_block {
        for fixture in &fixtures {
            if !blocked.contains(&fixture.fixture_id) && !unresolved.contains(&fixture.fixture_id) {
                unresolved.insert(fixture.fixture_id.clone());
            }
        }
    }
    if global_block {
        blocked.extend(fixture_order.iter().cloned());
        qualified.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if global_block || qualified_order.is_empty() && unresolved_order.is_empty() {
        "blocked"
    } else if !blocked_order.is_empty()
        || !unresolved_order.is_empty()
        || !omissions.is_empty()
        || !parity_match
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:semantic-parity-not-closed".into());
    }
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let mut effect_order = if disposition == "qualified" {
        vec![
            "exchange:semantic-parity-digests".to_string(),
            "manage:local-capability".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    effect_order.sort();
    let mut effect_receipts = effect_order
        .iter()
        .map(|effect| {
            if effect == "block:unsafe-release" {
                effect.clone()
            } else {
                format!("{effect}:{}", request.request_id)
            }
        })
        .collect::<Vec<_>>();
    effect_receipts.sort();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"required_study_order":BTreeSet::from_iter(request.required_study_order.iter().cloned()).into_iter().collect::<Vec<_>>(),"required_modality_order":BTreeSet::from_iter(request.required_modality_order.iter().cloned()).into_iter().collect::<Vec<_>>(),"disposition":disposition,"fixture_order":fixture_order,"qualified_order":qualified_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_study_order":required_studies.difference(&studies).cloned().collect::<Vec<_>>(),"missing_modality_order":required_modalities.difference(&modalities).cloned().collect::<Vec<_>>(),"schema_digest_order":schema.into_iter().collect::<Vec<_>>(),"semantic_digest_order":semantic.into_iter().collect::<Vec<_>>(),"artifact_order":artifacts.into_iter().collect::<Vec<_>>(),"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative_evidence_order,"effect_order":effect_order,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let parity_digest = ContentHash::of_value(&payload)
        .map_err(|error| SemanticParityError::Witness(error.to_string()))?;
    let receipt = IdsParityWitness9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        required_study_order: payload["required_study_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        required_modality_order: payload["required_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        disposition: disposition.into(),
        fixture_order: payload["fixture_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        qualified_order: payload["qualified_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        missing_study_order: payload["missing_study_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        missing_modality_order: payload["missing_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        schema_digest_order: payload["schema_digest_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| serde_json::from_value(value.clone()).ok())
            .collect(),
        semantic_digest_order: payload["semantic_digest_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| serde_json::from_value(value.clone()).ok())
            .collect(),
        artifact_order: payload["artifact_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| serde_json::from_value(value.clone()).ok())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        effect_order: payload["effect_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect(),
        replay_identity: request.replay_identity.clone(),
        parity_digest: parity_digest.clone(),
        artifact: IdsParityWitness9Artifact {
            artifact_id: format!("ids-parity-witness-9:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: parity_digest,
            semantic_loss: payload["omission_order"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts,
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
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn fixture(id: &str) -> IdsParityFixture8 {
        IdsParityFixture8 {
            fixture_id: id.into(),
            producer_id: format!("producer:{id}"),
            study_id: "study-1".into(),
            schema_digest: h("schema"),
            semantic_digest: h("semantic"),
            modality_order: vec!["imaging".into(), "omics".into()],
            artifact_digests: vec![h(id)],
            provenance_digest: h("provenance"),
            evidence_state: ParityEvidenceState::Supported,
            replay_identity: h("replay"),
            local: true,
            aggregate_only: true,
        }
    }
    fn request() -> IdsParityRequest7 {
        IdsParityRequest7 {
            request_id: "ids:parity:req".into(),
            purpose: "check parity".into(),
            semantic_profile: "ome-v1".into(),
            required_study_order: vec!["study-1".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            fixtures: vec![fixture("fixture:a"), fixture("fixture:b")],
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(semantic_parity_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn matching_fixtures_are_qualified() {
        assert_eq!(
            evaluate_ids_semantic_parity(&request())
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn semantic_disagreement_is_unresolved() {
        let mut q = request();
        q.fixtures[1].semantic_digest = h("other");
        assert_eq!(
            evaluate_ids_semantic_parity(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn missing_modality_is_unresolved() {
        let mut q = request();
        q.required_modality_order.push("spatial".into());
        assert_eq!(
            evaluate_ids_semantic_parity(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            evaluate_ids_semantic_parity(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn parity_digest_is_deterministic() {
        let a = evaluate_ids_semantic_parity(&request()).unwrap();
        let b = evaluate_ids_semantic_parity(&request()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
