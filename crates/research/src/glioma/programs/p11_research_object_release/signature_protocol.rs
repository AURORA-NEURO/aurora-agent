//! Canonical local Ed25519 signing-payload preparation and signature verification.
//!
//! GAF-GLIOMA-P11-F21 prepares a detached-signature payload from a ready P11-F13 workflow and a
//! completed P11-F17 checklist, then verifies a caller-supplied signature against the supplied
//! public key. It never receives or manages a private key. A valid mathematical signature does
//! not authenticate the signer/key authority, prevent challenge replay, authorize publication,
//! or move the research object. The external blueprint distribution is not included in this
//! checkout; the repository-local acceptance contract is recorded as P11-F21 in
//! `docs/glioma/PROGRAM_PLAN.md`.

use super::local_release_workflow::{
    GliomaLocalReleaseWorkflow, GliomaLocalReleaseWorkflowDisposition,
    GliomaLocalReleaseWorkflowError,
};
use super::release_gate::ReleaseGateStatus;
use super::review_workbench::{
    LocalReleaseReviewPacket, LocalReleaseReviewPacketDisposition, LocalReleaseReviewPacketError,
};
use bioprism_ids::{ContentHash, canonical::to_canonical_bytes_serializable};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F21";
pub const INPUT_SCHEMA: &str = "GliomaLocalReleaseSignatureContext1@1";
pub const OUTPUT_SCHEMA: &str = "GliomaLocalReleaseSignatureVerification1@1";
pub const SIGNATURE_ALGORITHM: &str = "ed25519";
pub const SIGNATURE_PURPOSE: &str = "p11-local-release-signing-approval";
pub const MAX_CONTEXT_ID_BYTES: usize = 128;
pub const MAX_PAYLOAD_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalReleaseSignatureContextRequest {
    pub request_id: String,
    pub workflow: GliomaLocalReleaseWorkflow,
    pub review_packet: LocalReleaseReviewPacket,
    pub signer_key_id: String,
    pub signer_public_key: [u8; 32],
    /// Caller-issued challenge commitment. This verifier does not track use or enforce uniqueness.
    pub challenge_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalReleaseSignaturePayload {
    pub schema_version: String,
    pub feature_id: String,
    pub purpose: String,
    pub request_id: String,
    pub research_id: String,
    pub study_id: String,
    pub manifest_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub review_packet_digest: ContentHash,
    pub release_gate_digest: ContentHash,
    pub signer_key_id: String,
    pub signer_public_key_digest: ContentHash,
    pub challenge_digest: ContentHash,
    pub signature_algorithm: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedLocalReleaseSignaturePayload {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub payload: LocalReleaseSignaturePayload,
    pub signer_public_key: [u8; 32],
    /// Exact compact JSON bytes that the accountable signer must sign.
    pub canonical_payload_bytes: Vec<u8>,
    pub payload_digest: ContentHash,
    pub signer_identity_authenticated: bool,
    pub key_authority_authenticated: bool,
    pub challenge_replay_checked: bool,
    pub release_authorized: bool,
    pub published: bool,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalReleaseSignatureVerificationRequest {
    pub context: LocalReleaseSignatureContextRequest,
    pub detached_signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalReleaseSignatureVerification {
    pub feature_id: String,
    pub input_schema: String,
    pub output_schema: String,
    /// Original context is retained so consumers can revalidate every digest binding.
    pub context: LocalReleaseSignatureContextRequest,
    pub payload: LocalReleaseSignaturePayload,
    pub payload_digest: ContentHash,
    pub signer_public_key: [u8; 32],
    pub detached_signature: Vec<u8>,
    pub signature_digest: ContentHash,
    pub signature_verified: bool,
    pub signer_identity_authenticated: bool,
    pub key_authority_authenticated: bool,
    pub challenge_replay_checked: bool,
    pub release_authorized: bool,
    pub published: bool,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LocalReleaseSignatureProtocolError {
    #[error("local release signature context is invalid: {0}")]
    InvalidContext(String),
    #[error("local release workflow is invalid: {0}")]
    Workflow(#[from] GliomaLocalReleaseWorkflowError),
    #[error("local release review packet is invalid: {0}")]
    ReviewPacket(#[from] LocalReleaseReviewPacketError),
    #[error("detached Ed25519 signature is invalid")]
    InvalidSignature,
    #[error("signature payload or verification record is invalid: {0}")]
    InvalidOutput(String),
    #[error("local release signature digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, LocalReleaseSignatureProtocolError> {
    ContentHash::of_serializable(value)
        .map_err(|error| LocalReleaseSignatureProtocolError::Digest(error.to_string()))
}

fn valid_id(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_CONTEXT_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@".contains(&byte))
}

fn payload_bytes(
    payload: &LocalReleaseSignaturePayload,
) -> Result<Vec<u8>, LocalReleaseSignatureProtocolError> {
    to_canonical_bytes_serializable(payload)
        .map_err(|error| LocalReleaseSignatureProtocolError::InvalidContext(error.to_string()))
}

fn payload_digest(
    payload: &LocalReleaseSignaturePayload,
) -> Result<ContentHash, LocalReleaseSignatureProtocolError> {
    let bytes = payload_bytes(payload)?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(LocalReleaseSignatureProtocolError::InvalidContext(
            "canonical signing payload exceeds the protocol size bound".into(),
        ));
    }
    Ok(ContentHash::of_bytes(&bytes))
}

fn next_operator_action() -> &'static str {
    "route the signature-verified record through institutional key-authority, replay, and release authorization checks"
}

fn prepared_digest(
    prepared: &PreparedLocalReleaseSignaturePayload,
) -> Result<ContentHash, LocalReleaseSignatureProtocolError> {
    digest(&(
        &prepared.feature_id,
        &prepared.input_schema,
        &prepared.output_schema,
        &prepared.payload,
        &prepared.signer_public_key,
        &prepared.canonical_payload_bytes,
        &prepared.payload_digest,
        prepared.signer_identity_authenticated,
        prepared.key_authority_authenticated,
        prepared.challenge_replay_checked,
        prepared.release_authorized,
        prepared.published,
        &prepared.next_operator_action,
    ))
}

fn verification_digest(
    record: &LocalReleaseSignatureVerification,
) -> Result<ContentHash, LocalReleaseSignatureProtocolError> {
    digest(&(
        &record.feature_id,
        &record.input_schema,
        &record.output_schema,
        &record.context,
        &record.payload,
        &record.payload_digest,
        &record.signer_public_key,
        &record.detached_signature,
        &record.signature_digest,
        record.signature_verified,
        record.signer_identity_authenticated,
        record.key_authority_authenticated,
        record.challenge_replay_checked,
        record.release_authorized,
        record.published,
        &record.next_operator_action,
    ))
}

fn prepare_payload(
    request: &LocalReleaseSignatureContextRequest,
) -> Result<LocalReleaseSignaturePayload, LocalReleaseSignatureProtocolError> {
    if !valid_id(&request.request_id) || !valid_id(&request.signer_key_id) {
        return Err(LocalReleaseSignatureProtocolError::InvalidContext(
            "request and signer key IDs must be non-empty bounded protocol identifiers".into(),
        ));
    }
    request.workflow.validate()?;
    request.review_packet.validate()?;
    let Some(gate) = request.workflow.gate.as_ref() else {
        return Err(LocalReleaseSignatureProtocolError::InvalidContext(
            "a release-gate evaluation is required before signing payload preparation".into(),
        ));
    };
    if request.workflow.disposition != GliomaLocalReleaseWorkflowDisposition::ReadyForSigning
        || gate.status != ReleaseGateStatus::Publishable
        || request.review_packet.disposition != LocalReleaseReviewPacketDisposition::ReviewComplete
        || request.review_packet.workflow_digest != request.workflow.digest
        || request.review_packet.research_id != request.workflow.research_id
        || request.review_packet.study_id != request.workflow.study_id
    {
        return Err(LocalReleaseSignatureProtocolError::InvalidContext(
            "workflow, release gate, and completed human checklist must refer to one ready local release".into(),
        ));
    }
    let public_key_digest = ContentHash::of_bytes(&request.signer_public_key);
    let payload = LocalReleaseSignaturePayload {
        schema_version: "aurora-research-contract/1.0".into(),
        feature_id: FEATURE_ID.into(),
        purpose: SIGNATURE_PURPOSE.into(),
        request_id: request.request_id.clone(),
        research_id: request.workflow.research_id.clone(),
        study_id: request.workflow.study_id.clone(),
        manifest_digest: request.workflow.bundle.manifest.manifest_digest.clone(),
        workflow_digest: request.workflow.digest.clone(),
        review_packet_digest: request.review_packet.packet_digest.clone(),
        release_gate_digest: gate.digest.clone(),
        signer_key_id: request.signer_key_id.clone(),
        signer_public_key_digest: public_key_digest,
        challenge_digest: request.challenge_digest.clone(),
        signature_algorithm: SIGNATURE_ALGORITHM.into(),
    };
    let _ = payload_digest(&payload)?;
    Ok(payload)
}

/// Prepare the exact compact JSON payload bytes for a caller-controlled institutional signer.
pub fn prepare_glioma_local_release_signature_payload(
    request: &LocalReleaseSignatureContextRequest,
) -> Result<PreparedLocalReleaseSignaturePayload, LocalReleaseSignatureProtocolError> {
    let payload = prepare_payload(request)?;
    let canonical_payload_bytes = payload_bytes(&payload)?;
    let payload_digest = ContentHash::of_bytes(&canonical_payload_bytes);
    let mut prepared = PreparedLocalReleaseSignaturePayload {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        payload,
        signer_public_key: request.signer_public_key,
        canonical_payload_bytes,
        payload_digest,
        signer_identity_authenticated: false,
        key_authority_authenticated: false,
        challenge_replay_checked: false,
        release_authorized: false,
        published: false,
        next_operator_action: next_operator_action().into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-local-release-signature-payload"),
    };
    prepared.digest = prepared_digest(&prepared)?;
    prepared.validate()?;
    Ok(prepared)
}

/// Verify a detached Ed25519 signature over the exact P11-F21 canonical release payload.
pub fn verify_glioma_local_release_signature(
    request: &LocalReleaseSignatureVerificationRequest,
) -> Result<LocalReleaseSignatureVerification, LocalReleaseSignatureProtocolError> {
    let payload = prepare_payload(&request.context)?;
    if request.detached_signature.len() != 64 {
        return Err(LocalReleaseSignatureProtocolError::InvalidSignature);
    }
    let bytes = payload_bytes(&payload)?;
    let public_key = VerifyingKey::from_bytes(&request.context.signer_public_key)
        .map_err(|_| LocalReleaseSignatureProtocolError::InvalidSignature)?;
    let signature = Signature::from_slice(&request.detached_signature)
        .map_err(|_| LocalReleaseSignatureProtocolError::InvalidSignature)?;
    public_key
        .verify_strict(&bytes, &signature)
        .map_err(|_| LocalReleaseSignatureProtocolError::InvalidSignature)?;
    let mut record = LocalReleaseSignatureVerification {
        feature_id: FEATURE_ID.into(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        context: request.context.clone(),
        payload_digest: ContentHash::of_bytes(&bytes),
        payload,
        signer_public_key: request.context.signer_public_key,
        signature_digest: ContentHash::of_bytes(&request.detached_signature),
        detached_signature: request.detached_signature.clone(),
        signature_verified: true,
        signer_identity_authenticated: false,
        key_authority_authenticated: false,
        challenge_replay_checked: false,
        release_authorized: false,
        published: false,
        next_operator_action: next_operator_action().into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-local-release-signature-verification"),
    };
    record.digest = verification_digest(&record)?;
    record.validate()?;
    Ok(record)
}

impl PreparedLocalReleaseSignaturePayload {
    /// Validate canonical payload bytes and explicit trust/effect boundaries.
    pub fn validate(&self) -> Result<(), LocalReleaseSignatureProtocolError> {
        let expected_bytes = payload_bytes(&self.payload)?;
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.payload.feature_id != FEATURE_ID
            || self.payload.schema_version != "aurora-research-contract/1.0"
            || self.payload.purpose != SIGNATURE_PURPOSE
            || self.payload.signature_algorithm != SIGNATURE_ALGORITHM
            || self.payload.signer_key_id.trim().is_empty()
            || self.payload.signer_public_key_digest
                != ContentHash::of_bytes(&self.signer_public_key)
            || self.canonical_payload_bytes != expected_bytes
            || self.canonical_payload_bytes.len() > MAX_PAYLOAD_BYTES
            || self.payload_digest != ContentHash::of_bytes(&self.canonical_payload_bytes)
            || self.signer_identity_authenticated
            || self.key_authority_authenticated
            || self.challenge_replay_checked
            || self.release_authorized
            || self.published
            || self.next_operator_action != next_operator_action()
            || self.digest != prepared_digest(self)?
        {
            return Err(LocalReleaseSignatureProtocolError::InvalidOutput(
                "prepared payload bytes, digest, authentication/effect boundary, or record digest is invalid".into(),
            ));
        }
        Ok(())
    }
}

impl LocalReleaseSignatureVerification {
    /// Revalidate the Ed25519 signature and all content-addressed verification fields.
    pub fn validate(&self) -> Result<(), LocalReleaseSignatureProtocolError> {
        let expected_payload = prepare_payload(&self.context)?;
        let bytes = payload_bytes(&self.payload)?;
        let public_key_digest = ContentHash::of_bytes(&self.signer_public_key);
        let verifying_key = VerifyingKey::from_bytes(&self.signer_public_key).map_err(|_| {
            LocalReleaseSignatureProtocolError::InvalidOutput("public key is malformed".into())
        })?;
        let signature = Signature::from_slice(&self.detached_signature).map_err(|_| {
            LocalReleaseSignatureProtocolError::InvalidOutput(
                "signature bytes are malformed".into(),
            )
        })?;
        verifying_key
            .verify_strict(&bytes, &signature)
            .map_err(|_| {
                LocalReleaseSignatureProtocolError::InvalidOutput(
                    "signature does not verify over the canonical payload".into(),
                )
            })?;
        if self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.payload.feature_id != FEATURE_ID
            || self.payload != expected_payload
            || self.signer_public_key != self.context.signer_public_key
            || self.payload.purpose != SIGNATURE_PURPOSE
            || self.payload.signature_algorithm != SIGNATURE_ALGORITHM
            || self.payload.signer_public_key_digest != public_key_digest
            || self.payload_digest != ContentHash::of_bytes(&bytes)
            || self.signature_digest != ContentHash::of_bytes(&self.detached_signature)
            || !self.signature_verified
            || self.signer_identity_authenticated
            || self.key_authority_authenticated
            || self.challenge_replay_checked
            || self.release_authorized
            || self.published
            || self.next_operator_action != next_operator_action()
            || self.digest != verification_digest(self)?
        {
            return Err(LocalReleaseSignatureProtocolError::InvalidOutput(
                "signature record binding, verification, authentication/effect boundary, or digest is invalid".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::local_release_workflow::{
        GliomaLocalReleaseWorkflowRequest, execute_glioma_local_release_workflow_dry_run,
    };
    use super::super::multimodal_bundle::{
        MultimodalResearchObjectInput, MultimodalResearchObjectRequest,
    };
    use super::super::release_gate::{
        ReleaseGateRequest, ReleaseReviewAttestation, ReleaseReviewDecision,
    };
    use super::super::replay::{ReplayCampaignRequest, ReplayTask};
    use super::super::review_workbench::{
        LocalReleaseReviewPacketRequest, ReleaseChecklistDecision, ReleaseChecklistResponse,
        ReleaseReviewRole, compile_glioma_local_release_review_packet,
    };
    use super::*;
    use crate::glioma::release::ResearchObjectRequest;
    use crate::glioma_engine::{GliomaModality, LocalArtifactRef};
    use ed25519_dalek::{Signer, SigningKey};
    use std::collections::BTreeSet;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn workflow_request() -> GliomaLocalReleaseWorkflowRequest {
        let artifact = LocalArtifactRef {
            artifact_id: "artifact-main".into(),
            content_hash: hash("artifact-main"),
            content_type: "application/vnd.aurora.glioma.result+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        };
        let release = ResearchObjectRequest {
            research_id: "p11-signing-research".into(),
            study_id: "p11-signing-study".into(),
            objective: "verify the exact local release signing payload".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["P03".into()],
            artifacts: vec![artifact.clone()],
            negative_evidence: Vec::new(),
            limitations: vec!["preclinical-only".into()],
            raw_data_local: true,
            aggregate_only: true,
        };
        GliomaLocalReleaseWorkflowRequest {
            package: MultimodalResearchObjectRequest {
                release: release.clone(),
                inputs: vec![MultimodalResearchObjectInput {
                    modality: GliomaModality::Imaging,
                    artifact: artifact.clone(),
                    source_program: "P03".into(),
                    schema_version: "1.0".into(),
                    semantic_loss_milli: 100,
                    provenance_digest: hash("provenance"),
                    upstream_artifact_ids: Vec::new(),
                    required: true,
                }],
                required_modalities: BTreeSet::from([GliomaModality::Imaging]),
                max_semantic_loss_milli: 200,
                require_cross_modal_alignment: false,
                max_inputs: 8,
            },
            max_dependency_depth: 8,
            required_programs: BTreeSet::new(),
            require_program_coverage: false,
            replay: ReplayCampaignRequest {
                release,
                tasks: vec![ReplayTask {
                    task_id: "artifact-main-replay".into(),
                    program_id: "P03".into(),
                    artifact_id: "artifact-main".into(),
                    expected_content_hash: artifact.content_hash,
                    cost_units: 1,
                    required: true,
                    deterministic: true,
                    depends_on: Vec::new(),
                }],
                budget_units: 1,
                max_rounds: 2,
                max_retries: 1,
                min_coverage_milli: 1_000,
                require_exact_hash: true,
            },
            gate: ReleaseGateRequest {
                required_coverage_milli: 1_000,
                require_exact_hash: true,
                require_reproducible: true,
                require_accountable_review: true,
                min_independent_approvals: 1,
                max_uncertainty_items: 32,
                reviews: vec![ReleaseReviewAttestation {
                    reviewer_id: "independent-reviewer".into(),
                    role: "reproducibility-steward".into(),
                    decision: ReleaseReviewDecision::Approve,
                    evidence_digest: hash("review-evidence"),
                    independent: true,
                }],
            },
        }
    }

    fn ready_workflow_and_review_packet() -> (GliomaLocalReleaseWorkflow, LocalReleaseReviewPacket)
    {
        let workflow = execute_glioma_local_release_workflow_dry_run(&workflow_request()).unwrap();
        assert_eq!(
            workflow.disposition,
            GliomaLocalReleaseWorkflowDisposition::ReadyForSigning
        );
        let request = LocalReleaseReviewPacketRequest {
            workflow: workflow.clone(),
            reviewer_commitment: hash("reviewer"),
            reviewer_role: ReleaseReviewRole::IndependentReviewer,
            independent_reviewer_claim: true,
            responses: Vec::new(),
        };
        let pending = compile_glioma_local_release_review_packet(&request).unwrap();
        let responses = pending
            .checklist
            .iter()
            .enumerate()
            .map(|(index, item)| ReleaseChecklistResponse {
                item_commitment: item.item_commitment.clone(),
                decision: ReleaseChecklistDecision::Acknowledge,
                response_digest: hash(&format!("response-{index}")),
            })
            .collect();
        let completed =
            compile_glioma_local_release_review_packet(&LocalReleaseReviewPacketRequest {
                responses,
                ..request
            })
            .unwrap();
        assert_eq!(
            completed.disposition,
            LocalReleaseReviewPacketDisposition::ReviewComplete
        );
        (workflow, completed)
    }

    fn signing_context(
        workflow: GliomaLocalReleaseWorkflow,
        review_packet: LocalReleaseReviewPacket,
        signing_key: &SigningKey,
    ) -> LocalReleaseSignatureContextRequest {
        LocalReleaseSignatureContextRequest {
            request_id: "request-001".into(),
            workflow,
            review_packet,
            signer_key_id: "institution-key-01".into(),
            signer_public_key: signing_key.verifying_key().to_bytes(),
            challenge_digest: hash("single-use-challenge-001"),
        }
    }

    pub(crate) fn signed_verification_fixture() -> LocalReleaseSignatureVerification {
        let (workflow, review_packet) = ready_workflow_and_review_packet();
        let signing_key = SigningKey::from_bytes(&[42; 32]);
        let context = signing_context(workflow, review_packet, &signing_key);
        let prepared = prepare_glioma_local_release_signature_payload(&context).unwrap();
        let detached_signature = signing_key.sign(&prepared.canonical_payload_bytes);
        verify_glioma_local_release_signature(&LocalReleaseSignatureVerificationRequest {
            context,
            detached_signature: detached_signature.to_bytes().to_vec(),
        })
        .unwrap()
    }

    #[test]
    fn canonical_payload_binds_ready_children_and_signature_verification_stays_non_authorizing() {
        let (workflow, review_packet) = ready_workflow_and_review_packet();
        let signing_key = SigningKey::from_bytes(&[42; 32]);
        let context = signing_context(workflow, review_packet, &signing_key);
        let prepared = prepare_glioma_local_release_signature_payload(&context).unwrap();
        prepared.validate().unwrap();
        assert_eq!(prepared.payload.workflow_digest, context.workflow.digest);
        assert_eq!(
            prepared.payload.review_packet_digest,
            context.review_packet.packet_digest
        );
        assert_eq!(
            prepared.payload.release_gate_digest,
            context.workflow.gate.as_ref().unwrap().digest
        );
        assert_eq!(
            prepared.payload.signer_public_key_digest,
            ContentHash::of_bytes(&context.signer_public_key)
        );
        assert_eq!(prepared.payload.challenge_digest, context.challenge_digest);
        assert_eq!(
            prepared.canonical_payload_bytes,
            payload_bytes(&prepared.payload).unwrap()
        );

        let detached_signature = signing_key.sign(&prepared.canonical_payload_bytes);
        let verification =
            verify_glioma_local_release_signature(&LocalReleaseSignatureVerificationRequest {
                context: context.clone(),
                detached_signature: detached_signature.to_bytes().to_vec(),
            })
            .unwrap();
        verification.validate().unwrap();
        assert!(verification.signature_verified);
        assert!(!verification.signer_identity_authenticated);
        assert!(!verification.key_authority_authenticated);
        assert!(!verification.challenge_replay_checked);
        assert!(!verification.release_authorized);
        assert!(!verification.published);
        assert_eq!(verification.context, context);

        let mut forged_authorization = verification.clone();
        forged_authorization.release_authorized = true;
        assert!(forged_authorization.validate().is_err());
    }

    #[test]
    fn detached_signature_cannot_be_replayed_under_a_different_challenge_or_mutated() {
        let (workflow, review_packet) = ready_workflow_and_review_packet();
        let signing_key = SigningKey::from_bytes(&[17; 32]);
        let context = signing_context(workflow, review_packet, &signing_key);
        let prepared = prepare_glioma_local_release_signature_payload(&context).unwrap();
        let mut detached_signature = signing_key
            .sign(&prepared.canonical_payload_bytes)
            .to_bytes()
            .to_vec();

        let mut changed_context = context.clone();
        changed_context.challenge_digest = hash("single-use-challenge-002");
        let changed_challenge =
            verify_glioma_local_release_signature(&LocalReleaseSignatureVerificationRequest {
                context: changed_context,
                detached_signature: detached_signature.clone(),
            });
        assert_eq!(
            changed_challenge.unwrap_err(),
            LocalReleaseSignatureProtocolError::InvalidSignature
        );

        detached_signature[0] ^= 0x01;
        let changed_signature =
            verify_glioma_local_release_signature(&LocalReleaseSignatureVerificationRequest {
                context,
                detached_signature,
            });
        assert_eq!(
            changed_signature.unwrap_err(),
            LocalReleaseSignatureProtocolError::InvalidSignature
        );
    }
}
