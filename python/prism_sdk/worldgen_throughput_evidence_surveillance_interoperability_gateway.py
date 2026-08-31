"""Python parity surface for AFA-worldgen-P01-F23."""
from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping, Sequence

from .worldgen_throughput_evidence_surveillance_research_workbench import render_throughput_evidence_surveillance_research_workbench
from .research_contracts import PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION, ResearchContractError, research_artifact_digest

FEATURE_ID = "AFA-worldgen-P01-F23"
CONTRACT_VERSION = "worldgen-throughput-evidence-surveillance-interoperability-gateway/1.0"
INPUT_SCHEMA = "EvidenceFeed3@1"
OUTPUT_SCHEMA = "QualifiedEvidenceSet6@1"
TARGET_PROTOCOL_VERSION = "1.0.0"


@dataclass(frozen=True)
class ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt:
    request_id: str; endpoint_id: str; negotiated_version: str; disposition: str
    input_schema: str; output_schema: str; migration_policy: str; semantic_loss_budget: int
    capability_order: tuple[str, ...]; artifact_digest_order: tuple[str, ...]; semantic_loss_order: tuple[str, ...]
    omissions: tuple[str, ...]; uncertainty: tuple[str, ...]; workbench_digest: str; protocol_digest: str
    effect_receipts: tuple[str, ...]; artifact: dict[str, Any]
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION; contract_version: str = CONTRACT_VERSION
    feature_id: str = FEATURE_ID; raw_data_local: bool = True; boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if ((self.schema_version, self.contract_version, self.feature_id) != (RESEARCH_CONTRACT_SCHEMA_VERSION, CONTRACT_VERSION, FEATURE_ID)
            or self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip()
            or not self.endpoint_id.strip() or not self.negotiated_version.strip() or self.input_schema != INPUT_SCHEMA
            or self.output_schema != OUTPUT_SCHEMA or not self.migration_policy.strip() or not self.capability_order
            or not self.effect_receipts or self.semantic_loss_budget <= 0):
            raise ResearchContractError("gateway identity, schemas, capabilities, locality, budget, or effects are incomplete")
        for values in (self.capability_order, self.artifact_digest_order, self.semantic_loss_order, self.omissions, self.uncertainty):
            if tuple(sorted(set(values))) != values: raise ResearchContractError("gateway output ordering is not canonical")
        for value in (self.workbench_digest, self.protocol_digest, self.artifact.get("content_hash")):
            if not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value): raise ResearchContractError("gateway digest is invalid")
        if any(effect not in {"exchange:permitted-artifact-digests-only", "block:unsafe-release"} for effect in self.effect_receipts):
            raise ResearchContractError("gateway effect is outside the exchange gate")


