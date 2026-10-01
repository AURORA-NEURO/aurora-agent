//! Glioma research release handlers grouped by release workflow.

use super::*;

impl Server {
    /// Normalize release metadata under an approved field map and controlled vocabularies.
    pub(super) fn glioma_release_metadata_normalize(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseMetadataNormalizationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_metadata_normalize requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release metadata request: {error}"))?;
        let normalization = normalize_glioma_release_metadata(&request)
            .map_err(|error| format!("glioma release metadata normalization refused: {error}"))?;
        Ok(json!({
            "normalization": normalization,
            "dispatch": "not_started",
            "guarantees": [
                "source fields, approved mappings, vocabulary transforms, conflicts, omissions, and reversible changes are explicit",
                "required target fields cannot be inferred without a confirmed inference rule",
                "normalization emits metadata only and does not publish or transfer artifacts"
            ]
        }))
    }

    /// Issue a content-bound release attestation after policy and verification gates.
    pub(super) fn glioma_release_attestation_issue(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: SignedReleaseAttestationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_attestation_issue requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release attestation request: {error}"))?;
        let attestation = attest_glioma_release(&request)
            .map_err(|error| format!("glioma release attestation refused: {error}"))?;
        Ok(json!({
            "attestation": attestation,
            "dispatch": "not_started",
            "guarantees": [
                "the attestation binds manifest, provenance, release-gate status, signer authority, policy scope, and verifier results",
                "revoked, inactive, out-of-scope authorities and failed required verifications cannot issue a publishable attestation",
                "this deterministic signature seam does not contact an external key service or publish the object"
            ]
        }))
    }

    /// Verify an attestation against caller-supplied trust roots and expected release identity.
    pub(super) fn glioma_release_signature_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseSignatureVerificationRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_signature_verify requires request".to_string())?,
        )
        .map_err(|error| {
            format!("invalid glioma release signature verification request: {error}")
        })?;
        let report = verify_glioma_release_signature(&request)
            .map_err(|error| format!("glioma release signature verification refused: {error}"))?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "guarantees": [
                "manifest, provenance, release gate, policy scope, key authority, signature bytes, revocation, and age are independently checked",
                "unknown trust roots and mismatched expected identities fail verification",
                "verification does not authorize publication or external artifact transfer"
            ]
        }))
    }

    /// Check a research object against a target schema and its required evidence profile.
    pub(super) fn glioma_research_object_conformance_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ConformanceRequest =
            serde_json::from_value(arguments.get("request").cloned().ok_or_else(|| {
                "glioma_research_object_conformance_check requires request".to_string()
            })?)
            .map_err(|error| {
                format!("invalid glioma research object conformance request: {error}")
            })?;
        let report = evaluate_glioma_research_object_conformance(&request).map_err(|error| {
            format!("glioma research object conformance check refused: {error}")
        })?;
        Ok(json!({
            "report": report,
            "dispatch": "not_started",
            "guarantees": [
                "required, allowed, forbidden, and extension fields are evaluated separately",
                "artifact presence and locality, provenance, uncertainty, negative evidence, signature, and migration constraints remain explicit",
                "conformance is a schema assessment and does not release or publish the research object"
            ]
        }))
    }

    /// Preview an audience-specific release while redacting restricted sections and artifacts.
    pub(super) fn glioma_release_preview(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReleasePreviewRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_preview requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release preview request: {error}"))?;
        let preview = preview_glioma_release_audience(&request)
            .map_err(|error| format!("glioma release preview refused: {error}"))?;
        Ok(json!({
            "preview": preview,
            "dispatch": "not_started",
            "guarantees": [
                "audience, export profile, allowed and redacted sections, artifact visibility, and prior-preview comparison are explicit",
                "the preview returns a bounded release view without transferring or publishing source artifacts"
            ]
        }))
    }

    /// Reconcile the release event ledger and produce a bounded review queue snapshot.
    pub(super) fn glioma_release_queue_snapshot(&self, arguments: &Value) -> Result<Value, String> {
        let request: ReleaseQueueRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_queue_snapshot requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release queue request: {error}"))?;
        let snapshot = snapshot_glioma_release_queue(&request)
            .map_err(|error| format!("glioma release queue snapshot refused: {error}"))?;
        Ok(json!({
            "snapshot": snapshot,
            "dispatch": "not_started",
            "guarantees": [
                "candidate state is reconciled from the bounded event ledger with stale telemetry and conflicting observations exposed",
                "reviewer assignment capacity and unresolved release checks remain visible",
                "a queue snapshot schedules no reviewer work and performs no publication"
            ]
        }))
    }

    /// Evaluate transitive release rights, locality, embargo, and audience constraints.
    pub(super) fn glioma_release_shareability_check(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request: ReleaseShareabilityRequest = serde_json::from_value(
            arguments
                .get("request")
                .cloned()
                .ok_or_else(|| "glioma_release_shareability_check requires request".to_string())?,
        )
        .map_err(|error| format!("invalid glioma release shareability request: {error}"))?;
        let decision = evaluate_glioma_release_shareability(&request)
            .map_err(|error| format!("glioma release shareability evaluation refused: {error}"))?;
        Ok(json!({
            "decision": decision,
            "dispatch": "not_started",
            "guarantees": [
                "transitive dependencies, field classifications, license terms, audience, embargo, rights, and locality are checked together",
                "unknown or restricted dependencies fail closed and remain visible in the decision",
                "a shareability decision authorizes no upload, publication, or artifact transfer"
            ]
        }))
    }
}
