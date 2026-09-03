"""Python mirror of the biolang publication-copilot receipt."""

from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any, Mapping

from .research_contracts import (
    BIOLANG_PUBLICATION_COPILOT_CONTRACT_VERSION,
    BIOLANG_PUBLICATION_COPILOT_FEATURE_ID,
    PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
    ResearchContractError,
    research_artifact_digest,
)


@dataclass(frozen=True)
class BiolangPublicationCopilotReceipt:
    """Cross-language validator for bounded publication preparation."""

    request_id: str
    workflow_id: str
    scope: str
    disposition: str
    ranked_order: tuple[str, ...]
    admitted_order: tuple[str, ...]
    blocked_order: tuple[str, ...]
    unknown_order: tuple[str, ...]
    release_order: tuple[str, ...]
    artifact_order: tuple[str, ...]
    evidence_order: tuple[str, ...]
    tool_invocation_order: tuple[str, ...]
    provenance_order: tuple[str, ...]
    replay_order: tuple[str, ...]
    benchmark_order: tuple[str, ...]
    omissions: tuple[str, ...]
    uncertainty: tuple[str, ...]
    negative_evidence: tuple[str, ...]
    replay_identity: str
    benchmark_digest: str | None
    effect_receipts: tuple[str, ...]
    objects: tuple[Mapping[str, Any], ...]
    publication_artifact: Mapping[str, Any]
    feature_id: str = BIOLANG_PUBLICATION_COPILOT_FEATURE_ID
    contract_version: str = BIOLANG_PUBLICATION_COPILOT_CONTRACT_VERSION
    schema_version: str = RESEARCH_CONTRACT_SCHEMA_VERSION
    raw_data_local: bool = True
    boundary: str = PRECLINICAL_BOUNDARY

    def validate(self) -> None:
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION or self.feature_id != BIOLANG_PUBLICATION_COPILOT_FEATURE_ID or self.contract_version != BIOLANG_PUBLICATION_COPILOT_CONTRACT_VERSION:
            raise ResearchContractError("biolang publication copilot schema, feature, or version mismatch")
        if self.boundary != PRECLINICAL_BOUNDARY or not self.raw_data_local or not self.request_id.strip() or not self.workflow_id.strip() or not self.scope.strip() or not self.ranked_order or not self.effect_receipts:
            raise ResearchContractError("publication copilot identity, ranking, locality, or effects are incomplete")
        if self.disposition not in {"qualified", "partial", "unknown", "blocked"}:
            raise ResearchContractError("publication copilot disposition is unknown")
        if any(value not in self.ranked_order for value in (*self.admitted_order, *self.blocked_order, *self.unknown_order)):
            raise ResearchContractError("publication copilot candidate state is not covered by ranking")
        for values in (self.ranked_order, self.admitted_order, self.blocked_order, self.unknown_order, self.release_order, self.artifact_order, self.evidence_order, self.tool_invocation_order, self.omissions, self.uncertainty, self.negative_evidence, self.effect_receipts):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("publication copilot ordering is invalid")
        for values in (self.provenance_order, self.replay_order, self.benchmark_order):
            if tuple(sorted(set(values))) != values:
                raise ResearchContractError("publication copilot digest ordering is invalid")
        digests = (*self.provenance_order, *self.replay_order, *self.benchmark_order, self.replay_identity, self.publication_artifact.get("content_hash"))
        if self.benchmark_digest is not None:
            digests += (self.benchmark_digest,)
        if any(not isinstance(value, str) or not re.fullmatch(r"[0-9a-f]{64}", value) for value in digests):
            raise ResearchContractError("publication copilot digest is invalid")
        for obj in self.objects:
            if obj.get("raw_data_local") is not True or obj.get("boundary") != PRECLINICAL_BOUNDARY or not str(obj.get("run_id", "")).strip() or not str(obj.get("release_id", "")).strip() or not obj.get("artifact_ids") or not obj.get("evidence_receipt_ids"):
                raise ResearchContractError("signed research object is incomplete or non-local")
        if self.admitted_order and any(not effect.startswith("invoke:declared-tools:") for effect in self.effect_receipts):
            raise ResearchContractError("admitted releases require a declared-tool invocation receipt")
        if not self.admitted_order and self.effect_receipts != ("block:unsafe-release",):
            raise ResearchContractError("empty publication result must be explicitly blocked")

    def digest(self) -> str:
        self.validate()
        return research_artifact_digest({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
            "scope": self.scope,
            "disposition": self.disposition,
            "ranked_order": list(self.ranked_order),
            "admitted_order": list(self.admitted_order),
            "blocked_order": list(self.blocked_order),
            "unknown_order": list(self.unknown_order),
            "release_order": list(self.release_order),
            "artifact_order": list(self.artifact_order),
            "evidence_order": list(self.evidence_order),
            "tool_invocation_order": list(self.tool_invocation_order),
            "provenance_order": list(self.provenance_order),
            "replay_order": list(self.replay_order),
            "benchmark_order": list(self.benchmark_order),
            "omissions": list(self.omissions),
            "uncertainty": list(self.uncertainty),
            "negative_evidence": list(self.negative_evidence),
            "replay_identity": self.replay_identity,
            "benchmark_digest": self.benchmark_digest,
            "effect_receipts": list(self.effect_receipts),
            "objects": [dict(obj) for obj in self.objects],
            "publication_artifact": dict(self.publication_artifact),
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        })
