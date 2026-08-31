//! Evidence-backed interpretation assurance for prospective multimodal research results.
//!
//! Atlas feature: `AFA-adapter-P14-F27`.
//!
//! This product verifies that interpretation claims name supporting local artifacts, required
//! modality views, uncertainty, and negative evidence before an interactive result is released.
//! It creates no plot, model, or scientific conclusion: unsupported claims become blocked and
//! protected omissions can only lower the verdict.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P14-F27";
pub const CONTRACT_VERSION: &str = "interpretation-assurance/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationClaim {
    pub claim_id: String,
    pub modality: String,
    pub statement: String,
    pub supporting_evidence: Vec<ContentHash>,
    pub uncertainty: String,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceBackedResult {
    pub result_id: String,
    pub evidence_digests: Vec<ContentHash>,
    pub required_modalities: Vec<String>,
    pub claims: Vec<InterpretationClaim>,
    pub protected_omissions: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationVerdict {
    Qualified,
    Conditional,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: EvidenceBackedResult,
    pub input_digest: ContentHash,
    pub result_id: String,
    pub evidence_digests: Vec<ContentHash>,
    pub required_modalities: Vec<String>,
    pub claims: Vec<InterpretationClaim>,
    pub protected_omissions: Vec<String>,
    pub verdict: InterpretationVerdict,
    pub claim_order: Vec<String>,
    pub covered_modalities: Vec<String>,
    pub omitted_modalities: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl InterpretationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), InterpretationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(InterpretationAssuranceError::Contract(
                "interpretation assurance identity mismatch".into(),
            ));
        }
        if self.result_id.trim().is_empty()
            || self.evidence_digests.is_empty()
            || self.required_modalities.is_empty()
            || self.claims.is_empty()
            || self.claim_order.is_empty()
            || self.reasons.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "interpretation identity, claims, reasons, locality, and boundary are required"
                    .into(),
            ));
        }
        validate_text("result_id", &self.result_id)?;
        validate_text("boundary", &self.boundary)?;
        if self
            .evidence_digests
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self
                .evidence_digests
                .iter()
                .any(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "evidence digests must be non-empty and strictly sorted".into(),
            ));
        }
        validate_sorted_strings("required_modalities", &self.required_modalities)?;
        validate_sorted_strings("protected_omissions", &self.protected_omissions)?;
        validate_sorted_strings("claim_order", &self.claim_order)?;
        validate_sorted_strings("covered_modalities", &self.covered_modalities)?;
        validate_sorted_strings("omitted_modalities", &self.omitted_modalities)?;
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
            return Err(InterpretationAssuranceError::InvalidRequest(
                "semantic-loss ordering is not canonical".into(),
            ));
        }
        if self.artifact.artifact_id != format!("interpretation-assurance:{}", self.result_id)
            || self.artifact.content_type != "application/vnd.aurora.interpretation-assurance+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != interpretation_provenance(&self.evidence_digests)
        {
            return Err(InterpretationAssuranceError::Contract(
                "interpretation artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationAssuranceError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&interpretation_payload(self))
            .map_err(|error| InterpretationAssuranceError::Contract(error.to_string()))?;
        validate_result(&self.input)?;
        if self.input_digest != interpretation_input_digest(&self.input)? {
            return Err(InterpretationAssuranceError::Contract(
                "interpretation retained input digest does not match the result".into(),
            ));
        }
        let derived = derive_interpretation(&canonical_result(&self.input));
        if self.verdict != derived.verdict
            || self.claim_order != derived.claim_order
            || self.covered_modalities != derived.covered_modalities
            || self.omitted_modalities != derived.omitted_modalities
            || self.uncertainty != derived.uncertainty
            || self.negative_evidence != derived.negative_evidence
            || self.semantic_loss != derived.semantic_loss
            || self.reasons != derived.reasons
        {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "interpretation verdict is not derived from retained claim evidence".into(),
            ));
        }
        if self.verdict == InterpretationVerdict::Qualified && !self.omitted_modalities.is_empty() {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "qualified interpretation cannot omit a required modality".into(),
            ));
        }
        if self.verdict == InterpretationVerdict::Qualified && !self.semantic_loss.is_empty() {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "qualified interpretation cannot retain decision-relevant semantic loss".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, InterpretationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), InterpretationAssuranceError> {
    if value.is_empty() || value.trim() != value {
        return Err(InterpretationAssuranceError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(InterpretationAssuranceError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn interpretation_input_digest(
    result: &EvidenceBackedResult,
) -> Result<ContentHash, InterpretationAssuranceError> {
    let value = serde_json::to_value(&canonical_result(result))
        .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), InterpretationAssuranceError> {
    if values.len() > MAX_ITEMS {
        return Err(InterpretationAssuranceError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(InterpretationAssuranceError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), InterpretationAssuranceError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(InterpretationAssuranceError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DerivedInterpretation {
    verdict: InterpretationVerdict,
    claim_order: Vec<String>,
    covered_modalities: Vec<String>,
    omitted_modalities: Vec<String>,
    uncertainty: Vec<String>,
    negative_evidence: Vec<String>,
    semantic_loss: Vec<SemanticLoss>,
    reasons: Vec<String>,
}

fn canonical_result(result: &EvidenceBackedResult) -> EvidenceBackedResult {
    let mut canonical = result.clone();
    canonical.evidence_digests.sort();
    canonical.required_modalities.sort();
    canonical.protected_omissions.sort();
    canonical
        .claims
        .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    for claim in &mut canonical.claims {
        claim.supporting_evidence.sort();
        claim.negative_evidence.sort();
    }
    canonical
}

fn interpretation_provenance(evidence_digests: &[ContentHash]) -> Vec<ProvenanceLink> {
    evidence_digests
        .iter()
        .map(|digest| ProvenanceLink {
            source_id: format!("evidence:{digest}"),
            relation: "interpretation-evidence-digest".into(),
            digest: digest.clone(),
        })
        .collect()
}

fn derive_interpretation(result: &EvidenceBackedResult) -> DerivedInterpretation {
    let evidence = result.evidence_digests.iter().collect::<BTreeSet<_>>();
    let claim_order = result
        .claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let covered_modalities = result
        .claims
        .iter()
        .map(|claim| claim.modality.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let omitted_modalities = result
        .required_modalities
        .iter()
        .filter(|modality| !covered_modalities.contains(modality))
        .cloned()
        .collect::<Vec<_>>();
    let mut omissions = result.protected_omissions.clone();
    omissions.extend(
        omitted_modalities
            .iter()
            .map(|modality| format!("required interpretation modality unavailable: {modality}")),
    );
    omissions.sort();
    omissions.dedup();
    let uncertainty = result
        .claims
        .iter()
        .map(|claim| format!("{}: {}", claim.claim_id, claim.uncertainty))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let negative_evidence = result
        .claims
        .iter()
        .flat_map(|claim| {
            claim
                .negative_evidence
                .iter()
                .map(move |item| format!("{}: {}", claim.claim_id, item))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let blocked = result.claims.iter().any(|claim| {
        claim
            .supporting_evidence
            .iter()
            .any(|digest| !evidence.contains(digest))
    });
    let verdict = if blocked {
        InterpretationVerdict::Blocked
    } else if !omissions.is_empty() {
        InterpretationVerdict::Conditional
    } else {
        InterpretationVerdict::Qualified
    };
    let mut reasons = vec![format!(
        "{} interpretation claims checked against local evidence and required modality views",
        result.claims.len()
    )];
    let mut semantic_loss = Vec::new();
    if !omitted_modalities.is_empty() {
        reasons.push("required modality coverage is incomplete".into());
        semantic_loss.push(SemanticLoss {
            field: "required_modalities".into(),
            reason: "visual interpretation cannot claim coverage for an unavailable modality"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    if !negative_evidence.is_empty() {
        reasons.push(
            "negative evidence is retained and visible to downstream publication gates".into(),
        );
    }
    if blocked {
        reasons.push("at least one claim references evidence absent from the local result".into());
        semantic_loss.push(SemanticLoss {
            field: "supporting_evidence".into(),
            reason: "unsupported interpretation claims are blocked".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    semantic_loss.sort_by(|left, right| {
        (left.field.as_str(), left.reason.as_str(), left.severity).cmp(&(
            right.field.as_str(),
            right.reason.as_str(),
            right.severity,
        ))
    });
    reasons.sort();
    reasons.dedup();
    DerivedInterpretation {
        verdict,
        claim_order,
        covered_modalities,
        omitted_modalities,
        uncertainty,
        negative_evidence,
        semantic_loss,
        reasons,
    }
}

fn interpretation_payload(receipt: &InterpretationAssuranceReceipt) -> serde_json::Value {
    interpretation_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.result_id,
        &receipt.evidence_digests,
        &receipt.required_modalities,
        &receipt.claims,
        &receipt.protected_omissions,
        receipt.verdict,
        &receipt.claim_order,
        &receipt.covered_modalities,
        &receipt.omitted_modalities,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.semantic_loss,
        &receipt.reasons,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn interpretation_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    result_id: &str,
    evidence_digests: &[ContentHash],
    required_modalities: &[String],
    claims: &[InterpretationClaim],
    protected_omissions: &[String],
    verdict: InterpretationVerdict,
    claim_order: &[String],
    covered_modalities: &[String],
    omitted_modalities: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "result_id": result_id,
        "evidence_digests": evidence_digests,
        "required_modalities": required_modalities,
        "claims": claims,
        "protected_omissions": protected_omissions,
        "verdict": verdict,
        "claim_order": claim_order,
        "covered_modalities": covered_modalities,
        "omitted_modalities": omitted_modalities,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

#[derive(Debug, Error)]
pub enum InterpretationAssuranceError {
    #[error("invalid interpretation assurance request: {0}")]
    InvalidRequest(String),
    #[error("interpretation assurance contract rejected: {0}")]
    Contract(String),
    #[error("duplicate interpretation claim {0}")]
    DuplicateClaim(String),
    #[error("interpretation claim lacks evidence coverage {0}")]
    MissingEvidence(String),
    #[error("interpretation assurance serialization failed: {0}")]
    Serialization(String),
}

pub fn assure_interpretation(
    result: &EvidenceBackedResult,
) -> Result<InterpretationAssuranceReceipt, InterpretationAssuranceError> {
    let receipt = build_interpretation(result)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_interpretation(
    result: &EvidenceBackedResult,
) -> Result<InterpretationAssuranceReceipt, InterpretationAssuranceError> {
    validate_result(result)?;
    let canonical = canonical_result(result);
    let derived = derive_interpretation(&canonical);
    let provenance = interpretation_provenance(&canonical.evidence_digests);
    let payload = interpretation_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &canonical.result_id,
        &canonical.evidence_digests,
        &canonical.required_modalities,
        &canonical.claims,
        &canonical.protected_omissions,
        derived.verdict,
        &derived.claim_order,
        &derived.covered_modalities,
        &derived.omitted_modalities,
        &derived.uncertainty,
        &derived.negative_evidence,
        &derived.semantic_loss,
        &derived.reasons,
        &provenance,
        canonical.raw_data_local,
        &canonical.boundary,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("interpretation-assurance:{}", canonical.result_id),
        "application/vnd.aurora.interpretation-assurance+json",
        &payload,
        derived.semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| InterpretationAssuranceError::Contract(error.to_string()))?;
    let receipt = InterpretationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical.clone(),
        input_digest: interpretation_input_digest(result)?,
        result_id: canonical.result_id,
        evidence_digests: canonical.evidence_digests,
        required_modalities: canonical.required_modalities,
        claims: canonical.claims,
        protected_omissions: canonical.protected_omissions,
        verdict: derived.verdict,
        claim_order: derived.claim_order,
        covered_modalities: derived.covered_modalities,
        omitted_modalities: derived.omitted_modalities,
        uncertainty: derived.uncertainty,
        negative_evidence: derived.negative_evidence,
        semantic_loss: derived.semantic_loss,
        reasons: derived.reasons,
        artifact,
        raw_data_local: canonical.raw_data_local,
        boundary: canonical.boundary,
    };
    Ok(receipt)
}

fn validate_result(result: &EvidenceBackedResult) -> Result<(), InterpretationAssuranceError> {
    if result.result_id.trim().is_empty()
        || result.evidence_digests.is_empty()
        || result.required_modalities.is_empty()
        || result.claims.is_empty()
        || !result.raw_data_local
        || result.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationAssuranceError::InvalidRequest("result identity, evidence, required modalities, claims, locality, and boundary are required".into()));
    }
    validate_text("result_id", &result.result_id)?;
    validate_text("boundary", &result.boundary)?;
    if result.evidence_digests.len() > MAX_ITEMS
        || result.required_modalities.len() > MAX_ITEMS
        || result.claims.len() > MAX_ITEMS
        || result.protected_omissions.len() > MAX_ITEMS
    {
        return Err(InterpretationAssuranceError::InvalidRequest(
            "interpretation evidence, modality, claim, or omission count exceeds its bound".into(),
        ));
    }
    let mut evidence = BTreeSet::new();
    for digest in &result.evidence_digests {
        if *digest == ContentHash::of_bytes(b"") || !evidence.insert(digest) {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "evidence digests must be non-empty and unique".into(),
            ));
        }
    }
    validate_unique_strings("required_modalities", &result.required_modalities)?;
    validate_unique_strings("protected_omissions", &result.protected_omissions)?;
    let mut ids = BTreeSet::new();
    for claim in &result.claims {
        if claim.claim_id.trim().is_empty()
            || claim.modality.trim().is_empty()
            || claim.statement.trim().is_empty()
            || claim.supporting_evidence.is_empty()
            || claim.uncertainty.trim().is_empty()
        {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "claim identity, modality, statement, evidence, and uncertainty are required"
                    .into(),
            ));
        }
        validate_text("claim_id", &claim.claim_id)?;
        validate_text("claim.modality", &claim.modality)?;
        validate_text("claim.statement", &claim.statement)?;
        validate_text("claim.uncertainty", &claim.uncertainty)?;
        validate_unique_strings("claim.negative_evidence", &claim.negative_evidence)?;
        if claim.supporting_evidence.len() > MAX_ITEMS {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "claim evidence count exceeds its bound".into(),
            ));
        }
        let mut supporting = BTreeSet::new();
        for digest in &claim.supporting_evidence {
            if *digest == ContentHash::of_bytes(b"") || !supporting.insert(digest) {
                return Err(InterpretationAssuranceError::InvalidRequest(format!(
                    "claim {} has duplicate or empty supporting evidence",
                    claim.claim_id
                )));
            }
        }
        if !ids.insert(claim.claim_id.clone()) {
            return Err(InterpretationAssuranceError::DuplicateClaim(
                claim.claim_id.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn result() -> EvidenceBackedResult {
        let digest = ContentHash::of_bytes(b"result");
        EvidenceBackedResult {
            result_id: "result:interpretation".into(),
            evidence_digests: vec![digest.clone()],
            required_modalities: vec!["imaging".into(), "omics".into()],
            claims: vec![InterpretationClaim {
                claim_id: "claim:a".into(),
                modality: "imaging".into(),
                statement: "signal is bounded".into(),
                supporting_evidence: vec![digest],
                uncertainty: "measurement uncertainty remains".into(),
                negative_evidence: vec!["null replicate is absent".into()],
            }],
            protected_omissions: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn missing_view_is_conditional_and_negative_evidence_survives() {
        let receipt = assure_interpretation(&result()).unwrap();
        assert_eq!(receipt.verdict, InterpretationVerdict::Conditional);
        assert!(!receipt.negative_evidence.is_empty());
    }
    #[test]
    fn claim_order_is_replayable() {
        let first = assure_interpretation(&result()).unwrap();
        let second = assure_interpretation(&result()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
    #[test]
    fn unsupported_claim_is_blocked() {
        let mut result = result();
        result.claims[0]
            .supporting_evidence
            .push(ContentHash::of_bytes(b"missing"));
        let receipt = assure_interpretation(&result).unwrap();
        assert_eq!(receipt.verdict, InterpretationVerdict::Blocked);
        assert!(receipt
            .semantic_loss
            .iter()
            .any(|loss| loss.field == "supporting_evidence"));
    }
    #[test]
    fn duplicate_claim_is_rejected() {
        let mut result = result();
        result.claims.push(result.claims[0].clone());
        assert!(assure_interpretation(&result).is_err());
    }

    #[test]
    fn duplicate_required_modality_is_rejected() {
        let mut result = result();
        result.required_modalities.push("imaging".into());
        assert!(assure_interpretation(&result).is_err());
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload_binding() {
        let mut receipt = assure_interpretation(&result()).unwrap();
        receipt.negative_evidence[0] = "tampered-negative".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn protected_omission_order_does_not_change_digest() {
        let mut first = result();
        first.protected_omissions = vec!["z-omission".into(), "a-omission".into()];
        let mut second = first.clone();
        second.protected_omissions.reverse();
        assert_eq!(
            assure_interpretation(&first).unwrap().digest().unwrap(),
            assure_interpretation(&second).unwrap().digest().unwrap()
        );
    }

    #[test]
    fn retained_claim_tampering_is_rejected() {
        let mut receipt = assure_interpretation(&result()).unwrap();
        receipt.claims[0].statement = "forged statement".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn evidence_provenance_tampering_is_rejected() {
        let mut receipt = assure_interpretation(&result()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_result_tampering_is_rejected() {
        let mut receipt = assure_interpretation(&result()).unwrap();
        receipt.input.result_id = "result:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn evidence_order_is_canonicalized() {
        let digest_a = ContentHash::of_bytes(b"a");
        let digest_b = ContentHash::of_bytes(b"b");
        let mut first = result();
        first.evidence_digests = vec![digest_a.clone(), digest_b.clone()];
        first.claims[0].supporting_evidence = vec![digest_b, digest_a];
        let mut second = first.clone();
        second.evidence_digests.reverse();
        second.claims[0].supporting_evidence.reverse();
        assert_eq!(
            assure_interpretation(&first).unwrap().digest().unwrap(),
            assure_interpretation(&second).unwrap().digest().unwrap()
        );
    }
}
