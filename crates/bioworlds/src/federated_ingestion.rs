//! Federated continual multimodal-ingestion contract.
//!
//! Atlas feature: `AFA-bioworlds-P06-F08`.
//! The contract is a typed-data boundary for harmonized research-object
//! manifests. It never accepts raw experimental bytes and never exports them;
//! only content-addressed metadata, provenance, omissions, and policy receipts
//! may leave the institution-local boundary.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioworlds-P06-F08";
pub const CONTRACT_VERSION: &str = "bioworlds-federated-multimodal-ingestion/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityArtifact {
    pub modality_id: String,
    pub modality: String,
    pub schema_version: String,
    pub coordinate_system: String,
    pub unit_system: String,
    pub artifact_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub state: ModalityState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawModalityBundle {
    pub request_id: String,
    pub workflow_id: String,
    pub institution_id: String,
    pub study_id: String,
    pub scope: String,
    pub required_modalities: Vec<String>,
    pub modalities: Vec<ModalityArtifact>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionDisposition {
    Harmonized,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonizedResearchObject {
    pub object_id: String,
    pub study_id: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: IngestionDisposition,
    pub modality_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub object_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedIngestionReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub institution_id: String,
    pub disposition: IngestionDisposition,
    pub object: HarmonizedResearchObject,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedIngestionError {
    #[error("invalid federated ingestion request: {0}")]
    Invalid(String),
    #[error("federated ingestion serialization failed: {0}")]
    Serialization(String),
}

impl FederatedIngestionReceipt {
    pub fn validate(&self) -> Result<(), FederatedIngestionError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.object.boundary != PRECLINICAL_BOUNDARY
            || self.object.scope.trim().is_empty()
            || self.object.study_id.trim().is_empty()
            || (self.object.accepted_order.is_empty()
                && self.object.blocked_order.is_empty()
                && self.object.omissions.is_empty()
                && self.object.uncertainty.is_empty()
                && self.object.negative_evidence.is_empty())
        {
            return Err(FederatedIngestionError::Invalid(
                "ingestion identity, object, checks, effects, locality, or boundary is incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.object.modality_order,
            &self.object.accepted_order,
            &self.object.blocked_order,
            &self.object.omissions,
            &self.object.uncertainty,
            &self.object.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedIngestionError::Invalid(
                    "federated ingestion ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.object.artifact_order, &self.object.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedIngestionError::Invalid(
                    "federated ingestion digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedIngestionError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedIngestionError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedIngestionError::Serialization(error.to_string()))
    }
}

pub fn operate_federated_ingestion(
    request: &RawModalityBundle,
) -> Result<FederatedIngestionReceipt, FederatedIngestionError> {
    validate_request(request)?;
    let mut modalities = request.modalities.clone();
    modalities.sort_by(|left, right| left.modality_id.cmp(&right.modality_id));
    let required = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut accepted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut modality_order = BTreeSet::new();
    let mut artifact_order = BTreeSet::new();
    let mut provenance_order = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for modality in &modalities {
        let cost = modality.modality.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = !modality.schema_version.trim().is_empty()
            && !modality.coordinate_system.trim().is_empty()
            && !modality.unit_system.trim().is_empty()
            && modality.omissions.is_empty()
            && modality.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.federation_allow
            && request.raw_data_local
            && modality.state == ModalityState::Supported
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            accepted.insert(modality.modality_id.clone());
            modality_order.insert(modality.modality.clone());
            artifact_order.insert(modality.artifact_digest.clone());
            provenance_order.insert(modality.provenance_digest.clone());
        } else {
            blocked.insert(modality.modality_id.clone());
            if modality.state != ModalityState::Supported {
                negative.insert(
                    format!(
                        "modality:{}:state-{:?}-not-harmonized",
                        modality.modality_id, modality.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if !complete {
                omissions.insert(format!(
                    "modality:{}:schema-coordinate-unit-or-evidence-incomplete",
                    modality.modality_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "modality:{}:budget-ceiling-exceeded",
                    modality.modality_id
                ));
            }
        }
    }
    for required_modality in required {
        if !modalities
            .iter()
            .any(|item| item.modality == required_modality && accepted.contains(&item.modality_id))
        {
            omissions.insert(format!(
                "modality:{required_modality}:required-but-not-admitted"
            ));
        }
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
    if !request.federation_allow {
        negative.insert("request:federation-manifest-exchange-denied".into());
    }
    let accepted_order = accepted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition =
        if !request.policy_allow || !request.signed_approval || !request.federation_allow {
            IngestionDisposition::Blocked
        } else if !request.protected_closure {
            IngestionDisposition::Unknown
        } else if accepted_order.is_empty() {
            IngestionDisposition::Unknown
        } else if blocked_order.is_empty() && omissions.is_empty() {
            IngestionDisposition::Harmonized
        } else {
            IngestionDisposition::Partial
        };
    let modality_order = modality_order.into_iter().collect::<Vec<_>>();
    let artifact_order = artifact_order.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance_order.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let semantic_profile = modalities
        .iter()
        .filter(|item| accepted_order.iter().any(|id| id == &item.modality_id))
        .map(|item| {
            format!(
                "{}:{}:{}",
                item.modality, item.schema_version, item.semantic_digest
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    let mut checks = vec![
        "canonical modality, artifact, provenance, omission, and effect ordering".into(),
        "policy, protected-closure, signed-approval, federation, locality, and budget gates".into(),
        "raw modality payloads remain institution-local; only typed digests and manifests cross sites".into(),
        "unknown, contradicted, unmeasured, and missing required modalities remain unresolved".into(),
    ];
    checks.sort();
    let mut effect_receipts = if !accepted_order.is_empty() {
        accepted_order
            .iter()
            .map(|id| format!("exchange:permitted-harmonized-manifest:{id}"))
            .collect::<Vec<_>>()
    } else {
        vec![format!("block:federated-ingestion:{disposition:?}").to_ascii_lowercase()]
    };
    effect_receipts.sort();
    let object_id = format!("harmonized-object:{}", request.request_id);
    let object_payload = json!({
        "object_id": object_id,
        "study_id": request.study_id,
        "scope": request.scope,
        "semantic_profile": semantic_profile,
        "disposition": disposition,
        "modality_order": modality_order,
        "accepted_order": accepted_order,
        "blocked_order": blocked_order,
        "artifact_order": artifact_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let object_digest = ContentHash::of_value(&object_payload)
        .map_err(|error| FederatedIngestionError::Serialization(error.to_string()))?;
    let object = HarmonizedResearchObject {
        object_id,
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        semantic_profile,
        disposition,
        modality_order,
        accepted_order,
        blocked_order,
        artifact_order,
        provenance_order,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: request.replay_identity.clone(),
        object_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = FederatedIngestionReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        institution_id: request.institution_id.clone(),
        disposition,
        object,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &RawModalityBundle) -> Result<(), FederatedIngestionError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.modalities.is_empty()
        || request.required_modalities.is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(FederatedIngestionError::Invalid(
            "ingestion identity, modalities, required modality order, budget, scope, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for modality in &request.modalities {
        if modality.modality_id.trim().is_empty()
            || modality.modality.trim().is_empty()
            || !ids.insert(modality.modality_id.clone())
            || modality.boundary != PRECLINICAL_BOUNDARY
            || modality.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || modality
                .uncertainty
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || modality
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedIngestionError::Invalid(format!(
                "modality {} is invalid or duplicated",
                modality.modality_id
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

    fn modality(id: &str, modality: &str, state: ModalityState) -> ModalityArtifact {
        ModalityArtifact {
            modality_id: id.into(),
            modality: modality.into(),
            schema_version: "ome-ngff/0.5".into(),
            coordinate_system: "sample-local".into(),
            unit_system: "si".into(),
            artifact_digest: hash(&format!("artifact:{id}")),
            semantic_digest: hash(&format!("semantic:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            state,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(modalities: Vec<ModalityArtifact>) -> RawModalityBundle {
        RawModalityBundle {
            request_id: "ingestion:federated".into(),
            workflow_id: "workflow:continual-ingestion".into(),
            institution_id: "site:a".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            required_modalities: vec!["imaging".into(), "omics".into()],
            modalities,
            replay_identity: hash("replay"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn harmonizes_supported_modalities_without_exporting_raw_data() {
        let receipt = operate_federated_ingestion(&request(vec![
            modality("imaging:a", "imaging", ModalityState::Supported),
            modality("omics:a", "omics", ModalityState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, IngestionDisposition::Harmonized);
        assert_eq!(receipt.object.accepted_order, vec!["imaging:a", "omics:a"]);
        assert!(receipt.raw_data_local);
        assert_eq!(receipt.digest(), receipt.digest());
    }

    #[test]
    fn contradiction_is_partial_and_negative_evidence_is_retained() {
        let receipt = operate_federated_ingestion(&request(vec![
            modality("imaging:a", "imaging", ModalityState::Supported),
            modality("omics:a", "omics", ModalityState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, IngestionDisposition::Partial);
        assert_eq!(receipt.object.blocked_order, vec!["omics:a"]);
        assert!(!receipt.negative_evidence.is_empty());
    }

    #[test]
    fn protected_closure_gap_is_unknown() {
        let mut input = request(vec![modality(
            "imaging:a",
            "imaging",
            ModalityState::Supported,
        )]);
        input.protected_closure = false;
        let receipt = operate_federated_ingestion(&input).unwrap();
        assert_eq!(receipt.disposition, IngestionDisposition::Unknown);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("protected-closure")));
    }

    #[test]
    fn federation_denial_blocks_manifest_exchange() {
        let mut input = request(vec![modality(
            "imaging:a",
            "imaging",
            ModalityState::Supported,
        )]);
        input.federation_allow = false;
        let receipt = operate_federated_ingestion(&input).unwrap();
        assert_eq!(receipt.disposition, IngestionDisposition::Blocked);
        assert!(receipt.effect_receipts[0].starts_with("block:"));
    }

    #[test]
    fn missing_required_modality_is_explicit() {
        let receipt = operate_federated_ingestion(&request(vec![modality(
            "imaging:a",
            "imaging",
            ModalityState::Supported,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, IngestionDisposition::Partial);
        assert!(receipt.omissions.iter().any(|item| item.contains("omics")));
    }
}
