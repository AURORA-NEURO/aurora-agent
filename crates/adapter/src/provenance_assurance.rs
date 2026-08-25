//! Multimodal provenance and signing assurance.
//!
//! Atlas feature: `AFA-adapter-P18-F26`.
//!
//! This verifier constructs a deterministic, content-addressed lineage envelope from caller-owned
//! artifact and derivation metadata. It checks references, acyclicity, tool determinism,
//! multimodal coverage, localization, and signing evidence without accepting raw payload bytes or
//! claiming that a signed lineage proves a scientific conclusion.

use bioprism_foundation::{
    LossSeverity, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P18-F26";
pub const CONTRACT_VERSION: &str = "multimodal-provenance-signing-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceArtifact {
    pub artifact_id: String,
    pub content_digest: ContentHash,
    pub study_id: String,
    pub modality: String,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivationStep {
    pub step_id: String,
    pub operation: String,
    pub input_artifact_ids: Vec<String>,
    pub output_artifact_id: String,
    pub output_digest: ContentHash,
    pub tool_digest: ContentHash,
    pub deterministic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAndDerivation {
    pub schema_version: String,
    pub envelope_id: String,
    pub root_artifact_id: String,
    pub root_digest: ContentHash,
    pub source_artifacts: Vec<ProvenanceArtifact>,
    pub derivation_steps: Vec<DerivationStep>,
    pub comparable: bool,
    pub protected_omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub policy_allow: bool,
    pub signer_public_key_hex: String,
    pub signer_signature_hex: String,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceAssuranceVerdict {
    Signed,
    Conditional,
    Unresolved,
    Contradicted,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenanceEnvelope {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub envelope_id: String,
    pub root_artifact_id: String,
    pub root_digest: ContentHash,
    pub verdict: ProvenanceAssuranceVerdict,
    pub lineage_order: Vec<String>,
    pub derivation_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub tool_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub signer_public_key_hex: String,
    pub signer_signature_hex: String,
    pub effect_receipt: String,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl SignedProvenanceEnvelope {
    pub fn validate(&self) -> Result<(), ProvenanceAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ProvenanceAssuranceError::Contract(
                "provenance assurance identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.envelope_id.trim().is_empty()
            || self.root_artifact_id.trim().is_empty()
            || self.lineage_order.is_empty()
            || self.derivation_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipt.trim().is_empty()
        {
            return Err(ProvenanceAssuranceError::InvalidRequest("provenance identity, lineage, derivations, reasons, locality, effects, and boundary are required".into()));
        }
        if self.lineage_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .derivation_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.study_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "provenance output ordering is not canonical".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProvenanceAssuranceError::Contract(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ProvenanceAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ProvenanceAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ProvenanceAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ProvenanceAssuranceError {
    #[error("invalid provenance assurance request: {0}")]
    InvalidRequest(String),
    #[error("provenance assurance contract rejected: {0}")]
    Contract(String),
    #[error("provenance reference is invalid: {0}")]
    InvalidReference(String),
    #[error("provenance cycle detected at {0}")]
    Cycle(String),
    #[error("provenance serialization failed: {0}")]
    Serialization(String),
}

pub fn assure_provenance(
    input: &ArtifactAndDerivation,
) -> Result<SignedProvenanceEnvelope, ProvenanceAssuranceError> {
    validate_input(input)?;
    let mut steps = input.derivation_steps.clone();
    steps.sort_by(|left, right| left.step_id.cmp(&right.step_id));
    let mut artifacts = input.source_artifacts.clone();
    artifacts.sort_by(|left, right| left.artifact_id.cmp(&right.artifact_id));
    let source_ids = artifacts
        .iter()
        .map(|artifact| artifact.artifact_id.clone())
        .collect::<BTreeSet<_>>();
    let mut producers = BTreeMap::new();
    for step in &steps {
        producers.insert(step.output_artifact_id.clone(), step.clone());
    }
    let derivation_order = topological_order(&steps, &source_ids)?;
    let mut lineage = source_ids.clone();
    lineage.extend(producers.keys().cloned());
    let lineage_order = lineage.into_iter().collect::<Vec<_>>();
    let study_order = artifacts
        .iter()
        .map(|artifact| artifact.study_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = artifacts
        .iter()
        .map(|artifact| artifact.modality.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let tool_order = steps
        .iter()
        .map(|step| step.tool_digest.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut omissions = input.protected_omissions.clone();
    let mut uncertainty = Vec::new();
    let negative_evidence = input.negative_evidence.clone();
    let has_multimodal = modality_order.iter().any(|modality| modality == "imaging")
        && modality_order.iter().any(|modality| modality == "omics");
    let signature_present = !input.signer_public_key_hex.trim().is_empty()
        && !input.signer_signature_hex.trim().is_empty();
    let verdict = if !input.policy_allow {
        ProvenanceAssuranceVerdict::Blocked
    } else if !signature_present {
        ProvenanceAssuranceVerdict::Unresolved
    } else if !input.comparable || !has_multimodal {
        ProvenanceAssuranceVerdict::Conditional
    } else if !omissions.is_empty() || !negative_evidence.is_empty() {
        ProvenanceAssuranceVerdict::Conditional
    } else {
        ProvenanceAssuranceVerdict::Signed
    };
    let mut reasons = vec![format!(
        "{} artifacts and {} deterministic derivation steps linked in canonical order",
        lineage_order.len(),
        steps.len()
    )];
    let mut semantic_loss = Vec::new();
    if !input.comparable {
        omissions.push("cross-study comparability was not established".into());
    }
    if !has_multimodal {
        omissions.push("imaging and omics coverage is incomplete".into());
    }
    if !omissions.is_empty() {
        reasons.push(
            "protected provenance gaps remain explicit and prevent unconditional signing".into(),
        );
        semantic_loss.push(SemanticLoss {
            field: "omissions".into(),
            reason: "missing provenance cannot be inferred from a content digest".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    if !signature_present {
        reasons.push("signer key or signature evidence is absent".into());
        uncertainty.push("lineage integrity is not equivalent to signer authorization".into());
    }
    if !negative_evidence.is_empty() {
        reasons.push("negative provenance evidence remains attached to the envelope".into());
    }
    if !input.policy_allow {
        reasons.push("policy denied the unsafe-release effect".into());
    }
    let effect_receipt = if verdict == ProvenanceAssuranceVerdict::Signed {
        "write_signed_provenance_envelope_local_only"
    } else {
        "block_unsafe_release_and_retain_provenance_receipt"
    };
    let payload = json!({ "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "envelope_id": input.envelope_id, "root_artifact_id": input.root_artifact_id, "root_digest": input.root_digest, "verdict": verdict, "lineage_order": lineage_order, "derivation_order": derivation_order, "study_order": study_order, "modality_order": modality_order, "tool_order": tool_order, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "semantic_loss": semantic_loss, "reasons": reasons, "signer_public_key_hex": input.signer_public_key_hex, "signer_signature_hex": input.signer_signature_hex, "effect_receipt": effect_receipt, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY });
    let artifact = TypedResearchArtifact::from_payload(
        format!("provenance-envelope:{}", input.envelope_id),
        "application/vnd.aurora.signed-provenance-envelope+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| ProvenanceAssuranceError::Contract(error.to_string()))?;
    let envelope = SignedProvenanceEnvelope {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        envelope_id: input.envelope_id.clone(),
        root_artifact_id: input.root_artifact_id.clone(),
        root_digest: input.root_digest.clone(),
        verdict,
        lineage_order,
        derivation_order,
        study_order,
        modality_order,
        tool_order,
        omissions,
        uncertainty,
        negative_evidence,
        semantic_loss,
        reasons,
        signer_public_key_hex: input.signer_public_key_hex.clone(),
        signer_signature_hex: input.signer_signature_hex.clone(),
        effect_receipt: effect_receipt.into(),
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    envelope.validate()?;
    Ok(envelope)
}

fn validate_input(input: &ArtifactAndDerivation) -> Result<(), ProvenanceAssuranceError> {
    if input.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || input.envelope_id.trim().is_empty()
        || input.root_artifact_id.trim().is_empty()
        || input.source_artifacts.is_empty()
        || input.derivation_steps.is_empty()
        || !input.raw_data_local
        || input.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ProvenanceAssuranceError::InvalidRequest(
            "provenance identity, artifacts, derivations, locality, and boundary are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for artifact in &input.source_artifacts {
        if artifact.artifact_id.trim().is_empty()
            || artifact.study_id.trim().is_empty()
            || artifact.modality.trim().is_empty()
            || !artifact.raw_data_local
        {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "source artifact metadata is incomplete or non-local".into(),
            ));
        }
        if !ids.insert(artifact.artifact_id.clone()) {
            return Err(ProvenanceAssuranceError::InvalidReference(
                artifact.artifact_id.clone(),
            ));
        }
    }
    let mut step_ids = BTreeSet::new();
    let mut outputs = BTreeSet::new();
    for step in &input.derivation_steps {
        if step.step_id.trim().is_empty()
            || step.operation.trim().is_empty()
            || step.input_artifact_ids.is_empty()
            || step.output_artifact_id.trim().is_empty()
            || !step.deterministic
        {
            return Err(ProvenanceAssuranceError::InvalidRequest(format!(
                "derivation step {} lacks deterministic metadata",
                step.step_id
            )));
        }
        if !step_ids.insert(step.step_id.clone())
            || !outputs.insert(step.output_artifact_id.clone())
            || ids.contains(&step.output_artifact_id)
        {
            return Err(ProvenanceAssuranceError::InvalidReference(
                step.step_id.clone(),
            ));
        }
    }
    if !outputs.contains(&input.root_artifact_id) {
        return Err(ProvenanceAssuranceError::InvalidReference(
            "root artifact is not a derivation output".into(),
        ));
    }
    if input
        .derivation_steps
        .iter()
        .find(|step| step.output_artifact_id == input.root_artifact_id)
        .map(|step| step.output_digest != input.root_digest)
        .unwrap_or(true)
    {
        return Err(ProvenanceAssuranceError::InvalidReference(
            "root digest does not match its derivation output".into(),
        ));
    }
    Ok(())
}

fn topological_order(
    steps: &[DerivationStep],
    source_ids: &BTreeSet<String>,
) -> Result<Vec<String>, ProvenanceAssuranceError> {
    let mut produced = source_ids.clone();
    let mut remaining = steps.iter().collect::<Vec<_>>();
    let mut ordered = Vec::new();
    while !remaining.is_empty() {
        let mut ready = remaining
            .iter()
            .filter(|step| {
                step.input_artifact_ids
                    .iter()
                    .all(|input| produced.contains(input))
            })
            .map(|step| step.step_id.clone())
            .collect::<Vec<_>>();
        ready.sort();
        if ready.is_empty() {
            return Err(ProvenanceAssuranceError::Cycle(
                remaining[0].step_id.clone(),
            ));
        }
        for step_id in ready {
            let index = remaining
                .iter()
                .position(|step| step.step_id == step_id)
                .expect("ready step exists");
            let step = remaining.remove(index);
            produced.insert(step.output_artifact_id.clone());
            ordered.push(step.step_id.clone());
        }
    }
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> ArtifactAndDerivation {
        let protocol = ContentHash::of_bytes(b"source");
        let derived = ContentHash::of_bytes(b"derived");
        ArtifactAndDerivation {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            envelope_id: "envelope:qc".into(),
            root_artifact_id: "artifact:root".into(),
            root_digest: derived.clone(),
            source_artifacts: vec![
                ProvenanceArtifact {
                    artifact_id: "artifact:omics".into(),
                    content_digest: protocol.clone(),
                    study_id: "study:1".into(),
                    modality: "omics".into(),
                    raw_data_local: true,
                },
                ProvenanceArtifact {
                    artifact_id: "artifact:imaging".into(),
                    content_digest: protocol,
                    study_id: "study:2".into(),
                    modality: "imaging".into(),
                    raw_data_local: true,
                },
            ],
            derivation_steps: vec![DerivationStep {
                step_id: "step:root".into(),
                operation: "harmonize".into(),
                input_artifact_ids: vec!["artifact:imaging".into(), "artifact:omics".into()],
                output_artifact_id: "artifact:root".into(),
                output_digest: derived,
                tool_digest: ContentHash::of_bytes(b"tool"),
                deterministic: true,
            }],
            comparable: true,
            protected_omissions: Vec::new(),
            negative_evidence: Vec::new(),
            policy_allow: true,
            signer_public_key_hex: "a".repeat(64),
            signer_signature_hex: "b".repeat(128),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn provenance_is_deterministic_under_input_order() {
        let mut reversed = input();
        reversed.source_artifacts.reverse();
        let first = assure_provenance(&input()).unwrap();
        let second = assure_provenance(&reversed).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.verdict, ProvenanceAssuranceVerdict::Signed);
    }
    #[test]
    fn missing_reference_is_rejected() {
        let mut input = input();
        input.derivation_steps[0]
            .input_artifact_ids
            .push("artifact:missing".into());
        assert!(matches!(
            assure_provenance(&input),
            Err(ProvenanceAssuranceError::Cycle(_))
        ));
    }
    #[test]
    fn protected_gap_is_conditional_and_retained() {
        let mut input = input();
        input
            .protected_omissions
            .push("tool attestation pending".into());
        let envelope = assure_provenance(&input).unwrap();
        assert_eq!(envelope.verdict, ProvenanceAssuranceVerdict::Conditional);
        assert!(!envelope.omissions.is_empty());
    }
    #[test]
    fn denied_policy_blocks_signing() {
        let mut input = input();
        input.policy_allow = false;
        assert_eq!(
            assure_provenance(&input).unwrap().verdict,
            ProvenanceAssuranceVerdict::Blocked
        );
    }
}
