//! Domain evidence provider normalization, replay, and external payload handoffs.

use super::*;

impl Server {
    /// Normalize caller-managed provider evidence and retain it through the ordinary intake
    /// boundary. No provider is contacted and no provider-shaped field is interpreted as truth.
    pub(super) fn domain_evidence_provider_normalize(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode provider normalization input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err(
                "provider normalization input exceeds the 20000000-byte safety bound".into(),
            );
        }
        let request: DomainEvidenceProviderNormalizationRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid provider normalization input: {error}"))?;
        let normalized = normalize_domain_evidence_provider(&request)
            .map_err(|error| format!("domain evidence provider normalization refused: {error}"))?;
        let intake = self.domain_evidence_intake(&normalized.intake_arguments)?;
        let catalogue_digest = intake
            .get("catalogue_digest")
            .and_then(Value::as_str)
            .ok_or("provider normalization intake omitted catalogue_digest")?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_NORMALIZATION_WORKFLOW,
            "group_id": normalized.group_id,
            "domains": normalized.domains,
            "subject_id": normalized.subject_id,
            "source_tool": normalized.source_tool,
            "connector_kind": normalized.connector_kind,
            "provider": normalized.provider,
            "outcome": normalized.outcome,
            "payload_digest": normalized.payload_digest,
            "request_digest": normalized.request_digest,
            "response": normalized.response,
            "shape_audit": normalized.shape_audit,
            "record_index": normalized.record_index,
            "normalization": normalized,
            "intake": intake,
            "artifact_registry": intake.get("artifact_registry"),
            "catalogue_digest": catalogue_digest,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "caller-managed provider payloads use the same catalogue-bound intake and coverage path as bounded source reads",
                "provider, connector, payload, and optional request identities remain explicit and digest-addressed",
                "connector-specific shape audit facts are deterministic, bounded, and value-free",
                "caller-supplied outcomes are preserved without inferring success from payload shape"
            ],
            "does_not_claim": [
                "provider authenticity, signature validity, or remote execution",
                "scientific, clinical, causal, provenance, regulatory, or release validity",
                "retrieval completeness, terminology resolution, or external-effect completion"
            ]
        }))
    }

    /// Verify a caller-managed provider payload against retained digest identities and index the
    /// value-free replay record idempotently. No connector, provider, or external effect runs.
    pub(super) fn domain_evidence_provider_replay_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode provider replay input: {error}"))?;
        if encoded.len() > 20_000_000 {
            return Err("provider replay input exceeds the 20000000-byte safety bound".into());
        }
        let request: DomainEvidenceProviderReplayRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid provider replay input: {error}"))?;
        let verification = verify_domain_evidence_provider_replay(&request)
            .map_err(|error| format!("domain evidence provider replay refused: {error}"))?;
        let artifact = serde_json::to_value(&verification)
            .map_err(|error| format!("cannot encode provider replay verification: {error}"))?;
        let artifact_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_replay",
                "subject_id": verification.subject_id,
                "domains": verification.domains,
                "parent_digests": [verification.expected_intake_digest, verification.expected_normalization_digest],
                "declared_digest": verification.replay_digest,
                "artifact": artifact
            }
        }))?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_REPLAY_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_REPLAY_WORKFLOW,
            "replay": verification,
            "matched": artifact["matched"],
            "replay_status": artifact["replay_status"],
            "replay_digest": artifact["replay_digest"],
            "artifact_registry": artifact_registry,
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "the supplied payload is compared to retained identities without provider contact",
                "mismatch dimensions remain visible instead of being collapsed into a success flag",
                "the value-free replay verification is retained through the idempotent artifact registry"
            ],
            "does_not_claim": [
                "provider authenticity, request execution, or retrieval completeness",
                "scientific, clinical, causal, regulatory, provenance, or release validity",
                "that a matching JSON replay proves the provider returned the payload originally"
            ]
        }))
    }

    /// Register a caller-managed provider connector declaration before payload intake. This is
    /// deliberately a handoff boundary: the core validates scope and digests, but never launches
    /// a plugin, resolves credentials, authenticates a provider, or contacts a network.
    pub(super) fn domain_evidence_provider_connector_handoff(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode provider handoff input: {error}"))?;
        if encoded.len() > 2_000_000 {
            return Err("provider handoff input exceeds the 2000000-byte safety bound".into());
        }
        let request: DomainEvidenceProviderHandoffRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid provider handoff input: {error}"))?;
        let handoff = handoff_domain_evidence_provider(&request)
            .map_err(|error| format!("domain evidence provider handoff refused: {error}"))?;
        let artifact = serde_json::to_value(&handoff)
            .map_err(|error| format!("cannot encode provider handoff artifact: {error}"))?;
        let artifact_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_handoff",
                "subject_id": handoff.subject_id,
                "domains": handoff.domains,
                "parent_digests": handoff.parent_digests,
                "declared_digest": handoff.handoff_digest,
                "artifact": artifact
            }
        }))?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_HANDOFF_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_HANDOFF_WORKFLOW,
            "handoff": handoff,
            "manifest_digest": handoff.manifest_digest,
            "handoff_digest": handoff.handoff_digest,
            "artifact_registry": artifact_registry,
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "connector scope, capabilities, and auth posture are validated before payload intake",
                "only opaque caller-owned secret references may be retained",
                "request and payload identities can parent a later provider-normalization artifact"
            ],
            "does_not_claim": [
                "plugin launch, provider authentication, authorization, or network execution",
                "provider authenticity, retrieval completeness, or payload correctness",
                "scientific, clinical, causal, provenance, regulatory, or release validity"
            ]
        }))
    }

    /// Retain caller-owned metadata for a large provider payload stored outside the core. The
    /// receipt is an integrity/lineage record only; it never opens the locator or copies payload
    /// bytes into MCP.
    pub(super) fn domain_evidence_provider_external_payload_receipt(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode external payload receipt input: {error}"))?;
        if encoded.len() > 2_000_000 {
            return Err(
                "external payload receipt input exceeds the 2000000-byte safety bound".into(),
            );
        }
        let request: DomainEvidenceProviderExternalPayloadReceiptRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid external payload receipt input: {error}"))?;
        let receipt = record_domain_evidence_provider_external_payload(&request)
            .map_err(|error| format!("external payload receipt refused: {error}"))?;
        let artifact = serde_json::to_value(&receipt)
            .map_err(|error| format!("cannot encode external payload receipt artifact: {error}"))?;
        let mut parents = receipt.parent_digests.clone();
        parents.push(receipt.handoff_digest.clone());
        let artifact_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_external_payload",
                "subject_id": receipt.subject_id,
                "domains": receipt.domains,
                "parent_digests": parents,
                "declared_digest": receipt.receipt_digest,
                "artifact": artifact
            }
        }))?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_WORKFLOW,
            "receipt": receipt,
            "handoff_digest": receipt.handoff_digest,
            "payload_digest": receipt.payload_digest,
            "receipt_digest": receipt.receipt_digest,
            "artifact_registry": artifact_registry,
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "the external payload is represented by exact digest, byte length, and transfer metadata",
                "the locator remains caller-owned and the receipt can parent later normalization",
                "the durable registry retains the receipt without copying payload bytes"
            ],
            "does_not_claim": [
                "store accessibility, retention, payload availability, or transfer completion beyond caller status",
                "payload authenticity, provider authenticity, decryption, or independent byte inspection",
                "scientific, clinical, causal, provenance, regulatory, or release validity"
            ]
        }))
    }

    /// Verify an out-of-line payload receipt against retained metadata identities. This route
    /// never dereferences the locator, reads bytes, or contacts the provider/store.
    pub(super) fn domain_evidence_provider_external_payload_replay_verify(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode external payload replay input: {error}"))?;
        if encoded.len() > 2_000_000 {
            return Err(
                "external payload replay input exceeds the 2000000-byte safety bound".into(),
            );
        }
        let request: DomainEvidenceProviderExternalPayloadReplayRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid external payload replay input: {error}"))?;
        let verification = verify_domain_evidence_provider_external_payload_replay(&request)
            .map_err(|error| format!("external payload replay refused: {error}"))?;
        let artifact = serde_json::to_value(&verification)
            .map_err(|error| format!("cannot encode external payload replay artifact: {error}"))?;
        let artifact_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_external_payload_replay",
                "subject_id": verification.subject_id,
                "domains": verification.domains,
                "parent_digests": [
                    verification.receipt.receipt_digest,
                    verification.observed_handoff_digest
                ],
                "declared_digest": verification.replay_digest,
                "artifact": artifact
            }
        }))?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_REPLAY_WORKFLOW,
            "replay": verification,
            "matched": artifact["matched"],
            "replay_status": artifact["replay_status"],
            "replay_digest": artifact["replay_digest"],
            "artifact_registry": artifact_registry,
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "external receipt identities are compared without opening the caller locator",
                "digest, handoff, payload, and byte-length drift remain individually visible",
                "the value-free replay verification is retained idempotently"
            ],
            "does_not_claim": [
                "external-store accessibility, byte re-reading, decryption, or provider contact",
                "payload or provider authenticity, retention, scientific, clinical, provenance, or release validity"
            ]
        }))
    }

    /// Materialize a bounded caller-owned JSON payload against an external receipt, then pass
    /// the digest-verified value through ordinary provider normalization and domain intake.
    /// Neither this bridge nor the receipt registration opens the caller locator.
    pub(super) fn domain_evidence_provider_external_payload_normalize(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode external payload normalization input: {error}")
        })?;
        if encoded.len() > 20_000_000 {
            return Err(
                "external payload normalization input exceeds the 20000000-byte safety bound"
                    .into(),
            );
        }
        let request: DomainEvidenceProviderExternalPayloadNormalizationRequest =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                format!("invalid external payload normalization input: {error}")
            })?;
        let bridged = normalize_domain_evidence_provider_external_payload(&request)
            .map_err(|error| format!("external payload normalization refused: {error}"))?;
        let receipt_artifact = serde_json::to_value(&bridged.receipt)
            .map_err(|error| format!("cannot encode external payload receipt artifact: {error}"))?;
        let mut receipt_parents = bridged.receipt.parent_digests.clone();
        receipt_parents.push(bridged.receipt.handoff_digest.clone());
        let receipt_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_external_payload",
                "subject_id": bridged.receipt.subject_id,
                "domains": bridged.receipt.domains,
                "parent_digests": receipt_parents,
                "declared_digest": bridged.receipt.receipt_digest,
                "artifact": receipt_artifact
            }
        }))?;
        let intake = self.domain_evidence_intake(&bridged.normalization.intake_arguments)?;
        let catalogue_digest = intake
            .get("catalogue_digest")
            .and_then(Value::as_str)
            .ok_or("external payload normalization intake omitted catalogue_digest")?;
        let normalization = &bridged.normalization;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_NORMALIZATION_WORKFLOW,
            "group_id": normalization.group_id,
            "domains": normalization.domains,
            "subject_id": normalization.subject_id,
            "source_tool": normalization.source_tool,
            "connector_kind": normalization.connector_kind,
            "provider": normalization.provider,
            "outcome": normalization.outcome,
            "payload_digest": normalization.payload_digest,
            "request_digest": normalization.request_digest,
            "response": normalization.response,
            "shape_audit": normalization.shape_audit,
            "record_index": normalization.record_index,
            "normalization": normalization,
            "receipt": bridged.receipt,
            "receipt_digest": bridged.receipt.receipt_digest,
            "materialization": {
                "mode": "canonical_json",
                "matched": true,
                "materialized_payload_digest": bridged.materialized_payload_digest,
                "receipt_payload_digest": bridged.receipt.payload_digest,
                "receipt_byte_length": bridged.receipt.byte_length,
                "receipt_content_encoding": bridged.receipt.content_encoding,
                "locator_opened": false
            },
            "intake": intake,
            "artifact_registry": intake.get("artifact_registry"),
            "receipt_artifact_registry": receipt_registry,
            "catalogue_digest": catalogue_digest,
            "readiness_claimed": false,
            "execution": "not_started",
            "guarantees": [
                "the caller-materialized canonical JSON digest exactly matches the external receipt payload digest",
                "the verified materialization uses the ordinary provider shape audit and catalogue-bound intake path",
                "the external locator remains unopened and the receipt remains an explicit lineage parent"
            ],
            "does_not_claim": [
                "external-store accessibility, transfer completion, decryption, or byte inspection beyond the supplied materialization",
                "provider authenticity, payload authenticity, scientific, clinical, causal, provenance, regulatory, or release validity",
                "execution or external effects; readiness remains false"
            ]
        }))
    }

    /// Audit an external receipt against the retained connector handoff in the local registry.
    /// Missing or mismatched lineage is reported explicitly and never becomes readiness.
    pub(super) fn domain_evidence_provider_external_payload_lineage_audit(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode external payload lineage input: {error}"))?;
        if encoded.len() > 2_000_000 {
            return Err(
                "external payload lineage input exceeds the 2000000-byte safety bound".into(),
            );
        }
        let request: DomainEvidenceProviderExternalPayloadLineageAuditRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid external payload lineage input: {error}"))?;
        let receipt = record_domain_evidence_provider_external_payload(&request.receipt)
            .map_err(|error| format!("external payload lineage receipt refused: {error}"))?;
        let handoff_value = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?
            .records_for_audit()
            .into_iter()
            .find(|record| {
                record.kind == "domain_evidence_provider_handoff"
                    && record
                        .artifact
                        .get("handoff_digest")
                        .and_then(Value::as_str)
                        == Some(receipt.handoff_digest.as_str())
            })
            .map(|record| record.artifact);
        let handoff = handoff_value
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("retained connector handoff is malformed: {error}"))
            })
            .transpose()?;
        let receipt_artifact = serde_json::to_value(&receipt)
            .map_err(|error| format!("cannot encode external payload receipt: {error}"))?;
        let mut receipt_parents = receipt.parent_digests.clone();
        receipt_parents.push(receipt.handoff_digest.clone());
        let receipt_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_external_payload",
                "subject_id": receipt.subject_id,
                "domains": receipt.domains,
                "parent_digests": receipt_parents,
                "declared_digest": receipt.receipt_digest,
                "artifact": receipt_artifact
            }
        }))?;
        let audit = audit_domain_evidence_provider_external_payload_lineage(receipt, handoff)
            .map_err(|error| format!("external payload lineage audit refused: {error}"))?;
        let audit_artifact = serde_json::to_value(&audit)
            .map_err(|error| format!("cannot encode external payload lineage audit: {error}"))?;
        let audit_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_external_payload_lineage_audit",
                "subject_id": audit.subject_id,
                "domains": audit.domains,
                "parent_digests": [audit.receipt.receipt_digest, audit.receipt.handoff_digest],
                "declared_digest": audit.lineage_digest,
                "artifact": audit_artifact
            }
        }))?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_LINEAGE_WORKFLOW,
            "audit": audit,
            "lineage_status": audit.lineage_status,
            "payload_binding_status": audit.payload_binding_status,
            "lineage_digest": audit.lineage_digest,
            "receipt_registry": receipt_registry,
            "artifact_registry": audit_registry,
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "receipt identity is checked against a retained connector handoff when present",
                "orphaned, partial, and mismatched lineage states remain distinct",
                "the audit never contacts providers, stores, locators, or credential systems"
            ],
            "does_not_claim": [
                "provider authentication, connector execution, transfer completion, or payload availability",
                "scientific, clinical, causal, provenance, regulatory, or release validity",
                "readiness; even matched lineage remains not_started"
            ]
        }))
    }

    /// Retain caller-reported transfer observations and compare them with a registry receipt.
    /// The status is evidence about the caller's observation, never provider authenticity or
    /// readiness, and this method performs no external I/O.
    pub(super) fn domain_evidence_provider_external_payload_execution_evidence(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments).map_err(|error| {
            format!("cannot encode external payload execution evidence input: {error}")
        })?;
        if encoded.len() > 2_000_000 {
            return Err(
                "external payload execution evidence input exceeds the 2000000-byte safety bound"
                    .into(),
            );
        }
        let request: DomainEvidenceProviderExternalPayloadExecutionEvidenceRequest =
            serde_json::from_value(arguments.clone()).map_err(|error| {
                format!("invalid external payload execution evidence input: {error}")
            })?;
        let receipt = record_domain_evidence_provider_external_payload(&request.receipt)
            .map_err(|error| format!("external payload execution receipt refused: {error}"))?;
        let retained_value = self
            .artifact_registry
            .lock()
            .map_err(|_| "artifact registry lock is poisoned".to_string())?
            .records_for_audit()
            .into_iter()
            .find(|record| {
                record.kind == "domain_evidence_provider_external_payload"
                    && record
                        .artifact
                        .get("receipt_digest")
                        .and_then(Value::as_str)
                        == Some(request.expected_receipt_digest.as_str())
            })
            .map(|record| record.artifact);
        let retained_receipt = retained_value
            .map(|value| {
                serde_json::from_value(value)
                    .map_err(|error| format!("retained external receipt is malformed: {error}"))
            })
            .transpose()?;
        let receipt_artifact = serde_json::to_value(&receipt)
            .map_err(|error| format!("cannot encode external payload receipt: {error}"))?;
        let mut receipt_parents = receipt.parent_digests.clone();
        receipt_parents.push(receipt.handoff_digest.clone());
        let receipt_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_external_payload",
                "subject_id": receipt.subject_id,
                "domains": receipt.domains,
                "parent_digests": receipt_parents,
                "declared_digest": receipt.receipt_digest,
                "artifact": receipt_artifact
            }
        }))?;
        let evidence = audit_domain_evidence_provider_external_payload_execution(
            receipt,
            retained_receipt,
            &request,
        )
        .map_err(|error| format!("external payload execution evidence refused: {error}"))?;
        let evidence_artifact = serde_json::to_value(&evidence).map_err(|error| {
            format!("cannot encode external payload execution evidence: {error}")
        })?;
        let artifact_registry = self.artifact_registry_audit(&json!({
            "operation": "register",
            "registration": {
                "kind": "domain_evidence_provider_external_payload_execution_evidence",
                "subject_id": evidence.subject_id,
                "domains": evidence.domains,
                "parent_digests": [evidence.receipt.receipt_digest, evidence.receipt.handoff_digest],
                "declared_digest": evidence.evidence_digest,
                "artifact": evidence_artifact
            }
        }))?;
        Ok(json!({
            "ok": true,
            "schema": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_SCHEMA,
            "workflow": DOMAIN_EVIDENCE_PROVIDER_EXTERNAL_PAYLOAD_EXECUTION_WORKFLOW,
            "evidence": evidence,
            "evidence_status": evidence.evidence_status,
            "evidence_digest": evidence.evidence_digest,
            "receipt_registry": receipt_registry,
            "artifact_registry": artifact_registry,
            "execution": "not_started",
            "readiness_claimed": false,
            "guarantees": [
                "caller-reported execution observations are compared with retained receipt metadata",
                "missing observations and mismatches remain visible rather than becoming success",
                "the core never contacts providers, stores, locators, credentials, or payloads"
            ],
            "does_not_claim": [
                "transfer execution by the core, provider authentication, or cryptographic attestation",
                "payload authenticity, scientific, clinical, causal, provenance, regulatory, or release validity",
                "readiness; every response remains not_started"
            ]
        }))
    }

    /// Return a deterministic joined receipt/lineage/execution projection from the local registry.
    /// This is read-only and never treats a complete join as external execution or readiness.
    pub(super) fn domain_evidence_provider_external_payload_evidence_query(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let encoded = serde_json::to_vec(arguments)
            .map_err(|error| format!("cannot encode external payload evidence query: {error}"))?;
        if encoded.len() > 1_000_000 {
            return Err(
                "external payload evidence query exceeds the 1000000-byte safety bound".into(),
            );
        }
        let request: DomainEvidenceProviderExternalPayloadEvidenceQueryRequest =
            serde_json::from_value(arguments.clone())
                .map_err(|error| format!("invalid external payload evidence query: {error}"))?;
        let (records, generation) = {
            let registry = self
                .artifact_registry
                .lock()
                .map_err(|_| "artifact registry lock is poisoned".to_string())?;
            (registry.records_for_audit(), registry.generation())
        };
        let report =
            query_domain_evidence_provider_external_payload_evidence(&records, generation, request)
                .map_err(|error| format!("external payload evidence query refused: {error}"))?;
        serde_json::to_value(report)
            .map_err(|error| format!("cannot encode external payload evidence query: {error}"))
    }
}
