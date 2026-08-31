//! Multimodal provenance and signing assurance.
//!
//! Atlas feature: `AFA-adapter-P18-F26`.
//!
//! This verifier constructs a deterministic, content-addressed lineage envelope from caller-owned
//! artifact and derivation metadata. It checks references, acyclicity, tool determinism,
//! multimodal coverage, localization, and signing evidence without accepting raw payload bytes or
//! claiming that a signed lineage proves a scientific conclusion.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P18-F26";
pub const CONTRACT_VERSION: &str = "multimodal-provenance-signing-assurance/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ARTIFACTS: usize = 8192;
const MAX_STEPS: usize = 8192;
const MAX_ITEMS: usize = 16384;

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
    pub comparable: bool,
    pub policy_allow: bool,
    pub verdict: ProvenanceAssuranceVerdict,
    pub lineage_order: Vec<String>,
    pub lineage_provenance: Vec<ProvenanceLink>,
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
            || self.lineage_order.is_empty()
            || self.lineage_provenance.is_empty()
            || self.derivation_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipt.trim().is_empty()
        {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "provenance identity, lineage, derivations, reasons, locality, effects, and boundary are required".into(),
            ));
        }
        validate_text("envelope_id", &self.envelope_id)?;
        validate_text("root_artifact_id", &self.root_artifact_id)?;
        validate_text("boundary", &self.boundary)?;
        if self.root_digest == ContentHash::of_bytes(b"") {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "root digest is required".into(),
            ));
        }
        validate_sorted_strings("lineage_order", &self.lineage_order)?;
        validate_lineage_provenance(&self.lineage_order, &self.lineage_provenance)?;
        validate_unique_strings("derivation_order", &self.derivation_order)?;
        validate_sorted_strings("study_order", &self.study_order)?;
        validate_sorted_strings("modality_order", &self.modality_order)?;
        if self.tool_order.len() > MAX_STEPS
            || self.tool_order.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "provenance tool ordering is not canonical".into(),
            ));
        }
        if !self.lineage_order.contains(&self.root_artifact_id) {
            return Err(ProvenanceAssuranceError::InvalidReference(
                "root artifact is absent from lineage".into(),
            ));
        }
        validate_optional_hex("signer_public_key_hex", &self.signer_public_key_hex, 64)?;
        validate_optional_hex("signer_signature_hex", &self.signer_signature_hex, 128)?;
        let has_multimodal = self
            .modality_order
            .iter()
            .any(|modality| modality == "imaging")
            && self
                .modality_order
                .iter()
                .any(|modality| modality == "omics");
        let signature_present =
            !self.signer_public_key_hex.is_empty() && !self.signer_signature_hex.is_empty();
        let expected_verdict = if !self.policy_allow {
            ProvenanceAssuranceVerdict::Blocked
        } else if !signature_present {
            ProvenanceAssuranceVerdict::Unresolved
        } else if !self.comparable
            || !has_multimodal
            || !self.omissions.is_empty()
            || !self.negative_evidence.is_empty()
        {
            ProvenanceAssuranceVerdict::Conditional
        } else {
            ProvenanceAssuranceVerdict::Signed
        };
        if self.verdict != expected_verdict {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "provenance verdict is inconsistent with its evidence and policy state".into(),
            ));
        }
        let expected_effect = if self.verdict == ProvenanceAssuranceVerdict::Signed {
            "write_signed_provenance_envelope_local_only"
        } else {
            "block_unsafe_release_and_retain_provenance_receipt"
        };
        if self.effect_receipt != expected_effect {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "provenance effect does not match verdict".into(),
            ));
        }
        if self.verdict == ProvenanceAssuranceVerdict::Signed
            && (!self.omissions.is_empty()
                || !self.uncertainty.is_empty()
                || !self.negative_evidence.is_empty()
                || !self.semantic_loss.is_empty())
        {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "signed provenance cannot contain unresolved loss or negative evidence".into(),
            ));
        }
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        for loss in &self.semantic_loss {
            validate_text("semantic_loss.field", &loss.field)?;
            validate_text("semantic_loss.reason", &loss.reason)?;
        }
        if self.semantic_loss.windows(2).any(|pair| {
            (
                pair[0].field.as_str(),
                pair[0].reason.as_str(),
                pair[0].severity,
            ) >= (
                pair[1].field.as_str(),
                pair[1].reason.as_str(),
                pair[1].severity,
            )
        }) {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "provenance semantic loss ordering is not canonical".into(),
            ));
        }
        if self.artifact.artifact_id != format!("provenance-envelope:{}", self.envelope_id)
            || self.artifact.content_type
                != "application/vnd.aurora.signed-provenance-envelope+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != self.lineage_provenance
        {
            return Err(ProvenanceAssuranceError::Contract(
                "provenance artifact is not bound to the envelope".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ProvenanceAssuranceError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&provenance_payload(self))
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

fn validate_text(field: &str, value: &str) -> Result<(), ProvenanceAssuranceError> {
    if value.is_empty() || value.trim() != value {
        return Err(ProvenanceAssuranceError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ProvenanceAssuranceError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), ProvenanceAssuranceError> {
    if values.len() > MAX_ITEMS {
        return Err(ProvenanceAssuranceError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ProvenanceAssuranceError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), ProvenanceAssuranceError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ProvenanceAssuranceError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_lineage_provenance(
    lineage_order: &[String],
    links: &[ProvenanceLink],
) -> Result<(), ProvenanceAssuranceError> {
    if links.len() != lineage_order.len() || links.len() > MAX_ITEMS {
        return Err(ProvenanceAssuranceError::InvalidRequest(
            "lineage provenance must cover each lineage artifact exactly once".into(),
        ));
    }
    let lineage_ids = lineage_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut linked_ids = BTreeSet::new();
    for link in links {
        validate_text("lineage_provenance.source_id", &link.source_id)?;
        validate_text("lineage_provenance.relation", &link.relation)?;
        if !matches!(
            link.relation.as_str(),
            "source-artifact" | "derivation-output"
        ) {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "lineage provenance relation is outside the adapter contract".into(),
            ));
        }
        if link.digest == ContentHash::of_bytes(b"") {
            return Err(ProvenanceAssuranceError::InvalidRequest(
                "lineage provenance digest is required".into(),
            ));
        }
        if !lineage_ids.contains(&link.source_id) || !linked_ids.insert(link.source_id.clone()) {
            return Err(ProvenanceAssuranceError::InvalidReference(
                "lineage provenance contains an unknown or duplicate artifact".into(),
            ));
        }
    }
    if linked_ids != lineage_ids
        || links.windows(2).any(|pair| {
            (pair[0].source_id.as_str(), pair[0].relation.as_str())
                >= (pair[1].source_id.as_str(), pair[1].relation.as_str())
        })
    {
        return Err(ProvenanceAssuranceError::InvalidRequest(
            "lineage provenance ordering or coverage is not canonical".into(),
        ));
    }
    Ok(())
}

fn validate_optional_hex(
    field: &str,
    value: &str,
    expected_hex_chars: usize,
) -> Result<(), ProvenanceAssuranceError> {
    if value.is_empty() {
        return Ok(());
    }
    if value.len() != expected_hex_chars
        || !value.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err(ProvenanceAssuranceError::InvalidRequest(format!(
            "{field} is not canonical hexadecimal evidence"
        )));
    }
    Ok(())
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
    let lineage_provenance = build_lineage_provenance(&artifacts, &steps);
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
    let mut negative_evidence = input.negative_evidence.clone();
    let has_multimodal = modality_order.iter().any(|modality| modality == "imaging")
        && modality_order.iter().any(|modality| modality == "omics");
    let signature_present = !input.signer_public_key_hex.trim().is_empty()
        && !input.signer_signature_hex.trim().is_empty();
    let verdict = if !input.policy_allow {
        ProvenanceAssuranceVerdict::Blocked
    } else if !signature_present {
        ProvenanceAssuranceVerdict::Unresolved
    } else if !input.comparable
        || !has_multimodal
        || !omissions.is_empty()
        || !negative_evidence.is_empty()
    {
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
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    reasons.sort();
    reasons.dedup();
    semantic_loss.sort_by(|left, right| left.field.cmp(&right.field));
    let effect_receipt = if verdict == ProvenanceAssuranceVerdict::Signed {
        "write_signed_provenance_envelope_local_only"
    } else {
        "block_unsafe_release_and_retain_provenance_receipt"
    };
    let payload = provenance_payload_from_parts(
        &input.envelope_id,
        &input.root_artifact_id,
        &input.root_digest,
        input.comparable,
        input.policy_allow,
        verdict,
        &lineage_order,
        &lineage_provenance,
        &derivation_order,
        &study_order,
        &modality_order,
        &tool_order,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &semantic_loss,
        &reasons,
        &input.signer_public_key_hex,
        &input.signer_signature_hex,
        effect_receipt,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("provenance-envelope:{}", input.envelope_id),
        "application/vnd.aurora.signed-provenance-envelope+json",
        &payload,
        semantic_loss.clone(),
        lineage_provenance.clone(),
    )
    .map_err(|error| ProvenanceAssuranceError::Contract(error.to_string()))?;
    let envelope = SignedProvenanceEnvelope {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        envelope_id: input.envelope_id.clone(),
        root_artifact_id: input.root_artifact_id.clone(),
        root_digest: input.root_digest.clone(),
        comparable: input.comparable,
        policy_allow: input.policy_allow,
        verdict,
        lineage_order,
        lineage_provenance,
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

fn build_lineage_provenance(
    artifacts: &[ProvenanceArtifact],
    steps: &[DerivationStep],
) -> Vec<ProvenanceLink> {
    let mut links = artifacts
        .iter()
        .map(|artifact| ProvenanceLink {
            source_id: artifact.artifact_id.clone(),
            relation: "source-artifact".into(),
            digest: artifact.content_digest.clone(),
        })
        .chain(steps.iter().map(|step| ProvenanceLink {
            source_id: step.output_artifact_id.clone(),
            relation: "derivation-output".into(),
            digest: step.output_digest.clone(),
        }))
        .collect::<Vec<_>>();
    links.sort_by(|left, right| {
        (left.source_id.as_str(), left.relation.as_str())
            .cmp(&(right.source_id.as_str(), right.relation.as_str()))
    });
    links
}

fn provenance_payload(envelope: &SignedProvenanceEnvelope) -> serde_json::Value {
    provenance_payload_from_parts(
        &envelope.envelope_id,
        &envelope.root_artifact_id,
        &envelope.root_digest,
        envelope.comparable,
        envelope.policy_allow,
        envelope.verdict,
        &envelope.lineage_order,
        &envelope.lineage_provenance,
        &envelope.derivation_order,
        &envelope.study_order,
        &envelope.modality_order,
        &envelope.tool_order,
        &envelope.omissions,
        &envelope.uncertainty,
        &envelope.negative_evidence,
        &envelope.semantic_loss,
        &envelope.reasons,
        &envelope.signer_public_key_hex,
        &envelope.signer_signature_hex,
        &envelope.effect_receipt,
        envelope.raw_data_local,
        &envelope.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn provenance_payload_from_parts(
    envelope_id: &str,
    root_artifact_id: &str,
    root_digest: &ContentHash,
    comparable: bool,
    policy_allow: bool,
    verdict: ProvenanceAssuranceVerdict,
    lineage_order: &[String],
    lineage_provenance: &[ProvenanceLink],
    derivation_order: &[String],
    study_order: &[String],
    modality_order: &[String],
    tool_order: &[ContentHash],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    signer_public_key_hex: &str,
    signer_signature_hex: &str,
    effect_receipt: &str,
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "envelope_id": envelope_id,
        "root_artifact_id": root_artifact_id,
        "root_digest": root_digest,
        "comparable": comparable,
        "policy_allow": policy_allow,
        "verdict": verdict,
        "lineage_order": lineage_order,
        "lineage_provenance": lineage_provenance,
        "derivation_order": derivation_order,
        "study_order": study_order,
        "modality_order": modality_order,
        "tool_order": tool_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "signer_public_key_hex": signer_public_key_hex,
        "signer_signature_hex": signer_signature_hex,
        "effect_receipt": effect_receipt,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

fn validate_input(input: &ArtifactAndDerivation) -> Result<(), ProvenanceAssuranceError> {
    if input.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || input.envelope_id.trim().is_empty()
        || input.root_artifact_id.trim().is_empty()
        || input.source_artifacts.is_empty()
        || input.derivation_steps.is_empty()
        || input.source_artifacts.len() > MAX_ARTIFACTS
        || input.derivation_steps.len() > MAX_STEPS
        || !input.raw_data_local
        || input.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ProvenanceAssuranceError::InvalidRequest(
            "provenance identity, artifacts, derivations, locality, and boundary are required"
                .into(),
        ));
    }
    validate_text("envelope_id", &input.envelope_id)?;
    validate_text("root_artifact_id", &input.root_artifact_id)?;
    if input.root_digest == ContentHash::of_bytes(b"") {
        return Err(ProvenanceAssuranceError::InvalidRequest(
            "root digest is required".into(),
        ));
    }
    validate_optional_hex("signer_public_key_hex", &input.signer_public_key_hex, 64)?;
    validate_optional_hex("signer_signature_hex", &input.signer_signature_hex, 128)?;
    validate_unique_strings("protected_omissions", &input.protected_omissions)?;
    validate_unique_strings("negative_evidence", &input.negative_evidence)?;
    let mut ids = BTreeSet::new();
    for artifact in &input.source_artifacts {
        validate_text("artifact_id", &artifact.artifact_id)?;
        validate_text("study_id", &artifact.study_id)?;
        validate_text("modality", &artifact.modality)?;
        if !artifact.raw_data_local || artifact.content_digest == ContentHash::of_bytes(b"") {
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
        validate_text("step_id", &step.step_id)?;
        validate_text("operation", &step.operation)?;
        validate_text("output_artifact_id", &step.output_artifact_id)?;
        if step.input_artifact_ids.is_empty() || !step.deterministic {
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
        validate_unique_strings("input_artifact_ids", &step.input_artifact_ids)?;
        if step.output_digest == ContentHash::of_bytes(b"")
            || step.tool_digest == ContentHash::of_bytes(b"")
        {
            return Err(ProvenanceAssuranceError::InvalidRequest(format!(
                "derivation step {} lacks content-addressed digests",
                step.step_id
            )));
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
            let Some(index) = remaining.iter().position(|step| step.step_id == step_id) else {
                return Err(ProvenanceAssuranceError::Cycle(step_id));
            };
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

    #[test]
    fn topological_order_is_not_confused_with_lexical_order() {
        let mut value = input();
        let intermediate = ContentHash::of_bytes(b"intermediate");
        let root_digest = value.root_digest.clone();
        value.derivation_steps = vec![
            DerivationStep {
                step_id: "step:z".into(),
                operation: "prepare".into(),
                input_artifact_ids: vec!["artifact:imaging".into()],
                output_artifact_id: "artifact:intermediate".into(),
                output_digest: intermediate.clone(),
                tool_digest: ContentHash::of_bytes(b"tool-z"),
                deterministic: true,
            },
            DerivationStep {
                step_id: "step:a".into(),
                operation: "harmonize".into(),
                input_artifact_ids: vec!["artifact:intermediate".into(), "artifact:omics".into()],
                output_artifact_id: "artifact:root".into(),
                output_digest: root_digest,
                tool_digest: ContentHash::of_bytes(b"tool-a"),
                deterministic: true,
            },
        ];
        let envelope = assure_provenance(&value).unwrap();
        assert_eq!(envelope.derivation_order, vec!["step:z", "step:a"]);
    }

    #[test]
    fn invalid_signer_encoding_is_rejected() {
        let mut value = input();
        value.signer_public_key_hex = "not-hex".into();
        assert!(assure_provenance(&value).is_err());
    }

    #[test]
    fn forged_provenance_effect_is_rejected() {
        let mut envelope = assure_provenance(&input()).unwrap();
        envelope.effect_receipt = "block_unsafe_release_and_retain_provenance_receipt".into();
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn root_must_remain_in_lineage() {
        let mut envelope = assure_provenance(&input()).unwrap();
        envelope.lineage_order.retain(|id| id != "artifact:root");
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn envelope_payload_digest_is_verified() {
        let mut envelope = assure_provenance(&input()).unwrap();
        envelope.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn lineage_digest_is_bound_to_the_artifact() {
        let mut envelope = assure_provenance(&input()).unwrap();
        envelope.lineage_provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(envelope.validate().is_err());
    }

    #[test]
    fn policy_state_is_bound_to_the_verdict() {
        let mut envelope = assure_provenance(&input()).unwrap();
        envelope.policy_allow = false;
        assert!(envelope.validate().is_err());
    }
}
