//! Multimodal research-object release assurance.
//!
//! Atlas feature: `AFA-adapter-P16-F26`.
//!
//! This product checks a caller-supplied release bundle before it can become a portable research
//! object. It does not move raw imaging or omics bytes and it never treats a missing modality,
//! incomplete provenance, contradictory evidence, or an unapproved policy as a successful
//! publication.

use bioprism_foundation::{
    LossSeverity, PolicyDecision, PolicyReceipt, ProvenanceLink, SemanticLoss,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P16-F26";
pub const CONTRACT_VERSION: &str = "multimodal-research-release-assurance/1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseStudyManifest {
    pub study_id: String,
    pub modality: String,
    pub protocol_digest: ContentHash,
    pub artifact_ids: Vec<String>,
    pub comparable: bool,
    pub uncertainty: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedResearchRun {
    pub schema_version: String,
    pub run_id: String,
    pub release_id: String,
    pub purpose: String,
    pub studies: Vec<ReleaseStudyManifest>,
    pub evidence_receipt_ids: Vec<String>,
    pub release_digest: ContentHash,
    pub policy: PolicyReceipt,
    pub provenance_complete: bool,
    pub raw_data_local: bool,
    pub localization_statement: String,
    pub signer_public_key_hex: String,
    pub signer_signature_hex: String,
    pub protected_omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseAssuranceVerdict {
    Released,
    Conditional,
    Incomplete,
    Incomparable,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub run_id: String,
    pub release_id: String,
    pub run: ValidatedResearchRun,
    pub verdict: ReleaseAssuranceVerdict,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<String>,
    pub evidence_receipt_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub policy_decision: PolicyDecision,
    pub effect_receipt: String,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ReleaseAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ReleaseAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ReleaseAssuranceError::Contract(
                "release assurance identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.run_id.trim().is_empty()
            || self.release_id.trim().is_empty()
            || self.study_order.is_empty()
            || self.evidence_receipt_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipt.trim().is_empty()
        {
            return Err(ReleaseAssuranceError::InvalidRequest(
                "release identity, studies, evidence, effects, locality, and boundary are required"
                    .into(),
            ));
        }
        if !canonical_unique(&self.study_order)
            || !canonical_unique(&self.modality_order)
            || !canonical_unique(&self.artifact_order)
            || !canonical_unique(&self.evidence_receipt_order)
        {
            return Err(ReleaseAssuranceError::InvalidRequest(
                "release orders must be canonical and unique".into(),
            ));
        }
        let expected_effect = if self.verdict == ReleaseAssuranceVerdict::Released {
            "write_signed_research_object_metadata_local_only"
        } else {
            "block_unsafe_release_and_retain_local_receipt"
        };
        if self.effect_receipt != expected_effect {
            return Err(ReleaseAssuranceError::InvalidRequest(
                "release effect does not match its verdict".into(),
            ));
        }
        if self.artifact.artifact_id != format!("release-assurance:{}", self.release_id)
            || self.artifact.content_type != "application/vnd.aurora.signed-research-object+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != release_provenance(&self.run)
        {
            return Err(ReleaseAssuranceError::Contract(
                "release artifact is not bound to the validated run".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ReleaseAssuranceError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&release_payload(self))
            .map_err(|error| ReleaseAssuranceError::Contract(error.to_string()))?;
        let expected = assure_release_internal(&self.run, false)?;
        if self != &expected {
            return Err(ReleaseAssuranceError::Contract(
                "release receipt is not derived from its validated run".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ReleaseAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ReleaseAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ReleaseAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum ReleaseAssuranceError {
    #[error("invalid release assurance request: {0}")]
    InvalidRequest(String),
    #[error("release assurance contract rejected: {0}")]
    Contract(String),
    #[error("duplicate release identifier {0}")]
    DuplicateIdentifier(String),
    #[error("release assurance serialization failed: {0}")]
    Serialization(String),
}

pub fn assure_release(
    run: &ValidatedResearchRun,
) -> Result<ReleaseAssuranceReceipt, ReleaseAssuranceError> {
    assure_release_internal(run, true)
}

fn assure_release_internal(
    run: &ValidatedResearchRun,
    validate_output: bool,
) -> Result<ReleaseAssuranceReceipt, ReleaseAssuranceError> {
    validate_run(run)?;
    let run = canonical_run(run);
    let studies = run.studies.clone();
    let study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let modality_order = studies
        .iter()
        .map(|study| study.modality.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let artifact_order = studies
        .iter()
        .flat_map(|study| study.artifact_ids.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut evidence_receipt_order = run.evidence_receipt_ids.clone();
    evidence_receipt_order.sort();
    let mut omissions = run.protected_omissions.clone();
    let mut uncertainty = studies
        .iter()
        .map(|study| format!("{}: {}", study.study_id, study.uncertainty))
        .collect::<Vec<_>>();
    let negative_evidence = run.negative_evidence.clone();
    let comparable = studies.iter().all(|study| study.comparable);
    let modalities_complete = modality_order.iter().any(|modality| modality == "imaging")
        && modality_order.iter().any(|modality| modality == "omics");
    let verdict = if run.policy.decision != PolicyDecision::Allow {
        ReleaseAssuranceVerdict::Blocked
    } else if !run.provenance_complete
        || run.signer_public_key_hex.trim().is_empty()
        || run.signer_signature_hex.trim().is_empty()
    {
        ReleaseAssuranceVerdict::Incomplete
    } else if !comparable {
        ReleaseAssuranceVerdict::Incomparable
    } else if !modalities_complete || !omissions.is_empty() || !negative_evidence.is_empty() {
        ReleaseAssuranceVerdict::Conditional
    } else {
        ReleaseAssuranceVerdict::Released
    };
    if !modalities_complete {
        omissions.push("required imaging and omics modality coverage is incomplete".into());
    }
    let mut reasons = vec![format!(
        "{} multimodal studies and {} portable artifacts evaluated in canonical order",
        studies.len(),
        artifact_order.len()
    )];
    let mut semantic_loss = Vec::new();
    if !omissions.is_empty() {
        reasons.push("protected omissions prevent an unconditional research-object release".into());
        semantic_loss.push(SemanticLoss {
            field: "omissions".into(),
            reason: "unobserved or incomplete release evidence cannot be inferred from metadata"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        });
    }
    if !negative_evidence.is_empty() {
        reasons.push("negative evidence remains attached to the release receipt".into());
        uncertainty
            .push("negative evidence is not a publication failure or a positive claim".into());
    }
    if !comparable {
        reasons.push("study protocol or modality comparability is not established".into());
    }
    if run.policy.decision != PolicyDecision::Allow {
        reasons.push("policy did not authorize release; no export effect is admitted".into());
    }
    let effect_receipt: String = if verdict == ReleaseAssuranceVerdict::Released {
        "write_signed_research_object_metadata_local_only".into()
    } else {
        "block_unsafe_release_and_retain_local_receipt".into()
    };
    let policy_decision = run.policy.decision;
    let provenance = release_provenance(&run);
    let payload = release_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &run,
        &verdict,
        &study_order,
        &modality_order,
        &artifact_order,
        &evidence_receipt_order,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &semantic_loss,
        &reasons,
        policy_decision,
        &effect_receipt,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("release-assurance:{}", run.release_id),
        "application/vnd.aurora.signed-research-object+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| ReleaseAssuranceError::Contract(error.to_string()))?;
    let receipt = ReleaseAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        run_id: run.run_id.clone(),
        release_id: run.release_id.clone(),
        run,
        verdict,
        study_order,
        modality_order,
        artifact_order,
        evidence_receipt_order,
        omissions,
        uncertainty,
        negative_evidence,
        semantic_loss,
        reasons,
        policy_decision,
        effect_receipt,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        receipt.validate()?;
    }
    Ok(receipt)
}

fn validate_run(run: &ValidatedResearchRun) -> Result<(), ReleaseAssuranceError> {
    if run.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || run.boundary != PRECLINICAL_BOUNDARY
        || run.run_id.trim().is_empty()
        || run.release_id.trim().is_empty()
        || run.purpose.trim().is_empty()
        || run.studies.len() < 2
        || run.evidence_receipt_ids.is_empty()
        || !run.raw_data_local
        || !run
            .localization_statement
            .to_ascii_lowercase()
            .contains("local")
    {
        return Err(ReleaseAssuranceError::InvalidRequest(
            "release run is incomplete, non-local, or lacks multimodal studies".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for study in &run.studies {
        if study.study_id.trim().is_empty()
            || study.modality.trim().is_empty()
            || study.artifact_ids.is_empty()
            || study.uncertainty.trim().is_empty()
        {
            return Err(ReleaseAssuranceError::InvalidRequest(
                "study identity, artifacts, modality, and uncertainty are required".into(),
            ));
        }
        if !ids.insert(study.study_id.clone()) {
            return Err(ReleaseAssuranceError::DuplicateIdentifier(
                study.study_id.clone(),
            ));
        }
    }
    let mut evidence = BTreeSet::new();
    for id in &run.evidence_receipt_ids {
        if id.trim().is_empty() || !evidence.insert(id.clone()) {
            return Err(ReleaseAssuranceError::DuplicateIdentifier(id.clone()));
        }
    }
    run.policy
        .validate()
        .map_err(|error| ReleaseAssuranceError::Contract(error.to_string()))
}

fn canonical_unique(values: &[String]) -> bool {
    !values.is_empty() && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn canonical_run(run: &ValidatedResearchRun) -> ValidatedResearchRun {
    let mut run = run.clone();
    run.studies
        .sort_by(|left, right| left.study_id.cmp(&right.study_id));
    for study in &mut run.studies {
        study.artifact_ids.sort();
    }
    run.evidence_receipt_ids.sort();
    run.policy.reasons.sort();
    run.policy.evaluated_artifacts.sort();
    run.protected_omissions.sort();
    run.protected_omissions.dedup();
    run.negative_evidence.sort();
    run.negative_evidence.dedup();
    run
}

fn release_provenance(run: &ValidatedResearchRun) -> Vec<ProvenanceLink> {
    let mut provenance = vec![ProvenanceLink {
        source_id: run.release_id.clone(),
        relation: "release-input-digest".into(),
        digest: run.release_digest.clone(),
    }];
    provenance.extend(run.studies.iter().map(|study| ProvenanceLink {
        source_id: study.study_id.clone(),
        relation: "release-study-protocol".into(),
        digest: study.protocol_digest.clone(),
    }));
    provenance
}

fn release_payload(receipt: &ReleaseAssuranceReceipt) -> serde_json::Value {
    release_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.run,
        &receipt.verdict,
        &receipt.study_order,
        &receipt.modality_order,
        &receipt.artifact_order,
        &receipt.evidence_receipt_order,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.semantic_loss,
        &receipt.reasons,
        receipt.policy_decision,
        &receipt.effect_receipt,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn release_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    run: &ValidatedResearchRun,
    verdict: &ReleaseAssuranceVerdict,
    study_order: &[String],
    modality_order: &[String],
    artifact_order: &[String],
    evidence_receipt_order: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    semantic_loss: &[SemanticLoss],
    reasons: &[String],
    policy_decision: PolicyDecision,
    effect_receipt: &str,
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "run": run,
        "verdict": verdict,
        "study_order": study_order,
        "modality_order": modality_order,
        "artifact_order": artifact_order,
        "evidence_receipt_order": evidence_receipt_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "policy_decision": policy_decision,
        "effect_receipt": effect_receipt,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> ValidatedResearchRun {
        let protocol = ContentHash::of_bytes(b"protocol");
        ValidatedResearchRun {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            run_id: "run:release".into(),
            release_id: "release:2026-q3".into(),
            purpose: "portable preclinical research object".into(),
            studies: vec![
                ReleaseStudyManifest {
                    study_id: "study:omics".into(),
                    modality: "omics".into(),
                    protocol_digest: protocol.clone(),
                    artifact_ids: vec!["artifact:omics".into()],
                    comparable: true,
                    uncertainty: "batch interval is bounded".into(),
                },
                ReleaseStudyManifest {
                    study_id: "study:imaging".into(),
                    modality: "imaging".into(),
                    protocol_digest: protocol,
                    artifact_ids: vec!["artifact:imaging".into()],
                    comparable: true,
                    uncertainty: "segmentation interval is bounded".into(),
                },
            ],
            evidence_receipt_ids: vec!["evidence:1".into()],
            release_digest: ContentHash::of_bytes(b"release"),
            policy: PolicyReceipt {
                schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                receipt_id: "policy:release".into(),
                decision: PolicyDecision::Allow,
                reasons: vec!["release approver allow".into()],
                evaluated_artifacts: Vec::new(),
                authority_reference: Some("authority:release".into()),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            provenance_complete: true,
            raw_data_local: true,
            localization_statement: "raw data remains institution-local".into(),
            signer_public_key_hex: "a".repeat(64),
            signer_signature_hex: "b".repeat(128),
            protected_omissions: Vec::new(),
            negative_evidence: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn multimodal_release_is_deterministic() {
        let mut reversed = run();
        reversed.studies.reverse();
        let first = assure_release(&run()).unwrap();
        let second = assure_release(&reversed).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.verdict, ReleaseAssuranceVerdict::Released);
    }

    #[test]
    fn incomparable_study_is_not_released() {
        let mut run = run();
        run.studies[0].comparable = false;
        assert_eq!(
            assure_release(&run).unwrap().verdict,
            ReleaseAssuranceVerdict::Incomparable
        );
    }

    #[test]
    fn denied_policy_blocks_release() {
        let mut run = run();
        run.policy.decision = PolicyDecision::Deny;
        assert_eq!(
            assure_release(&run).unwrap().verdict,
            ReleaseAssuranceVerdict::Blocked
        );
    }

    #[test]
    fn protected_omission_is_conditional_and_retained() {
        let mut run = run();
        run.protected_omissions
            .push("study:replicate missing".into());
        let receipt = assure_release(&run).unwrap();
        assert_eq!(receipt.verdict, ReleaseAssuranceVerdict::Conditional);
        assert!(!receipt.omissions.is_empty());
    }

    #[test]
    fn retained_study_input_tampering_is_rejected() {
        let mut receipt = assure_release(&run()).unwrap();
        receipt.run.studies[0].comparable = false;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn release_artifact_provenance_tampering_is_rejected() {
        let mut receipt = assure_release(&run()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_policy_decision_tampering_is_rejected() {
        let mut receipt = assure_release(&run()).unwrap();
        receipt.policy_decision = PolicyDecision::Deny;
        assert!(receipt.validate().is_err());
    }
}