def render_throughput_evidence_surveillance_interoperability_gateway(*, protocol: Mapping[str, Any], workbench_kwargs: Mapping[str, Any], requested_input_schema: str = INPUT_SCHEMA, requested_output_schema: str = OUTPUT_SCHEMA, migration_policy: str = "additive-minor-with-loss-receipt", semantic_loss_budget: int = 4, boundary: str = PRECLINICAL_BOUNDARY) -> ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt:
    if requested_input_schema != INPUT_SCHEMA or requested_output_schema != OUTPUT_SCHEMA or not migration_policy.strip() or semantic_loss_budget <= 0 or boundary != PRECLINICAL_BOUNDARY:
        raise ResearchContractError("protocol schemas, migration policy, loss budget, or boundary are invalid")
    if protocol.get("boundary") != PRECLINICAL_BOUNDARY or not protocol.get("raw_data_local", False):
        raise ResearchContractError("protocol locality and boundary are required")
    request_id = str(protocol.get("request_id", "")); endpoint_id = str(protocol.get("endpoint_id", "")); source_version = str(protocol.get("source_contract_version", "")); target_version = str(protocol.get("target_contract_version", TARGET_PROTOCOL_VERSION))
    offered = tuple(sorted(set(map(str, protocol.get("offered_capabilities", ())))) ); target_caps = tuple(sorted(set(map(str, protocol.get("target_capabilities", ()))))); artifacts = tuple(sorted(set(map(str, protocol.get("artifact_digests", ())))))
    if not request_id.strip() or not endpoint_id.strip() or not source_version.strip() or not offered:
        raise ResearchContractError("protocol identity and offered capabilities are required")
    omissions = []; uncertainty = []; loss = []
    missing = tuple(x for x in target_caps if x not in offered)
    if missing: omissions.append("target capabilities unavailable: " + ",".join(missing)); uncertainty.append("parity cannot be established for unavailable capabilities")
    if target_version == TARGET_PROTOCOL_VERSION and source_version == TARGET_PROTOCOL_VERSION: disposition = "accepted"; negotiated = TARGET_PROTOCOL_VERSION
    elif target_version == TARGET_PROTOCOL_VERSION and source_version == "0.9.0" and TARGET_PROTOCOL_VERSION in protocol.get("supported_contract_versions", ()):
        disposition = "migrated"; negotiated = TARGET_PROTOCOL_VERSION; omissions.append("legacy fields outside pinned target remain unknown"); loss.append("legacy_fields")
    else: disposition = "incompatible"; negotiated = target_version; uncertainty.append("source and target are outside the pinned compatibility window")
    if not protocol.get("policy_allow", False) or not protocol.get("permitted_export", False): disposition = "blocked"; omissions.append("policy or endpoint authorization denied artifact exchange")
    elif not protocol.get("protected_closure", False): disposition = "approval_required"; uncertainty.append("protected closure is incomplete")
    elif missing: disposition = "unknown"
    if len(loss) > semantic_loss_budget: raise ResearchContractError("semantic-loss budget exceeded")
    wb = render_throughput_evidence_surveillance_research_workbench(**dict(workbench_kwargs))
    omissions = tuple(sorted(set(omissions) | set(wb.omissions))); uncertainty = tuple(sorted(set(uncertainty) | set(wb.uncertainty)))
    effect = "block:unsafe-release" if disposition in {"blocked", "incompatible", "approval_required", "unknown"} else "exchange:permitted-artifact-digests-only"
    payload = {"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request_id, "endpoint_id": endpoint_id, "negotiated_version": negotiated, "disposition": disposition, "input_schema": requested_input_schema, "output_schema": requested_output_schema, "migration_policy": migration_policy, "semantic_loss_budget": semantic_loss_budget, "capability_order": list(offered), "artifact_digest_order": list(artifacts), "semantic_loss_order": list(loss), "omissions": list(omissions), "uncertainty": list(uncertainty), "workbench_digest": wb.workbench_digest, "raw_data_local": True, "boundary": PRECLINICAL_BOUNDARY}
    protocol_digest = research_artifact_digest(payload)
    receipt = ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt(request_id=request_id, endpoint_id=endpoint_id, negotiated_version=negotiated, disposition=disposition, input_schema=requested_input_schema, output_schema=requested_output_schema, migration_policy=migration_policy, semantic_loss_budget=semantic_loss_budget, capability_order=offered, artifact_digest_order=artifacts, semantic_loss_order=tuple(loss), omissions=omissions, uncertainty=uncertainty, workbench_digest=wb.workbench_digest, protocol_digest=protocol_digest, effect_receipts=(effect,), artifact={"content_hash": research_artifact_digest({**payload, "protocol_digest": protocol_digest}), "media_type": "application/vnd.aurora.worldgen-throughput-evidence-surveillance-interoperability+json"})
    receipt.validate(); return receipt


__all__ = ["FEATURE_ID", "CONTRACT_VERSION", "TARGET_PROTOCOL_VERSION", "ThroughputEvidenceSurveillanceInteroperabilityGatewayReceipt", "render_throughput_evidence_surveillance_interoperability_gateway"]



