//! Evidence-backed interpretation assurance for prospective multimodal research results.
//!
//! Atlas feature: `AFA-adapter-P14-F27`.
//!
//! This product verifies that interpretation claims name supporting local artifacts, required
//! modality views, uncertainty, and negative evidence before an interactive result is released.
//! It creates no plot, model, or scientific conclusion: unsupported claims become blocked and
//! protected omissions can only lower the verdict.

use bioprism_foundation::{
    LossSeverity, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P14-F27";
pub const CONTRACT_VERSION: &str = "interpretation-assurance/1.0";

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
    pub result_id: String,
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
        if self.claim_order.windows(2).any(|pair| pair[0] > pair[1])
            || self.claim_order.iter().collect::<BTreeSet<_>>().len() != self.claim_order.len()
        {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "claim order must be canonical and unique".into(),
            ));
        }
        if self.verdict == InterpretationVerdict::Qualified && !self.omitted_modalities.is_empty() {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "qualified interpretation cannot omit a required modality".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationAssuranceError::Contract(error.to_string()))?;
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
    validate_result(result)?;
    let mut claims = result.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let evidence = result.evidence_digests.iter().collect::<BTreeSet<_>>();
    let mut omissions = result.protected_omissions.clone();
    let mut covered = claims
        .iter()
        .map(|claim| claim.modality.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    covered.sort();
    let mut required = result.required_modalities.clone();
    required.sort();
    required.dedup();
    let omitted_modalities = required
        .iter()
        .filter(|modality| !covered.contains(modality))
        .cloned()
        .collect::<Vec<_>>();
    omissions.extend(
        omitted_modalities
            .iter()
            .map(|modality| format!("required interpretation modality unavailable: {modality}")),
    );
    let uncertainty = claims
        .iter()
        .map(|claim| format!("{}: {}", claim.claim_id, claim.uncertainty))
        .collect::<Vec<_>>();
    let negative_evidence = claims
        .iter()
        .flat_map(|claim| {
            claim
                .negative_evidence
                .iter()
                .map(move |item| format!("{}: {}", claim.claim_id, item))
        })
        .collect::<Vec<_>>();
    let verdict = if claims.iter().any(|claim| {
        claim
            .supporting_evidence
            .iter()
            .any(|digest| !evidence.contains(digest))
    }) {
        InterpretationVerdict::Blocked
    } else if !omissions.is_empty() {
        InterpretationVerdict::Conditional
    } else {
        InterpretationVerdict::Qualified
    };
    let mut reasons = vec![format!(
        "{} interpretation claims checked against local evidence and required modality views",
        claims.len()
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
    if verdict == InterpretationVerdict::Blocked {
        reasons.push("at least one claim references evidence absent from the local result".into());
        semantic_loss.push(SemanticLoss {
            field: "supporting_evidence".into(),
            reason: "unsupported interpretation claims are blocked".into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "result_id": result.result_id, "verdict": verdict, "claim_order": claims.iter().map(|claim| claim.claim_id.clone()).collect::<Vec<_>>(), "covered_modalities": covered, "omitted_modalities": omitted_modalities, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "semantic_loss": semantic_loss, "reasons": reasons, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("interpretation-assurance:{}", result.result_id),
        "application/vnd.aurora.interpretation-assurance+json",
        &payload,
        semantic_loss.clone(),
        Vec::new(),
    )
    .map_err(|error| InterpretationAssuranceError::Contract(error.to_string()))?;
    let receipt = InterpretationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        result_id: result.result_id.clone(),
        verdict,
        claim_order: claims.iter().map(|claim| claim.claim_id.clone()).collect(),
        covered_modalities: covered,
        omitted_modalities,
        uncertainty,
        negative_evidence,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
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
    let evidence = result.evidence_digests.iter().collect::<BTreeSet<_>>();
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
        if !ids.insert(claim.claim_id.clone()) {
            return Err(InterpretationAssuranceError::DuplicateClaim(
                claim.claim_id.clone(),
            ));
        }
        if claim
            .supporting_evidence
            .iter()
            .any(|digest| !evidence.contains(digest))
        {
            return Err(InterpretationAssuranceError::MissingEvidence(
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
        assert!(assure_interpretation(&result).is_err());
    }
    #[test]
    fn duplicate_claim_is_rejected() {
        let mut result = result();
        result.claims.push(result.claims[0].clone());
        assert!(assure_interpretation(&result).is_err());
    }
}
