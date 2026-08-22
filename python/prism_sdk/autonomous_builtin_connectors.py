"""Credentialless built-in connector adapters for offline autonomous execution.

The connector registry and durable worker deliberately stop at a caller-owned executor
boundary.  That is the right security boundary for external providers, but it left the
Python SDK without a useful adapter for local development, deterministic evaluation, and
air-gapped deployments.  This module supplies that missing middle layer.

The built-in adapter accepts caller-supplied JSON metadata and returns a transient,
metadata-only observation.  It never opens a socket, discovers a provider, interprets a
credential, or claims that a local fixture is external evidence.  Its purpose is to make the
same exact operation contracts usable before an embedding application wires a real connector.

Every operation is digest-bound to one of the twelve autonomous domains.  The adapter projects
field names, shapes, counts, and SHA-256 digests; it never echoes the supplied values.  A
receipt journal therefore remains safe to persist, while the caller can still use the transient
observation to drive an offline planner, evaluator, replay harness, or local test.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from .authoring import content_digest
from .autonomous_connector_worker import (
    AutonomousConnectorOperationRegistry,
)
from .autonomous_connectors import (
    AutonomousConnectorObservation,
    AutonomousConnectorRegistration,
    AutonomousConnectorRegistry,
)
from .domain_evidence_provider_handoff import (
    DomainEvidenceProviderAuthPosture,
    DomainEvidenceProviderConnectorManifest,
)
from .domain_tools import (
    AUTONOMOUS_DOMAIN_NAMES,
    _identifier,
    _json_safe,
    _reject_secret_fields,
)
from .errors import ArgumentError


AUTONOMOUS_BUILTIN_CONNECTOR_SCHEMA = "bioprism-python-autonomous-builtin-connector/0.1"
AUTONOMOUS_BUILTIN_CONNECTOR_ID = "builtin.offline-evidence"
AUTONOMOUS_BUILTIN_CONNECTOR_VERSION = "1.0.0"
AUTONOMOUS_BUILTIN_CONNECTOR_PROVIDER = "local-offline"
MAX_AUTONOMOUS_BUILTIN_INPUT_BYTES = 2_000_000
MAX_AUTONOMOUS_BUILTIN_FIELDS = 128
MAX_AUTONOMOUS_BUILTIN_FIELD_NAME_BYTES = 256
MAX_AUTONOMOUS_BUILTIN_SEQUENCE_ITEMS = 1_024
MAX_AUTONOMOUS_BUILTIN_SHAPE_DEPTH = 16


# These are review prompts for an evaluator, not hard validation requirements.  The adapter
# can therefore ingest a sparse fixture and report ``partial`` instead of inventing evidence.
_RECOMMENDED_FIELDS: dict[str, tuple[str, ...]] = {
    "coding.repository_change_analysis": ("repository_digest", "changed_files", "test_results"),
    "browser.web_evidence_retrieval": ("source_digests", "retrieved_at", "citation_metadata"),
    "data.dataset_quality_profile": ("schema", "row_count", "column_count", "lineage"),
    "science.reproducible_evidence_acquisition": ("hypothesis", "evidence_digests", "analysis_digest"),
    "biomedical.clinical_data_review": ("provenance", "cohort_digest", "review_questions"),
    "neuroscience.signal_study_analysis": ("signal_digest", "sampling_rate", "study_design"),
    "operations.incident_runbook_observation": ("incident_digest", "telemetry_digest", "runbook_digest"),
    "enterprise.workflow_record_governance": ("workflow_digest", "record_type", "policy_digest"),
    "multi_agent.delegated_consensus_handoff": ("delegation_digest", "agent_digests", "conflicts"),
    "multimodal.asset_alignment": ("modalities", "asset_digests", "alignment_digest"),
    "cross_domain.evidence_fanout_synthesis": ("domain_digests", "evidence_digests", "route_digest"),
    "evaluation.benchmark_replay_analysis": ("benchmark_digest", "case_count", "replay_digest"),
}

_IDENTITY_FIELDS = frozenset({"operation_id", "subject_digest"})


def _digest(name: str, value: Any) -> str:
    if not isinstance(value, str) or len(value) != 64 or any(
        character not in "0123456789abcdef" for character in value
    ):
        raise ArgumentError(f"{name} must be a lowercase SHA-256 digest")
    return value


def _bounded_field_name(value: Any) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise ArgumentError("built-in connector field names must be non-empty strings")
    if len(value.encode("utf-8")) > MAX_AUTONOMOUS_BUILTIN_FIELD_NAME_BYTES:
        raise ArgumentError("built-in connector field name exceeds its bound")
    return value


def _shape(value: Any, *, depth: int = 0) -> dict[str, Any]:
    """Return a bounded shape descriptor without retaining any input value."""

    if depth > MAX_AUTONOMOUS_BUILTIN_SHAPE_DEPTH:
        return {"type": "depth_limited"}
    if value is None:
        return {"type": "null"}
    if isinstance(value, bool):
        return {"type": "boolean"}
    if isinstance(value, str):
        return {"type": "string", "bytes": len(value.encode("utf-8"))}
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return {"type": "number"}
    if isinstance(value, Mapping):
        keys = tuple(str(key) for key in value)
        return {
            "type": "object",
            "field_count": len(keys),
            "field_names_digest": content_digest(sorted(keys)),
        }
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return {
            "type": "array",
            "item_count": len(value),
            "item_types": sorted({
                _shape(item, depth=depth + 1).get("type", "unknown")
                for item in value[:MAX_AUTONOMOUS_BUILTIN_SEQUENCE_ITEMS]
            }),
        }
    return {"type": type(value).__name__}


def _content_present(value: Any) -> bool:
    if value is None or value == "":
        return False
    if isinstance(value, Mapping):
        return any(_content_present(child) for child in value.values())
    if isinstance(value, Sequence) and not isinstance(value, (str, bytes, bytearray)):
        return any(_content_present(child) for child in value)
    return True


def _field_projection(request: Mapping[str, Any]) -> tuple[dict[str, Any], ...]:
    fields: list[dict[str, Any]] = []
    for raw_name in sorted(request):
        name = _bounded_field_name(raw_name)
        if name in _IDENTITY_FIELDS:
            continue
        value = request[name]
        fields.append(
            {
                "name": name,
                "digest": content_digest(value),
                "shape": _shape(value),
                "present": _content_present(value),
            }
        )
    if len(fields) > MAX_AUTONOMOUS_BUILTIN_FIELDS:
        raise ArgumentError("built-in connector request contains too many fields")
    return tuple(fields)


class AutonomousBuiltinConnectorAdapter:
    """Deterministic local executor for the full built-in operation catalogue."""

    def __init__(
        self,
        operation_registry: AutonomousConnectorOperationRegistry | None = None,
        *,
        connector_id: str = AUTONOMOUS_BUILTIN_CONNECTOR_ID,
        version: str = AUTONOMOUS_BUILTIN_CONNECTOR_VERSION,
    ) -> None:
        self.operation_registry = operation_registry or AutonomousConnectorOperationRegistry()
        if not isinstance(self.operation_registry, AutonomousConnectorOperationRegistry):
            raise ArgumentError("built-in connector operation_registry is invalid")
        self.connector_id = _identifier("built-in connector connector_id", connector_id)
        self.version = _identifier("built-in connector version", version)
        missing = sorted(
            set(contract.operation_id for contract in self.operation_registry.operations())
            .difference(_RECOMMENDED_FIELDS)
        )
        if missing:
            raise ArgumentError("built-in connector has no field profile for: " + ", ".join(missing))

    @property
    def domains(self) -> tuple[str, ...]:
        return tuple(AUTONOMOUS_DOMAIN_NAMES)

    @property
    def capabilities(self) -> tuple[str, ...]:
        # The handoff manifest intentionally caps capability labels at 64. Preserve the first
        # (primary) capability of every operation, then fill the remaining budget
        # deterministically with secondary aliases. The operation registry remains the
        # authority for the complete vocabulary; this is only the wire-level projection.
        operations = self.operation_registry.operations()
        primary = tuple(dict.fromkeys(operation.capabilities[0] for operation in operations))
        secondary = tuple(
            capability
            for capability in sorted({item for operation in operations for item in operation.capabilities})
            if capability not in primary
        )
        return (primary + secondary)[:64]

    def manifest(self) -> DomainEvidenceProviderConnectorManifest:
        return DomainEvidenceProviderConnectorManifest(
            connector_id=self.connector_id,
            version=self.version,
            provider=AUTONOMOUS_BUILTIN_CONNECTOR_PROVIDER,
            connector_kind="provider_api",
            domains=self.domains,
            capabilities=self.capabilities,
            auth_posture=DomainEvidenceProviderAuthPosture(
                status="none",
                secret_refs=(),
                does_not_claim=(
                    "no external provider was contacted",
                    "caller-supplied metadata is not independently verified",
                    "no credential material is accepted or retained",
                ),
            ),
        )

    def execute(
        self,
        manifest: DomainEvidenceProviderConnectorManifest,
        request: Mapping[str, Any],
    ) -> AutonomousConnectorObservation:
        if not isinstance(manifest, DomainEvidenceProviderConnectorManifest):
            raise ArgumentError("built-in connector manifest is invalid")
        if manifest.connector_id != self.connector_id or manifest.version != self.version:
            raise ArgumentError("built-in connector manifest identity does not match the adapter")
        if not isinstance(request, Mapping):
            raise ArgumentError("built-in connector request must be an object")
        safe_request = _json_safe(
            "built-in connector request",
            dict(request),
            maximum=MAX_AUTONOMOUS_BUILTIN_INPUT_BYTES,
        )
        _reject_secret_fields(safe_request)
        operation_id = _identifier("built-in connector operation_id", safe_request.get("operation_id"))
        contract = self.operation_registry.resolve(operation_id)
        subject_digest = _digest("built-in connector subject_digest", safe_request.get("subject_digest"))
        fields = _field_projection(safe_request)
        # Presence is based on an explicit field, not truthiness. Empty conflict lists,
        # zero counts, and nullability decisions are meaningful caller evidence and must not
        # be silently relabeled as omitted input.
        available_fields = tuple(field["name"] for field in fields)
        recommended_fields = _RECOMMENDED_FIELDS[contract.operation_id]
        missing_fields = tuple(field for field in recommended_fields if field not in available_fields)
        has_evidence = any(field["present"] for field in fields)
        status = "observed" if has_evidence and not missing_fields else "partial"
        field_digests = {field["name"]: field["digest"] for field in fields}
        field_shapes = {field["name"]: field["shape"] for field in fields}
        value = {
            "schema": AUTONOMOUS_BUILTIN_CONNECTOR_SCHEMA,
            "operation_id": contract.operation_id,
            "domain": contract.domain,
            "subject_digest": subject_digest,
            "operation_digest": contract.operation_digest,
            "operation_capabilities": list(contract.capabilities),
            "evaluator_signals": list(contract.evaluator_signals),
            "recommended_fields": list(recommended_fields),
            "available_fields": list(available_fields),
            "missing_fields": list(missing_fields),
            "field_digests": field_digests,
            "field_shapes": field_shapes,
            "field_count": len(fields),
            "input_digest": content_digest(safe_request),
            "status": status,
            "failure_class": None if status == "observed" else "incomplete_local_fixture",
            "evidence_posture": "caller_supplied_metadata;offline_fixture;not_external_validation",
            "effect_posture": "read_only;no_network;no_provider_invocation",
            "retention": "transient_metadata_projection;receipt_retains_digest_only",
            "secret_material": "never_accepted_or_returned",
        }
        return AutonomousConnectorObservation(
            value=value,
            status=status,
            failure_class=value["failure_class"],
        )

    def __call__(
        self,
        manifest: DomainEvidenceProviderConnectorManifest,
        request: Mapping[str, Any],
    ) -> AutonomousConnectorObservation:
        return self.execute(manifest, request)


def builtin_autonomous_connector_registration(
    operation_registry: AutonomousConnectorOperationRegistry | None = None,
    *,
    connector_id: str = AUTONOMOUS_BUILTIN_CONNECTOR_ID,
    version: str = AUTONOMOUS_BUILTIN_CONNECTOR_VERSION,
    approval_required: bool = True,
) -> AutonomousConnectorRegistration:
    """Create the reviewed all-domain registration without mutating a registry."""

    if not isinstance(approval_required, bool):
        raise ArgumentError("built-in connector approval_required must be a boolean")
    adapter = AutonomousBuiltinConnectorAdapter(
        operation_registry,
        connector_id=connector_id,
        version=version,
    )
    return AutonomousConnectorRegistration(adapter.manifest(), adapter, approval_required=approval_required)


def register_builtin_autonomous_connectors(
    registry: AutonomousConnectorRegistry,
    operation_registry: AutonomousConnectorOperationRegistry | None = None,
    *,
    connector_id: str = AUTONOMOUS_BUILTIN_CONNECTOR_ID,
    version: str = AUTONOMOUS_BUILTIN_CONNECTOR_VERSION,
    approval_required: bool = True,
    replace: bool = False,
) -> AutonomousConnectorRegistration:
    """Register the deterministic all-domain adapter in a caller-owned connector registry."""

    if not isinstance(registry, AutonomousConnectorRegistry):
        raise ArgumentError("built-in connector registration requires an AutonomousConnectorRegistry")
    registration = builtin_autonomous_connector_registration(
        operation_registry,
        connector_id=connector_id,
        version=version,
        approval_required=approval_required,
    )
    registry.register(registration, replace=replace)
    return registration


__all__ = [
    "AUTONOMOUS_BUILTIN_CONNECTOR_SCHEMA",
    "AUTONOMOUS_BUILTIN_CONNECTOR_ID",
    "AUTONOMOUS_BUILTIN_CONNECTOR_VERSION",
    "AUTONOMOUS_BUILTIN_CONNECTOR_PROVIDER",
    "MAX_AUTONOMOUS_BUILTIN_INPUT_BYTES",
    "MAX_AUTONOMOUS_BUILTIN_FIELDS",
    "MAX_AUTONOMOUS_BUILTIN_FIELD_NAME_BYTES",
    "MAX_AUTONOMOUS_BUILTIN_SEQUENCE_ITEMS",
    "MAX_AUTONOMOUS_BUILTIN_SHAPE_DEPTH",
    "AutonomousBuiltinConnectorAdapter",
    "builtin_autonomous_connector_registration",
    "register_builtin_autonomous_connectors",
]
